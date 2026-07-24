//! What a solid's local matrix *means*: a placement to apply, or a pose already baked into the
//! vertices and to be left alone — and the maths to apply it.
//!
//! This is format semantics, not presentation: the same file carries both kinds of matrix and
//! only the values tell them apart, so every consumer (the engine integration, the `ug2` exporter)
//! must make the same call or the same car comes out differently.

use crate::types::{Mat4, NfsMeshPart};

/// Determinant of a 4x4's upper-left 3x3 — its sign tells a proper placement transform
/// (rotation/scale, det > 0) from a reflection (det < 0).
#[inline]
pub fn det3(m: &Mat4) -> f32 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Mean of a part's local-space vertex positions — how far the part is modelled from the car
/// origin, which distinguishes a placement from an articulation pose (see [`should_place`]).
#[inline]
pub fn part_centroid(p: &NfsMeshPart) -> [f32; 3] {
    let n = p.positions.len().max(1) as f32;
    let mut s = [0.0f32; 3];
    for v in &p.positions {
        s[0] += v[0];
        s[1] += v[1];
        s[2] += v[2];
    }
    [s[0] / n, s[1] / n, s[2] / n]
}

/// Below this translation (L1, metres) a matrix is treated as carrying **no** placement, so the
/// articulation test below decides. A real placement moves a part by tens of centimetres or more
/// (the closest across the fleet: a 6 cm van body, an 8 cm licence plate, a 16 cm mirror); an
/// articulation pose carries only matrix noise, 1–3 cm. Taking any non-zero translation as a
/// placement misread that noise: the taxi and the bus applied their pose and inflated to a 7 m
/// and a 3×-scaled body, and the coupe rolled 90° onto its side.
const MIN_PLACEMENT_TRANSLATION: f32 = 0.05;

/// Decide whether a part's file matrix is a real *placement* to apply, or a transform to leave
/// alone because the vertices are already baked in their assembled position.
///
/// Left as-is:
/// - a **reflection** (`det < 0`): NFSU2's mark for a mirrored right-side part whose vertices
///   are already at their mirrored world position; applying it would flip it back across the
///   centreline.
/// - an **articulation pose** (proper transform, ~zero translation) on a part modelled *off*
///   the origin: the file stores the *open* transform of an animated part whose vertices are
///   already in the assembled *closed* pose. Applying it swings/scales the part around the car
///   origin — the Supra's KIT00 doors carry a 90° X-rotation (scissor-door pose) and fly up and
///   across; the Eclipse's `HOOD_A` carries a det≈15.6 scale and explodes to a 6 m wingspan.
///
/// Applied:
/// - a real **placement**: a *meaningful* translation (the rear wing sits ~2 m back, brake discs).
/// - an **origin-modelled** detail with only a rotation/scale (wheels, brake discs, exhaust
///   tips are modelled at the origin and merely oriented by the matrix): a rotation about the
///   origin does not move an origin-centred part, so applying it is what the clean cars already
///   relied on — keeping it here avoids regressing them.
#[inline]
pub fn should_place(m: &Mat4, centroid: &[f32; 3]) -> bool {
    if det3(m) <= 1e-6 {
        return false;
    }
    let translation = m[3][0].abs() + m[3][1].abs() + m[3][2].abs();
    if translation > MIN_PLACEMENT_TRANSLATION {
        return true;
    }
    // No translation: an origin-modelled part (small centroid) is merely oriented — apply it;
    // an off-origin part is already assembled and the matrix is an articulation pose — skip.
    let dist = (centroid[0] * centroid[0] + centroid[1] * centroid[1] + centroid[2] * centroid[2]).sqrt();
    dist < 0.35
}

/// Place a part's local-space vertex into NFSU2 car space by its file transform. The matrix is
/// row-major with the translation in the last **row** (row-vector convention, `v' = v · M`).
/// `apply` is the per-part decision from [`should_place`].
#[inline]
pub fn place_point(m: &Mat4, p: [f32; 3], apply: bool) -> [f32; 3] {
    if !apply {
        return p;
    }
    [
        p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0] + m[3][0],
        p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1] + m[3][1],
        p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2] + m[3][2],
    ]
}

/// Rotate a part's local-space normal by its file transform's 3x3 (no translation), gated by the
/// same per-part `apply` decision as [`place_point`].
#[inline]
pub fn place_dir(m: &Mat4, n: [f32; 3], apply: bool) -> [f32; 3] {
    if !apply {
        return n;
    }
    [
        n[0] * m[0][0] + n[1] * m[1][0] + n[2] * m[2][0],
        n[0] * m[0][1] + n[1] * m[1][1] + n[2] * m[2][1],
        n[0] * m[0][2] + n[1] * m[1][2] + n[2] * m[2][2],
    ]
}

