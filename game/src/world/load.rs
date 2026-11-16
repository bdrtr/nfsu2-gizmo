//! Getting a city off disk: which files, and what they hand over.
//!
//! Three binaries now want the same four things — the region bundles, their objects, the texture
//! packs that dress them, and the shared tiers the packs do not carry — and the order matters in a
//! way that is easy to get subtly wrong (the shared tiers are found by walking *up* from the
//! `TRACKS` directory, not from the current directory). It is written once here.

use super::SharedTextures;
use gizmo_nfs::world::{TrackPack, WorldMesh};
use std::path::{Path, PathBuf};

/// A loaded city: every region's objects and packs, plus the tiers shared between them.
pub struct Bundles {
    /// The files this came from, for a caller that wants to say how many.
    pub files: Vec<PathBuf>,
    /// Every region's objects, concatenated. Not filtered — [`super::is_drawn`] is the caller's
    /// call, because `nfs_city` counts what it is about to drop and the others do not.
    ///
    /// **The instanced models' own solids are not in here** — see [`instance`]. They stand at their
    /// authoring positions, and [`Self::scenery`] is where they really go.
    pub meshes: Vec<WorldMesh>,
    /// The copies the streaming sections place ([`instance`]): barriers, guard rails, lamps,
    /// trees. Already deduplicated and filtered with [`super::is_drawn`]. Kept apart from
    /// [`Self::meshes`] so that [`super::lod::keep_finest`] never sees them: one design is
    /// sometimes instanced through a `_1A` model and a `_1B` one, and the tier pass would delete
    /// every copy of the second.
    pub scenery: Vec<WorldMesh>,
    /// The same copies with all their detail levels, finest first — for a caller that draws them
    /// through a distance-chosen `LodGroup` (`world::build_lods`) rather than at full detail.
    /// Same order and filtering as [`Self::scenery`].
    pub scenery_lods: Vec<Vec<WorldMesh>>,
    /// What the game collides with, beyond [`Self::meshes`]: its own polygon soup
    /// (`gizmo_nfs::world::collision`, the fixed polygons only) cut into [`game_collision`]'s
    /// cells, plus the race barriers this event raises. **The rest of [`Self::scenery`] is drawn
    /// and not collided with** — trees, cat's-eyes, ground spotlights have no polygon in the game's
    /// soup, while its walls stand along every guard rail, fence and lamp post.
    /// `NFS_GAME_COLLISION=off` puts the old rule back: everything drawn is solid.
    pub collision: Vec<WorldMesh>,
    /// Every region's texture packs.
    pub packs: Vec<TrackPack<'static>>,
    /// `TRACKS/LOC4DYNTEX.BIN` and `GLOBAL/` — the keys no region carries itself. Empty rather than
    /// fatal when the install root cannot be found: that means grey walls, not a failure to start.
    pub shared: SharedTextures,
}

/// Every `STREAM*.BUN` at `path`, or `path` itself when it names one file.
///
/// Sorted, so two runs over the same directory load the same regions in the same order — a golden
/// screenshot needs that, and so does anyone comparing two runs' counts.
#[must_use]
pub fn bundles(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let Ok(dir) = std::fs::read_dir(path) else { return Vec::new() };
    let mut out: Vec<_> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|q| {
            q.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("STREAM") && n.ends_with(".BUN"))
        })
        .collect();
    out.sort();
    out
}

/// Read the whole city at `path` (a `TRACKS` directory, or one `STREAM*.BUN`).
///
/// **The bundles are not adjacent tiles.** They share one world coordinate system and overlap,
/// being per-race-route supersets of the same city, so a district's buildings and the roads under
/// them can live in different files and loading one alone leaves buildings hanging in the air.
/// Given a directory, this takes all of them.
///
/// The file bytes are **leaked on purpose**: a [`TrackPack`] borrows its pixel pool from them, and
/// this is the whole city staying loaded for the life of the process. Freeing them would mean
/// threading a lifetime through every caller to save memory at exit.
#[must_use]
pub fn load(path: &str) -> Bundles {
    let event = std::env::var("NFS_ROUTE").ok().and_then(|r| event_of(Path::new(&r)));
    load_event(path, event)
}

