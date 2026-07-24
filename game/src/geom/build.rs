//! Building GPU meshes: expanding the parser's indexed parts into the renderer's flat vertex
//! list, and the one world-placement helper every binary needs.

use super::frame::remap;
use gizmo_nfs::placement::{part_centroid, place_dir, place_point, should_place};
use gizmo::prelude::*;
use gizmo::renderer::gpu_types::Vertex;
use gizmo_nfs::NfsMeshPart;

/// Attach a `Transform` plus its matching `GlobalTransform` to an entity — the pair the
/// renderer needs to place anything in the world.
pub fn add_transform(world: &mut World, entity: gizmo::core::Entity, t: Transform) {
    world.add_component(entity, t);
    world.add_component(entity, GlobalTransform { matrix: t.local_matrix });
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
