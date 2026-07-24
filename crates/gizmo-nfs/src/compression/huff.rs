//! EA "HUFF" Huffman decompression (magic ASCII `"HUFF"`, `0x46465548` little-endian).
//!
//! NFSU2 compresses some `TEXTURES.BIN` textures with this codec (others use
//! [`crate::compression::jdlz`]). It is **order-0 Huffman over bytes** — no LZ/back-reference
//! layer — plus a single "clue" escape symbol used for run-length of the previous byte, for
//! literals that have no Huffman code, and for end-of-stream. An optional whole-buffer delta
//! pre-filter (selected by the stream's type word) is undone at the end.
//!
//! The algorithm is a faithful port of EA's `LZCompress` `HUFF_decompress` (the same library
//! NFSU2 calls natively), cross-checked against `dbalatoni13/nfsmw` and `NFSTools/GlobalLib`
//! and validated on real NFSU2 texture blobs. This uses the simple, quick-table-free decode
//! path (correctness over speed — texture blobs are small).

use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;

/// The four magic bytes at the start of a HUFF stream.
pub const MAGIC: &[u8; 4] = b"HUFF";

/// Refuse to allocate an output larger than this from the (attacker-controlled) size field.
const MAX_OUTPUT: usize = 256 * 1024 * 1024;
/// Maximum Huffman code length the format uses (tables are sized for this).
const MAX_LEN: usize = 32;

/// The 16-byte HUFF header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HuffHeader {
    /// Declared size of the decompressed output.
    pub uncompressed_size: u32,
    /// Declared size of the compressed stream, including this header.
    pub compressed_size: u32,
}

/// Parse the 16-byte HUFF header.
pub fn parse_header(buf: &[u8]) -> NfsResult<HuffHeader> {
    let mut r = ByteReader::new(buf);
    let magic = r.take(4)?;
    if magic != MAGIC.as_slice() {
        return Err(NfsError::BadMagic { context: "huff", found: first_four(buf) });
    }
    let _version = r.u8()?;
    let _header_size = r.u8()?; // 0x10
    let _flags = r.u16_le()?;
    let uncompressed_size = r.u32_le()?;
    let compressed_size = r.u32_le()?;
    Ok(HuffHeader { uncompressed_size, compressed_size })
}

/// A HUFF-specific decompression error.
fn err(detail: &'static str) -> NfsError {
    NfsError::Decompression { codec: "huff", detail }
}

/// MSB-first bit reader over big-endian 16-bit words (EA `SQgetbits` model).
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    /// Working accumulator; the next code sits at the top (bit 31).
    bits: u32,
    /// Signed "bits available" counter, relative to the 16-bit refill unit.
    left: i32,
    /// Rolling raw-byte staging register.
    bu: u32,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8]) -> Self {
        let mut b = Bits { d, pos: 0, bits: 0, left: -16, bu: 0 };
        b.getbits(0); // prime `bits` with the first word
        b
    }

    /// Append the next big-endian 16-bit word to the low half of the staging register.
    fn get16(&mut self) {
        let hi = self.d.get(self.pos).copied().unwrap_or(0) as u32;
        let lo = self.d.get(self.pos + 1).copied().unwrap_or(0) as u32;
        self.bu = (self.bu << 8) | hi;
        self.bu = (self.bu << 8) | lo;
        self.pos += 2;
    }

    /// Take the top `n` bits (MSB-first), refilling from the stream when underflowed.
    fn getbits(&mut self, n: u32) -> u32 {
        let mut v = 0;
        if n != 0 {
            v = self.bits >> (32 - n);
            self.bits = self.bits.wrapping_shl(n);
            self.left -= n as i32;
        }
        if self.left < 0 {
            self.get16();
            self.bits = self.bu.wrapping_shl((-self.left) as u32);
            self.left += 16;
        }
        v
    }

    /// Consume `n` bits already peeked from the accumulator (no value returned).
    fn consume(&mut self, n: u32) {
        self.bits = self.bits.wrapping_shl(n);
        self.left -= n as i32;
        if self.left < 0 {
            self.get16();
            self.bits = self.bu.wrapping_shl((-self.left) as u32);
            self.left += 16;
        }
    }
}

