//! The `GEOMETRY.BIN` on-disk constants: chunk IDs, strides and field offsets.
//!
//! Every offset here is **locked empirically** against a real install (see the crate README);
//! nothing in this file may be changed on the strength of an assumption. Keeping them in one
//! module means the format description lives in one place while the readers that consume it
//! ([`super::solid`], [`super::vertex`], [`super::index`], [`super::material`], [`super::name`])
//! stay small and independently testable.
//!
//! These are **public**: anything that shows a file's structure — the `ug2 probe` command, the
//! STRUKT inspector — needs the same offsets the parser reads. Copying them into a viewer is how
//! the viewer starts disagreeing with the parser about what a file says, which is the one thing
//! an inspector must never do.

/// A renderable part.
pub const SOLID: u32 = 0x8013_4010;
/// Solid header: name (ASCII) + local 4x4 matrix + bbox.
pub const SOLID_HEADER: u32 = 0x0013_4011;
/// The solid's asset-hash list (materials *and* textures, in file order).
pub const MATERIAL_LIST: u32 = 0x0013_4012;
/// Per-material shader-hash list (parallel role to [`MATERIAL_LIST`]; shaders are shared across
/// cars, so they identify the material *type* — glass, paint, chrome, …).
pub const MATERIAL_SHADERS: u32 = 0x0013_4013;
/// Mesh header: triangle and vertex counts.
pub const MESH_HEADER: u32 = 0x0013_4900;
/// Vertex buffer.
pub const VERTEX_BUFFER: u32 = 0x0013_4B01;
/// Per-material index-range table (splits the index buffer by material).
pub const MATERIAL_RANGES: u32 = 0x0013_4B02;
/// Index buffer.
pub const INDEX_BUFFER: u32 = 0x0013_4B03;

/// Bytes per `0x00134B02` entry (15 × u32). The entries occupy the *trailing*
/// `count * MAT_RANGE_STRIDE` bytes (any leading bytes are `0x11` alignment filler).
pub const MAT_RANGE_STRIDE: usize = 60;
/// u32 field offsets within a `0x00134B02` entry: index count, material index (into
/// `0x00134012`), shader index (into `0x00134013`), index offset. (The rest is a per-material
/// bbox — degenerate ±5000 in practice — and a constant `0x4180`.)
pub const MAT_RANGE_COUNT: usize = 3 * 4;
pub const MAT_RANGE_MATIDX: usize = 7 * 4;
pub const MAT_RANGE_SHADERIDX: usize = 8 * 4;
pub const MAT_RANGE_OFFSET: usize = 13 * 4;

/// Bytes per vertex in `0x00134B01` (9 × f32).
pub const VERTEX_STRIDE: usize = 36;
/// Offset of the local transform matrix within the `0x00134011` solid header.
pub const MATRIX_OFFSET: usize = 64;
/// u32 field index of the triangle count within the `0x00134900` mesh header.
pub const MESH_TRI_COUNT_FIELD: usize = 9;
/// u32 field index of the vertex count within the `0x00134900` mesh header.
pub const MESH_VERT_COUNT_FIELD: usize = 13;

/// The byte NFSU2 pads chunk payloads to alignment with.
pub const FILLER_BYTE: u8 = 0x11;