/// The race a route file belongs to: `Paths4102.bin` → `4102`. The number is what the game builds
/// its barrier-group names from (`BARRIERS_%d`).
#[must_use]
pub fn event_of(route: &Path) -> Option<u32> {
    let stem = route.file_stem()?.to_str()?;
    stem.strip_prefix("Paths")?.parse().ok()
}

/// [`load`] for a given race (`None`: free roam), rather than the one `NFS_ROUTE` names.
#[must_use]
pub fn load_event(path: &str, event: Option<u32>) -> Bundles {
    let files = bundles(Path::new(path));
    assert!(!files.is_empty(), "{path}: no STREAM*.BUN here");
    let loaded: &'static [Vec<u8>] = Box::leak(
        files
            .iter()
            .map(|f| std::fs::read(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display())))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let mut meshes = Vec::new();
    let mut scenery: Vec<Vec<WorldMesh>> = Vec::new();
    let mut collision = Vec::new();
    let mut packs = Vec::new();
    let game_collision_on = std::env::var("NFS_GAME_COLLISION").as_deref() != Ok("off");
    for (bytes, file) in loaded.iter().zip(&files) {
        if game_collision_on {
            collision.extend(game_collision(bytes));
        }
        let mut own = gizmo_nfs::world::meshes(bytes).expect("read a region's meshes");
        // NFS_SCENERY=off: the city as it was read before the instances were — every solid at its
        // own header, nothing copied. For A/B runs (the sweep), not for playing.
        if std::env::var("NFS_SCENERY").as_deref() != Ok("off") {
            let groups = region_beside(file).map(|r| shown_groups(&r, event)).unwrap_or_default();
            scenery.extend(instance_lods(bytes, &mut own, &groups).into_iter().filter(|l| !l.is_empty()));
        }
        meshes.extend(own);
        packs.extend(gizmo_nfs::world::packs(bytes).expect("read a region's texture packs"));
    }
    scenery.retain(|l| super::is_drawn(&l[0].header.name));
    // The same duplicate rule as `dedup`, decided on each copy's finest level.
    let mut seen = std::collections::HashSet::new();
    scenery.retain(|l| {
        let m = &l[0];
        seen.insert((m.header.hash.0, m.header.matrix.map(|row| row.map(f32::to_bits))))
    });
    let scenery_lods = scenery;
    let scenery: Vec<WorldMesh> = scenery_lods.iter().map(|l| l[0].clone()).collect();
    if game_collision_on {
        collision.extend(scenery.iter().filter(|m| is_race_barrier(&m.header.name)).cloned());
    } else {
        collision.extend(scenery.iter().cloned());
    }

    // The shared tiers live beside the install's `GLOBAL/`, which is above `TRACKS`. Walking up
    // from the path given is what makes this work whatever directory the binary was started in.
    let root = Path::new(path).ancestors().find(|a| a.join("GLOBAL").is_dir());
    let shared = root.map(SharedTextures::load).unwrap_or_default();

    Bundles { files, meshes, scenery, scenery_lods, collision, packs, shared }
}

/// Where in the city to start, from `NFS_AT="x,y,z"`, else `default`.
///
/// Bayview is 12 km across and almost all of it is not where you want to be, so every binary that
/// opens the city takes this. Anything unparseable falls back rather than failing: a typo in a
/// coordinate should put you at the default, not refuse to start.
#[must_use]
pub fn start_at(default: gizmo::prelude::Vec3) -> gizmo::prelude::Vec3 {
    std::env::var("NFS_AT")
        .ok()
        .and_then(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (v.len() == 3).then(|| gizmo::prelude::Vec3::new(v[0], v[1], v[2]))
        })
        .unwrap_or(default)
}

