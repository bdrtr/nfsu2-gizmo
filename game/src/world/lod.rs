//! The city's own detail tiers: `_1A`, `_1B`, `_1Z`.
//!
//! Every second world object carries a `_1<LETTER>` token in its name, and the letters are not a
//! sequence — across the eight bundles they are **A (1,819), B (1,392), Z (1,203)** and then
//! almost nothing (C 41, D 3, and four singletons). Three levels, not an alphabet, which is what
//! rules out "sections of one building" as a reading: sections would run A, B, C, D.
//!
//! What they are is the game's own LOD chain. Three things say so and none of them is the letters:
//!
//! - **The name repeats exactly apart from the letter.** `XB_3TOWERAPARTLK_1A_00`,
//!   `_1B_00`, `_1Z_00` — same design, same trailing instance number. That is why the family key
//!   here is *design + tail* and not the design alone: `_1A_00` and `_1A_01` are two placements of
//!   one design and must not be mistaken for two tiers of one placement.
//! - **The vertex count falls with the letter**, in 1,377 of 1,423 consecutive pairs.
//! - **The install spells it out.** 28 objects are named `LOD_<design>_1Z_LL` — the `_1Z` tier of a
//!   design, with `LOD` in the name.
//!
//! ## Only the finest is drawn — and the measurement that said otherwise was answering a
//! different question
//!
//! **This was decided the wrong way once and the correction is the useful part.** Everything in the
//! next section is still true: the tiers are placed, they stand on the ground as often as the
//! finest does, and they interpenetrate their neighbours no more than it does. From that it was
//! concluded that dropping them removes objects that belong somewhere, and the default was left at
//! "draw everything".
//!
//! Then someone drove through it. A coarse tier is a **distance imposter** — a box with a
//! photographic facade, authored to be read at five hundred metres — and from a car it is a blurred
//! slab standing in open ground where its building is not. `XB_LANDMARKTOWER_1Z_RB_00` sits on
//! grass beside a motorway junction 544 m from the tower it stands in for.
//!
//! Nothing in the measurements was wrong. They asked *whether the file places these objects like
//! buildings*, and it does. They could not ask whether the geometry is **finished enough to be
//! seen from the street**, which is a different question and the one that mattered. `ROADMAP.md`
//! said from the start that this decision needed an eye; it was overruled with numbers, and the
//! numbers were answering something else.
//!
//! So [`keep_finest`] is the default and `NFS_TIERS=all` is the way back.
//!
//! ## What the measurements do establish
//!
//! The obvious next thought — "then draw one of them" — was measured and refused. Two numbers say
//! why, both over the whole city (8 bundles, 13,985 objects after dedup):
//!
//! - **They do not share a spot.** 1,330 of 1,354 families stand apart rather than stacked, by up
//!   to 651 m, and 1,007 of them lie on a line parallel to one world axis. A pass that keeps the
//!   richest member moves 2,457 objects and 286,954 vertices out of the world; it does not collapse
//!   copies onto one another.
//! - **They stand on the ground exactly as often as the finest tier does.** Of the members over
//!   measurable drivable ground, 67 % of the coarse ones have their underside within 8 m of the
//!   surface, against 70 % of the finest ones. Whatever the offsets are, they are not a parking
//!   scheme that puts coarse copies out of the way: the file plants them on Bayview's streets the
//!   same way it plants everything else.
//!
//! What the tiers really are is the input a distance-based selector wants — and the renderer HAS
//! one now (`LodGroup` / `LodLevel`, honoured by the engine's own `collect_draw_items` since
//! engine `eda05c3`, which the pin has carried since 2026-08-14). So the day named below has come:
//! this module is where the selector asks, `families()` / `Family::finest()` are its input, and
//! the coarse tiers stop being something to drop and become something to swap in. See
//! `MOTOR-NOTLARI.md` item 8 — the engine half is done, this is the half that is waiting.
//!
//! Worth keeping in view before starting: the measured prize is small. Drawing all three tiers is
//! 5672 → 6406 meshes and 8.0 → 9.1 ms, so at most 1.1 ms is on the table, and `keep_finest`'s
//! reason was never speed — it is that a coarse tier read as a blurred box up close.
//!
//! What stays open is *why* a tier chain is spread across the map at all. That is a question about
//! the bundle rather than about the game: the visibility data the original engine used
//! (`TRACKS/PrecullerBooBooScript.hoo`, `ROUTES*/Paths*.bin`) is not parsed, and nothing in the
//! 192-byte solid header distinguishes the tiers — their records differ only in the hash, the
//! counters, and the one translation component.

