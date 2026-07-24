//! An open file: its bytes, its chunk tree, and everything derived from them.
//!
//! One rule governs this module: **opening must never fail silently, and never fail wholly**. A
//! file whose geometry will not parse still has a chunk tree worth browsing and an error worth
//! reading — the design's whole premise is a tool that says "I am not sure about this" instead of
//! producing nothing. So the tree is walked tolerantly, the geometry pass is separate, and
//! whatever went wrong is kept as a [`Note`] for the log panel.

use gizmo_nfs::chunk::{ChunkKind, ChunkNode, WalkOptions};
use gizmo_nfs::{NfsMeshPart, NfsResult};
use std::path::{Path, PathBuf};

/// How serious a note is. Mirrors the design's log-filter buttons.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    /// The glyph the design shows in the log's leading column.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Self::Info => "·",
            Self::Warn => "⚠",
            Self::Error => "✕",
        }
    }
}

/// One line in the log panel: what happened, and which chunk it happened to.
pub struct Note {
    pub level: Level,
    /// The chunk this is about, by its header offset — the same key selection uses, so clicking a
    /// log row can select the chunk it names.
    pub chunk: Option<usize>,
    /// The chunk id, pre-rendered for the log's middle column.
    pub chunk_id: String,
    pub message: String,
}

/// A flattened tree row: a node plus how deep it sits, so the tree panel can draw indentation
/// without walking the tree itself on every frame.
#[allow(dead_code)] // `size`/`data_offset` are what the inspector and hex panels will read next.
pub struct Row {
    /// Absolute offset of the chunk header — the selection key.
    pub offset: usize,
    pub data_offset: usize,
    pub id: u32,
    pub size: u32,
    pub depth: usize,
    pub container: bool,
    /// Whether the node has children worth expanding.
    pub has_children: bool,
}

/// An open file and everything STRUKT knows about it.
pub struct Doc {
    pub path: PathBuf,
    /// The file as read from disk (decompressed if it carried a codec). Every borrowed slice in
    /// the app points into this, so it outlives the tree by construction.
    pub bytes: Vec<u8>,
    /// Whether the bytes on disk were compressed — the status bar's JDLZ indicator.
    pub codec: gizmo_nfs::compression::Codec,
    /// Top-level chunks; children hang off them.
    pub roots: Vec<ChunkNode>,
    /// Every node, depth-first, in the order the tree draws them.
    pub rows: Vec<Row>,
    /// The parts, when the file is a `GEOMETRY.BIN` that parses. Read by the log's summary today,
    /// by the tree's part names next.
    #[allow(dead_code)]
    pub parts: Vec<NfsMeshPart>,
    /// Anything worth telling the user about the open.
    pub notes: Vec<Note>,
}

impl Doc {
    /// Read and analyse a file. Only a read error is fatal — a malformed *chunk stream* still
    /// yields whatever prefix parsed, with a note saying so.
    pub fn open(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let codec = gizmo_nfs::compression::detect(&raw);
        let mut notes = Vec::new();
        let bytes = match codec {
            gizmo_nfs::compression::Codec::None => raw,
            _ => match gizmo_nfs::compression::decompress(&raw) {
                Ok(b) => {
                    notes.push(Note {
                        level: Level::Info,
                        chunk: None,
                        chunk_id: format!("{codec:?}"),
                        message: format!("{} bayt → {} bayt açıldı", raw.len(), b.len()),
                    });
                    b
                }
                Err(e) => {
                    notes.push(Note {
                        level: Level::Error,
                        chunk: None,
                        chunk_id: format!("{codec:?}"),
                        message: format!("açılamadı: {e} — ham baytlar okunuyor"),
                    });
                    raw
                }
            },
        };

        // Tolerant on purpose: a file whose tail is not a chunk stream (tool-compiled TPKs do this)
        // still shows the part that is.
        let opts = WalkOptions { stop_on_overrun: true, ..WalkOptions::default() };
        let roots = match ChunkNode::parse_with(&bytes, opts) {
            Ok(r) => r,
            Err(e) => {
                notes.push(Note {
                    level: Level::Error,
                    chunk: None,
                    chunk_id: String::new(),
                    message: format!("chunk ağacı okunamadı: {e}"),
                });
                Vec::new()
            }
        };

        let mut rows = Vec::new();
        for r in &roots {
            flatten(r, 0, &mut rows);
        }

        // The geometry pass is separate and allowed to fail: the tree above is still useful.
        let parts = match parse_parts(&bytes) {
            Ok(p) => p,
            Err(e) => {
                notes.push(Note {
                    level: Level::Warn,
                    chunk: None,
                    chunk_id: String::new(),
                    message: format!("geometri okunamadı: {e}"),
                });
                Vec::new()
            }
        };

        notes.push(Note {
            level: Level::Info,
            chunk: None,
            chunk_id: String::new(),
            message: format!("{} chunk · {} parça", rows.len(), parts.len()),
        });

        Ok(Self { path: path.to_path_buf(), bytes, codec, roots, rows, parts, notes })
    }

