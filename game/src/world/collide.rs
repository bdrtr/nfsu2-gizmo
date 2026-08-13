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

/// Which cells of the city have ground in them — the map's edge, at the resolution the data
/// actually supports.
///
/// NFSU2 keeps the player in with barriers, and this install ships **none**: a chunk census over
/// `TRACKS/L4R*.BUN`, `GLOBAL/InGame*.bun` and every ROUTES file finds no `0x0003410B` anywhere
/// (`ROADMAP.md` §M4). They have to be derived from the route network, which is not read yet, so
/// until then the only thing that says "the world stops here" is the geometry. A cell with no
/// drivable triangle has no road, no pavement and no terrain in it — 256 m of nothing.
///
/// Deliberately no finer than a cell. Whether the car is over a surface *right now* is a different
/// and harder question, and [`crate::rig::CarRig::keep_in_world`] already answers it by watching
/// the wheels; this one only has to tell the inside of Bayview from the outside of it, and a
/// per-triangle test would call every kerb-side gap a map edge.
pub struct Bounds {
    cells: std::collections::HashSet<(i32, i32)>,
}

impl Bounds {
    /// Take the cells that have something to drive on. Built from the same colliders physics gets,
    /// so the boundary is the world the car can actually stand on rather than a second opinion.
    #[must_use]
    pub fn of(colliders: &[CityCollider]) -> Self {
        Self { cells: colliders.iter().filter(|c| c.drivable() > 0).map(|c| c.cell).collect() }
    }

    /// Whether `p` is over a cell with ground in it.
    #[must_use]
    pub fn contains(&self, p: Vec3) -> bool {
        self.cells.contains(&cell_of(p))
    }

    /// How many cells the city covers.
    #[must_use]
    pub fn cells(&self) -> usize {
        self.cells.len()
    }
}

/// How wide a [`Ground`] cell is, in metres.
///
/// Not [`super::CELL_SIZE`]. A 256 m cell is the right unit for *drawing* — it is what makes a
/// merged mesh's bounding box small enough to cull and big enough to be one draw — and much too
/// coarse for a height query, where every triangle in the cell is tested one by one. 64 m is
/// OpenUG's `GCELL`, arrived at independently for the same query, and on this city it puts about
/// 90 drivable triangles in a cell against roughly 1,600 at 256 m.
pub const GROUND_CELL: f32 = 64.0;

/// How high the drivable surface is at a given XZ — the question the city could never answer.
///
/// It is asked twice already and guessed both times. `Placement::clearance` exists only because
/// "the city's surface height at a given XZ is not known without querying it", so a car is dropped
/// from a little way up and the suspension settles it; and picking a spawn point at all has meant
/// flying there, pressing **F**, and writing the number down. Neither is a property of the city —
/// they are both this missing query.
///
/// Built from the same colliders physics gets, and only from the triangles [`Surface::Drivable`]
/// admits, so it answers about the surface a car can stand on rather than the first thing a ray
/// happens to hit — a building's wall is not ground.
///
/// **This is not a replacement for the vehicle's own raycast.** The suspension asks the engine,
/// against the real colliders, and that is what makes the car drive; this answers a cheaper
/// question for the code that has to place things *before* there is a car.
pub struct Ground {
    /// Drivable triangles in world space, ordered so that a cell's are contiguous.
    tris: Vec<[Vec3; 3]>,
    /// Each cell's half-open run in [`Self::tris`] — the CSR layout, so a cell costs no allocation.
    runs: std::collections::HashMap<(i32, i32), (u32, u32)>,
}

