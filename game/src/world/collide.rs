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
        Self::filtered(colliders, Some(Surface::Drivable))
    }

    /// The same index built from **every** triangle, walls included.
    ///
    /// Only a diagnostic: it answers "is there anything here at all", which is the question that
    /// separates a city with no data at a place from a city whose surface there was classified as
    /// a wall. Never drive against it — a building's face would read as floor.
    #[must_use]
    pub fn of_everything(colliders: &[CityCollider]) -> Self {
        Self::filtered(colliders, None)
    }

    /// The same index built from the **walls** — guardrails, kerbs, building faces.
    ///
    /// Also a diagnostic, and it answers the question a hole in the ground raises: a place a car
    /// cannot stand is only a defect if the car can get there, and what keeps it out is a barrier.
    /// Asking where the walls are is not the same as asking whether one blocks a given step, which
    /// is [`Walls::across_hit`]'s job and involves heights, decks and the road's own kerb.
    #[must_use]
    pub fn of_walls(colliders: &[CityCollider]) -> Self {
        Self::filtered(colliders, Some(Surface::Wall))
    }

    fn filtered(colliders: &[CityCollider], keep: Option<Surface>) -> Self {
        let key = |v: f32| (v / GROUND_CELL).floor() as i32;
        let mut by_cell: std::collections::HashMap<(i32, i32), Vec<[Vec3; 3]>> =
            std::collections::HashMap::new();

        for c in colliders {
            for (t, tri) in c.indices.chunks_exact(3).enumerate() {
                if keep.is_some_and(|k| c.surfaces.get(t) != Some(&k)) {
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

    /// Which way is **off** the city, from a point standing on it — the barrier the files do not
    /// carry, derived from the ground itself.
    ///
    /// A chunk census finds no `0x0003410B` anywhere in this install, so the fence that keeps a car
    /// on Bayview's roads has to come from somewhere. The route network is one candidate and it was
    /// measured: a corridor at its own half-width would have caught eight of the ten cars that left
    /// the world, and **two were still on the course when the ground ran out** — on `Paths4041` the
    /// race line runs along the lip of the void. The ground knows about that lip and the route does
    /// not, so the ground is asked.
    ///
    /// Probes `reach` metres out in a ring of directions and returns the normalised sum of the ones
    /// with **no drivable surface near the asker's own height**, or `None` where the ground holds
    /// all the way round. The result points away from the city, so a caller that wants to stop a car
    /// leaving removes the velocity along it — which leaves the car free to slide along the edge
    /// rather than being nailed to it.
    ///
    /// **At the asker's height, which is why this is not a map.** Bayview stacks: a raster of "is
    /// there ground at this XZ" cannot tell the edge of a bridge deck from a road passing under it,
    /// and would fence the road while letting cars off the bridge. Every question here goes through
    /// [`Self::gap_along`], which follows a height.
    #[must_use]
    pub fn edge_at(&self, at: Vec3, reach: f32, tolerance: f32) -> Option<Vec3> {
        /// Directions probed. Twelve is a 30° resolution: fine enough that the normal off a straight
        /// edge is within half a step of square to it, coarse enough to be one ring of cheap queries.
        const RAYS: usize = 12;
        // A quarter of the reach, because [`Self::gap_along`] does not sample its endpoint: at half
        // the reach a ray tests one point, its own midpoint, and a lip at the far end of the probe
        // is missed entirely. Three samples along each ray is the cheapest spacing that reaches out.
        let step = reach * 0.25;
        let mut out = Vec3::ZERO;
        for k in 0..RAYS {
            let a = std::f32::consts::TAU * k as f32 / RAYS as f32;
            let dir = Vec3::new(a.cos(), 0.0, a.sin());
            if self.gap_along(at, at + dir * reach, tolerance, step).is_some() {
                out += dir;
            }
        }
        (out.length_squared() > 1e-6).then(|| out.normalize())
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

/// What [`Walls::across_hit`] found standing in the way, and enough of the walk to judge it.
///
/// [`Walls::across`] answers `bool`, and that answer could not be defended. The walk follows the
/// ground, so where the city stacks it can step onto a deck and count the road *beneath* as an
/// obstacle — and a finding was withdrawn over exactly that: 3-25 % of race-line edges read
/// "blocked" and the reading had to be given up, because nothing came back but the verdict. See
/// `ROADMAP.md`, 2026-08-14.
///
/// Deliberately raw. Nothing here is a judgement: no "wall or kerb", no "wrong deck". The triangle
/// comes back whole and the walk's own belief about the floor comes back beside it, because which
/// of those makes a hit real is a **policy** question with a different answer for the pilot than
/// for a diagnostic — the same reason [`Surface`] is decided in [`surface_of`] rather than folded
/// into the geometry (see this file's header).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// The triangle that stopped the segment, in world space, exactly as [`Walls`] stores it. Its
    /// height range says whether this is a kerb, a rail or a building face; its normal and area are
    /// one cross product away. [`Walls`] keeps no provenance, so its geometry is the whole of what
    /// is known about it — no mesh name, no material, and no answer to "which barrier is this".
    pub tri: [Vec3; 3],
    /// Where the segment met it, at the height the walk was carrying — so `at.y` is a car's
    /// bumper height above whichever floor that step was on, not a point on the ground.
    pub at: Vec3,
    /// Ground-plane distance from the walk's start to [`Self::at`]. **Not the distance to the
    /// nearest blocker:** the search stops at the first triangle in cell order, which is what keeps
    /// it cheap. A caller that wants to shorten a probe rather than discard a heading needs the
    /// nearest hit, and that is a different search with a different cost.
    pub along: f32,
    /// The ground the walk believed it was on at this step — the surface chosen as nearest the
    /// height the walk already had. This is the suspect quantity, reported rather than trusted.
    ///
    /// It is the floor at the step's **far** sample, and the hit can be anywhere along that step.
    /// On a step that changes deck the two ends stand on different floors, so this is the wrong one
    /// for a hit that happened next to [`Self::from_y`]'s end — which is why [`Self::over_floor`]
    /// is only meaningful once [`Self::climb`] says the step did not change floors.
    pub walk_y: f32,
    /// The same, one step earlier, so `walk_y - from_y` is the floor change made at the very step
    /// that produced the hit. At `step == 1` it is the caller's own `a.y`, which never went through
    /// [`Ground::heights_at`] and is not a floor at all.
    pub from_y: f32,
    /// The floor the walk settled on at the first sample that **had** one — what [`Self::drift`]
    /// is measured against, and deliberately not the caller's `a.y`, for the reason above. A walk
    /// that never found ground at all has nothing to offer here and falls back to `a.y`;
    /// [`Self::layers`] is what says which of the two happened.
    pub start_y: f32,
    /// The largest change of floor between two successive samples that **had** a floor, anywhere in
    /// the walk up to the hit. The walk changing decks, measured rather than assumed.
    ///
    /// The first sample is excluded because its predecessor is not a floor, and a stretch with no
    /// ground under it is spanned rather than counted: the interpolated height the walk carries
    /// there is not a surface, and treating it as one turns every hole into a drop and a climb.
    /// So a large `climb` over a gap means the floor really is that far from where it was, not that
    /// there was a hole in between.
    pub climb: f32,
    /// How many drivable surfaces [`Ground::heights_at`] offered under this step, after its own
    /// dedup. **Zero means there was no ground at all** and the interpolated height stood in; a
    /// link with a genuine hole in it is [`Ground::gap_along`]'s finding, not this one's.
    pub layers: usize,
    /// Which step hit.
    pub step: usize,
    /// How many steps the walk was divided into. `step == steps` means the far end is `b` **in
    /// plan**; its height is the floor found under `b`, which is what the walk uses and need not be
    /// `b.y`.
    pub steps: usize,
    /// Index into this [`Walls`]'s own triangle list. Stable for the lifetime of this [`Walls`] and
    /// meaningless outside it — enough to tell one triangle from another, and **not** enough to
    /// count blockers. A rail is two triangles per quad baked into the road chunk, so probes a few
    /// metres apart along one barrier come back with different indices. Barrier identity would mean
    /// clustering by plane and adjacency, and [`Walls`] keeps no provenance to do it with.
    pub index: u32,
    /// Whether **anything** in the index is still hit when this step is carried **level**, at the
    /// height its near end already had. `false` means nothing stands across the way at the height
    /// the step came in at, and the walk reached [`Self::tri`] only by changing floors.
    ///
    /// It asks the whole index rather than re-testing [`Self::tri`], and the difference is not
    /// academic: a barrier is many triangles and the walk returns the first one in cell order, so
    /// re-testing that one answers `false` for a wall whose *other* triangle stands squarely across
    /// the level step. A walk with no horizontal extent is level by construction and answers `true`.
    ///
    /// Not a verdict on its own, in either direction. A walk that changed decks three steps ago and
    /// has been level since answers `true` and is still on the wrong deck; a barrier standing on a
    /// road that genuinely falls away can be stepped over by the level test and answer `false`.
    /// Read it with [`Self::drift`] and [`Self::climb`].
    pub flat_too: bool,
}

impl Hit {
    /// The triangle's own height range, lowest first.
    #[must_use]
    pub fn span(&self) -> (f32, f32) {
        let (a, b, c) = (self.tri[0].y, self.tri[1].y, self.tri[2].y);
        (a.min(b).min(c), a.max(b).max(c))
    }

    /// That range relative to the floor the walk believed it was on: metres over the road.
    ///
    /// Calibration, in the terms this can actually return. Every triangle a walk meets straddles
    /// the height it was carrying, so at `lift = 0.5` a hit reads `lo <= 0.5 <= hi` **on a level
    /// step**: a building face is `(≈0.0, tall)`, a low barrier `(≈0.0, 0.6)`. A positive base
    /// belongs to a parapet or a deck's edge fascia — not to the soffit under a bridge, which is
    /// horizontal, which makes it [`Surface::Drivable`], which keeps it out of [`Walls`] entirely.
    /// A top below zero is geometry underneath the walk.
    ///
    /// On a step that changed floors this is measured against the wrong end (see [`Self::walk_y`]),
    /// and the number means nothing until [`Self::climb`] says it did not.
    #[must_use]
    pub fn over_floor(&self) -> (f32, f32) {
        let (lo, hi) = self.span();
        (lo - self.walk_y, hi - self.walk_y)
    }

    /// How far the walk drifted from the floor it started on, signed.
    #[must_use]
    pub fn drift(&self) -> f32 {
        self.walk_y - self.start_y
    }
}

/// The triangles a car cannot drive through, indexed to answer one question: **is there something
/// between these two points.**
///
/// [`Ground`] is the other half of the same data and answers "is there road here". That is what
/// `Network::drop_walled` asks of every link the file claims, and it is not the same question: a
/// central reservation, a guardrail between two carriageways, a retaining wall beside a slip road —
/// all of them have perfectly good road on both sides and along the line between, so the road test
/// passes and the link stays. The network then hands a driver a branch with a barrier across it.
///
/// **Height is the whole difficulty.** `Surface::Wall` is a normal test, so it catches a building
/// face and a fifteen-centimetre kerb alike, and four filters have already been swept away in this
/// project for being sharp enough to catch kerbs. The lift below is what separates them: the
/// question asked is not "does the line cross a wall triangle" but "does it cross one **standing at
/// least this high**", tested by intersecting the segment carried at that height.
///
/// **What the answer may be used for is settled, and it is not deletion.** Cutting the links this
/// finds was written, swept and refuted — see [`Network::drop_walled`](crate::world::Network) and
/// `ROADMAP.md`. Six of eight routes did not move at all, and the two that did cancelled, because
/// taking away a link a car cannot drive takes away the way round it as well. The answer is a
/// **cost** on a branch, not the removal of one.
pub struct Walls {
    tris: Vec<[Vec3; 3]>,
    runs: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

impl Walls {
    /// Index every wall triangle by the cells its XZ footprint touches.
    #[must_use]
    pub fn of(colliders: &[CityCollider]) -> Self {
        let key = |v: f32| (v / GROUND_CELL).floor() as i32;
        let mut tris: Vec<[Vec3; 3]> = Vec::new();
        let mut runs: std::collections::HashMap<(i32, i32), Vec<u32>> = Default::default();
        for c in colliders {
            for (t, tri) in c.indices.chunks_exact(3).enumerate() {
                if c.surfaces.get(t) != Some(&Surface::Wall) {
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
                let i = u32::try_from(tris.len()).unwrap_or(u32::MAX);
                tris.push(w);
                let (lo_x, hi_x) = (
                    w.iter().fold(f32::MAX, |a, v| a.min(v.x)),
                    w.iter().fold(f32::MIN, |a, v| a.max(v.x)),
                );
                let (lo_z, hi_z) = (
                    w.iter().fold(f32::MAX, |a, v| a.min(v.z)),
                    w.iter().fold(f32::MIN, |a, v| a.max(v.z)),
                );
                for cx in key(lo_x)..=key(hi_x) {
                    for cz in key(lo_z)..=key(hi_z) {
                        runs.entry((cx, cz)).or_default().push(i);
                    }
                }
            }
        }
        Self { tris, runs }
    }

    /// Whether anything stands across the way from `a` to `b`.
    ///
    /// The verdict alone, which is all the pilot ever wanted from it. [`Self::across_hit`] is the
    /// same walk and names what it found.
    #[must_use]
    pub fn across(&self, ground: &Ground, a: Vec3, b: Vec3, lift: f32, step: f32) -> bool {
        self.across_hit(ground, a, b, lift, step).is_some()
    }

    /// What stands across the way from `a` to `b`, asked **along the road's own profile** rather
    /// than along the straight line between the two.
    ///
    /// `lift` is what makes this about cars rather than about normals: half a metre clears a kerb,
    /// and a guardrail, a central reservation or a building is taller than that. But the height has
    /// to be measured *from the road*, and that is the whole of this method. A straight chord
    /// between two nodes does not follow a crest or a dip — the same fact that forces
    /// [`Network::drop_walled`](crate::world::Network) to work at an 8 m tolerance — so a chord
    /// carried half a metre up runs **underground** wherever the road rises between its ends, and
    /// every steep triangle of the embankment it is buried in answers "wall". Swept and measured:
    /// see `ROADMAP.md`.
    ///
    /// So the walk follows the surface instead. At each step the local ground is taken **nearest
    /// the height the walk is already at** — not the highest, because [`Surface::Drivable`] admits
    /// a flat roof exactly as it admits a road — and the segment tested is the short one from the
    /// previous sample to this one, each carried `lift` above its own ground. The very first
    /// point is the exception: it is `lift` above the caller's own `a.y`, because no sample has
    /// been taken yet and `a.y` is the only height on offer.
    ///
    /// **Following the ground is also this method's own defect, and the returned [`Hit`] is how it
    /// is measured.** Where the city stacks, "nearest the height the walk is already at" can step
    /// onto a bridge deck and then answer about the road underneath it. That is not hypothetical:
    /// a race-line obstacle scan built on the boolean had to be withdrawn for it. Nothing here
    /// filters that out — [`Hit::flat_too`], [`Hit::climb`] and [`Hit::drift`] carry the evidence
    /// out so the caller can.
    ///
    /// **Rejecting a hit is not the same as clearing the way.** This returns at the *first* step
    /// that meets anything and the rest of the walk never runs, so a caller that reads the evidence
    /// and throws the hit away has learned that this blocker is not real — not that there is no
    /// other one further along. To learn that, ask again from just past [`Hit::at`]; there is no
    /// resumption to inherit, because the walk keeps no state a second call could not rebuild.
    #[must_use]
    pub fn across_hit(
        &self,
        ground: &Ground,
        a: Vec3,
        b: Vec3,
        lift: f32,
        step: f32,
    ) -> Option<Hit> {
        let run = Vec3::new(b.x - a.x, 0.0, b.z - a.z).length();
        let n = (run / step).ceil().max(1.0) as usize;
        let mut here = a.y;
        let mut prev = a + Vec3::Y * lift;
        // The caller's own `a.y` is not a floor — the pilot asks from the chassis origin, which
        // stands above the tarmac it is on — so the first *sampled* surface is what the rest of the
        // walk is judged against, and arriving on it is not a climb. Neither is the interpolated
        // height the walk carries where there is no ground: it is held out of both numbers rather
        // than counted as a drop to it and a climb back out.
        let mut floor: Option<f32> = None;
        let mut start = a.y;
        let mut climb = 0.0f32;
        for k in 1..=n {
            let p = a.lerp(b, k as f32 / n as f32);
            // Where the ground has nothing to say, the interpolated height stands in: a link with a
            // genuine hole in it is `drop_walled`'s finding, not this one's, and it has already run.
            let from = here;
            // Bound before the choice so the count of surfaces can be read off it. The selection
            // below is the same expression over the same `Vec` in the same order, and picks the
            // same surface it always did.
            let hs = ground.heights_at(p.x, p.z);
            let layers = hs.len();
            here = hs
                .into_iter()
                .min_by(|x, y| (x - from).abs().total_cmp(&(y - from).abs()))
                .unwrap_or(p.y);
            if layers > 0 {
                match floor {
                    None => start = here,
                    Some(f) => climb = climb.max((here - f).abs()),
                }
                floor = Some(here);
            }
            let cur = Vec3::new(p.x, here + lift, p.z);
            if let Some((tri, index, t)) = self.hits(prev, cur) {
                let at = prev + (cur - prev) * t;
                // Would a level step have met anything at all? That question separates a wall
                // standing across the road from the walk's own change of deck, and it costs one
                // more pass over the same cells on a path that is returning anyway. It asks the
                // index rather than re-testing `tri`, because `tri` is the first triangle in cell
                // order and a barrier is more than one triangle.
                let level = Vec3::new(cur.x, prev.y, cur.z);
                // A walk with no horizontal extent was never carried anywhere, so it is level by
                // construction; asking would hand `segment_hits` a zero-length segment, whose
                // determinant guard answers "miss" and would read as "reached only by climbing".
                let flat_too = run <= 0.0 || self.hits(prev, level).is_some();
                return Some(Hit {
                    tri,
                    at,
                    along: Vec3::new(at.x - a.x, 0.0, at.z - a.z).length(),
                    walk_y: here,
                    from_y: from,
                    start_y: start,
                    climb,
                    layers,
                    step: k,
                    steps: n,
                    index,
                    flat_too,
                });
            }
            prev = cur;
        }
        None
    }

    /// Which wall triangle the segment `p → q` — already at the height it is to be asked about —
    /// meets, with its index and how far along the segment it was met.
    ///
    /// The **first** crossed triangle in cell order, not the nearest one. The short circuit is what
    /// keeps this cheap; asking for the nearest would mean visiting every candidate in every
    /// overlapping cell, and [`Walls::of`] writes a straddling triangle into each cell it touches,
    /// so it would need de-duplicating as well. Cell order is a loop over an integer range rather
    /// than a hash iteration, so the triangle that comes back is the same one from run to run.
    fn hits(&self, p: Vec3, q: Vec3) -> Option<([Vec3; 3], u32, f32)> {
        let key = |v: f32| (v / GROUND_CELL).floor() as i32;
        let (lo_x, hi_x) = (p.x.min(q.x), p.x.max(q.x));
        let (lo_z, hi_z) = (p.z.min(q.z), p.z.max(q.z));
        for cx in key(lo_x)..=key(hi_x) {
            for cz in key(lo_z)..=key(hi_z) {
                let Some(list) = self.runs.get(&(cx, cz)) else { continue };
                for i in list {
                    if let Some(t) = self.tris.get(*i as usize) {
                        if let Some(at) = segment_hits(p, q, t) {
                            return Some((*t, *i, at));
                        }
                    }
                }
            }
        }
        None
    }

    /// How many wall triangles were indexed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tris.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }
}

/// Möller-Trumbore, bounded to the segment: **where** `a → b` passes through the triangle, as a
/// fraction of the segment, or `None` if it misses.
///
/// The fraction was always computed and always thrown away; returning it is what lets a caller say
/// where the blocker is rather than only that there was one.
fn segment_hits(a: Vec3, b: Vec3, t: &[Vec3; 3]) -> Option<f32> {
    let dir = b - a;
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = a - t[0];
    let u = s.dot(p) * inv;
    if !(-1e-4..=1.000_1).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -1e-4 || u + v > 1.000_1 {
        return None;
    }
    // `dir` is not normalised, so this is already a fraction of the segment rather than a distance.
    let hit = e2.dot(q) * inv;
    (0.0..=1.0).contains(&hit).then_some(hit)
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

    /// The derived barrier points away from the city, says nothing in the middle of a road, and
    /// answers about the deck the asker is on rather than the one under it.
    #[test]
    fn the_edge_points_off_the_city() {
        // `remap` is `(-y, z, -x)`: the file's x becomes Gizmo's **z** and its y becomes Gizmo's x.
        // So this is a deck spanning Gizmo x ∈ -40..0, z ∈ -to..0, at height `z_off`.
        let deck = |to: f32, z_off: f32| {
            mesh(
                vec![[0.0, 0.0, z_off], [to, 0.0, z_off], [0.0, 40.0, z_off], [to, 40.0, z_off]],
                vec![0, 1, 2, 1, 3, 2],
            )
        };
        // The lower one reaches z = -40, the upper one stops at z = -20 — so over z ∈ -20..0 there
        // are two surfaces nine metres apart, and the upper one has a lip the lower one does not.
        let g = Ground::of(&collision_cells(&[deck(40.0, 0.0), deck(20.0, 9.0)]));

        // Well inside the lower deck, nothing to report.
        assert_eq!(g.edge_at(Vec3::new(-20.0, 0.0, -30.0), 6.0, 4.0), None, "mid-road is not an edge");

        // Near the lower deck's edge at x = 0, the normal points out of the city — +x.
        let n = g.edge_at(Vec3::new(-3.0, 0.0, -30.0), 6.0, 4.0).expect("an edge near the lip");
        assert!(n.x > 0.7, "expected a normal pointing off the +x edge, got {n:?}");
        assert!(n.y.abs() < 1e-6, "the barrier is a plan-view direction");

        // **The stacked case, and the reason this is not a map.** Standing on the upper deck three
        // metres from its lip at z = -20, the edge is found and points off it — even though the
        // lower deck fills that XZ and a flat "is there ground here" raster would call it inside.
        let n = g.edge_at(Vec3::new(-20.0, 9.0, -17.0), 6.0, 4.0).expect("the upper deck has a lip");
        assert!(n.z < -0.7, "expected the upper deck's own edge, got {n:?}");
        // And on the lower deck directly beneath it, there is no edge at all: the road runs on.
        assert_eq!(g.edge_at(Vec3::new(-20.0, 0.0, -17.0), 6.0, 4.0), None, "the road under it runs on");
    }

    /// An object with no geometry, or an index past its own buffer, is skipped rather than
    /// producing a collider that reads whatever was next in memory.
    #[test]
    fn broken_objects_are_skipped() {
        assert!(collision_cells(&[mesh(Vec::new(), Vec::new())]).is_empty());
        let bad = mesh(vec![[0.0, 0.0, 0.0]], vec![0, 1, 2]);
        assert!(collision_cells(&[bad]).is_empty(), "an index past the end drops its triangle");
    }

    /// A deck spanning Gizmo `x ∈ -40..0`, `z ∈ -to..-from` at height `y_off`. `remap` is
    /// `(-y, z, -x)`, so a quad at a constant file z comes out flat.
    fn deck_between(from: f32, to: f32, y_off: f32) -> WorldMesh {
        mesh(
            vec![[from, 0.0, y_off], [to, 0.0, y_off], [from, 40.0, y_off], [to, 40.0, y_off]],
            vec![0, 1, 2, 1, 3, 2],
        )
    }

    /// The same, starting at the origin.
    fn deck(to: f32, y_off: f32) -> WorldMesh {
        deck_between(0.0, to, y_off)
    }

    /// A vertical face standing across Gizmo `z = -at`, spanning `x ∈ -40..0` and `y ∈ lo..hi`.
    /// A constant file **x** is a constant Gizmo z, and the file's z is Gizmo's height.
    fn wall(at: f32, lo: f32, hi: f32) -> WorldMesh {
        mesh(
            vec![[at, 0.0, lo], [at, 40.0, lo], [at, 0.0, hi], [at, 40.0, hi]],
            vec![0, 1, 2, 1, 3, 2],
        )
    }

    /// The walk stops at a wall and now says which one, where, and what the floor was doing —
    /// on a level road, where every one of those answers is knowable by hand.
    #[test]
    fn a_hit_names_the_triangle_it_met() {
        let cells = collision_cells(&[deck(40.0, 0.0), wall(20.0, 0.0, 3.0)]);
        let (ground, walls) = (Ground::of(&cells), Walls::of(&cells));
        assert_eq!(walls.len(), 2, "the wall quad is two triangles");

        let a = Vec3::new(-20.0, 0.0, -30.0);
        let b = Vec3::new(-20.0, 0.0, -10.0);
        let hit = walls.across_hit(&ground, a, b, 0.5, 3.0).expect("a wall stands across the road");

        let (lo, hi) = hit.span();
        assert!((lo - 0.0).abs() < 1e-3 && (hi - 3.0).abs() < 1e-3, "span {lo}..{hi}, want 0..3");
        assert!((hit.at.z + 20.0).abs() < 1e-2, "met at z {}, want -20", hit.at.z);
        assert!((hit.at.y - 0.5).abs() < 1e-2, "met at the lift height, got {}", hit.at.y);
        assert!((hit.along - 10.0).abs() < 0.1, "10 m along, got {}", hit.along);
        assert!(hit.walk_y.abs() < 1e-3 && hit.drift().abs() < 1e-3, "a level road does not drift");
        assert!(hit.climb.abs() < 1e-3, "a level road has no climb");
        assert_eq!(hit.layers, 1, "one deck under this step");
        // 20 m of run at 3 m a step, and the wall is 10 m in.
        assert_eq!((hit.step, hit.steps), (4, 7));
        assert!(hit.index < walls.len() as u32);
        // On a level road a real wall is still there when the step is carried flat. This is the
        // signature the stacked case below does **not** have.
        assert!(hit.flat_too, "a wall across a level road is hit level too");
    }

    /// **The regression test for a withdrawn finding.** An obstacle scan built on the boolean read
    /// 3-25 % of race-line edges as blocked, and the reading had to be given up because the walk
    /// follows the ground: leaving a deck it drops to the road beneath and answers about geometry
    /// standing down there. The boolean cannot tell that from a wall; the hit can.
    #[test]
    fn the_road_below_a_deck_is_not_a_wall_across_it() {
        // Lower road over z ∈ -40..0, upper deck over z ∈ -20..0 nine metres up, and a three-metre
        // face standing on the **lower** road just past the upper deck's lip.
        let cells = collision_cells(&[deck(40.0, 0.0), deck(20.0, 9.0), wall(21.0, 0.0, 3.0)]);
        let (ground, walls) = (Ground::of(&cells), Walls::of(&cells));
        assert_eq!(ground.heights_at(-20.0, -15.0).len(), 2, "the stack is where the test needs it");

        // Driving along the upper deck, off its end.
        let a = Vec3::new(-20.0, 9.0, -10.0);
        let b = Vec3::new(-20.0, 9.0, -30.0);
        assert!(walls.across(&ground, a, b, 0.5, 3.0), "the boolean says blocked — and is no use");
        let hit = walls.across_hit(&ground, a, b, 0.5, 3.0).expect("same walk, same answer");

        assert!((hit.start_y - 9.0).abs() < 1e-3, "the walk began on the upper deck");
        assert!((hit.from_y - 9.0).abs() < 1e-3 && hit.walk_y.abs() < 1e-3, "and fell off it here");
        assert!((hit.climb - 9.0).abs() < 1e-3, "a nine-metre change of floor, got {}", hit.climb);
        assert!((hit.drift() + 9.0).abs() < 1e-3, "nine metres below where it started");
        assert!(hit.span().1 <= hit.start_y, "the whole triangle is under the deck it set out on");
        let (over_lo, over_hi) = hit.over_floor();
        assert!(over_lo > -0.5 && over_hi > 2.0, "it stands on the lower road: {over_lo}..{over_hi}");
        // The separation itself: carried level at the height the step came in at, this step meets
        // nothing. Whatever stopped it, a car on the upper deck could not have driven into it.
        assert!(!hit.flat_too, "the walk reached this only by changing floors");
    }

    /// The old question, asked through the new one. Four shapes, so a later refactor cannot let the
    /// two answers drift apart.
    #[test]
    fn the_boolean_is_the_hit_asked_twice() {
        // The verdict is written down as well as compared. While `across` delegates, agreement is
        // a tautology and only catches a future re-implementation; the expected column is what
        // catches this walk quietly changing its mind.
        let cases = [
            ("clear road", false, vec![deck(40.0, 0.0)]),
            ("a wall across it", true, vec![deck(40.0, 0.0), wall(20.0, 0.0, 3.0)]),
            ("a stack", true, vec![deck(40.0, 0.0), deck(20.0, 9.0), wall(21.0, 0.0, 3.0)]),
            ("no ground at all", true, vec![wall(20.0, 0.0, 3.0)]),
        ];
        for (name, want, meshes) in cases {
            let cells = collision_cells(&meshes);
            let (ground, walls) = (Ground::of(&cells), Walls::of(&cells));
            let (a, b) = (Vec3::new(-20.0, 0.0, -30.0), Vec3::new(-20.0, 0.0, -10.0));
            assert_eq!(walls.across(&ground, a, b, 0.5, 3.0), want, "{name}");
            assert_eq!(
                walls.across(&ground, a, b, 0.5, 3.0),
                walls.across_hit(&ground, a, b, 0.5, 3.0).is_some(),
                "{name}"
            );
        }
    }

    /// Clear road, and the index knows it holds nothing.
    #[test]
    fn nothing_in_the_way_is_no_hit() {
        let cells = collision_cells(&[deck(40.0, 0.0)]);
        let (ground, walls) = (Ground::of(&cells), Walls::of(&cells));
        assert!(walls.is_empty(), "a flat deck contributes no wall triangles");
        let (a, b) = (Vec3::new(-20.0, 0.0, -30.0), Vec3::new(-20.0, 0.0, -10.0));
        assert_eq!(walls.across_hit(&ground, a, b, 0.5, 3.0), None);
        assert!(!walls.across(&ground, a, b, 0.5, 3.0));
    }

    /// Off the end of the road the walk carries the interpolated height instead of a floor, and
    /// says so. "No ground here" and "the floor changed" are different findings and this is what
    /// keeps them apart.
    #[test]
    fn a_step_with_no_ground_reports_no_layers() {
        // The road stops at z = -40; the face stands ten metres past it, over nothing.
        let cells = collision_cells(&[deck(40.0, 0.0), wall(50.0, 0.0, 3.0)]);
        let (ground, walls) = (Ground::of(&cells), Walls::of(&cells));
        let a = Vec3::new(-20.0, 0.0, -30.0);
        let b = Vec3::new(-20.0, 0.0, -60.0);
        let hit = walls.across_hit(&ground, a, b, 0.5, 3.0).expect("the face is still hit");
        assert_eq!(hit.layers, 0, "there is no drivable surface under this step");
        assert!((hit.at.z + 50.0).abs() < 1e-2, "met at z {}, want -50", hit.at.z);
        // `heights_at` gave nothing, so the walk kept the interpolated height rather than inventing
        // a floor — and that is not a change of deck.
        assert!(hit.walk_y.abs() < 1e-3 && hit.climb.abs() < 1e-3);
    }

    /// The parameter the triangle test always computed and always threw away: it is a fraction of
    /// the segment, not a distance, and the point it names lies in the triangle's plane.
    #[test]
    fn the_segment_parameter_lands_on_the_triangle() {
        let t = [
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(-10.0, 0.0, -10.0),
            Vec3::new(0.0, 10.0, -10.0),
        ];
        let (a, b) = (Vec3::new(-2.0, 2.0, -12.0), Vec3::new(-2.0, 2.0, -8.0));
        let f = segment_hits(a, b, &t).expect("straight through the middle of it");
        assert!((f - 0.5).abs() < 1e-4, "halfway along, got {f}");
        assert!(((a + (b - a) * f).z + 10.0).abs() < 1e-4, "and in the triangle's own plane");
        // Past the far end is a miss, which is what "bounded to the segment" means.
        assert_eq!(segment_hits(a, Vec3::new(-2.0, 2.0, -11.0), &t), None);
    }

    /// **A crossed triangle, not the nearest one.** The search short-circuits on the first hit in
    /// cell order, and that is deliberate — asking for the nearest ends the short circuit. Written
    /// down as a test so nobody builds a distance on top of it by accident.
    #[test]
    fn the_hit_is_a_crossed_triangle_not_the_nearest_one() {
        // Both faces sit in the same 64 m cell and one step crosses both: the walk runs 20 m in
        // seven steps, so step 4 spans z -21.4 → -18.6. Walking from z = -30 the **nearer** face is
        // the one at -21; the one at -20 is indexed first because its mesh is listed first, and
        // that is the one that comes back.
        let cells = collision_cells(&[deck(40.0, 0.0), wall(20.0, 0.0, 3.0), wall(21.0, 0.0, 3.0)]);
        let (ground, walls) = (Ground::of(&cells), Walls::of(&cells));
        let a = Vec3::new(-20.0, 0.0, -30.0);
        let hit = walls
            .across_hit(&ground, a, Vec3::new(-20.0, 0.0, -10.0), 0.5, 3.0)
            .expect("two faces, one step crosses both");
        assert!(
            (hit.at.z + 20.0).abs() < 1e-2,
            "met at z {}, want the farther face at -20 — index order, not distance",
            hit.at.z
        );
        // Swapping the two meshes swaps the answer, which is the whole point: this is an order, not
        // a distance. If it ever becomes a distance, this is the assertion that says so.
        let swapped =
            collision_cells(&[deck(40.0, 0.0), wall(21.0, 0.0, 3.0), wall(20.0, 0.0, 3.0)]);
        let (g2, w2) = (Ground::of(&swapped), Walls::of(&swapped));
        let other = w2
            .across_hit(&g2, a, Vec3::new(-20.0, 0.0, -10.0), 0.5, 3.0)
            .expect("same two faces");
        assert!((other.at.z + 21.0).abs() < 1e-2, "met at z {}, want -21", other.at.z);
    }

    /// **A wall on the deck the car is actually on.** The walk leaves the upper deck and drops to
    /// the road below in one step, and there is a face at that spot on *both* levels. The first
    /// triangle in cell order is the one on the lower road — so re-testing that one level would
    /// answer "reached only by changing floors" and the caller would throw away a barrier standing
    /// squarely across the deck it set out on. Asking the index instead is what keeps it.
    #[test]
    fn a_wall_on_the_deck_is_not_dismissed_with_the_road_below() {
        let cells = collision_cells(&[
            deck(40.0, 0.0),
            deck(20.0, 9.0),
            wall(21.0, 0.0, 3.0),
            wall(21.0, 9.0, 12.0),
        ]);
        let (ground, walls) = (Ground::of(&cells), Walls::of(&cells));
        let a = Vec3::new(-20.0, 9.0, -10.0);
        let b = Vec3::new(-20.0, 9.0, -30.0);
        let hit = walls.across_hit(&ground, a, b, 0.5, 3.0).expect("something is in the way");

        // The triangle returned is the lower one — it was indexed first — and on its own it reads
        // exactly like the withdrawn finding's false positive.
        assert!(hit.span().1 <= 3.0 + 1e-3, "the lower face came back first, as it must");
        assert!((hit.climb - 9.0).abs() < 1e-3, "and the walk did change deck, got {}", hit.climb);
        // But something *does* stand across the way at the height the step came in at.
        assert!(hit.flat_too, "the face on the upper deck is still there when the step is level");
    }

    /// **A hole in the road is not a change of floor.** Where `heights_at` has nothing to say the
    /// walk carries the caller's own interpolated height, which is not a surface — counting it
    /// would make every gap read as a drop onto another deck and back out of it.
    #[test]
    fn a_stretch_with_no_ground_is_not_a_climb() {
        // Road from z = 0 to -12, nothing until -28, road again to -40, and a face at -34. The
        // query is aimed eight metres up, so over the gap the interpolated height climbs away from
        // the road on both sides of it — the shape that used to be counted as a change of deck.
        let cells = collision_cells(&[
            deck_between(0.0, 12.0, 0.0),
            deck_between(28.0, 40.0, 0.0),
            wall(34.0, 0.0, 3.0),
        ]);
        let (ground, walls) = (Ground::of(&cells), Walls::of(&cells));
        assert!(ground.heights_at(-20.0, -20.0).is_empty(), "the gap is where the test needs it");
        let a = Vec3::new(-20.0, 0.0, -5.0);
        let b = Vec3::new(-20.0, 8.0, -38.0);
        let hit = walls.across_hit(&ground, a, b, 0.5, 3.0).expect("the face is met");

        assert!((hit.at.z + 34.0).abs() < 1e-2, "met at z {}, want -34", hit.at.z);
        assert_eq!(hit.layers, 1, "back on the road by then");
        assert!(hit.walk_y.abs() < 1e-3 && hit.start_y.abs() < 1e-3, "one road, one height");
        assert!(hit.climb.abs() < 1e-3, "level road on both sides of the gap, got {}", hit.climb);
        assert!(hit.drift().abs() < 1e-3, "and the walk ends where it started");
    }
}
