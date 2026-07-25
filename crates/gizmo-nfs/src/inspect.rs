//! What a chunk *says*: its bytes read back as labelled fields.
//!
//! This is the model behind an inspector pane — and, deliberately, it is in the parser rather
//! than in the viewer. A viewer that re-declares the offsets is a viewer that can disagree with
//! the parser about what a file contains, and an inspector whose numbers differ from the reader's
//! is worse than none. Everything here reads through [`crate::geometry::format`], the same map
//! the parser reads.
//!
//! Two properties make it usable for a format that is not fully decoded:
//!
//! * The **generic** fields (id, size, kind, offsets, alignment filler, a byte preview) are built
//!   for *every* chunk, so an undecoded chunk is still worth clicking on.
//! * The alignment filler is a first-class row rather than something to infer from the hex. In
//!   this format a `0x11`-shifted header is the difference between a correct read and garbage,
//!   and that shift is invisible unless something says it out loud.

use crate::chunk::{ChunkKind, ChunkNode};
use crate::fourcc::FourCc;
use crate::geometry::{format, mesh_field, part_name, read_matrix, skip_leading_filler, VERTEX_STRIDE};
use crate::types::Mat4;

/// A value read out of a chunk, kept typed so a viewer can render (and a test can assert) it
/// without parsing a string back.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// A plain count or size.
    Num(u64),
    /// An identifier better read in hex.
    Hex(u32),
    /// A ratio worth showing as its own arithmetic (`26284 / 730 = 36.0`).
    Ratio { num: u64, den: u64, value: f64 },
    Text(String),
    Float(f32),
    Float3([f32; 3]),
    /// The solid's local 4x4, for the inspector's matrix grid.
    Matrix(Box<Mat4>),
    /// Raw bytes, shown as a hex preview.
    Bytes(Vec<u8>),
}

/// One row of the inspector.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Field {
    /// English label. Short, because it sits in a narrow pane.
    pub label: &'static str,
    /// Where the value lives in the file, when it is read rather than derived.
    pub offset: Option<usize>,
    /// How many bytes it occupies (0 when derived).
    pub len: usize,
    pub value: Value,
    /// Why this value is worth a second look — a shifted header, an unusual stride.
    pub note: Option<String>,
}

impl Field {
    fn new(label: &'static str, offset: Option<usize>, len: usize, value: Value) -> Self {
        Self { label, offset, len, value, note: None }
    }

    fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// A byte range worth highlighting in a hex view, and what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct KeySpan {
    pub start: usize,
    pub len: usize,
    pub role: SpanRole,
}

impl KeySpan {
    /// A span of `len` bytes at `start`. Provided because the struct is `#[non_exhaustive]`, and a
    /// consumer (a hex view, a test) still needs to build one.
    #[must_use]
    pub const fn new(start: usize, len: usize, role: SpanRole) -> Self {
        Self { start, len, role }
    }
}

/// What a highlighted span means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpanRole {
    /// A count the rest of the chunk's layout is derived from.
    Counter,
    /// An ASCII name.
    Name,
    /// The local transform.
    Matrix,
    /// `0x11` alignment filler.
    Filler,
}

/// Everything the inspector shows for one chunk.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ChunkModel {
    /// A readable name for the chunk type, when this project has locked one down.
    pub type_name: Option<&'static str>,
    /// One line under the header — a part's name, a count.
    pub summary: Option<String>,
    pub fields: Vec<Field>,
    /// Ranges a hex view should mark more strongly than the chunk's own extent.
    pub key_spans: Vec<KeySpan>,
}

/// A readable name for a chunk id, or `None` for one nobody has decoded.
///
/// Unknown ids deliberately get no invented label: "chunk" is honest, a made-up name is not.
#[must_use]
pub fn type_name(id: u32) -> Option<&'static str> {
    Some(match id {
        0x8013_4000 => "SolidList",
        0x8013_4001 => "SolidListHeader",
        0x0013_4002 => "ListInfo",
        format::SOLID => "SolidObject",
        format::SOLID_HEADER => "ObjectHeader",
        format::MATERIAL_LIST => "MaterialList",
        format::MATERIAL_SHADERS => "ShaderList",
        0x8013_4100 => "MeshData",
        format::MESH_HEADER => "MeshHeader",
        format::VERTEX_BUFFER => "VertexBuffer",
        format::MATERIAL_RANGES => "MaterialRanges",
        format::INDEX_BUFFER => "IndexBuffer",
        0xB330_0000 => "TexturePack",
        0xB331_0000 => "TPK InfoPart",
        0x3331_0001 => "TPK Header",
        0x3331_0002 => "TextureHashes",
        0x3331_0003 => "CompDescTable",
        0xB331_2000 => "TPK DataPart",
        _ => return None,
    })
}

