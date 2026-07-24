//! The NFSU2 → Gizmo coordinate frame, and bounds measured in it.

use super::place::{part_centroid, place_point, should_place};
use gizmo::prelude::*;
use gizmo_nfs::NfsMeshPart;

/// NFSU2 (Z-up, X = length, Y = width) → Gizmo drive frame: length → −Z (forward),
/// height → +Y. All mesh building and bounds go through this so the whole car shares one
/// consistent frame.
#[inline]
#[must_use]
pub fn remap(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
}

/// Axis-aligned bounds of `parts` in the Gizmo frame (after [`remap`]).
#[must_use]
pub fn bbox(parts: &[&NfsMeshPart]) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut any = false;
    for p in parts {
        let apply = should_place(&p.transform, &part_centroid(p));
        for v in &p.positions {
            let g = remap(place_point(&p.transform, *v, apply));
            lo = lo.min(g);
            hi = hi.max(g);
            any = true;
        }
    }
    // An empty set would otherwise return (+inf, -inf), whose midpoint is NaN — and the caller
    // subtracts that midpoint from every vertex, so one model with no renderable part would
    // poison a whole car's geometry silently. `CARS/WHEELS/GEOMETRY.BIN` is exactly that: a
    // 64-byte stub next to the real per-brand rim files.
    if any {
        (lo, hi)
    } else {
        (Vec3::ZERO, Vec3::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bounds_are_zero_not_infinite() {
        let (lo, hi) = bbox(&[]);
        assert_eq!((lo, hi), (Vec3::ZERO, Vec3::ZERO));
        // The midpoint the callers recenter by must be a real number.
        assert!(((lo + hi) * 0.5).is_finite());
    }

    #[test]
    fn remap_puts_length_on_negative_z_and_height_on_y() {
        // NFSU2 raw: x = length (nose +x), y = width, z = height.
        assert_eq!(remap([1.0, 0.0, 0.0]), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(remap([0.0, 1.0, 0.0]), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(remap([0.0, 0.0, 1.0]), Vec3::new(0.0, 1.0, 0.0));
    }
}
