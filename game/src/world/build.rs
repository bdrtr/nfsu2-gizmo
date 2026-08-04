//! Turning a region's objects into cell-sized, per-texture GPU meshes.

use super::cell::{cell_centre, cell_of};
use super::{dedup, world_point};
use gizmo::prelude::*;
use gizmo::renderer::gpu_types::Vertex;
use gizmo::wgpu;
use gizmo_nfs::types::AssetHash;
use gizmo_nfs::world::{TrackPack, WorldMesh};
use std::collections::BTreeMap;

/// What a merged mesh is keyed by: the cell it sits in and the texture its triangles wear.
/// `None` is the runs whose slot resolved in no pack of this region.
type BucketKey = ((i32, i32), Option<AssetHash>);

/// One merged draw: every triangle in a cell that wears the same texture.
pub struct CityMesh {
    /// The cell it belongs to.
    pub cell: (i32, i32),
    /// The texture every triangle in it uses, or `None` for the runs whose slot resolved nowhere.
    pub texture: Option<AssetHash>,
    /// Vertices already relative to [`Self::origin`].
    pub mesh: Mesh,
    /// Where to spawn it. Vertices are recentred on this so the mesh's own bounds are the cell's
    /// and not the city's — which is the whole reason cells exist.
    pub origin: Vec3,
}

/// A region, ready to spawn.
pub struct CityVisuals {
    /// The merged meshes, one per (cell, texture).
    pub meshes: Vec<CityMesh>,
    /// Objects the region declared, after duplicates were dropped.
    pub objects: usize,
    /// Objects dropped as exact duplicates.
    pub duplicates: usize,
    /// Index runs whose texture slot resolved in no pack of this region.
    pub unresolved_runs: usize,
    /// The distinct keys behind those runs, so a caller can try the shared tiers for them rather
    /// than guessing at which ones went missing.
    pub unresolved_keys: Vec<AssetHash>,
}

/// Which texture a run uses: its group names a slot by position, and the slot names a key.
///
/// `shared` is consulted after the region's own packs — see [`super::SharedTextures`] for why the
/// order is here and not in the parser.
///
/// Returning `None` for a key no pack in this region carries is deliberate rather than a fallback.
/// 1,289 of the city's 70,439 slots are in that state — most live in `TRACKS/LOC4DYNTEX.BIN` or
/// under `GLOBAL/`, and 111 exist nowhere in the install — and quietly substituting some other
/// texture would make a missing tier look like a working one.
fn texture_for(
    key: AssetHash,
    packs: &[TrackPack<'_>],
    shared: Option<&super::SharedTextures>,
) -> Option<AssetHash> {
    if packs.iter().any(|p| p.get(key).is_some()) {
        return Some(key);
    }
    shared.and_then(|s| s.get(key)).map(|_| key)
}

/// Build one region's meshes.
///
/// `budget` caps how many objects are taken, nearest-first from `around` — the throwaway that
/// makes a 10,735-object region usable before streaming exists. `None` takes everything.
pub fn build_region(
    device: &wgpu::Device,
    meshes: Vec<WorldMesh>,
    packs: &[TrackPack<'_>],
    shared: Option<&super::SharedTextures>,
    around: Vec3,
    budget: Option<usize>,
) -> CityVisuals {
    let declared = meshes.len();
    let mut objects = dedup(meshes);
    let duplicates = declared - objects.len();

    if let Some(n) = budget {
        if objects.len() > n {
            // Nearest-first by the object's own centre, so what survives is a contiguous
            // neighbourhood rather than whatever the file happened to list first.
            objects.sort_by(|a, b| {
                let d = |m: &WorldMesh| {
                    let c = world_point(&m.header, m.header.bbox_min)
                        .midpoint(world_point(&m.header, m.header.bbox_max));
                    (c - around).length_squared()
                };
                d(a).total_cmp(&d(b))
            });
            objects.truncate(n);
        }
    }

    // (cell, texture) → vertices. `BTreeMap` rather than a hash map so a run over the same region
    // twice produces the same meshes in the same order — a golden screenshot needs that.
    let mut buckets: BTreeMap<BucketKey, Vec<Vertex>> = BTreeMap::new();
    let mut unresolved_runs = 0usize;
    let mut unresolved_keys: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();

    for object in &objects {
        if object.positions.is_empty() {
            continue;
        }
        // The whole object lands in one cell, chosen by its centre. Splitting a solid across cells
        // would cut road surfaces at cell edges for no visibility gained; a solid is already the
        // granularity the game authored.
        let centre = world_point(&object.header, object.header.bbox_min)
            .midpoint(world_point(&object.header, object.header.bbox_max));
        let cell = cell_of(centre);
        let origin = cell_centre(cell);

        // A run with no group table still draws — it is the whole index buffer with slot 0.
        let runs: Vec<(usize, usize, Option<AssetHash>)> = if object.groups.is_empty() {
            let key = object.texture_slots.first().copied();
            vec![(0, object.indices.len(), key.and_then(|k| texture_for(k, packs, shared)))]
        } else {
            object
                .groups
                .iter()
                .map(|g| {
                    let tex = texture_for(g.hash, packs, shared);
                    if tex.is_none() {
                        unresolved_runs += 1;
                        unresolved_keys.insert(g.hash.0);
                    }
                    (g.index_offset, g.index_count, tex)
                })
                .collect()
        };

        for (offset, count, texture) in runs {
            let Some(slice) = object.indices.get(offset..offset + count) else { continue };
            let verts = buckets.entry((cell, texture)).or_default();
            for &idx in slice {
                let i = idx as usize;
                let Some(&p) = object.positions.get(i) else { continue };
                let gp = world_point(&object.header, p) - origin;
                let n = object.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                let gn = super::remap(n);
                // The vertex colour **is** the city's lighting. A static world stores it here
                // rather than recomputing it, which is why the scenery needs no light to look
                // right — and why throwing it away (the `Vertex` default is white) leaves the city
                // to be lit by whatever the scene happens to have, which is not what it was drawn
                // for. `unlit.wgsl` multiplies it in; the PBR path discards it.
                let c = object.colours.get(i).copied().unwrap_or([255, 255, 255, 255]);
                let srgb = |b: u8| {
                    let v = f32::from(b) / 255.0;
                    // The bytes are sRGB and the shader works in linear, the same conversion the
                    // texture upload does for its own pixels.
                    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
                };
                verts.push(Vertex {
                    position: [gp.x, gp.y, gp.z],
                    color: [srgb(c[0]), srgb(c[1]), srgb(c[2])],
                    normal: [gn.x, gn.y, gn.z],
                    tex_coords: object.uvs.get(i).copied().unwrap_or([0.0, 0.0]),
                    ..Default::default()
                });
            }
        }
    }

    let meshes = buckets
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|((cell, texture), verts)| {
            let label = match texture {
                Some(t) => format!("city_{}_{}_{:08X}", cell.0, cell.1, t.0),
                None => format!("city_{}_{}_untextured", cell.0, cell.1),
            };
            CityMesh {
                cell,
                texture,
                mesh: Mesh::from_vertices(device, &verts, label),
                origin: cell_centre(cell),
            }
        })
        .collect();

    CityVisuals {
        meshes,
        objects: objects.len(),
        duplicates,
        unresolved_runs,
        unresolved_keys: unresolved_keys.into_iter().map(AssetHash).collect(),
    }
}