impl Ground {
    /// Index every drivable triangle by the cells its XZ footprint touches.
    ///
    /// By footprint, not by centroid: a triangle that straddles a cell edge is ground on both
    /// sides of it, and indexing it once by its centre would leave a seam of unanswerable queries
    /// along every boundary.
    #[must_use]
    pub fn of(colliders: &[CityCollider]) -> Self {
        let key = |v: f32| (v / GROUND_CELL).floor() as i32;
        let mut by_cell: std::collections::HashMap<(i32, i32), Vec<[Vec3; 3]>> =
            std::collections::HashMap::new();

        for c in colliders {
            for (t, tri) in c.indices.chunks_exact(3).enumerate() {
                if c.surfaces.get(t) != Some(&Surface::Drivable) {
                    continue;
                }
                let Some(p) = tri
                    .iter()
                    .map(|&i| c.vertices.get(i as usize).map(|v| *v + c.origin))
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                let w = [p[0], p[1], p[2]];
                let (lo_x, hi_x) = (w.iter().fold(f32::MAX, |a, v| a.min(v.x)), w.iter().fold(f32::MIN, |a, v| a.max(v.x)));
                let (lo_z, hi_z) = (w.iter().fold(f32::MAX, |a, v| a.min(v.z)), w.iter().fold(f32::MIN, |a, v| a.max(v.z)));
                for cx in key(lo_x)..=key(hi_x) {
                    for cz in key(lo_z)..=key(hi_z) {
                        by_cell.entry((cx, cz)).or_default().push(w);
                    }
                }
            }
        }

        let mut tris = Vec::with_capacity(by_cell.values().map(Vec::len).sum());
        let mut runs = std::collections::HashMap::with_capacity(by_cell.len());
        for (cell, mut group) in by_cell {
            let from = u32::try_from(tris.len()).unwrap_or(u32::MAX);
            tris.append(&mut group);
            let to = u32::try_from(tris.len()).unwrap_or(u32::MAX);
            runs.insert(cell, (from, to));
        }
        Self { tris, runs }
    }

    /// The highest drivable surface at `at`'s XZ that is **not above** `at.y`, or `None` where the
    /// city has no ground under that point.
    ///
    /// At or below, rather than nearest: a point under a bridge wants the road it is standing on,
    /// not the deck over its head, and the caller always knows roughly where it is looking from.
    #[must_use]
    pub fn height_at(&self, at: Vec3) -> Option<f32> {
        let key = |v: f32| (v / GROUND_CELL).floor() as i32;
        let &(from, to) = self.runs.get(&(key(at.x), key(at.z)))?;
        let mut best: Option<f32> = None;
        for t in self.tris.get(from as usize..to as usize)? {
            let Some(y) = surface_y(t, at.x, at.z) else { continue };
            // A small tolerance, because the caller's own `y` is usually a hand-written round
            // number sitting a few centimetres inside the tarmac it is naming.
            if y <= at.y + 0.5 && best.is_none_or(|b| y > b) {
                best = Some(y);
            }
        }
        best
    }

    /// Every drivable surface at an XZ, lowest first.
    ///
    /// [`Self::height_at`] answers with one of these — the highest not above the asker — which is
    /// the right question for *placing* something. Following a line across the city is a different
    /// question, and neither "highest" nor "lowest" answers it: [`Surface::Drivable`] classifies a
    /// triangle by its normal, so a **flat roof is admitted exactly as a road is**, and a car park
    /// under a building is too. What a follower wants is the surface nearest where it already was,
    /// and it can only ask that if it can see the candidates.
    ///
    /// Measured need: seeding a route path from the topmost surface put six of `Paths4001`'s 40
    /// paths on rooftops at y 115–133 with the road at 22–28 beneath them.
    #[must_use]
    pub fn heights_at(&self, x: f32, z: f32) -> Vec<f32> {
        let key = |v: f32| (v / GROUND_CELL).floor() as i32;
        let Some(&(from, to)) = self.runs.get(&(key(x), key(z))) else { return Vec::new() };
        let mut out: Vec<f32> = self
            .tris
            .get(from as usize..to as usize)
            .unwrap_or_default()
            .iter()
            .filter_map(|t| surface_y(t, x, z))
            .collect();
        out.sort_by(f32::total_cmp);
        // Two triangles of one quad meet along a diagonal, so a query on that line answers twice
        // with the same height. Keeping both would let a caller mistake tessellation for a stack of
        // surfaces.
        out.dedup_by(|a, b| (*a - *b).abs() < 0.05);
        out
    }

