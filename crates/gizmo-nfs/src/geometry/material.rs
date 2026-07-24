//! The solid's material tables: the `0x00134012`/`0x00134013` hash lists and the
//! `0x00134B02` per-material index ranges that split the index buffer.

use super::format::{
    MAT_RANGE_COUNT, MAT_RANGE_MATIDX, MAT_RANGE_OFFSET, MAT_RANGE_SHADERIDX, MAT_RANGE_STRIDE,
};
use crate::reader::ByteReader;
use crate::types::{AssetHash, NfsMaterialRange};

/// The `0x00134012` material list is an array of 8-byte entries (`u32` hash, `u32` zero).
/// The list mixes shader/material hashes with the solid's texture hash(es), in no fixed
/// order, so all non-zero hashes are returned and the texture is resolved downstream against
/// the `TEXTURES.BIN` pack.
pub(super) fn ordered_hashes(data: &[u8]) -> Vec<AssetHash> {
    let count = data.len() / 8;
    let mut hashes = Vec::with_capacity(count);
    let mut r = ByteReader::new(data);
    for _ in 0..count {
        let hash = r.u32_le().unwrap_or(0);
        let _pad = r.u32_le().unwrap_or(0);
        hashes.push(AssetHash(hash));
    }
    hashes
}

/// Parse the `0x00134B02` material list into per-material index runs.
///
/// Each 60-byte entry carries an index `count` and `offset` (into the index buffer) plus a
/// `matidx` into `ordered` (the solid's `0x00134012` hash list). The entries are the trailing
/// `n * 60` bytes; leading bytes are `0x11` alignment filler. Runs whose slice falls outside
/// the index buffer are skipped rather than trusted (panic-free against untrusted input).
pub(super) fn material_ranges(
    data: &[u8],
    ordered: &[AssetHash],
    shaders: &[AssetHash],
    index_count: usize,
) -> Vec<NfsMaterialRange> {
    let n = data.len() / MAT_RANGE_STRIDE;
    if n == 0 {
        return Vec::new();
    }
    let start = data.len() - n * MAT_RANGE_STRIDE;
    let mut runs = Vec::with_capacity(n);
    for i in 0..n {
        let base = start + i * MAT_RANGE_STRIDE;
        let count = u32_at(data, base + MAT_RANGE_COUNT) as usize;
        let matidx = u32_at(data, base + MAT_RANGE_MATIDX) as usize;
        let shaderidx = u32_at(data, base + MAT_RANGE_SHADERIDX) as usize;
        let offset = u32_at(data, base + MAT_RANGE_OFFSET) as usize;
        if count == 0 || offset > index_count || count > index_count - offset {
            continue; // out-of-range run — don't emit a slice we can't trust
        }
        let hash = ordered.get(matidx).copied().unwrap_or(AssetHash(0));
        let shader = shaders.get(shaderidx).copied().unwrap_or(AssetHash(0));
        runs.push(NfsMaterialRange { hash, shader, index_offset: offset, index_count: count });
    }
    runs
}

/// Read a little-endian `u32` at `pos`, or `0` if it is out of range.
fn u32_at(data: &[u8], pos: usize) -> u32 {
    ByteReader::at(data, pos).and_then(|mut r| r.u32_le()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_ranges_split_index_buffer() {
        let ordered = [AssetHash(0xAA), AssetHash(0xBB)];
        let shaders = [AssetHash(0x11), AssetHash(0x22)];
        let entry = |count: u32, matidx: u32, shaderidx: u32, offset: u32| {
            let mut e = [0u32; 15];
            e[3] = count;
            e[7] = matidx;
            e[8] = shaderidx;
            e[13] = offset;
            e.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()
        };
        // 12 bytes leading 0x11 filler, then two runs tiling a 9-index buffer.
        let mut data = vec![0x11u8; 12];
        data.extend(entry(6, 0, 0, 0));
        data.extend(entry(3, 1, 1, 6));
        let runs = material_ranges(&data, &ordered, &shaders, 9);
        assert_eq!(runs, vec![
            NfsMaterialRange { hash: AssetHash(0xAA), shader: AssetHash(0x11), index_offset: 0, index_count: 6 },
            NfsMaterialRange { hash: AssetHash(0xBB), shader: AssetHash(0x22), index_offset: 6, index_count: 3 },
        ]);
        // A run that overshoots the index buffer is dropped, not trusted.
        let mut bad = vec![0x11u8; 12];
        bad.extend(entry(100, 0, 0, 0));
        assert!(material_ranges(&bad, &ordered, &shaders, 9).is_empty());
    }

    #[test]
    fn hash_list_keeps_file_order_including_zero_slots() {
        let mut data = Vec::new();
        for h in [0xAAu32, 0, 0xBB] {
            data.extend_from_slice(&h.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
        }
        assert_eq!(ordered_hashes(&data), vec![AssetHash(0xAA), AssetHash(0), AssetHash(0xBB)]);
    }
}
