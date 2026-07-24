//! Rendering a chunk tree as an indented listing — the primary reverse-engineering lever.

use super::header::ChunkKind;
use super::tree::ChunkNode;
use super::walk::{WalkOptions, DEFAULT_MAX_DEPTH};
use crate::error::{NfsError, NfsResult};

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
        WalkOptions { max_depth: DEFAULT_MAX_DEPTH, strict: false, stop_on_overrun: false },
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
    use crate::chunk::tests::chunk;

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