/// Build the inspector model for one chunk.
///
/// `solid` is the `0x80134010` the chunk belongs to, when the caller knows it: a vertex buffer's
/// stride is only readable together with the vertex count from its sibling mesh header, and the
/// chunk alone cannot reach it. Pass `None` and that one row is simply absent.
///
/// Never fails: a truncated or unknown chunk yields the generic fields rather than an error, which
/// is what keeps a half-decoded file browsable.
#[must_use]
pub fn model(node: &ChunkNode, solid: Option<&ChunkNode>, root: &[u8]) -> ChunkModel {
    let data = node.data(root);
    let mut m = ChunkModel { type_name: type_name(node.header.id), ..Default::default() };
    generic(node, data, &mut m);
    match node.header.id {
        format::SOLID_HEADER => solid_header(node, data, &mut m),
        format::MESH_HEADER => mesh_header(node, data, &mut m),
        format::VERTEX_BUFFER => vertex_buffer(data, sibling_vertex_count(solid, root), &mut m),
        format::INDEX_BUFFER => index_buffer(node, data, &mut m),
        format::MATERIAL_RANGES => material_ranges(node, data, &mut m),
        format::MATERIAL_LIST | format::MATERIAL_SHADERS => hash_list(node, data, &mut m),
        _ => {}
    }
    m
}

/// The rows every chunk has, decoded or not.
fn generic(node: &ChunkNode, data: &[u8], m: &mut ChunkModel) {
    m.fields.push(
        Field::new("chunk id", Some(node.offset), 4, Value::Hex(node.header.id))
            .with_note(format!("\"{}\"", FourCc(node.header.id))),
    );
    m.fields.push(Field::new("size", Some(node.offset + 4), 4, Value::Num(u64::from(node.header.size))));
    m.fields.push(Field::new(
        "kind",
        None,
        0,
        Value::Text(
            match node.kind() {
                ChunkKind::Container => "container",
                ChunkKind::Leaf => "leaf",
                ChunkKind::Padding => "padding",
            }
            .to_owned(),
        ),
    ));
    if !node.children.is_empty() {
        m.fields.push(Field::new("children", None, 0, Value::Num(node.children.len() as u64)));
    }
    let filler = filler_words(data);
    if filler > 0 {
        m.fields.push(
            Field::new("0x11 filler", Some(node.data_offset), filler * 4, Value::Num(filler as u64))
                .with_note("alignment filler — every field below is shifted by this many words"),
        );
        m.key_spans.push(KeySpan { start: node.data_offset, len: filler * 4, role: SpanRole::Filler });
    }
    if !data.is_empty() {
        let n = data.len().min(16);
        m.fields.push(Field::new("preview", Some(node.data_offset), n, Value::Bytes(data[..n].to_vec())));
    }
}

/// `0x00134011` — the part's name and its local matrix.
fn solid_header(node: &ChunkNode, data: &[u8], m: &mut ChunkModel) {
    let name = part_name(data);
    if !name.is_empty() {
        // The name is the longest printable run; report where it actually starts so the hex view
        // can point at it.
        let at = find_ascii_run(data, &name).map(|o| node.data_offset + o);
        m.summary = Some(name.clone());
        m.fields.push(Field::new("name", at, name.len(), Value::Text(name)));
        if let Some(start) = at {
            m.key_spans.push(KeySpan { start, len: m.fields.last().map_or(0, |f| f.len), role: SpanRole::Name });
        }
    }
    if data.len() >= format::MATRIX_OFFSET + 64 {
        let at = node.data_offset + format::MATRIX_OFFSET;
        m.fields.push(Field::new("local matrix", Some(at), 64, Value::Matrix(Box::new(read_matrix(data)))));
        m.key_spans.push(KeySpan { start: at, len: 64, role: SpanRole::Matrix });
    }
}