/// EA `SQgetnum`: a variable-length signed integer, values down to −4.
///
/// The length is a unary run of zeros, so a stream *of* zeros — the shape of truncated or
/// garbage input, since the bit reader pads with zeros past the end — would scan forever. Stop
/// at the longest run the format can encode and report a corrupt stream instead: this parser
/// may not panic or hang on untrusted bytes, only fail.
fn getnum(b: &mut Bits) -> NfsResult<i32> {
    if (b.bits as i32) < 0 {
        // Top bit set → tiny value in `-4..=3`.
        return Ok(b.getbits(3) as i32 - 4);
    }
    let mut n: u32 = 2;
    if b.bits >> 16 != 0 {
        // The terminating 1 lies within the top 16 bits: count leading zeros directly.
        loop {
            b.bits = b.bits.wrapping_shl(1);
            n += 1;
            if (b.bits as i32) < 0 {
                break;
            }
        }
        b.bits = b.bits.wrapping_shl(1); // consume the terminating 1
        b.left -= (n - 1) as i32;
        b.getbits(0); // refill only
    } else {
        // A long run of leading zeros crossing the refill boundary.
        loop {
            n += 1;
            if b.getbits(1) != 0 {
                break;
            }
            if n as usize > MAX_LEN {
                return Err(err("unterminated length run"));
            }
        }
    }
    let bias = ((1i64 << n) - 4) as i32;
    // Wrapping, not checked: `n` is bounded above, but a corrupt stream can still yield the
    // full u32 range here, and the callers range-check what they get.
    let raw = if n > 16 {
        let hi = b.getbits(n - 16);
        let lo = b.getbits(16);
        (lo | (hi << 16)) as i32
    } else {
        b.getbits(n) as i32
    };
    Ok(raw.wrapping_add(bias))
}

