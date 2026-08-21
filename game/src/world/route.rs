//! The race line, standing on the city.
//!
//! [`gizmo_nfs::world::routes`] reads a route file into nodes: a position in the world's own frame,
//! which path of the file's network the node belongs to, the junctions to other paths, and the
//! distance along. What it cannot give is **height** — the record has none, and the city is the only
//! thing that knows. That is this module's whole job, and it is the game's rather than the parser's
//! for exactly that reason: answering it needs the collision geometry.
//!
//! ## Which surface, when there is more than one
//!
//! Two rules, and both were arrived at by getting it wrong first.
//!
//! **Ask only the roads.** Bayview stacks, and [`crate::world::Surface`] judges a triangle by its
//! normal, so a flat roof is drivable in exactly the sense tarmac is. Over every drivable triangle
//! the stack under a node is 79 m deep; taking the topmost put six of `Paths4001`'s 40 paths on
//! rooftops at y 115–133 with the road at 22–28 beneath them. [`road_ground`] is the fix and it is
//! most of the answer: it leaves 259 of 341 nodes with exactly one candidate.
//!
//! **Where candidates remain, take the sequence that climbs least.** Those are overpasses, and a
//! per-node rule cannot choose between the deck and the road under it — but a path can, because it
//! is continuous. [`follow`] minimises the total `|Δh|` over the whole path, so no seed is needed
//! and no node decides alone.
//!
//! Least-climb is only safe *because* of the road filter: over all drivable triangles it is
//! degenerate, since the flat shelf beneath the city climbs by nothing at all and wins everywhere.
//! That is what put the line under the road when it was tried the other way round.
//!
//! It is still a judgement, so it is left measurable. [`RoutePath::climbed`] is what shows it
//! failing — a path that gains a storey between two nodes 29 m apart did not drive there.

use super::{collision_cells, remap, Ground};
use gizmo::prelude::*;
use gizmo_nfs::world::routes::{paths, RaceEvent, RouteNode, StartMarker};
use gizmo_nfs::world::WorldMesh;

/// Whether a city object is road surface.
///
/// A **render-and-placement category, not a parser fact**, in the same family as
/// [`super::is_backdrop`]: the file does not label roads, and the name is what the artists left
/// behind. The city names its terrain `TRN_<place>_<class>_..._CHOP_<cell>_<lod>`, and this catches
/// every class whose name carries `ROAD`: **1,928 of the city's 13,986 objects, 89 of its 4,699
/// name families**, with every bundle loaded. Mostly `ROADA` (1,537), then `ROAD#` (184),
/// `ROADDRAG` (88), `ROAD` (39), `ROADB` (27) and the `ROADPIECE*` set. Names are truncated to 27
/// characters, but the class field starts at character eight, so the token is never cut.
///
/// **Fifteen of those are not surface** — `ROADSIGNB` (3), `ROADBARRIERB` (3), `ROADSKID*` (9). A
/// sign and a barrier are vertical geometry and [`super::surface_of`] should call them wall, but
/// that has not been measured.
///
/// **An earlier version of this sentence also claimed `RDP_*`, and that is wrong.** `RDP` is not a
/// road class but a *place* — the airport: its 699 objects are `TRN_RDP_RUNWAY_*` (630),
/// `TRN_RDP_DRAG#_*` (44) and `TRN_RDP_RUNWAYSKID_*` (25), and **none** of them has `ROAD` in its
/// name, so this matches zero of them.
///
/// **Swept 2026-08-21, and it is not too narrow.** What a name filter drops is the half that has to
/// be measured, so the other classes were counted against the eight sweep routes' own nodes: 131 of
/// their 2,052 have no road object under them. Adding `TUNNEL`, `TUNNNEL` (the city's own
/// misspelling, six objects), `BRIDGE`, `MERIDIAN`, `RUNWAY`, `DRIFT` or `PUDDLE` to this test
/// rescues **zero** of them, on every route. The three that would rescue any must not be added:
/// `TERRAIN` (131 — all of them) is the flat shelf this filter exists to exclude, `CEILING` (7) is
/// an overpass soffit, and `TRAINTRACK` (34) is a rail — on `Paths4041` the rails lie at y = −1
/// with the ground the race drives on 9.7 m above them.
///
/// Nor is the surface test hiding roads behind a classification: of those nodes, **none** has a
/// road-named object under it that [`super::surface_of`] called a wall. Where this says no road,
/// there is no road object — [`super::Network::of`] answers those nodes from the drivable ground.
///
/// Ask it with `NFS_ROADNAMES=<n>` in `nfs_sim`; [`super::surfaces_by_object`] is what that uses.
pub fn is_road(name: &str) -> bool {
    name.contains("ROAD")
}

/// The surface a route is allowed to stand on: [`Ground`] built from road objects alone.
///
/// **This filter is the difference between a race line and a guess.** Built from every drivable
/// triangle, the stack under a node of `Paths4001` is at least two surfaces deep at *every* node,
/// four or more at 179 of its 341, and 79 m from top to bottom — terrain shelves, car park decks,
/// and flat roofs, which [`super::Surface`] cannot tell from tarmac because it judges by the normal
/// and a roof is horizontal. Built from roads, 259 of the 341 have exactly **one** candidate and
/// the spread of the rest falls to 15 m, which is what an overpass actually is.
#[must_use]
pub fn road_ground(meshes: &[WorldMesh]) -> Ground {
    let roads: Vec<WorldMesh> =
        meshes.iter().filter(|m| is_road(&m.header.name)).cloned().collect();
    Ground::of(&collision_cells(&roads))
}

/// One path of a route file, in the Gizmo frame and standing on the city.
#[derive(Debug, Clone)]
pub struct RoutePath {
    /// Which path of its file this is.
    pub index: u16,
    /// The path in world space, in driving order.
    pub points: Vec<Vec3>,
    /// The file's own cumulative distance at each point, in world units. Carried rather than
    /// recomputed: it is what the game itself measured a lap by, and a recomputed length would
    /// quietly differ wherever the height changed something.
    pub progress: Vec<f32>,
    /// Points the city had no drivable ground under, whose height came from their neighbours
    /// instead. A path with many of these is standing somewhere the reader does not cover.
    pub filled: usize,
}

impl RoutePath {
    /// The largest height change between two consecutive points.
    ///
    /// The number that shows the follow rule went wrong: a path that gains a storey between two
    /// nodes 29 m apart climbed onto something rather than driving up it.
    #[must_use]
    pub fn climbed(&self) -> f32 {
        self.points
            .windows(2)
            .map(|p| (p[1].y - p[0].y).abs())
            .fold(0.0, f32::max)
    }

    /// Length of the polyline as built, in metres.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|p| (p[1] - p[0]).length()).sum()
    }
}

/// Put a route file's paths on the city.
///
/// The nodes come from [`gizmo_nfs::world::routes::nodes`] and keep their file order, which *is*
/// the driving order once split by path — see that module for what pins it.
#[must_use]
pub fn build(nodes: &[RouteNode], ground: &Ground) -> Vec<RoutePath> {
    paths(nodes)
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|path| {
            // **File order is the sequence, not the direction.** Nearly half the install's paths —
            // 1,361 of 2,923 — are stored with `progress` falling along the file, and the file says
            // so itself in `0x00034149`. Left as they are, half the race line is drawn and driven
            // backwards, which is not visible in a ribbon and is very visible in a car.
            let path: Vec<_> = if path[path.len() - 1].progress < path[0].progress {
                path.iter().rev().copied().collect()
            } else {
                path.to_vec()
            };
            let flat: Vec<Vec3> = path.iter().map(|n| remap([n.x, n.y, 0.0])).collect();

            let candidates: Vec<Vec<f32>> =
                flat.iter().map(|p| ground.heights_at(p.x, p.z)).collect();
            let mut heights = follow(&candidates);
            let filled = fill(&mut heights);

            RoutePath {
                index: path[0].path,
                points: flat
                    .iter()
                    .zip(&heights)
                    .map(|(p, h)| Vec3::new(p.x, h.unwrap_or(0.0), p.z))
                    .collect(),
                progress: path.iter().map(|n| n.progress).collect(),
                filled,
            }
        })
        .collect()
}

