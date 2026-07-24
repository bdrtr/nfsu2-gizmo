//! Parse NFSU2 `TEXTURES.BIN` (TPK) into per-texture RGBA8 images.
//!
//! The format was reverse-engineered against real cars, cross-checked with the community
//! `xan1242/xnfstpktool` and `NFSTools/GlobalLib` sources.
//!
//! # Layout
//!
//! ```text
//! 0xB3300000  TPK root
//!   0xB3310000  directory
//!     0x33310001  info (name + source .tpk path)
//!     0x33310002  N × (u32 hash, u32 = 0)
//!     0x33310003  ← DESCRIPTORS: N × 24-byte record (below)
//!   0xB3320000  data
//!     0x33320002  the compressed texture blobs
//! ```
//!
//! **Each texture is independent.** The 24-byte descriptor (`0x33310003`, LE `u32`) is:
//!
//! | off | field | role |
//! |-----|-------|------|
//! | 0x00 | `hash`               | asset key |
//! | 0x04 | `abs_offset`         | **whole-file** byte offset of this texture's compressed blob |
//! | 0x08 | `size`               | compressed byte length at `abs_offset` |
//! | 0x0C | `out_size`           | decompressed size of the blob |
//! | 0x10 | `header_from_end`    | distance from the decompressed end back to the header (const `0x100`) |
//! | 0x14 | `unk`                | ignored |
//!
//! To decode one texture: read `file[abs_offset .. abs_offset + size]`, decompress it (JDLZ
//! or HUFF, by magic) into an `out_size`-byte buffer, then read an embedded `OldTextureInfo`
//! header near its tail for the dimensions and pixel format. Pixels always start at buffer
//! offset 0. The header sits at `P = out_size − header_from_end + 0x64 + 0x24`, where the
//! `u32` at `P` is the texture's own hash (a self-check); from `P`: `Width = u16@P+32`,
//! `Height = u16@P+34`, `ImageCompressionType = u8@P+38`. The image is the *top mip* only,
//! decoded by [`dxt`] (DXT1/3/5) or unpacked directly (RGBA).
//!
//! HUFF-compressed textures are skipped until [`crate::compression::huff`] is implemented.
//!
//! The module is split by layer: [`directory`] reads the descriptor table (and the DebugNames
//! beside it), [`decode`] turns one descriptor's blob into RGBA8, and [`dxt`] holds the S3TC
//! block decoders. This file is only the container: find the directory, decode what it lists.

pub mod dxt;
mod decode;
mod directory;

pub use directory::TpkEntry;

use crate::chunk::{ChunkNode, WalkOptions};
use directory::DESCRIPTORS;
use crate::error::{NfsError, NfsResult};
use crate::types::{AssetHash, NfsTexture};
use std::collections::HashMap;

/// A parsed TPK: the raw per-texture descriptors plus every texture we could decode to RGBA8.
///
/// Textures compressed with a codec that is not yet implemented (HUFF) are present in
/// [`entries`](Tpk::entries) but absent from [`textures`](Tpk::textures).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Tpk {
    /// Every texture descriptor, in file order.
    pub entries: Vec<TpkEntry>,
    /// Decoded RGBA8 textures, keyed by hash.
    pub textures: HashMap<AssetHash, NfsTexture>,
}

impl Tpk {
    /// Parse a (raw, on-disk) `TEXTURES.BIN` buffer, decoding every texture whose codec is
    /// supported. Returns an error only if the descriptor chunk is absent or malformed; an
    /// individual texture that fails to decode is skipped, not fatal.
    pub fn parse(bytes: &[u8]) -> NfsResult<Tpk> {
        // Only the directory (near the file start) is needed here — the pixel blobs are read
        // later by absolute offset, never by walking. Tool-compiled TPKs (e.g. nfsu360's
        // Texture Compiler) pack raw compressed blocks after the directory with no wrapping
        // chunk, so a strict full-file walk misreads them and overruns; walk tolerantly so the
        // clean directory still yields the descriptor table.
        let opts = WalkOptions { stop_on_overrun: true, ..Default::default() };
        let roots = ChunkNode::parse_with(bytes, opts)?;
        let desc = directory::find_leaf(&roots, DESCRIPTORS, bytes)
            .ok_or(NfsError::CorruptArchive { detail: "TPK missing descriptor chunk 0x33310003" })?;
        let entries = directory::parse_descriptors(desc);
        let mut textures = HashMap::new();
        for e in &entries {
            if let Ok(tex) = decode::decode_texture(bytes, e) {
                textures.insert(e.hash, tex);
            }
        }
        Ok(Tpk { entries, textures })
    }

    /// Look up a decoded texture by asset hash.
    #[must_use]
    pub fn texture(&self, hash: AssetHash) -> Option<&NfsTexture> {
        self.textures.get(&hash)
    }

    /// Look up a texture descriptor by asset hash (present even for un-decodable codecs).
    #[must_use]
    pub fn entry(&self, hash: AssetHash) -> Option<&TpkEntry> {
        self.entries.iter().find(|e| e.hash == hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a chunk: 8-byte header (id LE, size LE) then `payload`.
    fn chunk(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn missing_descriptor_chunk_is_an_error() {
        // A pixel-data chunk but no descriptor table.
        let file = chunk(0x3332_0002, &[0u8; 8]);
        assert!(matches!(Tpk::parse(&file), Err(NfsError::CorruptArchive { .. })));
    }
}