/// `0x00134900` — the counts every buffer's layout is derived from.
fn mesh_header(node: &ChunkNode, data: &[u8], m: &mut ChunkModel) {
    let shift = filler_words(data) * 4;
    let body = skip_leading_filler(data);
    let at = |field: usize| node.data_offset + shift + field * 4;
    if let Ok(tris) = mesh_field(body, format::MESH_TRI_COUNT_FIELD) {
        m.fields.push(Field::new("triangles", Some(at(format::MESH_TRI_COUNT_FIELD)), 4, Value::Num(u64::from(tris))));
        m.key_spans.push(KeySpan { start: at(format::MESH_TRI_COUNT_FIELD), len: 4, role: SpanRole::Counter });
    }
    if let Ok(verts) = mesh_field(body, format::MESH_VERT_COUNT_FIELD) {
        m.fields.push(Field::new("vertices", Some(at(format::MESH_VERT_COUNT_FIELD)), 4, Value::Num(u64::from(verts))));
        m.key_spans.push(KeySpan { start: at(format::MESH_VERT_COUNT_FIELD), len: 4, role: SpanRole::Counter });
        m.summary = Some(format!("{verts} vertices"));
    }
}

/// `0x00134B01` — how the vertices are actually stored.
///
/// The stride is not in the buffer: it is `len / vertex_count`, and the parser only decodes the
/// standard 36. Showing the division rather than the answer is deliberate — it is what lets a
/// reader check the claim by hand, which is how the packed-layout solids were found.
fn vertex_buffer(data: &[u8], verts: Option<usize>, m: &mut ChunkModel) {
    let Some(verts) = verts.filter(|v| *v > 0) else {
        m.fields.push(Field::new("bytes", None, 0, Value::Num(data.len() as u64)));
        return;
    };
    let bpv = data.len() as f64 / verts as f64;
    let field = Field::new(
        "bytes per vertex",
        None,
        0,
        Value::Ratio { num: data.len() as u64, den: verts as u64, value: bpv },
    );
    let field = if verts * VERTEX_STRIDE > data.len() {
        field.with_note(format!(
            "does not fit stride {VERTEX_STRIDE} — this solid is skipped rather than mis-read"
        ))
    } else {
        field.with_note(format!(
            "stride {VERTEX_STRIDE} + {} B leading pad",
            data.len() - verts * VERTEX_STRIDE
        ))
    };
    m.fields.push(field);
    m.summary = Some(format!("{verts} × {bpv:.1} B"));
}

/// `0x00134B03` — a `u16` triangle list behind the filler.
fn index_buffer(node: &ChunkNode, data: &[u8], m: &mut ChunkModel) {
    let filler = filler_words(data) * 4;
    let usable = data.len().saturating_sub(filler);
    m.fields.push(Field::new("triangles that fit", None, 0, Value::Num((usable / 6) as u64)));
    if let Some(tri) = data.get(filler..filler + 6) {
        let idx: Vec<u16> = tri.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        m.fields.push(Field::new(
            "first triangle",
            Some(node.data_offset + filler),
            6,
            Value::Text(format!("{}, {}, {}", idx[0], idx[1], idx[2])),
        ));
        m.key_spans.push(KeySpan { start: node.data_offset + filler, len: 6, role: SpanRole::Counter });
    }
}

/// `0x00134B02` — the per-material index ranges, stored as the trailing `n × 60` bytes.
fn material_ranges(node: &ChunkNode, data: &[u8], m: &mut ChunkModel) {
    let n = data.len() / format::MAT_RANGE_STRIDE;
    m.fields.push(Field::new("material runs", None, 0, Value::Num(n as u64)));
    if n == 0 {
        return;
    }
    let start = data.len() - n * format::MAT_RANGE_STRIDE;
    m.fields.push(
        Field::new("table starts at", Some(node.data_offset + start), 0, Value::Num(start as u64))
            .with_note("entries are the trailing n × 60 bytes; anything before is filler"),
    );
    for i in 0..n.min(4) {
        let base = start + i * format::MAT_RANGE_STRIDE;
        let count = u32_at(data, base + format::MAT_RANGE_COUNT);
        let offset = u32_at(data, base + format::MAT_RANGE_OFFSET);
        m.fields.push(Field::new(
            "run",
            Some(node.data_offset + base),
            format::MAT_RANGE_STRIDE,
            Value::Text(format!("{count} indices @ {offset}")),
        ));
    }
}

/// `0x00134012` / `0x00134013` — 8-byte entries, a hash and a zero.
fn hash_list(node: &ChunkNode, data: &[u8], m: &mut ChunkModel) {
    let n = data.len() / 8;
    m.fields.push(Field::new("entries", None, 0, Value::Num(n as u64)));
    for i in 0..n.min(6) {
        let hash = u32_at(data, i * 8);
        m.fields.push(Field::new("hash", Some(node.data_offset + i * 8), 4, Value::Hex(hash)));
    }
}

/// The vertex count from the mesh header hanging off the same solid.
fn sibling_vertex_count(solid: Option<&ChunkNode>, root: &[u8]) -> Option<usize> {
    let header = solid?.find(format::MESH_HEADER)?;
    let body = skip_leading_filler(header.data(root));
    mesh_field(body, format::MESH_VERT_COUNT_FIELD).ok().map(|v| v as usize)
}