/// The `TRACKS` directory to read: an explicit argument, else `$NFSU2_ROOT/TRACKS`.
///
/// Its own function because the alternative is each binary inventing a different half of it, and
/// the install path is deliberately not written down anywhere in this repo.
#[must_use]
pub fn tracks_path(arg: Option<String>) -> String {
    arg.unwrap_or_else(|| {
        std::env::var("NFSU2_ROOT")
            .map(|r| format!("{r}/TRACKS"))
            .expect("no TRACKS directory given and NFSU2_ROOT is unset")
    })
}

/// The city bundle a route file belongs to.
///
/// `TRACKS/ROUTESL4RA/Paths4001.bin` → `TRACKS/STREAML4RA.BUN`. The directory name carries the
/// region and nothing else has to be looked up.
///
/// **Why this matters more than it looks.** The eight `STREAM*.BUN` are not eight versions of one
/// city and they are not adjacent tiles either: of the 13,986 distinct (object, position) pairs
/// across them, exactly **one** appears in all eight and **5,377 appear in only one**. Two of them
/// carry their own ground — `STREAML4RB` contributes 397 `TRN_GRASS` and 359 `TRN_RDP` pieces
/// nobody else has, `STREAML4RG` 542 and 340, each spread over some 3.7 × 8 km — so loading all
/// eight lays several route-specific terrain and road layers over the same map.
///
/// Measured under one race's 341 route nodes: with all eight loaded, 16 nodes have **three** road
/// surfaces stacked under them and 56 have two; with that route's own bundle alone, none have three
/// and 21 have two, and the height solve's worst step falls from 20.4 m to 13.1 m.
///
/// The cost is stated rather than hidden: alone, 53 of those nodes have no road-named object under
/// them at all against 10 with everything loaded. One bundle is not a complete world — choosing
/// *which* regions a race needs is what `ROADMAP.md` calls M5, and this is the one-line version of
/// it that a single race can be driven on.
#[must_use]
pub fn bundle_for_route(route: &std::path::Path) -> Option<std::path::PathBuf> {
    let dir = route.parent()?;
    let region = dir.file_name()?.to_str()?.strip_prefix("ROUTES")?;
    let bundle = dir.parent()?.join(format!("STREAM{region}.BUN"));
    bundle.is_file().then_some(bundle)
}

/// What kind of place a region is.
///
/// Three things had to line up before these labels were more than a guess, and they do:
/// which track numbers live in the region's `ROUTES` directory, how much free-roam data its
/// `PathsFreeRoam.bin` carries, and where its terrain actually sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Bayview itself — the free-roam city.
    City,
    /// The city again, near enough to be a copy, packed for a handful of its own races.
    CityAgain,
    /// A standalone venue: its own ground, its own patch of the coordinate system.
    Arena,
    /// Ships with the game, hosts no event, and is not part of the map.
    TestTrack,
}

