//! RefPack / QFS decompression (EA's LZ77-style codec).
//!
//! Header (the NFS variant): a signature byte whose low 7 bits are `0x10` (i.e. `0x10`
//! with an optional `0x01` flag), followed by `0xFB`, then a 3-byte **big-endian**
//! uncompressed size. When the flag bit `0x01` is set, three extra bytes (a second size
//! field) follow and are skipped. The signature may sit at offset 0 or after a 4-byte
//! little-endian compressed-size prefix (offset 4).
//!
//! Opcode stream (fully specified, byte-for-byte): each control byte selects one of five
//! forms that emit some literal bytes and optionally copy a run from earlier in the
//! output. Copies are performed one byte at a time so overlapping back-references (an
//! offset of 1 repeats the previous byte) work correctly.
//!
//! Only decompression is implemented — this crate never writes NFS assets.
//!
//! NOTE: the exact header layout (flag semantics, prefix presence) is the one part most
//! worth double-checking against a real NFSU2 file; the opcode loop below is the
//! canonical, well-documented RefPack algorithm.

use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;

/// A single control byte never asks to allocate more than this on its own; the declared
/// output size (3 bytes) is inherently capped at 16 MiB, but we still guard the reserve.
const MAX_OUTPUT: usize = 64 * 1024 * 1024;

/// Return the offset of the `10 FB` signature (0 or 4), or `None` if absent.
#[must_use]
pub(crate) fn signature_offset(buf: &[u8]) -> Option<usize> {
    for off in [0usize, 4] {
        match (buf.get(off), buf.get(off + 1)) {
            (Some(&a), Some(&b)) if (a & 0xFE) == 0x10 && b == 0xFB => return Some(off),
            _ => {}
        }
    }
    None
}

/// Decompress a RefPack/QFS stream into its original bytes.
pub fn decompress(buf: &[u8]) -> NfsResult<Vec<u8>> {
    let sig = signature_offset(buf).ok_or(NfsError::BadMagic {
        context: "refpack",
        found: first_four(buf),
    })?;
    let mut r = ByteReader::at(buf, sig)?;
    let flag = r.u8()?; // 0x10 (or 0x11 when the size-prefix flag is set)
    let _fb = r.u8()?; // 0xFB
    let out_len = r.u24_be()? as usize;
    if flag & 0x01 != 0 {
        // A second 3-byte size field is present; we don't need it, so skip it.
        r.skip(3)?;
    }
    if out_len > MAX_OUTPUT {
        return Err(NfsError::Allocation { requested: out_len });
    }

    let mut out: Vec<u8> = Vec::new();
    out.try_reserve(out_len).map_err(|_| NfsError::Allocation { requested: out_len })?;

    loop {
        let b0 = r.u8()?;
        let op = decode_opcode(b0, &mut r)?;

        if op.literals > 0 {
            let lit = r.take(op.literals)?;
            out.extend_from_slice(lit);
        }

        if op.copy_len > 0 {
            if op.copy_off == 0 || op.copy_off > out.len() {
                return Err(NfsError::Decompression {
                    codec: "refpack",
                    detail: "back-reference points before the start of the output",
                });
            }
            let start = out.len() - op.copy_off;
            // Byte-by-byte so overlapping copies (offset < len) repeat correctly.
            for i in 0..op.copy_len {
                let byte = *out.get(start + i).ok_or(NfsError::Decompression {
                    codec: "refpack",
                    detail: "internal copy index out of range",
                })?;
                out.push(byte);
            }
        }

        if op.terminate {
            break;
        }
        // Defensive stop: a well-formed stream always ends on a terminator, but if we've
        // already produced the declared length, don't spin on trailing bytes.
        if out_len != 0 && out.len() >= out_len {
            break;
        }
    }

    Ok(out)
}

/// The decoded meaning of one control byte.
struct Opcode {
    literals: usize,
    copy_len: usize,
    copy_off: usize,
    terminate: bool,
}