use crate::world::world_point;
use gizmo::prelude::*;
use gizmo_nfs::world::WorldMesh;
use std::collections::{BTreeMap, HashSet};

/// The `_1<LETTER>` token in a world object's name, split into the parts that identify it.
///
/// `XB_3TOWERAPARTLK_1A_00` → design `XB_3TOWERAPARTLK`, letter `A`, tail `_00`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierName<'a> {
    /// Everything before the token: the design.
    pub design: &'a str,
    /// The tier letter itself. `A` is the richest the city ships, `Z` the poorest.
    pub letter: char,
    /// Everything after the token: the district code and the instance number, when they survived
    /// the name field's truncation.
    pub tail: &'a str,
}

impl TierName<'_> {
    /// The key that means "this design, at this placement" — everything but the tier letter.
    ///
    /// Joined with a byte that cannot occur in a name, so `A_B` + `C` and `A` + `B_C` stay apart.
    #[must_use]
    pub fn family(&self) -> String {
        format!("{}\u{1}{}", self.design, self.tail)
    }
}

/// Read the tier token out of a name, if it has one.
///
/// The token is `_1<UPPERCASE>` **at a name boundary** — end of string, or followed by `_`. The
/// boundary is what keeps `ARC_1WOOD4CONSTRUCTION` and `L4RCS_1TOWER` out: a letter that merely
/// happens to follow `_1` is not a tier.
#[must_use]
pub fn tier_of(name: &str) -> Option<TierName<'_>> {
    let b = name.as_bytes();
    (0..b.len().saturating_sub(2)).find_map(|i| {
        let end = i + 3;
        let boundary = end == b.len() || b[end] == b'_';
        (b[i] == b'_' && b[i + 1] == b'1' && b[i + 2].is_ascii_uppercase() && boundary).then(|| {
            TierName { design: &name[..i], letter: b[i + 2] as char, tail: &name[end..] }
        })
    })
}

/// One member of a family: an object, and where and how big it is.
#[derive(Debug, Clone, Copy)]
pub struct Member {
    /// Index into the slice the family was read from.
    pub index: usize,
    pub letter: char,
    /// Centre of the object's box, in world space.
    pub centre: Vec3,
    /// The box's extent, in world space.
    pub size: Vec3,
    pub vertices: usize,
    /// Whether the object carries a placement matrix, or is already in world space.
    ///
    /// Kept on the member because "the coarse tiers stand somewhere else" and "we are putting the
    /// coarse tiers somewhere else" look identical in a centre, and only this tells them apart.
    pub placed: bool,
}

/// One design at one placement, in every tier the city ships for it.
#[derive(Debug, Clone)]
pub struct Family {
    pub design: String,
    pub tail: String,
    /// Sorted richest-first, so `members[0]` is what a "finest only" pass keeps.
    pub members: Vec<Member>,
}

impl Family {
    /// The member with the most vertices — the letter is a convention, the count is the thing.
    #[must_use]
    pub fn finest(&self) -> Member {
        self.members[0]
    }

    /// How far the family's members stand apart: the furthest any member's centre is from the
    /// finest one's.
    ///
    /// **This is the number the whole question turns on.** A chain of LODs authored for one spot
    /// would read ~0 here and dropping the coarse ones would be free. It does not: the city's
    /// families are laid out apart, so a coarse tier occupies ground of its own.
    #[must_use]
    pub fn spread(&self) -> f32 {
        let c = self.finest().centre;
        self.members.iter().map(|m| (m.centre - c).length()).fold(0.0, f32::max)
    }