/// The eight `STREAM*.BUN` regions and what each one is.
///
/// The counts are from the install and are what the labels rest on. `tracks` is the number of
/// `Paths####.bin` beside the bundle; `free_roam` the byte length of `PathsFreeRoam.bin`'s
/// `0x0003414A` region chunk; `ground` the extent of the bundle's own `TRN_*` terrain, backdrop
/// excluded, because the backdrop is fifteen kilometres wide and would drown the answer.
///
/// | region | tracks | numbers | free_roam | ground centre | ground |
/// |--------|--------|---------|-----------|---------------|--------|
/// | L4RA   | 60     | 40xx 41xx | 69,628  | (−540, 606)   | 5328 × 5188 |
/// | L4RB   |  9     | 42xx    |  1,700    | (490, −1250)  | 3741 × 3977 |
/// | L4RC   | 12     | 43xx    |    700    | (−157, 9)     | 1820 × 2571 |
/// | L4RD   |  3     | 44xx    | 32,164    | (−540, 606)   | 5328 × 5188 |
/// | L4RF   |  8     | 46xx    |    672    | (1193, 960)   | 1994 × 1507 |
/// | L4RG   | 13     | 47xx    |  1,600    | (458, −533)   | 3708 × 5411 |
/// | L4RH   |  0     | —       |  1,600    | (−1, 1)       |  435 × 1147 |
/// | L4RR   |  0     | —       |      0    | (0, 0)        | 2800 × 2800 |
///
/// **This is why the city looked like worlds stacked inside a skydome.** Six of the eight are not
/// parts of one map that tile together — they are separate places that share an origin. Load them
/// at once and L4RD lands exactly on top of L4RA while four arenas and two test tracks sit through
/// the middle of it. `L4RH` and `L4RR` give themselves away twice over: no events at all, and
/// terrain built out of `TRN_ROADA`/`TRN_TERRAINA` in a 2800 × 2800 square centred on the origin.
///
/// `L4RD` is the one real ambiguity. Its ground is the city's, to the metre, and it holds *more*
/// objects than `L4RA` (11,135 against 10,735) — but 70 of them are `ZPM_4401`/`ZPM_4402` props
/// for its own three races, and its free-roam chunk is less than half the size. So `L4RA` is the
/// city and `L4RD` is a race packing of it, not the other way round.
pub const REGIONS: [(&str, Place); 8] = [
    ("L4RA", Place::City),
    ("L4RB", Place::Arena),
    ("L4RC", Place::Arena),
    ("L4RD", Place::CityAgain),
    ("L4RF", Place::Arena),
    ("L4RG", Place::Arena),
    ("L4RH", Place::TestTrack),
    ("L4RR", Place::TestTrack),
];

/// What a region is, or `None` for a name that is not one.
#[must_use]
pub fn place_of(region: &str) -> Option<Place> {
    REGIONS.iter().find(|(k, _)| *k == region).map(|(_, p)| *p)
}

/// The bundle the free-roam city is made of.
///
/// One bundle, not a set: `STREAML4RA.BUN`. See [`REGIONS`] for what the other seven are and why
/// loading them together is what produced a city with several grounds.
///
/// The `PathsFreeRoam.bin` beside it is worth knowing about but is *not* a route: all eight of them
/// carry no node table — they are exactly the eight files in the install that don't — and hold only
/// the `0x0003414A` region chunk and `0x0003414D`. Both are byte-identical to the copies inside
/// every race file of the same region (60/60 in L4RA, and the same in the other five), so a race
/// file is its region's free-roam data with the AI network and the event catalogue appended.
/// Free roam has no path to follow, which is the point of it.
#[must_use]
pub fn free_roam_bundle(tracks: &std::path::Path) -> Option<std::path::PathBuf> {
    let bundle = tracks.join("STREAML4RA.BUN");
    bundle.is_file().then_some(bundle)
}

/// Place a region's instanced scenery, and take the models it instances out of `meshes`.
///
/// Each streaming section lists the models it uses and the copies it places
/// (`gizmo_nfs::world::scenery`). A copy here is the model's richest solid with its header matrix
/// replaced by the instance's, so everything downstream — [`super::place`], cells, collision —
/// treats it as one more placed object. Two cases are left alone:
///
/// - an instance whose model is a **baked** solid (identity matrix, vertices already in world
///   space) is that solid where it already is — 4,393 of `STREAML4RA`'s 48,270;
/// - a **mirrored** instance (negative determinant) gets its triangles' winding reversed, or the
///   backface cull would show it inside out.
///
/// The instanced models themselves (every non-baked solid an instance names) are removed from
/// `meshes`: by their own headers they stand at authoring positions — `(-8093, -527, -978)` is a
/// kilometre under the city — and that grid of stray copies is what `lod.rs` once measured as coarse
/// tiers "standing apart by up to 651 m on a line parallel to one world axis".
pub fn instance(bytes: &[u8], meshes: &mut Vec<WorldMesh>, groups: &Visibility) -> Vec<WorldMesh> {
    instance_lods(bytes, meshes, groups).into_iter().filter_map(|mut l| (!l.is_empty()).then(|| l.swap_remove(0))).collect()
}

