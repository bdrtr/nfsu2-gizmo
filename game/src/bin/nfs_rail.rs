//! # Every race, on rails, with no window
//!
//! `nfs_sim` asks how far a simulated pilot gets; this asks whether a race **finishes**. It builds
//! each race's line the way the game will ([`city::race_ring`] → [`RaceLine::build`]), puts a full
//! grid of [`Rail`]s on it, and runs the race to the flag with the same [`Race`] arithmetic
//! `nfs_sim` uses — counting in metres instead of waypoints.
//!
//! **Why every route.** The pilot work's last lesson was that eight routes are a fitting set, not a
//! test: a package that won +19 % on them lost on 24 it had never seen. A rail has nothing to fit,
//! but its line does — smoothing, the deck choice, the speed profile — so it is judged on all 105
//! races in the install, each region loaded once. A car is still built, headless, because the
//! rails' limits are measured on it ([`Limits::measure`]).
//!
//! ```bash
//! cargo run --release -p nfsu2 --bin nfs_rail -- "$NFSU2_ROOT/TRACKS" "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
//! ```
//!
//! Env: `NFS_ROUTES=4001,4002` limits the run to those events (default: all of them) ·
//! `NFS_RIVALS=<n>` cars per race (default: the whole grid).
//!
//! One line per race, then a summary. The columns that matter are on the right: **who finished**,
//! and whether any two cars ever stood in each other.

use gizmo::prelude::*;
use gizmo::renderer::Renderer;
use nfsu2::race::Race;
use nfsu2::rig::rail::{self, PACE};
use nfsu2::rig::{Limits, RaceLine, Rail};
use nfsu2::world as city;
use std::path::{Path, PathBuf};

/// Spacing of the ring the line is built from — the same 40 m every ring in the project uses.
const WAYPOINT_STEP: f32 = 40.0;
/// The race's step, in seconds. Rails have no physics to keep stable, so a frame is enough.
const DT: f32 = 1.0 / 60.0;
/// Two cars closer than this along the line and across it are standing in each other: a stock
/// 240SX's own length and width, 4.52 × 1.69 m.
const TOUCH_ALONG: f32 = 4.52;
const TOUCH_ACROSS: f32 = 1.69;

/// What one race came to.
struct Outcome {
    name: String,
    line: rail::LineReport,
    circuit: bool,
    chords: usize,
    heading_cos: f32,
    finished: usize,
    cars: usize,
    first: Option<f32>,
    last: Option<f32>,
    touching: usize,
    walled: usize,
    flipped: bool,
}

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let tracks = city::tracks_path(std::env::args().nth(1));
    let car_path = std::env::args()
        .nth(2)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .or_else(|| std::env::var("NFSU2_ROOT").ok().map(|r| format!("{r}/CARS/240SX/GEOMETRY.BIN")))
        .expect("no car given and NFSU2_ROOT is unset");
    let only: Option<Vec<u16>> = std::env::var("NFS_ROUTES")
        .ok()
        .filter(|v| v != "all")
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    let rivals: Option<usize> = std::env::var("NFS_RIVALS").ok().and_then(|v| v.parse().ok());

    // Every race file, grouped by the region it is driven in, so each city is loaded once.
    let mut by_region: std::collections::BTreeMap<PathBuf, Vec<PathBuf>> = Default::default();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&tracks)
        .unwrap_or_else(|e| panic!("read {tracks}: {e}"))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("ROUTES")))
        .collect();
    dirs.sort();
    for dir in dirs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|r| r.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        files.retain(|f| event_of(f).is_some_and(|e| only.as_ref().is_none_or(|o| o.contains(&e))));
        files.sort();
        if let (Some(bundle), false) = (files.first().and_then(|f| city::bundle_for_route(f)), files.is_empty()) {
            by_region.entry(bundle).or_default().extend(files);
        }
    }
    let total: usize = by_region.values().map(Vec::len).sum();
    println!("{total} races in {} regions", by_region.len());

    assert!(Renderer::headless_adapter_available().await, "no GPU adapter for a headless run");
    let renderer = Renderer::new_headless(64, 64, None).await;
    let mut assets = AssetManager::new();
    let limits = Limits::measure(&renderer, &mut assets, &car_path);

    let mut outcomes: Vec<Outcome> = Vec::new();
    for (bundle, routes) in &by_region {
        let source = bundle.display().to_string();
        let city::Bundles { meshes, .. } = city::load(&source);
        let meshes = city::dedup(meshes);
        let objects: Vec<_> = meshes.into_iter().filter(|m| city::is_drawn(&m.header.name)).collect();
        let objects = city::lod::keep_finest(objects);
        let colliders = city::collision_cells(&objects);
        let ground = city::Ground::of(&colliders);
        let walls = city::Walls::of(&colliders);
        let roads = city::road_ground(&objects);
        println!("\n== {} · {} objects · {} races", source.rsplit('/').next().unwrap_or(&source), objects.len(), routes.len());
        for route in routes {
            match race(route, &ground, &walls, &roads, &limits, rivals) {
                Ok(o) => {
                    print_outcome(&o);
                    outcomes.push(o);
                }
                Err(why) => println!("{:<10} · atlandı: {why}", name_of(route)),
            }
        }
    }
    summary(&outcomes, total);
}

