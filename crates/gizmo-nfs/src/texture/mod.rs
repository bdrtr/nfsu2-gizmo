//! Parse NFSU2 `TEXTURES.BIN` (TPK) into an RGBA8 pixel pool + per-texture descriptors.
//!
//! The format was reverse-engineered empirically against real cars (decoding the pool and
//! rendering it produced recognisable 240SX textures — number plate, head-lamps, mirror).
//!
//! # Layout
//!
//! ```text
//! 0xB3300000                        TPK root
//!   0xB3310000                      directory
//!     0x33310001                    info: name + source .tpk path
//!     0x33310002                    hash table: N × (u32 hash, u32 = 0)
//!     0x33310003  ← DESCRIPTORS     N × 24-byte descriptor (below)
//!   0xB3320000                      data
//!     0x33320001                    24-byte data header
//!     0x33320002  ← PIXEL_DATA      0x11 padding, then back-to-back JDLZ blocks
//! ```
//!
//! **Pixel pool.** The `0x33320002` payload is leading `0x11` alignment padding followed by
//! a run of [`crate::compression::jdlz`] blocks. Decompressing each block and concatenating
//! yields the **RGBA8 pixel pool** (4 bytes/pixel — this format is *not* DXT). The pool is
//! organised as pages [`PAGE_WIDTH`] pixels wide; textures are shelf-packed rectangles
//! within it.
//!
//! **Descriptor** (`0x33310003`, 24 bytes, little-endian `u32` fields):
//!
//! | field | offset | meaning |
//! |-------|--------|---------|
//! | hash          | 0  | asset hash (matches the hash table) |
//! | `pool_offset` | 4  | **byte offset of the texture's top-left pixel in the pool** → `(x, y)` at [`PAGE_WIDTH`] |
//! | `field2`      | 8  | not yet confirmed (block-local offset / size candidate) |
//! | `block_size`  | 12 | decompressed size of the JDLZ block this texture belongs to |
//! | `format_code` | 16 | constant `0x100` on every sample → RGBA8888 ([`FORMAT_RGBA8`]) |
//! | reserved      | 20 | constant `0` |
//!
//! # Not yet decoded
//!
//! Per-texture **width/height** are not present in any identified descriptor field
//! (`format_code` is a format tag, not a dimension). Candidates still to resolve: the x-gap
//! to the next shelf-neighbour, `field2`, or the extent of the geometry's UVs that reference
//! the texture. Until then this module exposes the pool and each texture's *origin*, not
//! cropped images — so it deliberately does **not** yet build [`crate::NfsTexture`].

use crate::chunk::ChunkNode;
use crate::compression::jdlz;
use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;
use crate::types::{AssetHash, NfsTexture, PixelFormat, TexFormat};

/// Descriptor table chunk id (`N × 24` bytes).
const DESCRIPTORS: u32 = 0x3331_0003;
/// Pixel-data chunk id (padding + JDLZ blocks).
const PIXEL_DATA: u32 = 0x3332_0002;
/// Bytes per descriptor entry.
const DESCRIPTOR_STRIDE: usize = 24;

/// Width, in pixels, of the RGBA pages the pixel pool is laid out at (empirically 512).
pub const PAGE_WIDTH: u32 = 512;
/// The `format_code` value seen on every texture: `0x100` = RGBA8888.
pub const FORMAT_RGBA8: u32 = 0x100;
/// Bytes per pixel in the decoded pool.
pub const BYTES_PER_PIXEL: usize = 4;

/// Refuse to assemble a pool larger than this from the (untrusted) block stream. A real car
/// TPK decompresses to well under 16 MB; this leaves generous headroom without letting a
/// crafted file exhaust memory.
const MAX_POOL: usize = 128 * 1024 * 1024;

