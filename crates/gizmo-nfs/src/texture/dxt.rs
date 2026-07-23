//! S3TC / DXT (BC1/BC2/BC3) block decompression to RGBA8.
//!
//! NFSU2 stores most car textures as DXT1 (`0x22`), DXT3 (`0x24`) or DXT5 (`0x26`); this
//! module turns their block data into a tight, row-major RGBA8 buffer. The layout is the
//! universal S3TC one (little-endian RGB565 endpoints, LSB-first 2-bit colour indices), so
//! nothing here is NFS-specific. Panic-free: every read is bounds-checked and missing block
//! bytes decode as transparent black rather than indexing out of range.

/// Expand a 5-6-5 packed colour to RGB888.
#[inline]
fn rgb565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 0x1f) as u8;
    let g = ((c >> 5) & 0x3f) as u8;
    let b = (c & 0x1f) as u8;
    // Scale to 8-bit by replicating the high bits into the low ones.
    [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)]
}

#[inline]
fn u16_le(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d.get(o).copied().unwrap_or(0), d.get(o + 1).copied().unwrap_or(0)])
}

/// The four-entry RGB palette of one BC1-style colour block, plus whether index 3 is the
/// transparent "punch-through" slot (DXT1 only, when `color0 <= color1`).
fn color_palette(block: &[u8], allow_punch_through: bool) -> ([[u8; 3]; 4], bool) {
    let c0 = u16_le(block, 0);
    let c1 = u16_le(block, 2);
    let e0 = rgb565(c0);
    let e1 = rgb565(c1);
    let mut pal = [[0u8; 3]; 4];
    pal[0] = e0;
    pal[1] = e1;
    let punch = allow_punch_through && c0 <= c1;
    if punch {
        // 3-colour block: half-way blend, then a transparent slot.
        for k in 0..3 {
            pal[2][k] = ((e0[k] as u16 + e1[k] as u16) / 2) as u8;
        }
        pal[3] = [0, 0, 0]; // colour ignored; index 3 is transparent
    } else {
        // 4-colour block: 1/3 and 2/3 blends.
        for k in 0..3 {
            pal[2][k] = ((2 * e0[k] as u16 + e1[k] as u16) / 3) as u8;
            pal[3][k] = ((e0[k] as u16 + 2 * e1[k] as u16) / 3) as u8;
        }
    }
    (pal, punch)
}

/// Write the 4x4 colour indices of a BC1-style block into `out` at block `pos = (bx, by)`
/// for an image of `dims = (width, height)`, using `alpha_of(col, row) -> u8` for the alpha
/// channel. `block` is the 8-byte colour half.
fn emit_color_block<A: Fn(usize, usize) -> u8>(
    block: &[u8],
    allow_punch_through: bool,
    pos: (usize, usize),
    dims: (usize, usize),
    out: &mut [u8],
    alpha_of: A,
) {
    let (bx, by) = pos;
    let (width, height) = dims;
    let (pal, punch) = color_palette(block, allow_punch_through);
    let bits = u32::from_le_bytes([
        block.get(4).copied().unwrap_or(0),
        block.get(5).copied().unwrap_or(0),
        block.get(6).copied().unwrap_or(0),
        block.get(7).copied().unwrap_or(0),
    ]);
    for row in 0..4 {
        for col in 0..4 {
            let px = bx * 4 + col;
            let py = by * 4 + row;
            if px >= width || py >= height {
                continue;
            }
            let idx = ((bits >> (2 * (row * 4 + col))) & 0x3) as usize;
            let rgb = pal[idx];
            let a = if punch && idx == 3 { 0 } else { alpha_of(col, row) };
            let o = (py * width + px) * 4;
            if let Some(px4) = out.get_mut(o..o + 4) {
                px4.copy_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
            }
        }
    }
}

/// Decode a DXT1 (BC1) image of `width`x`height` into row-major RGBA8.
#[must_use]
pub fn decode_dxt1(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    let bw = width.div_ceil(4);
    let bh = height.div_ceil(4);
    for by in 0..bh {
        for bx in 0..bw {
            let off = (by * bw + bx) * 8;
            let block = data.get(off..off + 8).unwrap_or(&[]);
            emit_color_block(block, true, (bx, by), (width, height), &mut out, |_, _| 255);
        }
    }
    out
}

