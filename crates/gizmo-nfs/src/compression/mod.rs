//! Compression codecs used by NFSU2 assets.
//!
//! Two schemes appear: EA's **RefPack/QFS** (LZ77-style, magic `10 FB`) and EA Black
//! Box's **JDLZ** (magic ASCII `"JDLZ"`). A `.LZC` file may be *either*, so the codec is
//! always detected by magic bytes, never assumed from the extension.

pub mod jdlz;
pub mod refpack;

use crate::error::NfsResult;

/// A compression codec, as detected from a buffer's leading bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Codec {
    /// EA RefPack / QFS (magic `10 FB`).
    RefPack,
    /// EA Black Box JDLZ (magic `"JDLZ"`).
    Jdlz,
    /// No recognised compression signature — treat the buffer as already plain.
    None,
}

/// Detect the compression codec of `buf` by inspecting its leading bytes.
#[must_use]
pub fn detect(buf: &[u8]) -> Codec {
    if buf.get(..4) == Some(b"JDLZ".as_slice()) {
        Codec::Jdlz
    } else if refpack::signature_offset(buf).is_some() {
        Codec::RefPack
    } else {
        Codec::None
    }
}

/// Decompress `buf` if it carries a recognised codec, otherwise return a plain copy.
pub fn decompress(buf: &[u8]) -> NfsResult<Vec<u8>> {
    match detect(buf) {
        Codec::RefPack => refpack::decompress(buf),
        Codec::Jdlz => jdlz::decompress(buf),
        Codec::None => Ok(buf.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_jdlz_magic() {
        assert_eq!(detect(b"JDLZ\x02\x10\0\0"), Codec::Jdlz);
    }

    #[test]
    fn detects_refpack_magic_at_offset_zero() {
        assert_eq!(detect(&[0x10, 0xFB, 0x00, 0x00, 0x08]), Codec::RefPack);
    }

    #[test]
    fn unknown_bytes_are_plain() {
        assert_eq!(detect(&[1, 2, 3, 4, 5]), Codec::None);
        // decompress of plain data is an identity copy.
        assert_eq!(decompress(&[1, 2, 3]).unwrap(), vec![1, 2, 3]);
    }
}