/// [`instance`], with every detail level of each copy: finest first, as many as the model has
/// solids for (`_1A`, `_1B`, `_1Z`). Level 0 is what [`instance`] returns.
pub fn instance_lods(bytes: &[u8], meshes: &mut Vec<WorldMesh>, groups: &Visibility) -> Vec<Vec<WorldMesh>> {
    let Ok(sections) = gizmo_nfs::world::scenery::sections(bytes) else { return Vec::new() };
    let index: std::collections::HashMap<u32, usize> =
        meshes.iter().enumerate().map(|(i, m)| (m.header.hash.0, i)).rev().collect();
    let mut models = std::collections::HashSet::new();
    let mut out = Vec::new();
    for section in &sections {
        for (k, inst) in section.instances.iter().enumerate() {
            let Some(model) = section.models.get(inst.model) else { continue };
            let Some(&i) = model.lods.iter().flatten().find_map(|h| index.get(&h.0)) else { continue };
            let key = (section.number as u16, k as u16);
            let hidden = if groups.known {
                groups.hidden.contains(&key)
            } else {
                is_race_barrier(&meshes[i].header.name)
            };
            if hidden {
                // Still taken out of `meshes` below (it is an instanced model), just not placed.
                for h in model.lods.iter().flatten() {
                    models.insert(h.0);
                }
                continue;
            }
            for h in model.lods.iter().flatten() {
                if index.get(&h.0).is_some_and(|&j| meshes[j].header.is_placed()) {
                    models.insert(h.0);
                }
            }
            if !meshes[i].header.is_placed() {
                continue;
            }
            let mut levels = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for h in model.lods.iter().flatten() {
                let Some(&j) = index.get(&h.0) else { continue };
                if !meshes[j].header.is_placed() || !seen.insert(j) {
                    continue;
                }
                let mut copy = meshes[j].clone();
                copy.header.matrix = inst.matrix;
                if copy.header.basis_determinant() < 0.0 {
                    for tri in copy.indices.as_chunks_mut::<3>().0 {
                        tri.swap(1, 2);
                    }
                }
                levels.push(copy);
            }
            out.push(levels);
        }
    }
    meshes.retain(|m| !models.contains(&m.header.hash.0));
    out
}

/// Whether a model is one of the race barriers, `XO_TRACKBARRIER*` — 5,622 copies in `STREAML4RA`.
///
/// Only the fallback for a bundle whose region file is missing: [`shown_groups`] is the real rule.
/// Placing every event's barriers at once walls the courses off — the eight-route sweep fell from
/// 1,498 waypoints driven past to 449 with all of them in.
#[must_use]
pub fn is_race_barrier(name: &str) -> bool {
    name.to_ascii_uppercase().contains("TRACKBARRIER")
}

/// Which grouped instances to leave out, as `(section number, instance index)`.
#[derive(Debug, Default)]
pub struct Visibility {
    /// Whether the region's groups were read. Without them [`is_race_barrier`] stands in.
    pub known: bool,
    /// Members of a group this race (or free roam) does not show.
    pub hidden: std::collections::HashSet<(u16, u16)>,
}

/// The region file beside a stream bundle: `STREAML4RA.BUN` → `L4RA.BUN`.
#[must_use]
pub fn region_beside(bundle: &Path) -> Option<Vec<u8>> {
    let name = bundle.file_name()?.to_str()?;
    let region = name.strip_prefix("STREAM")?;
    std::fs::read(bundle.with_file_name(region)).ok()
}

