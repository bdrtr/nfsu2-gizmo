//! The route file's node table as a graph you can drive.
//!
//! [`route::build`](super::route::build) turns the table into polylines — one per path, standing on
//! the city — which is what a *drawn* race line needs. A driven one needs the other half: where the
//! paths meet, so a car that reaches the end of one can carry on down another.
//!
//! ## Why this exists rather than more decoding
//!
//! The file does not say which sequence of paths a race takes. Two readings were tried and both are
//! refuted with numbers, in `ROADMAP.md`: ordering nodes by `progress` (44 % of consecutive steps
//! over 100 m), and walking the junctions by "take the link that carries progress on" (one route
//! covers its whole progress span in a sixth of its length, another starts at 804 m rather than
//! zero — progress is not a lap coordinate across the network). A third field, `0x00034149`'s
//! `+36`, names a real successor and gets 43 of 105 routes over 95 %, which is a lead and not an
//! answer.
//!
//! So the sequence is taken from the one course description that *is* decoded: the event outline.
//! It is far too coarse to drive — 17 points over 6 km, median step 425 m — but **14 of its 17
//! corners have road under them**, so it is a list of waypoints in lap order. A driver that walks
//! this graph and, at every junction, takes the branch that gets it nearer the next waypoint is
//! driving the course the event describes, over roads the network says exist.
//!
//! That is a weaker claim than "this is the race line" and it is the claim the data supports.

use super::{remap, Ground};
use gizmo::prelude::*;
use gizmo_nfs::world::routes::RouteNode;

/// The steepest a link may be before it is a lift shaft rather than a road, as rise over run.
///
/// A very steep city street is about 25 %; San Francisco's worst is 31.5 %. Anything past that is
/// two decks that happen to be near each other in plan. See [`Network::drop_climbing`].
const MAX_GRADE: f32 = 0.0;

/// One place a car can be, and where it can go from there.
#[derive(Debug, Clone)]
pub struct Junction {
    /// Position in the Gizmo frame, standing on the city.
    pub at: Vec3,
    /// Which path of the file this node belongs to.
    pub path: u16,
    /// Every node reachable in one step: the neighbours along its own path and the junctions the
    /// record links to.
    pub links: Vec<u32>,
}

/// The whole table as a graph.
#[derive(Debug, Clone, Default)]
pub struct Network {
    nodes: Vec<Junction>,
    /// Links dropped because the road does not continue along them — see [`Network::drop_walled`].
    walled: usize,
    /// Links dropped because no road could climb them — see [`Network::drop_climbing`].
    climbed: usize,
    /// How many separate trees the height solve fell into — see [`route::follow_graph`].
    trees: usize,
    /// How many nodes took their height from a neighbour rather than from the city.
    filled: usize,
    /// Which nodes are on the **race's own line**, if anybody has said where that is.
    ///
    /// Empty until [`Network::mark_line`] runs, and then one flag per node. The graph knows about
    /// roads; it does not know which of them this race uses, and the difference is not academic —
    /// Bayview's route files link parallel carriageways to each other, so a step that is perfectly
    /// sensible as *road* can put a car 50 m from the race. See [`Network::step_avoiding`].
    near_line: Vec<bool>,
}

