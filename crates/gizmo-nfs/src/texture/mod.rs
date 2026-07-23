//! Parse NFSU2 `TEXTURES.BIN` (TPK) into per-texture RGBA8 images.
//!
//! The format was reverse-engineered against real cars, cross-checked with the community
//! `xan1242/xnfstpktool` and `NFSTools/GlobalLib` sources.
//!
//! # Layout
//!
//! ```text
//! 0xB3300000  TPK root
//!   0xB3310000  directory
//!     0x33310001  info (name + source .tpk path)
//!     0x33310002  N × (u32 hash, u32 = 0)
//!     0x33310003  ← DESCRIPTORS: N × 24-byte record (below)
//!   0xB3320000  data
//!     0x33320002  the compressed texture blobs
//! ```
//!
//! **Each texture is independent.** The 24-byte descriptor (`0x33310003`, LE `u32`) is:
//!
//! | off | field | role |
//! |-----|-------|------|
//! | 0x00 | `hash`               | asset key |
//! | 0x04 | `abs_offset`         | **whole-file** byte offset of this texture's compressed blob |
//! | 0x08 | `size`               | compressed byte length at `abs_offset` |
//! | 0x0C | `out_size`           | decompressed size of the blob |
//! | 0x10 | `header_from_end`    | distance from the decompressed end back to the header (const `0x100`) |
//! | 0x14 | `unk`                | ignored |
//!
//! To decode one texture: read `file[abs_offset .. abs_offset + size]`, decompress it (JDLZ
//! or HUFF, by magic) into an `out_size`-byte buffer, then read an embedded `OldTextureInfo`
//! header near its tail for the dimensions and pixel format. Pixels always start at buffer
//! offset 0. The header sits at `P = out_size − header_from_end + 0x64 + 0x24`, where the
//! `u32` at `P` is the texture's own hash (a self-check); from `P`: `Width = u16@P+32`,
//! `Height = u16@P+34`, `ImageCompressionType = u8@P+38`. The image is the *top mip* only,
//! decoded by [`dxt`] (DXT1/3/5) or unpacked directly (RGBA).
//!
//! HUFF-compressed textures are skipped until [`crate::compression::huff`] is implemented.

pub mod dxt;

use crate::chunk::ChunkNode;
use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;
use crate::types::{AssetHash, NfsTexture, PixelFormat, TexFormat};
use std::collections::HashMap;

/// Descriptor table chunk id (`N × 24` bytes).
const DESCRIPTORS: u32 = 0x3331_0003;
/// Bytes per descriptor entry.
const DESCRIPTOR_STRIDE: usize = 24;
/// Bytes per pixel in the decoded output.
const RGBA: usize = 4;

/// `ImageCompressionType` codes (embedded `OldTextureInfo`, byte at `P+38`).
mod fmt {
    pub const RGBA8888: u8 = 0x20; // 32bpp, stored B,G,R,A
    pub const DXT1: u8 = 0x22;
    pub const DXT3: u8 = 0x24;
    pub const DXT5: u8 = 0x26;
    pub const P8: u8 = 0x08; // palettised — not decoded yet
}

/// The largest texture dimension we will decode (guards allocation from a corrupt header).
const MAX_DIM: usize = 4096;

/// One texture's 24-byte descriptor within a [`Tpk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TpkEntry {
    /// The texture's asset hash (its key; matches a `material_ref` on a mesh part).
    pub hash: AssetHash,
    /// Whole-file byte offset of the compressed blob.
    pub abs_offset: u32,
    /// Compressed byte length of the blob.
    pub size: u32,
    /// Decompressed size of the blob.
    pub out_size: u32,
    /// Distance from the decompressed end back to the embedded header (`0x100`).
    pub header_from_end: u32,
}

/// A parsed TPK: the raw per-texture descriptors plus every texture we could decode to RGBA8.
///
/// Textures compressed with a codec that is not yet implemented (HUFF) are present in
/// [`entries`](Tpk::entries) but absent from [`textures`](Tpk::textures).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Tpk {
    /// Every texture descriptor, in file order.
    pub entries: Vec<TpkEntry>,
    /// Decoded RGBA8 textures, keyed by hash.
    pub textures: HashMap<AssetHash, NfsTexture>,
}