/// Leading `0x11111111` words in a payload.
fn filler_words(data: &[u8]) -> usize {
    let mut n = 0;
    while data.get(n * 4..n * 4 + 4) == Some(&[format::FILLER_BYTE; 4]) {
        n += 1;
    }
    n
}

fn u32_at(data: &[u8], pos: usize) -> u32 {
    data.get(pos..pos + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Where a printable run sits inside a header.
fn find_ascii_run(data: &[u8], needle: &str) -> Option<usize> {
    data.windows(needle.len()).position(|w| w == needle.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_bytes(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    fn only_node(bytes: &[u8]) -> ChunkNode {
        ChunkNode::parse(bytes).unwrap().remove(0)
    }

    #[test]
    fn every_chunk_gets_the_generic_rows() {
        let bytes = chunk_bytes(0xDEAD_BEEF, &[1, 2, 3, 4]);
        let m = model(&only_node(&bytes), None, &bytes);
        assert_eq!(m.type_name, None, "an unknown id is not given an invented name");
        let labels: Vec<_> = m.fields.iter().map(|f| f.label).collect();
        assert!(labels.contains(&"chunk id") && labels.contains(&"size") && labels.contains(&"preview"));
        // The id and size rows point at the header, which is what lets the hex view light up.
        assert_eq!(m.fields[0].offset, Some(0));
        assert_eq!(m.fields[1].offset, Some(4));
    }

    #[test]
    fn a_shifted_mesh_header_reports_the_shift_and_still_reads_its_counts() {
        // Two filler words in front of the counters — SENTRA_KIT00_BODY_A's real layout. Read at
        // the nominal offsets this chunk looks empty; the shift is why it must be said out loud.
        let mut payload = vec![format::FILLER_BYTE; 8];
        let mut words = [0u32; 16];
        words[format::MESH_TRI_COUNT_FIELD] = 488;
        words[format::MESH_VERT_COUNT_FIELD] = 508;
        for w in words {
            payload.extend_from_slice(&w.to_le_bytes());
        }
        let bytes = chunk_bytes(format::MESH_HEADER, &payload);
        let m = model(&only_node(&bytes), None, &bytes);
        assert_eq!(m.type_name, Some("MeshHeader"));
        let by = |label: &str| m.fields.iter().find(|f| f.label == label).cloned();
        assert_eq!(by("0x11 filler").unwrap().value, Value::Num(2));
        assert_eq!(by("triangles").unwrap().value, Value::Num(488));
        assert_eq!(by("vertices").unwrap().value, Value::Num(508));
        // The counter offsets must include the shift, or the hex view highlights the wrong bytes.
        let tri_off = by("triangles").unwrap().offset.unwrap();
        assert_eq!(tri_off, 8 + 8 + format::MESH_TRI_COUNT_FIELD * 4);
        assert!(m.key_spans.iter().any(|s| s.role == SpanRole::Counter && s.start == tri_off));
    }

    #[test]
    fn a_solid_header_yields_its_name_and_matrix() {
        let mut payload = vec![0u8; format::MATRIX_OFFSET];
        payload[8..8 + 18].copy_from_slice(b"240SX_KIT00_HOOD_A");
        for row in 0..4 {
            for col in 0..4 {
                let v: f32 = if row == col { 1.0 } else { 0.0 };
                payload.extend_from_slice(&v.to_le_bytes());
                let _ = col;
            }
        }
        let bytes = chunk_bytes(format::SOLID_HEADER, &payload);
        let m = model(&only_node(&bytes), None, &bytes);
        assert_eq!(m.summary.as_deref(), Some("240SX_KIT00_HOOD_A"));
        assert!(m.key_spans.iter().any(|s| s.role == SpanRole::Name));
        assert!(m.key_spans.iter().any(|s| s.role == SpanRole::Matrix));
        let matrix = m.fields.iter().find(|f| f.label == "local matrix").unwrap();
        assert!(matches!(&matrix.value, Value::Matrix(m) if (m[0][0] - 1.0).abs() < 1e-6));
    }

    #[test]
    fn a_truncated_chunk_still_produces_a_model() {
        // Four bytes of payload where a solid header wants 128: the generic rows must survive.
        let bytes = chunk_bytes(format::SOLID_HEADER, &[0, 0, 0, 0]);
        let m = model(&only_node(&bytes), None, &bytes);
        assert!(!m.fields.is_empty());
    }
}