/// Where a race starts, as the file puts it: the pole slot of one of the event's starting grids.
///
/// This replaces a guess. Before the grids were read, the start was taken to be the path node of
/// least `progress`, which is *a* place the race passes and not where the cars are put — testing
/// the corridor from it dropped the car on a bridge approach over water.
///
/// **The heading is derived, and the derivation is measured.** A marker carries a position and no
/// direction, so the way the car faces comes from the grid's own shape: four cars abreast and two
/// rows deep, so the axis between the rows is the axis of the road. That is not an assumption —
/// against the route network, all **191** grids that belong to an event sit a median 11 m from its
/// nearest node and the angle between the grid axis and the road there is under 30° or over 150°
/// for **every one of them**, with nothing in between.
///
/// What the file does *not* say is which end is the front. Pole leads, so the row holding slots
/// `0..3` is taken as the front one and the car faces away from the row behind it. A track carries
/// two grids, one per race direction, and this returns the first. Both mistakes look the same and
/// cost one line: a car facing backwards.
///
/// **"The first" turns out to be right, and that is now measured rather than hoped.** A track's two
/// grids really are its two race directions: of 86 tracks with exactly two, **74 have them facing
/// opposite ways**, and in **82** exactly one of the two agrees with the direction the event's
/// outline is drawn in. Held against that, the first grid is the agreeing one in **81 of those 82**
/// — against the 41 a coin flip would give. So the file's own grid order carries the answer, and
/// this function was accidentally right.
///
/// [`start_grid_facing`] derives it instead of assuming it and fixes the remaining one. Prefer it
/// where the event is at hand; this stays as the fallback for when it is not.
///
/// What none of this settles is the global sense of the front/back rule. If slots `0..3` were the
/// back row everywhere, the same 81 of 82 would hold with every grid's heading flipped. It is one
/// global bit rather than a per-race coin flip, and a wrong one shows up as every car on every race
/// facing backwards — visible in a single run.
///
/// **The `Routes####F.bin` / `Routes####B.bin` files were expected to settle this and do not.**
/// They are read now — `gizmo_nfs::world::routes::lanes` — and they do say what a forward and a
/// backward reading of a route differ by: the same blocks, same names, same order, same point
/// counts, with only the along-route measure taken from opposite ends. But that measure runs along
/// a *block*, and a block is a stretch of road shared between races rather than one race's line, so
/// it does not locate a start. Asked directly, of 442 full grids the nearest lane point puts 116
/// near a block's start, 54 near its end and 212 in the middle — no answer. The assumption stands
/// and is still an assumption.
#[must_use]
pub fn start_grid(markers: &[StartMarker], event: u16) -> Option<(Vec3, Vec3)> {
    let grid = gizmo_nfs::world::routes::grids(markers)
        .into_iter()
        .find(|g| g[0].track == u32::from(event))?;
    let mean = |slots: &[StartMarker]| {
        slots.iter().fold(Vec3::ZERO, |a, m| a + remap(m.at)) / slots.len() as f32
    };
    let (front, back) = (mean(&grid[..4]), mean(&grid[4..]));
    let heading = (front - back).normalize_or_zero();
    (heading != Vec3::ZERO).then(|| (remap(grid[0].at), heading))
}

/// Where a race starts, with the two grids told apart by the event's own outline.
///
/// [`start_grid`] returns the first of a track's two grids; this returns the one whose derived
/// heading agrees with the direction [`RaceEvent::outline`] is drawn in. Measured over the install:
/// of 86 tracks with exactly two full grids, 74 have the two facing opposite ways and **82 have
/// exactly one of the two agreeing with the outline**, so the outline is a real selector and not a
/// tie-break dressed up as one.
///
/// **It changes the answer for exactly one track.** The first grid is already the agreeing one in
/// 81 of the 82, which is the useful finding here — the file's grid order means something — and it
/// makes this function a derivation of a rule that was being assumed, plus one fix, rather than the
/// correction of a coin flip. Worth having for the derivation; not worth overstating.
///
/// Falls back to [`start_grid`] when the event has no usable outline, so a caller never loses a
/// spawn to a missing one.
#[must_use]
pub fn start_grid_facing(markers: &[StartMarker], event: &RaceEvent) -> Option<(Vec3, Vec3)> {
    match pick_grid(markers, event) {
        Some((slots, heading)) => Some((slots[0], heading)),
        None => start_grid(markers, event.id),
    }
}

/// The whole grid the race uses: all eight places, in slot order, and the way they face.
///
/// Same choice of grid as [`start_grid_facing`] — they share [`pick_grid`], so the pole this
/// returns and the pole that returns cannot drift apart. Slot 0 is pole.
///
/// The heading is one vector for the eight, because a starting grid is a formation: the file gives
/// eight positions and no directions, and the direction is a property of the formation rather than
/// of any car in it.
#[must_use]
pub fn start_slots(markers: &[StartMarker], event: &RaceEvent) -> Option<(Vec<Vec3>, Vec3)> {
    pick_grid(markers, event)
}

/// The grid a race uses, as eight world positions in slot order plus the shared heading.
///
/// Picks between a track's two grids by the direction its event outline is drawn in — see
/// [`start_grid_facing`] for what that is worth in numbers. `None` when the event has no usable
/// outline or no full grid of its own.
fn pick_grid(markers: &[StartMarker], event: &RaceEvent) -> Option<(Vec<Vec3>, Vec3)> {
    if event.outline.len() < 2 {
        return None;
    }
    let mean = |slots: &[StartMarker]| {
        slots.iter().fold(Vec3::ZERO, |a, m| a + remap(m.at)) / slots.len() as f32
    };
    let mut best: Option<(f32, Vec<Vec3>, Vec3)> = None;
    for grid in gizmo_nfs::world::routes::grids(markers)
        .into_iter()
        .filter(|g| g[0].track == u32::from(event.id))
    {
        let (front, back) = (mean(&grid[..4]), mean(&grid[4..]));
        let heading = (front - back).normalize_or_zero();
        if heading == Vec3::ZERO {
            continue;
        }
        let centre = (front + back) * 0.5;
        // The outline is in the file's own 2-D frame, so bring it into ours the same way a marker
        // comes: through `remap`, with no height to speak of.
        let seg = event
            .outline
            .windows(2)
            .map(|w| {
                let (a, b) = (remap([w[0][0], w[0][1], 0.0]), remap([w[1][0], w[1][1], 0.0]));
                ((a.midpoint(b) - centre).length_squared(), b - a)
            })
            .min_by(|x, y| x.0.total_cmp(&y.0));
        let Some((d, dir)) = seg else { continue };
        let along = heading.dot(dir.normalize_or_zero());
        // Nearest segment wins the tie; among grids, the one that agrees wins outright.
        let score = if along >= 0.0 { -d } else { f32::MIN };
        if best.as_ref().is_none_or(|(s, _, _)| score > *s) {
            best = Some((score, grid.iter().map(|m| remap(m.at)).collect(), heading));
        }
    }
    best.map(|(_, slots, dir)| (slots, dir))
}

