//! Parse NFSU2 `GEOMETRY.BIN` car models into engine-agnostic [`NfsMeshPart`]s.
//!
//! Layout (confirmed empirically against real cars — see `examples/geo_probe.rs`):
//!
//! ```text
//! 0x80134000                      root ("solid list")
//!   0x80134001                    global header (skipped)
//!   0x80134010  (one per SOLID)   a renderable part
//!     0x00134011                  solid header: name (ASCII) + local 4x4 matrix + bbox
//!     0x00134900                  mesh header: u32[9]=triangle_count, u32[13]=vertex_count
//!     0x80134100                  mesh container
//!       0x00134B01                vertex buffer (see below)
//!       0x00134B03                index buffer (u16 triangle list)
//! ```
//!
//! **Vertex buffer** `0x00134B01`: leading alignment padding (`0x11` filler) followed by
//! `vertex_count` vertices of **stride 36 bytes = 9 little-endian f32**:
//! `position[3]`, `normal[3]`, one reserved sentinel float, `uv[2]`. The vertices occupy
//! the *last* `vertex_count * 36` bytes, so the pad size is derived, not scanned.
//!
//! **Index buffer** `0x00134B03`: leading padding followed by `triangle_count * 3`
//! little-endian `u16` indices (the last `triangle_count * 6` bytes).

use crate::chunk::ChunkNode;
use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;
use crate::types::{AssetHash, LodLevel, Mat4, NfsMeshPart, PartRole};

const SOLID: u32 = 0x8013_4010;
const SOLID_HEADER: u32 = 0x0013_4011;
const MATERIAL_LIST: u32 = 0x0013_4012;
const MESH_HEADER: u32 = 0x0013_4900;
const VERTEX_BUFFER: u32 = 0x0013_4B01;
const INDEX_BUFFER: u32 = 0x0013_4B03;

/// Bytes per vertex in `0x00134B01` (9 × f32).
pub const VERTEX_STRIDE: usize = 36;
/// Offset of the local transform matrix within the `0x00134011` solid header.
const MATRIX_OFFSET: usize = 64;
/// u32 field index of the triangle count within the `0x00134900` mesh header.
const MESH_TRI_COUNT_FIELD: usize = 9;
/// u32 field index of the vertex count within the `0x00134900` mesh header.
const MESH_VERT_COUNT_FIELD: usize = 13;

/// Parse an (already decompressed) `GEOMETRY.BIN` buffer into its renderable parts.
///
/// Solids that carry no mesh (mount/dummy points) are skipped. Returns an error only on
/// malformed data (index out of range, buffers too small for the declared counts).
pub fn parse_geometry(bytes: &[u8]) -> NfsResult<Vec<NfsMeshPart>> {
    let roots = ChunkNode::parse(bytes)?;
    let mut parts = Vec::new();
    for top in &roots {
        for solid in top.find_all(SOLID) {
            if let Some(part) = parse_solid(solid, bytes)? {
                parts.push(part);
            }
        }
    }
    Ok(parts)
}

fn parse_solid(solid: &ChunkNode, root: &[u8]) -> NfsResult<Option<NfsMeshPart>> {
    let (Some(mesh), Some(vbuf), Some(ibuf)) = (
        solid.find(MESH_HEADER),
        solid.find(VERTEX_BUFFER),
        solid.find(INDEX_BUFFER),
    ) else {
        return Ok(None); // not a renderable solid (e.g. a mount point)
    };

    let mesh_data = mesh.data(root);
    let tri_count = mesh_field(mesh_data, MESH_TRI_COUNT_FIELD)? as usize;
    let vert_count = mesh_field(mesh_data, MESH_VERT_COUNT_FIELD)? as usize;
    if vert_count == 0 || tri_count == 0 {
        return Ok(None);
    }

    let (positions, normals, uvs) = parse_vertices(vbuf.data(root), vert_count)?;
    let indices = parse_indices(ibuf.data(root), tri_count, vert_count)?;

    // Name / role / LOD / transform from the solid header (optional but usually present).
    let (name, transform) = match solid.find(SOLID_HEADER) {
        Some(h) => {
            let d = h.data(root);
            (part_name(d), read_matrix(d))
        }
        None => (String::new(), crate::types::IDENTITY),
    };
    let role = classify_role(&name);
    let lod = classify_lod(&name);
    let (bbox_min, bbox_max) = bounds(&positions);
    let material_refs =
        solid.find(MATERIAL_LIST).map(|m| material_hashes(m.data(root))).unwrap_or_default();

    Ok(Some(NfsMeshPart {
        name,
        hash: AssetHash(0),
        positions,
        normals,
        uvs,
        indices,
        material_refs,
        role,
        lod,
        transform,
        bbox_min,
        bbox_max,
    }))
}

