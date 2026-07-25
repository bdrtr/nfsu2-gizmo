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
//! * [`placement`] — what a solid's local matrix means (a placement to apply, or a pose already
//!   baked into the vertices), which every consumer must decide the same way.
//! * [`parts`] — which parts make up a given configuration of a car, and which material group
//!   each belongs to. Pure name/triangle-count policy, shared by the engine layer and the CLI.
//! * [`inspect`] — a chunk's bytes read back as labelled fields, for anything that shows a file's
//!   structure. It reads through the same offset map the parser does, so a viewer cannot drift
//!   from the reader about what a file says.
//! * [`validate`] — the checks a person would run by hand after decoding something: stride,
//!   bounding box, normals, index range, chunk bounds. Each records what it examined, so "clean"
//!   is never confused with "unread".
//!
//! [`texture`] parses `TPK` (`TEXTURES.BIN`) into an RGBA8 pixel pool + per-texture
//! descriptors; [`geometry`] parses `GEOMETRY.BIN`. World modules come later. Several of
//! these byte layouts have no public spec and were locked empirically with the `ug2` tool against a legally-owned install.
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
pub mod diff;
pub mod discover;
pub mod error;
pub mod export;
pub mod fourcc;
pub mod geometry;
pub mod globalb;
pub mod inspect;
pub mod parts;
pub mod placement;
pub mod reader;
pub mod texture;
pub mod types;
pub mod validate;
pub mod viv;

pub use error::{NfsError, NfsResult};
pub use geometry::{parse_geometry, parse_geometry_reporting, SkipReason, Skipped};
pub use globalb::{parse_cartypeinfos, CarTypeInfo, WheelSpec};
pub use parts::{group_of, select_car, select_stock_car, CarConfig, Grp};
pub use texture::{Tpk, TpkEntry};
pub use types::{
    AssetHash, LodLevel, Mat4, NfsCar, NfsMaterialRange, NfsMeshPart, NfsTexture, PartRole,
    PixelFormat, TexFormat,
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
    pub use crate::texture::{Tpk, TpkEntry};
    pub use crate::types::{AssetHash, NfsCar, NfsMeshPart, NfsTexture};
    pub use crate::viv::VivArchive;
    pub use crate::{NfsError, NfsResult};
}