/// Split a course outline into steps no longer than `step`, in order.
///
/// **An outline is a description, not a line to drive.** `0x3414C` gives 17 corners over 6 km — a
/// median step of 425 m — so the nearest corner to a starting grid can be 200 m away simply
/// because the grid is in the middle of one. Measured over eight races, the nearest corner was
/// 10 and 13 m on two of them and 133 and 197 m on two others, and a pilot aiming that far off
/// drives that far off.
///
/// Subdividing costs nothing and removes the whole class of problem: the course becomes a sequence
/// of points a car is never far from, and "the waypoint nearest the grid" means what it sounds
/// like. It adds no information — the corners are still the only thing the file said — it just
/// stops the gaps between them being places a driver can get lost in.
///
/// **40 m came from a sweep over eight races**, counting cars that got away and how far the field
/// reached: 25 m gave 29 cars and 6.3 km, **40 m gave 29 and 8.1 km**, 80 m gave 27 and 7.8 km,
/// 200 m gave 24 and 6.8 km, and leaving the outline alone gave 20 and 6.7 km. Too fine and the
/// pilot chases a point under its own bumper; too coarse and the gaps come back.
#[must_use]
pub fn densify(outline: &[Vec3], step: f32) -> Vec<Vec3> {
    let mut out = Vec::new();
    for w in outline.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = ((b - a).length() / step).ceil().max(1.0) as usize;
        for k in 0..n {
            out.push(a.lerp(b, k as f32 / n as f32));
        }
    }
    if let Some(last) = outline.last() {
        out.push(*last);
    }
    out
}

/// The course ring, built by **walking the roads** between the outline's corners instead of
/// interpolating across them.
///
/// **Why.** [`densify`] draws a straight chord between corners that are up to 425 m apart, and
/// measured over the eight sweep routes that puts **55 % of the ring outside the course corridor**
/// and 11 % of it over no ground at all — 928 waypoints, 518 outside, 108 in the air. The pilot
/// aims at that ring, so more than half the time it is aiming at somewhere there is no road; four
/// separate routing rules were refuted in one day trying to pull cars *towards* it.
///
/// The corners still say where the race goes — that is all the file gives — but the way between two
/// of them is taken from [`Network`], which is roads. Returns the ring and how many legs had to
/// fall back to the chord because the graph could not join their ends; a caller that does not print
/// that number is hiding the part of the course that is still a guess.
///
/// **Swept over eight routes, and as built it is a regression — `NFS_WALKLINE` is off by default.**
/// The ring it draws is very nearly perfect as geometry: the chord version puts 518 of 928
/// waypoints outside the corridor and 108 over nothing, this one puts **5 of 1,426** outside and 3
/// over nothing. The driving is worse anyway:
///
/// | ring | furthest | never lost the course | fell |
/// |---|---|---|---|
/// | chord | **5 413 m** | **32 / 64** | 4 |
/// | walked | 5 193 m | 15 / 64 | 2 |
/// | walked, refusing walled links | 5 155 m | 21 / 64 | 3 |
///
/// **And the reason is the one the graph's own header warns about.** This network joins roads that
/// merely run beside each other, so a *committed* shortest path crosses joins a car cannot take —
/// the same sentence that explains why `guide_to` lost in the driving role. Counted on the ring
/// itself, consecutive waypoints with something standing between them at car height: **4001 has 15,
/// 4081 has 14, 4121 has 8, 4021 has 7 — and 4061 has none.** Those are exactly the routes whose
/// cars stopped staying on the course (4001 8 → 0, 4081 5 → 2, 4021 3 → 0), and the one with a
/// clean ring is the one that did not move. The pilot is being steered through a central
/// reservation.
///
/// **Teaching the search that a walled link is not a road recovers a third of the loss and confirms
/// the diagnosis** — the `passable` argument, and `NFS_WALKWALLS=0` to take it away again. Refusing
/// the links something stands across takes the ring's own crossed pairs from 15 to 8 on 4001 and
/// the field's cars-that-stay-on-course from 15 back to **21**, at the price of four legs that can
/// no longer be joined and fall back to the chord. It is not enough: the chord ring still keeps 32.
///
/// **The remaining gap is not a defect in this ring: it is that the two rings are different
/// courses.** Counted route by route, every walked ring is longer than its chord (+5 % to **+118 %**)
/// and three to six times as twisty — waypoints imposing a limit under 80 km/h go from 4-13 to
/// 24-61. Of course they do: it follows the roads and the chord cuts across the blocks. So
/// `furthest` and "waypoints driven past" **flatter the fictional ring**, and cannot rank the two;
/// `away` (64 either way) and `fallen` are what survive.
///
/// Two real defects fell out of that count and both are fixed above: a leg that turns straight back
/// out of where the last one arrived (4001's revisits 9 → 1, its ring 7,159 → 6,238 m) and a leg
/// whose road is **22.6 times** its chord (4021's leg 6, 108 m across and 2,436 m by road — half
/// that ring on its own; guarded, the ring goes 5,192 → 2,811 m and its revisits 81 → 3).
///
/// **Neither moved the field**, which is the point worth keeping: over eight routes the walked ring
/// scores 5,155 m and 21 cars before the fixes and 5,157 m and 21 after, with only `fallen` better
/// at 2. 4021 says it loudest — its ring halved and the driving result is identical, because its
/// cars stop at 623 m and the leg that was wrong begins at 619. The shape of the ring past where
/// anybody reaches is not what is holding the field.
///
/// The graph is deliberately untouched by all of this. [`super::Network::drop_walled`] asks whether
/// the *ground* continues along a link and nothing asks it about walls, because a wall filter
/// inside the graph was swept and thrown out once already — this test is given to the course
/// builder alone.
#[must_use]
pub fn along_roads(
    net: &super::Network,
    outline: &[Vec3],
    step: f32,
    detour: f32,
    passable: impl Fn(u32, u32) -> bool,
) -> (Vec<Vec3>, usize) {
    let mut poly: Vec<Vec3> = Vec::new();
    let mut chords = 0usize;
    // Two nodes of a route file can sit on top of each other; a polyline with a zero-length segment
    // has no direction, which every consumer of this ring asks it for.
    let push = |p: Vec3, poly: &mut Vec<Vec3>| {
        if poly.last().is_none_or(|q| Vec3::new(p.x - q.x, 0.0, p.z - q.z).length() > 0.5) {
            poly.push(p);
        }
    };
    // **Snap in plan, not in space.** [`Network::nearest`] is deliberately three-dimensional —
    // Bayview stacks four roads over one another and a driver's "nearest node" has to mean the one
    // on its own deck. An outline corner has no deck: it arrives as `remap([x, y, 0.0])`, so its
    // height is a literal zero and a 3-D snap prefers whichever node happens to sit lowest. The
    // corner is a plan-view mark on a map and is matched as one.
    let snap = |p: Vec3| {
        (0..net.len() as u32)
            .filter_map(|i| net.node(i).map(|n| (i, n.at)))
            .min_by(|a, b| {
                let d = |q: Vec3| Vec3::new(q.x - p.x, 0.0, q.z - p.z).length();
                d(a.1).total_cmp(&d(b.1))
            })
            .map(|(i, _)| i)
    };
    // **Where the last leg came in from, so this one does not go straight back out of it.**
    // Measured on `Paths4021`: joining legs with no memory drives a street twice — 80 of its
    // waypoints revisit somewhere the ring has already been and the ring more than doubles in
    // length — because the shortest way to the next corner is often back down the road just
    // travelled. A race does not do that. Only the one step is forbidden, not the whole leg: a
    // course *can* legitimately come back along a road later, and forbidding that would be the
    // refuted "cannot terminate on a cycle" mistake in another costume.
    let mut arrived_from: Option<u32> = None;
    for w in outline.windows(2) {
        let legs = snap(w[0])
            .zip(snap(w[1]))
            .and_then(|(a, b)| {
                let back = arrived_from;
                net.path_where(a, b, |x, y| {
                    passable(x, y) && !(x == a && Some(y) == back)
                })
            })
            .filter(|ids| {
                // **A road route many times the length of the chord is not this leg's road.**
                // Measured on `Paths4021`: one leg is 108 m across and 2,436 m by road — **×22.6**,
                // half the whole ring on its own, and the source of the 80 places that ring comes
                // back to somewhere it has already been. A detour that large means the graph has no
                // road for these two corners and is going round a whole block system instead; the
                // chord is the more honest answer and is at least short. `detour` is the multiple
                // allowed, and 0 turns the guard off.
                if detour <= 0.0 {
                    return true;
                }
                let plan = |a: Vec3, b: Vec3| Vec3::new(b.x - a.x, 0.0, b.z - a.z).length();
                let road: f32 = ids
                    .windows(2)
                    .filter_map(|p| Some(plan(net.node(p[0])?.at, net.node(p[1])?.at)))
                    .sum();
                road <= plan(w[0], w[1]).max(1.0) * detour
            })
            .map(|ids| {
                arrived_from = ids.iter().rev().nth(1).copied();
                ids.iter().filter_map(|i| net.node(*i)).map(|n| n.at).collect::<Vec<_>>()
            });
        match legs {
            Some(pts) if pts.len() > 1 => {
                for p in pts {
                    push(p, &mut poly);
                }
            }
            // No road joins these two corners: keep the chord rather than dropping the leg, so the
            // ring still goes where the race says even where this cannot improve on it.
            _ => {
                chords += 1;
                arrived_from = None;
                push(w[0], &mut poly);
                push(w[1], &mut poly);
            }
        }
    }
    (densify(&poly, step), chords)
}