impl Tpk {
    /// Parse a (raw, on-disk) `TEXTURES.BIN` buffer, decoding every texture whose codec is
    /// supported. Returns an error only if the descriptor chunk is absent or malformed; an
    /// individual texture that fails to decode is skipped, not fatal.
    pub fn parse(bytes: &[u8]) -> NfsResult<Tpk> {
        let roots = ChunkNode::parse(bytes)?;
        let desc = find_leaf(&roots, DESCRIPTORS, bytes)
            .ok_or(NfsError::CorruptArchive { detail: "TPK missing descriptor chunk 0x33310003" })?;
        let entries = parse_descriptors(desc);
        let mut textures = HashMap::new();
        for e in &entries {
            if let Ok(tex) = decode_texture(bytes, e) {
                textures.insert(e.hash, tex);
            }
        }
        Ok(Tpk { entries, textures })
    }

    /// Look up a decoded texture by asset hash.
    #[must_use]
    pub fn texture(&self, hash: AssetHash) -> Option<&NfsTexture> {
        self.textures.get(&hash)
    }

    /// Look up a texture descriptor by asset hash (present even for un-decodable codecs).
    #[must_use]
    pub fn entry(&self, hash: AssetHash) -> Option<&TpkEntry> {
        self.entries.iter().find(|e| e.hash == hash)
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

/// Parse the fixed-stride descriptor table (trailing partial bytes ignored).
fn parse_descriptors(desc: &[u8]) -> Vec<TpkEntry> {
    let count = desc.len() / DESCRIPTOR_STRIDE;
    let mut entries = Vec::with_capacity(count);
    let mut r = ByteReader::new(desc);
    for _ in 0..count {
        // Reads cannot fail: `count` is derived from the actual buffer length.
        let hash = r.u32_le().unwrap_or(0);
        let abs_offset = r.u32_le().unwrap_or(0);
        let size = r.u32_le().unwrap_or(0);
        let out_size = r.u32_le().unwrap_or(0);
        let header_from_end = r.u32_le().unwrap_or(0);
        let _unk = r.u32_le().unwrap_or(0);
        entries.push(TpkEntry {
            hash: AssetHash(hash),
            abs_offset,
            size,
            out_size,
            header_from_end,
        });
    }
    entries
}

/// Decompress and decode a single texture into RGBA8. Errors (unsupported codec, malformed
/// header, out-of-range dimensions) mean "skip this texture", not a corrupt file.
fn decode_texture(file: &[u8], e: &TpkEntry) -> NfsResult<NfsTexture> {
    let abs = e.abs_offset as usize;
    let end = abs.checked_add(e.size as usize).ok_or(NfsError::CorruptArchive {
        detail: "TPK texture offset+size overflow",
    })?;
    let blob = file
        .get(abs..end)
        .ok_or(NfsError::CorruptArchive { detail: "TPK texture blob out of range" })?;

    // JDLZ / HUFF (HUFF currently returns NotImplemented → this texture is skipped).
    let pool = crate::compression::decompress(blob)?;
    let out_size = e.out_size as usize;
    if pool.len() < out_size {
        return Err(NfsError::BufferSizeMismatch { detail: "TPK blob decompressed short" });
    }

    // Locate the embedded OldTextureInfo header and read the fields we need.
    let p = out_size
        .checked_sub(e.header_from_end as usize)
        .and_then(|h| h.checked_add(0x64 + 0x24))
        .ok_or(NfsError::CorruptArchive { detail: "TPK header offset underflow" })?;
    let hdr = pool
        .get(p..p + 39)
        .ok_or(NfsError::CorruptArchive { detail: "TPK header out of range" })?;
    // Self-check: the u32 at P is the texture's own hash. If it isn't, the header formula
    // doesn't apply to this texture — skip rather than decode noise.
    let name_hash = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    if name_hash != e.hash.0 {
        return Err(NfsError::CorruptArchive { detail: "TPK header hash mismatch" });
    }
    let width = u16::from_le_bytes([hdr[32], hdr[33]]) as usize;
    let height = u16::from_le_bytes([hdr[34], hdr[35]]) as usize;
    let comp = hdr[38];
    if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
        return Err(NfsError::CorruptArchive { detail: "TPK texture dimensions out of range" });
    }

    let top = top_mip_size(width, height, comp)
        .ok_or(NfsError::NotImplemented { feature: "TPK pixel format" })?;
    let pixels = pool
        .get(0..top)
        .ok_or(NfsError::BufferSizeMismatch { detail: "TPK pixel data shorter than top mip" })?;

    let (rgba, source_format) = match comp {
        fmt::DXT1 => (dxt::decode_dxt1(pixels, width, height), TexFormat::Dxt1),
        fmt::DXT3 => (dxt::decode_dxt3(pixels, width, height), TexFormat::Dxt3),
        fmt::DXT5 => (dxt::decode_dxt5(pixels, width, height), TexFormat::Dxt5),
        fmt::RGBA8888 => (unpack_bgra(pixels, width, height), TexFormat::Unknown(0x20)),
        _ => return Err(NfsError::NotImplemented { feature: "TPK pixel format" }),
    };

    Ok(NfsTexture {
        name: String::new(),
        hash: e.hash,
        width: width as u32,
        height: height as u32,
        rgba,
        source_format,
        format: PixelFormat::Rgba8,
    })
}

/// Byte size of the top mipmap for `width`x`height` in the given compression type, or `None`
/// for a format we do not decode.
fn top_mip_size(width: usize, height: usize, comp: u8) -> Option<usize> {
    let blocks = width.div_ceil(4) * height.div_ceil(4);
    match comp {
        fmt::DXT1 => Some(blocks * 8),
        fmt::DXT3 | fmt::DXT5 => Some(blocks * 16),
        fmt::RGBA8888 => Some(width * height * RGBA),
        fmt::P8 => Some(width * height), // decode not yet supported, but size is known
        _ => None,
    }
}

/// Unpack the `0x20` format: 32-bit source stored B,G,R,A → RGBA8, alpha preserved.
fn unpack_bgra(src: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * RGBA];
    for (dst, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        dst.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
    }
    out
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

