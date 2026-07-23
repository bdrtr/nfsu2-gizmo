//! BIGF / VIV archive reader (NFSU2's per-car packaging).
//!
//! Layout:
//! ```text
//! char[4] "BIGF"
//! u32     archiveSize     // endianness varies by file — validated, not assumed
//! u32(BE) numFiles
//! u32(BE) headerSize      // offset where file data begins
//! numFiles × { u32(BE) offset, u32(BE) size, cstr name }   // table of contents
//! ... padding to headerSize, then the raw file blobs at their offsets
//! ```
//!
//! All returned slices borrow the original buffer (zero-copy); a contained file that is
//! itself compressed is decompressed by the caller via [`crate::compression`].

use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;

/// One entry in a BIGF archive's table of contents.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VivEntry {
    /// The entry's file name.
    pub name: String,
    /// Absolute byte offset of the entry's data within the archive.
    pub offset: u32,
    /// Size of the entry's data in bytes.
    pub size: u32,
}

/// A parsed BIGF/VIV archive over a borrowed buffer.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VivArchive<'a> {
    buf: &'a [u8],
    /// The table of contents, in file order.
    pub entries: Vec<VivEntry>,
    /// Which `archiveSize` endianness matched the buffer length (`true` = big-endian).
    /// This resolves the documented per-file ambiguity for that one field.
    pub big_endian_size: bool,
}

impl<'a> VivArchive<'a> {
    /// Parse a BIGF archive, validating the magic and every table-of-contents entry.
    pub fn parse(buf: &'a [u8]) -> NfsResult<VivArchive<'a>> {
        let mut r = ByteReader::new(buf);
        let magic = r.take(4)?;
        if magic != b"BIGF" {
            return Err(NfsError::BadMagic { context: "BIGF", found: first_four(buf) });
        }

        // archiveSize: read both interpretations; note which matches the real length.
        let size_bytes = r.take(4)?;
        let arr = [size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3]];
        let size_be = u32::from_be_bytes(arr) as usize;
        let size_le = u32::from_le_bytes(arr) as usize;
        let big_endian_size = size_be == buf.len() || size_le != buf.len();

        let num_files = r.u32_be()? as usize;
        let _header_size = r.u32_be()?;

        // Sanity-check the file count before trusting it: each entry needs at least
        // 4 + 4 + 2 bytes (offset, size, one-char name + NUL).
        const MIN_ENTRY_BYTES: usize = 10;
        if num_files.saturating_mul(MIN_ENTRY_BYTES) > buf.len() {
            return Err(NfsError::CorruptArchive { detail: "implausible file count" });
        }

        let mut entries = Vec::new();
        entries
            .try_reserve(num_files)
            .map_err(|_| NfsError::Allocation { requested: num_files })?;

        for _ in 0..num_files {
            let offset = r.u32_be()?;
            let size = r.u32_be()?;
            let name = r.cstr()?.to_owned();
            // Every entry must lie fully within the archive.
            let end = (offset as usize)
                .checked_add(size as usize)
                .ok_or(NfsError::CorruptArchive { detail: "entry offset+size overflows" })?;
            if end > buf.len() {
                return Err(NfsError::CorruptArchive { detail: "entry extends past end of archive" });
            }
            entries.push(VivEntry { name, offset, size });
        }

        Ok(VivArchive { buf, entries, big_endian_size })
    }

    /// Look up an entry by exact name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&VivEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Borrow an entry's raw (possibly still-compressed) data.
    pub fn data(&self, entry: &VivEntry) -> NfsResult<&'a [u8]> {
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        self.buf.get(start..end).ok_or(NfsError::CorruptArchive {
            detail: "entry data slice out of range",
        })
    }

    /// Convenience: look up by name and return its data.
    pub fn read(&self, name: &str) -> NfsResult<&'a [u8]> {
        let entry = self
            .get(name)
            .ok_or(NfsError::CorruptArchive { detail: "no such entry" })?;
        self.data(entry)
    }

    /// Iterate the table of contents.
    pub fn iter(&self) -> impl Iterator<Item = &VivEntry> {
        self.entries.iter()
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

    /// Build a minimal valid BIGF archive from `(name, data)` pairs.
    fn build_bigf(files: &[(&str, &[u8])]) -> Vec<u8> {
        // TOC size: 16-byte header + per-entry (4 + 4 + name.len() + 1).
        let toc_len: usize = 16
            + files
                .iter()
                .map(|(n, _)| 4 + 4 + n.len() + 1)
                .sum::<usize>();
        // Lay data blobs right after the TOC.
        let mut offsets = Vec::new();
        let mut cursor = toc_len;
        for (_, d) in files {
            offsets.push(cursor as u32);
            cursor += d.len();
        }
        let total = cursor;

        let mut out = Vec::new();
        out.extend_from_slice(b"BIGF");
        out.extend_from_slice(&(total as u32).to_be_bytes()); // archiveSize (BE)
        out.extend_from_slice(&(files.len() as u32).to_be_bytes()); // numFiles (BE)
        out.extend_from_slice(&(toc_len as u32).to_be_bytes()); // headerSize (BE)
        for ((name, _), off) in files.iter().zip(offsets.iter()) {
            out.extend_from_slice(&off.to_be_bytes());
            out.extend_from_slice(&(files_data_len(name, files)).to_be_bytes());
            out.extend_from_slice(name.as_bytes());
            out.push(0);
        }
        for (_, d) in files {
            out.extend_from_slice(d);
        }
        assert_eq!(out.len(), total);
        out
    }

    fn files_data_len(name: &str, files: &[(&str, &[u8])]) -> u32 {
        files.iter().find(|(n, _)| *n == name).map(|(_, d)| d.len() as u32).unwrap_or(0)
    }

    #[test]
    fn parses_toc_and_reads_entries() {
        let archive = build_bigf(&[("GEOMETRY.BIN", &[1, 2, 3, 4]), ("TEXTURES.BIN", &[9, 8])]);
        let viv = VivArchive::parse(&archive).unwrap();
        assert_eq!(viv.entries.len(), 2);
        assert_eq!(viv.read("GEOMETRY.BIN").unwrap(), &[1, 2, 3, 4]);
        assert_eq!(viv.read("TEXTURES.BIN").unwrap(), &[9, 8]);
        assert!(viv.get("MISSING").is_none());
        assert!(viv.big_endian_size);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bad = vec![b'N', b'O', b'P', b'E'];
        bad.extend_from_slice(&[0u8; 12]);
        assert!(matches!(VivArchive::parse(&bad), Err(NfsError::BadMagic { .. })));
    }

    #[test]
    fn rejects_entry_pointing_past_end() {
        let mut archive = build_bigf(&[("A", &[1, 2])]);
        // Corrupt the first entry's size (at offset 16 + 4) to a huge value.
        let size_pos = 16 + 4;
        archive[size_pos..size_pos + 4].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        assert!(matches!(VivArchive::parse(&archive), Err(NfsError::CorruptArchive { .. })));
    }

    #[test]
    fn rejects_implausible_file_count() {
        let mut archive = build_bigf(&[("A", &[1])]);
        // Set numFiles (offset 8, BE) to something absurd.
        archive[8..12].copy_from_slice(&0x00FF_FFFFu32.to_be_bytes());
        assert!(matches!(VivArchive::parse(&archive), Err(NfsError::CorruptArchive { .. })));
    }
}