/// Read u32 field `n` (0-based) from a mesh header, bounds-checked.
fn mesh_field(data: &[u8], n: usize) -> NfsResult<u32> {
    let mut r = ByteReader::at(data, n * 4)?;
    r.u32_le()
}

/// The vertices occupy the last `count * STRIDE` bytes of the buffer (leading bytes are
/// alignment padding). Returns parallel position/normal/uv arrays.
#[allow(clippy::type_complexity)]
fn parse_vertices(
    vbuf: &[u8],
    count: usize,
) -> NfsResult<(Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>)> {
    let needed = count
        .checked_mul(VERTEX_STRIDE)
        .ok_or(NfsError::BufferSizeMismatch { detail: "vertex count overflow" })?;
    if needed > vbuf.len() {
        return Err(NfsError::BufferSizeMismatch { detail: "vertex buffer smaller than count*stride" });
    }
    let start = vbuf.len() - needed;
    let mut positions = Vec::with_capacity(count);
    let mut normals = Vec::with_capacity(count);
    let mut uvs = Vec::with_capacity(count);
    let mut r = ByteReader::at(vbuf, start)?;
    for _ in 0..count {
        let px = r.f32_le()?;
        let py = r.f32_le()?;
        let pz = r.f32_le()?;
        let nx = r.f32_le()?;
        let ny = r.f32_le()?;
        let nz = r.f32_le()?;
        let _reserved = r.f32_le()?; // constant sentinel (~-1.7e38); unused
        let u = r.f32_le()?;
        let v = r.f32_le()?;
        positions.push([px, py, pz]);
        normals.push([nx, ny, nz]);
        uvs.push([u, v]);
    }
    Ok((positions, normals, uvs))
}

/// The indices occupy the last `tri_count * 3` u16s of the buffer. Every index is
/// validated against the vertex count.
fn parse_indices(ibuf: &[u8], tri_count: usize, vert_count: usize) -> NfsResult<Vec<u32>> {
    let index_count = tri_count
        .checked_mul(3)
        .ok_or(NfsError::BufferSizeMismatch { detail: "triangle count overflow" })?;
    let needed = index_count
        .checked_mul(2)
        .ok_or(NfsError::BufferSizeMismatch { detail: "index count overflow" })?;
    if needed > ibuf.len() {
        return Err(NfsError::BufferSizeMismatch { detail: "index buffer smaller than tri_count*6" });
    }
    let start = ibuf.len() - needed;
    let mut r = ByteReader::at(ibuf, start)?;
    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        let i = r.u16_le()? as u32;
        if i as usize >= vert_count {
            return Err(NfsError::IndexOutOfRange { index: i, vertex_count: vert_count as u32 });
        }
        indices.push(i);
    }
    Ok(indices)
}

/// Read the 4x4 local matrix from a solid header (row-major, 16 f32 at [`MATRIX_OFFSET`]).
/// Falls back to identity if the header is too short.
fn read_matrix(header: &[u8]) -> Mat4 {
    let mut m = crate::types::IDENTITY;
    if let Ok(mut r) = ByteReader::at(header, MATRIX_OFFSET) {
        for row in &mut m {
            for cell in row.iter_mut() {
                match r.f32_le() {
                    Ok(v) => *cell = v,
                    Err(_) => return crate::types::IDENTITY,
                }
            }
        }
    }
    m
}

