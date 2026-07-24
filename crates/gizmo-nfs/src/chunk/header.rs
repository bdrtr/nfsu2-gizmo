//! The 8-byte chunk header and what its `id` says about the chunk.

use crate::fourcc::FourCc;

/// The high bit of a chunk `id`; when set, the chunk is a container.
pub const CONTAINER_FLAG: u32 = 0x8000_0000;

/// The 8-byte header that precedes every chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinSectionHeader {
    /// The chunk type ID (little-endian on disk).
    pub id: u32,
    /// Size in bytes of the payload that follows the header.
    pub size: u32,
}

/// How a chunk header should be interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChunkKind {
    /// Holds a sequence of sub-chunks (recurse into the payload).
    Container,
    /// Holds raw payload data.
    Leaf,
    /// Alignment filler (`id == 0`); skip its payload.
    Padding,
}

impl BinSectionHeader {
    /// Classify this header as container / leaf / padding.
    #[must_use]
    pub fn kind(&self) -> ChunkKind {
        if self.id == 0 {
            ChunkKind::Padding
        } else if self.id & CONTAINER_FLAG != 0 {
            ChunkKind::Container
        } else {
            ChunkKind::Leaf
        }
    }

    /// The ID rendered as a printable four-character code.
    #[must_use]
    pub fn fourcc(&self) -> FourCc {
        FourCc(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_header_kinds() {
        assert_eq!(BinSectionHeader { id: 0, size: 4 }.kind(), ChunkKind::Padding);
        assert_eq!(BinSectionHeader { id: 0x0013_4002, size: 4 }.kind(), ChunkKind::Leaf);
        assert_eq!(BinSectionHeader { id: 0x8013_4001, size: 4 }.kind(), ChunkKind::Container);
    }
}