impl Network {
    /// Build the graph, standing every node on the city.
    ///
    /// **Two grounds, and they answer different questions.** `roads` is
    /// [`route::road_ground`](super::route::road_ground) — the city filtered to road objects — and
    /// it is what a node's height is chosen from, so a car following this and a ribbon drawn from
    /// [`route::build`](super::route::build) cannot disagree about where the road is. `ground` is
    /// every drivable surface, and it is what [`Network::drop_walled`] asks whether the ground
    /// continues along a link.
    ///
    /// **The split is not tidiness; handing the height solve the unfiltered ground is degenerate.**
    /// [`route::road_ground`](super::route::road_ground) says why in one sentence — over all
    /// drivable triangles the flat shelf beneath the city climbs by nothing at all and wins
    /// everywhere — and least-climb ([`route::follow_graph`]) is exactly the rule that shelf beats.
    /// This constructor was handed `Ground::of` from the day it was written and the comment here
    /// claimed the opposite. Swept over the eight routes: the field's deck disagreement — how often
    /// a car is more than 3 m above the node its own pilot is holding — falls from **18.6 % of
    /// steps to 7.9 %**, `Paths4081` from 63.6 % to 1.7 % and `Paths4001` from 33.7 % to 7.3 %,
    /// while links no road could climb go 11 to 6 and the driving does not move (1 449 → 1 451
    /// waypoints, 64 of 64 cars still taking junctions, no falls). `NFS_ROADHEIGHT=0` goes back to
    /// the unfiltered ground, which is the arm every number before 2026-08-21 was taken on.
    ///
    /// `drop_walled` deliberately keeps the unfiltered ground: asking it about roads alone cuts
    /// every link whose middle is a grass median, which is the wall filter this repo swept and
    /// threw out once already — see [`Network::drop_walled`].
    ///
    /// **Both kinds of edge, and both directions.** Along a path, file order is the sequence — and
    /// only the sequence: nearly half the install's paths are stored against the way they are
    /// driven, so an edge here is undirected and the direction is the driver's business. Across
    /// paths, the record's own `+12`/`+14`/`+16`, which are directed in the file and made mutual
    /// here for the same reason: only 29 % are mutual as stored, because a junction is written from
    /// one side.
    #[must_use]
    pub fn of(nodes: &[RouteNode], roads: &Ground, ground: &Ground) -> Self {
        // Heights the same way the drawn line gets them, and for the same reason. Asking
        // `height_at` from two metres up answers "the highest surface at or below two metres",
        // which on an elevated road is **nothing** — the node then falls back to zero and sits ten
        // metres under the tarmac. Six of seven rivals drove into the ground aiming at one.
        //
        // `route::follow_graph` is the fix already written: every candidate *road* surface at the
        // node's XZ, then the assignment over the whole graph that climbs least. Shared rather than
        // reimplemented — but a shared solver over a different candidate set is not a shared
        // answer, which is what the unfiltered ground cost this for nine days.
        let flat: Vec<Vec3> = nodes.iter().map(|n| remap([n.x, n.y, 0.0])).collect();
        let mut height: Vec<f32> = vec![0.0; nodes.len()];
        // **Ask the roads; where the city has no road, ask the city.**
        //
        // `roads` alone is what the least-climb rule needs, and on the eight sweep routes it is
        // also 4-44 nodes per route the road filter has nothing under at all. Those are not holes
        // in the race — the cars drive through them — so leaving them to `fill`'s interpolation
        // invents a height that is no surface, and `drop_walled` then cuts their links: on
        // `Paths4001` 86 walled links became 152 and 18 nodes were left with no way out, and the
        // field stopped moving. Falling back to the drivable ground at those nodes keeps the
        // degeneracy `road_ground` warns about local and bounded — the shelf can only win where
        // there is no road to beat it, and its road-solved neighbours still pull on it.
        //
        // `NFS_ROADHEIGHT=strict` refuses the fallback (roads or nothing), `NFS_ROADHEIGHT=0` goes
        // back to every drivable surface everywhere, which is what this did until 2026-08-21 and
        // what every number before that date was taken on.
        let how = std::env::var("NFS_ROADHEIGHT").unwrap_or_default();
        let candidates_at = |x: f32, z: f32| -> Vec<f32> {
            if how == "0" {
                return ground.heights_at(x, z);
            }
            let road = roads.heights_at(x, z);
            if road.is_empty() && how != "strict" { ground.heights_at(x, z) } else { road }
        };

        // **`NFS_DECK=0` goes back to solving one path at a time.** Per-path is right for a path
        // and blind to a junction: two carriageways that meet can be solved onto different decks
        // of the same interchange, and every test downstream is plan-view so nothing says so. When
        // that was measured over eight routes, cars drove as much as 22.5 m above the node their
        // own pilot held, on seven routes of eight. See [`route::follow_graph`] — and note that
        // most of what was left after the graph solve turned out to be the *ground* rather than
        // the solve, which is the fallback above.
        //
        // The adjacency is built here rather than below because the graph solve needs it, and it
        // is a property of the file: neighbours along a path, plus the record's own cross-links.
        let mut adj: Vec<Vec<u32>> = vec![Vec::new(); nodes.len()];
        let join = |a: usize, b: usize, adj: &mut Vec<Vec<u32>>| {
            if a != b && a < adj.len() && b < adj.len() {
                if !adj[a].contains(&(b as u32)) {
                    adj[a].push(b as u32);
                }
                if !adj[b].contains(&(a as u32)) {
                    adj[b].push(a as u32);
                }
            }
        };
        // **`NFS_DEDUP=<m>`: one junction, one edge.**
        //
        // A cross-path link is the file saying "these two paths meet". Measured over the install's
        // 19,409 present links: they fall into 8,063 distinct (path, target path) pairs, so **58.5 %
        // of the writes restate a junction another write already made**. The nearest member of each
        // pair sits a median **2.7 m** away — that is where the two roads physically touch — while
        // the restatements run a median 33.8 m and **40.5 % of them are over 40 m**. Building an
        // undirected edge per write, as this did, turns one junction into a fan of edges that
        // teleport a walker up to a hundred metres sideways.
        //
        // `Paths4121` shows it whole. Nodes 108, 109, 110 and 111 of path 2 all name node 294 of
        // path 1, at **3, 27, 65 and 118 m**. The junction is 108↔294; the other three are the same
        // sentence said again from further away — and the 118 m one is the arm that has been read
        // all day as "node 111's only forward arm leaves the race line".
        //
        // The filter keeps the shortest write of each pair **per cluster**, so two genuine meetings
        // of the same two paths survive: writes are grouped by where they land, and only a write
        // that has a shorter neighbour within `NFS_DEDUP` metres is dropped.
        //
        // **Refuted, and the reason corrects the reading above.** At 60 m it drops 122 writes on
        // `Paths4121` — including node 111's 118 m arm, exactly as intended — and the field loses:
        // 1 451 → 1 285 waypoints, and the cars that stay on course fall from 14 covering 18.7 each
        // to 11 covering **9.9**. At 30 m it is 1 290 with seven more cars whose progress stops.
        //
        // A restatement is redundant to a *reader* and not to a *driver*. The writes at 27, 65 and
        // 118 m are how a car already past the junction still gets onto the other path; deleting
        // them because node 108 has a 3 m write leaves a car at node 110 with no way across at all.
        // The fan is a set of on-ramps, not one sentence said four times.
        //
        // Off by default.
        let dedup: f32 =
            std::env::var("NFS_DEDUP").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let span = |a: usize, b: usize| (flat[a].x - flat[b].x).hypot(flat[a].z - flat[b].z);
        // Every cross-path write, as (writer, target), longest first so a drop test only has to
        // look at what it has already kept.
        let mut cross: Vec<(usize, usize)> = Vec::new();
        for i in 0..nodes.len() {
            for l in nodes[i].linked() {
                let j = l as usize;
                if j < nodes.len() && nodes[j].path != nodes[i].path {
                    cross.push((i, j));
                }
            }
        }
        cross.sort_by(|a, b| span(a.0, a.1).total_cmp(&span(b.0, b.1)));
        let mut kept: Vec<(usize, usize)> = Vec::new();
        let mut dropped = 0usize;
        for (i, j) in cross {
            let restates = dedup > 0.0
                && kept.iter().any(|(a, b)| {
                    (nodes[*a].path, nodes[*b].path) == (nodes[i].path, nodes[j].path)
                        && span(*a, i).min(span(*b, i)) <= dedup
                });
            if restates {
                dropped += 1;
            } else {
                kept.push((i, j));
            }
        }

        for i in 0..nodes.len() {
            if i + 1 < nodes.len() && nodes[i + 1].path == nodes[i].path {
                join(i, i + 1, &mut adj);
            }
            // Same-path links stay as written; only the junction writes are de-duplicated.
            for l in nodes[i].linked() {
                let j = l as usize;
                if j < nodes.len() && nodes[j].path == nodes[i].path {
                    join(i, j, &mut adj);
                }
            }
        }
        for (i, j) in &kept {
            join(*i, *j, &mut adj);
        }
        if dedup > 0.0 {
            eprintln!("kavşak tekrarları elendi: {dropped} bağlantı yazımı ({dedup:.0} m)");
        }

        // Where one path ends and the next begins, in file order. Both branches want it: the
        // per-path solve to slice its chain, and the graph solve to fill its gaps without
        // interpolating across a boundary.
        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut i = 0;
        while i < nodes.len() {
            let mut j = i;
            while j + 1 < nodes.len() && nodes[j + 1].path == nodes[i].path {
                j += 1;
            }
            spans.push((i, j));
            i = j + 1;
        }

        let (mut trees, mut filled) = (0usize, 0usize);
        if std::env::var("NFS_DECK").ok().is_none_or(|v| v != "0") {
            let candidates: Vec<Vec<f32>> =
                flat.iter().map(|p| candidates_at(p.x, p.z)).collect();
            let (mut solved, count) = super::route::follow_graph(&candidates, &adj);
            trees = count;
            // **Filled a path at a time, not over the whole table.** `fill` interpolates by node
            // index and holds flat past either end, which is honest inside a path — median spacing
            // 29 m — and meaningless across a boundary, where the neighbour is a different road
            // somewhere else in the city. The per-path branch below always did this; the graph
            // branch handed `fill` the whole array and lerped between two unrelated paths.
            for &(a, b) in &spans {
                filled += super::route::fill(&mut solved[a..=b]);
            }
            for (k, h) in solved.iter().enumerate() {
                height[k] = h.unwrap_or(0.0);
            }
        } else {
            for &(a, b) in &spans {
                trees += 1;
                let candidates: Vec<Vec<f32>> =
                    (a..=b).map(|k| candidates_at(flat[k].x, flat[k].z)).collect();
                let mut solved = super::route::follow(&candidates);
                filled += super::route::fill(&mut solved);
                for (k, h) in (a..=b).zip(&solved) {
                    height[k] = h.unwrap_or(0.0);
                }
            }
        }

        let mut out: Vec<Junction> = nodes
            .iter()
            .enumerate()
            .map(|(k, n)| Junction {
                at: Vec3::new(flat[k].x, height[k], flat[k].z),
                path: n.path,
                links: Vec::new(),
            })
            .collect();

        for (j, links) in out.iter_mut().zip(adj) {
            j.links = links;
        }
        let mut me =
            Self { nodes: out, walled: 0, climbed: 0, trees, filled, near_line: Vec::new() };
        me.drop_climbing();
        // `NFS_WALLED=0` leaves the link filter off. Not a setting — the one way to ask whether a
        // change to the height solve moved the driving or merely moved what this filter cuts,
        // which is a real confusion: the filter's own chord endpoints are the heights being
        // changed.
        if std::env::var("NFS_WALLED").ok().is_none_or(|v| v != "0") {
            me.drop_walled(ground);
        }
        me
    }