/// The game's scenery groups for one race, from a region file (`L4R?.BUN`, chunk `0x34108`).
///
/// `SPEED2.EXE` raises `BARRIERS_<race>` and `PLAYER_BARRIERS_<race>` (names built at `0x578060`
/// and `0x5F54A0`) — the barriers that close that race's side streets and its own start and finish
/// props. Every other race's are down, and so are the `FREE_ROAM*` groups during a race. In free
/// roam (`event` = `None`) it is the reverse for `FREE_ROAM`, and every race's barriers are down.
/// Groups this does not name (`SMOKEABLE`, the breakables) are always shown.
#[must_use]
pub fn shown_groups(region: &[u8], event: Option<u32>) -> Visibility {
    let Ok(groups) = gizmo_nfs::world::scenery::groups(region) else { return Visibility::default() };
    let wanted = |name: &str| match event {
        Some(n) => name == format!("BARRIERS_{n}") || name == format!("PLAYER_BARRIERS_{n}"),
        None => name == "FREE_ROAM",
    };
    let switchable = |name: &str| {
        name.starts_with("BARRIERS") || name.starts_with("PLAYER_BARRIERS") || name.starts_with("FREE_ROAM")
    };
    let mut shown = std::collections::HashSet::new();
    let mut hidden = std::collections::HashSet::new();
    for (g, members) in &groups {
        if !switchable(&g.name) {
            continue;
        }
        if wanted(&g.name) {
            shown.extend(members.iter().copied());
        } else {
            hidden.extend(members.iter().copied());
        }
    }
    // An instance in a shown group and a hidden one (a barrier two races share) is shown.
    hidden.retain(|k| !shown.contains(k));
    Visibility { known: true, hidden }
}

/// Side of the square cells [`game_collision`] cuts the polygon soup into, in metres. Smaller than
/// the 256 m collision cell so that no piece straddles more than its neighbours.
const SOUP_CELL: f32 = 128.0;

/// The game's own fixed collision polygons in a stream bundle, as meshes `collision_cells` can
/// take: one per [`SOUP_CELL`] square, vertices already in world space (identity matrix).
///
/// Switchable polygons (`switch != 0`) are left out — 70 % of them stand in race barriers, which
/// come in as the barrier instances this event raises instead.
#[must_use]
pub fn game_collision(bundle: &[u8]) -> Vec<WorldMesh> {
    use gizmo_nfs::types::{AssetHash, IDENTITY};
    let Ok(mut polys) = gizmo_nfs::world::collision::polygons(bundle) else { return Vec::new() };
    let leveled = if std::env::var("NFS_LEVEL_ENDLESS").as_deref() == Ok("off") { 0 } else { level_endless(&mut polys) };
    if std::env::var("NFS_DIAG").is_ok() {
        println!("game collision: {leveled} endless walls given the height of the floor they edge");
    }
    type Piece = (Vec<[f32; 3]>, Vec<u32>);
    let mut cells: std::collections::BTreeMap<(i32, i32), Piece> = std::collections::BTreeMap::new();
    for p in polys.iter().filter(|p| p.switch == 0) {
        let n = p.corners.len() as f32;
        let cx = p.corners.iter().map(|c| c[0]).sum::<f32>() / n;
        let cy = p.corners.iter().map(|c| c[1]).sum::<f32>() / n;
        let key = ((cx / SOUP_CELL).floor() as i32, (cy / SOUP_CELL).floor() as i32);
        let (pos, idx) = cells.entry(key).or_default();
        let base = pos.len() as u32;
        pos.extend(p.corners.iter().copied());
        // Both windings: the soup's facing is not known, and a one-sided wall lets a car through
        // from behind.
        for t in 1..p.corners.len() as u32 - 1 {
            idx.extend([base, base + t, base + t + 1, base, base + t + 1, base + t]);
        }
    }
    cells
        .into_iter()
        .map(|((cx, cy), (positions, indices))| {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for q in &positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(q[k].clamp(-1000.0, 1000.0));
                    hi[k] = hi[k].max(q[k].clamp(-1000.0, 1000.0));
                }
            }
            let name = format!("GAME_COLLISION_{cx}_{cy}");
            WorldMesh {
                header: gizmo_nfs::world::WorldSolidHeader {
                    hash: AssetHash(gizmo_nfs::hash::string_hash(&name)),
                    name,
                    bbox_min: lo,
                    bbox_max: hi,
                    matrix: IDENTITY,
                },
                normals: Vec::new(),
                colours: Vec::new(),
                uvs: Vec::new(),
                groups: Vec::new(),
                texture_slots: Vec::new(),
                indices,
                positions,
            }
        })
        .collect()
}


