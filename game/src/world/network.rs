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
        let mut out: Vec<Junction> = nodes
            .iter()
            .map(|n| {
                let flat = remap([n.x, n.y, 0.0]);
                let y = ground.height_at(flat + Vec3::Y * 2.0).unwrap_or(flat.y);
                Junction { at: Vec3::new(flat.x, y, flat.z), path: n.path, links: Vec::new() }
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
        Self { nodes: out }
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

    /// The node nearest a point, in the ground plane.
    #[must_use]
    pub fn nearest(&self, to: Vec3) -> Option<u32> {
        self.nodes
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let d = |j: &Junction| (j.at.x - to.x).powi(2) + (j.at.z - to.z).powi(2);
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
    pub fn shape(&self) -> (usize, usize) {
        (
            self.nodes.iter().map(|n| n.links.len()).sum::<usize>() / 2,
            self.nodes.iter().filter(|n| n.links.is_empty()).count(),
        )
    }
}