/// One texture's descriptor within a [`Tpk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TpkEntry {
    /// The texture's asset hash (its key; matches a `material_ref` on a mesh part).
    pub hash: AssetHash,
    /// Byte offset of the texture's top-left pixel within [`Tpk::pool`].
    pub pool_offset: u32,
    /// Descriptor field 2 — meaning not yet confirmed.
    pub field2: u32,
    /// Decompressed size of the JDLZ block this texture belongs to (descriptor field 3).
    pub block_size: u32,
    /// Pixel-format code (descriptor field 4); [`FORMAT_RGBA8`] on every sample seen.
    pub format_code: u32,
}

impl TpkEntry {
    /// The texture's top-left `(x, y)` in the [`PAGE_WIDTH`]-wide pool, in pixels.
    ///
    /// This is only the origin: the texture's width/height are not yet decoded, so a full
    /// rectangle cannot be cropped from the pool from this alone.
    #[must_use]
    pub fn origin(&self) -> (u32, u32) {
        let px = self.pool_offset / BYTES_PER_PIXEL as u32;
        (px % PAGE_WIDTH, px / PAGE_WIDTH)
    }
}

/// A parsed TPK: the assembled RGBA8 pixel pool plus per-texture descriptors.
///
/// The pool is the concatenation of the file's JDLZ-decompressed pixel blocks, laid out as
/// [`PAGE_WIDTH`]-wide RGBA8 pages. Per-texture width/height are not yet decoded (see the
/// module docs), so this exposes the pool and each texture's [`TpkEntry::origin`] rather
/// than cropped images.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Tpk {
    /// RGBA8 pixel pool (`4` bytes/pixel), laid out as [`PAGE_WIDTH`]-wide pages.
    pub pool: Vec<u8>,
    /// Per-texture descriptors, in file order.
    pub entries: Vec<TpkEntry>,
}

impl Tpk {
    /// Parse a (raw, on-disk) `TEXTURES.BIN` buffer.
    ///
    /// Returns an error if the TPK's descriptor or pixel chunks are absent or malformed. A
    /// pixel blob with no recognisable JDLZ block yields an empty [`pool`](Tpk::pool) rather
    /// than an error, so a partially-understood file still surfaces its descriptors.
    pub fn parse(bytes: &[u8]) -> NfsResult<Tpk> {
        let roots = ChunkNode::parse(bytes)?;
        let desc = find_leaf(&roots, DESCRIPTORS, bytes)
            .ok_or(NfsError::CorruptArchive { detail: "TPK missing descriptor chunk 0x33310003" })?;
        let blob = find_leaf(&roots, PIXEL_DATA, bytes)
            .ok_or(NfsError::CorruptArchive { detail: "TPK missing pixel chunk 0x33320002" })?;
        let pool = assemble_pool(blob)?;
        let entries = parse_descriptors(desc)?;
        Ok(Tpk { pool, entries })
    }

    /// Height in pixels of the pool when viewed as a single [`PAGE_WIDTH`]-wide image
    /// (`pool_len / (PAGE_WIDTH * 4)`, rounded down).
    #[must_use]
    pub fn pool_rows(&self) -> u32 {
        (self.pool.len() / (PAGE_WIDTH as usize * BYTES_PER_PIXEL)) as u32
    }

    /// Look up a texture descriptor by its asset hash.
    #[must_use]
    pub fn entry(&self, hash: AssetHash) -> Option<&TpkEntry> {
        self.entries.iter().find(|e| e.hash == hash)
    }

