//! Bounds-checked little/big-endian cursor over a byte slice.
//!
//! Every primitive read funnels through [`ByteReader::take`] — the single place a short
//! read can occur. It returns [`NfsError::UnexpectedEof`] instead of panicking, which
//! makes "never panic on untrusted input" a local invariant rather than a discipline
//! spread across every parser.

use crate::error::{NfsError, NfsResult};

/// A random-access cursor over `&[u8]` with typed, bounds-checked reads.
#[derive(Debug, Clone)]
pub struct ByteReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    /// Start a cursor at the beginning of `buf`.
    #[must_use]
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Start a cursor at absolute position `pos` (must be within `buf`).
    pub fn at(buf: &'a [u8], pos: usize) -> NfsResult<Self> {
        if pos > buf.len() {
            return Err(NfsError::UnexpectedEof { offset: pos, needed: 0, remaining: buf.len() });
        }
        Ok(Self { buf, pos })
    }

    /// Current absolute offset within the buffer.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Number of unread bytes remaining.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// Whether the cursor has reached the end of the buffer.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    /// Borrow `len` bytes and advance past them. The single bounds-check choke point:
    /// every other read is expressed in terms of this.
    pub fn take(&mut self, len: usize) -> NfsResult<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or(NfsError::UnexpectedEof {
            offset: self.pos,
            needed: len,
            remaining: self.remaining(),
        })?;
        let slice = self.buf.get(self.pos..end).ok_or(NfsError::UnexpectedEof {
            offset: self.pos,
            needed: len,
            remaining: self.remaining(),
        })?;
        self.pos = end;
        Ok(slice)
    }

    /// Advance the cursor by `len` bytes without returning them.
    pub fn skip(&mut self, len: usize) -> NfsResult<()> {
        self.take(len).map(|_| ())
    }

    /// Move the cursor to an absolute position within the buffer.
    pub fn seek(&mut self, pos: usize) -> NfsResult<()> {
        if pos > self.buf.len() {
            return Err(NfsError::UnexpectedEof { offset: pos, needed: 0, remaining: self.buf.len() });
        }
        self.pos = pos;
        Ok(())
    }

    /// Read a fixed-size byte array (the primitive behind the integer readers).
    fn arr<const N: usize>(&mut self) -> NfsResult<[u8; N]> {
        let s = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(s);
        Ok(out)
    }

    /// Read a single byte.
    pub fn u8(&mut self) -> NfsResult<u8> {
        Ok(self.arr::<1>()?[0])
    }

    /// Read a little-endian `u16`.
    pub fn u16_le(&mut self) -> NfsResult<u16> {
        Ok(u16::from_le_bytes(self.arr::<2>()?))
    }

    /// Read a little-endian `u32`.
    pub fn u32_le(&mut self) -> NfsResult<u32> {
        Ok(u32::from_le_bytes(self.arr::<4>()?))
    }

    /// Read a big-endian `u16`.
    pub fn u16_be(&mut self) -> NfsResult<u16> {
        Ok(u16::from_be_bytes(self.arr::<2>()?))
    }

    /// Read a big-endian `u32`.
    pub fn u32_be(&mut self) -> NfsResult<u32> {
        Ok(u32::from_be_bytes(self.arr::<4>()?))
    }

    /// Read a 3-byte big-endian unsigned integer (RefPack/QFS size fields).
    pub fn u24_be(&mut self) -> NfsResult<u32> {
        let b = self.arr::<3>()?;
        Ok((u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]))
    }

    /// Read a little-endian `f32`.
    pub fn f32_le(&mut self) -> NfsResult<f32> {
        Ok(f32::from_le_bytes(self.arr::<4>()?))
    }

    /// Read a NUL-terminated ASCII/UTF-8 string, consuming the terminator.
    pub fn cstr(&mut self) -> NfsResult<&'a str> {
        let start = self.pos;
        let rest = self.buf.get(start..).unwrap_or(&[]);
        match rest.iter().position(|&b| b == 0) {
            Some(n) => {
                let bytes = rest.get(..n).unwrap_or(&[]);
                self.pos = start + n + 1; // consume the NUL
                std::str::from_utf8(bytes).map_err(NfsError::from)
            }
            None => Err(NfsError::UnexpectedEof {
                offset: start,
                needed: rest.len() + 1,
                remaining: rest.len(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_mixed_endianness_and_advances() {
        let data = [0x01, 0x02, 0x03, 0x04, 0xAA, 0xBB];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.u32_le().unwrap(), 0x0403_0201);
        assert_eq!(r.position(), 4);
        assert_eq!(r.u16_be().unwrap(), 0xAABB);
        assert!(r.is_empty());
    }

    #[test]
    fn u24_be_matches_manual() {
        let data = [0x12, 0x34, 0x56];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.u24_be().unwrap(), 0x0012_3456);
    }

    #[test]
    fn take_past_end_errors_instead_of_panicking() {
        let data = [0u8; 3];
        let mut r = ByteReader::new(&data);
        let err = r.take(4).unwrap_err();
        assert!(matches!(err, NfsError::UnexpectedEof { needed: 4, remaining: 3, .. }));
        // Cursor must not have advanced on failure.
        assert_eq!(r.position(), 0);
    }

    #[test]
    fn checked_add_guards_against_overflow() {
        let data = [0u8; 4];
        let mut r = ByteReader::new(&data);
        assert!(r.take(usize::MAX).is_err());
    }

    #[test]
    fn cstr_reads_and_consumes_terminator() {
        let data = b"AB\0CD\0";
        let mut r = ByteReader::new(data);
        assert_eq!(r.cstr().unwrap(), "AB");
        assert_eq!(r.position(), 3);
        assert_eq!(r.cstr().unwrap(), "CD");
        assert_eq!(r.position(), 6);
    }

    #[test]
    fn cstr_without_terminator_errors() {
        let data = b"NOTERM";
        let mut r = ByteReader::new(data);
        assert!(matches!(r.cstr(), Err(NfsError::UnexpectedEof { .. })));
    }
}
