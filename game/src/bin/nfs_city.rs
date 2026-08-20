//! Headless one-frame render of a city region, for visual QA of the world path.
//!
//! `nfs_shot` does this for a car. This does it for a `TRACKS/STREAM*.BUN`: read the region's
//! objects and its texture packs, merge them per (cell, texture), and render one frame from above
//! into an offscreen target. No window, so it works over SSH and in CI.
//!
//! Usage: `nfs_city <STREAM*.BUN | TRACKS-dir> OUT.raw [W H]`
//!
//! Given the `TRACKS` directory it loads every region at once. The bundles are not adjacent tiles
//! — they share one world coordinate system and overlap, being per-race-route supersets of the same
//! city — so a district's buildings and the roads under them can live in different files, and
//! loading one alone leaves the buildings hanging in the air.
//!
//! Env:
//! - `NFS_BUDGET=<n>` — take only the `n` objects nearest the region's centre. The throwaway that
//!   makes `STREAML4RA`'s 10,735 objects usable before streaming exists; delete it with the rest of
//!   the radius filter.
//! - `NFS_AT="x,y,z"` — what to look at, in world space. Defaults to the centre of what loaded,
//!   which is only useful for one region; with the whole `TRACKS` directory the centre is a point
//!   in the middle of eight overlapping districts.
//! - `NFS_EYE="x,y,z"` — camera eye, relative to the look-at point. Default is a high
//!   three-quarter view framing the whole of what loaded.
//! - `NFS_ONLY=<substr>` — draw only the objects whose name contains it.
//! - `NFS_TIERS=all|coarse` — put the coarse detail tiers back, or draw *only* them. Only the
//!   richest member of each family is drawn by default; see [`nfsu2::world::lod`].
//! - `NFS_TIERS_LIST=<n>` — the detail families in numbers, and the n widest by member spread.
//! - `NFS_ROUTE=<Paths*.bin>` — put that route file's race line on the city and draw it as a
//!   ribbon. Prints what the placement cost: how many points took a neighbour's height, the line's
//!   length, and the worst step up, which is the number that shows the surface choice going wrong.
//!   `NFS_ROUTE_DUMP=1` adds the first ten nodes with every surface the city offers under them,
//!   road-only and unfiltered, which is how the two were told apart.
//! - `NFS_GRIDS=1` — hold every start marker against the city under it. The marker is the only
//!   record in these files with a height and nothing had checked it: 140 of the 141 race grids
//!   with a road beneath them sit within **1 m** of what the ground answers (median −0.0 m), and
//!   22 of free roam's 24 lone markers do too. That is what says the field is a height and `remap`
//!   is right. It also names the exceptions, which is how the free-roam grid's 30 m was found.
//! - `NFS_PROBE=<dir>` — ask the city's collision triangles what is under a list of world points
//!   (`<dir>/nodes.csv`, one `x,y` per line) and beside a list of segments (`<dir>/segments.csv`,
//!   `x1,y1,x2,y2`), both in the world's own frame. Written for one undecoded chunk and kept
//!   because it carries its own control: the points are a set that must come out on the road, so a
//!   run that fails there says the reading is wrong before any other row is believed.
//!
//! Prints one line of counts before rendering, because most of what can go wrong here is visible
//! in them: an object count that does not match the manifest, a duplicate count of zero (dedup not
//! running), or an unresolved-run count in the thousands (texture packs not found).

use gizmo::prelude::*;
use gizmo::renderer::Renderer;
use gizmo::wgpu;
use gizmo_nfs::types::AssetHash;
use nfsu2::scene::{self, Textures};
use nfsu2::world::{build_region, CityVisuals};
use std::collections::HashMap;

/// How much of a texture may be see-through before it is *left opaque*, as a share. **0 is off**,
/// which is the shipped value, and the reason is measured rather than chosen.
///
/// Bayview's foliage is flat cards with the tree cut out of a quad, and the city is spawned
/// opaque, so those cards render as **dark slabs standing in the air** — which is what a player
/// reports as objects floating in the sky. The engine offers this path exactly one lever,
/// `Material::with_transparent`, which is alpha *blending*; the alpha *test* that a cut-out
/// actually wants (`discard` on `alpha < cutoff`) exists in the renderer's G-buffer shader but is
/// reachable only from the glTF loader, and `with_baked_lit` does not go through the G-buffer.
///
/// Blending fixes the trees and costs the decals. A decal — the circular paving pattern, road
/// grime, the lane markings' overlay — is coplanar with the surface under it, and the transparent
/// pass will not hold it there: it vanishes and the plaza reads as flat grey. Measured over
/// `STREAML4RA`'s 1,500 textures, 235 carry transparency and their shares run smoothly from 0 to
/// 100 % (quartiles 48 / 68 / 82 %), so **no threshold separates foliage from decals** — at 11
/// textures the decals are right and the trees are slabs, at 235 the trees are right and the
/// plaza is grey, and every value between is one or the other.
///
/// So this ships off, the black slabs stay, and what the city needs from the engine is one field:
/// an alpha cutoff on the baked-lit material. `NFS_CUT=1` shows the other side of the trade.
const CUT_SHARE: f32 = 0.0;

fn main() {
    let path = std::env::args().nth(1).expect("usage: nfs_city STREAM*.BUN OUT.raw [W H]");
    let out = std::env::args().nth(2).expect("usage: nfs_city STREAM*.BUN OUT.raw [W H]");
    let w: u32 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1024);
    let h: u32 = std::env::args().nth(4).and_then(|s| s.parse().ok()).unwrap_or(768);
    pollster::block_on(run(&path, &out, w, h));
}