    /// Whether every member sits on one line parallel to a world axis — two of the three
    /// coordinates shared to within a metre.
    ///
    /// A city places repeats of one design wherever the streets go: across a junction, around a
    /// corner, at whatever angle the block runs. Members strung along a single axis are a *layout*,
    /// not an address, and the rate across the whole city is what separates "the same building
    /// three times" from "three buildings of the same design".
    #[must_use]
    pub fn axis_aligned(&self) -> bool {
        let c = self.finest().centre;
        let shares = |f: fn(Vec3) -> f32| self.members.iter().all(|m| (f(m.centre) - f(c)).abs() < 1.0);
        let (x, y, z) = (shares(|v| v.x), shares(|v| v.y), shares(|v| v.z));
        usize::from(x) + usize::from(y) + usize::from(z) >= 2
    }

    /// Whether the members' boxes overlap the finest one's — the same question as [`spread`] asked
    /// against the objects' own size rather than an absolute distance, because 40 m apart means one
    /// thing for a tower and another for a kerbstone.
    ///
    /// [`spread`]: Family::spread
    #[must_use]
    pub fn stacked(&self) -> bool {
        let f = self.finest();
        self.members.iter().all(|m| {
            let gap = (m.centre - f.centre).abs();
            let reach = (m.size + f.size) * 0.5;
            gap.x <= reach.x && gap.y <= reach.y && gap.z <= reach.z
        })
    }
}

/// Every design-and-placement that ships in more than one tier.
///
/// Objects with no geometry are skipped: the `ANM_*` markers all sit at the origin, so they would
/// read as a family stacked at one point and say the opposite of what the geometry says.
///
/// **A key that collects the same letter twice is thrown away, not used.** World object names are
/// truncated to 27 characters — 746 of the city's names sit exactly on that cap and 164 end in a
/// bare `_` — so two placements of a long-named design arrive with *identical* names and merge into
/// one family. The tell is the letters: one placement has one `_1A`. A key holding two of them is
/// not identifying a placement, and acting on it would drop a building that has no other copy. It
/// found real damage — without the check the widest "family" spanned 2.3 km.
#[must_use]
pub fn families(objects: &[WorldMesh]) -> Vec<Family> {
    let mut by_key: BTreeMap<String, (String, String, Vec<Member>)> = BTreeMap::new();
    for (index, m) in objects.iter().enumerate() {
        if m.positions.is_empty() {
            continue;
        }
        let Some(t) = tier_of(&m.header.name) else { continue };
        let lo = world_point(&m.header, m.header.bbox_min);
        let hi = world_point(&m.header, m.header.bbox_max);
        let entry = by_key.entry(t.family()).or_insert_with(|| {
            (t.design.to_string(), t.tail.to_string(), Vec::new())
        });
        entry.2.push(Member {
            index,
            letter: t.letter,
            centre: (lo + hi) * 0.5,
            size: (hi - lo).abs(),
            vertices: m.positions.len(),
            placed: m.header.is_placed(),
        });
    }

    by_key
        .into_values()
        .filter(|(_, _, members)| members.len() > 1 && one_of_each_letter(members))
        .map(|(design, tail, mut members)| {
            members.sort_by(|a, b| b.vertices.cmp(&a.vertices).then(a.letter.cmp(&b.letter)));
            Family { design, tail, members }
        })
        .collect()
}

/// Whether a key's members are one placement's tiers rather than several placements merged by a
/// truncated name — see [`families`].
fn one_of_each_letter(members: &[Member]) -> bool {
    let mut seen = HashSet::new();
    members.iter().all(|m| seen.insert(m.letter))
}