fn decode_opcode(b0: u8, r: &mut ByteReader<'_>) -> NfsResult<Opcode> {
    let b0u = b0 as usize;
    if b0 < 0x80 {
        // 2-byte form: 0oocccpp oooooooo
        let b1 = r.u8()? as usize;
        Ok(Opcode {
            literals: b0u & 0x03,
            copy_len: ((b0u & 0x1C) >> 2) + 3,
            copy_off: ((b0u & 0x60) << 3) + b1 + 1,
            terminate: false,
        })
    } else if b0 < 0xC0 {
        // 3-byte form: 10cccccc ppoooooo oooooooo
        let b1 = r.u8()? as usize;
        let b2 = r.u8()? as usize;
        Ok(Opcode {
            literals: (b1 & 0xC0) >> 6,
            copy_len: (b0u & 0x3F) + 4,
            copy_off: ((b1 & 0x3F) << 8) + b2 + 1,
            terminate: false,
        })
    } else if b0 < 0xE0 {
        // 4-byte form: 110occpp oooooooo oooooooo cccccccc
        let b1 = r.u8()? as usize;
        let b2 = r.u8()? as usize;
        let b3 = r.u8()? as usize;
        Ok(Opcode {
            literals: b0u & 0x03,
            copy_len: ((b0u & 0x0C) << 6) + b3 + 5,
            copy_off: ((b0u & 0x10) << 12) + (b1 << 8) + b2 + 1,
            terminate: false,
        })
    } else if b0 < 0xFC {
        // 1-byte literal run: 111ppppp (4..112 literals, multiples of 4)
        Ok(Opcode {
            literals: ((b0u & 0x1F) << 2) + 4,
            copy_len: 0,
            copy_off: 0,
            terminate: false,
        })
    } else {
        // Terminator: 111111pp (0..3 final literals)
        Ok(Opcode { literals: b0u & 0x03, copy_len: 0, copy_off: 0, terminate: true })
    }
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

    /// A minimal RefPack encoder used ONLY by these tests: it frames `data` using literal
    /// runs plus a terminator (no back-references), producing a stream our decoder reads.
    /// This validates the header handling, the literal-run opcodes, and the terminator.
    fn encode_literals(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x10, 0xFB];
        let n = data.len() as u32;
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
        out.push(n as u8);

        let mut i = 0usize;
        let full = data.len() - (data.len() % 4); // bytes coverable by (multiple-of-4) runs
        while i < full {
            let run = (full - i).min(112); // multiple of 4, in 4..=112
            out.push(0xE0 | (((run - 4) >> 2) as u8));
            out.extend_from_slice(&data[i..i + run]);
            i += run;
        }
        let rem = data.len() - i; // 0..=3
        out.push(0xFC | (rem as u8));
        out.extend_from_slice(&data[i..]);
        out
    }

    #[test]
    fn round_trips_literal_framing_various_sizes() {
        for len in [0usize, 1, 3, 4, 7, 100, 113, 250, 4096] {
            let data: Vec<u8> = (0..len).map(|i| (i * 31 + 7) as u8).collect();
            let compressed = encode_literals(&data);
            let restored = decompress(&compressed).unwrap();
            assert_eq!(restored, data, "mismatch at len {len}");
        }
    }

    #[test]
    fn decodes_a_non_overlapping_back_reference() {
        // out_len = 8. Emit 4 literal 'A's, then copy 4 bytes from offset 4, then stop.
        // literal run of 4: 0xE0
        // 2-byte copy: literals=0, len=4 -> (b0&0x1C)>>2 = 1 -> b0&0x1C=0x04; off=4 -> b1=3
        let stream = [
            0x10, 0xFB, 0x00, 0x00, 0x08, // header, out_len = 8
            0xE0, b'A', b'A', b'A', b'A', // 4 literal A's
            0x04, 0x03, // copy len=4 off=4 -> "AAAA"
            0xFC, // terminator, 0 trailing literals
        ];
        assert_eq!(decompress(&stream).unwrap(), b"AAAAAAAA");
    }

    #[test]
    fn decodes_an_overlapping_back_reference() {
        // out_len = 5. Opcode with 1 literal 'B' then copy len=4 off=1 -> repeats 'B'.
        // b0: literals=1 (0x01), len=4 (0x04), off=1 (b0&0x60=0, b1=0) -> b0=0x05, b1=0x00
        let stream = [
            0x10, 0xFB, 0x00, 0x00, 0x05, // header, out_len = 5
            0x05, 0x00, b'B', // 1 literal 'B', then copy len=4 off=1
            0xFC, // terminator
        ];
        assert_eq!(decompress(&stream).unwrap(), b"BBBBB");
    }

    #[test]
    fn finds_signature_after_four_byte_prefix() {
        let mut stream = vec![0xDE, 0xAD, 0xBE, 0xEF]; // 4-byte compressed-size prefix
        // out_len=1; terminator 0xFD = 0xFC|1 → one trailing literal 'Z'.
        stream.extend_from_slice(&[0x10, 0xFB, 0x00, 0x00, 0x01, 0xFD, b'Z']);
        assert_eq!(decompress(&stream).unwrap(), b"Z");
    }

    #[test]
    fn missing_signature_errors() {
        assert!(matches!(decompress(&[0, 1, 2, 3]), Err(NfsError::BadMagic { .. })));
    }

    #[test]
    fn back_reference_before_start_errors() {
        // A copy opcode as the very first thing (nothing in the output to copy from).
        let stream = [0x10, 0xFB, 0x00, 0x00, 0x04, 0x04, 0x03, 0xFC];
        assert!(matches!(decompress(&stream), Err(NfsError::Decompression { .. })));
    }
}
