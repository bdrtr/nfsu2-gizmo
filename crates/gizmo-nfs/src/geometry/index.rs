//! The `0x00134B03` index buffer: a `u16` triangle list behind `0x11` alignment filler.

use super::format::FILLER_BYTE;
use crate::error::{NfsError, NfsResult};
use crate::reader::ByteReader;

/// The indices are `tri_count * 3` little-endian u16s that follow the buffer's leading `0x11`
/// alignment filler (the game aligns the index stream to 0x10). They must be read **forward
/// from just after that filler**, not from the tail: some solids (wheels, bumpers) carry a couple
/// of trailing pad bytes, and a tail read would swallow the first real index and pull in the
/// trailing pad — shifting every triangle by one vertex and shredding the mesh into shards. Every
/// index is validated against the vertex count.
pub(super) fn parse_indices(ibuf: &[u8], tri_count: usize, vert_count: usize) -> NfsResult<Vec<u32>> {
    let index_count = tri_count
        .checked_mul(3)
        .ok_or(NfsError::BufferSizeMismatch { detail: "triangle count overflow" })?;
    let needed = index_count
        .checked_mul(2)
        .ok_or(NfsError::BufferSizeMismatch { detail: "index count overflow" })?;
    // Skip the leading 0x11 alignment filler, then take exactly `needed` bytes forward.
    let start = ibuf.iter().take_while(|&&b| b == FILLER_BYTE).count();
    if start.checked_add(needed).is_none_or(|end| end > ibuf.len()) {
        return Err(NfsError::BufferSizeMismatch { detail: "index buffer smaller than tri_count*6" });
    }
    let mut r = ByteReader::at(ibuf, start)?;
    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        let i = r.u16_le()? as u32;
        if i as usize >= vert_count {
            return Err(NfsError::IndexOutOfRange { index: i, vertex_count: vert_count as u32 });
        }
        indices.push(i);
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_read_forward_past_leading_filler_and_ignore_trailing_pad() {
        // The real-world layout that a tail read gets wrong: [0x11 filler][indices][trailing pad].
        // Reading from the tail would drop the first index and swallow the trailing pad, shifting
        // every triangle by one vertex (this was the wheel/bumper "shard" bug). Read forward.
        let mut ibuf = vec![0x11u8; 4]; // leading alignment filler
        for i in [0u16, 1, 2, 2, 3, 0] {
            ibuf.extend_from_slice(&i.to_le_bytes()); // two triangles: (0,1,2),(2,3,0)
        }
        ibuf.extend_from_slice(&[0x00, 0x00]); // trailing pad the tail read must not consume
        let idx = parse_indices(&ibuf, 2, 4).unwrap();
        assert_eq!(idx, vec![0, 1, 2, 2, 3, 0]);
    }

    #[test]
    fn rejects_out_of_range_index() {
        let mut ibuf = Vec::new();
        for i in [0u16, 1, 5] {
            ibuf.extend_from_slice(&i.to_le_bytes());
        }
        assert!(matches!(parse_indices(&ibuf, 1, 2), Err(NfsError::IndexOutOfRange { index: 5, .. })));
    }
}