    /// Walk a straight line over the city and say where the ground stops holding it up.
    ///
    /// Returns the distance along `a → b`, in the ground plane, of the first sample with no
    /// drivable surface within `tolerance` of the interpolated height — or `None` where the surface
    /// holds the whole way. Endpoints are not sampled: the caller already knows about them, and a
    /// node standing a hair off its own tarmac would otherwise fail its own link.
    ///
    /// **Height matters, which is why this is not `heights_at` in a loop.** `Surface::Drivable`
    /// admits a flat roof exactly as it admits a road, so "is there any surface at this XZ" says yes
    /// over a car park with a building on it. Asking for one near where the line already is turns
    /// that into the question a car has: does *this* road go on.
    ///
    /// Two callers, and they want the same thing from opposite ends. [`Network::drop_walled`](
    /// crate::world::Network) asks it of a link the file claims, to find the wall in between; the
    /// sim asks it of a car's own heading at the moment the world let go, to tell a car that drove
    /// off an edge from one that went through a triangle. A gap is a gap either way.
    #[must_use]
    pub fn gap_along(&self, a: Vec3, b: Vec3, tolerance: f32, step: f32) -> Option<f32> {
        let run = Vec3::new(b.x - a.x, 0.0, b.z - a.z).length();
        let n = (run / step).ceil().max(1.0) as usize;
        (1..n).find_map(|k| {
            let t = k as f32 / n as f32;
            let p = a.lerp(b, t);
            let held = self.heights_at(p.x, p.z).iter().any(|h| (h - p.y).abs() <= tolerance);
            (!held).then_some(t * run)
        })
    }

    /// Cells with drivable ground in them.
    #[must_use]
    pub fn cells(&self) -> usize {
        self.runs.len()
    }

    /// Triangle references held, counting a straddling triangle once per cell it touches.
    #[must_use]
    pub fn refs(&self) -> usize {
        self.tris.len()
    }
}

