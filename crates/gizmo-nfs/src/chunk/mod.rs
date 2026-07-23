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
//! bounds-checked core: a zero-allocation [`walk`] visitor, and a materialised
//! [`ChunkNode`] tree for ergonomic querying / reverse-engineering.
//!
//! Robustness: every child's declared size is checked against *its parent's* remaining
//! bytes (never the root), recursion depth is capped, and no `size` field is ever used
//! as an allocation length — the tree stores only offsets and borrows leaf payloads.

use crate::error::{NfsError, NfsResult};
use crate::fourcc::FourCc;
use crate::reader::ByteReader;

/// The high bit of a chunk `id`; when set, the chunk is a container.
pub const CONTAINER_FLAG: u32 = 0x8000_0000;

/// Default recursion-depth ceiling for chunk walks (guards against pathological nesting).
pub const DEFAULT_MAX_DEPTH: u32 = 64;

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

/// Whether a container-visitor wants the walker to descend into a container's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visit {
    /// Recurse into the container's sub-chunks.
    Descend,
    /// Treat the container as opaque; do not recurse.
    SkipChildren,
}

/// Options controlling a chunk walk.
#[derive(Debug, Clone, Copy)]
pub struct WalkOptions {
    /// Maximum nesting depth before [`NfsError::MaxDepthExceeded`].
    pub max_depth: u32,
    /// When true, trailing bytes too short to form a header (< 8) are an error; when
    /// false they are tolerated (the common real-world case, due to alignment padding).
    pub strict: bool,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self { max_depth: DEFAULT_MAX_DEPTH, strict: false }
    }
}

/// Walk `buf` as a chunk stream with default options, invoking callbacks for each chunk.
///
/// `on_container(depth, header)` decides whether to descend; `on_leaf(depth, header, data)`
/// receives each leaf's payload. Padding chunks are skipped silently. Never panics.
pub fn walk<C, L>(buf: &[u8], mut on_container: C, mut on_leaf: L) -> NfsResult<()>
where
    C: FnMut(u32, BinSectionHeader) -> NfsResult<Visit>,
    L: FnMut(u32, BinSectionHeader, &[u8]) -> NfsResult<()>,
{
    walk_inner(buf, 0, WalkOptions::default(), &mut on_container, &mut on_leaf)
}

/// Like [`walk`] but with explicit [`WalkOptions`].
pub fn walk_with<C, L>(
    buf: &[u8],
    opts: WalkOptions,
    mut on_container: C,
    mut on_leaf: L,
) -> NfsResult<()>
where
    C: FnMut(u32, BinSectionHeader) -> NfsResult<Visit>,
    L: FnMut(u32, BinSectionHeader, &[u8]) -> NfsResult<()>,
{
    walk_inner(buf, 0, opts, &mut on_container, &mut on_leaf)
}

fn walk_inner<C, L>(
    buf: &[u8],
    depth: u32,
    opts: WalkOptions,
    on_container: &mut C,
    on_leaf: &mut L,
) -> NfsResult<()>
where
    C: FnMut(u32, BinSectionHeader) -> NfsResult<Visit>,
    L: FnMut(u32, BinSectionHeader, &[u8]) -> NfsResult<()>,
{
    let mut r = ByteReader::new(buf);
    while r.remaining() >= 8 {
        let start = r.position();
        let id = r.u32_le()?;
        let size = r.u32_le()?;
        let header = BinSectionHeader { id, size };
        let size_usize = size as usize;
        if size_usize > r.remaining() {
            return Err(NfsError::ChunkOverrun {
                offset: start,
                size,
                parent_remaining: r.remaining(),
            });
        }
        let payload = r.take(size_usize)?;
        match header.kind() {
            ChunkKind::Padding => {}
            ChunkKind::Leaf => on_leaf(depth, header, payload)?,
            ChunkKind::Container => {
                if on_container(depth, header)? == Visit::Descend {
                    let next_depth = depth + 1;
                    if next_depth > opts.max_depth {
                        return Err(NfsError::MaxDepthExceeded { max_depth: opts.max_depth });
                    }
                    walk_inner(payload, next_depth, opts, on_container, on_leaf)?;
                }
            }
        }
    }
    if opts.strict && r.remaining() != 0 {
        return Err(NfsError::UnexpectedEof {
            offset: r.position(),
            needed: 8,
            remaining: r.remaining(),
        });
    }
    Ok(())
}

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

/// Options for [`dump`].
#[derive(Debug, Clone, Copy)]
pub struct DumpOptions {
    /// Maximum nesting depth to descend when dumping.
    pub max_depth: u32,
    /// Number of leading payload bytes to show as hex per leaf (0 = none).
    pub hex_leaf_bytes: usize,
}

