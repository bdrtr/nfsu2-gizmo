//! The city as something to drive on.
//!
//! One collider for the whole city would have the whole city's bounding box, so the broadphase
//! would pair it with every dynamic body every step; one per solid would be 14,000 bodies. A cell
//! is the unit that is neither — the same 256 m cell the meshes merge into, so a car standing in a
//! cell tests against the triangles that are actually near it.
//!
//! Two things are deliberately **not** done here.
//!
//! **The triangles are not welded.** Two road quads meeting at a shared edge stay two triangles
//! with their own vertices. A per-triangle narrowphase treats them as separate surfaces either way,
//! and welding would only hide the internal-edge problem rather than fix it — a box crossing that
//! seam still gets a closest-feature normal from each triangle.
//!
//! **Nothing is classified.** Which triangles are road and which are wall is a *policy* question
//! with a measured answer — a guardrail is not a separate mesh, it is near-vertical triangles baked
//! into the road chunk — but the threshold that separates them is a judgement call, so it lives in
//! [`surface_of`] where it can be read, rather than being folded into the geometry.

use super::cell::{cell_centre, cell_of};
use super::world_point;
use gizmo::prelude::*;
use gizmo_nfs::world::WorldMesh;
use std::collections::BTreeMap;

/// How steep a triangle has to be before it counts as something to hit rather than to drive on.
///
/// Measured rather than chosen: drivable tarmac sits at `|n.y|` ≈ 0.98, and the guardrail
/// triangles baked into the same chunks come in under 0.30. Anything between is a kerb or a ramp,
/// and calling those drivable is the friendlier error.
pub const WALL_NORMAL_Y: f32 = 0.30;

/// What a triangle is, as far as a car is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Flat enough to drive on.
    Drivable,
    /// Steep enough to stop a car: guardrails, kerbs, building faces.
    Wall,
}

/// Classify one triangle by its own normal.
#[must_use]
pub fn surface_of(a: Vec3, b: Vec3, c: Vec3) -> Surface {
    let n = (b - a).cross(c - a);
    let len = n.length();
    if len < 1e-9 {
        // A degenerate triangle has no normal to judge it by. Calling it a wall would put an
        // invisible barrier in the road; calling it drivable puts a sliver of floor nobody stands
        // on.
        return Surface::Drivable;
    }
    if (n.y / len).abs() >= WALL_NORMAL_Y {
        Surface::Drivable
    } else {
        Surface::Wall
    }
}

/// A cell's geometry while it is being accumulated: vertices, indices, per-triangle surface.
#[derive(Default)]
struct Accum {
    vertices: Vec<Vec3>,
    indices: Vec<u32>,
    surfaces: Vec<Surface>,
}

/// One cell's collision geometry, in the cell's own frame.
pub struct CityCollider {
    pub cell: (i32, i32),
    /// Where the cell sits. Vertices are relative to this, for the same reason the visual meshes
    /// are: a collider whose vertices are city-scale has a city-scale bounding box.
    pub origin: Vec3,
    /// Vertices, already relative to [`Self::origin`].
    pub vertices: Vec<Vec3>,
    /// Triangle-list indices into [`Self::vertices`].
    pub indices: Vec<u32>,
    /// Per-triangle surface, parallel to `indices.len() / 3`.
    pub surfaces: Vec<Surface>,
}

impl CityCollider {
    /// Triangles in this cell.
    #[must_use]
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    /// How many of them a car can drive on.
    #[must_use]
    pub fn drivable(&self) -> usize {
        self.surfaces.iter().filter(|s| **s == Surface::Drivable).count()
    }
}

