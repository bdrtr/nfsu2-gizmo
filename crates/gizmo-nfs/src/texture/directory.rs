//! The TPK directory: the fixed-stride descriptor table and the texture DebugNames beside it.

use crate::chunk::ChunkNode;
use crate::reader::ByteReader;
use crate::types::AssetHash;

/// Descriptor table chunk id (`N × 24` bytes).
pub(super) const DESCRIPTORS: u32 = 0x3331_0003;
/// Bytes per descriptor entry.
const DESCRIPTOR_STRIDE: usize = 24;

/// One texture's 24-byte descriptor within a [`Tpk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TpkEntry {
    /// The texture's asset hash (its key; matches a `material_ref` on a mesh part).
    pub hash: AssetHash,
    /// Whole-file byte offset of the compressed blob.
    pub abs_offset: u32,
    /// Compressed byte length of the blob.
    pub size: u32,
    /// Decompressed size of the blob.
    pub out_size: u32,
    /// Distance from the decompressed end back to the embedded header (`0x100`).
    pub header_from_end: u32,
}

/// Find the payload of the first chunk (top-level or descendant) with `id`.
pub(super) fn find_leaf<'a>(roots: &[ChunkNode], id: u32, root_buf: &'a [u8]) -> Option<&'a [u8]> {
    for r in roots {
        if r.header.id == id {
            return Some(r.data(root_buf));
        }
        if let Some(n) = r.find(id) {
            return Some(n.data(root_buf));
        }
    }
    None
}

/// Parse the fixed-stride descriptor table (trailing partial bytes ignored).
pub(super) fn parse_descriptors(desc: &[u8]) -> Vec<TpkEntry> {
    let count = desc.len() / DESCRIPTOR_STRIDE;
    let mut entries = Vec::with_capacity(count);
    let mut r = ByteReader::new(desc);
    for _ in 0..count {
        // Reads cannot fail: `count` is derived from the actual buffer length.
        let hash = r.u32_le().unwrap_or(0);
        let abs_offset = r.u32_le().unwrap_or(0);
        let size = r.u32_le().unwrap_or(0);
        let out_size = r.u32_le().unwrap_or(0);
        let header_from_end = r.u32_le().unwrap_or(0);
        let _unk = r.u32_le().unwrap_or(0);
        entries.push(TpkEntry {
            hash: AssetHash(hash),
            abs_offset,
            size,
            out_size,
            header_from_end,
        });
    }
    entries
}

/// Read the `DebugName[24]` that precedes the `NameHash` at pool offset `p` (ASCII, NUL-
/// terminated). Empty if it runs off the buffer.
pub(super) fn texture_name(pool: &[u8], p: usize) -> String {
    let start = p.saturating_sub(0x18);
    pool.get(start..p)
        .unwrap_or(&[])
        .iter()
        .take_while(|&&b| b != 0)
        .filter(|&&b| b.is_ascii_graphic() || b == b' ')
        .map(|&b| b as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(fields: [u32; 6]) -> Vec<u8> {
        fields.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    #[test]
    fn parses_descriptor_fields() {
        let d = descriptor([0xAABB_CCDD, 0xAE000, 1123, 11068, 0x100, 0]);
        let entries = parse_descriptors(&d);
        assert_eq!(entries.len(), 1);
        let e = entries[0];
        assert_eq!(e.hash, AssetHash(0xAABB_CCDD));
        assert_eq!(e.abs_offset, 0xAE000);
        assert_eq!(e.size, 1123);
        assert_eq!(e.out_size, 11068);
        assert_eq!(e.header_from_end, 0x100);
    }

    #[test]
    fn a_debug_name_is_read_back_from_before_the_hash() {
        // DebugName[24] sits at P-0x18, NUL-terminated ASCII.
        let mut pool = vec![0u8; 0x18];
        pool[..13].copy_from_slice(b"240SX_BADGING");
        assert_eq!(texture_name(&pool, 0x18), "240SX_BADGING");
    }
}
