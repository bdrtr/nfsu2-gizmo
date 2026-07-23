//! JDLZ decompression (EA Black Box's LZ variant).
//!
//! JDLZ streams start with the ASCII magic `"JDLZ"`, a version/flags pair, then the
//! uncompressed and compressed sizes (both little-endian `u32`), for a 16-byte header.
//! The body interleaves two 1-bit flag streams (`flags1` selects literal vs. copy;
//! `flags2` selects the copy token's short/long form). Copies are byte-by-byte so
//! overlapping back-references work.
//!
//! The bit-layout below was **validated byte-for-byte** against a real golden pair from
//! an NFSU2 install (`GLOBAL/InGameCommon.lzc` decompresses exactly to
//! `GLOBAL/InGameCommon.bun`); see the env-gated `tests/golden_assets.rs`.
//!
//! Only decompression is implemented — this crate never writes NFS assets.

use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;

/// Refuse to allocate an output larger than this from the (attacker-controlled) size field.
const MAX_OUTPUT: usize = 256 * 1024 * 1024;

/// The 16-byte JDLZ header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JdlzHeader {
    /// Declared size of the decompressed output.
    pub uncompressed_size: u32,
    /// Declared size of the compressed stream (including this header).
    pub compressed_size: u32,
}

/// Parse the JDLZ header. Layout: `"JDLZ"`, two bytes (version/flags), a 2-byte gap,
/// then `uncompressed_size` (LE) and `compressed_size` (LE).
pub fn parse_header(buf: &[u8]) -> NfsResult<JdlzHeader> {
    let mut r = ByteReader::new(buf);
    let magic = r.take(4)?;
    if magic != b"JDLZ" {
        return Err(NfsError::BadMagic { context: "jdlz", found: first_four(buf) });
    }
    let _version = r.u8()?;
    let _flags = r.u8()?;
    let _reserved = r.u16_le()?;
    let uncompressed_size = r.u32_le()?;
    let compressed_size = r.u32_le()?;
    Ok(JdlzHeader { uncompressed_size, compressed_size })
}

/// Decompress a JDLZ stream into its original bytes.
pub fn decompress(buf: &[u8]) -> NfsResult<Vec<u8>> {
    let header = parse_header(buf)?;
    let out_len = header.uncompressed_size as usize;
    if out_len > MAX_OUTPUT {
        return Err(NfsError::Allocation { requested: out_len });
    }

    let mut out: Vec<u8> = Vec::new();
    out.try_reserve(out_len).map_err(|_| NfsError::Allocation { requested: out_len })?;

    let read = |p: usize| -> NfsResult<usize> {
        buf.get(p)
            .map(|&b| b as usize)
            .ok_or(NfsError::UnexpectedEof { offset: p, needed: 1, remaining: 0 })
    };

    let mut in_pos = 16usize; // past the header
    let mut flags1: u32 = 1;
    let mut flags2: u32 = 1;

    // The `| 0x100` sentinel makes a flag word reach the value 1 exactly when its 8 real
    // bits are spent, which is the signal to reload the next flag byte.
    while in_pos < buf.len() && out.len() < out_len {
        if flags1 == 1 {
            flags1 = read(in_pos)? as u32 | 0x100;
            in_pos += 1;
        }
        if flags2 == 1 {
            flags2 = read(in_pos)? as u32 | 0x100;
            in_pos += 1;
        }

        if flags1 & 1 == 1 {
            // A copy token: two bytes, interpreted per the current `flags2` bit.
            let b0 = read(in_pos)?;
            let b1 = read(in_pos + 1)?;
            in_pos += 2;
            let (length, dist) = if flags2 & 1 == 1 {
                // "near" form: length 3..=4098, distance 1..=16
                (((b0 & 0xF0) << 4 | b1) + 3, (b0 & 0x0F) + 1)
            } else {
                // "far" form: length 3..=34, distance 17..~2064
                ((b0 & 0x1F) + 3, (b1 | (b0 & 0xE0) << 3) + 17)
            };
            if dist > out.len() {
                return Err(NfsError::Decompression {
                    codec: "jdlz",
                    detail: "back-reference points before the start of the output",
                });
            }
            let start = out.len() - dist;
            for i in 0..length {
                if out.len() >= out_len {
                    break;
                }
                let byte = *out.get(start + i).ok_or(NfsError::Decompression {
                    codec: "jdlz",
                    detail: "internal copy index out of range",
                })?;
                out.push(byte);
            }
            flags2 >>= 1;
        } else {
            // A literal byte.
            let byte = read(in_pos)? as u8;
            in_pos += 1;
            out.push(byte);
        }
        flags1 >>= 1;
    }

    Ok(out)
}

