//! Parse NFSU2 `GEOMETRY.BIN` car models into engine-agnostic [`NfsMeshPart`]s.
//!
//! Layout (confirmed empirically against real cars — see `examples/nfs_dump.rs`):
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
//! **Index buffer** `0x00134B03`: leading `0x11` padding followed by `triangle_count * 3`
//! little-endian `u16` indices, read **forward** from just past that padding.
//!
//! The module is split by what each piece reads, so every reader is small and testable on its
//! own byte buffer: [`format`] holds the empirically-locked chunk IDs and offsets, [`solid`]
//! assembles one part, and [`vertex`], [`index`], [`material`] and [`name`] decode the buffers
//! and tables it pulls together.

pub mod format;
mod index;
mod material;
mod name;
mod solid;
mod vertex;

pub use format::VERTEX_STRIDE;
pub use name::{part_name, read_matrix};
pub use solid::{mesh_field, skip_leading_filler, SkipReason, Skipped};
pub use vertex::standard_vertex_layout;

use crate::chunk::ChunkNode;
use crate::error::NfsResult;
use crate::types::NfsMeshPart;

/// Parse an (already decompressed) `GEOMETRY.BIN` buffer into its renderable parts.
///
/// Solids that carry no mesh (mount/dummy points) are skipped. Returns an error only on
/// malformed data (index out of range, buffers too small for the declared counts).
pub fn parse_geometry(bytes: &[u8]) -> NfsResult<Vec<NfsMeshPart>> {
    Ok(parse_geometry_reporting(bytes)?.0)
}

/// Like [`parse_geometry`], but also returns every solid that yielded no part and why.
///
/// A silently dropped solid is a panel a car loses without anyone noticing — the reason it exists
/// as a return value rather than a log line is that the caller (a viewer, a validator) is the one
/// that can say whether it matters.
pub fn parse_geometry_reporting(
    bytes: &[u8],
) -> NfsResult<(Vec<NfsMeshPart>, Vec<solid::Skipped>)> {
    let roots = ChunkNode::parse(bytes)?;
    let mut parts = Vec::new();
    let mut skipped = Vec::new();
    for top in &roots {
        for node in top.find_all(format::SOLID) {
            match solid::parse_solid(node, bytes)? {
                Ok(part) => parts.push(part),
                Err(reason) => skipped.push(solid::Skipped {
                    offset: node.offset,
                    name: node
                        .find(format::SOLID_HEADER)
                        .map(|h| name::part_name(h.data(bytes)))
                        .unwrap_or_default(),
                    reason,
                }),
            }
        }
    }
    Ok((parts, skipped))
}