impl Default for DumpOptions {
    fn default() -> Self {
        Self { max_depth: DEFAULT_MAX_DEPTH, hex_leaf_bytes: 16 }
    }
}

/// Render `buf`'s chunk tree as an indented, human-readable listing — the primary lever
/// for reverse-engineering an unknown NFSU2 file. Best-effort: it parses non-strict so a
/// partially-understood file still dumps as far as it can.
pub fn dump(buf: &[u8], out: &mut impl std::fmt::Write, opts: DumpOptions) -> NfsResult<()> {
    // Parse the whole tree (generous depth); `opts.max_depth` only limits how deep we
    // *print*, not how deep we parse.
    let nodes = ChunkNode::parse_with(
        buf,
        WalkOptions { max_depth: DEFAULT_MAX_DEPTH, strict: false },
    )?;
    for node in &nodes {
        dump_node(node, buf, out, 0, opts)?;
    }
    Ok(())
}

fn dump_node(
    node: &ChunkNode,
    root: &[u8],
    out: &mut impl std::fmt::Write,
    depth: u32,
    opts: DumpOptions,
) -> NfsResult<()> {
    let indent = "  ".repeat(depth as usize);
    let kind = match node.kind() {
        ChunkKind::Container => "container",
        ChunkKind::Leaf => "leaf",
        ChunkKind::Padding => "padding",
    };
    write!(
        out,
        "{indent}0x{:08X} \"{}\"  size={}  @{}  [{kind}]",
        node.header.id, node.header.fourcc(), node.header.size, node.offset,
    )
    .map_err(|_| NfsError::CorruptArchive { detail: "dump write failed" })?;
    if node.kind() == ChunkKind::Leaf && opts.hex_leaf_bytes > 0 {
        let data = node.data(root);
        let n = opts.hex_leaf_bytes.min(data.len());
        out.write_str("  ").ok();
        for b in data.get(..n).unwrap_or(&[]) {
            write!(out, "{b:02X} ").ok();
        }
    }
    out.write_char('\n').ok();
    if depth < opts.max_depth {
        for child in &node.children {
            dump_node(child, root, out, depth + 1, opts)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a chunk: 8-byte header (id LE, size LE) followed by `payload`.
    fn chunk(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn classifies_header_kinds() {
        assert_eq!(BinSectionHeader { id: 0, size: 4 }.kind(), ChunkKind::Padding);
        assert_eq!(BinSectionHeader { id: 0x0013_4002, size: 4 }.kind(), ChunkKind::Leaf);
        assert_eq!(BinSectionHeader { id: 0x8013_4001, size: 4 }.kind(), ChunkKind::Container);
    }

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
    fn visitor_walk_sees_containers_and_leaves() {
        let leaf = chunk(0x0000_0009, &[1, 2, 3]);
        let container = chunk(0x8000_0007, &leaf);
        let mut leaves = 0usize;
        let mut containers = 0usize;
        walk(
            &container,
            |_d, _h| {
                containers += 1;
                Ok(Visit::Descend)
            },
            |_d, h, data| {
                leaves += 1;
                assert_eq!(h.id, 0x0000_0009);
                assert_eq!(data, &[1, 2, 3]);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(containers, 1);
        assert_eq!(leaves, 1);
    }

    #[test]
    fn skip_children_does_not_recurse() {
        let leaf = chunk(0x0000_0009, &[1]);
        let container = chunk(0x8000_0007, &leaf);
        let mut leaves = 0usize;
        walk(
            &container,
            |_d, _h| Ok(Visit::SkipChildren),
            |_d, _h, _data| {
                leaves += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(leaves, 0, "SkipChildren must not descend");
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
    fn depth_bomb_is_capped() {
        // Deeply nested containers, each wrapping the next, exceeding a tiny max_depth.
        let mut payload = chunk(0x0000_0001, &[0xFF]);
        for _ in 0..10 {
            payload = chunk(0x8000_0001, &payload);
        }
        let opts = WalkOptions { max_depth: 3, strict: false };
        assert!(matches!(
            ChunkNode::parse_with(&payload, opts),
            Err(NfsError::MaxDepthExceeded { max_depth: 3 })
        ));
    }

    #[test]
    fn dump_produces_indented_listing() {
        let leaf = chunk(0x0000_0002, &[0xDE, 0xAD]);
        let container = chunk(0x8000_0001, &leaf);
        let mut s = String::new();
        dump(&container, &mut s, DumpOptions::default()).unwrap();
        assert!(s.contains("0x80000001"));
        assert!(s.contains("[container]"));
        assert!(s.contains("0x00000002"));
        assert!(s.contains("DE AD"));
    }
}
