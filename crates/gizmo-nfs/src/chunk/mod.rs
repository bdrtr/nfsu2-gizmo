//! The universal NFSU2 chunk tree.
//!
//! Almost every NFSU2 asset file is a flat/recursive stream of 8-byte-headed sections
//! (EA calls them "bChunks"):
//!
//! ```text
//! struct BinSectionHeader { id: u32 (LE), size: u32 (LE) }  // size = bytes AFTER the header
//! ```
//!
//! Classification of a header by its `id`:
//! * high bit set (`0x8xxx_xxxx`) → **container**: its `size` bytes are themselves a
//!   sequence of sub-chunks — recurse.
//! * high bit clear → **leaf**: `size` bytes of payload data.
//! * `id == 0` → **padding/alignment**: skip `size` bytes, emit no node.
//!
//! Parsing walks `offset += 8 + size` at each level. Two consumption styles share one
//! bounds-checked core: a zero-allocation [`walk`] visitor ([`walk`] module) and a
//! materialised [`ChunkNode`] tree ([`tree`]) for ergonomic querying; [`dump`] renders
//! either as a listing.
//!
//! Robustness: every child's declared size is checked against *its parent's* remaining
//! bytes (never the root), recursion depth is capped, and no `size` field is ever used
//! as an allocation length — the tree stores only offsets and borrows leaf payloads.

mod dump;
mod header;
mod tree;
mod walk;

pub use dump::{dump, DumpOptions};
pub use header::{BinSectionHeader, ChunkKind, CONTAINER_FLAG};
pub use tree::ChunkNode;
pub use walk::{walk, walk_with, Visit, WalkOptions, DEFAULT_MAX_DEPTH};

#[cfg(test)]
pub(crate) mod tests {
    /// Build a chunk: 8-byte header (id LE, size LE) followed by `payload`. Shared by the
    /// submodules' tests, which all need to hand-assemble byte buffers.
    pub(crate) fn chunk(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }
}
