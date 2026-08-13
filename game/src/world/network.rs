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
}

impl Network {
    /// Build the graph, standing every node on the city.
    ///
    /// Heights come from the same [`Ground`] the drawn line uses, so a car following this and a
    /// ribbon drawn from `route::build` cannot disagree about where the road is.
    ///
    /// **Both kinds of edge, and both directions.** Along a path, file order is the sequence — and
    /// only the sequence: nearly half the install's paths are stored against the way they are
    /// driven, so an edge here is undirected and the direction is the driver's business. Across
    /// paths, the record's own `+12`/`+14`/`+16`, which are directed in the file and made mutual
    /// here for the same reason: only 29 % are mutual as stored, because a junction is written from
    /// one side.
    #[must_use]
    pub fn of(nodes: &[RouteNode], ground: &Ground) -> Self {
        // Heights the same way the drawn line gets them, and for the same reason. Asking
        // `height_at` from two metres up answers "the highest surface at or below two metres",
        // which on an elevated road is **nothing** — the node then falls back to zero and sits ten
        // metres under the tarmac. Six of seven rivals drove into the ground aiming at one.
        //
        // `route::follow` is the fix already written: every candidate surface at the node's XZ,
        // then the sequence over the path that climbs least. Shared rather than reimplemented, so
        // a car and a ribbon cannot end up on different decks of the same interchange.
        let flat: Vec<Vec3> = nodes.iter().map(|n| remap([n.x, n.y, 0.0])).collect();
        let mut height: Vec<f32> = vec![0.0; nodes.len()];
        let mut i = 0;
        while i < nodes.len() {
            let mut j = i;
            while j + 1 < nodes.len() && nodes[j + 1].path == nodes[i].path {
                j += 1;
            }
            let candidates: Vec<Vec<f32>> =
                (i..=j).map(|k| ground.heights_at(flat[k].x, flat[k].z)).collect();
            let mut solved = super::route::follow(&candidates);
            super::route::fill(&mut solved);
            for (k, h) in (i..=j).zip(&solved) {
                height[k] = h.unwrap_or(0.0);
            }
            i = j + 1;
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

        let edge = |a: usize, b: usize, out: &mut Vec<Junction>| {
            if a != b && a < out.len() && b < out.len() {
                if !out[a].links.contains(&(b as u32)) {
                    out[a].links.push(b as u32);
                }
                if !out[b].links.contains(&(a as u32)) {
                    out[b].links.push(a as u32);
                }
            }
        };
        for i in 0..nodes.len() {
            if i + 1 < nodes.len() && nodes[i + 1].path == nodes[i].path {
                edge(i, i + 1, &mut out);
            }
            for l in nodes[i].linked() {
                edge(i, l as usize, &mut out);
            }
        }
        let mut me = Self { nodes: out, walled: 0 };
        me.drop_walled(ground);
        me
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
        node.links
            .iter()
            .filter(|l| Some(**l) != came_from && !blocked.contains(l))
            .min_by(|a, b| dist(a).total_cmp(&dist(b)))
            // Falling back past its own memory: a car with nowhere left it has not given up on has
            // to go somewhere, and standing still is not somewhere.
            .or_else(|| node.links.iter().find(|l| Some(**l) != came_from))
            .or_else(|| node.links.first())
            .copied()
    }

    /// How many links the graph holds, and how many nodes have none.
    ///
    /// Reported rather than assumed: a node with no way out is a car that stops, and the count is
    /// the difference between "the driver is bad" and "the road ends here".
    #[must_use]
    pub fn shape(&self) -> (usize, usize, usize, usize) {
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
        )
    }
}