fn first_four(buf: &[u8]) -> [u8; 4] {
    let mut out = [0u8; 4];
    for (dst, src) in out.iter_mut().zip(buf.iter()) {
        *dst = *src;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header_sizes() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"JDLZ");
        buf.push(0x02); // version
        buf.push(0x10); // flags
        buf.extend_from_slice(&[0, 0]); // reserved
        buf.extend_from_slice(&1000u32.to_le_bytes()); // uncompressed
        buf.extend_from_slice(&320u32.to_le_bytes()); // compressed
        let h = parse_header(&buf).unwrap();
        assert_eq!(h.uncompressed_size, 1000);
        assert_eq!(h.compressed_size, 320);
    }

    #[test]
    fn rejects_bad_magic() {
        // 16+ bytes but the wrong magic must be a clean BadMagic, not a panic.
        assert!(matches!(decompress(b"XXXX_not_jdlz_hdr!!"), Err(NfsError::BadMagic { .. })));
    }

    #[test]
    fn garbage_after_valid_header_does_not_panic() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"JDLZ");
        buf.extend_from_slice(&[0x02, 0x10, 0, 0]);
        buf.extend_from_slice(&64u32.to_le_bytes()); // claims 64 output bytes
        buf.extend_from_slice(&40u32.to_le_bytes());
        buf.extend_from_slice(&[0xAB; 24]); // arbitrary token bytes
        let _ = decompress(&buf); // must return (Ok or Err), never panic
    }

    #[test]
    fn near_copy_round_trips() {
        // Emit literals "ABC" then a near copy (flags2 bit set) of dist=3, length=3 → "ABCABC".
        // flags1 = 0x08: tokens 1-3 literals (bits 0-2 = 0), token 4 copy (bit 3 = 1).
        // flags2 = 0x01: first copy uses the near form (bit 0 = 1).
        // near copy operands: length=((b0&0xF0)<<4|b1)+3=3 and dist=(b0&0x0F)+1=3 → b0=0x02, b1=0x00.
        let mut buf = Vec::new();
        buf.extend_from_slice(b"JDLZ");
        buf.extend_from_slice(&[0x02, 0x10, 0, 0]);
        buf.extend_from_slice(&6u32.to_le_bytes()); // uncompressed
        buf.extend_from_slice(&(buf.len() as u32 + 4 + 7).to_le_bytes()); // compressed (informational)
        buf.push(0x08); // flags1
        buf.push(0x01); // flags2
        buf.extend_from_slice(b"ABC");
        buf.extend_from_slice(&[0x02, 0x00]); // near copy: dist=3 len=3
        assert_eq!(decompress(&buf).unwrap(), b"ABCABC");
    }

    #[test]
    fn literal_only_stream_round_trips() {
        // flags1 = 0x00 -> all eight tokens are literals; flags2 unused. Emit 8 literals.
        let mut buf = Vec::new();
        buf.extend_from_slice(b"JDLZ");
        buf.extend_from_slice(&[0x02, 0x10, 0, 0]);
        buf.extend_from_slice(&8u32.to_le_bytes()); // uncompressed
        buf.extend_from_slice(&26u32.to_le_bytes()); // compressed (informational)
        buf.push(0x00); // flags1: 8 literal bits
        buf.push(0x00); // flags2: consumed once at start, unused thereafter
        buf.extend_from_slice(b"ABCDEFGH");
        assert_eq!(decompress(&buf).unwrap(), b"ABCDEFGH");
    }
}
