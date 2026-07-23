//! Turning parsed CPU parts into Gizmo GPU meshes and placing them in the world.
//!
//! This is the engine-coupled counterpart to [`crate::part_groups`]: it depends on `gizmo`
//! and `wgpu`, expands the parser's *indexed* geometry into the flat vertex list the
//! renderer wants, and applies the NFSU2 → Gizmo coordinate remap.

use gizmo::prelude::*;
use gizmo::renderer::gpu_types::Vertex;
use gizmo_nfs::NfsMeshPart;

/// NFSU2 (Z-up, X = length, Y = width) → Gizmo drive frame: length → −Z (forward),
/// height → +Y. All mesh building and bounds go through this so the whole car shares one
/// consistent frame.
#[inline]
#[must_use]
pub fn remap(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
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
            let g = remap(*v);
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
    let mut verts = Vec::new();
    for p in parts {
        let has_n = !p.normals.is_empty();
        for &idx in &p.indices {
            let i = idx as usize;
            let Some(&pos) = p.positions.get(i) else { continue };
            let n = if has_n {
                p.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0])
            } else {
                [0.0, 0.0, 1.0]
            };
            let gp = remap(pos) - off;
            let gn = remap(n);
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
