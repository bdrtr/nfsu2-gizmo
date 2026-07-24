//! Turning parsed CPU parts into Gizmo GPU meshes and placing them in the world.
//!
//! This is the engine-coupled counterpart to [`crate::part_groups`]: it depends on `gizmo`
//! and `wgpu`, expands the parser's *indexed* geometry into the flat vertex list the
//! renderer wants, and applies the NFSU2 → Gizmo coordinate remap.

use gizmo::prelude::*;
use gizmo::renderer::gpu_types::Vertex;
use gizmo_nfs::{Mat4, NfsMeshPart};

/// NFSU2 (Z-up, X = length, Y = width) → Gizmo drive frame: length → −Z (forward),
/// height → +Y. All mesh building and bounds go through this so the whole car shares one
/// consistent frame.
#[inline]
#[must_use]
pub fn remap(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
}

/// Determinant of a 4x4's upper-left 3x3 — its sign tells a proper placement transform
/// (rotation/scale, det > 0) from a reflection (det < 0).
#[inline]
fn det3(m: &Mat4) -> f32 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Mean of a part's local-space vertex positions — how far the part is modelled from the car
/// origin, which distinguishes a placement from an articulation pose (see [`should_place`]).
#[inline]
fn part_centroid(p: &NfsMeshPart) -> [f32; 3] {
    let n = p.positions.len().max(1) as f32;
    let mut s = [0.0f32; 3];
    for v in &p.positions {
        s[0] += v[0];
        s[1] += v[1];
        s[2] += v[2];
    }
    [s[0] / n, s[1] / n, s[2] / n]
}

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
/// - a real **placement**: non-zero translation (the rear wing sits ~2 m back, brake discs).
/// - an **origin-modelled** detail with only a rotation/scale (wheels, brake discs, exhaust
///   tips are modelled at the origin and merely oriented by the matrix): a rotation about the
///   origin does not move an origin-centred part, so applying it is what the clean cars already
///   relied on — keeping it here avoids regressing them.
#[inline]
fn should_place(m: &Mat4, centroid: &[f32; 3]) -> bool {
    if det3(m) <= 1e-6 {
        return false;
    }
    let translation = m[3][0].abs() + m[3][1].abs() + m[3][2].abs();
    if translation > 1e-4 {
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
fn place_point(m: &Mat4, p: [f32; 3], apply: bool) -> [f32; 3] {
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
fn place_dir(m: &Mat4, n: [f32; 3], apply: bool) -> [f32; 3] {
    if !apply {
        return n;
    }
    [
        n[0] * m[0][0] + n[1] * m[1][0] + n[2] * m[2][0],
        n[0] * m[0][1] + n[1] * m[1][1] + n[2] * m[2][1],
        n[0] * m[0][2] + n[1] * m[1][2] + n[2] * m[2][2],
    ]
}

/// Attach a `Transform` plus its matching `GlobalTransform` to an entity — the pair the
/// renderer needs to place anything in the world.
pub fn add_transform(world: &mut World, entity: gizmo::core::Entity, t: Transform) {
    world.add_component(entity, t);
    world.add_component(entity, GlobalTransform { matrix: t.local_matrix });
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

/// Build a solid axis-aligned box mesh spanning `min`..`max` — used as a dark cabin filler
/// so the camera can't see through the glass-less window openings into the hollow body shell.
#[must_use]
pub fn build_box(device: &wgpu::Device, min: Vec3, max: Vec3, label: &str) -> Mesh {
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    // Six faces, each an outward normal and its four corners (CCW seen from outside).
    let faces: [(Vec3, [Vec3; 4]); 6] = [
        (Vec3::X, [v(max.x, min.y, max.z), v(max.x, min.y, min.z), v(max.x, max.y, min.z), v(max.x, max.y, max.z)]),
        (Vec3::NEG_X, [v(min.x, min.y, min.z), v(min.x, min.y, max.z), v(min.x, max.y, max.z), v(min.x, max.y, min.z)]),
        (Vec3::Y, [v(min.x, max.y, max.z), v(max.x, max.y, max.z), v(max.x, max.y, min.z), v(min.x, max.y, min.z)]),
        (Vec3::NEG_Y, [v(min.x, min.y, min.z), v(max.x, min.y, min.z), v(max.x, min.y, max.z), v(min.x, min.y, max.z)]),
        (Vec3::Z, [v(min.x, min.y, max.z), v(max.x, min.y, max.z), v(max.x, max.y, max.z), v(min.x, max.y, max.z)]),
        (Vec3::NEG_Z, [v(max.x, min.y, min.z), v(min.x, min.y, min.z), v(min.x, max.y, min.z), v(max.x, max.y, min.z)]),
    ];
    let mut verts = Vec::with_capacity(36);
    for (n, c) in faces {
        for &i in &[0usize, 1, 2, 0, 2, 3] {
            verts.push(Vertex {
                position: [c[i].x, c[i].y, c[i].z],
                normal: [n.x, n.y, n.z],
                ..Default::default()
            });
        }
    }
    Mesh::from_vertices(device, &verts, label.to_string())
}

/// Build one GPU mesh from a set of parts, remapped into the Gizmo frame and recentered by
/// `off` (usually the car centre). Returns `None` if the parts contribute no triangles.
#[must_use]
pub fn build_mesh(device: &wgpu::Device, parts: &[&NfsMeshPart], off: Vec3, label: &str) -> Option<Mesh> {
    build_mesh_inflated(device, parts, off, |_| 0.0, label)
}

/// Like [`build_mesh`], but each part's vertices are pushed along their normals by
/// `inflate(part)` metres. A small negative inflation on one of two near-coplanar shells
/// (e.g. a car's shared `BASE` body under a kit panel) sinks it behind the other and breaks
/// the depth-fighting they would otherwise flicker into.
#[must_use]
pub fn build_mesh_inflated(
    device: &wgpu::Device,
    parts: &[&NfsMeshPart],
    off: Vec3,
    inflate: impl Fn(&NfsMeshPart) -> f32,
    label: &str,
) -> Option<Mesh> {
    let items: Vec<(&NfsMeshPart, &[u32])> = parts.iter().map(|p| (*p, p.indices.as_slice())).collect();
    build_mesh_items(device, &items, off, inflate, label)
}

/// The core mesh builder: each item is a part paired with the exact slice of *its* index
/// buffer to draw. This is how one solid whose triangles are split across several materials
/// (headlights, brake lights, glass…) becomes several sub-meshes — pass each material run's
/// index slice as its own item. `build_mesh`/`build_mesh_inflated` are the whole-part cases.
#[must_use]
pub fn build_mesh_items(
    device: &wgpu::Device,
    items: &[(&NfsMeshPart, &[u32])],
    off: Vec3,
    inflate: impl Fn(&NfsMeshPart) -> f32,
    label: &str,
) -> Option<Mesh> {
    let mut verts = Vec::new();
    for (p, indices) in items {
        let has_n = !p.normals.is_empty();
        let push = inflate(p);
        let apply = should_place(&p.transform, &part_centroid(p));
        for &idx in *indices {
            let i = idx as usize;
            let Some(&pos) = p.positions.get(i) else { continue };
            let n = if has_n {
                p.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0])
            } else {
                [0.0, 0.0, 1.0]
            };
            let gn = remap(place_dir(&p.transform, n, apply));
            let gp = remap(place_point(&p.transform, pos, apply)) - off + gn * push;
            verts.push(Vertex {
                position: [gp.x, gp.y, gp.z],
                normal: [gn.x, gn.y, gn.z],
                tex_coords: p.uvs.get(i).copied().unwrap_or([0.0, 0.0]),
                ..Default::default()
            });
        }
    }
    if verts.is_empty() {
        None
    } else {
        Some(Mesh::from_vertices(device, &verts, label.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{det3, should_place};
    use gizmo_nfs::Mat4;

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