/// Bucket the city's triangles into per-cell collision meshes.
///
/// Takes the same objects the visuals are built from, so what you hit is what you see. Objects with
/// no geometry are skipped; so is anything the caller has already filtered out (backdrop, LOD).
#[must_use]
pub fn collision_cells(meshes: &[WorldMesh]) -> Vec<CityCollider> {
    let mut cells: BTreeMap<(i32, i32), Accum> = BTreeMap::new();

    for object in meshes {
        if object.positions.is_empty() || object.indices.len() < 3 {
            continue;
        }
        let lo = world_point(&object.header, object.header.bbox_min);
        let hi = world_point(&object.header, object.header.bbox_max);
        if !lo.is_finite() || !hi.is_finite() {
            continue;
        }
        let cell = cell_of((lo + hi) * 0.5);
        let origin = cell_centre(cell);
        let entry = cells.entry(cell).or_default();

        for tri in object.indices.chunks_exact(3) {
            let Some(p) = tri
                .iter()
                .map(|&i| object.positions.get(i as usize).copied())
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let w: Vec<Vec3> = p.iter().map(|&q| world_point(&object.header, q) - origin).collect();
            entry.surfaces.push(surface_of(w[0], w[1], w[2]));
            let base = entry.vertices.len() as u32;
            entry.vertices.extend_from_slice(&w);
            entry.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }

    cells
        .into_iter()
        .filter(|(_, a)| !a.vertices.is_empty())
        .map(|(cell, a)| CityCollider {
            cell,
            origin: cell_centre(cell),
            vertices: a.vertices,
            indices: a.indices,
            surfaces: a.surfaces,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizmo_nfs::types::{AssetHash, IDENTITY};
    use gizmo_nfs::world::WorldSolidHeader;

    fn mesh(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> WorldMesh {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &positions {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        WorldMesh {
            header: WorldSolidHeader {
                hash: AssetHash(1),
                name: "TRN_ROADA".into(),
                bbox_min: lo,
                bbox_max: hi,
                matrix: IDENTITY,
            },
            positions,
            normals: Vec::new(),
            colours: Vec::new(),
            uvs: Vec::new(),
            indices,
            groups: Vec::new(),
            texture_slots: Vec::new(),
        }
    }

    /// The file's frame is Z-up, so a road lies in its X/Y plane and comes out flat in Gizmo's.
    #[test]
    fn a_road_is_drivable_and_a_wall_is_not() {
        // Flat in the file frame (constant Z) → flat in the Gizmo frame (constant Y).
        let road = mesh(vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]], vec![0, 1, 2]);
        let cells = collision_cells(&[road]);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].triangles(), 1);
        assert_eq!(cells[0].drivable(), 1, "a flat road is drivable");

        // Vertical in the file frame (constant Y, varying Z = height).
        let wall = mesh(vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 0.0, 10.0]], vec![0, 1, 2]);
        let cells = collision_cells(&[wall]);
        assert_eq!(cells[0].drivable(), 0, "a vertical face is not drivable");
    }

    /// The whole point of cells: two objects a kilometre apart must not share a collider, or its
    /// bounding box spans the gap and the broadphase pairs it with everything in between.
    #[test]
    fn distant_objects_land_in_different_cells() {
        let near = mesh(vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]], vec![0, 1, 2]);
        let far = mesh(
            vec![[2000.0, 0.0, 0.0], [2010.0, 0.0, 0.0], [2000.0, 10.0, 0.0]],
            vec![0, 1, 2],
        );
        let cells = collision_cells(&[near, far]);
        assert_eq!(cells.len(), 2, "one cell each");
        assert_ne!(cells[0].cell, cells[1].cell);
    }

    /// Vertices are relative to the cell, not to the world — a collider carrying city-scale
    /// coordinates has a city-scale AABB whatever cell it belongs to.
    #[test]
    fn vertices_are_relative_to_their_cell() {
        let far = mesh(
            vec![[2000.0, 0.0, 0.0], [2010.0, 0.0, 0.0], [2000.0, 10.0, 0.0]],
            vec![0, 1, 2],
        );
        let cells = collision_cells(&[far]);
        let c = &cells[0];
        assert_ne!(c.origin, Vec3::ZERO, "the cell is not at the origin");
        for v in &c.vertices {
            assert!(v.length() < super::super::CELL_SIZE, "{v:?} is not local to its cell");
        }
    }

    /// Indices address this cell's own vertex list, not the object's — several objects merge into
    /// one collider and an unrebased index would read another object's geometry.
    #[test]
    fn indices_are_rebased_when_objects_merge() {
        let a = mesh(vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]], vec![0, 1, 2]);
        let b = mesh(vec![[1.0, 1.0, 0.0], [11.0, 1.0, 0.0], [1.0, 11.0, 0.0]], vec![0, 1, 2]);
        let cells = collision_cells(&[a, b]);
        assert_eq!(cells.len(), 1, "both are in the same cell");
        let c = &cells[0];
        assert_eq!(c.triangles(), 2);
        assert_eq!(c.vertices.len(), 6);
        assert!(c.indices.iter().all(|&i| (i as usize) < c.vertices.len()));
        assert_eq!(&c.indices, &[0, 1, 2, 3, 4, 5], "the second object's indices are rebased");
    }

    /// An object with no geometry, or an index past its own buffer, is skipped rather than
    /// producing a collider that reads whatever was next in memory.
    #[test]
    fn broken_objects_are_skipped() {
        assert!(collision_cells(&[mesh(Vec::new(), Vec::new())]).is_empty());
        let bad = mesh(vec![[0.0, 0.0, 0.0]], vec![0, 1, 2]);
        assert!(collision_cells(&[bad]).is_empty(), "an index past the end drops its triangle");
    }
}
