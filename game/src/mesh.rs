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

/// Place a part's local-space vertex into NFSU2 car space by its file transform.
///
/// The matrix is row-major with the translation in the last **row** (row-vector convention,
/// `v' = v · M`). We apply it only for a *proper* transform (`det > 0`): parts modelled around
/// their own origin (the rear wing, brake discs) carry a real translation there and must be
/// placed. A **reflection** (`det < 0`) is how NFSU2 marks a mirrored right-side part whose
/// vertices are *already* baked at their mirrored world position — applying it would flip the
/// part back across the centreline, so those are left as-is.
#[inline]
fn place_point(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    if det3(m) <= 1e-6 {
        return p;
    }
    [
        p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0] + m[3][0],
        p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1] + m[3][1],
        p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2] + m[3][2],
    ]
}

/// Rotate a part's local-space normal by its file transform's 3x3 (no translation), gated the
/// same way as [`place_point`]: proper transforms only, reflections left as baked.
#[inline]
fn place_dir(m: &Mat4, n: [f32; 3]) -> [f32; 3] {
    if det3(m) <= 1e-6 {
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
        for v in &p.positions {
            let g = remap(place_point(&p.transform, *v));
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
        for &idx in *indices {
            let i = idx as usize;
            let Some(&pos) = p.positions.get(i) else { continue };
            let n = if has_n {
                p.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0])
            } else {
                [0.0, 0.0, 1.0]
            };
            let gn = remap(place_dir(&p.transform, n));
            let gp = remap(place_point(&p.transform, pos)) - off + gn * push;
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