#[cfg(test)]
mod tests {
    use super::{det3, should_place};
    use crate::types::Mat4;

    // Row-major 4x4 with translation in the last row (the file's `v · M` convention).
    fn m(rows: [[f32; 4]; 4]) -> Mat4 {
        rows
    }

    #[test]
    fn articulation_pose_on_an_off_origin_part_is_skipped() {
        // Supra KIT00 door: a 90° X-rotation with no translation on a part already modelled at
        // its closed position (centroid off-origin, on the left flank). Applying it swung the
        // door up and across — the whole car exploded. It must be treated as baked-and-left.
        let door = m([[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        assert!((det3(&door) - 1.0).abs() < 1e-4, "door matrix is a proper rotation");
        assert!(!should_place(&door, &[-0.03, 0.73, 0.59]));

        // Eclipse KIT00_HOOD_A: a ~2.5x scale (det ≈ 15.6), no translation, off-origin — it blew
        // the car out to a 6 m wingspan. Also an articulation/spurious pose to skip.
        let hood = m([[2.5, 0.0, 0.0, 0.0], [0.0, 2.5, 0.0, 0.0], [0.0, 0.0, 2.5, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        assert!(det3(&hood) > 10.0);
        assert!(!should_place(&hood, &[1.46, 0.0, 0.58]));
    }

    #[test]
    fn real_translation_is_a_placement_and_is_applied() {
        // Supra/Eclipse KIT00_SPOILER: identity 3x3 with a real translation (~2 m back, ~0.8 m
        // up) — the wing is modelled at the origin and genuinely needs placing.
        let spoiler = m([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [-1.997, 0.0, 0.822, 1.0]]);
        assert!(should_place(&spoiler, &[0.13, 0.0, 0.10]));
    }

    #[test]
    fn origin_modelled_detail_keeps_its_orientation_matrix() {
        // These are the clean-car parts the old "apply any det>0" rule already relied on; the
        // fix must keep applying them so 350Z / Skyline don't regress. Each is modelled at the
        // origin (an instanced detail) and merely oriented by the matrix — a rotation about the
        // origin does not move an origin-centred part.
        // Skyline brake disc: 180° about X.
        let brake = m([[1.0, 0.0, 0.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 0.0, -1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        assert!(should_place(&brake, &[0.0, -0.02, 0.0]));
        // 350Z exhaust tip: 90° about Y.
        let exhaust = m([[0.0, 0.0, 1.0, 0.0], [0.0, 1.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        assert!(det3(&exhaust) > 0.0);
        assert!(should_place(&exhaust, &[0.11, -0.03, 0.0]));
    }

    #[test]
    fn centimetre_scale_translation_is_noise_not_a_placement() {
        // TAXI_BODY_A: a 1.6x scale (det ≈ 4.23) whose only translation is 2 cm of matrix noise,
        // on a body modelled off the origin (0.69 m up). Reading that noise as a placement applied
        // the scale and inflated the taxi to a 7.3 m blob; it is an articulation pose to skip.
        let taxi = m([[1.62, 0.0, 0.0, 0.0], [0.0, 1.62, 0.0, 0.0], [0.0, 0.0, 1.62, 0.0], [0.01, 0.0, 0.01, 1.0]]);
        assert!(det3(&taxi) > 4.0);
        assert!(!should_place(&taxi, &[-0.02, 0.0, 0.69]));

        // COUPE_BODY_A: the same shape of noise (3 cm) but a 90° roll about the length axis — it
        // laid the car on its side. det ≈ 1, so only the translation test separates it.
        let coupe = m([[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.01, 0.01, 0.01, 1.0]]);
        assert!(!should_place(&coupe, &[0.01, 0.0, 0.58]));

        // The closest real placement across the fleet — a 6 cm van body offset — still applies.
        let van = m([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.06, 0.0, 0.0, 1.0]]);
        assert!(should_place(&van, &[0.0, 0.0, 0.9]));
    }

    #[test]
    fn a_reflection_is_never_applied() {
        // det < 0 marks an already-mirrored right-side part; applying it flips it back.
        let refl = m([[-1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        assert!(det3(&refl) < 0.0);
        assert!(!should_place(&refl, &[0.0, 0.0, 0.0]));
        // …even a reflection carrying a translation stays baked.
        let refl_t = m([[-1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.5, 0.0, 0.3, 1.0]]);
        assert!(!should_place(&refl_t, &[0.0, 0.8, 0.5]));
    }
}