/// Decompress a HUFF stream into its original bytes.
pub fn decompress(buf: &[u8]) -> NfsResult<Vec<u8>> {
    let header = parse_header(buf)?;
    let out_len = header.uncompressed_size as usize;
    if out_len > MAX_OUTPUT {
        return Err(NfsError::Allocation { requested: out_len });
    }
    let stream = buf.get(16..).ok_or_else(|| err("truncated header"))?;
    let mut b = Bits::new(stream);

    // ── Prologue: type word, uncompressed length, clue byte ──
    let mut type_ = b.getbits(16);
    let ulen: u32 = if type_ & 0x8000 != 0 {
        // "Big" variant: 32-bit sizes.
        if type_ & 0x100 != 0 {
            b.getbits(16);
            b.getbits(16);
        }
        type_ &= !0x100;
        let hi = b.getbits(16);
        let lo = b.getbits(16);
        lo | (hi << 16)
    } else {
        // "Small" variant: 24-bit sizes.
        if type_ & 0x100 != 0 {
            b.getbits(8);
            b.getbits(16);
        }
        type_ &= !0x100;
        let hi = b.getbits(8);
        let lo = b.getbits(16);
        lo | (hi << 16)
    };
    let clue = b.getbits(8) as u8;
    let ulen = ulen as usize;
    if ulen > MAX_OUTPUT {
        return Err(NfsError::Allocation { requested: ulen });
    }

    // ── Canonical code-length table ──
    let mut bitnum = [0i32; MAX_LEN + 1];
    let mut delta = [0u32; MAX_LEN + 1];
    let mut cmp_tbl = [0u32; MAX_LEN + 2];
    let mut numchars = 0i32;
    let mut numbits = 1usize;
    let mut basecmp = 0u32;
    loop {
        if numbits > MAX_LEN {
            return Err(err("code length overflow"));
        }
        basecmp <<= 1;
        delta[numbits] = basecmp.wrapping_sub(numchars as u32);
        let bn = getnum(&mut b)?;
        if !(0..=256).contains(&bn) {
            return Err(err("bad code-length count"));
        }
        bitnum[numbits] = bn;
        numchars += bn;
        basecmp = basecmp.wrapping_add(bn as u32);
        let cmp = if bn != 0 { (basecmp << (16 - numbits.min(16))) & 0xffff } else { 0 };
        cmp_tbl[numbits] = cmp;
        numbits += 1;
        if bn == 0 || cmp != 0 {
            continue;
        }
        break;
    }
    let mostbits = numbits - 1;
    cmp_tbl[mostbits] = 0xffff_ffff; // sentinel
    if numchars <= 0 || numchars > 256 {
        return Err(err("bad symbol count"));
    }
    let numchars = numchars as usize;

    // ── Symbol table via "leap" decode (skip over used slots) ──
    let mut codetbl = [0u8; 256];
    let mut used = [false; 256];
    let mut nextchar: u8 = 0xFF;
    for slot in codetbl.iter_mut().take(numchars) {
        let mut leap = getnum(&mut b)?.saturating_add(1);
        if leap < 1 {
            return Err(err("bad symbol leap"));
        }
        loop {
            nextchar = nextchar.wrapping_add(1);
            if !used[nextchar as usize] {
                leap -= 1;
            }
            if leap == 0 {
                break;
            }
        }
        used[nextchar as usize] = true;
        *slot = nextchar;
    }

    // ── Main decode loop (quick-table-free canonical decode) ──
    let mut out = Vec::new();
    out.try_reserve(ulen).map_err(|_| NfsError::Allocation { requested: ulen })?;
    let mut guard = 0usize;
    let guard_max = ulen.saturating_mul(4).saturating_add(4096);
    while out.len() < ulen {
        guard += 1;
        if guard > guard_max {
            return Err(err("decode did not terminate"));
        }
        // Determine this code's length by comparing the top 16 bits against the limits.
        let cmp16 = b.bits >> 16;
        let mut len = 1usize;
        while len < mostbits && cmp16 >= cmp_tbl[len] {
            len += 1;
        }
        let code_val = b.bits >> (32 - len as u32);
        b.consume(len as u32);
        let idx = code_val.wrapping_sub(delta[len]) as usize;
        let code = *codetbl.get(idx).ok_or_else(|| err("symbol index out of range"))?;

        if code != clue {
            out.push(code);
            continue;
        }
        // Clue escape: run-length, raw literal, or end-of-stream.
        let runlen = getnum(&mut b)?;
        if runlen != 0 {
            if runlen < 0 {
                return Err(err("negative run length"));
            }
            let prev = *out.last().ok_or_else(|| err("run before any output"))?;
            for _ in 0..runlen {
                if out.len() >= ulen {
                    break;
                }
                out.push(prev);
            }
        } else if b.getbits(1) != 0 {
            break; // end of stream
        } else {
            out.push(b.getbits(8) as u8);
        }
    }
    out.truncate(ulen);

    // ── Output post-filter (image delta), selected by the type word ──
    match type_ {
        0x32fb | 0xb2fb => {
            let mut acc = 0u32;
            for x in &mut out {
                acc = acc.wrapping_add(*x as u32);
                *x = acc as u8;
            }
        }
        0x34fb | 0xb4fb => {
            let (mut acc, mut acc2) = (0u32, 0u32);
            for x in &mut out {
                acc = acc.wrapping_add(*x as u32);
                acc2 = acc2.wrapping_add(acc);
                *x = acc2 as u8;
            }
        }
        _ => {}
    }

    Ok(out)
}

fn first_four(buf: &[u8]) -> [u8; 4] {
    let mut out = [0u8; 4];
    for (o, b) in out.iter_mut().zip(buf.iter()) {
        *o = *b;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_filled_stream_fails_instead_of_spinning() {
        // A valid magic + version and nothing else. The bit reader pads with zeros past the end,
        // so `getnum`'s unary length run never terminates: it used to count until `n` overflowed
        // (a panic in debug, a nonsense shift in release). Untrusted input may only fail.
        let mut buf = vec![0u8; 16];
        buf[..4].copy_from_slice(MAGIC);
        buf[4] = 1;
        assert!(matches!(decompress(&buf), Err(NfsError::Decompression { codec: "huff", .. })));
    }

    #[test]
    fn a_truncated_header_is_an_error_not_a_panic() {
        assert!(decompress(&[b'H', b'U', b'F', b'F']).is_err());
        assert!(parse_header(&[]).is_err());
    }
}