/// Decode a DXT3 (BC2) image — explicit 4-bit alpha followed by a BC1 colour block.
#[must_use]
pub fn decode_dxt3(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    let bw = width.div_ceil(4);
    let bh = height.div_ceil(4);
    for by in 0..bh {
        for bx in 0..bw {
            let off = (by * bw + bx) * 16;
            let block = data.get(off..off + 16).unwrap_or(&[]);
            let alpha = &block[..block.len().min(8)];
            let color = block.get(8..16).unwrap_or(&[]);
            // DXT3/5 colour blocks never use punch-through (alpha is separate).
            emit_color_block(color, false, (bx, by), (width, height), &mut out, |col, row| {
                let pixel = row * 4 + col;
                let nib = alpha.get(pixel / 2).copied().unwrap_or(0);
                let a4 = if pixel % 2 == 0 { nib & 0x0f } else { nib >> 4 };
                (a4 << 4) | a4 // 4-bit -> 8-bit
            });
        }
    }
    out
}

/// Decode a DXT5 (BC3) image — two alpha endpoints + 3-bit interpolated alpha indices,
/// followed by a BC1 colour block.
#[must_use]
pub fn decode_dxt5(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    let bw = width.div_ceil(4);
    let bh = height.div_ceil(4);
    for by in 0..bh {
        for bx in 0..bw {
            let off = (by * bw + bx) * 16;
            let block = data.get(off..off + 16).unwrap_or(&[]);
            let a0 = block.first().copied().unwrap_or(0);
            let a1 = block.get(1).copied().unwrap_or(0);
            let alpha = build_alpha_table(a0, a1);
            // 48 bits of 3-bit alpha indices in block[2..8].
            let mut abits: u64 = 0;
            for i in 0..6 {
                abits |= (block.get(2 + i).copied().unwrap_or(0) as u64) << (8 * i);
            }
            let color = block.get(8..16).unwrap_or(&[]);
            emit_color_block(color, false, (bx, by), (width, height), &mut out, |col, row| {
                let pixel = row * 4 + col;
                let idx = ((abits >> (3 * pixel)) & 0x7) as usize;
                alpha[idx]
            });
        }
    }
    out
}

/// The eight interpolated alpha values of a BC3 alpha block.
fn build_alpha_table(a0: u8, a1: u8) -> [u8; 8] {
    let mut a = [0u8; 8];
    a[0] = a0;
    a[1] = a1;
    if a0 > a1 {
        for i in 1..7 {
            a[i + 1] = (((7 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 7) as u8;
        }
    } else {
        for i in 1..5 {
            a[i + 1] = (((5 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 5) as u8;
        }
        a[6] = 0;
        a[7] = 255;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dxt1_solid_color_block() {
        // color0 = pure red (RGB565 0xF800), color1 = 0, all indices 0 -> every pixel red.
        let mut block = [0u8; 8];
        block[0..2].copy_from_slice(&0xF800u16.to_le_bytes());
        block[2..4].copy_from_slice(&0x0000u16.to_le_bytes());
        // indices all 0 (bytes 4..8 = 0)
        let rgba = decode_dxt1(&block, 4, 4);
        assert_eq!(rgba.len(), 4 * 4 * 4);
        // Every pixel should be (255, 0, 0, 255).
        for px in rgba.chunks_exact(4) {
            assert_eq!(px, &[255, 0, 0, 255]);
        }
    }

    #[test]
    fn dxt1_punch_through_transparent() {
        // color0 <= color1 enables the transparent index-3 slot.
        let mut block = [0u8; 8];
        block[0..2].copy_from_slice(&0x0000u16.to_le_bytes()); // c0
        block[2..4].copy_from_slice(&0xF800u16.to_le_bytes()); // c1 (c0 <= c1)
        // set all 16 indices to 3 -> 0b11 repeated = 0xFFFFFFFF
        block[4..8].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let rgba = decode_dxt1(&block, 4, 4);
        for px in rgba.chunks_exact(4) {
            assert_eq!(px[3], 0, "index 3 must be transparent in punch-through mode");
        }
    }

    #[test]
    fn dxt5_alpha_endpoints() {
        // color half = solid via c0=white; alpha endpoints 0 and 255, indices 0 -> alpha 0.
        let mut block = [0u8; 16];
        block[0] = 0; // a0
        block[1] = 255; // a1
        // alpha indices all 0 -> alpha = a0 = 0
        block[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes()); // c0 white
        let rgba = decode_dxt5(&block, 4, 4);
        for px in rgba.chunks_exact(4) {
            assert_eq!(px[3], 0);
        }
    }

    #[test]
    fn non_multiple_of_four_dims_do_not_panic() {
        // A 3x5 texture (partial blocks) must decode without panicking.
        let data = vec![0u8; 2 * 8]; // enough for 1x2 blocks
        let rgba = decode_dxt1(&data, 3, 5);
        assert_eq!(rgba.len(), 3 * 5 * 4);
    }
}
