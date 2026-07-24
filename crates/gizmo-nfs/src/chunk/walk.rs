//! The zero-allocation visitor walk over a chunk stream, and the options every consumption
//! style (walk and [`super::tree`]) shares.

use super::header::{BinSectionHeader, ChunkKind};
use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;

/// Default recursion-depth ceiling for chunk walks (guards against pathological nesting).
pub const DEFAULT_MAX_DEPTH: u32 = 64;

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
    /// When true, a chunk whose declared size overruns its parent ends the walk of that
    /// level gracefully (the chunks already parsed are kept) instead of raising
    /// [`NfsError::ChunkOverrun`]. Off by default. Used to read files whose header is a clean
    /// chunk tree but whose trailing payload region is not (e.g. tool-compiled TPKs that pack
    /// raw compressed blocks after the directory, which a strict walk would misread as chunks).
    pub stop_on_overrun: bool,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self { max_depth: DEFAULT_MAX_DEPTH, strict: false, stop_on_overrun: false }
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
            if opts.stop_on_overrun {
                break;
            }
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::tests::chunk;

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
}