/// Keys thrown away by [`families`] for holding a letter twice, and how many objects they held.
///
/// Reported rather than silently dropped: this is the city telling us the name field lost the
/// information we were keying on, and a run where the number jumps is a run where a name change
/// upstream broke the reading.
#[must_use]
pub fn ambiguous(objects: &[WorldMesh]) -> (usize, usize) {
    let mut by_key: BTreeMap<String, Vec<char>> = BTreeMap::new();
    for m in objects.iter().filter(|m| !m.positions.is_empty()) {
        if let Some(t) = tier_of(&m.header.name) {
            by_key.entry(t.family()).or_default().push(t.letter);
        }
    }
    by_key
        .into_values()
        .filter(|letters| letters.len() > 1)
        .filter(|letters| {
            let mut seen = HashSet::new();
            !letters.iter().all(|l| seen.insert(*l))
        })
        .fold((0, 0), |(keys, objects), letters| (keys + 1, objects + letters.len()))
}

/// The indices of every object that is a coarser tier of a family whose finest member is also
/// loaded — i.e. exactly what a "draw the finest only" pass would remove.
#[must_use]
pub fn coarser(objects: &[WorldMesh]) -> HashSet<usize> {
    families(objects)
        .iter()
        .flat_map(|f| f.members.iter().skip(1).map(|m| m.index))
        .collect()
}

/// What a tier pass would do, in numbers, so a caller can print it rather than re-derive it.
#[derive(Debug, Default, Clone, Copy)]
pub struct Report {
    pub families: usize,
    pub members: usize,
    /// Members that would go if only the finest were kept.
    pub coarser: usize,
    /// Vertices those members carry.
    pub coarser_vertices: usize,
    /// Families whose members' boxes overlap the finest one's — LODs authored for one spot.
    pub stacked: usize,
    /// Families laid out apart. A coarse member here stands on ground of its own.
    pub apart: usize,
    /// The widest [`Family::spread`] seen.
    pub widest: f32,
    /// Families whose members lie on one axis-parallel line — see [`Family::axis_aligned`].
    pub axis_aligned: usize,
    /// Families whose key rests on a name the 27-character field cut short. Not refused — the
    /// repeated-letter check above catches the collisions that actually happened — but counted,
    /// because such a key could still collide with a placement no loaded bundle carries.
    pub truncated: usize,
    /// Keys refused for holding a letter twice — a truncated name merging two placements.
    pub ambiguous_keys: usize,
    /// Objects those keys held, none of which is touched by a tier pass.
    pub ambiguous_objects: usize,
}

/// Measure the tier families in a set of objects.
#[must_use]
pub fn report(objects: &[WorldMesh]) -> Report {
    let mut r = Report::default();
    (r.ambiguous_keys, r.ambiguous_objects) = ambiguous(objects);
    for f in families(objects) {
        r.families += 1;
        r.members += f.members.len();
        r.coarser += f.members.len() - 1;
        r.coarser_vertices += f.members.iter().skip(1).map(|m| m.vertices).sum::<usize>();
        if f.stacked() {
            r.stacked += 1;
        } else {
            r.apart += 1;
        }
        if f.axis_aligned() {
            r.axis_aligned += 1;
        }
        // The stored hash is of the *whole* name, so this is a proof rather than a length guess.
        if f.members.iter().any(|m| !objects[m.index].header.name_is_whole()) {
            r.truncated += 1;
        }
        r.widest = r.widest.max(f.spread());
    }
    r
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tiers: {} families, {} members · {} coarser ({} vertices) · {} stacked, {} apart \
             (widest {:.0} m, {} on one axis) · {} on a truncated name · {} ambiguous keys held \
             {} objects, untouched",
            self.families,
            self.members,
            self.coarser,
            self.coarser_vertices,
            self.stacked,
            self.apart,
            self.widest,
            self.axis_aligned,
            self.truncated,
            self.ambiguous_keys,
            self.ambiguous_objects
        )
    }
}

/// Keep only the richest member of each family; every other tier goes.
///
/// **Not the default anywhere.** The families are not stacked, so this deletes objects that stand
/// somewhere rather than duplicates that stand nowhere.
#[must_use]
pub fn keep_finest(objects: Vec<WorldMesh>) -> Vec<WorldMesh> {
    let drop = coarser(&objects);
    retain_indices(objects, &drop, false)
}