async fn run(path: &str, out: &str, w: u32, h: u32) {
    assert!(Renderer::headless_adapter_available().await, "no GPU adapter for headless render");
    let mut renderer = Renderer::new_headless(w, h, None).await;
    let mut world = World::new();
    let mut assets = AssetManager::new();

    // Every bundle's bytes stay alive for the whole run: a pack borrows its pixel pool from them.
    let files = bundles(path);
    assert!(!files.is_empty(), "{path}: no STREAM*.BUN here");
    let loaded: Vec<Vec<u8>> = files
        .iter()
        .map(|f| std::fs::read(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display())))
        .collect();

    let mut meshes = Vec::new();
    let mut packs = Vec::new();
    for bytes in &loaded {
        meshes.extend(gizmo_nfs::world::meshes(bytes).expect("read a region's meshes"));
        packs.extend(gizmo_nfs::world::packs(bytes).expect("read a region's texture packs"));
    }
    println!("{} bundle(s), {} objects, {} packs", loaded.len(), meshes.len(), packs.len());

    // NFS_BUNDLES=1: what each of the eight regions is. They are not eight versions of one city
    // and not eight tiles of it either — one is Bayview, one is Bayview again, four are standalone
    // venues and two are test tracks, and all of them share an origin. `nfsu2::world::REGIONS`
    // carries the table this diagnostic produced; the ground extent below is the measurement that
    // settles it, with the backdrop excluded because it is fifteen kilometres wide.
    if std::env::var("NFS_BUNDLES").is_ok() {
        let key = |m: &gizmo_nfs::world::WorldMesh| {
            let t = m.header.matrix[3];
            (m.header.hash.0, t[0].to_bits(), t[1].to_bits(), t[2].to_bits())
        };
        let mut per_file: Vec<(String, std::collections::HashSet<_>)> = Vec::new();
        let mut by_hash: HashMap<u32, std::collections::HashSet<(u32, u32, u32)>> = HashMap::new();
        for (f, bytes) in files.iter().zip(&loaded) {
            let ms = gizmo_nfs::world::meshes(bytes).expect("read a region's meshes");
            let mut set = std::collections::HashSet::new();
            for m in &ms {
                set.insert(key(m));
                let t = m.header.matrix[3];
                by_hash
                    .entry(m.header.hash.0)
                    .or_default()
                    .insert((t[0].to_bits(), t[1].to_bits(), t[2].to_bits()));
            }
            per_file.push((f.file_stem().unwrap_or_default().to_string_lossy().into_owned(), set));
        }
        let union: std::collections::HashSet<_> =
            per_file.iter().flat_map(|(_, s)| s.iter().copied()).collect();
        let shared_by_all = union
            .iter()
            .filter(|k| per_file.iter().all(|(_, s)| s.contains(k)))
            .count();
        let only_one = union
            .iter()
            .filter(|k| per_file.iter().filter(|(_, s)| s.contains(k)).count() == 1)
            .count();
        let multi_placed = by_hash.values().filter(|v| v.len() > 1).count();
        println!(
            "bundles: union {} distinct (hash, position) · in all {} files {} · in exactly one {} \
             · designs standing at more than one position {}",
            union.len(),
            per_file.len(),
            shared_by_all,
            only_one,
            multi_placed
        );
        // What a bundle holds that no other does is the question that decides whether loading all
        // eight is completeness or superposition: a district nobody else covers is the former, a
        // race's own dressing is the latter.
        for ((name, set), bytes) in per_file.iter().zip(&loaded) {
            let own: std::collections::HashSet<_> = set
                .iter()
                .filter(|k| per_file.iter().filter(|(_, s)| s.contains(*k)).count() == 1)
                .copied()
                .collect();
            let ms = gizmo_nfs::world::meshes(bytes).expect("read a region's meshes");
            let mut families: HashMap<String, usize> = HashMap::new();
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for m in ms.iter().filter(|m| own.contains(&key(m))) {
                let stem: String = m.header.name.split('_').take(2).collect::<Vec<_>>().join("_");
                *families.entry(stem).or_default() += 1;
                if !m.positions.is_empty() {
                    let c = nfsu2::world::world_point(&m.header, m.header.bbox_min)
                        .midpoint(nfsu2::world::world_point(&m.header, m.header.bbox_max));
                    lo = lo.min(c);
                    hi = hi.max(c);
                }
            }
            // Where the whole bundle sits, not just what it alone carries: a streamed city is
            // regions, and a region is a place before it is a set of objects.
            let (mut flo, mut fhi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            // Ground only. The backdrop is tens of kilometres wide and would drown the answer;
            // what is being asked is where you can drive, and that is the terrain family.
            for m in ms.iter().filter(|m| !m.positions.is_empty() && m.header.name.starts_with("TRN_")) {
                let c = nfsu2::world::world_point(&m.header, m.header.bbox_min)
                    .midpoint(nfsu2::world::world_point(&m.header, m.header.bbox_max));
                flo = flo.min(c);
                fhi = fhi.max(c);
            }
            let mid = (flo + fhi) * 0.5;
            let mut top: Vec<_> = families.into_iter().collect();
            top.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            let span = if own.is_empty() { Vec3::ZERO } else { hi - lo };
            let place = name
                .strip_prefix("STREAM")
                .and_then(|k| k.split('.').next())
                .and_then(nfsu2::world::place_of)
                .map_or("?", |p| match p {
                    nfsu2::world::Place::City => "SEHIR (free roam)",
                    nfsu2::world::Place::CityAgain => "sehir, yeniden",
                    nfsu2::world::Place::Arena => "ayri mekan",
                    nfsu2::world::Place::TestTrack => "test pisti",
                });
            println!(
                "  {name:<12} {place:<18} {:>6} objects · centre ({:>6.0},{:>6.0}) ground {:>5.0}x{:<5.0} m \
                 · {:>5} only here, spread {:.0}x{:.0} m · {}",
                ms.len(),
                mid.x,
                mid.z,
                fhi.x - flo.x,
                fhi.z - flo.z,
                own.len(),
                span.x,
                span.z,
                top.iter().take(4).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(" ")
            );
        }
    }

    // NFS_BACKDROP=1: draw the sky shell and the panorama panels too, through the engine's
    // `MaterialType::BackdropPlaced` — drawn first, depth writes off, and **not** camera-locked.
    // The comment used to name `Backdrop`, the locked variant, which is the one items 7 and 9
    // measured and rejected: NFSU2's panorama is world-placed geometry twelve to eighteen
    // kilometres across, and locking it to the camera pastes a three-kilometre panel onto the
    // lens (move the camera 1000 m and 45.4 % of the top-left quadrant stayed identical; with
    // the placed variant that is 0.0 %). The code below has always called
    // `with_backdrop_placed` — only this line was wrong.
    //
    // Off by default so every existing measurement here keeps meaning what it meant, and because
    // this binary's job is the *city*: a backdrop that spans fifteen kilometres would dominate any
    // frame statistic taken to answer a question about a street. On, it is the instrument
    // `MOTOR-NOTLARI.md` item 7 has been waiting for — the game's own painted backdrop either
    // reaches the screen or it does not, and a frame either shows that or it does not.
    let want_backdrop = std::env::var("NFS_BACKDROP").is_ok_and(|v| v != "0");
    let backdrop: Vec<gizmo_nfs::world::WorldMesh> = if want_backdrop {
        meshes.iter().filter(|m| nfsu2::world::is_backdrop(&m.header.name)).cloned().collect()
    } else {
        Vec::new()
    };
    let sky = meshes.iter().filter(|m| nfsu2::world::is_backdrop(&m.header.name)).count();
    let lod = meshes.iter().filter(|m| nfsu2::world::is_distant_lod(&m.header.name)).count();
    meshes.retain(|m| {
        !nfsu2::world::is_backdrop(&m.header.name) && !nfsu2::world::is_distant_lod(&m.header.name)
    });
    let declared = meshes.len();

    // NFS_ONLY=<substr>: draw nothing but the objects whose name contains it. `nfs_shot` has the
    // same switch for a car's parts, and for the same reason — a family of objects is only really
    // comparable with the rest of the city out of the way.
    if let Ok(pat) = std::env::var("NFS_ONLY") {
        meshes.retain(|m| m.header.name.contains(&pat));
        println!("NFS_ONLY={pat:?}: {} objects", meshes.len());
    }

    // NFS_TOP=<n>: name the n objects with the largest extent, which is how you find out what a
    // frame-filling surface actually is instead of guessing at it.
    if let Ok(n) = std::env::var("NFS_TOP").map(|v| v.parse::<usize>().unwrap_or(20)) {
        let mut by_size: Vec<(f32, &str, usize)> = meshes
            .iter()
            .filter(|m| !m.positions.is_empty())
            .map(|m| {
                let lo = nfsu2::world::world_point(&m.header, m.header.bbox_min);
                let hi = nfsu2::world::world_point(&m.header, m.header.bbox_max);
                ((hi - lo).length(), m.header.name.as_str(), m.positions.len())
            })
            .collect();
        by_size.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (span, name, verts) in by_size.iter().take(n) {
            println!("  span {span:>10.0}  verts {verts:>7}  {name}");
        }
    }

    // The shared tiers: `TRACKS/LOC4DYNTEX.BIN` and `GLOBAL/`, found from the TRACKS directory's
    // parent. Missing means grey walls, not a failure to start.
    let root = std::path::Path::new(&path)
        .ancestors()
        .find(|a| a.join("GLOBAL").is_dir())
        .map(std::path::Path::to_path_buf);
    let shared = root.map(|r| nfsu2::world::SharedTextures::load(&r)).unwrap_or_default();
    println!("{} shared textures", shared.len());

    // NFS_NEAR=<radius>: name the objects whose box contains or nearly contains the look-at
    // point, with their resolved texture. For finding out what a surface actually is.
    if let Ok(r) = std::env::var("NFS_NEAR").map(|v| v.parse::<f32>().unwrap_or(40.0)) {
        let at = std::env::var("NFS_AT")
            .ok()
            .and_then(|s| vec3_of(&s))
            .unwrap_or(Vec3::ZERO);
        let mut near: Vec<(f32, &str, usize, String)> = meshes
            .iter()
            .filter(|m| !m.positions.is_empty())
            .filter_map(|m| {
                let lo = nfsu2::world::world_point(&m.header, m.header.bbox_min);
                let hi = nfsu2::world::world_point(&m.header, m.header.bbox_max);
                let c = (lo + hi) * 0.5;
                // Containment on the ground plane, not centre distance: a terrain plane centred a
                // kilometre away is still the thing under your feet.
                let inside = at.x >= lo.x.min(hi.x) - r
                    && at.x <= lo.x.max(hi.x) + r
                    && at.z >= lo.z.min(hi.z) - r
                    && at.z <= lo.z.max(hi.z) + r;
                let d = (c - at).length();
                inside.then(|| {
                    let keys: Vec<String> = m
                        .texture_slots
                        .iter()
                        .map(|k| {
                            let own = packs.iter().any(|p| p.get(*k).is_some());
                            let sh = shared.get(*k).is_some();
                            format!("{:08X}{}", k.0, if own { "" } else if sh { "^shared" } else { "^MISSING" })
                        })
                        .collect();
                    (d, m.header.name.as_str(), m.positions.len(), keys.join(" "))
                })
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (d, name, v, keys) in near.iter().take(16) {
            println!("  d={d:>7.0}  v={v:>6}  {name:<28} {keys}");
        }
    }

    // NFS_FIND=<substr>: how many loaded objects match, and where the first few are — for
    // answering "is the road even here" without flying around looking for it.
    if let Ok(pat) = std::env::var("NFS_FIND") {
        let hits: Vec<&gizmo_nfs::world::WorldMesh> =
            meshes.iter().filter(|m| m.header.name.contains(&pat)).collect();
        println!("{} objects match {pat:?}", hits.len());
        for m in hits.iter().take(6) {
            let lo = nfsu2::world::world_point(&m.header, m.header.bbox_min);
            let hi = nfsu2::world::world_point(&m.header, m.header.bbox_max);
            let c = (lo + hi) * 0.5;
            println!("  {:<28} centre {:.0},{:.0},{:.0}  v={}", m.header.name, c.x, c.y, c.z, m.positions.len());
        }
    }

    // Dedup here rather than leaving it to `build_region`, because the tier measurement below has
    // to see each placement once: a byte-identical repeat of `_1A_00` would otherwise read as a
    // second member of its own family, standing exactly where the first one does, and report the
    // families as stacked when they are not.
    let meshes = nfsu2::world::dedup(meshes);
    let duplicates = declared - meshes.len();
    println!("{}", nfsu2::world::lod::report(&meshes));

    // NFS_TIERS_LIST=<n>: how far apart the families stand, and the n widest by name. A summary
    // that says "1,330 apart" cannot tell a 40 m row of towers from a key that merged two ends of
    // the city, and the difference decides whether a tier pass is safe.
    if let Ok(n) = std::env::var("NFS_TIERS_LIST").map(|v| v.parse::<usize>().unwrap_or(12)) {
        let mut fams = nfsu2::world::lod::families(&meshes);
        let mut bands = [0usize; 5];
        for f in &fams {
            let s = f.spread();
            bands[usize::from(s >= 1.0)
                + usize::from(s >= 10.0)
                + usize::from(s >= 50.0)
                + usize::from(s >= 200.0)] += 1;
        }
        println!(
            "  spread: <1m {} · 1-10m {} · 10-50m {} · 50-200m {} · >200m {}",
            bands[0], bands[1], bands[2], bands[3], bands[4]
        );
        // Does the member stand on the ground, or is it parked? A building's underside meets the
        // surface under it; a copy put somewhere out of the way does not. Asked of the finest tier
        // and of the coarser ones separately, because the whole question is whether they differ.
        let ground = nfsu2::world::Ground::of(&nfsu2::world::collision_cells(&meshes));
        let band = |members: &mut dyn Iterator<Item = &nfsu2::world::lod::Member>, what: &str| {
            let (mut on, mut off, mut unknown) = (0usize, 0usize, 0usize);
            let mut worst = 0.0f32;
            for m in members {
                let base = m.centre.y - m.size.y * 0.5;
                match ground.height_at(m.centre) {
                    None => unknown += 1,
                    Some(h) => {
                        let d = base - h;
                        // A building may be sunk into its terrain; 8 m either way is the tolerance
                        // that calls a plinth "on the ground" and a buried tower "not".
                        if d.abs() <= 8.0 {
                            on += 1;
                        } else {
                            off += 1;
                            if d.abs() > worst.abs() {
                                worst = d;
                            }
                        }
                    }
                }
            }
            println!(
                "  {what}: {on} on the ground, {off} off it (worst {worst:+.0} m), \
                 {unknown} over no drivable ground"
            );
        };
        band(&mut fams.iter().map(|f| &f.members[0]), "finest ");
        band(&mut fams.iter().flat_map(|f| f.members.iter().skip(1)), "coarser");

        // Does the member stand *through* another building? Two buildings do not interpenetrate,
        // so a tier that does is not where the city meant it to be. Asked against `XB_*` only —
        // roads, terrain and props legitimately pass through a building's box.
        let blocks: Vec<(Vec3, Vec3, usize)> = meshes
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.positions.is_empty() && m.header.name.starts_with("XB_"))
            .map(|(i, m)| {
                let lo = nfsu2::world::world_point(&m.header, m.header.bbox_min);
                let hi = nfsu2::world::world_point(&m.header, m.header.bbox_max);
                ((lo + hi) * 0.5, (hi - lo).abs(), i)
            })
            .collect();
        let family_of: std::collections::HashMap<usize, usize> = fams
            .iter()
            .enumerate()
            .flat_map(|(fi, f)| f.members.iter().map(move |m| (m.index, fi)))
            .collect();
        let clashes = |members: &mut dyn Iterator<Item = (usize, &nfsu2::world::lod::Member)>, what: &str| {
            let (mut hit, mut total) = (0usize, 0usize);
            for (fi, m) in members {
                total += 1;
                // 2 m of mutual penetration on every axis: touching facades are not a clash.
                let deep = blocks.iter().any(|(c, s, i)| {
                    *i != m.index
                        && family_of.get(i) != Some(&fi)
                        && (*c - m.centre).abs().cmplt((*s + m.size) * 0.5 - Vec3::splat(2.0)).all()
                });
                if deep {
                    hit += 1;
                }
            }
            let pct = if total == 0 { 0.0 } else { 100.0 * hit as f32 / total as f32 };
            println!("  {what}: {hit} of {total} ({pct:.0} %) stand inside another XB_ building");
        };
        clashes(&mut fams.iter().enumerate().map(|(fi, f)| (fi, &f.members[0])), "finest ");
        clashes(
            &mut fams.iter().enumerate().flat_map(|(fi, f)| f.members.iter().skip(1).map(move |m| (fi, m))),
            "coarser",
        );

        fams.sort_by(|a, b| b.spread().total_cmp(&a.spread()));
        for f in fams.iter().take(n) {
            println!("  spread {:>6.0} m  {}{}", f.spread(), f.design, f.tail);
            for m in &f.members {
                println!(
                    "      _1{}  v={:<6} {}  #{:<6} centre {:>7.0},{:>5.0},{:>7.0}  size {:>5.0}x{:>4.0}x{:>5.0}",
                    m.letter,
                    m.vertices,
                    if m.placed { "placed  " } else { "identity" },
                    m.index,
                    m.centre.x, m.centre.y, m.centre.z,
                    m.size.x, m.size.y, m.size.z
                );
            }
        }
    }

    // NFS_PROBE=<dir>: ask the city what is underneath a list of world points and segments.
    //
    // A throwaway for one question — whether the route files' `0x0003414D` segments are the edge
    // of the road — and built so the answer can be trusted: `nodes.csv` is the **positive
    // control**. Those are the race line, which must come out on drivable surface. If it does not,
    // the frame conversion below is wrong and nothing the segment rows say means anything.
    if let Ok(dir) = std::env::var("NFS_PROBE") {
        probe(&dir, &meshes);
    }

    // Only the richest member of each detail family, by default — see [`nfsu2::world::lod`] for
    // what the coarse ones turned out to be up close. `NFS_TIERS=all` puts them back and
    // `NFS_TIERS=coarse` draws *only* them, which is the frame that shows what is being left out.
    let meshes = match std::env::var("NFS_TIERS").ok().as_deref() {
        Some("all") => meshes,
        Some("coarse") => nfsu2::world::lod::keep_coarser(meshes),
        _ => nfsu2::world::lod::keep_finest(meshes),
    };

    // NFS_VCOL=1: the distribution of the city's baked vertex colour.
    //
    // The city's whole lighting is this one number per vertex — `(vcol · shadow + ambient) ·
    // albedo · texture` — and the window textures peak at 250/255, so anything dark on screen is
    // dark because of what is here. Worth a census rather than an assumption: a decode that read
    // the wrong bytes would show as a distribution pinned near zero or near 255, and an authored
    // night would show as a spread.
    if std::env::var("NFS_VCOL").is_ok() {
        let mut lum: Vec<u8> = Vec::new();
        let mut alpha: Vec<u8> = Vec::new();
        for m in &meshes {
            for c in &m.colours {
                lum.push(
                    ((u32::from(c[0]) * 299 + u32::from(c[1]) * 587 + u32::from(c[2]) * 114) / 1000)
                        as u8,
                );
                alpha.push(c[3]);
            }
        }
        lum.sort_unstable();
        alpha.sort_unstable();
        let q = |v: &[u8], f: f64| v.get((v.len() as f64 * f) as usize).copied().unwrap_or(0);
        let under = |v: &[u8], t: u8| 100.0 * v.iter().filter(|x| **x < t).count() as f64 / v.len() as f64;
        println!(
            "vertex colour over {} vertices: p05 {} · p25 {} · median {} · p75 {} · p95 {} · max {}",
            lum.len(), q(&lum, 0.05), q(&lum, 0.25), q(&lum, 0.5), q(&lum, 0.75), q(&lum, 0.95),
            lum.last().copied().unwrap_or(0)
        );
        println!(
            "  under 8: {:.1}% · under 32: {:.1}% · over 200: {:.1}%",
            under(&lum, 8), under(&lum, 32),
            100.0 - under(&lum, 201)
        );
        println!(
            "  alpha: p05 {} · median {} · p95 {} · under 255: {:.1}%",
            q(&alpha, 0.05), q(&alpha, 0.5), q(&alpha, 0.95), under(&alpha, 255)
        );
    }

    // NFS_SHADERS=1: what shader hashes the city's material runs carry, and what they cover.
    //
    // Every run has one (`NfsMaterialRange::shader`) and the world path has never looked at it —
    // `build_region` merges by texture key alone. On a car the same field is what tells glass from
    // paint, so if the city distinguishes lit windows from wall at all, this is where it would say
    // so. Reported as a census with the textures each covers, because a hash on its own is a
    // number and a hash with `ARC_*_WINDOW` under it is a finding.
    if std::env::var("NFS_SHADERS").is_ok() {
        let mut by: std::collections::BTreeMap<u32, (usize, usize, std::collections::BTreeSet<String>)> =
            std::collections::BTreeMap::new();
        for m in &meshes {
            for g in &m.groups {
                let e = by.entry(g.shader.0).or_default();
                e.0 += 1;
                e.1 += g.index_count / 3;
                if e.2.len() < 6 {
                    let name = packs
                        .iter()
                        .find_map(|p| p.get(g.hash))
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| format!("{:08X}", g.hash.0));
                    e.2.insert(name);
                }
            }
        }
        let runs: usize = by.values().map(|v| v.0).sum();
        println!("shader hashes: {} distinct over {runs} runs", by.len());
        let mut rows: Vec<_> = by.into_iter().collect();
        rows.sort_by_key(|(_, v)| std::cmp::Reverse(v.1));
        for (h, (n, tris, names)) in rows.iter().take(12) {
            println!(
                "  {h:08X}  {n:>6} runs  {tris:>8} tris  {}",
                names.iter().cloned().collect::<Vec<_>>().join(", ")
            );
        }
    }

    // NFS_GRIDS=1: does a start marker's own height agree with the city under it?
    //
    // The marker is the only record in these files carrying a height and nothing had ever checked
    // it. If the field is a height and `remap` is right, a grid sits on a road: marker minus ground
    // should be near zero for most grids. A constant offset would mean it is measured from
    // somewhere else; a scatter of tens of metres would mean it is not a height at all.
    if std::env::var("NFS_GRIDS").is_ok() {
        let ground = nfsu2::world::road_ground(&meshes);
        let root = std::path::Path::new(&path);
        let root = if root.is_file() { root.parent().unwrap_or(root) } else { root };
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else { continue };
            for e in entries.filter_map(std::result::Result::ok) {
                let f = e.path();
                if f.is_dir() {
                    stack.push(f);
                } else if f.file_name().and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("TrackPosMarkers") && n.ends_with(".bin"))
                {
                    files.push(f);
                }
            }
        }
        files.sort();
        let mut rows: Vec<(u32, f32, f32)> = Vec::new();
        let mut no_ground = 0usize;
        for f in &files {
            let Ok(bytes) = std::fs::read(f) else { continue };
            let Ok(m) = gizmo_nfs::world::routes::markers(&bytes) else { continue };
            for g in gizmo_nfs::world::routes::grids(&m) {
                let q = nfsu2::world::remap(g[0].at);
                match ground.height_at(q + Vec3::Y * 2.0) {
                    Some(y) => rows.push((g[0].track, q.y, y)),
                    None => no_ground += 1,
                }
            }
        }
        let mut diffs: Vec<f32> = rows.iter().map(|(_, said, got)| said - got).collect();
        diffs.sort_by(f32::total_cmp);
        let q = |f: f64| diffs.get(((diffs.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(f32::NAN);
        let within = |t: f32| diffs.iter().filter(|d| d.abs() <= t).count();
        println!(
            "grids vs ground: {} grids with road under them, {no_ground} without\n  \
             marker minus ground: p05 {:.1} · median {:.1} · p95 {:.1} m · within 1 m {} · \
             within 3 m {} · over 10 m {}",
            rows.len(), q(0.05), q(0.50), q(0.95),
            within(1.0), within(3.0), diffs.iter().filter(|d| d.abs() > 10.0).count()
        );
        let mut worst = rows.clone();
        worst.sort_by(|a, b| (b.1 - b.2).abs().total_cmp(&(a.1 - a.2).abs()));
        for (track, said, got) in worst.iter().take(5) {
            println!("    track {track:<5} marker y={said:>7.1} ground y={got:>7.1} off by {:>6.1} m", said - got);
        }
        // Free roam's own grid, named rather than left to the tail of a sorted list: it is the one
        // this mode spawns on, so it is the one whose number has to be seen.
        for (track, said, got) in rows.iter().filter(|(t, _, _)| *t == 4000) {
            println!("    track {track:<5} marker y={said:>7.1} ground y={got:>7.1} off by {:>6.1} m  <- free roam", said - got);
        }
        // The 24 lone free-roam markers, each against the city under it. If these stand on roads
        // and the grid does not, they are the places free roam is about and the grid is something
        // else — which is a question about the data, not a preference.
        {
            let all = nfsu2::world::Ground::of(&nfsu2::world::collision_cells(&meshes));
            let fr = root.join("ROUTESL4RA/TrackPosMarkersFreeRoam.bin");
            if let Ok(bytes) = std::fs::read(&fr) {
                if let Ok(m) = gizmo_nfs::world::routes::markers(&bytes) {
                    let spots = nfsu2::world::free_roam_spots(&m);
                    let (mut on_road, mut on_any, mut nothing) = (0, 0, 0);
                    let mut offs: Vec<f32> = Vec::new();
                    for q in &spots {
                        let r = ground.heights_at(q.x, q.z);
                        let a = all.heights_at(q.x, q.z);
                        if let Some(y) = r.iter().rev().find(|y| **y <= q.y + 2.0) {
                            on_road += 1;
                            offs.push(q.y - y);
                        } else if let Some(y) = a.iter().rev().find(|y| **y <= q.y + 2.0) {
                            on_any += 1;
                            offs.push(q.y - y);
                        } else {
                            nothing += 1;
                        }
                    }
                    let mut listed: Vec<(usize, f32, f32, f32)> = Vec::new();
                    for (i, q) in spots.iter().enumerate() {
                        let a = all.heights_at(q.x, q.z);
                        if let Some(y) = a.iter().rev().find(|y| **y <= q.y + 2.0) {
                            listed.push((i, q.y, *y, q.y - y));
                        }
                    }
                    listed.sort_by(|a, b| b.3.abs().total_cmp(&a.3.abs()));
                    for (i, said, got, d) in listed.iter().take(6) {
                        println!("    spot {i:>2}  marker y={said:>7.1}  ground y={got:>7.1}  {d:>7.1} m up");
                    }
                    println!(
                        "    within 1 m: {} of {}",
                        listed.iter().filter(|(_, _, _, d)| d.abs() <= 1.0).count(),
                        listed.len()
                    );
                    offs.sort_by(f32::total_cmp);
                    println!(
                        "  free-roam spots: {} · {on_road} over a road · {on_any} over other ground \
                         · {nothing} over nothing · marker minus ground median {:.1} m, worst {:.1} m",
                        spots.len(),
                        offs.get(offs.len() / 2).copied().unwrap_or(f32::NAN),
                        offs.iter().fold(0.0_f32, |a, b| if b.abs() > a.abs() { *b } else { a })
                    );
                }
            }
        }
        // The same point through the collision set the game actually drives on, which is a
        // different question from "is there a road named here" and had better not disagree.
        {
            let all = nfsu2::world::Ground::of(&nfsu2::world::collision_cells(&meshes));
            for f in &files {
                let Ok(bytes) = std::fs::read(f) else { continue };
                let Ok(m) = gizmo_nfs::world::routes::markers(&bytes) else { continue };
                for g in gizmo_nfs::world::routes::grids(&m).iter().filter(|g| g[0].track == 4000) {
                    let q = nfsu2::world::remap(g[0].at);
                    println!(
                        "    free roam pole ({:.0},{:.0}) marker y={:.1} · road surfaces {:?} · all surfaces {:?}",
                        q.x, q.z, q.y,
                        ground.heights_at(q.x, q.z).iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>(),
                        all.heights_at(q.x, q.z).iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>()
                    );
                }
            }
        }
    }

    // NFS_ROUTE=<Paths*.bin>: put that file's race line on the city and draw it. Built here rather
    // than at the spawn below because the height comes from the collision geometry, and `meshes` is
    // about to be moved into `build_region`.
    let routes: Vec<nfsu2::world::RoutePath> = match std::env::var("NFS_ROUTE") {
        Err(_) => Vec::new(),
        Ok(file) => {
            let path_of_route = file.clone();
            let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("read {file}: {e}"));
            let nodes = gizmo_nfs::world::routes::nodes(&bytes).expect("read the route's nodes");
            let ground = nfsu2::world::route::road_ground(&meshes);
            // How many surfaces does the city offer at a route node, and how far apart are they?
            // The seed rule below has to choose among them, so the shape of that choice is worth
            // seeing rather than assuming.
            {
                let mut counts = [0usize; 5];
                let mut spans: Vec<f32> = Vec::new();
                for n in &nodes {
                    let p = nfsu2::world::remap([n.x, n.y, 0.0]);
                    let c = ground.heights_at(p.x, p.z);
                    counts[c.len().min(4)] += 1;
                    if c.len() > 1 {
                        spans.push(c[c.len() - 1] - c[0]);
                    }
                }
                spans.sort_by(f32::total_cmp);
                let med = spans.get(spans.len() / 2).copied().unwrap_or(0.0);
                println!(
                    "  surfaces under a node: 0 -> {} · 1 -> {} · 2 -> {} · 3 -> {} · 4+ -> {} \
                     (median top-to-bottom spread {med:.1} m)",
                    counts[0], counts[1], counts[2], counts[3], counts[4]
                );
            }
            let built = nfsu2::world::build_route(&nodes, &ground);
            if std::env::var("NFS_ROUTE_DUMP").is_ok() {
                let all = nfsu2::world::Ground::of(&nfsu2::world::collision_cells(&meshes));
                for (n, pt) in nodes.iter().take(10).zip(built.first().map(|r| r.points.clone()).unwrap_or_default()) {
                    let p = nfsu2::world::remap([n.x, n.y, 0.0]);
                    println!(
                        "    node ({:.0},{:.0}) chose y={:.1} · road candidates {:?} · all candidates {:?}",
                        p.x, p.z, pt.y,
                        ground.heights_at(p.x, p.z).iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>(),
                        all.heights_at(p.x, p.z).iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>()
                    );
                }
            }
            // The event outline as a driving line. It is the one course description already
            // decoded — `0x3414C`, in lap order, closed exactly when the event is a circuit — and
            // it was good enough to pick a starting grid. The question here is whether it is good
            // enough to *drive*: how coarse are its steps, and does it stay on the road.
            if std::env::var("NFS_LINE").is_ok() {
                let file = std::path::Path::new(&path_of_route);
                if let Ok(bytes) = std::fs::read(file) {
                    let event: u16 = file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_start_matches(|c: char| !c.is_ascii_digit()))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    if let Ok(evs) = gizmo_nfs::world::routes::events(&bytes) {
                        if let Some(e) = evs.iter().find(|e| e.id == event) {
                            let pts: Vec<Vec3> = e
                                .outline
                                .iter()
                                .map(|p| nfsu2::world::remap([p[0], p[1], 0.0]))
                                .collect();
                            let mut steps: Vec<f32> =
                                pts.windows(2).map(|w| (w[1] - w[0]).length()).collect();
                            let perimeter: f32 = steps.iter().sum();
                            steps.sort_by(f32::total_cmp);
                            let q = |f: f64| {
                                steps.get((steps.len() as f64 * f) as usize).copied().unwrap_or(0.0)
                            };
                            // How much of it is over road at all: the same question `road_ground`
                            // answers for a route node, asked of the outline's own corners.
                            let on_road = pts
                                .iter()
                                .filter(|p| !ground.heights_at(p.x, p.z).is_empty())
                                .count();
                            println!(
                                "  event {event} outline: {} points · {perimeter:.0} m · \
                                 circuit {} · step p05 {:.0} median {:.0} p95 {:.0} m · \
                                 {on_road} of {} corners have road under them",
                                pts.len(),
                                e.circuit,
                                q(0.05),
                                q(0.5),
                                q(0.95),
                                pts.len()
                            );
                        }
                    }
                }
            }

            // NFS_LINE=1: can the race line be walked out of the network's own junctions?
            //
            // The nodes carry `links` — up to three indices into the same table, 94.8 % of them
            // pointing at a *different* path — so the network is paths joined at junctions, and a
            // race is a walk over it. The walk needs a rule for which link to take, and `progress`
            // is the candidate: it rises along a path, so the continuation should be the linked
            // node whose progress carries on from where the walk is.
            //
            // Judged the way a driveable line has to be judged: how far it covers against the
            // route's own stated length, and whether consecutive points are a car's distance apart
            // rather than a jump across the map.
            if std::env::var("NFS_LINE").is_ok() {
                let by_index: Vec<&gizmo_nfs::world::routes::RouteNode> = nodes.iter().collect();
                {
                    // Where progress actually runs. `start_of` treats the minimum as the start
                    // line, which assumes the measure is a lap coordinate beginning at zero.
                    let mut v: Vec<f32> = nodes.iter().map(|n| n.progress).collect();
                    v.sort_by(f32::total_cmp);
                    let zeros = v.iter().filter(|x| **x < 0.5).count();
                    println!(
                        "  progress runs {:.0}..{:.0} m · {zeros} nodes at zero · p25 {:.0} \
                         median {:.0} p75 {:.0}",
                        v[0], v[v.len() - 1], v[v.len() / 4], v[v.len() / 2], v[v.len() * 3 / 4]
                    );
                }
                let start = nodes
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.progress.total_cmp(&b.1.progress))
                    .map(|(i, _)| i);
                // A path's extent in file order, so it can be run either way.
                let span = |i: usize| {
                    let path = by_index[i].path;
                    let mut lo = i;
                    while lo > 0 && by_index[lo - 1].path == path {
                        lo -= 1;
                    }
                    let mut hi = i;
                    while by_index.get(hi + 1).is_some_and(|m| m.path == path) {
                        hi += 1;
                    }
                    (lo, hi)
                };
                let mut walk: Vec<usize> = Vec::new();
                let mut seen_path = std::collections::HashSet::new();
                let mut cur = start;
                let mut why = "ran off the table";
                while let Some(i) = cur {
                    if !seen_path.insert(by_index[i].path) {
                        why = "came back to a path it had already run";
                        break;
                    }
                    // **File order is not driving order.** A third of the paths are stored with
                    // progress falling along the file — 14 of 40 here, 20 of 51 elsewhere — so the
                    // direction to run a path is the one its own progress rises in, and running it
                    // by index leaves most of it behind or walks it backwards.
                    let (lo, hi) = span(i);
                    let forward = by_index[hi].progress >= by_index[lo].progress;
                    let end = if forward { hi } else { lo };
                    if forward {
                        walk.extend(i..=hi);
                    } else {
                        walk.extend((lo..=i).rev());
                    }
                    let here = by_index[end].progress;
                    let links: Vec<usize> = by_index[end]
                        .linked()
                        .map(|l| l as usize)
                        .filter(|l| *l < by_index.len())
                        .collect();
                    let onward: Vec<usize> =
                        links.iter().copied().filter(|l| by_index[*l].progress > here).collect();
                    if onward.is_empty() {
                        why = if links.is_empty() {
                            "the last node of the path has no links at all"
                        } else {
                            "the links all go back — none carries progress on"
                        };
                        // What the links *do* say, so "none carries progress on" is a fact about
                        // this rule and not about the data.
                        println!(
                            "    stopped at progress {here:.0} m with {} link(s): {:?}",
                            links.len(),
                            links.iter().map(|l| by_index[*l].progress.round()).collect::<Vec<_>>()
                        );
                        cur = None;
                    } else {
                        cur = onward
                            .into_iter()
                            .min_by(|a, b| by_index[*a].progress.total_cmp(&by_index[*b].progress));
                    }
                }
                let pts: Vec<Vec3> =
                    walk.iter().map(|i| nfsu2::world::remap([by_index[*i].x, by_index[*i].y, 0.0])).collect();
                let mut steps: Vec<f32> = pts.windows(2).map(|w| (w[1] - w[0]).length()).collect();
                let covered: f32 = steps.iter().sum();
                let jumps = steps.iter().filter(|d| **d > 100.0).count();
                steps.sort_by(f32::total_cmp);
                let q = |f: f64| steps.get((steps.len() as f64 * f) as usize).copied().unwrap_or(0.0);
                let stated = nodes.iter().map(|n| n.progress).fold(0.0_f32, f32::max);
                // Why the walk stops where it does. Two candidates and they are cheap to separate:
                // a path whose progress falls along file order breaks the "carry on" rule, and a
                // start that is not at the head of its own path leaves most of the path behind.
                let mut falling = 0;
                for run in gizmo_nfs::world::routes::paths(&nodes) {
                    if run.windows(2).any(|w| w[1].progress < w[0].progress) {
                        falling += 1;
                    }
                }
                let head = start.is_some_and(|i| i == 0 || by_index[i - 1].path != by_index[i].path);
                println!(
                    "  of {} paths, {falling} have progress falling along file order · the \
                     least-progress node is at the head of its path: {head}",
                    gizmo_nfs::world::routes::paths(&nodes).len()
                );
                println!(
                    "  walked line: {} of {} nodes over {} paths · {covered:.0} m against the \
                     file's {stated:.0} m ({:.0}%) · step p05 {:.1} median {:.1} p95 {:.1} m · \
                     over 100 m: {jumps} · stopped because it {why}",
                    walk.len(), nodes.len(), seen_path.len(),
                    100.0 * covered / stated.max(1.0), q(0.05), q(0.5), q(0.95)
                );
            }

            // Is the race line just "every node in progress order"? `progress` is the file's own
            // cumulative distance along the route, and `start_of` already trusts its minimum to be
            // the start. If it orders the whole network into something driveable, a pilot has a
            // line to follow instead of one street of a graph — which is the difference between
            // driving the race and driving into the barrier beside it.
            {
                let mut all: Vec<(f32, Vec3)> = built
                    .iter()
                    .flat_map(|r| r.progress.iter().copied().zip(r.points.iter().copied()))
                    .collect();
                all.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut steps: Vec<f32> = all.windows(2).map(|w| (w[1].1 - w[0].1).length()).collect();
                let jumps = steps.iter().filter(|d| **d > 100.0).count();
                let ties = all.windows(2).filter(|w| (w[1].0 - w[0].0).abs() < 0.01).count();
                steps.sort_by(f32::total_cmp);
                let q = |f: f64| steps.get((steps.len() as f64 * f) as usize).copied().unwrap_or(0.0);
                println!(
                    "  in progress order: {} points · step p05 {:.1} · median {:.1} · p95 {:.1} m · \
                     over 100 m: {jumps} ({:.1}%) · shared progress values: {ties}",
                    all.len(), q(0.05), q(0.5), q(0.95),
                    100.0 * jumps as f64 / steps.len().max(1) as f64
                );
            }
            let (pts, filled) = built.iter().fold((0, 0), |(p, f), r| (p + r.points.len(), f + r.filled));
            let worst = built.iter().map(nfsu2::world::RoutePath::climbed).fold(0.0, f32::max);
            let length: f32 = built.iter().map(nfsu2::world::RoutePath::length).sum();
            println!(
                "route {}: {} paths · {pts} points · {filled} took a neighbour's height · \
                 {length:.0} m of line · worst step up {worst:.1} m",
                std::path::Path::new(&file).file_stem().unwrap_or_default().to_string_lossy(),
                built.len()
            );
            let mut steps: Vec<(f32, u16, usize, Vec3, Vec3)> = built
                .iter()
                .flat_map(|r| {
                    r.points.windows(2).enumerate().map(move |(i, w)| {
                        ((w[1].y - w[0].y).abs(), r.index, i, w[0], w[1])
                    })
                })
                .collect();
            steps.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (d, path, i, a, b) in steps.iter().take(6) {
                println!(
                    "  step {d:>6.1} m  path {path:>3} node {i:>3}  \
                     ({:.0},{:.0},{:.0}) -> ({:.0},{:.0},{:.0})  {:.0} m apart",
                    a.x, a.y, a.z, b.x, b.y, b.z, (*b - *a).length()
                );
            }
            // How wide is the road the race is driven on? Not "how far is any road from a
            // path" — that measures the city, not the course. Walk out sideways from each point of
            // each path until the road stops answering, and the distance where it stops is the
            // half-width a barrier should use.
            let corridor = nfsu2::world::Corridor::of(&built, 12.0);
            let mut edge: Vec<f32> = Vec::new();
            for path in &built {
                for w in path.points.windows(2) {
                    let along = (w[1] - w[0]).normalize_or_zero();
                    if along == Vec3::ZERO {
                        continue;
                    }
                    let n = Vec3::new(-along.z, 0.0, along.x);
                    let mid = (w[0] + w[1]) * 0.5;
                    for side in [1.0f32, -1.0] {
                        let mut last = 0.0f32;
                        for step in 1..=60 {
                            let t = step as f32;
                            let p = mid + n * side * t;
                            // Still road, and still *this* road: a surface more than 4 m from the
                            // one under the path is the deck next door, not this carriageway.
                            let here = ground
                                .heights_at(p.x, p.z)
                                .into_iter()
                                .any(|y| (y - mid.y).abs() < 4.0);
                            if !here {
                                break;
                            }
                            last = t;
                        }
                        edge.push(last);
                    }
                }
            }
            // A corridor built from these paths has to find these paths. Every node of every path
            // must locate at ~0 m and report a progress close to the file's own — a check that
            // costs nothing and would catch a frame conversion or an index slipping.
            let (mut worst_d, mut worst_p, mut missed) = (0.0f32, 0.0f32, 0usize);
            for path in &built {
                for (p, prog) in path.points.iter().zip(&path.progress) {
                    match corridor.locate(*p) {
                        None => missed += 1,
                        Some(f) => {
                            worst_d = worst_d.max(f.distance);
                            worst_p = worst_p.max((f.progress - prog).abs());
                        }
                    }
                }
            }
            println!(
                "  corridor self-check: {missed} nodes the grid missed · worst distance \
                 {worst_d:.3} m · worst progress error {worst_p:.3}"
            );

            edge.sort_by(f32::total_cmp);
            if !edge.is_empty() {
                let q = |t: f32| edge[((edge.len() - 1) as f32 * t) as usize];
                println!(
                    "  corridor: {} segments · road reaches sideways {} samples · p10 {:.0} · \
                     p50 {:.0} · p90 {:.0} · p99 {:.0} m",
                    corridor.segments(),
                    edge.len(),
                    q(0.10),
                    q(0.50),
                    q(0.90),
                    q(0.99)
                );
            }
            built
        }
    };

    // NFS_REGIONS=<Paths*.bin>: draw that file's `0x0003414A` polygons, coloured by their `kind`
    // code. Fourteen codes exist across the install and nobody knows what they mean; if one of them
    // is the course corridor and another the start grid, a picture is what will say so.
    let regions: Vec<(u32, Vec<Vec3>)> = match std::env::var("NFS_REGIONS") {
        Err(_) => Vec::new(),
        Ok(file) => {
            let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("read {file}: {e}"));
            let raw = gizmo_nfs::world::routes::regions(&bytes).expect("read the route's regions");
            let ground = nfsu2::world::route::road_ground(&meshes);
            let mut kinds: std::collections::BTreeMap<u32, (usize, f32)> = Default::default();
            let out = raw
                .iter()
                .filter(|r| r.points.len() >= 3)
                .map(|r| {
                    let flat: Vec<Vec3> =
                        r.points.iter().map(|p| nfsu2::world::remap([p[0], p[1], 0.0])).collect();
                    let c = flat.iter().fold(Vec3::ZERO, |a, p| a + *p) / flat.len() as f32;
                    // One height for the whole polygon, from the road under its centre. A region is
                    // flat in the file; giving each corner its own height would tilt it by whatever
                    // the kerb beside it does.
                    let y = ground
                        .heights_at(c.x, c.z)
                        .first()
                        .copied()
                        .unwrap_or_else(|| ground.heights_at(c.x, c.z).first().copied().unwrap_or(0.0));
                    let area = flat
                        .iter()
                        .zip(flat.iter().cycle().skip(1))
                        .map(|(a, b)| a.x * b.z - b.x * a.z)
                        .sum::<f32>()
                        .abs()
                        / 2.0;
                    let e = kinds.entry(r.kind).or_default();
                    e.0 += 1;
                    e.1 += area;
                    (r.kind, flat.iter().map(|p| Vec3::new(p.x, y, p.z)).collect())
                })
                .collect();
            // Which kinds stand on the road and which cover the blocks between roads? The
            // picture says the small ones are on tarmac and the big ones are not; this is that,
            // counted.
            let mut on_road: std::collections::BTreeMap<u32, (usize, usize)> = Default::default();
            for r in raw.iter().filter(|r| r.points.len() >= 3) {
                let flat: Vec<Vec3> =
                    r.points.iter().map(|p| nfsu2::world::remap([p[0], p[1], 0.0])).collect();
                let c = flat.iter().fold(Vec3::ZERO, |a, p| a + *p) / flat.len() as f32;
                let e = on_road.entry(r.kind).or_default();
                e.1 += 1;
                if !ground.heights_at(c.x, c.z).is_empty() {
                    e.0 += 1;
                }
            }
            println!("regions: {} polygons", raw.len());
            for (k, (n, a)) in &kinds {
                let (hit, tot) = on_road.get(k).copied().unwrap_or((0, 1));
                println!(
                    "  kind {k:>3}: {n:>5} polygons · mean area {:>9.0} m2 · centre over road {:>3.0} %",
                    a / *n as f32,
                    100.0 * hit as f32 / tot as f32
                );
            }
            out
        }
    };

    let budget = std::env::var("NFS_BUDGET").ok().and_then(|s| s.parse::<usize>().ok());
    // Frame on the region's own centre so a budget takes a neighbourhood rather than an edge.
    let around = centre_of(&meshes);
    let city = build_region(&renderer.device, meshes, &packs, Some(&shared), around, budget);
    // Built here, not at the spawn below, because its textures have to be in the upload loop:
    // the backdrop's keys are not the city's — most of them live in the shared tier — and a key
    // nobody uploaded binds to nothing and draws white. That was the whole of "the backdrop is
    // pale": 153 meshes declared a texture and 85 of them found one.
    let sky_visuals = (!backdrop.is_empty())
        .then(|| build_region(&renderer.device, backdrop, &packs, Some(&shared), around, None));

    println!(
        "{declared} declared, {sky} backdrop, {lod} world-LOD, {} kept ({} duplicates), {} packs, {} merged meshes, {} unresolved runs",
        city.objects,
        duplicates,
        packs.len(),
        city.meshes.len(),
        city.unresolved_runs
    );

    // ── Textures: decode each key once, only the ones something actually uses ──
    // NFS_BLOOM="threshold[,intensity]": what crosses the glare threshold, and how hard.
    //
    // The renderer's default is 0.85, which nothing in this city ever reaches: the brightest
    // surface is a p95 vertex colour of 183 against a window texture peaking at 250, which is
    // about 0.44 in linear. So the glare pass runs every frame and extracts nothing. That is a
    // threshold tuned for a scene with values above 1.0, and a baked-lit night map is not one.
    let (bloom_t, bloom_i) = scene::city_glare();
    renderer.bloom_threshold = bloom_t;
    renderer.bloom_intensity = bloom_i;
    println!("bloom: threshold {bloom_t:.2}, intensity {bloom_i:.2}");

    // The two arms the engine gained and nothing was pulling — see `scene::city_lift`.
    let lift = scene::city_lift();
    println!("city lift: ambient {:?}, emissive {:?}", lift.0, lift.1);
    let white =
        assets.create_white_texture(&renderer.device, &renderer.queue, &renderer.scene.texture_bind_group_layout);
    let mut tex = Textures {
        assets: &mut assets,
        device: &renderer.device,
        queue: &renderer.queue,
        layout: &renderer.scene.texture_bind_group_layout,
    };
    let mut bound: HashMap<AssetHash, _> = HashMap::new();
    // Which textures are **cut-outs**: they carry real transparency rather than a full alpha
    // channel of 255. Bayview's foliage is flat cards with the tree cut out of a quad, and drawn
    // opaque those cards are dark slabs standing in the air — which is what a player sees and
    // reports as objects floating in the sky. Only these need the transparent pipeline; putting
    // the whole city in it would cost a sorted pass for the sake of the trees.
    let mut cut: std::collections::HashSet<AssetHash> = std::collections::HashSet::new();
    // How much of a texture has to be see-through before it counts as a cut-out. Measured, not
    // guessed: at "any transparent pixel at all" 235 of 1500 textures qualify and the roads go
    // with them — a road's texture carries a few transparent texels and drawing it in the sorted
    // transparent pass washes the whole surface out.
    let cut_share: f32 =
        std::env::var("NFS_CUT").ok().and_then(|v| v.parse().ok()).unwrap_or(CUT_SHARE);
    let mut shares: Vec<f32> = Vec::new();
    let mut decoded = 0usize;
    let sky_keys = sky_visuals.iter().flat_map(|s| s.meshes.iter()).filter_map(|m| m.texture);
    for key in city.meshes.iter().filter_map(|m| m.texture).chain(sky_keys) {
        if bound.contains_key(&key) {
            continue;
        }
        let own = packs.iter().find_map(|p| p.get(key).and_then(|r| p.decode(r).ok()));
        let Some(image) = own.or_else(|| shared.get(key).cloned()) else { continue };
        // A pixel under half opacity is the cut part of a cut-out; a texture with none is opaque
        // whatever its channel says. Counted rather than assumed, and printed, because "how much
        // of this city is foliage" is a fact about the city nobody here had.
        let px = image.rgba.len() / 4;
        let clear = image.rgba.chunks_exact(4).filter(|p| p[3] < 128).count();
        let share = clear as f32 / px.max(1) as f32;
        shares.push(share);
        // **A band, not a floor.** The two things that carry alpha here are not the same kind:
        // foliage is a card cropped tight to the tree, so only a small share of it is cut away,
        // while a *decal* — the circular paving pattern, road grime — is mostly transparent by
        // construction. Blending the first fixes the black slabs; blending the second loses it
        // entirely, because a decal is coplanar with the surface it sits on and the transparent
        // pass will not hold it there. So the band takes the low end and leaves the high end
        // opaque. `NFS_CUT=<share>` moves the ceiling.
        if cut_share > 0.0 && share > 0.0 && share <= cut_share {
            cut.insert(key);
        }
        let name = format!("city_{:08X}", key.0);
        if let Some(bg) = tex.upload(&name, &image.rgba, image.width, image.height) {
            bound.insert(key, bg);
            decoded += 1;
        }
    }
    {
        let mut v: Vec<f32> = shares.iter().copied().filter(|s| *s > 0.0).collect();
        v.sort_by(f32::total_cmp);
        if std::env::var("NFS_CUTLIST").is_ok() {
            let mut hist = [0usize; 10];
            for x in &v {
                hist[((x * 10.0) as usize).min(9)] += 1;
            }
            for (i, n) in hist.iter().enumerate() {
                println!("   %{:>3}-{:<3} · {n:>4} doku", i * 10, i * 10 + 10);
            }
        }
        let q = |f: f32| v.get(((v.len() as f32 - 1.0) * f) as usize).copied().unwrap_or(0.0);
        println!(
            "{decoded} textures decoded and uploaded · {} carry any transparency · \
             çeyrekler %{:.1} / %{:.1} / %{:.1} / %{:.1} · {} kesim sayıldı (eşik %{:.0})",
            v.len(),
            100.0 * q(0.25),
            100.0 * q(0.5),
            100.0 * q(0.75),
            100.0 * q(0.95),
            cut.len(),
            100.0 * cut_share
        );
    }

    // NFS_ID="x,y": render every merged mesh in a colour that encodes its index, then say which
    // one covers that pixel. Four hypotheses about the flat white ground were each eliminated by
    // filtering a family out and finding the frame unchanged; this asks the frame directly instead.
    //
    // The albedo is pre-converted linear→sRGB's inverse so the byte that lands in an
    // `Rgba8UnormSrgb` target is the index back again.
    let id_probe = std::env::var("NFS_ID").ok().and_then(|s| {
        let v: Vec<u32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        (v.len() == 2).then(|| (v[0], v[1]))
    });

    // ── Spawn ──
    //
    // Baked-lit: the city's own lighting is in the vertex colour, and the one thing the file
    // cannot know is where the car in front of it is casting. `BakedLit` multiplies the baked
    // colour in the way `unlit` does and adds the sun's cascade term, for one forward draw per
    // batch instead of the eleven a deferred-lit one costs.
    let mut spawned = 0usize;
    for (idx, m) in city.meshes.iter().enumerate() {
        if id_probe.is_some() {
            let to_linear = |b: u32| {
                let v = b as f32 / 255.0;
                if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            let (r, g) = ((idx & 0xFF) as u32, ((idx >> 8) & 0xFF) as u32);
            let material = Material::new(white.clone())
                .with_unlit(Vec4::new(to_linear(r), to_linear(g), 0.0, 1.0));
            scene::spawn_mesh(&mut world, m.mesh.clone(), material, Transform::new(m.origin));
            spawned += 1;
            continue;
        }
        // NFS_DOUBLE=1: draw the city double-sided. The city is authored to be backface-culled —
        // `world::remap`'s determinant is +1 so the winding survives the frame change — and this is
        // how that claim is tested rather than trusted: if a hole fills in when the back faces are
        // drawn, the winding is wrong somewhere and what you were seeing was a building's inside.
        let double = std::env::var("NFS_DOUBLE").is_ok();
        let material = match m.texture.and_then(|k| bound.get(&k)) {
            Some(bg) => Material::new(bg.clone()).with_baked_lit(Vec4::new(1.0, 1.0, 1.0, 1.0)),
            // A run whose texture resolved nowhere draws in a flat grey rather than vanishing —
            // a hole in the world reads as a parser bug, and this is not one.
            None => Material::new(white.clone()).with_baked_lit(Vec4::new(0.35, 0.35, 0.38, 1.0)),
        };
        let material = material.with_ambient(lift.0).with_emissive(lift.1);
        let material = if double { material.with_double_sided(true) } else { material };
        // And the cut-outs get the transparent pipeline. `NFS_ALPHA=0` puts them back the way
        // they were, which is how the before-and-after was taken.
        let material = if m.texture.is_some_and(|k| cut.contains(&k))
            && std::env::var("NFS_ALPHA").as_deref() != Ok("0")
        {
            material.with_transparent(true)
        } else {
            material
        };
        scene::spawn_mesh(&mut world, m.mesh.clone(), material, Transform::new(m.origin));
        spawned += 1;
    }
    println!("{spawned} entities spawned");

    // The backdrop, if asked for. Double-sided because a sky shell is seen from the inside and its
    // triangles face out — single-sided it culls to nothing, which looks exactly like not drawing
    // it at all, and that mistake is indistinguishable from the bug being checked for.
    if let Some(sky) = &sky_visuals {
        for m in &sky.meshes {
            let material = match m.texture.and_then(|k| bound.get(&k)) {
                Some(bg) => Material::new(bg.clone()),
                None => Material::new(white.clone()),
            };
            let material = material.with_backdrop_placed(Vec4::ONE).with_double_sided(true);
            scene::spawn_mesh(&mut world, m.mesh.clone(), material, Transform::new(m.origin));
        }
        // "textured" used to count meshes that *declare* a key, which is not the same as meshes
        // whose key was found — and the difference is the whole story here: the backdrop's
        // textures do not live in the region's own packs. Count what actually bound.
        let declared = sky.meshes.iter().filter(|m| m.texture.is_some()).count();
        let resolved =
            sky.meshes.iter().filter(|m| m.texture.is_some_and(|k| bound.contains_key(&k))).count();
        println!(
            "backdrop: {} meshes · {declared} declare a texture · {resolved} of those bound · \
             {} unresolved runs",
            sky.meshes.len(),
            sky.unresolved_runs
        );
    }

    // The regions, as flat fans lifted clear of the road, one colour per `kind`.
    if !regions.is_empty() {
        let mut by_kind: std::collections::BTreeMap<u32, Vec<gizmo::renderer::gpu_types::Vertex>> =
            Default::default();
        for (kind, poly) in &regions {
            let verts = by_kind.entry(*kind).or_default();
            for i in 1..poly.len() - 1 {
                for p in [poly[0], poly[i], poly[i + 1]] {
                    verts.push(gizmo::renderer::gpu_types::Vertex {
                        position: [p.x, p.y + 2.0, p.z],
                        color: [1.0, 1.0, 1.0, 1.0],
                        normal: [0.0, 1.0, 0.0],
                        tex_coords: [0.0, 0.0],
                        ..Default::default()
                    });
                }
            }
        }
        // Distinct rather than pretty: fourteen codes have to be told apart at a glance.
        const HUES: [[f32; 3]; 8] = [
            [0.95, 0.20, 0.20], [0.20, 0.85, 0.35], [0.25, 0.45, 0.95], [0.95, 0.85, 0.20],
            [0.85, 0.30, 0.90], [0.20, 0.90, 0.90], [0.95, 0.55, 0.15], [0.60, 0.60, 0.60],
        ];
        for (n, (kind, verts)) in by_kind.iter().enumerate() {
            if verts.is_empty() {
                continue;
            }
            let mesh = Mesh::from_vertices(&renderer.device, verts, format!("region_{kind}"));
            let c = HUES[n % HUES.len()];
            let material =
                Material::new(white.clone()).with_unlit(Vec4::new(c[0], c[1], c[2], 1.0));
            scene::spawn_mesh(&mut world, mesh, material, Transform::new(Vec3::ZERO));
            println!("  kind {kind} -> rgb({:.2},{:.2},{:.2}), {} triangles", c[0], c[1], c[2], verts.len() / 3);
        }
    }

    // The race line, as a flat ribbon lifted clear of the tarmac. Unlit and bright on purpose: the
    // city is baked-lit and dark, and a diagnostic that has to be hunted for is not one. Lifted 3 m
    // rather than the half metre that would look right: this frames the whole city from over a
    // kilometre up, and at that near:far ratio half a metre is inside the depth buffer's noise —
    // the ribbon was drawn correctly and vanished into the road.
    if !routes.is_empty() {
        let verts = nfsu2::world::route::ribbon(&routes, 4.0, 3.0);
        if !verts.is_empty() {
            let mesh = Mesh::from_vertices(&renderer.device, &verts, String::from("route"));
            let material = Material::new(white.clone()).with_unlit(Vec4::new(0.95, 0.15, 0.15, 1.0));
            scene::spawn_mesh(&mut world, mesh, material, Transform::new(Vec3::ZERO));
            println!("route ribbon: {} triangles", verts.len() / 3);
        }
    }

    // ── Camera: frame what actually loaded ──
    let (lo, hi) = bounds(&city);
    let mid = std::env::var("NFS_AT")
        .ok()
        .and_then(|s| vec3_of(&s))
        .unwrap_or_else(|| (lo + hi) * 0.5);
    let radius = ((hi - lo).length() * 0.5).max(1.0);
    let eye_off = std::env::var("NFS_EYE")
        .ok()
        .and_then(|s| vec3_of(&s))
        .unwrap_or_else(|| Vec3::new(radius * 0.9, radius * 1.1, radius * 0.9));
    let eye = mid + eye_off;

    scene::add_lights(
        &mut world,
        Transform::new(eye + Vec3::new(0.0, radius, 0.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.9)),
        2.4,
        mid,
    );

    let dir = (mid - eye).normalize();
    let cam = world.spawn();
    world.add_component(cam, Transform::new(eye));
    world.add_component(cam, GlobalTransform::default());
    // Near is derived from the eye distance rather than from the framing radius. Tying it to the
    // radius — which is what a car viewer does, correctly, because the car *is* the radius — put
    // the near plane at 95 m on a street-level shot and clipped every building the camera stood
    // next to. What matters at city scale is the near:far ratio, not the absolute near, so this
    // takes a small fraction of the eye distance and floors it at half a metre.
    let eye_dist = (mid - eye).length();
    let near = (eye_dist * 0.002).clamp(0.5, 10.0);
    let far = eye_dist + radius * 3.0;
    world.add_component(
        cam,
        Camera::new(std::f32::consts::FRAC_PI_4, near, far, dir.z.atan2(dir.x), dir.y.asin(), true),
    );
    println!("camera at {eye:?} looking at {mid:?}, near={near:.1} far={far:.1}");

    let pixels = shoot(&mut world, &mut renderer, out, w, h);

    if let Some((px, py)) = id_probe {
        let i = ((py.min(h - 1) * w + px.min(w - 1)) * 4) as usize;
        let (r, g) = (u32::from(pixels[i]), u32::from(pixels[i + 1]));
        let idx = (g << 8 | r) as usize;
        println!("pixel ({px},{py}) = rgb({r},{g},{}) → mesh #{idx}", pixels[i + 2]);
        match city.meshes.get(idx) {
            Some(m) => {
                let name = m
                    .texture
                    .and_then(|k| packs.iter().find_map(|p| p.get(k)).map(|t| t.name.clone()))
                    .or_else(|| m.texture.map(|k| format!("shared {:08X}", k.0)))
                    .unwrap_or_else(|| "(no texture)".into());
                println!("  cell {:?}  origin {:?}  texture {name}", m.cell, m.origin);
            }
            None => println!("  no mesh with that index — the pixel is background"),
        }
    }
}