/// The event number in a `Paths####.bin` name, or `None` for anything else in the directory.
fn event_of(path: &Path) -> Option<u16> {
    let name = path.file_name()?.to_str()?;
    name.strip_prefix("Paths")?.strip_suffix(".bin")?.parse().ok()
}

fn name_of(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_string()
}

/// Build one race's line, put the grid on it, and run it to the flag.
fn race(
    route: &Path,
    ground: &city::Ground,
    walls: &city::Walls,
    roads: &city::Ground,
    limits: &Limits,
    rivals: Option<usize>,
) -> Result<Outcome, String> {
    let bytes = std::fs::read(route).map_err(|e| e.to_string())?;
    let nodes = gizmo_nfs::world::routes::nodes(&bytes).map_err(|e| format!("{e:?}"))?;
    let event = event_of(route).ok_or("not a race file")?;
    let catalogue = gizmo_nfs::world::routes::events(&bytes).unwrap_or_default();
    let ev = catalogue.iter().find(|e| e.id == event).ok_or("the file does not describe its own event")?;
    let markers = route
        .parent()
        .map(|d| d.join("TrackPosMarkersAll.bin"))
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| gizmo_nfs::world::routes::markers(&b).ok())
        .unwrap_or_default();
    let (slots, heading) = city::start_slots(&markers, ev).ok_or("no full starting grid")?;

    let net = city::Network::of(&nodes, roads, ground);
    let coarse: Vec<Vec3> = ev.outline.iter().map(|p| city::remap([p[0], p[1], 0.0])).collect();
    let (ring, chords) = city::race_ring(&net, ground, walls, &coarse, WAYPOINT_STEP);
    // `NFS_RAILENDS=1`: how far the grid is from each end of the ring, and which way it faces
    // relative to each — the evidence for which end a race starts from.
    if std::env::var("NFS_RAILENDS").is_ok_and(|v| v != "0") && ring.len() > 2 {
        let c = slots.iter().copied().sum::<Vec3>() / slots.len() as f32;
        let h = Vec3::new(heading.x, 0.0, heading.z).normalize_or_zero();
        let plan = |a: Vec3, b: Vec3| Vec3::new(b.x - a.x, 0.0, b.z - a.z);
        let (first, last) = (ring[0], ring[ring.len() - 1]);
        let out = plan(ring[0], ring[1]).normalize_or_zero();
        let into = plan(ring[ring.len() - 2], ring[ring.len() - 1]).normalize_or_zero();
        println!(
            "UÇLAR {} {} · baştan {:.0} m (yön·çıkış {:+.2}, gride→baş {:+.2}) · sondan {:.0} m (yön·varış {:+.2})",
            name_of(route),
            if ev.circuit { "devre" } else { "sprint" },
            plan(c, first).length(),
            h.dot(out),
            h.dot(plan(c, first).normalize_or_zero()),
            plan(c, last).length(),
            h.dot(into),
        );
    }
    // The grid's heading is the reliable half; three outlines are written finish first.
    let (ring, flipped) = rail::orient_ring(&ring, ev.circuit, &slots, heading);
    let line = RaceLine::build(&ring, ev.circuit, Some((&slots, heading)), roads, ground, limits)
        .ok_or("ring too short")?;

    // `NFS_RAILDUMP=<event>`: the line sample by sample — where it is, the height it stands at, and
    // every surface on offer there. What says *why* a stretch climbs a step or leaves the road.
    if std::env::var("NFS_RAILDUMP").ok().and_then(|v| v.parse::<u16>().ok()) == Some(event) {
        let every: usize = std::env::var("NFS_RAILEVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
        for (i, w) in ring.iter().enumerate() {
            println!("   halka {i:>4} ({:>7.1},{:>7.1}) y {:>6.1}", w.x, w.z, w.y);
        }
        for (i, p) in line.points().iter().enumerate().step_by(every) {
            let r = roads.heights_at(p.x, p.z);
            let g = ground.heights_at(p.x, p.z);
            let fmt = |v: &[f32]| v.iter().map(|y| format!("{y:.1}")).collect::<Vec<_>>().join(",");
            println!(
                "   örnek {i:>5} ({:>7.1},{:>7.1}) y {:>6.1} · {:>3.0} km/h · yol [{}] · zemin [{}]",
                p.x,
                p.z,
                p.y,
                line.speed_at(i as f32 * nfsu2::rig::rail::SAMPLE) * 3.6,
                fmt(&r),
                fmt(&g)
            );
        }
    }

    let walled = line.through_walls(walls, ground);
    // `NFS_RAILFLAGS=1`: where the line steps, curves tighter than 8 m, or goes through a wall.
    let flags = std::env::var("NFS_RAILFLAGS").is_ok_and(|v| v != "0");
    if flags {
        let pts = line.points();
        let ring_head: Vec<String> = ring.iter().take(3).map(|p| format!("({:.0},{:.0})", p.x, p.z)).collect();
        let slot_list: Vec<String> = slots.iter().map(|p| format!("({:.0},{:.0})", p.x, p.z)).collect();
        println!(
            "   {} · grid {} · yön ({:.2},{:.2}) · halka başı {} · hat başı ({:.0},{:.0}) → ({:.0},{:.0})",
            name_of(route), slot_list.join(" "), heading.x, heading.z, ring_head.join(" "),
            pts[0].x, pts[0].z, pts[20].x, pts[20].z
        );
        for (i, grade, r) in line.flags(8.0) {
            let p = pts[i];
            println!(
                "   {} · örnek {i:>5} ({:>7.1},{:>7.1}) y {:>6.1} · eğim %{:>4.0} · yarıçap {:>6.1} m · yol {:?}",
                name_of(route), p.x, p.z, p.y, grade * 100.0, r, roads.heights_at(p.x, p.z)
            );
        }
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for &i in &walled {
            match runs.last_mut() {
                Some(r) if i <= r.1 + 2 => r.1 = i,
                _ => runs.push((i, i)),
            }
        }
        for (a, b) in runs {
            let p = pts[a];
            println!("   {} · duvar örnek {a}-{b} ({:>7.1},{:>7.1}) y {:>6.1}", name_of(route), p.x, p.z, p.y);
        }
    }
    let cars = rivals.unwrap_or(slots.len()).min(slots.len());
    let mut field = Rail::grid(&line, &slots[..cars], heading, &PACE).ok_or("grid is nowhere near the line")?;
    let from = field.first().map_or(0.0, Rail::from);
    let heading_cos = line.tangent(from).dot(Vec3::new(heading.x, 0.0, heading.z).normalize_or_zero());

    let course = line.course(from);
    let mut race = Race::new(course, line.closed());
    let laps = if line.closed() { Race::LAPS as f32 } else { 1.0 };
    let slowest_pace = PACE.iter().copied().fold(1.0f32, f32::min);
    let cap = Race::COUNTDOWN + 1.6 * laps * line.report.lap_time / slowest_pace + 60.0;

    let trace: Option<(usize, f32, f32)> = std::env::var("NFS_RAILTRACE").ok().and_then(|v| {
        let (car, win) = v.split_once(':')?;
        let (a, b) = win.split_once('-')?;
        Some((car.parse().ok()?, a.parse().ok()?, b.parse().ok()?))
    });
    let mut t = 0.0f32;
    let mut times: Vec<Option<f32>> = vec![None; field.len()];
    let mut touching = 0usize;
    while t < cap {
        race.tick(DT);
        rail::advance(&mut field, &[], &line, limits, DT, race.holding());
        t += DT;
        // `NFS_RAILTRACE=<car>:<from>-<to>`: one car, step by step, over a window of race seconds.
        if let Some((car, a, b)) = trace {
            let rt = t - Race::COUNTDOWN;
            if rt >= a && rt <= b {
                if let Some(r) = field.get(car) {
                    let (cap, boxed, held, goal) = r.traffic();
                    println!(
                        "   iz t={rt:.2} araba {car} s={:.1} {:.1} km/h şerit {:+.2}→{goal:+.1} · sınır {} · kutulu {boxed} · bekledi {held:.1} s · önündekiler {}",
                        r.s(), r.speed() * 3.6, r.lane(),
                        cap.map_or("—".into(), |c| format!("{:.1}", c * 3.6)),
                        field.iter().enumerate().filter(|(j, o)| *j != car && (o.s() - r.s()).abs() < 40.0)
                            .map(|(j, o)| format!("[{j} {:+.1} m {:+.1} {:.0}]", o.s() - r.s(), o.lane(), o.speed() * 3.6))
                            .collect::<Vec<_>>().join(" ")
                    );
                }
            }
        }
        race.standings(&field);
        for &c in race.finishers() {
            times[c].get_or_insert(t);
        }
        if race.over(field.len()) {
            break;
        }
        if !race.holding() {
            for i in 0..field.len() {
                for j in i + 1..field.len() {
                    let mut d = (field[i].s() - field[j].s()).abs();
                    if line.closed() {
                        d = d.rem_euclid(line.length());
                        d = d.min(line.length() - d);
                    }
                    // Only while both are racing: after the flag a sprint's field parks nose to
                    // tail at the end of its line, which is not a race any more.
                    let racing = times[i].is_none() && times[j].is_none();
                    if racing && d < TOUCH_ALONG && (field[i].lane() - field[j].lane()).abs() < TOUCH_ACROSS {
                        touching += 1;
                        if flags && (touching <= 3 || touching.is_multiple_of(50)) {
                            println!(
                                "   {} · temas t={:.1}s · araba {i} s={:.1} şerit {:+.2} {:.0} km/h · araba {j} s={:.1} şerit {:+.2} {:.0} km/h · bitti {:?}/{:?}",
                                name_of(route), t - Race::COUNTDOWN,
                                field[i].s(), field[i].lane(), field[i].speed() * 3.6,
                                field[j].s(), field[j].lane(), field[j].speed() * 3.6,
                                times[i].is_some(), times[j].is_some()
                            );
                        }
                    }
                }
            }
        }
    }
    let done: Vec<f32> = times.iter().filter_map(|x| *x).map(|x| x - Race::COUNTDOWN).collect();
    Ok(Outcome {
        name: name_of(route),
        line: line.report.clone(),
        circuit: line.closed(),
        chords,
        heading_cos,
        finished: done.len(),
        cars: field.len(),
        first: done.iter().copied().reduce(f32::min),
        last: done.iter().copied().reduce(f32::max),
        touching,
        walled: walled.len(),
        flipped,
    })
}