    /// Crop the texture keyed by `hash` out of the pool as a tight RGBA8 [`NfsTexture`].
    ///
    /// **Heuristic**, pending the real width/height decode: the descriptor gives only the
    /// texture's top-left origin, so the extent is recovered from the pixels — textures sit
    /// on a transparent (`alpha ≈ 0x3b`) background, so the returned rectangle is the opaque
    /// content region growing from the origin, stopping at the first fully-transparent
    /// separator row/column. Good for the mostly-opaque detail textures (head-lamps, badges,
    /// plates); a texture with genuinely transparent interior padding may crop slightly
    /// tight. Returns `None` if the hash is unknown or the region holds no opaque pixels.
    #[must_use]
    pub fn texture(&self, hash: AssetHash) -> Option<NfsTexture> {
        let entry = self.entry(hash)?;
        let (x0, y0) = entry.origin();
        let (rx, ry, w, h) = self.content_rect(x0 as usize, y0 as usize)?;
        let page_w = PAGE_WIDTH as usize;
        let mut rgba = Vec::with_capacity(w * h * BYTES_PER_PIXEL);
        for row in 0..h {
            let start = ((ry + row) * page_w + rx) * BYTES_PER_PIXEL;
            let end = start + w * BYTES_PER_PIXEL;
            rgba.extend_from_slice(self.pool.get(start..end)?);
        }
        Some(NfsTexture {
            name: String::new(),
            hash,
            width: w as u32,
            height: h as u32,
            rgba,
            source_format: TexFormat::Unknown(entry.format_code),
            format: PixelFormat::Rgba8,
        })
    }

    /// Find the opaque content rectangle `(x, y, w, h)` for a texture whose top-left origin
    /// is `(x0, y0)` — see [`texture`]. Seeds on the first opaque pixel at/after the origin
    /// and flood-fills the connected opaque blob, returning its bounding box.
    fn content_rect(&self, x0: usize, y0: usize) -> Option<(usize, usize, usize, usize)> {
        const WINDOW: usize = 320;
        // The inter-texture background is a low-alpha fill (~0x3b); real texture pixels are
        // opaque (0xff). Threshold between the two so the flood-fill can't leak through it.
        const ALPHA_MIN: u8 = 128;
        let page_w = PAGE_WIDTH as usize;
        let rows = self.pool_rows() as usize;
        let x_hi = (x0 + WINDOW).min(page_w);
        let y_hi = (y0 + WINDOW).min(rows);
        let opaque = |x: usize, y: usize| -> bool {
            let o = (y * page_w + x) * BYTES_PER_PIXEL + 3;
            self.pool.get(o).copied().unwrap_or(0) > ALPHA_MIN
        };

        // A texture's own content is the first *substantial* opaque blob at/after its
        // origin; smaller specks (anti-aliasing crumbs from a neighbour) are skipped.
        const MIN_BLOB: usize = 48;
        let idx = |x: usize, y: usize| (y - y0) * (x_hi - x0) + (x - x0);
        let mut seen = vec![false; (x_hi - x0) * (y_hi - y0)];

        for sy in y0..y_hi {
            for sx in x0..x_hi {
                if seen[idx(sx, sy)] || !opaque(sx, sy) {
                    continue;
                }
                // Flood-fill this 4-connected opaque blob, tracking size and bounding box.
                let (mut min_x, mut min_y, mut max_x, mut max_y) = (sx, sy, sx, sy);
                let mut size = 0usize;
                let mut stack = vec![(sx, sy)];
                seen[idx(sx, sy)] = true;
                while let Some((x, y)) = stack.pop() {
                    size += 1;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    let visit = |nx: usize, ny: usize, st: &mut Vec<(usize, usize)>, sn: &mut [bool]| {
                        if nx >= x0 && nx < x_hi && ny >= y0 && ny < y_hi && !sn[idx(nx, ny)] && opaque(nx, ny) {
                            sn[idx(nx, ny)] = true;
                            st.push((nx, ny));
                        }
                    };
                    if x > x0 { visit(x - 1, y, &mut stack, &mut seen); }
                    visit(x + 1, y, &mut stack, &mut seen);
                    if y > y0 { visit(x, y - 1, &mut stack, &mut seen); }
                    visit(x, y + 1, &mut stack, &mut seen);
                }
                if size >= MIN_BLOB {
                    return Some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1));
                }
            }
        }
        None
    }
}

/// Find the payload of the first chunk (top-level or descendant) with `id`.
fn find_leaf<'a>(roots: &[ChunkNode], id: u32, root_buf: &'a [u8]) -> Option<&'a [u8]> {
    for r in roots {
        if r.header.id == id {
            return Some(r.data(root_buf));
        }
        if let Some(n) = r.find(id) {
            return Some(n.data(root_buf));
        }
    }
    None
}