/// The inverse: keep *only* what [`keep_finest`] would drop.
///
/// This is the diagnostic that makes the decision lookable-at — a frame of nothing but the coarse
/// tiers answers "what would be lost" directly, instead of asking an eye to spot an absence.
#[must_use]
pub fn keep_coarser(objects: Vec<WorldMesh>) -> Vec<WorldMesh> {
    let keep = coarser(&objects);
    retain_indices(objects, &keep, true)
}

fn retain_indices(objects: Vec<WorldMesh>, set: &HashSet<usize>, keep_when_in: bool) -> Vec<WorldMesh> {
    objects
        .into_iter()
        .enumerate()
        .filter(|(i, _)| set.contains(i) == keep_when_in)
        .map(|(_, m)| m)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizmo_nfs::types::{AssetHash, IDENTITY};
    use gizmo_nfs::world::WorldSolidHeader;

    fn object(name: &str, verts: usize, at: [f32; 3], size: f32) -> WorldMesh {
        WorldMesh {
            header: WorldSolidHeader {
                hash: AssetHash(0),
                name: name.to_string(),
                // NFSU2 is Z-up and `world_point` remaps; the test only needs the box to have a
                // place and an extent, so it states both in file order.
                bbox_min: [at[0], at[1], at[2]],
                bbox_max: [at[0] + size, at[1] + size, at[2] + size],
                matrix: IDENTITY,
            },
            positions: vec![[0.0; 3]; verts],
            normals: Vec::new(),
            colours: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
            groups: Vec::new(),
            texture_slots: Vec::new(),
        }
    }

    /// The names are the ones in the install, including the truncated tail.
    #[test]
    fn the_tier_token_is_read_at_a_name_boundary() {
        let t = tier_of("XB_3TOWERAPARTLK_1A_00").expect("a tier");
        assert_eq!(t.design, "XB_3TOWERAPARTLK");
        assert_eq!(t.letter, 'A');
        assert_eq!(t.tail, "_00");

        let t = tier_of("LOD_CASINOA_1Z_LL").expect("a tier");
        assert_eq!((t.design, t.letter, t.tail), ("LOD_CASINOA", 'Z', "_LL"));

        // End of string is a boundary too: not every name kept its instance number through the
        // name field's truncation.
        let t = tier_of("ARC_BLDING_D_1A").expect("a tier");
        assert_eq!((t.design, t.letter, t.tail), ("ARC_BLDING_D", 'A', ""));
    }

    /// A letter that merely follows `_1` is not a tier — these two names are in the install and
    /// grouping them as tiers would put unrelated objects in one family.
    #[test]
    fn a_letter_inside_a_word_is_not_a_tier() {
        assert!(tier_of("ARC_1WOOD4CONSTRUCTION").is_none());
        assert!(tier_of("L4RCS_1TOWER").is_none());
        assert!(tier_of("XB_PLAINBUILDING").is_none());
    }

    /// The tail is part of the family key. Without it, two placements of one design read as two
    /// tiers of one placement and the pass deletes a building that has no other copy.
    #[test]
    fn two_placements_of_one_design_are_two_families_not_one() {
        let objects = vec![
            object("XB_TOWER_1A_00", 200, [0.0, 0.0, 0.0], 10.0),
            object("XB_TOWER_1Z_00", 20, [0.0, 0.0, 0.0], 10.0),
            object("XB_TOWER_1A_01", 200, [500.0, 0.0, 0.0], 10.0),
            object("XB_TOWER_1Z_01", 20, [500.0, 0.0, 0.0], 10.0),
        ];
        let fams = families(&objects);
        assert_eq!(fams.len(), 2, "one family per placement");
        assert_eq!(coarser(&objects).len(), 2, "one coarse member each");
    }

    /// The 27-character name field merges two placements of a long-named design into one key. The
    /// repeated letter is the tell, and the whole key is refused rather than half-acted on: without
    /// this, one "family" spanned 2.3 km of city and a pass would have deleted the far end of it.
    #[test]
    fn a_key_that_holds_a_letter_twice_is_refused() {
        let objects = vec![
            object("XWU_REDWALL_1A_02_CHOP_C", 200, [0.0, 0.0, 0.0], 10.0),
            object("XWU_REDWALL_1Z_02_CHOP_C", 20, [0.0, 0.0, 0.0], 10.0),
            // The same truncated name again, 2 km away: a different wall the field could not name.
            object("XWU_REDWALL_1A_02_CHOP_C", 200, [2000.0, 0.0, 0.0], 10.0),
        ];
        assert!(families(&objects).is_empty(), "an unreadable key is not acted on");
        assert!(coarser(&objects).is_empty(), "and nothing is dropped for it");
        assert_eq!(ambiguous(&objects), (1, 3), "it is counted, not ignored");
    }

    /// A design that ships in one tier only is not a family, and nothing about it is dropped.
    #[test]
    fn a_lone_tier_is_not_a_family() {
        let objects = vec![
            object("XB_TOWER_1A_00", 200, [0.0, 0.0, 0.0], 10.0),
            object("XB_SHED_1A_00", 40, [50.0, 0.0, 0.0], 5.0),
        ];
        assert!(families(&objects).is_empty());
        assert!(coarser(&objects).is_empty());
    }

    /// The finest member is chosen by vertex count, not by the letter, because the letter is a
    /// convention and 46 of 1,423 consecutive pairs in the install do not follow it.
    #[test]
    fn the_finest_member_is_the_richest_one_not_the_first_letter() {
        let objects = vec![
            object("XB_TOWER_1A_00", 20, [0.0, 0.0, 0.0], 10.0),
            object("XB_TOWER_1Z_00", 200, [0.0, 0.0, 0.0], 10.0),
        ];
        let fams = families(&objects);
        assert_eq!(fams[0].finest().letter, 'Z');
        assert_eq!(coarser(&objects), HashSet::from([0]));
    }

    /// Stacked and apart are the two readings the decision turns on, so both have a test.
    #[test]
    fn a_family_knows_whether_its_members_share_a_spot() {
        let stacked = vec![
            object("XB_TOWER_1A_00", 200, [0.0, 0.0, 0.0], 20.0),
            object("XB_TOWER_1Z_00", 20, [0.0, 0.0, 0.0], 20.0),
        ];
        assert!(families(&stacked)[0].stacked());
        assert!(families(&stacked)[0].spread() < 0.01);

        // 40 m apart with a 20 m footprint: two spots, not one.
        let apart = vec![
            object("XB_TOWER_1A_00", 200, [0.0, 0.0, 0.0], 20.0),
            object("XB_TOWER_1Z_00", 20, [0.0, 40.0, 0.0], 20.0),
        ];
        assert!(!families(&apart)[0].stacked());
        assert!((families(&apart)[0].spread() - 40.0).abs() < 0.01);
    }

    #[test]
    fn keeping_the_finest_and_keeping_the_rest_partition_the_input() {
        let objects = || {
            vec![
                object("XB_TOWER_1A_00", 200, [0.0, 0.0, 0.0], 10.0),
                object("XB_TOWER_1B_00", 100, [40.0, 0.0, 0.0], 10.0),
                object("XB_TOWER_1Z_00", 20, [80.0, 0.0, 0.0], 10.0),
                object("XB_ROADA", 60, [0.0, 0.0, 0.0], 10.0),
            ]
        };
        let finest = keep_finest(objects());
        let coarse = keep_coarser(objects());
        assert_eq!(finest.len(), 2, "the richest tier and the object with no tier");
        assert_eq!(coarse.len(), 2, "the two coarser tiers");
        assert_eq!(finest.len() + coarse.len(), objects().len(), "nothing is in both or neither");
    }
}