/// The track id free roam's own markers carry.
///
/// Not a race number: no `Paths4000.bin` exists. It is the id the free-roam markers are filed
/// under, and it appears nowhere else.
pub const FREE_ROAM_TRACK: u16 = 4000;

/// Where free roam starts, and the way the car faces.
///
/// `TrackPosMarkersFreeRoam.bin` holds no route and one grid. In `ROUTESL4RA` — the only region
/// whose copy has any content at all, the other seven being 16-byte shells — it holds 32 markers
/// in 25 groups, every one of them `track 4000`: **one full eight-car grid** and 24 lone points.
/// This returns the grid, through the same derivation [`start_grid`] uses, so a free-roam spawn
/// and a race spawn are the same code and fail the same way if the front/back reading is wrong.
///
/// **The grid's own height is 30 m above the ground and that is not a decoding error.** At its
/// pole, `(884, −1695)` in the engine's frame, the marker says `y = 53.5` and the only surface the
/// city offers is `23.8` — with the coarse tiers restored, and with all eight bundles loaded, still
/// only `23.8`. The height field is not in doubt: across the install's start markers, 140 of 141
/// race grids that have a road under them sit within **1 m** of it (median −0.0, p05 −0.5, p95
/// +0.3), and 22 of the 24 free-roam spots do too. This one grid floats, along with the lone marker
/// sharing its position, and the reason is not known.
///
/// So the caller must put the car down rather than trust the number — which `nfs_cruise` already
/// does for every spawn, and the car lands on four wheels and drives. The grid is still worth
/// having over a spot because it carries a *heading*, and a spawn without one faces a wall.
#[must_use]
pub fn free_roam_start(markers: &[StartMarker]) -> Option<(Vec3, Vec3)> {
    start_grid(markers, FREE_ROAM_TRACK)
}

/// The lone free-roam markers, nearest-first from a point — places in the city the file names.
///
/// The 24 are not a grid and are not a route: each is its own group, each holds one slot, and they
/// are spread over the whole map (x −1929..1698, y −1885..1167 in the file's frame). Their group
/// ids give the rest away — `540257916`/`540257917`, `931508017`/`931508018`,
/// `1608732748`/`1608732749`, `3585301327`/`3585301328` are consecutive, and consecutive is what
/// [`gizmo_nfs::hash`]'s `h * 33 + byte` produces for two names differing in their last character.
/// So the id is a name hash and these are *named* places — shops, safe houses, the things a free
/// roam has — not anonymous spawn points.
///
/// Which name is which is not answered here; recovering it means hashing candidates against these
/// ids, the same way a truncated part name is recovered. What is offered is the position, which is
/// enough to stand somewhere the game itself considers a place.
///
/// **They stand on the city.** Asked what surface is under each, 22 of the 24 answer within 1 m of
/// the marker's own height — which is the check that says both this reading and `remap` are right,
/// because 24 numbers do not land on a city's surface by accident. The two that do not are 29.7 m
/// and 180 m up; the first is the one sharing the free-roam grid's position (see
/// [`free_roam_start`]), and the second is alone.
///
/// The order is the group id's, so an index means the same place from one run to the next.
#[must_use]
pub fn free_roam_spots(markers: &[StartMarker]) -> Vec<Vec3> {
    let mut by: std::collections::BTreeMap<u32, Vec<&StartMarker>> =
        std::collections::BTreeMap::new();
    for m in markers.iter().filter(|m| m.track == u32::from(FREE_ROAM_TRACK)) {
        by.entry(m.group).or_default().push(m);
    }
    by.into_values().filter(|g| g.len() == 1).map(|g| remap(g[0].at)).collect()
}

/// Where a race starts when no grid is available: the point of least progress, and the way it faces.
///
/// The file's `progress` is the race's own measure, so its minimum is the start line — there is
/// nothing else in the data that names one. The facing comes from the next point of the same path,
/// because a start position without a direction puts the car on the grid pointing at a wall.
///
/// `None` when no path has two points to take a direction from.
#[must_use]
pub fn start_of(paths: &[RoutePath]) -> Option<(Vec3, Vec3)> {
    paths
        .iter()
        .filter(|p| p.points.len() >= 2)
        .filter_map(|p| {
            let (i, _) = p
                .progress
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.total_cmp(b.1))?;
            // The direction is taken forward, or backward at the very end of a path — a path's
            // last point still has a direction, it is just the one it arrived on.
            let (a, b) = if i + 1 < p.points.len() {
                (p.points[i], p.points[i + 1])
            } else {
                (p.points[i - 1], p.points[i])
            };
            let dir = (b - a).normalize_or_zero();
            (dir != Vec3::ZERO).then_some((p.progress[i], p.points[i], dir))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, at, dir)| (at, dir))
}

/// Gates along a race, in the file's own distance, and which one is next.
///
/// A route file is a **network** rather than a directed course — its cross-path links go forward
/// and back about equally, and its paths do not tile the progress axis (`Paths4001` covers 6,820 of
/// its 7,600 units, `Paths4021` 2,160 of 6,084). So a checkpoint cannot be a place. It can be a
/// *distance*: whichever path the car is on, its `progress` says how far round it is, and a gate is
/// a value of that.
#[derive(Debug, Clone)]
pub struct Checkpoints {
    gates: Vec<f32>,
    next: usize,
}

impl Checkpoints {
    /// `count` gates spread evenly over the progress the paths actually cover, the last of them at
    /// the finish. The start is not a gate — you are already there.
    #[must_use]
    pub fn along(paths: &[RoutePath], count: usize) -> Self {
        let all: Vec<f32> = paths.iter().flat_map(|p| p.progress.iter().copied()).collect();
        let lo = all.iter().copied().fold(f32::MAX, f32::min);
        let hi = all.iter().copied().fold(f32::MIN, f32::max);
        let gates = if all.is_empty() || count == 0 || hi <= lo {
            Vec::new()
        } else {
            (1..=count).map(|i| lo + (hi - lo) * i as f32 / count as f32).collect()
        };
        Checkpoints { gates, next: 0 }
    }

    /// How many gates have been passed, and how many there are.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.next
    }

    #[must_use]
    pub fn total(&self) -> usize {
        self.gates.len()
    }

    /// The distance the next gate sits at, or `None` when the race is finished.
    #[must_use]
    pub fn next_gate(&self) -> Option<f32> {
        self.gates.get(self.next).copied()
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        !self.gates.is_empty() && self.next >= self.gates.len()
    }

    /// Offer a fix; returns whether a gate was just passed.
    ///
    /// **Only counts while the car is on the course**, and only ever one gate at a time. Both
    /// matter: a fix taken from off the network can land anywhere on the progress axis, and a
    /// single frame that jumped three gates is a car that was put back by the fall guard rather
    /// than one that drove them.
    pub fn offer(&mut self, fix: Option<Fix>, half_width: f32) -> bool {
        let Some(f) = fix else { return false };
        if f.distance > half_width {
            return false;
        }
        let Some(gate) = self.next_gate() else { return false };
        if f.progress >= gate {
            self.next += 1;
            return true;
        }
        false
    }

    /// Back to the start line.
    pub fn reset(&mut self) {
        self.next = 0;
    }
}