    /// Remove the links no road could climb — the ones that join two **decks**.
    ///
    /// Heights were solved one path at a time ([`route::follow`] over each path's own candidates)
    /// when this was written — [`route::follow_graph`] now solves them over the graph, and
    /// `NFS_DECK=0` still asks for the old way. Either way the link filter below is a separate
    /// question from the height solve. Where two
    /// carriageways cross, their nodes can sit a couple of metres apart in plan and ten or twenty
    /// apart in height, and the link between them is then a lift shaft the graph calls a road.
    ///
    /// Nothing downstream can catch it, because everything downstream is plan-view:
    /// [`Self::step_avoiding`] ranks arms by `(x, z)` distance, the pilot's node advance measures
    /// in plan, and `Corridor::locate` drops height on purpose so that a car on a bridge belongs
    /// to the bridge. [`Self::drop_walled`] does not catch it either — it asks whether there is
    /// *a* surface near the interpolated height, and with 8 m of tolerance and good road at both
    /// ends of a 10 m step, there is.
    ///
    /// **Measured, and the correlation is exact.** Over the eight sweep routes, cars end up above
    /// the node their own pilot is holding — 15.3 m on one route, 22.5 on another, and for 70.6 %
    /// of the race on a third. The one route with **no link steeper than 15 %** is the one route
    /// where that never happens at all. The offenders are unambiguous: 14.8 m of height over 2 m
    /// of plan (941 %), 12.2 m over 1 m (1084 %), and **every single one of them crosses between
    /// paths**.
    ///
    /// A very steep city street is about 25 %. This keeps that and everything gentler.
    ///
    /// **REFUTED (2026-08-21), and the refutation is what located the real mechanism.** Dropping
    /// them changes the deck divergence hardly at all — 70.6 % → 69.7 % of the race on the worst
    /// route, and *worse* on another (31.4 % → 53.7 %) — while costing 217 waypoints, 171
    /// junctions and 506 m of `furthest` over the eight routes.
    ///
    /// The steep links are real and are not the cause. The divergence arrives through a link that
    /// looks **gentle**: node 237 at y=10.68 to node 55 at y=5.71 is a 4.97 m drop over 30.5 m of
    /// plan, a 16 % grade that any street could have. What is wrong is not that link's slope, it
    /// is that its two ends were solved on **different decks** — heights are chosen one path at a
    /// time, so nothing ever compares the two sides of a path boundary. Catching it needs the
    /// heights themselves, not a filter over them. Off by default; `NFS_MAXGRADE=0.25` restores it.
    fn drop_climbing(&mut self) {
        let max: f32 =
            std::env::var("NFS_MAXGRADE").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_GRADE);
        if max <= 0.0 {
            return;
        }
        let mut steep: Vec<(u32, u32)> = Vec::new();
        for i in 0..self.nodes.len() {
            let (at, links) = {
                let n = &self.nodes[i];
                (n.at, n.links.clone())
            };
            for l in links {
                let Some(b) = self.nodes.get(l as usize).map(|n| n.at) else { continue };
                let run = Vec3::new(b.x - at.x, 0.0, b.z - at.z).length();
                if run >= 1.0 && (b.y - at.y).abs() / run > max {
                    steep.push((i as u32, l));
                }
            }
        }
        for (i, l) in &steep {
            if let Some(n) = self.nodes.get_mut(*i as usize) {
                n.links.retain(|x| x != l);
            }
        }
        self.climbed = steep.len() / 2;
    }

    /// Remove the links a car cannot actually drive, because the road does not continue along them.
    ///
    /// **Twice now the field has stopped against something the graph said was a road**: first a
    /// crash barrier between two carriageways of an interchange, then a retaining wall beside one.
    /// Both times the node on the far side was a few metres away and the link between them is in
    /// the file — the network joins roads that run beside each other, and nothing in it says there
    /// is a wall in between.
    ///
    /// Nothing in the *parser* can say so either; it is a question about the city, and the city
    /// answers it. Walk the link and ask [`Ground`] for a surface near the interpolated height at
    /// each step. Where the road continues there is one; where a wall or a drop separates the two
    /// there is not.
    ///
    /// Kept only if it earns its place — see `ROADMAP.md` for the sweep that threw out the previous
    /// filter, which was just as plausible and made the field get less far at every setting.
    fn drop_walled(&mut self, ground: &Ground) {
        const STEP: f32 = 3.0;
        // Swept, and it is the sweep that decides it rather than the idea: 0 (no filter) leaves the
        // field at 21 junctions and the second waypoint, 2.5 makes it *worse* at 17, 5 worse still
        // at 15, and 8 and 12 both take it to **33 junctions and the sixth waypoint**. Tight
        // tolerances cut real roads, because a straight line between two nodes does not follow a
        // crest or a dip; loose ones cut only what the road genuinely does not cross.
        let tolerance: f32 =
            std::env::var("NFS_WALL").ok().and_then(|v| v.parse().ok()).unwrap_or(8.0);
        if tolerance <= 0.0 {
            return;
        }
        // Only one thing is asked along a link: is the road *there*. Asking whether it is also
        // *level* — no step from one sample to the next, which is what a raised kerb is and what
        // `Paths4002`'s field piled into — is **refuted**. It does not fix the route it was written
        // for (still 42 m) and it costs the ones that worked: 4001 falls from 991 m to 268 m,
        // 4081 from 746 to 333, and the eight-route total from 8.1 km to 6.8 km at every threshold
        // swept. A kerb has a drivable surface on top, so it passes "is there road here" — and
        // every rule sharp enough to catch it cuts crests and dips that are road.
        //
        // And the class this filter is *known* to let through — a central reservation, a guardrail,
        // a retaining wall, all of which have good road on both sides and along the line between —
        // was written as its own filter, swept and **refuted**. An index of every `Surface::Wall`
        // triangle was asked "does anything stand across this link at car height", first along the
        // straight chord and then along the road's own profile. The chord is not even the right
        // instrument: its cut count does not fall as the height rises (126 links at 0.15 m, 140 at
        // 0.5, 108 at 1.5), because a chord runs *underground* wherever the road climbs between its
        // ends and answers with the embankment it is buried in. The profile walk is monotone
        // (168 → 151 → 127 → 121 → 77) and so measures what it claims — and it loses too, at every
        // height: 605 waypoints covered with no filter against 564 / 577 / 597 / 595 at 0.5 / 1 /
        // 2 / 3 m. The whole of the effect is two routes in eight, and they cancel: `Paths4002`'s
        // leading car gets four times as far (176 → 739 m) while the field's coverage there does
        // not move (27 → 26), and `Paths4001` loses a third of its coverage (141 → 103) at every
        // setting, its dead ends going 1 → 6. Removing a link a car cannot drive also removes the
        // way round it, and this graph cannot spare it.
        //
        // So the barrier was **priced** instead — the branch left in the graph and made to look a
        // hundred metres further off, which loses to any ordinary branch and still wins when it is
        // the only way on, so no dead end can be created by construction. Swept on both axes and
        // **refuted** as well. It beats deleting on every column and still does not earn its place:
        // the cost saturates at 100 m (300 is identical to it) and the best cell of the two,
        // 1 m of lift, leaves the field at 609 waypoints against 605 with no mechanism at all —
        // the margin this project called "no gain" when the blacklists came to 461 against 462.
        // Half a metre of lift is the interesting one and it splits the columns rather than winning
        // them: 4,717 m against 4,187 and 973 distinct nodes against 926, for 593 waypoints
        // against 605.
        //
        // The instrument was cleared on the way past, and that is the part worth keeping. Of the
        // 168 links it marks on 4001, **154 join two paths of the file** — which is exactly what a
        // central reservation between two carriageways is — against 14 that run along a single path
        // and therefore cannot have a barrier across them at all. `Paths4061` has none, which is
        // why five of the eight routes never move by a digit at any setting. What the marks cost is
        // real detour, not mismarking. See `ROADMAP.md`.
        let solid = |a: Vec3, b: Vec3| ground.gap_along(a, b, tolerance, STEP).is_none();
        let mut cut: Vec<(usize, u32)> = Vec::new();
        for i in 0..self.nodes.len() {
            for l in &self.nodes[i].links {
                if (*l as usize) > i && !solid(self.nodes[i].at, self.nodes[*l as usize].at) {
                    cut.push((i, *l));
                }
            }
        }
        self.walled = cut.len();
        for (a, b) in cut {
            self.nodes[a].links.retain(|l| *l != b);
            self.nodes[b as usize].links.retain(|l| *l != a as u32);
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    #[must_use]
    pub fn node(&self, i: u32) -> Option<&Junction> {
        self.nodes.get(i as usize)
    }

    /// How steep a link is, as rise over run.
    ///
    /// Reported, not enforced. Refusing steep links was tried — the idea being that Bayview stacks
    /// and a graph measured in the ground plane would route a car onto the deck above — and it is
    /// **refuted**: at every threshold swept the field got *less* far than with no filter at all
    /// (0.30 → 34 junctions, 0.60 → 29, 1.00 → 33, none → the furthest, reaching the second
    /// waypoint). The city's ramps are steeper than they look and the filter cuts them, so a
    /// number that was meant to keep cars off bridges keeps them off roads.
    #[must_use]
    fn grade(&self, a: u32, b: u32) -> f32 {
        match (self.node(a), self.node(b)) {
            (Some(x), Some(y)) => {
                let flat = ((y.at.x - x.at.x).powi(2) + (y.at.z - x.at.z).powi(2)).sqrt();
                (y.at.y - x.at.y).abs() / flat.max(0.001)
            }
            _ => 0.0,
        }
    }

    /// The node nearest a point — **in three dimensions**, because the ground plane is not enough
    /// where the city has four roads above one another.
    #[must_use]
    pub fn nearest(&self, to: Vec3) -> Option<u32> {
        self.nodes
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let d = |j: &Junction| (j.at - to).length_squared();
                d(a.1).total_cmp(&d(b.1))
            })
            .map(|(i, _)| i as u32)
    }

    /// The next node to head for, from `here`, having come from `came_from`, aiming at `toward`.
    ///
    /// **Straight line, and a shortest path over the roads is refuted.** The obvious complaint about
    /// this rule is that a bearing is a bad compass in a city — greedy descent on the crow's flight
    /// walks into whatever happens to *point* at the target — so it was replaced with the honest
    /// question: Dijkstra outward from the waypoint's own node, every branch scored by how much road
    /// is left. Over eight routes it is **worse on every measure**: waypoints actually driven past
    /// 462 → 391, distance 4,129 → 2,779 m, cars that stop making progress before t=30 s 39 → 44.
    ///
    /// And it is not a broken implementation, which is what makes it worth writing down. Measured on
    /// four routes, the fields solve **the entire graph** (341 of 341 nodes, 340 of 341, 154 of 154,
    /// 227 of 227) and every waypoint sits a median 17-19 m from the node its field was solved from,
    /// worst 107 m. The compass was correct and the car still did worse with it.
    ///
    /// The reading that survives: **this graph is not a road map that can be followed exactly.** It
    /// joins roads that merely run beside each other — [`Self::drop_walled`] exists because of two
    /// such links with a wall between them — so the shortest path routes a car across joins it
    /// cannot physically take, and commits to that one route. A bearing wanders, and wandering is
    /// what tolerates a graph that is only roughly right. Make the graph true before making the
    /// route optimal.
    ///
    /// Chooses the neighbour that gets nearest the target, and **refuses to turn round**: the node
    /// just left is excluded unless it is the only way out. Without that a car at a dead end
    /// oscillates, and with it a dead end is driven out of rather than rattled in.
    #[must_use]
    pub fn step(&self, here: u32, came_from: Option<u32>, toward: Vec3) -> Option<u32> {
        self.step_avoiding(here, came_from, toward, &[])
    }

    /// The shortest way through the graph from one node to another, by plan-view length.
    ///
    /// **Not for driving.** Following a solved route is separately refuted — a field solved over the
    /// whole graph made the cars do *worse*, because this graph joins roads that merely run beside
    /// each other and a committed route crosses joins a car cannot take (see this module's own
    /// header). What it is for is **building the course**: the event outline gives corners up to
    /// 425 m apart and the ring between them is currently a straight chord, which measured 55 % of
    /// its waypoints outside the corridor. A path over the roads is a better guess at what the race
    /// goes round, and the pilot still walks the graph greedily as it always did.
    ///
    /// Dijkstra over a few hundred nodes at setup, which is nothing; the heap is ordered on a
    /// scaled integer because `f32` is not `Ord`.
    #[must_use]
    pub fn path(&self, from: u32, to: u32) -> Option<Vec<u32>> {
        self.path_where(from, to, |_, _| true)
    }

    /// The same, over the links a caller is willing to use.
    ///
    /// **Why the caller decides.** Whether a link is drivable is a question about the *city*, not
    /// about the table — and the graph already answers half of it ([`Self::drop_walled`] drops the
    /// links where the ground does not continue) but not the other half: two carriageways with a
    /// barrier between them have good road on both sides and a link across it. Baking a wall test
    /// into the graph itself was tried once and swept out, so the test stays out here where it can
    /// be given to one caller — the course builder — without touching the graph the pilot drives.
    #[must_use]
    pub fn path_where(
        &self,
        from: u32,
        to: u32,
        passable: impl Fn(u32, u32) -> bool,
    ) -> Option<Vec<u32>> {
        use std::collections::BinaryHeap;
        if from as usize >= self.nodes.len() || to as usize >= self.nodes.len() {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }
        let plan = |a: Vec3, b: Vec3| Vec3::new(b.x - a.x, 0.0, b.z - a.z).length();
        let mut best = vec![f32::INFINITY; self.nodes.len()];
        let mut came: Vec<Option<u32>> = vec![None; self.nodes.len()];
        // `Reverse` on a scaled cost: centimetres are finer than any decision this makes.
        let mut heap = BinaryHeap::new();
        best[from as usize] = 0.0;
        heap.push((std::cmp::Reverse(0i64), from));
        while let Some((std::cmp::Reverse(c), i)) = heap.pop() {
            if i == to {
                break;
            }
            if (c as f32) / 100.0 > best[i as usize] + 1e-3 {
                continue;
            }
            let Some(node) = self.node(i) else { continue };
            for &l in &node.links {
                let Some(n) = self.node(l) else { continue };
                if !passable(i, l) {
                    continue;
                }
                let step = best[i as usize] + plan(node.at, n.at);
                if step + 1e-3 < best[l as usize] {
                    best[l as usize] = step;
                    came[l as usize] = Some(i);
                    heap.push((std::cmp::Reverse((step * 100.0) as i64), l));
                }
            }
        }
        if !best[to as usize].is_finite() {
            return None;
        }
        let mut out = vec![to];
        let mut at = to;
        while let Some(prev) = came[at as usize] {
            out.push(prev);
            at = prev;
            // A cycle here would be a bug in the relaxation, not a graph the caller can fix.
            if out.len() > self.nodes.len() {
                return None;
            }
        }
        out.reverse();
        Some(out)
    }

    /// Say where the race's own line runs, so the walk can prefer to stay on it.
    ///
    /// **Measured, and this is why it exists.** With no such preference, on `Paths4121` all eight
    /// cars take the step from node 110 to node 111 and node 111 is **50.9 m from the nearest
    /// waypoint**; on `Paths4102` seven take node 11 → 10, 58.6 m off. In **21 of 21** such
    /// departures an arm that stayed on the line existed and was not chosen, because
    /// [`Self::step_avoiding`] picks by straight-line distance to the goal and a carriageway running
    /// alongside the race is often nearer by that measure. The cars then drive a road the race does
    /// not use until it bends away from them — which is the "corner" three pilot sweeps were tuned
    /// against.
    ///
    /// One pass over nodes × waypoints at setup, and nothing per tick.
    pub fn mark_line(&mut self, course: &[Vec3], within: f32) {
        self.near_line = if course.is_empty() || within <= 0.0 {
            Vec::new()
        } else {
            self.nodes
                .iter()
                .map(|n| {
                    course.iter().any(|w| {
                        let d = Vec3::new(w.x - n.at.x, 0.0, w.z - n.at.z);
                        d.length() <= within
                    })
                })
                .collect()
        };
    }

    /// Whether one node carries the mark. `false` when nothing has been marked, so a caller that
    /// forgets [`Self::mark_line`] gets "off the line everywhere" rather than a silent pass.
    #[must_use]
    pub fn on_line_at(&self, i: u32) -> bool {
        self.near_line.get(i as usize).copied().unwrap_or(false)
    }

    /// How many nodes are on the line, and how many there are — for a caller that wants to know
    /// whether the mark says anything before trusting a result that depends on it.
    #[must_use]
    pub fn on_line(&self) -> (usize, usize) {
        (self.near_line.iter().filter(|b| **b).count(), self.nodes.len())
    }

    /// The same, skipping nodes a caller has already failed to reach.
    ///
    /// The list is a driver's own experience and not a property of the graph, which is why it is a
    /// parameter rather than state here: two cars on the same road can have hit different things.
    #[must_use]
    pub fn step_avoiding(
        &self,
        here: u32,
        came_from: Option<u32>,
        toward: Vec3,
        blocked: &[u32],
    ) -> Option<u32> {
        let node = self.node(here)?;
        let dist = |i: &u32| {
            self.node(*i).map_or(f32::MAX, |j| {
                (j.at.x - toward.x).powi(2) + (j.at.z - toward.z).powi(2)
            })
        };
        // **Preferring an arm that stays on the race's line is refuted, in both shapes.** The
        // measurement behind it stands — see [`Self::mark_line`], 21 of 21 departures from the line
        // had an arm that would have stayed — and the inference from it does not.
        //
        // Filtering the arms down to the on-line ones empties `Paths4121`: 204 junctions to **36**,
        // 30 waypoints to 10, because a junction whose only on-line arm is the one the car came from
        // leaves the walk nothing to take. Penalising the cost instead of filtering, so the old
        // choice survives wherever it is clearly better and only near-ties change, does the same
        // thing: 30 waypoints to **11** at 40 m of penalty and to 10 at 100 m. Over eight routes the
        // penalty form takes the field from 986 waypoints to 815, and 32 cars that never lose the course to 24.
        //
        // So the arm the graph calls wrong is the one that can actually be driven, and the line's
        // own continuation cannot — which is a fact about the course description, not about this
        // choice. Whatever fixes it starts by asking where node 110's on-line arms go and what is
        // there, not by weighting this `min_by` again.
        // **`NFS_SAMEPATH=1`: break a tie in favour of the path the car is already on.**
        //
        // Not the refuted rule above, and the difference is what it is a fact about. That one
        // ranked arms by the *race line*, a description the graph does not share; this one only
        // looks at arms the graph itself says lead to the **same place** — within `TWIN` metres in
        // plan and in height — and among those takes the one on the current path. Two nodes that
        // close together on the same deck are one road described twice, so this is not a choice
        // being made differently, it is a choice that was never real.
        //
        // The five places that account for 69 % of the field's departures were mapped, and three
        // of them have exactly this shape: `Paths4081`'s junction 206 offers three arms on a
        // parallel path 30 m away, `Paths4001`'s junction 13 offers three at one spot on three
        // paths at y 24.1-24.2, `Paths4102`'s junction 209 two at (46,−75) and (47,−75).
        const TWIN: f32 = 6.0;
        let same_path = std::env::var("NFS_SAMEPATH").is_ok_and(|v| !v.is_empty() && v != "0");
        let pick = node
            .links
            .iter()
            .filter(|l| Some(**l) != came_from && !blocked.contains(l))
            .min_by(|a, b| dist(a).total_cmp(&dist(b)));
        let pick = match (same_path, pick) {
            (true, Some(best)) => {
                let b = self.node(*best);
                node.links
                    .iter()
                    .filter(|l| Some(**l) != came_from && !blocked.contains(l))
                    .find(|l| {
                        self.node(**l).is_some_and(|n| {
                            n.path == node.path
                                && b.is_some_and(|q| {
                                    (n.at.x - q.at.x).hypot(n.at.z - q.at.z) <= TWIN
                                        && (n.at.y - q.at.y).abs() <= TWIN
                                })
                        })
                    })
                    .or(Some(best))
            }
            (_, p) => p,
        };
        pick.into_iter()
            .min_by(|a, b| dist(a).total_cmp(&dist(b)))
            // Falling back past its own memory: a car with nowhere left it has not given up on has
            // to go somewhere, and standing still is not somewhere.
            .or_else(|| node.links.iter().find(|l| Some(**l) != came_from))
            .or_else(|| node.links.first())
            .copied()
    }

    /// How many links the graph holds, how many nodes have none, and what the height solve had to
    /// guess at.
    ///
    /// Reported rather than assumed: a node with no way out is a car that stops, and the count is
    /// the difference between "the driver is bad" and "the road ends here". The last two are the
    /// same argument about heights — `trees` is how many pieces the solve fell into, and `filled`
    /// how many nodes took a height from a neighbour instead of from the city. Both are zero on the
    /// unfiltered ground and neither is zero on roads alone, which is the cost side of that trade.
    #[must_use]
    pub fn shape(&self) -> (usize, usize, usize, usize, usize, usize) {
        let steep = (0..self.nodes.len())
            .flat_map(|i| self.nodes[i].links.iter().map(move |l| (i as u32, *l)))
            .filter(|(a, b)| self.grade(*a, *b) > 1.0)
            .count()
            / 2;
        (
            self.nodes.iter().map(|n| n.links.len()).sum::<usize>() / 2,
            self.nodes.iter().filter(|n| n.links.is_empty()).count(),
            steep,
            self.walled,
            self.trees,
            self.filled,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A graph built by hand, because [`Network::of`] needs a parsed route file and a city.
    fn graph(at: &[(f32, f32)], links: &[(u32, u32)]) -> Network {
        let mut nodes: Vec<Junction> = at
            .iter()
            .map(|(x, z)| Junction { at: Vec3::new(*x, 0.0, *z), path: 0, links: Vec::new() })
            .collect();
        for (a, b) in links {
            nodes[*a as usize].links.push(*b);
            nodes[*b as usize].links.push(*a);
        }
        Network { nodes, walled: 0, climbed: 0, trees: 1, filled: 0, near_line: Vec::new() }
    }

    /// The short way round, not the first way found. A greedy walk down the long arm would answer
    /// three hops; the point of a search is that it does not.
    #[test]
    fn the_path_is_the_short_way_round() {
        //   0 ─ 1 ─ 2      (10 m apart)
        //   └────────┘     one 100 m link straight from 0 to 2
        let net = graph(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0), (0.0, 100.0)], &[(0, 1), (1, 2), (0, 3), (3, 2)]);
        assert_eq!(net.path(0, 2), Some(vec![0, 1, 2]), "20 m of road beats 200 m of road");
        assert_eq!(net.path(0, 0), Some(vec![0]), "already there");
    }

    /// Two islands. A course built over this has to know the difference between "the way is long"
    /// and "there is no way", because the first is a route and the second is a hole to report.
    #[test]
    fn nothing_joins_two_islands() {
        let net = graph(&[(0.0, 0.0), (10.0, 0.0), (500.0, 0.0), (510.0, 0.0)], &[(0, 1), (2, 3)]);
        assert_eq!(net.path(0, 3), None);
        assert_eq!(net.path(0, 9), None, "a node that is not there is not reachable");
    }

    /// The walk comes back in order, from the node asked for to the node asked about, and every
    /// step of it is a link the graph holds.
    #[test]
    fn the_path_is_a_sequence_of_real_links() {
        let net = graph(
            &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0), (30.0, 0.0), (40.0, 0.0)],
            &[(0, 1), (1, 2), (2, 3), (3, 4)],
        );
        let p = net.path(0, 4).expect("a straight chain is connected");
        assert_eq!(p.first(), Some(&0));
        assert_eq!(p.last(), Some(&4));
        for w in p.windows(2) {
            assert!(
                net.node(w[0]).is_some_and(|n| n.links.contains(&w[1])),
                "{} → {} is not a link",
                w[0],
                w[1]
            );
        }
    }
}
