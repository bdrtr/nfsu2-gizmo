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

mod format;
mod index;
mod material;
mod name;
mod solid;
mod vertex;

pub use format::VERTEX_STRIDE;

use crate::chunk::ChunkNode;
use crate::error::NfsResult;
use crate::types::NfsMeshPart;

/// Parse an (already decompressed) `GEOMETRY.BIN` buffer into its renderable parts.
///
/// Solids that carry no mesh (mount/dummy points) are skipped. Returns an error only on
/// malformed data (index out of range, buffers too small for the declared counts).
pub fn parse_geometry(bytes: &[u8]) -> NfsResult<Vec<NfsMeshPart>> {
    let roots = ChunkNode::parse(bytes)?;
    let mut parts = Vec::new();
    for top in &roots {
        for node in top.find_all(format::SOLID) {
            if let Some(part) = solid::parse_solid(node, bytes)? {
                parts.push(part);
            }
        }
    }
    Ok(parts)
}