/// A route's paths as a flat ribbon: one quad per segment, `width` across and `lift` above the
/// surface the path was placed on.
///
/// Quads per segment rather than a mitred strip. A mitre needs the turn angle and gets ugly at the
/// hairpins this city has; two triangles per segment overlap slightly on a corner and that is
/// invisible on a ribbon lying on tarmac.
///
/// **`lift` is a viewing decision, not a placement one.** Framing the whole city from a kilometre
/// up puts half a metre inside the depth buffer's noise, and the ribbon is drawn correctly and
/// vanishes into the road; from a chase camera half a metre is right and three metres floats.
#[must_use]
pub fn ribbon(
    paths: &[RoutePath],
    width: f32,
    lift: f32,
) -> Vec<gizmo::renderer::gpu_types::Vertex> {
    use gizmo::renderer::gpu_types::Vertex;
    let mut out = Vec::new();
    for path in paths {
        for pair in path.points.windows(2) {
            let (a, b) = (pair[0] + Vec3::Y * lift, pair[1] + Vec3::Y * lift);
            let along = (b - a).normalize_or_zero();
            if along == Vec3::ZERO {
                continue;
            }
            let side = Vec3::new(-along.z, 0.0, along.x) * (width * 0.5);
            let quad = [a - side, a + side, b + side, b - side];
            let v = |p: Vec3| Vertex {
                position: [p.x, p.y, p.z],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [0.0, 0.0],
                ..Default::default()
            };
            for i in [0usize, 1, 2, 0, 2, 3] {
                out.push(v(quad[i]));
            }
        }
    }
    out
}

/// How far a point is from the road network, and how far along it is.
///
/// The answer to both questions a race needs — "am I still on the course" and "where am I on it" —
/// and they are one lookup because the route's own `progress` travels with the geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fix {
    /// Distance from the point to the nearest path, in plan view. Height is deliberately not in it:
    /// a car on a bridge belongs to the bridge's path, and the road under it is not "close".
    pub distance: f32,
    /// The file's own cumulative distance at the nearest point, interpolated along the segment.
    pub progress: f32,
    /// Which path of the file the nearest segment belongs to.
    pub path: u16,
    /// The nearest point itself, in plan view — `at.y` is the asker's own height, untouched.
    ///
    /// Carried because "how far off the course is this" and "where would it be *on* it" are the
    /// same query, and the second has no cheaper answer: repeating `locate`'s grid walk outside
    /// would cost the same and could disagree with it about which segment won.
    pub at: Vec3,
}

/// The road network of one route file, as something to ask questions of.
///
/// **This is where a barrier has to come from.** A chunk census over `TRACKS/L4R*.BUN`,
/// `GLOBAL/InGame*.bun` and every route file finds no `0x0003410B` anywhere, so the install ships
/// none. The one candidate that looked like it might stand in — the `0x0003414A` regions, which are
/// large, lie on the road and carry a heading — was measured and ruled out: its three road-covering
/// kinds contain only **33 %** of the route nodes, and 12 % of the nodes are inside no region of any
/// kind. What is left is the paths themselves, which *are* the drivable network of that race, so
/// "off the course" is "far from every path".
///
/// Indexed on a grid because the question is asked once a frame.
pub struct Corridor {
    /// `(a, b, progress at a, progress at b, path)` in world space.
    segments: Vec<(Vec3, Vec3, f32, f32, u16)>,
    cells: std::collections::HashMap<(i32, i32), Vec<u32>>,
    half_width: f32,
}

/// How far off the nearest path still counts as on the course.
///
/// Measured rather than chosen: walking sideways from every path point until the road stops
/// answering at that level, Bayview's carriageways reach out a median of 9-11 m across three route
/// files. Twelve sits above that and well under the 60 m a junction opens out to.
///
/// Lives here rather than in a binary because it is a fact about the city, and two callers now need
/// to agree on it — the game, which uses it to tell "off the course" from "out of the world", and
/// the sim, which uses it to ask whether a car that fell had left the course first.
pub const COURSE_HALF_WIDTH: f32 = 12.0;

/// Grid pitch for [`Corridor`]. Nodes of a path are 29 m apart at the median, so a cell this size
/// holds a handful of segments and a query touches nine of them.
pub const CORRIDOR_CELL: f32 = 64.0;

impl Corridor {
    /// Build from paths already standing on the city, with the half-width that counts as "on it".
    #[must_use]
    pub fn of(paths: &[RoutePath], half_width: f32) -> Self {
        let mut segments = Vec::new();
        for p in paths {
            for i in 0..p.points.len().saturating_sub(1) {
                segments.push((
                    p.points[i],
                    p.points[i + 1],
                    p.progress.get(i).copied().unwrap_or(0.0),
                    p.progress.get(i + 1).copied().unwrap_or(0.0),
                    p.index,
                ));
            }
        }
        let key = |v: f32| (v / CORRIDOR_CELL).floor() as i32;
        let pad = (half_width / CORRIDOR_CELL).ceil() as i32;
        let mut cells: std::collections::HashMap<(i32, i32), Vec<u32>> = Default::default();
        for (i, (a, b, ..)) in segments.iter().enumerate() {
            // By footprint plus a margin, so a query up to `half_width` off the side of a segment
            // still lands in a cell that lists it.
            for gx in key(a.x.min(b.x)) - pad..=key(a.x.max(b.x)) + pad {
                for gz in key(a.z.min(b.z)) - pad..=key(a.z.max(b.z)) + pad {
                    cells.entry((gx, gz)).or_default().push(i as u32);
                }
            }
        }
        Corridor { segments, cells, half_width }
    }

    /// The half-width this corridor was built with.
    #[must_use]
    pub fn half_width(&self) -> f32 {
        self.half_width
    }

    /// Segments held.
    #[must_use]
    pub fn segments(&self) -> usize {
        self.segments.len()
    }

    /// Where a point is relative to the network, or `None` if the grid lists nothing near it.
    ///
    /// `None` is not "far away", it is "this race's network does not reach here" — which for a car
    /// that has left the map is the honest answer, and is why this is not an `f32`.
    #[must_use]
    pub fn locate(&self, at: Vec3) -> Option<Fix> {
        let key = |v: f32| (v / CORRIDOR_CELL).floor() as i32;
        let (gx, gz) = (key(at.x), key(at.z));
        let mut best: Option<Fix> = None;
        for dx in -1..=1 {
            for dz in -1..=1 {
                for i in self.cells.get(&(gx + dx, gz + dz)).into_iter().flatten() {
                    let (a, b, pa, pb, path) = self.segments[*i as usize];
                    let (d, t) = point_to_segment(at, a, b);
                    if best.is_none_or(|f| d < f.distance) {
                        let on = a + (b - a) * t;
                        best = Some(Fix {
                            distance: d,
                            progress: pa + (pb - pa) * t,
                            path,
                            at: Vec3::new(on.x, at.y, on.z),
                        });
                    }
                }
            }
        }
        best
    }

    /// Whether a point is on the course.
    #[must_use]
    pub fn contains(&self, at: Vec3) -> bool {
        self.locate(at).is_some_and(|f| f.distance <= self.half_width)
    }
}

/// Plan-view distance from a point to a segment, and how far along it the closest point lies.
fn point_to_segment(p: Vec3, a: Vec3, b: Vec3) -> (f32, f32) {
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let len2 = dx * dx + dz * dz;
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.z - a.z) * dz) / len2).clamp(0.0, 1.0)
    };
    let (cx, cz) = (a.x + dx * t, a.z + dz * t);
    ((p.x - cx).hypot(p.z - cz), t)
}