/// Extract the part name: the longest printable run in the header (names are ASCII with
/// `_`, e.g. `240SX_BASE_A`, `240SX_KIT01_HOOD_A`).
fn part_name(header: &[u8]) -> String {
    let mut best = "";
    let mut start = 0usize;
    let mut i = 0usize;
    while i <= header.len() {
        let printable = header.get(i).is_some_and(|&b| b.is_ascii_graphic());
        if !printable {
            if i - start > best.len() {
                best = std::str::from_utf8(&header[start..i]).unwrap_or("");
            }
            start = i + 1;
        }
        i += 1;
    }
    best.to_owned()
}

/// The `0x00134012` material list is an array of 8-byte entries (`u32` hash, `u32` zero).
/// The list mixes shader/material hashes with the solid's texture hash(es), in no fixed
/// order, so all non-zero hashes are returned and the texture is resolved downstream against
/// the `TEXTURES.BIN` pack.
fn material_hashes(data: &[u8]) -> Vec<AssetHash> {
    let count = data.len() / 8;
    let mut hashes = Vec::with_capacity(count);
    let mut r = ByteReader::new(data);
    for _ in 0..count {
        let hash = r.u32_le().unwrap_or(0);
        let _pad = r.u32_le().unwrap_or(0);
        if hash != 0 {
            hashes.push(AssetHash(hash));
        }
    }
    hashes
}

fn classify_role(name: &str) -> PartRole {
    if name.contains("_BASE") {
        PartRole::Body
    } else if let Some(pos) = name.find("_KIT") {
        let digits: String = name[pos + 4..].chars().take_while(|c| c.is_ascii_digit()).collect();
        PartRole::Kit { slot: digits.parse().unwrap_or(0) }
    } else if name.contains("WHEEL") || name.contains("TIRE") || name.contains("RIM") {
        PartRole::Wheel
    } else {
        PartRole::Unknown
    }
}