    fn descriptor(fields: [u32; 6]) -> Vec<u8> {
        fields.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    #[test]
    fn parses_descriptor_fields() {
        let d = descriptor([0xAABB_CCDD, 0xAE000, 1123, 11068, 0x100, 0]);
        let entries = parse_descriptors(&d);
        assert_eq!(entries.len(), 1);
        let e = entries[0];
        assert_eq!(e.hash, AssetHash(0xAABB_CCDD));
        assert_eq!(e.abs_offset, 0xAE000);
        assert_eq!(e.size, 1123);
        assert_eq!(e.out_size, 11068);
        assert_eq!(e.header_from_end, 0x100);
    }

    #[test]
    fn top_mip_sizes_match_s3tc() {
        // DXT1 128x128 = 32*32 blocks * 8 = 8192 bytes.
        assert_eq!(top_mip_size(128, 128, fmt::DXT1), Some(8192));
        // DXT3/5 double that.
        assert_eq!(top_mip_size(128, 128, fmt::DXT3), Some(16384));
        assert_eq!(top_mip_size(64, 32, fmt::DXT5), Some(64 / 4 * 32 / 4 * 16));
        // RGBA is 4 bytes/pixel.
        assert_eq!(top_mip_size(16, 16, fmt::RGBA8888), Some(16 * 16 * 4));
        // Unknown format → None.
        assert_eq!(top_mip_size(16, 16, 0x99), None);
    }

    #[test]
    fn bgra_unpack_reorders_to_rgba() {
        // one BGRA pixel (B=1,G=2,R=3,A=4) -> RGBA (3,2,1,4)
        let out = unpack_bgra(&[1, 2, 3, 4], 1, 1);
        assert_eq!(out, vec![3, 2, 1, 4]);
    }

    #[test]
    fn missing_descriptor_chunk_is_an_error() {
        // A pixel-data chunk but no descriptor table.
        let file = chunk(0x3332_0002, &[0u8; 8]);
        assert!(matches!(Tpk::parse(&file), Err(NfsError::CorruptArchive { .. })));
    }
}