/// The same choice, made over the whole **graph** instead of one path at a time.
///
/// [`follow`] is a Viterbi pass along a chain, and a path is a chain — which is exactly why it
/// cannot see the thing that goes wrong at a junction. Heights solved one path at a time leave
/// each path internally smooth and say nothing about the two sides of a path boundary, so two
/// carriageways that meet can be solved onto **different decks** of the same interchange. Measured
/// over the eight sweep routes when this was written: cars ended up as much as 22.5 m above the
/// node their own pilot was holding, on seven routes of eight, and for 70.6 % of the race on the
/// worst — and in 88 % of those steps the node's own XZ *had* a surface at the car's height. The
/// surface is there. The choice is wrong.
///
/// Solving over the graph halved the worst of that and left the rest. What removed it was not this
/// function at all but the ground it is asked about — see [`Network::of`](super::Network::of): over
/// every drivable surface, least-climb is degenerate, and the field's deck disagreement fell from
/// 18.6 % of steps to 7.9 % the day the road filter reached this solve.
///
/// So the same minimum-climb rule is applied over a spanning tree of the graph: a leaf-to-root DP
/// that is exact on a tree exactly as [`follow`] is exact on a chain. Links that close a cycle are
/// not constrained — capturing those needs loopy belief propagation and the tree already ties
/// every path to its neighbours, which is the part that was missing.
///
/// **A node with no candidates is a hole in the road, and holes are bridged.** The walk carries the
/// nearest live ancestor through them, so two nodes with a hole between them are still parent and
/// child in the DP tree. That is not tidiness: with the road-filtered ground this is solved over,
/// the holes are the nodes the city has no road under, they sit at path boundaries, and cutting
/// there would fragment the graph into pieces that each pick a deck on their own — which is the
/// per-path failure this function exists to fix, arriving by a different door.
///
/// Returns the heights and **how many trees the graph fell into**, because one anchor, one prior or
/// one tie-break reaches exactly one of them and a caller that does not know the count is guessing
/// about its own reach.
pub(crate) fn follow_graph(
    candidates: &[Vec<f32>],
    links: &[Vec<u32>],
) -> (Vec<Option<f32>>, usize) {
    let n = candidates.len();
    let mut out = vec![None; n];
    let live = |i: usize| candidates.get(i).is_some_and(|c| !c.is_empty());

    let mut seen = vec![false; n];
    let mut trees = 0usize;
    for root in 0..n {
        if seen[root] || !live(root) {
            continue;
        }
        trees += 1;
        // Breadth-first, so the tree is shallow and the DP below is a single reverse pass.
        //
        // **A node the city could not answer for is bridged, not cut.** It joins its neighbours
        // rather than separating them: the walk carries the nearest live ancestor through it, so
        // two nodes with a hole between them end up parent and child in the DP tree and the hole
        // decides nothing — the same property [`follow`] gets from filtering its chain down to the
        // live nodes. Traversing only live nodes instead makes every hole a cut vertex, and with
        // road-filtered ground the holes are real: they fall exactly on the path boundaries, which
        // is where the whole point of solving over the graph is to hold two paths together.
        let mut order = vec![root];
        let mut parent = vec![usize::MAX; n];
        seen[root] = true;
        // `(node, the nearest live node at or above it)` — for a live node that is itself.
        let mut queue = std::collections::VecDeque::from([(root, root)]);
        while let Some((i, up)) = queue.pop_front() {
            for &l in links.get(i).into_iter().flatten() {
                let j = l as usize;
                if j >= n || seen[j] {
                    continue;
                }
                seen[j] = true;
                if live(j) {
                    parent[j] = up;
                    order.push(j);
                    queue.push_back((j, j));
                } else {
                    queue.push_back((j, up));
                }
            }
        }

        // `cost[i][j]` is the cheapest total climb in `i`'s subtree given `i` takes candidate `j`;
        // `pick[i][j]` records, per child, which of its candidates that answer chose.
        let mut cost: Vec<Vec<f32>> = order.iter().map(|&i| vec![0.0; candidates[i].len()]).collect();
        let mut pick: std::collections::HashMap<(usize, usize), Vec<usize>> =
            std::collections::HashMap::new();
        let slot: std::collections::HashMap<usize, usize> =
            order.iter().enumerate().map(|(s, &i)| (i, s)).collect();

        for s in (1..order.len()).rev() {
            let i = order[s];
            let p = parent[i];
            let ps = slot[&p];
            let mut chosen = vec![0usize; candidates[p].len()];
            let mut add = vec![0.0f32; candidates[p].len()];
            for (a, g) in candidates[p].iter().enumerate() {
                let mut best = f32::MAX;
                for (b, h) in candidates[i].iter().enumerate() {
                    let c = cost[s][b] + (h - g).abs();
                    if c < best {
                        best = c;
                        chosen[a] = b;
                    }
                }
                add[a] = if best.is_finite() { best } else { 0.0 };
            }
            for (a, v) in add.iter().enumerate() {
                cost[ps][a] += v;
            }
            pick.insert((p, i), chosen);
        }

        // Cheapest root, ties to the lower surface (candidate lists are sorted), then walk down.
        let rs = slot[&root];
        let mut take = vec![usize::MAX; n];
        take[root] = cost[rs]
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(j, _)| j);
        for &i in order.iter().skip(1) {
            let p = parent[i];
            take[i] = pick.get(&(p, i)).and_then(|c| c.get(take[p]).copied()).unwrap_or(0);
        }
        for &i in &order {
            out[i] = candidates[i].get(take[i]).copied();
        }
    }
    (out, trees)
}

/// Choose one surface per node: the sequence that climbs least in total.
///
/// This is the whole of "which surface", and it replaces every rule that tried to pick per node.
/// Bayview stacks — every node of `Paths4001` has **at least two** drivable surfaces under it, 179
/// of its 341 have four or more, and the median distance from the top one to the bottom one is
/// **79 m**. Taking the topmost lands on roofs, taking the lowest lands under the road, and any
/// seed at all is a guess about a stack that deep.
///
/// A road does not need a seed to be recognised. It is the surface that is *there at every node*
/// and *at nearly the same height each time*, because that is what a road is; a roof exists only
/// over its building and a deck only over its span. So the surface to take is the one that makes
/// the whole path climb as little as it can — minimise the sum of `|Δh|` over the path, which is a
/// shortest path through the candidate lists and costs nothing at four candidates and ten nodes.
///
/// Nodes the city could not answer for are left `None` here and filled by [`fill`]; they carry no
/// cost, so a hole does not decide anything for its neighbours.
pub(crate) fn follow(candidates: &[Vec<f32>]) -> Vec<Option<f32>> {
    let live: Vec<usize> = (0..candidates.len()).filter(|i| !candidates[*i].is_empty()).collect();
    let mut out = vec![None; candidates.len()];
    let Some((&first, rest)) = live.split_first() else { return out };

    // cost[j] is the cheapest way to arrive at candidate j of the node being considered, and
    // back[step][j] which candidate of the previous live node that came from.
    let mut cost: Vec<f32> = vec![0.0; candidates[first].len()];
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(rest.len());
    let mut prev = first;
    for &i in rest {
        let (here, there) = (&candidates[i], &candidates[prev]);
        let mut next = vec![f32::MAX; here.len()];
        let mut from = vec![0usize; here.len()];
        for (j, h) in here.iter().enumerate() {
            for (k, g) in there.iter().enumerate() {
                let c = cost[k] + (h - g).abs();
                if c < next[j] {
                    next[j] = c;
                    from[j] = k;
                }
            }
        }
        cost = next;
        back.push(from);
        prev = i;
    }

    // Cheapest end, ties going to the lower surface — candidate lists are sorted, so the first
    // minimum is already the lowest one.
    let mut j = cost
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(j, _)| j);
    let mut walk = live.clone();
    out[*walk.last().unwrap_or(&first)] = candidates[*walk.last().unwrap_or(&first)].get(j).copied();
    while let Some(step) = back.pop() {
        j = step[j];
        walk.pop();
        if let Some(&i) = walk.last() {
            out[i] = candidates[i].get(j).copied();
        }
    }
    out
}

