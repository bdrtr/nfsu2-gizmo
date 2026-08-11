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
use gizmo_nfs::world::routes::{paths, RouteNode};
use gizmo_nfs::world::WorldMesh;

/// Whether a city object is road surface.
///
/// A **render-and-placement category, not a parser fact**, in the same family as
/// [`super::is_backdrop`]: the file does not label roads, and the name is what the artists left
/// behind. `TRN_ROADA_CHOP_*`, `TRN_CS_ROADA_*`, `TRN_CN_ROADA_*` and `RDP_*` all carry it; 1,928 of
/// the city's 13,985 objects match.
#[must_use]
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
fn follow(candidates: &[Vec<f32>]) -> Vec<Option<f32>> {
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
fn fill(heights: &mut [Option<f32>]) -> usize {
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