/// Where a vertical line through `(x, z)` meets a triangle's plane, or `None` if it misses.
///
/// Barycentric in XZ, so a triangle standing exactly on its edge (zero area from above) is a miss
/// rather than a division by zero.
fn surface_y(t: &[Vec3; 3], x: f32, z: f32) -> Option<f32> {
    let (a, b, c) = (t[0], t[1], t[2]);
    let det = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
    if det.abs() < 1e-9 {
        return None;
    }
    let l1 = ((b.z - c.z) * (x - c.x) + (c.x - b.x) * (z - c.z)) / det;
    let l2 = ((c.z - a.z) * (x - c.x) + (a.x - c.x) * (z - c.z)) / det;
    let l3 = 1.0 - l1 - l2;
    let inside = |v: f32| (-1e-4..=1.0 + 1e-4).contains(&v);
    (inside(l1) && inside(l2) && inside(l3)).then_some(l1 * a.y + l2 * b.y + l3 * c.y)
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

    /// The boundary is the cells with something to drive on — and a cell whose only geometry is
    /// vertical is not one of them, which is what keeps a building face from counting as ground.
    #[test]
    fn bounds_take_the_cells_with_drivable_ground() {
        let road = mesh(vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]], vec![0, 1, 2]);
        let far_wall = mesh(
            vec![[2000.0, 0.0, 0.0], [2010.0, 0.0, 0.0], [2000.0, 0.0, 10.0]],
            vec![0, 1, 2],
        );
        let cells = collision_cells(&[road, far_wall]);
        assert_eq!(cells.len(), 2, "one cell each");

        let bounds = Bounds::of(&cells);
        assert_eq!(bounds.cells(), 1, "only the cell with a flat triangle is inside");

        let road_cell = cells.iter().find(|c| c.drivable() > 0).expect("the road cell");
        let wall_cell = cells.iter().find(|c| c.drivable() == 0).expect("the wall cell");
        assert!(bounds.contains(road_cell.origin), "the road's cell is in bounds");
        assert!(!bounds.contains(wall_cell.origin), "a wall-only cell is not ground");
        // And well outside either of them there is nothing at all.
        assert!(!bounds.contains(Vec3::new(50_000.0, 0.0, 50_000.0)));
    }

    /// The height query answers about drivable ground, picks the surface *below* the asker, and
    /// says nothing where the city has none.
    #[test]
    fn ground_answers_with_the_surface_under_the_asker() {
        // Two flat quads at different heights over the same XZ — a road and the deck above it —
        // plus a vertical face, which is geometry but not ground.
        let flat = |z_off: f32| {
            mesh(
                vec![
                    [0.0, 0.0, z_off],
                    [40.0, 0.0, z_off],
                    [0.0, 40.0, z_off],
                    [40.0, 40.0, z_off],
                ],
                vec![0, 1, 2, 1, 3, 2],
            )
        };
        // The file frame is Z-up, so a constant Z is a constant height in the Gizmo frame.
        let cells = collision_cells(&[flat(0.0), flat(9.0)]);
        let g = Ground::of(&cells);
        assert!(g.cells() > 0, "the quads land in at least one cell");

        // Standing on the road, the deck overhead is not the answer.
        let on_road = Vec3::new(-20.0, 1.0, -20.0);
        let y = g.height_at(on_road).expect("ground under the road point");
        assert!((y - 0.0).abs() < 1e-3, "expected the lower deck at y=0, got {y}");

        // Above both, the higher one wins.
        let above = Vec3::new(-20.0, 50.0, -20.0);
        let y = g.height_at(above).expect("ground under a point above both");
        assert!((y - 9.0).abs() < 1e-3, "expected the upper deck at y=9, got {y}");

        // Off the quads entirely, there is no ground and it says so.
        assert!(g.height_at(Vec3::new(5_000.0, 10.0, 5_000.0)).is_none());
    }

    /// A wall is geometry and not ground, so it never answers a height query.
    #[test]
    fn a_vertical_face_is_not_ground() {
        let wall = mesh(
            vec![[0.0, 0.0, 0.0], [30.0, 0.0, 0.0], [0.0, 0.0, 30.0], [30.0, 0.0, 30.0]],
            vec![0, 1, 2, 1, 3, 2],
        );
        let cells = collision_cells(&[wall]);
        assert_eq!(cells[0].drivable(), 0, "the face is vertical");
        let g = Ground::of(&cells);
        assert_eq!(g.refs(), 0, "no drivable triangle, so nothing to answer with");
        assert!(g.height_at(Vec3::new(-15.0, 10.0, 0.0)).is_none());
    }

    /// Walking a line: ground that holds says nothing, ground that ends says where — and a line
    /// above the surface is a gap, which is the whole difference from `heights_at` in a loop.
    #[test]
    fn a_gap_is_found_where_the_surface_stops_holding_the_line() {
        // One flat quad. `remap` is `(-y, z, -x)`, so file x,y ∈ 0..40 is Gizmo x,z ∈ -40..0 at
        // height 0 — the road ends at Gizmo x = 0.
        let road = mesh(
            vec![[0.0, 0.0, 0.0], [40.0, 0.0, 0.0], [0.0, 40.0, 0.0], [40.0, 40.0, 0.0]],
            vec![0, 1, 2, 1, 3, 2],
        );
        let g = Ground::of(&collision_cells(&[road]));

        // Wholly on the quad: nothing to report.
        assert_eq!(
            g.gap_along(Vec3::new(-30.0, 0.0, -20.0), Vec3::new(-10.0, 0.0, -20.0), 4.0, 3.0),
            None,
            "the road holds the whole way"
        );

        // Off the edge 30 m along. The answer is the first sample *past* it, so within one step.
        let d = g
            .gap_along(Vec3::new(-30.0, 0.0, -20.0), Vec3::new(30.0, 0.0, -20.0), 4.0, 3.0)
            .expect("the road ends and the line goes on");
        assert!((30.0..=33.5).contains(&d), "expected the edge at ~30 m, got {d}");

        // The same XZ, a hundred metres up. Every sample has a surface under it and none of them is
        // near the line, which is a car in the air over its own road rather than on it.
        assert!(
            g.gap_along(Vec3::new(-30.0, 100.0, -20.0), Vec3::new(-10.0, 100.0, -20.0), 4.0, 3.0)
                .is_some(),
            "a surface far below the line does not hold it up"
        );
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