/// Walk `blob`'s back-to-back JDLZ blocks, decompressing and concatenating them into the
/// RGBA8 pool. Leading padding and any non-JDLZ gap bytes are skipped; a byte run that looks
/// like a `JDLZ` magic but fails to decompress is treated as a false positive and stepped
/// over. Never allocates past [`MAX_POOL`].
fn assemble_pool(blob: &[u8]) -> NfsResult<Vec<u8>> {
    let mut pool = Vec::new();
    let mut p = 0usize;
    while p + 16 <= blob.len() {
        if &blob[p..p + 4] == b"JDLZ" {
            if let Ok(dec) = jdlz::decompress(&blob[p..]) {
                if pool.len().saturating_add(dec.len()) > MAX_POOL {
                    return Err(NfsError::Allocation { requested: pool.len() + dec.len() });
                }
                pool.extend_from_slice(&dec);
                // Advance to the next block by the header's declared compressed size.
                let cs = jdlz::parse_header(&blob[p..]).map(|h| h.compressed_size as usize).unwrap_or(0);
                p += if cs >= 16 { cs } else { 16 };
                continue;
            }
        }
        p += 1;
    }
    Ok(pool)
}

/// Parse the fixed-stride descriptor table. Trailing bytes shorter than one entry are
/// ignored (the table is always a whole number of entries in practice).
fn parse_descriptors(desc: &[u8]) -> NfsResult<Vec<TpkEntry>> {
    let count = desc.len() / DESCRIPTOR_STRIDE;
    let mut entries = Vec::with_capacity(count);
    let mut r = ByteReader::new(desc);
    for _ in 0..count {
        let hash = r.u32_le()?;
        let pool_offset = r.u32_le()?;
        let field2 = r.u32_le()?;
        let block_size = r.u32_le()?;
        let format_code = r.u32_le()?;
        let _reserved = r.u32_le()?;
        entries.push(TpkEntry {
            hash: AssetHash(hash),
            pool_offset,
            field2,
            block_size,
            format_code,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a chunk: 8-byte header (id LE, size LE) then `payload`.
    fn chunk(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    /// A 24-byte descriptor built from its six little-endian u32 fields.
    fn descriptor(fields: [u32; 6]) -> Vec<u8> {
        fields.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    #[test]
    fn parses_descriptors_and_reports_empty_pool_without_jdlz() {
        // pool_offset for (x=288, y=348) at width 512 = (348*512 + 288) * 4 = 713856.
        let d0 = descriptor([0xAABB_CCDD, 713856, 42, 5596, FORMAT_RGBA8, 0]);
        let d1 = descriptor([0x1122_3344, 0, 7, 11036, FORMAT_RGBA8, 0]);
        let mut descs = d0.clone();
        descs.extend_from_slice(&d1);

        let mut file = chunk(DESCRIPTORS, &descs);
        // Pixel chunk with only padding (no JDLZ) → empty pool, still valid.
        file.extend_from_slice(&chunk(PIXEL_DATA, &[0x11; 32]));

        let tpk = Tpk::parse(&file).expect("parse synthetic TPK");
        assert_eq!(tpk.entries.len(), 2);
        assert!(tpk.pool.is_empty(), "no JDLZ block → empty pool");

        let e0 = tpk.entries[0];
        assert_eq!(e0.hash, AssetHash(0xAABB_CCDD));
        assert_eq!(e0.pool_offset, 713856);
        assert_eq!(e0.format_code, FORMAT_RGBA8);
        assert_eq!(e0.origin(), (288, 348));
        assert_eq!(tpk.entry(AssetHash(0x1122_3344)), Some(&tpk.entries[1]));
    }

    #[test]
    fn missing_chunks_are_errors() {
        // Only a descriptor chunk, no pixel data.
        let file = chunk(DESCRIPTORS, &descriptor([1, 2, 3, 4, FORMAT_RGBA8, 0]));
        assert!(matches!(Tpk::parse(&file), Err(NfsError::CorruptArchive { .. })));
    }
}
