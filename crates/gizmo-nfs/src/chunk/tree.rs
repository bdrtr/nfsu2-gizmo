//! The materialised chunk tree: the same bounds-checked walk, kept as nodes for querying.

use super::header::{BinSectionHeader, ChunkKind};
use super::walk::WalkOptions;
use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;

/// A node in a materialised chunk tree.
///
/// Leaf payloads are *not* copied — fetch them lazily via [`ChunkNode::data`] against the
/// original root buffer. All offsets are absolute within that root.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ChunkNode {
    /// This chunk's header.
    pub header: BinSectionHeader,
    /// Absolute offset of this chunk's 8-byte header within the root buffer.
    pub offset: usize,
    /// Absolute offset of this chunk's payload (i.e. `offset + 8`).
    pub data_offset: usize,
    /// Child nodes (containers only; empty for leaves).
    pub children: Vec<ChunkNode>,
}

impl ChunkNode {
    /// Parse an entire buffer into a forest of top-level chunks (default options).
    pub fn parse(buf: &[u8]) -> NfsResult<Vec<ChunkNode>> {
        Self::parse_with(buf, WalkOptions::default())
    }

    /// Parse an entire buffer into a forest of top-level chunks with explicit options.
    pub fn parse_with(buf: &[u8], opts: WalkOptions) -> NfsResult<Vec<ChunkNode>> {
        parse_nodes(buf, 0, 0, opts)
    }

    /// This node's [`ChunkKind`].
    #[must_use]
    pub fn kind(&self) -> ChunkKind {
        self.header.kind()
    }

    /// Borrow this chunk's payload from the original root buffer.
    ///
    /// Returns an empty slice if the recorded offsets fall outside `root` (they never do
    /// for a tree parsed from that same `root`, but this stays panic-free regardless).
    #[must_use]
    pub fn data<'a>(&self, root: &'a [u8]) -> &'a [u8] {
        let end = self.data_offset.saturating_add(self.header.size as usize);
        root.get(self.data_offset..end).unwrap_or(&[])
    }

    /// Find the first descendant (depth-first, self excluded) whose ID equals `id`.
    #[must_use]
    pub fn find(&self, id: u32) -> Option<&ChunkNode> {
        for child in &self.children {
            if child.header.id == id {
                return Some(child);
            }
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }

    /// Collect all descendants (depth-first, self excluded) whose ID equals `id`.
    #[must_use]
    pub fn find_all(&self, id: u32) -> Vec<&ChunkNode> {
        let mut out = Vec::new();
        self.collect(id, &mut out);
        out
    }

    fn collect<'a>(&'a self, id: u32, out: &mut Vec<&'a ChunkNode>) {
        for child in &self.children {
            if child.header.id == id {
                out.push(child);
            }
            child.collect(id, out);
        }
    }
}