    /// The node whose header sits at `offset`.
    #[must_use]
    pub fn node_at(&self, offset: usize) -> Option<&ChunkNode> {
        fn find(nodes: &[ChunkNode], offset: usize) -> Option<&ChunkNode> {
            for n in nodes {
                if n.offset == offset {
                    return Some(n);
                }
                // Children are nested strictly inside the parent's span, so only descend when the
                // offset can actually be in there — on a 7 500-node tree that keeps this O(depth).
                if offset > n.offset && offset < n.data_offset + n.header.size as usize {
                    if let Some(found) = find(&n.children, offset) {
                        return Some(found);
                    }
                }
            }
            None
        }
        find(&self.roots, offset)
    }

    /// The deepest chunk that owns byte `pos` — what the hex view highlights under the cursor.
    #[must_use]
    pub fn owner_of_byte(&self, pos: usize) -> Option<&ChunkNode> {
        fn walk<'a>(nodes: &'a [ChunkNode], pos: usize, best: &mut Option<&'a ChunkNode>) {
            for n in nodes {
                let end = n.data_offset + n.header.size as usize;
                if pos >= n.offset && pos < end {
                    *best = Some(n);
                    walk(&n.children, pos, best);
                }
            }
        }
        let mut best = None;
        walk(&self.roots, pos, &mut best);
        best
    }

    /// The file's name, for the status bar and the window title.
    #[must_use]
    pub fn file_name(&self) -> String {
        self.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    }
}

/// Parse geometry, mapping the crate's error into a string the log can show.
fn parse_parts(bytes: &[u8]) -> NfsResult<Vec<NfsMeshPart>> {
    gizmo_nfs::parse_geometry(bytes)
}

/// Depth-first flatten, in draw order.
fn flatten(node: &ChunkNode, depth: usize, out: &mut Vec<Row>) {
    out.push(Row {
        offset: node.offset,
        data_offset: node.data_offset,
        id: node.header.id,
        size: node.header.size,
        depth,
        container: node.kind() == ChunkKind::Container,
        has_children: !node.children.is_empty(),
    });
    for c in &node.children {
        flatten(c, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a chunk: 8-byte header (id LE, size LE) then payload.
    fn chunk(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    /// `Doc::open` reads from disk, so a test needs a file. The name carries the test's own name
    /// because the suite runs in parallel and two tests sharing a path would delete each other's.
    fn doc_of(name: &str, bytes: Vec<u8>) -> Doc {
        let path = std::env::temp_dir().join(format!("strukt-{name}.bin"));
        std::fs::write(&path, &bytes).unwrap();
        let doc = Doc::open(&path).unwrap();
        std::fs::remove_file(&path).ok();
        doc
    }

    #[test]
    fn rows_are_depth_first_and_carry_their_depth() {
        let leaf = chunk(0x0000_0002, &[1, 2, 3]);
        let inner = chunk(0x8000_0001, &leaf);
        let doc = doc_of("depth", inner);
        assert_eq!(doc.rows.len(), 2);
        assert_eq!((doc.rows[0].depth, doc.rows[0].container), (0, true));
        assert_eq!((doc.rows[1].depth, doc.rows[1].container), (1, false));
    }

    #[test]
    fn a_byte_maps_to_the_deepest_chunk_that_owns_it() {
        let leaf = chunk(0x0000_0002, &[1, 2, 3]);
        let inner = chunk(0x8000_0001, &leaf);
        let doc = doc_of("owner", inner);
        // A byte inside the leaf's payload belongs to the leaf, not to the container it sits in.
        let leaf_payload = doc.rows[1].data_offset;
        assert_eq!(doc.owner_of_byte(leaf_payload).map(|n| n.header.id), Some(0x0000_0002));
        // A byte in the container's own header belongs to the container.
        assert_eq!(doc.owner_of_byte(0).map(|n| n.header.id), Some(0x8000_0001));
    }

    #[test]
    fn a_broken_chunk_stream_still_opens_with_a_note() {
        // A leaf claiming far more payload than exists: the tolerant walk keeps the clean prefix,
        // and the file must still open — refusing to show anything is the failure mode this tool
        // exists to avoid.
        let mut bytes = chunk(0x0000_0011, &[1, 2, 3, 4]);
        bytes.extend_from_slice(&0x0000_0099u32.to_le_bytes());
        bytes.extend_from_slice(&0x7fff_ffffu32.to_le_bytes());
        bytes.extend_from_slice(&[0xDE, 0xAD]);
        let doc = doc_of("broken", bytes);
        assert_eq!(doc.rows.len(), 1, "the clean leaf before the bad region survives");
        assert!(doc.notes.iter().any(|n| n.level == Level::Warn || n.level == Level::Error));
    }

    #[test]
    fn node_lookup_finds_a_nested_chunk_by_its_offset() {
        let leaf = chunk(0x0000_0002, &[7; 8]);
        let inner = chunk(0x8000_0001, &leaf);
        let doc = doc_of("lookup", inner);
        let nested = doc.rows[1].offset;
        assert_eq!(doc.node_at(nested).map(|n| n.header.id), Some(0x0000_0002));
        assert_eq!(doc.node_at(nested + 1).map(|n| n.header.id), None, "offsets are exact");
    }
}
