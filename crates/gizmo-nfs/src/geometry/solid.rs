//! One `0x80134010` solid → one [`NfsMeshPart`]: the per-part assembly that pulls the mesh
//! header, buffers and material tables together.

use super::format::{
    FILLER_BYTE, INDEX_BUFFER, MATERIAL_LIST, MATERIAL_RANGES, MATERIAL_SHADERS, MESH_HEADER,
    MESH_TRI_COUNT_FIELD, MESH_VERT_COUNT_FIELD, SOLID_HEADER, VERTEX_BUFFER,
};
use super::{index, material, name, vertex};
use crate::chunk::ChunkNode;
use crate::error::NfsResult;
use crate::reader::ByteReader;
use crate::types::{AssetHash, NfsMeshPart};

/// Why a solid yielded no part. A skipped solid used to vanish without a word, which is how a
/// car can quietly lose a panel: `CARS/3000GT` carries 579 solids and returns 578 parts, and
/// nothing said so. Every `None` now names its reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkipReason {
    /// No mesh header / vertex buffer / index buffer — a mount or dummy point, not a mesh.
    NotAMesh,
    /// The mesh header declares no triangles or no vertices.
    EmptyMesh,
    /// `vert_count * 36` overruns the vertex buffer: a packed layout this crate does not decode.
    /// Skipping beats mis-reading it as stride 36 and emitting shredded geometry.
    PackedVertexLayout {
        /// Vertices the header declares.
        vert_count: usize,
        /// Bytes actually present in the buffer.
        vbuf_len: usize,
    },
}

/// A solid that produced no part, and why.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Skipped {
    /// Offset of the solid's `0x80134010` header — the key a viewer selects it by.
    pub offset: usize,
    /// The solid's name, when its header carries one.
    pub name: String,
    pub reason: SkipReason,
}

/// Parse one solid, or say why it yields no part.
pub(super) fn parse_solid(
    solid: &ChunkNode,
    root: &[u8],
) -> NfsResult<Result<NfsMeshPart, SkipReason>> {
    let (Some(mesh), Some(vbuf), Some(ibuf)) = (
        solid.find(MESH_HEADER),
        solid.find(VERTEX_BUFFER),
        solid.find(INDEX_BUFFER),
    ) else {
        return Ok(Err(SkipReason::NotAMesh));
    };

    let mesh_data = skip_leading_filler(mesh.data(root));
    let tri_count = mesh_field(mesh_data, MESH_TRI_COUNT_FIELD)? as usize;
    let vert_count = mesh_field(mesh_data, MESH_VERT_COUNT_FIELD)? as usize;
    if vert_count == 0 || tri_count == 0 {
        return Ok(Err(SkipReason::EmptyMesh));
    }
    let vbuf_len = vbuf.data(root).len();
    if !vertex::standard_vertex_layout(vert_count, vbuf_len) {
        return Ok(Err(SkipReason::PackedVertexLayout { vert_count, vbuf_len }));
    }

    let (positions, normals, uvs) = vertex::parse_vertices(vbuf.data(root), vert_count)?;
    let indices = index::parse_indices(ibuf.data(root), tri_count, vert_count)?;

    // Name / role / LOD / transform from the solid header (optional but usually present).
    let (part, transform) = match solid.find(SOLID_HEADER) {
        Some(h) => {
            let d = h.data(root);
            (name::part_name(d), name::read_matrix(d))
        }
        None => (String::new(), crate::types::IDENTITY),
    };
    let role = name::classify_role(&part);
    let lod = name::classify_lod(&part);
    let (bbox_min, bbox_max) = vertex::bounds(&positions);
    // The `0x00134012` list, in file order *including* zero slots — the material list indexes
    // into it, so positions must be preserved (`material_refs` drops the zeros for lookup).
    let ordered =
        solid.find(MATERIAL_LIST).map(|m| material::ordered_hashes(m.data(root))).unwrap_or_default();
    let shaders =
        solid.find(MATERIAL_SHADERS).map(|m| material::ordered_hashes(m.data(root))).unwrap_or_default();
    let material_refs = ordered.iter().copied().filter(|h| h.0 != 0).collect();
    let materials = solid
        .find(MATERIAL_RANGES)
        .map(|m| material::material_ranges(m.data(root), &ordered, &shaders, indices.len()))
        .unwrap_or_default();

    Ok(Ok(NfsMeshPart {
        name: part,
        hash: AssetHash(0),
        positions,
        normals,
        uvs,
        indices,
        material_refs,
        materials,
        role,
        lod,
        transform,
        bbox_min,
        bbox_max,
    }))
}

/// Skip a chunk payload's leading `0x11` alignment filler.
///
/// NFSU2 pads to alignment with `0x11` bytes, and on a large minority of solids that padding sits
/// *in front of* the `0x00134900` mesh header, shifting every field by one or two u32s. Read at the
/// nominal field offsets, those solids look empty (`vert_count == 0`) and were skipped as
/// mount/dummy points: fleet-wide that silently dropped ~2500 meshes, including **every** LOD of
/// `SENTRA_KIT00_BODY` (the Sentra then rendered with no body shell at all, a hole straight into the
/// cabin) and the highest LOD of many other cars' bodies, doors and headlights. The same filler
/// already prefixes the index buffer and the material-range table.
pub fn skip_leading_filler(data: &[u8]) -> &[u8] {
    let mut off = 0;
    while data.len() >= off + 4 && data[off..off + 4] == [FILLER_BYTE; 4] {
        off += 4;
    }
    &data[off..]
}

/// Read u32 field `n` (0-based) from a chunk payload, bounds-checked. Public because every
/// reader of a `0x00134900`-style counter block needs exactly this, filler already skipped.
pub fn mesh_field(data: &[u8], n: usize) -> NfsResult<u32> {
    let mut r = ByteReader::at(data, n * 4)?;
    r.u32_le()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_filler_words_are_skipped_whole() {
        // Two filler words in front of the real header (SENTRA_KIT00_BODY_A's layout).
        let mut data = vec![FILLER_BYTE; 8];
        data.extend_from_slice(&7u32.to_le_bytes());
        assert_eq!(mesh_field(skip_leading_filler(&data), 0).unwrap(), 7);
        // An unshifted header is untouched.
        let plain = 9u32.to_le_bytes();
        assert_eq!(mesh_field(skip_leading_filler(&plain), 0).unwrap(), 9);
        // A partial (non-word) run of filler is not a shift — leave it to the field offsets.
        let partial = [FILLER_BYTE, FILLER_BYTE, 0, 0];
        assert_eq!(skip_leading_filler(&partial).len(), 4);
    }
}
