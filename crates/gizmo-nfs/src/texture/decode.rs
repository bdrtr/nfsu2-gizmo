//! Decoding one texture blob: JDLZ/RefPack decompression, the embedded `OldTextureInfo`
//! header, and the pixel formats behind its `ImageCompressionType` code.

use super::directory::{texture_name, TpkEntry};
use super::dxt;
use crate::error::{NfsError, NfsResult};
use crate::types::{NfsTexture, PixelFormat, TexFormat};

/// Bytes per pixel in the decoded output.
const RGBA: usize = 4;

/// `ImageCompressionType` codes (embedded `OldTextureInfo`, byte at `P+38`).
mod fmt {
    pub const RGBA8888: u8 = 0x20; // 32bpp, stored B,G,R,A
    pub const DXT1: u8 = 0x22;
    pub const DXT3: u8 = 0x24;
    pub const DXT5: u8 = 0x26;
    pub const P8: u8 = 0x08; // palettised — not decoded yet
}

/// The largest texture dimension we will decode (guards allocation from a corrupt header).
const MAX_DIM: usize = 4096;

/// Decompress and decode a single texture into RGBA8. Errors (unsupported codec, malformed
/// header, out-of-range dimensions) mean "skip this texture", not a corrupt file.
pub(super) fn decode_texture(file: &[u8], e: &TpkEntry) -> NfsResult<NfsTexture> {
    let abs = e.abs_offset as usize;
    let end = abs.checked_add(e.size as usize).ok_or(NfsError::CorruptArchive {
        detail: "TPK texture offset+size overflow",
    })?;
    let blob = file
        .get(abs..end)
        .ok_or(NfsError::CorruptArchive { detail: "TPK texture blob out of range" })?;

    // JDLZ / HUFF (HUFF currently returns NotImplemented → this texture is skipped).
    let pool = crate::compression::decompress(blob)?;
    let out_size = e.out_size as usize;
    if pool.len() < out_size {
        return Err(NfsError::BufferSizeMismatch { detail: "TPK blob decompressed short" });
    }

    // Locate the embedded OldTextureInfo header and read the fields we need.
    let p = out_size
        .checked_sub(e.header_from_end as usize)
        .and_then(|h| h.checked_add(0x64 + 0x24))
        .ok_or(NfsError::CorruptArchive { detail: "TPK header offset underflow" })?;
    let hdr = pool
        .get(p..p + 39)
        .ok_or(NfsError::CorruptArchive { detail: "TPK header out of range" })?;
    // Self-check: the u32 at P is the texture's own hash. If it isn't, the header formula
    // doesn't apply to this texture — skip rather than decode noise.
    let name_hash = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    if name_hash != e.hash.0 {
        return Err(NfsError::CorruptArchive { detail: "TPK header hash mismatch" });
    }
    // The `DebugName[24]` sits just before the NameHash (struct 0x0C, i.e. `P − 0x18`); it
    // carries the texture's readable name (e.g. `240SX_KIT00_HEADLIGHT`), which the renderer
    // matches to part names.
    let name = texture_name(&pool, p);
    let width = u16::from_le_bytes([hdr[32], hdr[33]]) as usize;
    let height = u16::from_le_bytes([hdr[34], hdr[35]]) as usize;
    let comp = hdr[38];
    if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
        return Err(NfsError::CorruptArchive { detail: "TPK texture dimensions out of range" });
    }

    let top = top_mip_size(width, height, comp)
        .ok_or(NfsError::NotImplemented { feature: "TPK pixel format" })?;
    let pixels = pool
        .get(0..top)
        .ok_or(NfsError::BufferSizeMismatch { detail: "TPK pixel data shorter than top mip" })?;

    let (rgba, source_format) = match comp {
        fmt::DXT1 => (dxt::decode_dxt1(pixels, width, height), TexFormat::Dxt1),
        fmt::DXT3 => (dxt::decode_dxt3(pixels, width, height), TexFormat::Dxt3),
        fmt::DXT5 => (dxt::decode_dxt5(pixels, width, height), TexFormat::Dxt5),
        fmt::RGBA8888 => (unpack_bgra(pixels, width, height), TexFormat::Unknown(0x20)),
        _ => return Err(NfsError::NotImplemented { feature: "TPK pixel format" }),
    };

    Ok(NfsTexture {
        name,
        hash: e.hash,
        width: width as u32,
        height: height as u32,
        rgba,
        source_format,
        format: PixelFormat::Rgba8,
    })
}

/// Byte size of the top mipmap for `width`x`height` in the given compression type, or `None`
/// for a format we do not decode.
fn top_mip_size(width: usize, height: usize, comp: u8) -> Option<usize> {
    let blocks = width.div_ceil(4) * height.div_ceil(4);
    match comp {
        fmt::DXT1 => Some(blocks * 8),
        fmt::DXT3 | fmt::DXT5 => Some(blocks * 16),
        fmt::RGBA8888 => Some(width * height * RGBA),
        fmt::P8 => Some(width * height), // decode not yet supported, but size is known
        _ => None,
    }
}

/// Unpack the `0x20` format: 32-bit source stored B,G,R,A → RGBA8, alpha preserved.
fn unpack_bgra(src: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * RGBA];
    for (dst, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        dst.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_mip_sizes_match_s3tc() {
        // DXT1 128x128 = 32*32 blocks * 8 = 8192 bytes.
        assert_eq!(top_mip_size(128, 128, fmt::DXT1), Some(8192));
        // DXT3/5 double that.
        assert_eq!(top_mip_size(128, 128, fmt::DXT3), Some(16384));
        assert_eq!(top_mip_size(64, 32, fmt::DXT5), Some(64 / 4 * 32 / 4 * 16));
        // RGBA is 4 bytes/pixel.
        assert_eq!(top_mip_size(16, 16, fmt::RGBA8888), Some(16 * 16 * 4));
        // Unknown format -> None.
        assert_eq!(top_mip_size(16, 16, 0x99), None);
    }

    #[test]
    fn bgra_unpack_reorders_to_rgba() {
        // one BGRA pixel (B=1,G=2,R=3,A=4) -> RGBA (3,2,1,4)
        assert_eq!(unpack_bgra(&[1, 2, 3, 4], 1, 1), vec![3, 2, 1, 4]);
    }
}
