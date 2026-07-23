//! Printable rendering of 32-bit chunk IDs.
//!
//! NFSU2 chunk IDs are `u32`s stored little-endian. Many are meaningful as four ASCII
//! bytes ("SHPI", "BIGF", ...), while others are opaque numeric tags. [`FourCc`] renders
//! either kind for dumps and readable match arms without ever failing.

/// A 32-bit ID rendered as its four little-endian bytes.
///
/// `Display` shows each byte as its ASCII character when printable, otherwise `.`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FourCc(pub u32);

impl FourCc {
    /// The four little-endian bytes of the ID, in file order.
    #[must_use]
    pub fn bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
}

impl std::fmt::Display for FourCc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.bytes() {
            let c = if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' };
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

impl From<u32> for FourCc {
    fn from(v: u32) -> Self {
        FourCc(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_ascii_tag_in_file_order() {
        // "BIGF" stored little-endian is the u32 0x4647_4942.
        let id = u32::from_le_bytes(*b"BIGF");
        assert_eq!(FourCc(id).to_string(), "BIGF");
        assert_eq!(FourCc(id).bytes(), *b"BIGF");
    }

    #[test]
    fn masks_non_printable_bytes_as_dots() {
        let id = u32::from_le_bytes([0x00, b'A', 0xFF, b'Z']);
        assert_eq!(FourCc(id).to_string(), ".A.Z");
    }
}
