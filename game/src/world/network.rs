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
        let solid = |a: Vec3, b: Vec3| {
            let d = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
            let n = (d.length() / STEP).ceil().max(1.0) as usize;
            (1..n).all(|k| {
                let t = k as f32 / n as f32;
                let p = a.lerp(b, t);
                ground.heights_at(p.x, p.z).iter().any(|h| (h - p.y).abs() <= tolerance)
            })
        };
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
    /// Chooses the neighbour that gets nearest the target, and **refuses to turn round**: the node
    /// just left is excluded unless it is the only way out. Without that a car at a dead end
    /// oscillates, and with it a dead end is driven out of rather than rattled in.
    #[must_use]
    pub fn step(&self, here: u32, came_from: Option<u32>, toward: Vec3) -> Option<u32> {
        let node = self.node(here)?;
        let dist = |i: &u32| {
            self.node(*i).map_or(f32::MAX, |j| {
                (j.at.x - toward.x).powi(2) + (j.at.z - toward.z).powi(2)
            })
        };
        node.links
            .iter()
            .filter(|l| Some(**l) != came_from)
            .min_by(|a, b| dist(a).total_cmp(&dist(b)))
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