/// `"x,y,z"` → a vector, or `None` if it is not three numbers.
fn vec3_of(s: &str) -> Option<Vec3> {
    let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
}

/// One bundle, or every `STREAM*.BUN` in a directory, in name order.
fn bundles(path: &str) -> Vec<std::path::PathBuf> {
    let p = std::path::Path::new(path);
    if p.is_file() {
        return vec![p.to_path_buf()];
    }
    let Ok(dir) = std::fs::read_dir(p) else { return Vec::new() };
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

/// The centre of every object's bounding box, in the Gizmo frame.
fn centre_of(meshes: &[gizmo_nfs::world::WorldMesh]) -> Vec3 {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    // Objects with no geometry are skipped: the `ANM_*` animation markers carry an infinite
    // bounding box, and one of those turns the whole city's centre into a NaN.
    for m in meshes.iter().filter(|m| !m.positions.is_empty()) {
        for corner in [m.header.bbox_min, m.header.bbox_max] {
            let p = nfsu2::world::world_point(&m.header, corner);
            if p.is_finite() {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    if lo.is_finite() && hi.is_finite() {
        (lo + hi) * 0.5
    } else {
        Vec3::ZERO
    }
}

/// Bounds of the cells that were actually built, which is what the camera frames.
fn bounds(city: &CityVisuals) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for m in &city.meshes {
        let half = Vec3::splat(nfsu2::world::CELL_SIZE * 0.5);
        lo = lo.min(m.origin - half);
        hi = hi.max(m.origin + half);
    }
    if lo.is_finite() && hi.is_finite() {
        (lo, hi)
    } else {
        (Vec3::ZERO, Vec3::ONE)
    }
}

/// Render one frame into an offscreen target and write it out, tightly packed.
fn shoot(world: &mut World, renderer: &mut Renderer, out: &str, w: u32, h: u32) -> Vec<u8> {
    let format = renderer.config.format;
    let bpp = 4u32;
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("nfs-city-target"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder =
        renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    gizmo::systems::default_render_pass(world, &mut encoder, &view, renderer);

    let unpadded = w * bpp;
    let padded = unpadded.div_ceil(256) * 256;
    let staging = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("nfs-city-readback"),
        size: (padded * h) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    renderer.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
    let _ = renderer.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    // wgpu 30: `get_mapped_range` artık `Result` döndürüyor. Eşleme yukarıda
    // `rx.recv().unwrap().unwrap()` ile zaten beklendi ve başarısı doğrulandı, yani
    // buradaki hata "eşlenmemiş tampon" demek olurdu — o da bir kaçırılmış hata değil,
    // bir mantık hatası. Motorun kendi `capture.rs`'i de aynı deseni kullanıyor.
    let data = slice.get_mapped_range().expect("mapped range");

    let mut tight = Vec::with_capacity((unpadded * h) as usize);
    for y in 0..h {
        let start = (y * padded) as usize;
        tight.extend_from_slice(&data[start..start + unpadded as usize]);
    }
    std::fs::write(out, &tight).expect("write raw");
    println!("{w}x{h} format={format:?} -> {out}");
    tight
}

// ── NFS_PROBE: what is under a point, and what is beside a segment ────────────────────────────
//
// Deliberately self-contained and deliberately crude. It exists to answer one question about an
// undecoded chunk and it should leave with the answer.

/// A 2-D grid over the city's collision triangles, so a point query touches ~50 of 1.2 million.
struct Soup {
    tris: Vec<([Vec3; 3], nfsu2::world::Surface)>,
    grid: HashMap<(i32, i32), Vec<u32>>,
}

const PROBE_CELL: f32 = 32.0;

impl Soup {
    fn of(meshes: &[gizmo_nfs::world::WorldMesh]) -> Self {
        let mut tris = Vec::new();
        for c in nfsu2::world::collision_cells(meshes) {
            for (t, surface) in c.indices.chunks_exact(3).zip(c.surfaces.iter()) {
                let v = |i: u32| c.origin + c.vertices[i as usize];
                tris.push(([v(t[0]), v(t[1]), v(t[2])], *surface));
            }
        }
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, (t, _)) in tris.iter().enumerate() {
            let key = |p: Vec3| ((p.x / PROBE_CELL).floor() as i32, (p.z / PROBE_CELL).floor() as i32);
            let (lo, hi) = t.iter().fold(((i32::MAX, i32::MAX), (i32::MIN, i32::MIN)), |(lo, hi), p| {
                let k = key(*p);
                ((lo.0.min(k.0), lo.1.min(k.1)), (hi.0.max(k.0), hi.1.max(k.1)))
            });
            // A triangle spanning many cells is terrain; cap the spread so one of those does not
            // land in a thousand buckets.
            if (hi.0 - lo.0) > 8 || (hi.1 - lo.1) > 8 {
                continue;
            }
            for gx in lo.0..=hi.0 {
                for gz in lo.1..=hi.1 {
                    grid.entry((gx, gz)).or_default().push(i as u32);
                }
            }
        }
        Soup { tris, grid }
    }

    /// Whether a triangle's footprint contains the point, in plan view.
    fn covers(t: &[Vec3; 3], p: Vec3) -> bool {
        let s = |a: Vec3, b: Vec3| (b.x - a.x) * (p.z - a.z) - (b.z - a.z) * (p.x - a.x);
        let (d1, d2, d3) = (s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0]));
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(neg && pos)
    }

    /// Is there a triangle of this kind standing over the point?
    fn over(&self, p: Vec3, want: nfsu2::world::Surface) -> bool {
        let k = ((p.x / PROBE_CELL).floor() as i32, (p.z / PROBE_CELL).floor() as i32);
        self.grid.get(&k).is_some_and(|ids| {
            ids.iter().any(|i| {
                let (t, s) = &self.tris[*i as usize];
                *s == want && Self::covers(t, p)
            })
        })
    }
}

fn probe(dir: &str, meshes: &[gizmo_nfs::world::WorldMesh]) {
    use nfsu2::world::Surface;
    let soup = Soup::of(meshes);
    let drivable = soup.tris.iter().filter(|(_, s)| *s == Surface::Drivable).count();
    println!(
        "probe: {} collision triangles ({drivable} drivable, {} wall) in {} cells",
        soup.tris.len(),
        soup.tris.len() - drivable,
        soup.grid.len()
    );

    // The route frame is the world's own, so the city's own conversion applies unchanged. Height
    // is unknown, and every test below is in plan view, so zero is honest rather than a guess.
    let at = |x: f32, y: f32| nfsu2::world::remap([x, y, 0.0]);
    let rows = |name: &str| -> Vec<Vec<f32>> {
        std::fs::read_to_string(std::path::Path::new(dir).join(name))
            .unwrap_or_else(|e| panic!("{dir}/{name}: {e}"))
            .lines()
            .map(|l| l.split(',').filter_map(|v| v.trim().parse().ok()).collect())
            .filter(|v: &Vec<f32>| !v.is_empty())
            .collect()
    };

    // ── The control ──
    let nodes = rows("nodes.csv");
    let on = nodes.iter().filter(|r| soup.over(at(r[0], r[1]), Surface::Drivable)).count();
    println!(
        "  CONTROL nodes.csv: {on}/{} ({:.0} %) stand over drivable surface",
        nodes.len(),
        100.0 * on as f32 / nodes.len() as f32
    );

    // ── The question ──
    let segs = rows("segments.csv");
    let (mut both, mut one, mut neither, mut walled) = (0usize, 0usize, 0usize, 0usize);
    let (mut on_road, mut ends_off, mut ends_on, mut ends_half) = (0usize, 0usize, 0usize, 0usize);
    for r in &segs {
        let (a, b) = (at(r[0], r[1]), at(r[2], r[3]));
        let mid = (a + b) * 0.5;
        let dir = (b - a).normalize_or_zero();
        let n = Vec3::new(-dir.z, 0.0, dir.x); // perpendicular, in plan
        let l = soup.over(mid + n * 4.0, Surface::Drivable);
        let r_ = soup.over(mid - n * 4.0, Surface::Drivable);
        match (l, r_) {
            (true, true) => both += 1,
            (false, false) => neither += 1,
            _ => one += 1,
        }
        // Is the segment itself standing on a wall? Sample along it rather than at the midpoint —
        // a barrier is a line, and one sample can miss the gap in a kerb.
        if (0..=4).any(|i| soup.over(a.lerp(b, i as f32 / 4.0), Surface::Wall)) {
            walled += 1;
        }
        // Does it span the road rather than run along it? A line drawn across a carriageway has
        // its middle on tarmac and both ends at the edge, so stepping past either end leaves it.
        if soup.over(mid, Surface::Drivable) {
            on_road += 1;
        }
        let past = |p: Vec3, d: Vec3| soup.over(p + d * 3.0, Surface::Drivable);
        match (past(a, -dir), past(b, dir)) {
            (false, false) => ends_off += 1,
            (true, true) => ends_on += 1,
            _ => ends_half += 1,
        }
    }
    let pct = |v: usize| 100.0 * v as f32 / segs.len() as f32;
    println!("  segments.csv: {} segments", segs.len());
    println!(
        "    4 m to each side: drivable on BOTH {both} ({:.0} %) · ONE {one} ({:.0} %) · NEITHER {neither} ({:.0} %)",
        pct(both),
        pct(one),
        pct(neither)
    );
    println!("    a wall triangle stands on the segment itself: {walled} ({:.0} %)", pct(walled));
    println!("    midpoint is on drivable surface: {on_road} ({:.0} %)", pct(on_road));
    println!(
        "    3 m past each end: off-road at BOTH {ends_off} ({:.0} %) · on-road at both {ends_on} ({:.0} %) · one each {ends_half} ({:.0} %)",
        pct(ends_off),
        pct(ends_on),
        pct(ends_half)
    );
}