/// Give every gap a height from the nearest known neighbours on each side, and return how many
/// were filled.
///
/// Linear between two known values, held flat past either end. A gap is the city not covering a
/// point, not the road vanishing, so the honest fill is the one that keeps the path continuous —
/// and it is separated out from [`build`] because it is the part worth testing without a city.
///
/// All-unknown stays all-unknown: inventing a height for a path the city knows nothing about would
/// put a race line at `y = 0` and let it look like it had been placed.
pub(crate) fn fill(heights: &mut [Option<f32>]) -> usize {
    let known: Vec<usize> = (0..heights.len()).filter(|i| heights[*i].is_some()).collect();
    if known.is_empty() || known.len() == heights.len() {
        return 0;
    }
    let (first, last) = (known[0], known[known.len() - 1]);
    for i in 0..heights.len() {
        if heights[i].is_some() {
            continue;
        }
        heights[i] = Some(if i < first {
            heights[first].unwrap_or_default()
        } else if i > last {
            heights[last].unwrap_or_default()
        } else {
            // Between two known points: the position along the gap, in node counts. Node spacing is
            // even enough (median 29 m) that weighting by index rather than by distance is not
            // worth the extra state.
            let lo = known.iter().rev().find(|k| **k < i).copied().unwrap_or(first);
            let hi = known.iter().find(|k| **k > i).copied().unwrap_or(last);
            let (a, b) = (heights[lo].unwrap_or_default(), heights[hi].unwrap_or_default());
            let t = (i - lo) as f32 / (hi - lo) as f32;
            a + (b - a) * t
        });
    }
    heights.len() - known.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{CityCollider, Surface};
    use gizmo_nfs::world::routes::read_nodes;

    fn node(path: u16, x: f32, y: f32, progress: f32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&x.to_le_bytes());
        v.extend_from_slice(&y.to_le_bytes());
        v.extend_from_slice(&path.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&[0xFF; 6]); // three absent links
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&progress.to_le_bytes());
        v
    }

    /// A flat drivable slab at `y`, covering a generous square about the origin.
    fn slab(y: f32, half: f32) -> CityCollider {
        CityCollider {
            cell: (0, 0),
            origin: Vec3::ZERO,
            vertices: vec![
                Vec3::new(-half, y, -half),
                Vec3::new(half, y, -half),
                Vec3::new(half, y, half),
                Vec3::new(-half, y, half),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            surfaces: vec![Surface::Drivable; 2],
        }
    }

    #[test]
    fn a_path_lands_on_the_surface_under_it() {
        let mut bytes = node(0, 0.0, 0.0, 0.0);
        bytes.extend(node(0, -30.0, 0.0, 30.0));
        let nodes = read_nodes(&bytes).unwrap();
        let ground = Ground::of(&[slab(12.5, 400.0)]);
        let built = build(&nodes, &ground);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].index, 0);
        assert_eq!(built[0].filled, 0, "the slab answers for both");
        assert!(built[0].points.iter().all(|p| (p.y - 12.5).abs() < 1e-3));
        assert_eq!(built[0].progress, vec![0.0, 30.0]);
    }

    /// The frame conversion is the city's own, so a route point and a city vertex agree. Getting it
    /// wrong puts the race line somewhere plausible and wrong, which is the failure that costs a day.
    #[test]
    fn a_node_reaches_the_gizmo_frame_the_way_the_city_does() {
        let nodes = read_nodes(&node(0, 100.0, 200.0, 0.0)).unwrap();
        let built = build(&nodes, &Ground::of(&[slab(0.0, 1000.0)]));
        let p = built[0].points[0];
        assert!((p.x - -200.0).abs() < 1e-3, "world Y becomes −X");
        assert!((p.z - -100.0).abs() < 1e-3, "world X becomes −Z");
    }

    /// Two paths in one table stay two polylines. Welding them would draw a race line that
    /// teleports across the city between the end of one and the start of the next.
    #[test]
    fn each_path_becomes_its_own_polyline() {
        let mut bytes = node(0, 0.0, 0.0, 0.0);
        bytes.extend(node(0, -30.0, 0.0, 30.0));
        bytes.extend(node(1, 0.0, 200.0, 0.0));
        let built = build(&read_nodes(&bytes).unwrap(), &Ground::of(&[slab(0.0, 1000.0)]));
        assert_eq!(built.len(), 2);
        assert_eq!((built[0].points.len(), built[1].points.len()), (2, 1));
        assert_eq!(built[1].index, 1);
    }

    /// The names are the ones in the install. A category built on a substring is only as good as
    /// the substring, so it is asserted against real road names and real non-road ones.
    #[test]
    fn the_road_category_matches_what_the_city_calls_a_road() {
        for name in [
            "TRN_ROADA_CHOP_D1_R6", "TRN_CS_ROADA_CHOP_N18_R1", "TRN_CN_ROADA_CHOP_O12_R0",
            "RDP_ROADSIDE_NL_AA_KT",
        ] {
            assert!(is_road(name), "{name} is road");
        }
        for name in [
            "TRN_TERRAINA_CHOP_D1_32", "XB_LANDMARKTOWER_1A_RB_00", "TRN_CN_TUNNELA_NRND_CHOP_O1",
            "XO_PARKBENCHA_1B_00",
        ] {
            assert!(!is_road(name), "{name} is not road");
        }
    }

    /// Least-climb over a stack: the deck and the road are both there, and only one of them is
    /// continuous with the rest of the path.
    #[test]
    fn the_least_climbing_sequence_is_the_one_taken() {
        // Node 0 sees only the road, node 1 sees road and deck, node 2 only the road.
        let picked = follow(&[vec![10.0], vec![10.5, 40.0], vec![11.0]]);
        assert_eq!(picked, vec![Some(10.0), Some(10.5), Some(11.0)]);
        // And it is not simply "lowest": where the high surface is the continuous one, it wins.
        // (10 → 11 → 41 costs 31 and 39 → 40 → 41 costs 2.)
        let climbed = follow(&[vec![10.0, 39.0], vec![11.0, 40.0], vec![41.0]]);
        assert_eq!(climbed, vec![Some(39.0), Some(40.0), Some(41.0)]);
    }

    /// A node the city cannot answer for costs nothing and decides nothing for its neighbours.
    #[test]
    fn a_node_with_no_surface_does_not_steer_the_others() {
        let picked = follow(&[vec![10.0], Vec::new(), vec![11.0, 60.0]]);
        assert_eq!(picked, vec![Some(10.0), None, Some(11.0)]);
    }

    /// A chain, solved over the graph, gives what the chain solve gives. The tree DP is a
    /// generalisation and not a replacement, so the case they share has to agree or one of them is
    /// wrong.
    #[test]
    fn the_graph_solve_agrees_with_the_chain_where_the_graph_is_one() {
        let candidates = vec![vec![10.0, 39.0], vec![11.0, 40.0], vec![41.0]];
        let links = vec![vec![1], vec![0, 2], vec![1]];
        let (picked, trees) = follow_graph(&candidates, &links);
        assert_eq!(picked, follow(&candidates));
        assert_eq!(picked, vec![Some(39.0), Some(40.0), Some(41.0)]);
        assert_eq!(trees, 1);
    }

    /// **A node the city has no road under joins its neighbours; it does not cut them apart.**
    ///
    /// Four nodes in a line. The first stands where the only road is the upper deck, the second has
    /// no road at all, and the last two are under an interchange and could be on either. Bridged,
    /// the upper deck carries across the hole and all three solved nodes stay on it. Cut — which is
    /// what traversing live nodes only does — the far pair becomes its own tree, picks its own
    /// cheapest assignment, and the tie there goes to the lower surface: `[40, _, 10, 10]`, two
    /// decks in one road.
    ///
    /// Not a hypothetical. Holes appear the moment heights are solved over the road-filtered ground
    /// rather than over every drivable surface, and they land at path boundaries.
    #[test]
    fn a_hole_joins_its_neighbours_rather_than_cutting_them() {
        let candidates =
            vec![vec![40.0], Vec::new(), vec![10.0, 40.0], vec![10.0, 40.5]];
        let links = vec![vec![1], vec![0, 2], vec![1, 3], vec![2]];
        let (picked, trees) = follow_graph(&candidates, &links);
        assert_eq!(picked, vec![Some(40.0), None, Some(40.0), Some(40.5)]);
        assert_eq!(trees, 1, "a hole is not a component boundary");
    }

    /// Two pieces nothing joins are two pieces, and the count is reported rather than assumed —
    /// a rule that acts at one node reaches one tree and no further.
    #[test]
    fn pieces_nothing_joins_are_counted() {
        let candidates = vec![vec![10.0], vec![11.0], vec![70.0], vec![71.0]];
        let links = vec![vec![1], vec![0], vec![3], vec![2]];
        let (picked, trees) = follow_graph(&candidates, &links);
        assert_eq!(picked, vec![Some(10.0), Some(11.0), Some(70.0), Some(71.0)]);
        assert_eq!(trees, 2);
    }

    /// **Where two decks are both flat, least-climb cannot choose and the tie goes to the lower
    /// one.** Written down because it is the failure everything else in this module is about:
    /// `heights_at` returns its candidates lowest first and `min_by` keeps the first minimum, so a
    /// road on a flyover over a road solves onto the road underneath at no cost at all.
    ///
    /// The way out is not a tie-break. It is that on the road-filtered ground the two decks are
    /// rarely both present — 259 of `Paths4001`'s 341 nodes have exactly one candidate — so the
    /// tie mostly stops being reachable. Anchoring the solve at the starting grid was the other
    /// candidate and it is refuted: over the eight sweep routes the grid's own node already carries
    /// the height the grid stands on, to within 1.4 m, so the anchor has nothing to correct.
    #[test]
    fn two_flat_decks_tie_and_the_lower_one_is_taken() {
        let candidates = vec![vec![0.0, 10.0]; 3];
        let links = vec![vec![1], vec![0, 2], vec![1]];
        let (picked, _) = follow_graph(&candidates, &links);
        assert_eq!(picked, vec![Some(0.0); 3]);
    }

    fn straight_path(index: u16) -> RoutePath {
        RoutePath {
            index,
            points: vec![Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, 5.0, -100.0)],
            progress: vec![200.0, 300.0],
            filled: 0,
        }
    }

    /// The two questions a race asks, from one lookup.
    #[test]
    fn a_point_on_the_path_is_on_the_course_and_says_how_far_along() {
        let c = Corridor::of(&[straight_path(7)], 8.0);
        let f = c.locate(Vec3::new(0.0, 5.0, -50.0)).expect("a segment is indexed here");
        assert!(f.distance < 1e-4);
        assert!((f.progress - 250.0).abs() < 0.01, "halfway is halfway along the progress too");
        assert_eq!(f.path, 7);
        assert!(c.contains(Vec3::new(0.0, 5.0, -50.0)));
    }

    /// Width is what makes it a corridor rather than a line, and height is deliberately not in it:
    /// a car on a bridge belongs to the bridge's path, not to the road forty metres below.
    #[test]
    fn the_edge_of_the_corridor_is_the_half_width_and_height_is_not_in_it() {
        let c = Corridor::of(&[straight_path(0)], 8.0);
        assert!(c.contains(Vec3::new(7.5, 5.0, -50.0)));
        assert!(!c.contains(Vec3::new(8.5, 5.0, -50.0)));
        assert!(c.contains(Vec3::new(0.0, 45.0, -50.0)), "forty metres up is still over the path");
    }

    /// Past the end of a path the nearest point is its endpoint, not the infinite line.
    #[test]
    fn beyond_the_end_of_a_path_the_distance_is_to_its_end() {
        let c = Corridor::of(&[straight_path(0)], 8.0);
        let f = c.locate(Vec3::new(0.0, 5.0, -130.0)).expect("the margin indexes the end cell");
        assert!((f.distance - 30.0).abs() < 0.01);
        assert!((f.progress - 300.0).abs() < 0.01, "progress clamps to the path's end");
    }

    /// Off the network entirely is `None`, not a large number — the race's own roads do not reach
    /// there, and saying "1,400 m away" would invite someone to compare it with a half-width.
    #[test]
    fn a_point_the_network_does_not_reach_has_no_fix() {
        let c = Corridor::of(&[straight_path(0)], 8.0);
        assert!(c.locate(Vec3::new(5000.0, 5.0, 5000.0)).is_none());
        assert!(!c.contains(Vec3::new(5000.0, 5.0, 5000.0)));
    }

    #[test]
    fn a_gap_is_interpolated_and_the_ends_are_held() {
        let mut h = [None, Some(10.0), None, Some(14.0), None];
        assert_eq!(fill(&mut h), 3);
        assert_eq!(h[0], Some(10.0), "before the first known value, hold it");
        assert_eq!(h[2], Some(12.0), "halfway between 10 and 14");
        assert_eq!(h[4], Some(14.0), "past the last known value, hold it");
    }

    /// A path the city knows nothing about must not come back sitting at zero — that reads as a
    /// placed race line and is a road nobody found.
    #[test]
    fn all_unknown_stays_unknown() {
        let mut h = [None, None, None];
        assert_eq!(fill(&mut h), 0);
        assert!(h.iter().all(Option::is_none));
    }

    /// A deck over the whole path does not lift it. Taking the topmost surface is what put six real
    /// paths on rooftops, and this is that failure in miniature.
    #[test]
    fn a_path_under_a_deck_stays_on_the_road() {
        let mut bytes = node(0, 0.0, 0.0, 0.0);
        bytes.extend(node(0, -30.0, 0.0, 30.0));
        bytes.extend(node(0, -60.0, 0.0, 60.0));
        let nodes = read_nodes(&bytes).unwrap();
        let built = build(&nodes, &Ground::of(&[slab(0.0, 400.0), slab(20.0, 400.0)]));
        assert!(built[0].points.iter().all(|p| p.y.abs() < 1e-3), "the road, not the deck");
        assert!(built[0].climbed() < 0.001);
    }

    /// And a roof over *one* node does not pull the path up to it, because a median does not move
    /// for one sample and the follow then prefers what is nearest.
    #[test]
    fn a_roof_over_one_node_does_not_lift_the_path() {
        let mut bytes = node(0, 0.0, 0.0, 0.0);
        bytes.extend(node(0, -30.0, 0.0, 30.0));
        bytes.extend(node(0, -60.0, 0.0, 60.0));
        let nodes = read_nodes(&bytes).unwrap();
        // A small roof at y = 110, covering only the middle node's XZ.
        let mut roof = slab(110.0, 12.0);
        roof.origin = Vec3::new(0.0, 0.0, 30.0);
        let built = build(&nodes, &Ground::of(&[slab(0.0, 400.0), roof]));
        assert!(built[0].points.iter().all(|p| p.y.abs() < 1e-3), "still on the road");
    }

    /// A path that genuinely climbs is still allowed to: the rule is "nearest", not "flattest".
    #[test]
    fn a_path_that_climbs_a_ramp_follows_it() {
        let mut bytes = node(0, 0.0, 0.0, 0.0);
        bytes.extend(node(0, -30.0, 0.0, 30.0));
        let nodes = read_nodes(&bytes).unwrap();
        let mut upper = slab(6.0, 12.0);
        upper.origin = Vec3::new(0.0, 0.0, 30.0);
        let built = build(&nodes, &Ground::of(&[slab(0.0, 12.0), upper]));
        assert!((built[0].points[1].y - 6.0).abs() < 1e-3, "the only surface there is the ramp");
    }
}