/// How far below and above the floor it edges an endless wall is made to reach, in metres.
const ENDLESS_BELOW: f32 = 2.0;
const ENDLESS_ABOVE: f32 = 8.0;

/// Give the game's endless walls a height.
///
/// An endless wall (`flags & 0x04`) carries no height at all — its base field is 0 in every one of
/// `STREAML4RA`'s 21,699 — and `SPEED2.EXE` stretches it to ±2,048 m. Taken literally that makes
/// the guard rail of a road under a bridge a wall across the bridge: 56 of the 83 places where the
/// rails' line still crossed a wall in `Paths4104`/`Paths4401` were such a wall, its floor far
/// below the line. The game keeps them apart through its own spatial tree (`0x34135`–`0x34137`,
/// not read).
///
/// What the soup does say is which floor a wall runs along: 16,536 of the 21,699 lie on an edge of
/// a floor polygon at one level. Those are cut to [`ENDLESS_BELOW`] under and [`ENDLESS_ABOVE`]
/// over that floor; the rest (320 at several levels, 4,843 beside no floor edge) stay endless.
/// Returns how many were cut.
fn level_endless(polys: &mut [gizmo_nfs::world::collision::CollisionPolygon]) -> usize {
    const CELL: f32 = 8.0;
    const NEAR: f32 = 0.6;
    let key = |x: f32, y: f32| ((x / CELL).floor() as i32, (y / CELL).floor() as i32);
    let mut floors: std::collections::HashMap<(i32, i32), Vec<usize>> = Default::default();
    for (i, p) in polys.iter().enumerate() {
        if p.switch != 0 || p.is_endless() || p.uprightness() < 0.5 {
            continue;
        }
        let (lo, hi) = p.corners.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), c| {
            ([lo[0].min(c[0]), lo[1].min(c[1])], [hi[0].max(c[0]), hi[1].max(c[1])])
        });
        let (a, b) = (key(lo[0], lo[1]), key(hi[0], hi[1]));
        for x in a.0..=b.0 {
            for y in a.1..=b.1 {
                floors.entry((x, y)).or_default().push(i);
            }
        }
    }
    let dist = |p: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx * dx + dy * dy;
        let t = if l > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l).clamp(0.0, 1.0) } else { 0.0 };
        (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
    };
    let mut levels: Vec<(usize, f32, f32)> = Vec::new();
    for (i, w) in polys.iter().enumerate() {
        if w.switch != 0 || !w.is_endless() {
            continue;
        }
        // The wall's foot: its two distinct plan points.
        let mut foot: Vec<[f32; 2]> = Vec::new();
        for c in &w.corners {
            let q = [c[0], c[1]];
            if foot.iter().all(|f| (f[0] - q[0]).hypot(f[1] - q[1]) > 0.05) {
                foot.push(q);
            }
        }
        if foot.len() < 2 {
            continue;
        }
        let (a, b) = (foot[0], foot[1]);
        let mid = key((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5);
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for &k in floors.get(&mid).into_iter().flatten() {
            let f = &polys[k].corners;
            for e in 0..f.len() {
                let (p, q) = (f[e], f[(e + 1) % f.len()]);
                let (pp, qq) = ([p[0], p[1]], [q[0], q[1]]);
                let along = (dist(pp, a, b) < NEAR && dist(qq, a, b) < NEAR)
                    || (dist(a, pp, qq) < NEAR && dist(b, pp, qq) < NEAR);
                if along {
                    lo = lo.min(p[2].min(q[2]));
                    hi = hi.max(p[2].max(q[2]));
                }
            }
        }
        if lo <= hi && hi - lo < 4.0 {
            levels.push((i, lo, hi));
        }
    }
    for &(i, lo, hi) in &levels {
        for c in &mut polys[i].corners {
            c[2] = if c[2] < 0.0 { lo - ENDLESS_BELOW } else { hi + ENDLESS_ABOVE };
        }
    }
    levels.len()
}
