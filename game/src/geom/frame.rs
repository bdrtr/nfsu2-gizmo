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
    for p in parts {
        let apply = should_place(&p.transform, &part_centroid(p));
        for v in &p.positions {
            let g = remap(place_point(&p.transform, *v, apply));
            lo = lo.min(g);
            hi = hi.max(g);
        }
    }
    (lo, hi)
}