fn clock(s: f32) -> String {
    format!("{}:{:04.1}", (s / 60.0).floor() as u32, s % 60.0)
}

fn print_outcome(o: &Outcome) {
    let r = &o.line;
    println!(
        "{:<10} · {:<6} · hat {:>6.0} m · kirişte {} bacak · yolsuz {:>3} · onarılan {:>3} · köprülenen {:>3} \
         · basamak {} · en dik %{:>3.0} · en dar {:>5.1} m · en yavaş {:>3.0} km/h · ideal tur {} · grid yönü {:+.2} \
         · duvar {}{} · bitiren {}/{} · birinci {} · sonuncu {} · temas {}",
        o.name,
        if o.circuit { "devre" } else { "sprint" },
        r.length,
        o.chords,
        r.off_road,
        r.repaired,
        r.bridged,
        r.steps,
        r.steepest * 100.0,
        r.tightest,
        r.slowest * 3.6,
        clock(r.lap_time),
        o.heading_cos,
        o.walled,
        if o.flipped { " · halka ters çevrildi" } else { "" },
        o.finished,
        o.cars,
        o.first.map_or_else(|| "—".into(), clock),
        o.last.map_or_else(|| "—".into(), clock),
        o.touching,
    );
}

fn summary(all: &[Outcome], total: usize) {
    let finished_all = all.iter().filter(|o| o.finished == o.cars).count();
    let cars: usize = all.iter().map(|o| o.cars).sum();
    let finished: usize = all.iter().map(|o| o.finished).sum();
    let backwards = all.iter().filter(|o| o.heading_cos < 0.0).count();
    let touching: usize = all.iter().map(|o| o.touching).sum();
    let samples: usize = all.iter().map(|o| o.line.samples).sum();
    let off: usize = all.iter().map(|o| o.line.off_road).sum();
    let steps: usize = all.iter().map(|o| o.line.steps).sum();
    let walled: usize = all.iter().map(|o| o.walled).sum();
    let flipped = all.iter().filter(|o| o.flipped).count();
    let metres: f32 = all.iter().map(|o| o.line.length).sum();
    println!("\n== özet");
    println!("koşulan yarış {} / {total} · hepsi bitiren yarış {finished_all} · bitiren araba {finished}/{cars}", all.len());
    println!(
        "hat toplamı {:.0} km · örnek {samples} · yolsuz %{:.1} · basamak {steps} · duvardan geçen adım {walled} · grid yönüne ters hat {backwards} · gride göre çevrilen halka {flipped} · temas adımı {touching}",
        metres / 1000.0,
        100.0 * off as f32 / samples.max(1) as f32,
    );
    let mut worst: Vec<&Outcome> = all.iter().collect();
    worst.sort_by(|a, b| {
        let share = |o: &Outcome| o.line.off_road as f32 / o.line.samples.max(1) as f32;
        share(b).total_cmp(&share(a))
    });
    println!("en kötü hatlar (yolsuz payı):");
    for o in worst.iter().take(8) {
        println!(
            "   {:<10} yolsuz {:>4} / {} örnek · basamak {} · kirişte {} bacak",
            o.name, o.line.off_road, o.line.samples, o.line.steps, o.chords
        );
    }
}