fn parse_nodes(buf: &[u8], base: usize, depth: u32, opts: WalkOptions) -> NfsResult<Vec<ChunkNode>> {
    let mut nodes = Vec::new();
    let mut r = ByteReader::new(buf);
    while r.remaining() >= 8 {
        let local_start = r.position();
        let id = r.u32_le()?;
        let size = r.u32_le()?;
        let header = BinSectionHeader { id, size };
        let size_usize = size as usize;
        if size_usize > r.remaining() {
            if opts.stop_on_overrun {
                break;
            }
            return Err(NfsError::ChunkOverrun {
                offset: base + local_start,
                size,
                parent_remaining: r.remaining(),
            });
        }
        let data_local = r.position();
        let payload = r.take(size_usize)?;
        let abs_offset = base + local_start;
        let abs_data = base + data_local;
        match header.kind() {
            ChunkKind::Padding => {}
            ChunkKind::Leaf => nodes.push(ChunkNode {
                header,
                offset: abs_offset,
                data_offset: abs_data,
                children: Vec::new(),
            }),
            ChunkKind::Container => {
                let next_depth = depth + 1;
                if next_depth > opts.max_depth {
                    return Err(NfsError::MaxDepthExceeded { max_depth: opts.max_depth });
                }
                let children = parse_nodes(payload, abs_data, next_depth, opts)?;
                nodes.push(ChunkNode { header, offset: abs_offset, data_offset: abs_data, children });
            }
        }
    }
    if opts.strict && r.remaining() != 0 {
        return Err(NfsError::UnexpectedEof {
            offset: base + r.position(),
            needed: 8,
            remaining: r.remaining(),
        });
    }
    Ok(nodes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::tests::chunk;

    #[test]
    fn parses_nested_tree_with_padding() {
        let leaf_a = chunk(0x0000_0002, &[0xAA, 0xBB]);
        let leaf_b = chunk(0x0000_0003, &[0xCC]);
        let mut inner = Vec::new();
        inner.extend_from_slice(&leaf_a);
        inner.extend_from_slice(&leaf_b);
        let container = chunk(0x8000_0001, &inner);
        let padding = chunk(0x0000_0000, &[0, 0, 0, 0]);
        let mut root = Vec::new();
        root.extend_from_slice(&container);
        root.extend_from_slice(&padding);

        let tree = ChunkNode::parse(&root).unwrap();
        // Padding produced no node → exactly one top-level container.
        assert_eq!(tree.len(), 1);
        let c = &tree[0];
        assert_eq!(c.kind(), ChunkKind::Container);
        assert_eq!(c.children.len(), 2);
        assert_eq!(c.children[0].data(&root), &[0xAA, 0xBB]);
        assert_eq!(c.children[1].data(&root), &[0xCC]);
        // find / find_all locate leaves by id.
        assert!(c.find(0x0000_0003).is_some());
        assert_eq!(c.find_all(0x0000_0002).len(), 1);
    }

    #[test]
    fn oversized_child_is_reported_as_overrun() {
        // A leaf claiming 100 bytes of payload but with only 2 present.
        let mut bad = Vec::new();
        bad.extend_from_slice(&0x0000_0002u32.to_le_bytes());
        bad.extend_from_slice(&100u32.to_le_bytes());
        bad.extend_from_slice(&[0xAA, 0xBB]);
        assert!(matches!(ChunkNode::parse(&bad), Err(NfsError::ChunkOverrun { size: 100, .. })));
    }

    #[test]
    fn stop_on_overrun_keeps_clean_prefix() {
        // Mirror a tool-compiled TPK: a root container whose directory (a clean leaf) is
        // followed by a raw payload region that a strict walk misreads as an oversized chunk.
        let mut inner = chunk(0x0000_0011, &[1, 2, 3, 4]); // the "descriptor" leaf
        inner.extend_from_slice(&0x0000_0099u32.to_le_bytes());
        inner.extend_from_slice(&0x7fff_ffffu32.to_le_bytes()); // absurd size -> overrun
        inner.extend_from_slice(&[0xDE, 0xAD]);
        let buf = chunk(0x8000_0010, &inner);

        // Default: the overrun is fatal and loses the whole tree.
        assert!(matches!(ChunkNode::parse(&buf), Err(NfsError::ChunkOverrun { .. })));

        // Tolerant: the clean prefix (and the descriptor leaf inside it) survives.
        let opts = WalkOptions { stop_on_overrun: true, ..WalkOptions::default() };
        let roots = ChunkNode::parse_with(&buf, opts).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].header.id, 0x8000_0010);
        assert!(roots[0].find(0x0000_0011).is_some(), "the clean leaf before the bad region is kept");
    }

    #[test]
    fn depth_bomb_is_capped() {
        // Deeply nested containers, each wrapping the next, exceeding a tiny max_depth.
        let mut payload = chunk(0x0000_0001, &[0xFF]);
        for _ in 0..10 {
            payload = chunk(0x8000_0001, &payload);
        }
        let opts = WalkOptions { max_depth: 3, strict: false, stop_on_overrun: false };
        assert!(matches!(
            ChunkNode::parse_with(&payload, opts),
            Err(NfsError::MaxDepthExceeded { max_depth: 3 })
        ));
    }
}
