//! Error types for NFSU2 asset parsing.
//!
//! House style (matching `gizmo-scene`/`gizmo-renderer`): a hand-written
//! `#[non_exhaustive]` enum with a terse `Display`, a `source()`-chained
//! `std::error::Error`, and `From` glue — no `thiserror`.
//!
//! The core parsers all take `&[u8]` and return [`NfsResult`]; the only variant that
//! carries an `std::io::Error` is [`NfsError::Io`], produced solely by the thin
//! file-loading helpers. Everything below the file boundary is `io`-free and fuzzable.

/// Convenience alias for the crate's fallible results.
pub type NfsResult<T> = Result<T, NfsError>;

/// Anything that can go wrong while parsing an NFSU2 asset file.
///
/// Input is always treated as untrusted: a truncated or malicious file yields one of
/// these errors, never a panic or an unbounded allocation.
#[derive(Debug)]
#[non_exhaustive]
pub enum NfsError {
    /// A read needed more bytes than remain in the buffer.
    UnexpectedEof {
        /// Byte offset at which the read was attempted.
        offset: usize,
        /// Number of bytes the read wanted.
        needed: usize,
        /// Number of bytes actually left.
        remaining: usize,
    },
    /// A chunk's declared size runs past the end of its parent container.
    ChunkOverrun {
        /// Offset of the offending chunk header.
        offset: usize,
        /// The declared payload size.
        size: u32,
        /// Bytes left in the parent at that point.
        parent_remaining: usize,
    },
    /// Chunk nesting exceeded the configured maximum depth (anti-stack-overflow guard).
    MaxDepthExceeded {
        /// The depth limit that was hit.
        max_depth: u32,
    },
    /// A magic / signature check failed (BIGF, JDLZ, RefPack, ...).
    BadMagic {
        /// Human-readable name of the thing whose magic failed.
        context: &'static str,
        /// The first four bytes that were found instead.
        found: [u8; 4],
    },
    /// A BIGF/VIV table-of-contents entry references bytes outside the archive.
    CorruptArchive {
        /// What specifically was inconsistent.
        detail: &'static str,
    },
    /// A compressed stream was malformed (bad opcode, back-reference before the start
    /// of the output, or a size mismatch).
    Decompression {
        /// Which codec produced the error (`"refpack"` / `"jdlz"`).
        codec: &'static str,
        /// What specifically went wrong.
        detail: &'static str,
    },
    /// A size field would require allocating more than we will accept, or the allocation
    /// itself failed. Produced by wrapping `Vec::try_reserve`.
    Allocation {
        /// The number of bytes that were requested.
        requested: usize,
    },
    /// A code path that is deliberately not implemented yet (e.g. JDLZ decoding, which is
    /// ported and validated against real files in a later phase).
    NotImplemented {
        /// Name of the unimplemented feature.
        feature: &'static str,
    },
    /// A geometry triangle index referenced a vertex outside the vertex buffer.
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// The number of vertices available.
        vertex_count: u32,
    },
    /// A vertex/index buffer was too small for the counts declared in the mesh header.
    BufferSizeMismatch {
        /// What specifically did not fit.
        detail: &'static str,
    },
    /// Filesystem I/O failure (only from the thin file-loading helpers).
    Io(std::io::Error),
    /// A name field expected to be ASCII/UTF-8 was not valid UTF-8.
    Utf8(std::str::Utf8Error),
}

impl std::fmt::Display for NfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NfsError::UnexpectedEof { offset, needed, remaining } => write!(
                f,
                "unexpected end of data at offset {offset}: needed {needed} byte(s), {remaining} remaining"
            ),
            NfsError::ChunkOverrun { offset, size, parent_remaining } => write!(
                f,
                "chunk at offset {offset} declares size {size} but only {parent_remaining} byte(s) remain in its parent"
            ),
            NfsError::MaxDepthExceeded { max_depth } => {
                write!(f, "chunk nesting exceeded the maximum depth of {max_depth}")
            }
            NfsError::BadMagic { context, found } => {
                write!(f, "bad {context} signature: found {found:02X?}")
            }
            NfsError::CorruptArchive { detail } => write!(f, "corrupt archive: {detail}"),
            NfsError::Decompression { codec, detail } => {
                write!(f, "{codec} decompression failed: {detail}")
            }
            NfsError::Allocation { requested } => {
                write!(f, "refused or failed to allocate {requested} byte(s)")
            }
            NfsError::NotImplemented { feature } => write!(f, "not implemented yet: {feature}"),
            NfsError::IndexOutOfRange { index, vertex_count } => {
                write!(f, "triangle index {index} out of range ({vertex_count} vertices)")
            }
            NfsError::BufferSizeMismatch { detail } => write!(f, "buffer size mismatch: {detail}"),
            NfsError::Io(_) => write!(f, "file I/O error"),
            NfsError::Utf8(_) => write!(f, "invalid UTF-8 in a name field"),
        }
    }
}

impl std::error::Error for NfsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NfsError::Io(e) => Some(e),
            NfsError::Utf8(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for NfsError {
    fn from(e: std::io::Error) -> Self {
        NfsError::Io(e)
    }
}

impl From<std::str::Utf8Error> for NfsError {
    fn from(e: std::str::Utf8Error) -> Self {
        NfsError::Utf8(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    // `?`/`From` glue: an io::Error must land in the `Io` variant, keep a readable
    // Display, and expose the original error via `source()` for downcasting.
    #[test]
    fn io_error_converts_and_preserves_source() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let err: NfsError = io.into();
        assert!(matches!(err, NfsError::Io(_)));
        assert_eq!(err.to_string(), "file I/O error");
        let src = err.source().expect("Io must expose its source");
        assert!(src.to_string().contains("no such file"));
    }

    // A UTF-8 failure in a name field maps to `Utf8` and chains its source.
    #[test]
    fn utf8_error_converts_and_preserves_source() {
        // Build the bytes at runtime so the `invalid_from_utf8` lint doesn't flag a literal.
        let bytes = vec![0xFFu8, 0xFE];
        let bad = std::str::from_utf8(&bytes).unwrap_err();
        let err: NfsError = bad.into();
        assert!(matches!(err, NfsError::Utf8(_)));
        assert!(err.source().is_some());
    }

    // Data-only variants (the common case) carry no source — they are self-describing.
    #[test]
    fn data_variants_have_no_source_but_distinct_display() {
        let eof = NfsError::UnexpectedEof { offset: 4, needed: 8, remaining: 2 };
        let overrun = NfsError::ChunkOverrun { offset: 0, size: 99, parent_remaining: 10 };
        assert!(eof.source().is_none());
        assert!(overrun.source().is_none());
        assert_ne!(eof.to_string(), overrun.to_string());
        assert!(eof.to_string().contains("offset 4"));
    }
}