/// Classify a part's level of detail from the trailing `_A`..`_D` suffix of its name
/// (`A` = highest detail). Anything else — including names whose LOD letter was cut off by
/// the fixed-length name field — is [`LodLevel::Unknown`].
///
/// Empirically locked against 54 stock cars (~15k parts): a genuine LOD tag is always a
/// single `A`..`D` immediately after `_`, and truncation never forges a spurious `_A`..`_D`
/// tail (the only truncated single-letter tails observed are `L`/`R`/`Q`/`W`/`M`/`U`), so
/// this predicate has no false positives. Parts whose LOD letter was truncated away
/// (`..._HEADLIGHT_LEFT_`, `..._HEADLIGHT_L`, `..._SIDE_MIRRO`) are deliberately left
/// `Unknown` rather than reconstructed by triangle count: names that collide after
/// truncation are *not* clean per-component LOD ladders — they merge left/right panels and
/// `STYLE##` variants and carry duplicate triangle counts, so a rank-based guess would be
/// wrong. Downstream code that needs the best LOD picks it per component by triangle count
/// instead (see the game's `select_stock_car`), which is why this field can stay honest.
///
/// The suffix test mirrors the game's `component_key`, which strips the very same `_[A-D]`.
fn classify_lod(name: &str) -> LodLevel {
    // A LOD tag is a single A..D at the end, immediately preceded by '_'. Requiring the
    // delimiter (rather than just the last '_'-segment) means a bare letter or an
    // underscore-less name can never be misread as a LOD.
    let b = name.as_bytes();
    if b.len() >= 2 && b[b.len() - 2] == b'_' {
        return match b[b.len() - 1] {
            b'A' => LodLevel::A,
            b'B' => LodLevel::B,
            b'C' => LodLevel::C,
            b'D' => LodLevel::D,
            _ => LodLevel::Unknown,
        };
    }
    LodLevel::Unknown
}

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in positions {
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    if positions.is_empty() {
        (([0.0; 3]), ([0.0; 3]))
    } else {
        (min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_names() {
        assert_eq!(classify_role("240SX_BASE_A"), PartRole::Body);
        assert_eq!(classify_lod("240SX_BASE_A"), LodLevel::A);
        assert_eq!(classify_role("FOCUS_KIT03_HOOD_B"), PartRole::Kit { slot: 3 });
        assert_eq!(classify_lod("FOCUS_KIT03_HOOD_B"), LodLevel::B);
        assert_eq!(classify_role("FRONT_WHEEL"), PartRole::Wheel);
    }

    /// Locks the empirical LOD-naming survey (54 stock cars, ~15k parts): genuine LOD tags
    /// are exactly a trailing `_[A-D]`; every truncation pattern seen in the wild must fall
    /// through to `Unknown` rather than be misread as (or fabricated into) a LOD.
    #[test]
    fn classify_lod_locks_empirical_naming() {
        // Genuine LOD suffixes at every level.
        assert_eq!(classify_lod("240SX_BASE_A"), LodLevel::A);
        assert_eq!(classify_lod("240SX_KIT00_BODY_B"), LodLevel::B);
        assert_eq!(classify_lod("240SX_KIT00_REAR_BUMPER_C"), LodLevel::C);
        assert_eq!(classify_lod("240SX_KIT00_BODY_D"), LodLevel::D);
        // Truncated away entirely (trailing '_'): two LODs collapse to one name → Unknown.
        assert_eq!(classify_lod("240SX_KIT00_HEADLIGHT_LEFT_"), LodLevel::Unknown);
        // Left/right letter survived but the LOD letter is gone.
        assert_eq!(classify_lod("MUSTANGGT_KIT00_HEADLIGHT_L"), LodLevel::Unknown);
        assert_eq!(classify_lod("MUSTANGGT_KIT00_HEADLIGHT_R"), LodLevel::Unknown);
        // Mid-word truncations observed in the survey.
        assert_eq!(classify_lod("240SX_KIT00_LEFT_SIDE_MIRRO"), LodLevel::Unknown);
        assert_eq!(classify_lod("240SX_KIT00_TRUNK_AUDIO_UNL"), LodLevel::Unknown);
        // Non-LOD single-letter tails that DO occur (`R` from `..._QUARTER_R`) must not read
        // as a LOD — only A..D are LOD letters.
        assert_eq!(classify_lod("240SX_DECAL_RIGHT_QUARTER_R"), LodLevel::Unknown);
        // Robustness: a bare letter or an underscore-less/empty name is never a LOD.
        assert_eq!(classify_lod("A"), LodLevel::Unknown);
        assert_eq!(classify_lod("BODY"), LodLevel::Unknown);
        assert_eq!(classify_lod(""), LodLevel::Unknown);
    }

    #[test]
    fn parses_a_synthetic_solid_end_slice() {
        // 2 vertices of stride 36, prefixed with 8 pad bytes, and 1 triangle of u16 indices.
        let mut vbuf = vec![0x11u8; 8];
        for v in 0..2 {
            let base = v as f32;
            for f in [base, base + 0.1, base + 0.2, 0.0, 1.0, 0.0, -1.0e38, 0.5, 0.25] {
                vbuf.extend_from_slice(&f.to_le_bytes());
            }
        }
        let (pos, nrm, uv) = parse_vertices(&vbuf, 2).unwrap();
        assert_eq!(pos.len(), 2);
        assert!((pos[1][0] - 1.0).abs() < 1e-6);
        assert_eq!(nrm[0], [0.0, 1.0, 0.0]);
        assert_eq!(uv[0], [0.5, 0.25]);

        let mut ibuf = vec![0x11u8; 4];
        for i in [0u16, 1, 0] {
            ibuf.extend_from_slice(&i.to_le_bytes());
        }
        let idx = parse_indices(&ibuf, 1, 2).unwrap();
        assert_eq!(idx, vec![0, 1, 0]);
    }

    #[test]
    fn rejects_out_of_range_index() {
        let mut ibuf = Vec::new();
        for i in [0u16, 1, 5] {
            ibuf.extend_from_slice(&i.to_le_bytes());
        }
        assert!(matches!(parse_indices(&ibuf, 1, 2), Err(NfsError::IndexOutOfRange { index: 5, .. })));
    }
}
