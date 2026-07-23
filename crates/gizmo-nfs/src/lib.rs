//! `gizmo-nfs` — parse Need for Speed: Underground 2 (2004) asset files into
//! **engine-agnostic CPU data**.
//!
//! The crate is a pure, leaf-level data layer: it depends on no `gizmo-*` crate and no
//! GPU/graphics types, so it stays independently testable and reusable. Turning its
//! output ([`NfsCar`], [`NfsMeshPart`], [`NfsTexture`]) into engine meshes/materials is
//! the job of a separate integration layer.
//!
//! # Layers (bottom-up)
//! * [`reader`] — a bounds-checked byte cursor; the panic-free foundation.
//! * [`fourcc`] — printable rendering of 32-bit chunk IDs.
//! * [`chunk`] — the universal `BinSectionHeader` tree ([`chunk::walk`], [`chunk::ChunkNode`], [`chunk::dump`]).
//! * [`compression`] — RefPack/QFS and JDLZ, detected by magic bytes.
//! * [`viv`] — BIGF/VIV archive extraction.
//! * [`types`] — the engine-agnostic output contract.
//!
//! Texture (`TPK`/DXT), geometry (`GEOMETRY.BIN`) and world modules are added in later
//! phases; several of their exact byte layouts have no public spec and are locked
//! empirically with the `nfs_dump` example against a legally-owned install.
//!
//! # Safety / robustness
//! Input is always untrusted. Every read is bounds-checked and returns an [`NfsError`];
//! no parsing path panics, unwraps, or allocates from an unchecked size field.
//!
//! # Legal
//! This crate ships no copyrighted game data. You must own your copy of the game to read
//! its assets; reading is done at runtime from a user-provided install path.

#![forbid(unsafe_code)]

pub mod chunk;
pub mod compression;
pub mod error;
pub mod fourcc;
pub mod geometry;
pub mod reader;
pub mod types;
pub mod viv;

pub use error::{NfsError, NfsResult};
pub use geometry::parse_geometry;
pub use types::{
    AssetHash, LodLevel, Mat4, NfsCar, NfsMeshPart, NfsTexture, PartRole, PixelFormat, TexFormat,
};

/// Read a file from disk and decompress it if it carries a recognised codec.
///
/// This is one of the few functions that touches the filesystem; the resulting bytes can
/// then be fed to the pure `&[u8]`-based parsers ([`viv::VivArchive::parse`],
/// [`chunk::ChunkNode::parse`], ...).
pub fn decompress_file(path: impl AsRef<std::path::Path>) -> NfsResult<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    compression::decompress(&bytes)
}

/// Convenience re-exports for downstream users.
pub mod prelude {
    pub use crate::chunk::{BinSectionHeader, ChunkKind, ChunkNode, Visit};
    pub use crate::compression::{decompress, detect, Codec};
    pub use crate::fourcc::FourCc;
    pub use crate::types::{AssetHash, NfsCar, NfsMeshPart, NfsTexture};
    pub use crate::viv::VivArchive;
    pub use crate::{NfsError, NfsResult};
}
