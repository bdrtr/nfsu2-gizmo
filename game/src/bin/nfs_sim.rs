//! # Race a field with no window (M4)
//!
//! `nfs_cruise` with the eyes taken out. It loads the same city, builds the same network and the
//! same waypoints, puts the same cars on the same grid and steps the same physics — and then, in
//! place of a frame, prints where everyone got to.
//!
//! **Why it exists.** Every question about the drivers so far has been answered by running the
//! game for a hundred seconds and reading the telemetry, and a sweep over one parameter is five of
//! those: eight minutes of wall clock, a window that must stay open, and a person who has to be
//! there. Simulated, the same run costs a fraction of that and any number of them can go in a loop.
//!
//! A GPU is still opened, headless, because [`spawn_car`] uploads textures and builds materials —
//! the car is real, not a stand-in. What is skipped is presenting it.
//!
//! ```bash
//! NFS_ROUTE="$NFSU2_ROOT/TRACKS/ROUTESL4RA/Paths4001.bin" \
//!   cargo run --release -p nfsu2 --bin nfs_sim -- "$NFSU2_ROOT/TRACKS" "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
//! ```
//!
//! Env: `NFS_SECONDS=<n>` how long to simulate (default 120) · `NFS_RIVALS=<n>` how many cars ·
//! plus everything the world and the pilots read (`NFS_WALL`, `NFS_TIERS`, …).
//!
//! Prints one line per rival at the end and a summary the caller can sort on. The summary is the
//! point: **junctions taken and waypoints reached** are what say a driver is getting somewhere,
//! and a metre count is not — a car spinning against a wall covers distance.

use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use gizmo::renderer::Renderer;
use nfsu2::rig::{spawn_car, CarRig, Controls, Pilot, Placement, FIXED_DT};
use nfsu2::scene;
use nfsu2::world as city;

/// How far apart the driven waypoints are after the outline is subdivided.
///
/// Short enough that a grid is never far from one and long enough that a pilot is not chasing a
/// point under its own bumper.
const WAYPOINT_STEP: f32 = 40.0;

/// How much of a car's course loss `NFS_LOST=1` keeps, in 20 Hz samples.
///
/// Eight seconds before and two after. `lost` only calls a departure permanent once the car has
/// been off the corridor for three continuous seconds, so the first three of those eight are spent
/// getting back to the crossing itself and the remaining five are the approach — about 90 m at the
/// 65 km/h the corner is taken at, which is two or three nodes of run-up.
const LOST_BEFORE: usize = 160;
const LOST_AFTER: usize = 40;

/// How slow counts as standing still, in m/s, and how long the grid is left alone first.
///
/// The same 0.7 m/s the pilot's own stall rule uses and the same three seconds it settles for, so
/// "this car is stopped" means the same thing in the measurement as in the thing being measured.
const STILL_SPEED: f32 = 0.7;
const SETTLE: f32 = 3.0;
/// How far ahead another car has to be to count as the reason this one is standing still.
///
/// Two car lengths. Short enough that it is the car in front rather than a car somewhere up the
/// road, long enough to cover the 1.3 m of clear air a grid row leaves.
const QUEUE_LOOK: f32 = 9.0;

/// How far to walk the city on from where the world let go, before calling the road unbroken.
///
/// Long enough that a car doing 100 km/h covers it in under three seconds — the scale a driving
/// mistake happens on — and short enough that the answer is about the place it left rather than
/// about the far side of the block.
const PROBE: f32 = 80.0;

/// The height window that probe allows, and deliberately the same number as `NFS_WALL`'s default.
///
/// A road climbs and dips, and `Network::drop_walled` measured what happens to a tighter tolerance:
/// it cuts real roads. Reusing the value means a gap found here is a gap by the same standard the
/// network already routes by, rather than a second opinion with its own threshold to argue about.
const PROBE_SLACK: f32 = 8.0;

/// Where a car last had the world under it.
///
/// A fall is *noticed* at the bottom, and the bottom is the least informative place to look at one:
/// by then every car that left is a kilometre under an empty map at terminal velocity, and they all
/// look the same whatever went wrong. What separates the causes is the moment the ground let go.
#[derive(Clone, Copy)]
struct Contact {
    /// When, in simulated seconds.
    t: f32,
    /// Where the car was.
    at: Vec3,
    /// Which way it was actually travelling, flattened and normalised — measured from the step it
    /// just took, not from where its nose pointed. A car sliding sideways off a kerb leaves along
    /// its velocity, and the whole question is what is out there in the direction it left.
    going: Vec3,
    /// Speed in m/s. Passing through a triangle is a speed failure and driving off an edge is not,
    /// so the number is worth carrying before anything is concluded from it.
    speed: f32,
    /// Plan-view distance to the nearest of the race's own paths.
    off: f32,
    /// How long it had been outside the course corridor by then, unbroken.
    ///
    /// The question a barrier has to answer before it is worth building: a car that fell the instant
    /// it left the course needs a fence at the kerb, and one that had been off it for seconds needs
    /// something that noticed earlier. They are not the same fix and the numbers say which.
    off_for: f32,
}

/// One car's fall, as far as the sim can witness it.
#[derive(Clone, Copy, Default)]
struct Fall {
    /// The last pose it properly **stood** at, and what it was doing there. Not merely the last
    /// thing it touched: a car dropping through a hole clips the underside of the world on the way
    /// past, and taking that as the departure point puts the whole diagnosis inside the fall.
    last: Option<Contact>,
    /// How far below that pose it is now.
    below: f32,
    /// The highest it got above it since. A car that was *launched* was over the edge in the air
    /// and no barrier at ground level would have stopped it; a car that drove off one never left
    /// the surface it was on. Same ending, different bug.
    peak: f32,
    /// Whether it ever stood anywhere at all. False means the grid place itself has never been
    /// shown to be ground.
    ever: bool,
    /// Unbroken seconds spent outside the course corridor, right now.
    off_for: f32,
}

/// One instant of a car losing the course, as the pilot saw it.
///
/// **Why a trace and not another sweep.** Three levers have now been swept at the one corner that
/// takes six of eight cars on `Paths4121` — brake threshold, steering rate, aim distance — and all
/// three were refuted, twice by a half-sweep that read as a win. A sweep answers "did this help";
/// it cannot say *which term let go*, and after three refutations that is the question. Every field
/// here is something the pilot either asked for or was looking at, so the failing one can be read
/// off the seconds before the car went wide rather than guessed at from where it ended up.
#[derive(Clone, Copy)]
struct Moment {
    /// Simulated seconds.
    t: f32,
    at: Vec3,
    /// m/s, signed the way the pose reports it.
    speed: f32,
    /// Plan-view distance to the nearest path — the number that crossing
    /// [`city::COURSE_HALF_WIDTH`] is what "left the course" means.
    off: f32,
    /// What the pilot asked for this step: steering input (−1..1), throttle, brake.
    steer: f32,
    throttle: f32,
    brake: f32,
    /// How far ahead the aim point was, and how far off the nose — the pure-pursuit pair. A corner
    /// taken wide with a small angle is the lookahead reaching past the bend; a large angle with
    /// the wheel not following is the smoothing.
    aim: Option<(f32, f32)>,
    /// The node the pilot was holding, and how far the car was from it.
    node: Option<(u32, f32)>,
    /// Where that waypoint actually was — distance and bearing off the nose, like [`Self::aim`].
    ///
    /// The aim walk steers the graph *toward this point*, so an aim that comes back behind the car
    /// has two possible causes and they need opposite fixes: the goal itself is behind (the
    /// waypoint counter is stuck) or the goal is ahead and the graph walk to it starts by going
    /// backwards. Nothing else in the trace separates them.
    goal_at: Option<(f32, f32)>,
    /// How far the **next** waypoint was, in plan.
    ///
    /// The counter advances when this is smaller than the distance to the one being held, so the
    /// two numbers side by side are the whole of why a goal does or does not move on. On a bend
    /// tight enough, the next waypoint is *further away* than the one just driven past, and then
    /// the rule has no reason to fire however far behind the held one gets.
    next_at: Option<f32>,
    /// The waypoint it was heading for, and how many nodes it had given up on by then.
    ///
    /// A goal that stops advancing and a blacklist that keeps growing are the two ways the pilot
    /// can be steering at something the road does not lead to, and neither is visible in the
    /// steering itself.
    goal: usize,
    given_up: usize,
    /// Whether an escape manoeuvre was overriding the aim. Without this the trace cannot tell the
    /// road's own aim from a reverse-out, and the two are opposite findings.
    escaping: bool,
}

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";

/// A picture of what the city has to stand on around a point, for when a distance is not enough.
///
/// A gap measured along one line says the road ran out; it does not say whether the car slipped
/// through a seam between two meshes that should have met, or drove off the edge of a world that
/// simply stops there. Those need different fixes — one is an assembly bug, the other is the
/// barriers `ROADMAP.md` §M4 says have to be derived — and a 80 m square of the answer tells them
/// apart at a glance.
///
/// `#` drivable surface within [`PROBE_SLACK`] of the centre's height: road this car could be on.
/// `:` a surface at that XZ, but too far above or below to be the same road — another deck, a roof.
/// `.` nothing at all. `O` is the point itself and `>` is 10 m along the way it was going.
fn ground_map(ground: &city::Ground, at: Vec3, going: Vec3, half: f32, step: f32) -> String {
    let n = (half / step) as i32;
    let ahead = at + going * 10.0;
    let mut out = String::new();
    for iz in -n..=n {
        for ix in -n..=n {
            let (x, z) = (at.x + ix as f32 * step, at.z + iz as f32 * step);
            let near = |p: Vec3| (p.x - x).abs() <= step * 0.5 && (p.z - z).abs() <= step * 0.5;
            let hs = ground.heights_at(x, z);
            out.push(if near(at) {
                'O'
            } else if near(ahead) {
                '>'
            } else if hs.iter().any(|h| (h - at.y).abs() <= PROBE_SLACK) {
                '#'
            } else if hs.is_empty() {
                '.'
            } else {
                ':'
            });
        }
        out.push('\n');
    }
    out
}

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let tracks = city::tracks_path(std::env::args().nth(1));
    let car_path = std::env::args()
        .nth(2)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .unwrap_or_else(|| DEFAULT_CAR.to_string());
    let seconds: f32 =
        std::env::var("NFS_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(120.0);

    let route = std::env::var("NFS_ROUTE").expect("NFS_SIM needs NFS_ROUTE: a race to drive");
    let region = city::bundle_for_route(std::path::Path::new(&route));
    let source = region.map_or_else(|| tracks.clone(), |b| b.display().to_string());
    let city::Bundles { meshes, .. } = city::load(&source);

    // The same preparation the game does, and in the same order: the drivers must be given the
    // world the player would have been given, or the answer is about a different city.
    let meshes = city::dedup(meshes);
    // NFS_COLLIDE=all: build the collision from **everything** the bundle ships except the backdrop
    // — distant-LOD proxies kept, coarse tiers kept, nothing dropped.
    //
    // The one experiment that separates "the city has a hole here" from "our assembly opened one".
    // Both exclusions the drawn set applies are *visual* decisions — a kilometre-wide proxy plane at
    // arm's length is worse than none, a coarse tier over its own fine version is a blurry box — and
    // collision inherited both without ever being asked whether they applied to it. They may not: a
    // plane you cannot see through is perfectly good to stand on.
    //
    // **Answered, and the answer is no.** 582,304 collision triangles become 734,880 with the 2,100
    // coarse tiers put back, and over the eight routes the result is identical: the same ten cars
    // fall off the same three places. The ground the cars need is not in the bundle under any
    // filter. Kept as a knob because that is a claim worth being able to re-check in one run.
    let collide_all = std::env::var("NFS_COLLIDE").is_ok_and(|v| v == "all");
    // What each filter costs, printed rather than assumed. A knob whose effect is invisible is a
    // knob that can be swept all day against a set it never changed.
    let (backdrop, distant) = meshes.iter().fold((0usize, 0usize), |(b, d), m| {
        (
            b + usize::from(city::is_backdrop(&m.header.name)),
            d + usize::from(city::is_distant_lod(&m.header.name)),
        )
    });
    let objects: Vec<_> = meshes
        .into_iter()
        .filter(|m| {
            if collide_all {
                !city::is_backdrop(&m.header.name)
            } else {
                city::is_drawn(&m.header.name)
            }
        })
        .collect();
    let before = objects.len();
    let objects = if collide_all { objects } else { city::lod::keep_finest(objects) };
    println!(
        "objects: {before} kept of {} · {backdrop} backdrop, {distant} distant-LOD proxies · \
         {} coarse tiers dropped{}",
        before + backdrop + distant,
        before - objects.len(),
        if collide_all { " · NFS_COLLIDE=all: nothing dropped" } else { "" }
    );
    let colliders = city::collision_cells(&objects);
    let ground = city::Ground::of(&colliders);
    // **A measurement, not a mechanism.** Both ways of acting on a barrier through the *graph* were
    // swept and refuted (`ROADMAP.md`), and what is left to try is the pilot's aim — so the first
    // question is whether a pilot is in fact steering at points with something standing in the way,
    // and on which routes. `NFS_AIMWALL=<metres>` is the lift the question is asked at.
    //
    // It answered, the answer became `Pilot`'s way-round rule, and that rule is now on by default —
    // so what this measures today is the **residual**: how often a pilot is still aiming through
    // something after the rule has had its go. For the number the rule was written from, run it
    // with `NFS_AIMCLEAR=0`.
    let aim_lift: f32 =
        std::env::var("NFS_AIMWALL").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let walls = city::Walls::of(&colliders);
    // Which cells have anything to drive on at all. A fall inside the mapped city and a fall past
    // its edge are different findings, and only this can tell them apart.
    let bounds = city::Bounds::of(&colliders);

    let bytes = std::fs::read(&route).unwrap_or_else(|e| panic!("read {route}: {e}"));
    let nodes = gizmo_nfs::world::routes::nodes(&bytes).expect("read the route file's nodes");
    let event: u16 = std::path::Path::new(&route)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim_start_matches(|c: char| !c.is_ascii_digit()))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let catalogue = gizmo_nfs::world::routes::events(&bytes).unwrap_or_default();
    let ev = catalogue.iter().find(|e| e.id == event);

    // The race's own line, as something to ask "is this car still on the course" of. The same
    // construction `nfs_cruise` draws, so the sim and the game cannot disagree about where the
    // course is.
    let corridor =
        city::Corridor::of(&city::build_route(&nodes, &city::road_ground(&objects)), city::COURSE_HALF_WIDTH);

    let mut net = city::Network::of(&nodes, &ground);
    let (edges, dead, steep, walled) = net.shape();
    println!(
        "network: {} nodes · {edges} links · {dead} with no way out · {steep} steeper than 1:1 · \
         {walled} dropped because the road does not continue along them",
        net.len()
    );
    // Subdivided, because the outline's own corners are up to 425 m apart — see `route::densify`.
    let coarse: Vec<Vec3> = ev
        .map(|e| e.outline.iter().map(|p| city::remap([p[0], p[1], 0.0])).collect())
        .unwrap_or_default();
    let step: f32 =
        std::env::var("NFS_WPSTEP").ok().and_then(|v| v.parse().ok()).unwrap_or(WAYPOINT_STEP);
    // **The ring, from the roads rather than across them.** `NFS_WALKLINE=0` goes back to the
    // straight chord between outline corners — which is what every measurement before 2026-08-20
    // was taken against, and what put 55 % of the ring outside the corridor.
    let walk = std::env::var("NFS_WALKLINE").is_ok_and(|v| v != "0");
    let waypoints = if walk {
        // **A link with a wall across it is not a road, and the ring is where that matters.** The
        // graph joins carriageways that merely run beside each other, so a committed shortest path
        // crosses joins a car cannot take — measured on the first version of this ring, 15
        // consecutive-waypoint pairs on 4001 alone with something standing between them. Tested
        // once per link here rather than inside the search, and given only to the course builder:
        // the graph the *pilot* walks is left exactly as it is, because a wall filter inside
        // `drop_walled` was swept and thrown out once already.
        let mut blocked: std::collections::HashSet<(u32, u32)> = Default::default();
        if std::env::var("NFS_WALKWALLS").map_or(true, |v| v != "0") {
            for i in 0..net.len() as u32 {
                let Some(a) = net.node(i) else { continue };
                for &l in &a.links {
                    if l <= i {
                        continue;
                    }
                    let Some(b) = net.node(l) else { continue };
                    if walls.across(&ground, a.at, b.at, 0.5, 3.0) {
                        blocked.insert((i, l));
                        blocked.insert((l, i));
                    }
                }
            }
            println!("kurs kurulumunda duvarlı sayılan bağlantı: {}", blocked.len() / 2);
        }
        // `NFS_WALKLEGS=1`: what each leg of the outline costs in road, against the chord it
        // replaces. A ring that doubles in length is either driving real roads round real blocks or
        // taking a detour, and only the per-leg numbers tell those apart.
        if std::env::var("NFS_WALKLEGS").is_ok() {
            println!("anahat bacakları (kiriş → yol):");
            for (k, cw) in coarse.windows(2).enumerate() {
                let plan = |a: Vec3, b: Vec3| Vec3::new(b.x - a.x, 0.0, b.z - a.z).length();
                let chord = plan(cw[0], cw[1]);
                let road = net
                    .nearest(cw[0])
                    .zip(net.nearest(cw[1]))
                    .and_then(|(a, b)| net.path_where(a, b, |x, y| !blocked.contains(&(x, y))))
                    .map(|ids| {
                        ids.windows(2)
                            .filter_map(|p| {
                                Some(plan(net.node(p[0])?.at, net.node(p[1])?.at))
                            })
                            .sum::<f32>()
                    });
                match road {
                    Some(d) => println!(
                        "   {k:>2}: ({:>7.0},{:>7.0}) → ({:>7.0},{:>7.0}) · {chord:>5.0} m → \
                         {d:>5.0} m  (×{:.1})",
                        cw[0].x,
                        cw[0].z,
                        cw[1].x,
                        cw[1].z,
                        d / chord.max(1.0)
                    ),
                    None => println!(
                        "   {k:>2}: ({:>7.0},{:>7.0}) → ({:>7.0},{:>7.0}) · {chord:>5.0} m → yol yok",
                        cw[0].x, cw[0].z, cw[1].x, cw[1].z
                    ),
                }
            }
        }
        // How far round a leg may go before the graph is admitting it has no road for it.
        let detour: f32 =
            std::env::var("NFS_WALKDETOUR").ok().and_then(|v| v.parse().ok()).unwrap_or(3.0);
        let (w, chords) =
            city::along_roads(&net, &coarse, step, detour, |a, b| !blocked.contains(&(a, b)));
        // **The caution this inherits, measured rather than assumed.** A committed shortest path
        // over this graph crosses joins a car cannot take — the graph links carriageways that
        // merely run beside each other, which is why `guide_to` was refuted in the driving role
        // (`ROADMAP.md`, `Network` header). A ring drawn through such a join drags the pilot
        // sideways across a central reservation, so count them: consecutive ring points with
        // something standing between them at car height.
        let crossed = w
            .windows(2)
            .filter(|p| walls.across(&ground, p[0], p[1], 0.5, 3.0))
            .count();
        println!(
            "yarış hattı ağda yürünerek kuruldu: {} waypoint · {chords} bacak yol bulunamayıp \
             kirişte kaldı · {crossed} ardışık nokta arasında araç boyunda engel var",
            w.len()
        );
        w
    } else {
        city::densify(&coarse, step)
    };
    // **Tell the graph which of its roads this race uses.** Without it the walk picks the neighbour
    // nearest the goal in a straight line, and on Bayview that is regularly a parallel carriageway:
    // measured, 21 of 21 departures from the racing line had an arm that would have stayed on it.
    // `NFS_ONLINE=0` turns the preference off; the width is in metres and defaults to a waypoint's
    // spacing, which is the scale at which "this node belongs to the race" stops being a question.
    let online: f32 =
        std::env::var("NFS_ONLINE").ok().and_then(|v| v.parse().ok()).unwrap_or(WAYPOINT_STEP);
    net.mark_line(&waypoints, online);
    {
        let (on, all) = net.on_line();
        println!("yarış hattındaki düğüm: {on} / {all} (genişlik {online} m)");
    }

    // The grid, through the same choice the game makes.
    let dir = std::path::Path::new(&route).parent().expect("route has a directory");
    let markers = std::fs::read(dir.join("TrackPosMarkersAll.bin"))
        .ok()
        .and_then(|b| gizmo_nfs::world::routes::markers(&b).ok())
        .unwrap_or_default();
    let (slots, heading) = ev
        .and_then(|e| city::start_slots(&markers, e))
        .expect("this race names no full starting grid");
    println!("grid: {} places · {} waypoints", slots.len(), waypoints.len());
    // Where the course starts relative to the grid. The pilot heads for waypoint 0 first, and
    // nothing says waypoint 0 is anywhere near the line — an outline is a closed ring drawn from
    // wherever its author began.
    if let Some(first) = slots.first() {
        let d = |w: &Vec3| ((w.x - first.x).powi(2) + (w.z - first.z).powi(2)).sqrt();
        let near = waypoints
            .iter()
            .enumerate()
            .min_by(|a, b| d(a.1).total_cmp(&d(b.1)))
            .map(|(i, w)| (i, d(w)));
        println!(
            "  waypoint 0 is {:.0} m from the grid · nearest is {:?}",
            waypoints.first().map_or(f32::NAN, d),
            near.map(|(i, m)| format!("#{i} at {m:.0} m"))
        );
    }

    // Headless, but a real device: the car's materials and textures are built the way the game
    // builds them, so a sim result is about the same car.
    assert!(Renderer::headless_adapter_available().await, "no GPU adapter for a headless run");
    let renderer = Renderer::new_headless(64, 64, None).await;
    let mut world = World::new();
    let mut assets = AssetManager::new();
    let mut phys = PhysicsWorld::new();

    // The city, as physics only — nothing is drawn, so nothing needs a mesh. Same cells and the
    // same trimesh per cell the game builds, because a driver that hits different geometry from
    // the player's is answering a different question.
    let mut triangles = 0;
    for cell in colliders {
        triangles += cell.triangles();
        let at_cell = Transform::new(cell.origin);
        let entity = world.spawn();
        nfsu2::geom::add_transform(&mut world, entity, at_cell);
        let collider = Collider::trimesh(cell.vertices, cell.indices);
        world.add_component(entity, RigidBody::new_static());
        world.add_component(entity, Velocity::default());
        world.add_component(entity, collider.clone());
        world.add_component(entity, gizmo::physics::components::PhysicsMaterial::ASPHALT);
        phys.add_body(
            gizmo::physics::BodyHandle::from_id(entity.id()),
            RigidBody::new_static(),
            at_cell,
            Velocity::default(),
            collider,
        );
    }
    println!("city: {triangles} collision triangles");

    let rivals: usize = std::env::var("NFS_RIVALS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(slots.len());
    // NFS_SLOT_FROM=<k>: leave the first k places empty. The one way to ask whether a car that
    // stops is stopping because of where it starts or because of the car in front of it.
    let from: usize =
        std::env::var("NFS_SLOT_FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut field: Vec<(CarRig, Pilot)> = Vec::new();
    // NFS_SPREAD=<x>: push the grid apart by this factor about its own centre. Not a feature —
    // the one experiment that separates "the driver cannot drive here" from "the driver cannot
    // drive next to another driver", which is a real distinction because the pilot has no traffic
    // model at all.
    let spread: f32 = std::env::var("NFS_SPREAD").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let centre = slots.iter().fold(Vec3::ZERO, |a, s| a + *s) / slots.len() as f32;
    let slots: Vec<Vec3> = slots.iter().map(|s| centre + (*s - centre) * spread).collect();
    for (k, slot) in slots.iter().enumerate().skip(from).take(rivals) {
        // What the grid place actually offers. A slot whose marker floats, or that has no road
        // under it at all, is a car that starts in trouble — and that is a different failure from
        // a car that gets going and hits something.
        let surfaces = ground.heights_at(slot.x, slot.z);
        let stand = ground
            .height_at(*slot + Vec3::Y * 2.0)
            .map_or(*slot, |y| Vec3::new(slot.x, y, slot.z));
        println!(
            "  slot {k}: ({:>7.1},{:>7.1}) marker y={:>6.2} · stands at {:>6.2} ({:+.2}) · \
             surfaces {:?}",
            slot.x,
            slot.z,
            slot.y,
            stand.y,
            stand.y - slot.y,
            surfaces.iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>()
        );
        let rig = spawn_car(
            &mut world,
            &renderer,
            &mut assets,
            &mut phys,
            &car_path,
            Placement::facing(stand, heading, 1.5),
        );
        let mut pilot = Pilot::new();
        pilot.place(stand, heading, &net, &waypoints);
        // NFS_STAGGER=<s>: seconds between one car pulling away and the next.
        let stagger: f32 =
            std::env::var("NFS_STAGGER").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        pilot.hold_for(stagger * k as f32);
        if let Some(j) = pilot.node().and_then(|i| net.node(i)) {
            println!(
                "           -> node {:?} at ({:>7.1},{:>6.1},{:>7.1}), {:.1} m away, {} links",
                pilot.node().unwrap_or(0),
                j.at.x,
                j.at.y,
                j.at.z,
                (j.at - stand).length(),
                j.links.len()
            );
        }
        field.push((rig, pilot));
    }
    world.insert_resource(assets);
    world.insert_resource(phys);
    scene::add_lights(&mut world, Transform::new(slots[0] + Vec3::Y * 60.0), 2.0, slots[0]);
    println!("field: {} cars", field.len());

    // The race itself: a countdown, laps if the event says it is a circuit, and a running order.
    let mut race = nfsu2::race::Race::new(waypoints.len(), ev.is_some_and(|e| e.circuit));
    println!(
        "race: {} · {} waypoints · finish at {} driven · {:.0} s countdown",
        if ev.is_some_and(|e| e.circuit) {
            format!("{} laps", nfsu2::race::Race::LAPS)
        } else {
            "sprint".to_string()
        },
        waypoints.len(),
        race.distance(),
        race.countdown()
    );

    // Fixed steps, not frames: a simulation has no reason to pretend it is being watched, and a
    // fixed step is what makes two runs of the same parameters give the same answer.
    // NFS_TRACE=<n>: print the field every n simulated seconds. A summary says where everyone
    // ended; this says when they stopped, which is a different question and usually the useful one.
    let trace: f32 = std::env::var("NFS_TRACE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let mut next_trace = trace;
    let watch: Option<usize> = std::env::var("NFS_WATCH").ok().and_then(|v| v.parse().ok());
    let floor: Option<usize> = std::env::var("NFS_FLOOR").ok().and_then(|v| v.parse().ok());
    let fence = std::env::var("NFS_FENCE").ok().is_none_or(|v| v != "0");
    let mut held = 0usize;
    // When each car last got further round the course, and how far it has been from the node it
    // holds. A field that stops is not one thing: a car still gaining waypoints at the final second
    // only wants more seconds, a car that stopped at t=20 is stuck, and a car a hundred metres from
    // its own held node has lost the graph. The summary cannot tell those apart and they need
    // different work.
    let mut along = vec![0usize; field.len()];
    let mut moved_at = vec![0.0f32; field.len()];
    let mut strayed = vec![0.0f32; field.len()];
    let mut falls: Vec<Fall> = vec![Fall::default(); field.len()];
    let mut was: Vec<Option<Vec3>> = vec![None; field.len()];
    let mut aim_seen = vec![0usize; field.len()];
    let mut aim_walled = vec![0usize; field.len()];
    // NFS_LOST=1: keep the last few seconds before each car loses the course, and a couple after.
    //
    // A ring while nothing has happened, frozen and then extended once it has, because the moment
    // worth seeing can only be recognised three seconds after it — that is how long off the
    // corridor `lost` waits before calling a departure permanent, so a buffer that started at the
    // announcement would begin well past the cause.
    let losing = std::env::var("NFS_LOST").is_ok();
    // **A car that stops is as much a departure as one that drives off, and the ring buffer was
    // only catching the second.** On `Paths4021` three cars stand still on flat open ground with
    // eleven of twelve directions clear, the pilot asking 0.03 throttle, seventeen escapes that
    // moved it zero metres and a blacklist holding all but seven of its hundred-odd ways out —
    // and none of it is traced, because `lost` never fires for a car that never leaves the course.
    // Standing still this long is the trigger for the same window.
    let stuck_for: f32 =
        std::env::var("NFS_STUCK").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
    let mut standing = vec![0.0f32; field.len()];
    let mut ring: Vec<std::collections::VecDeque<Moment>> =
        vec![std::collections::VecDeque::new(); field.len()];
    let mut around: Vec<Option<Vec<Moment>>> = vec![None; field.len()];
    // Where and when a car first stood still long enough to be worth a trace.
    let mut stuck: Vec<Option<(f32, Vec3)>> = vec![None; field.len()];
    // **Why a car stopped, which the summary cannot say.** A field that stops is not one thing, and
    // the three that matter want completely different work: a car pinned by the fence, a car
    // grinding against geometry, and a car queued behind another car look identical in every number
    // printed so far. Counted per car so the answer is per car.
    let mut fenced = vec![0usize; field.len()];
    let mut still = vec![0usize; field.len()];
    let mut rolled = vec![0usize; field.len()];
    let mut rescued = vec![0usize; field.len()];
    let mut cmd = vec![(0.0f32, 0.0f32, 0.0f32); field.len()];
    let mut spoke = vec![false; field.len()];
    // **Does the escape actually escape.** The stall rule reverses at -0.7 throttle; whether the
    // car then moves backwards is a different question from whether it was told to, and negative
    // throttle in a forward gear is a brake rather than a reverse. Counted as displacement along
    // the car's own nose, so backwards is negative and a car that reverses two metres and drives
    // back into the same wall does not read the same as one that never moved.
    let mut back_ticks = vec![0usize; field.len()];
    let mut back_move = vec![0.0f32; field.len()];
    let mut prev_pos: Vec<Option<Vec3>> = vec![None; field.len()];
    // NFS_WRONGWAY=1: every step of the graph walk that leaves the course, and whether it had
    // anywhere else to go.
    //
    // **The question this exists to settle.** The trace says 28 of the 50 cars that lose the course
    // do it steering at a point behind them, and that the aim points backwards because the *goal*
    // is behind — the car having been walked onto a road the course does not take. Softening the
    // response has now been refuted three times, so the question moved upstream: when the walk
    // steps off the course, was there an arm at that junction that would have stayed on it? Those
    // are opposite findings. One is a choice a rule can make better; the other is a graph that does
    // not contain the road, and no steering rule fixes that.
    //
    // **The first version of this asked the wrong thing and answered zero.** It compared the held
    // node against the *corridor*, and the corridor is built from every path in the route file
    // merged together — so a car driving a different carriageway of the same route is "on course"
    // by that test however far it is from the race. It measured zero on two routes and the zero was
    // real and useless.
    //
    // What it asks now is the distance from the held node to the nearest **waypoint**, which is the
    // race's own line. That is the number that was hiding: on `Paths4121` the cars run at 60 km/h
    // inside the corridor with their goal waypoint **150-159 m** away, and the place where seven of
    // eight finally cross out has no waypoint within 80 m of it.
    let wrongway = std::env::var("NFS_WRONGWAY").is_ok();
    let mut held_node: Vec<Option<u32>> = vec![None; field.len()];
    // **A car cannot leave a course it was never on.** Some grids sit outside the corridor: on
    // `Paths4002` the start line is 21 m from the nearest waypoint and **six of eight cars begin
    // 12.3-19.4 m off it**, so the departure rule below fired for all six at t=3 s, before they had
    // driven anywhere. Every count of "lost the course" carried them. Traced, all eight of the
    // field's "already crawling when it left" cases were this and nothing else.
    let mut entered = vec![false; field.len()];
    // (t, car, from, to, how far `to` is off the course, how many arms stayed on it)
    let mut strayed_at: Vec<(f32, usize, u32, u32, f32, usize)> = Vec::new();
    let mut silent = vec![0usize; field.len()];
    let mut asked = vec![(0.0f32, 0.0f32); field.len()];
    let mut footed = vec![0usize; field.len()];
    let mut touching = vec![0usize; field.len()];
    let mut touched = vec![0usize; field.len()];
    // How much of its own weight the car's wheels were carrying while it stood still. Four
    // grounded wheels and no load on any of them is a car sitting on its belly, which no steering
    // rule can reach; four loaded ones that still go nowhere are pinned against something.
    let mut loaded = vec![0.0f32; field.len()];
    // What the drivetrain delivered while it stood there. Asked-for pedal and delivered torque
    // are different numbers, and only the second one says whether standing still is a drivetrain
    // failure or a traction one.
    let mut geared = vec![0.0f32; field.len()];
    // Steps spent in neutral, not the mean gear: a car alternating between neutral and second
    // averages the same 1.5 as one sitting in a gear that does not exist, and only the fraction
    // tells those apart.
    let mut neutral = vec![0usize; field.len()];
    let mut revved = vec![0.0f32; field.len()];
    let mut torqued = vec![0.0f32; field.len()];
    let mut spun = vec![0.0f32; field.len()];
    // When and where each car first lost the corridor for good, and how far along it was.
    let mut lost: Vec<Option<(f32, Vec3, usize)>> = vec![None; field.len()];
    let rescue = std::env::var("NFS_RESCUE").is_ok_and(|v| v != "0");
    let mut ticks = vec![0usize; field.len()];
    let mut queued = vec![0usize; field.len()];
    let steps = (seconds / FIXED_DT) as usize;
    for step in 0..steps {
        let now = step as f32 * FIXED_DT;
        if trace > 0.0 && now >= next_trace {
            next_trace += trace;
            let line: Vec<String> = field
                .iter()
                .map(|(r, p)| {
                    let s = r.pose(&world).map_or(0.0, |q| q.speed * 3.6);
                    format!("{:>3.0}/{:<2}", s, p.passed())
                })
                .collect();
            println!("  t={now:>5.1}s  km/h·junctions: {}", line.join(" "));
        }
        // NFS_WATCH=<k>: one car, in detail, at the trace interval. Where it is, where its pilot
        // is on the network, and how far apart those two are — a car that is not moving and a node
        // that is not advancing look the same in a summary and are different failures.
        if let Some(k) = watch {
            if trace > 0.0 && (now / trace).fract() < FIXED_DT / trace {
                if let Some((rig, pilot)) = field.get(k) {
                    if let Some(q) = rig.pose(&world) {
                        let n = pilot.node().and_then(|i| net.node(i));
                        let (nx, ny, nz, gap) = n.map_or((0.0, 0.0, 0.0, 0.0), |j| {
                            (j.at.x, j.at.y, j.at.z, (j.at - q.position).length())
                        });
                        // Which way is up, and what is underneath. A car on its roof reports a
                        // pose like any other and sits perfectly still, which from a summary is
                        // indistinguishable from a car against a wall.
                        let up = (q.rotation * Vec3::Y).y;
                        let under = ground.heights_at(q.position.x, q.position.z);
                        // The aim point as well as the held node. They are *supposed* to differ —
                        // the aim walks the network forward past the node until it is a lookahead
                        // away — and a trace showing only the node makes a pilot driving correctly
                        // past a sticky node look identical to one ignoring its own graph.
                        let aim = pilot.aim().unwrap_or(Vec3::ZERO);
                        let off = aim - q.position;
                        println!(
                            "    watch {k} t={now:>5.1}s ({:>7.1},{:>6.2},{:>7.1}) {:>5.1} km/h · \
                             up {up:>5.2} · under {under:?} · \
                             node {:?} ({:>7.1},{:>6.2},{:>7.1}) {gap:>5.1} m · \
                             aim ({:>7.1},{:>7.1}) {:>5.1} m · {} junctions · waypoint {}",
                            q.position.x,
                            q.position.y,
                            q.position.z,
                            q.speed * 3.6,
                            pilot.node(),
                            nx,
                            ny,
                            nz,
                            aim.x,
                            aim.z,
                            Vec3::new(off.x, 0.0, off.z).length(),
                            pilot.passed(),
                            pilot.goal()
                        );
                    }
                }
            }
        }
        race.tick(FIXED_DT);
        // Where everyone is, read before anyone is driven, so all eight pilots see the same field.
        // A car's own position is in here and does not need taking out: it is not ahead of itself.
        let traffic: Vec<Vec3> =
            field.iter().filter_map(|(r, _)| r.pose(&world).map(|p| p.position)).collect();
        // Controls first for the whole field, then one physics step: every car sees the same
        // world state, which a loop that stepped physics per car would not give.
        for (k, (rig, pilot)) in field.iter_mut().enumerate() {
            if race.holding() {
                rig.drive(
                    &mut world,
                    &nfsu2::rig::Controls {
                        throttle: 0.0,
                        brake: 1.0,
                        steer: 0.0,
                        toggle_auto_shift: false,
                    },
                );
                continue;
            }
            let Some(pose) = rig.pose(&world) else { continue };
            // A pilot with no node returns nothing at all, and a car that is told nothing is not
            // being driven badly — it is not being driven. That is a third state next to "asked for
            // throttle" and "asked for brake", and it has to be counted or it hides inside them.
            spoke[k] = false;
            // NFS_FLOOR=<k>: take the pilot out of the loop for one car and hold the throttle down,
            // wheels straight. The pilot's reasoning and the car's ability to move are two separate
            // claims, and every measurement that goes through `pilot.drive` tests them together —
            // a car that will not move under a pedal nobody is second-guessing has a fault that no
            // steering rule, waypoint or escape can be responsible for.
            if floor == Some(k) {
                // NFS_FLOORSTEER holds a lock along with the pedal. Full throttle with the wheels
                // straight tests two things at once — that the car can move and that the pilot's
                // pedal was the thing stopping it — and only holding the lock separates them.
                let st: f32 = std::env::var("NFS_FLOORSTEER")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                let c = Controls { throttle: 1.0, brake: 0.0, steer: st, toggle_auto_shift: false };
                cmd[k] = (c.throttle, c.brake, c.steer);
                spoke[k] = true;
                rig.drive(&mut world, &c);
                continue;
            }
            if let Some(c) =
                pilot.drive(
                    FIXED_DT,
                    pose.position,
                    pose.rotation,
                    pose.speed,
                    &net,
                    &waypoints,
                    &traffic,
                    Some((&walls, &ground)),
                )
            {
                // What the pilot is *asking* for, kept so a car that is not moving can be asked
                // whether it is being told to go. A car standing still under full throttle and a
                // car standing still because nobody pressed anything are different failures.
                cmd[k] = (c.throttle, c.brake, c.steer);
                spoke[k] = true;
                rig.drive(&mut world, &c);
            }
        }
        gizmo::physics::vehicle_controller_system(&world, FIXED_DT);
        // The fence, between the forces and the integration, so the step that would have carried a
        // car over the lip is the step that does not. NFS_FENCE=0 turns it off, which is the only
        // way to say what it is worth.
        if fence {
            for (k, (rig, _)) in field.iter_mut().enumerate() {
                let Some(p) = rig.pose(&world) else { continue };
                let hit = rig.hold_at_edge(&mut world, p, &ground);
                held += usize::from(hit);
                fenced[k] += usize::from(hit);
            }
        }
        gizmo::physics::physics_step_system(&world, FIXED_DT);

        // Where the world last held each car up — the same question `keep_in_world` asks in the
        // game, through the same code, with the rescue left out. Watching a field leave the world
        // and catching it are different jobs, and only the first one answers why.
        for (k, (rig, pilot)) in field.iter_mut().enumerate() {
            let Some(p) = rig.pose(&world) else { continue };
            let mut still_now = false;
            // Counted only once the grid has been left alone to settle, for the same reason the
            // pilot's own stall rule waits: a car being dropped on the line is not a car that has
            // stopped.
            if now > SETTLE {
                ticks[k] += 1;
                // **On its side.** Nothing in the game puts a car back on its wheels — falling has
                // `keep_in_world` and the map edge has `hold_at_edge`, and rolling over has
                // nothing — so this is terminal, and it has to be counted separately because a
                // flipped car goes on *taking junctions*: its pilot walks the graph exactly as
                // before while the car itself has not moved a millimetre.
                if (p.rotation * Vec3::Y).y < 0.5 {
                    rolled[k] += 1;
                }
                if p.speed.abs() < STILL_SPEED {
                    still[k] += 1;
                    still_now = true;
                    // Standing still *behind somebody* is a different failure from standing still
                    // against a wall, and it is the one that needs no new mechanism — the traffic
                    // rule is already doing what it was asked to.
                    let f = Vec3::new(
                        (p.rotation * Vec3::NEG_Z).x,
                        0.0,
                        (p.rotation * Vec3::NEG_Z).z,
                    )
                    .normalize_or_zero();
                    let side = Vec3::new(-f.z, 0.0, f.x);
                    if traffic.iter().enumerate().any(|(j, o)| {
                        let d = Vec3::new(o.x - p.position.x, 0.0, o.z - p.position.z);
                        j != k && d.dot(f) > 0.0 && d.dot(f) < QUEUE_LOOK && d.dot(side).abs() < 2.0
                    }) {
                        queued[k] += 1;
                    }
                }
            }
            let round = pilot.along(waypoints.len());
            if round > along[k] {
                along[k] = round;
                moved_at[k] = now;
            }
            // Sampled every tenth step because it is a rate, not an event.
            if let Some(a) = pilot.aim().filter(|_| aim_lift > 0.0 && step % 10 == 0) {
                aim_seen[k] += 1;
                aim_walled[k] += usize::from(walls.across(&ground, p.position, a, aim_lift, 3.0));
            }
            if let Some(j) = pilot.node().and_then(|i| net.node(i)) {
                let d = Vec3::new(j.at.x - p.position.x, 0.0, j.at.z - p.position.z).length();
                strayed[k] = strayed[k].max(d);
            }
            if wrongway {
                let now_node = pilot.node();
                if now_node != held_node[k] {
                    // Distance from a node to the race's own line — the nearest waypoint, not the
                    // corridor, for the reason in the declaration above.
                    let off = |i: u32| {
                        net.node(i).map_or(f32::INFINITY, |n| {
                            waypoints
                                .iter()
                                .map(|w| Vec3::new(w.x - n.at.x, 0.0, w.z - n.at.z).length())
                                .fold(f32::INFINITY, f32::min)
                        })
                    };
                    if let (Some(a), Some(b)) = (held_node[k], now_node) {
                        // Only the step that crosses out matters: from a node on the race's line to
                        // one off it. A walk already outside has nothing left to choose wrongly.
                        if off(a) <= WAYPOINT_STEP && off(b) > WAYPOINT_STEP {
                            // How many of the arms out of `a` — other than the one it came from —
                            // would have stayed on the course. This is the whole of the finding.
                            let kept = net
                                .node(a)
                                .map_or(0, |n| n.links.iter().filter(|l| off(**l) <= WAYPOINT_STEP).count());
                            strayed_at.push((now, k, a, b, off(b), kept));
                        }
                    }
                    held_node[k] = now_node;
                }
            }
            // **Whether the rivals get the net the player has always had.** `keep_in_world` is
            // called for `state.rig` in all three windowed binaries and for nobody else, so a
            // rival that falls stays fallen and a rival that rolls stays rolled — in the game as
            // well as in here. This sim watches rather than catches on purpose, because only
            // watching answers *why*; but "the field is not caught at all" is a decision that has
            // never been measured, so it is switchable. `NFS_RESCUE=1` gives the field the net.
            //
            // `keep_in_world` runs `watch_ground` itself, so it is one or the other — calling both
            // would advance the airborne clock twice a step.
            let g = if rescue {
                if !matches!(rig.keep_in_world(&mut world, p, FIXED_DT), nfsu2::rig::Rescue::None) {
                    rescued[k] += 1;
                }
                // Read the footing back without advancing anything a second time. The pose is the
                // pre-rescue one, which is the one the fall diagnostics are about.
                rig.watch_ground(&world, p, 0.0)
            } else {
                rig.watch_ground(&world, p, FIXED_DT)
            };
            // **Standing still with the wheels off the ground is a different failure from standing
            // still with them on it.** On the ground means the car is being driven into something
            // it cannot climb, or is not being driven at all; off it means the car is resting on
            // its floor — high-centred on a kerb — where no amount of throttle reaches the road.
            // The two want different work and the stopped count cannot tell them apart.
            if cmd[k].0 < 0.0 {
                back_ticks[k] += 1;
                if let Some(b) = prev_pos[k] {
                    let nose = p.rotation * Vec3::NEG_Z;
                    let f = Vec3::new(nose.x, 0.0, nose.z).normalize_or_zero();
                    back_move[k] += (p.position - b).dot(f);
                }
            }
            prev_pos[k] = Some(p.position);
            if still_now {
                if g.on_all_four {
                    footed[k] += 1;
                }
                asked[k].0 += cmd[k].0;
                asked[k].1 += cmd[k].1;
                silent[k] += usize::from(!spoke[k]);
                let (down, _) = rig.wheels_down(&world);
                touching[k] += down;
                touched[k] += 1;
                loaded[k] += rig.wheel_load(&world);
                let (gear, rpm, torque, spin) = rig.drivetrain(&world);
                geared[k] += gear as f32;
                neutral[k] += usize::from(gear == 1);
                revved[k] += rpm;
                torqued[k] += torque;
                spun[k] += spin;
            }
            let f = &mut falls[k];
            f.below = g.below;
            f.ever = g.ever;
            let off = corridor.locate(p.position).map_or(f32::INFINITY, |x| x.distance);
            if off > city::COURSE_HALF_WIDTH {
                f.off_for += FIXED_DT;
            } else {
                f.off_for = 0.0;
            }
            // **Where the course was lost, not how far it strayed afterwards.** A car ends its run
            // 60-130 m off the line, and that number says nothing about the moment it went wrong —
            // by then it has been wandering for minutes. This records the first departure that
            // *stuck*: off the corridor for three continuous seconds, which is long enough not to
            // count a corner cut and short enough to still be near the cause.
            if off <= city::COURSE_HALF_WIDTH {
                entered[k] = true;
            }
            // The clock that says "this one has stopped", reset by any real movement.
            if p.speed.abs() < STILL_SPEED {
                standing[k] += FIXED_DT;
            } else {
                standing[k] = 0.0;
            }
            if losing && around[k].is_none() && stuck_for > 0.0 && standing[k] >= stuck_for {
                around[k] = Some(ring[k].iter().copied().collect());
                stuck[k] = Some((now, p.position));
            }
            if entered[k] && lost[k].is_none() && f.off_for >= 3.0 {
                lost[k] = Some((now, p.position, pilot.covered()));
                // Freeze what led here. `LOST_BEFORE` is counted back from *this* instant, which is
                // already three seconds after the car crossed the line, so the window has to be
                // long enough to reach behind that or it shows only the aftermath.
                if losing {
                    around[k] = Some(ring[k].iter().copied().collect());
                }
            }
            // Sampled at 20 Hz rather than every step: the terms below are smoothed and a 240 Hz
            // dump of them is twelve identical lines per reading.
            if losing && step % 12 == 0 {
                let nose = p.rotation * Vec3::NEG_Z;
                let fwd = Vec3::new(nose.x, 0.0, nose.z).normalize_or_zero();
                let side = Vec3::new(-fwd.z, 0.0, fwd.x);
                let m = Moment {
                    t: now,
                    at: p.position,
                    speed: p.speed,
                    off,
                    steer: cmd[k].2,
                    throttle: cmd[k].0,
                    brake: cmd[k].1,
                    aim: pilot.aim().map(|a| {
                        let d = Vec3::new(a.x - p.position.x, 0.0, a.z - p.position.z);
                        (d.length(), d.dot(side).atan2(d.dot(fwd)).to_degrees())
                    }),
                    goal_at: waypoints.get(pilot.goal()).map(|w| {
                        let d = Vec3::new(w.x - p.position.x, 0.0, w.z - p.position.z);
                        (d.length(), d.dot(side).atan2(d.dot(fwd)).to_degrees())
                    }),
                    next_at: waypoints
                        .get((pilot.goal() + 1) % waypoints.len().max(1))
                        .map(|w| Vec3::new(w.x - p.position.x, 0.0, w.z - p.position.z).length()),
                    goal: pilot.goal(),
                    given_up: pilot.given_up().len(),
                    escaping: pilot.escaping().is_some(),
                    node: pilot.node().and_then(|i| net.node(i)).map(|j| {
                        (
                            pilot.node().unwrap_or_default(),
                            Vec3::new(j.at.x - p.position.x, 0.0, j.at.z - p.position.z).length(),
                        )
                    }),
                };
                match &mut around[k] {
                    // Keep going for a moment past the departure: what the pilot does *while*
                    // losing it is half the evidence — a wheel that finally comes round after the
                    // car is already off says the steering was late, not absent.
                    Some(v) if v.len() < LOST_BEFORE + LOST_AFTER => v.push(m),
                    Some(_) => {}
                    None => {
                        if ring[k].len() >= LOST_BEFORE {
                            ring[k].pop_front();
                        }
                        ring[k].push_back(m);
                    }
                }
            }
            if g.stood {
                let moved = was[k].map_or(Vec3::ZERO, |b| p.position - b);
                let going = Vec3::new(moved.x, 0.0, moved.z).normalize_or_zero();
                // A car that has not moved this step has no direction of travel; where it is
                // pointing is the honest stand-in, and it is only ever used for a car that
                // afterwards falls, which means it was going somewhere.
                let nose = p.rotation * Vec3::NEG_Z;
                let going = if going == Vec3::ZERO {
                    Vec3::new(nose.x, 0.0, nose.z).normalize_or_zero()
                } else {
                    going
                };
                let off_for = f.off_for;
                f.last =
                    Some(Contact { t: now, at: p.position, going, speed: p.speed, off, off_for });
                f.peak = 0.0;
            } else {
                f.peak = f.peak.max(-g.below);
            }
            was[k] = Some(p.position);
        }
    }

    let pilots: Vec<Pilot> = field.iter().map(|(_, p)| p.clone()).collect();
    let order = race.standings(&pilots);
    println!("\nfinishing order after {seconds:.0} s:");
    for (place, s) in order.iter().enumerate() {
        println!(
            "  {}. car {} · {} laps · {} waypoints driven{}",
            place + 1,
            s.car,
            s.laps,
            s.along,
            if s.done { " · FINISHED" } else { "" }
        );
    }
    println!("  {} of {} finished", race.finishers().len(), field.len());

    println!("\nafter {seconds:.0} s:");
    let mut junctions = 0;
    let mut best_waypoint = 0;
    // How far the field actually got, which is the one measure that survives a change to how the
    // waypoints are counted or how many junctions a route happens to have. Straight-line from the
    // grid: crude, and crude in the same way for every run, which is what a comparison needs.
    let mut furthest = 0.0f32;
    let mut fallen = 0usize;
    let mut away = 0usize;
    let line = slots[0];
    // **A car that has driven off the world is not a car that got far.** On one route six of eight
    // ended at -219 to -310 km/h, which is not reverse — it is falling — and one was 2.4 km outside
    // the map. Straight-line distance counted every one of them as progress, so the measure was
    // rewarding the exact failure it was there to detect.
    //
    // The first correction for that was itself wrong, and wrong in the mirror image: "fifty metres
    // below the grid" invents falls on any route with vertical extent. `Paths4061` starts at
    // `y = 323`, descends, and three cars that had driven down and parked — upright, stationary, all
    // four wheels down at the final step — were counted as having left the world. The test is now
    // [`rig::FALL_DEPTH`] below where the car itself last **stood**, which is what the game's own
    // `keep_in_world` has always used and needs no grid to mean something.
    for (k, (rig, pilot)) in field.iter().enumerate() {
        let p = rig.pose(&world);
        let (at, speed) = p.map_or((Vec3::ZERO, 0.0), |p| (p.position, p.speed));
        junctions += pilot.passed();
        best_waypoint = best_waypoint.max(pilot.goal());
        // "Cars that got away" has been quoted in `ROADMAP.md` since the drivers existed and has
        // never been in the code — it was recomputed by eye or by awk from the per-car lines each
        // time, which is why the figures in two entries cannot both be reproduced from either. One
        // junction taken is the definition, and it is printed now so it cannot drift again.
        away += usize::from(pilot.passed() > 0);
        let gone = falls[k].below > nfsu2::rig::FALL_DEPTH;
        if gone {
            fallen += 1;
        } else {
            furthest = furthest
                .max((Vec3::new(at.x, 0.0, at.z) - Vec3::new(line.x, 0.0, line.z)).length());
        }
        println!(
            "  car {k}: ({:>7.0},{:>6.0},{:>7.0}) {:>4.0} km/h{} · {:>4} junctions over {:>3} \
             distinct nodes · {} waypoints driven past, last gained at {:>5.1}s · strayed {:>5.0} m \
             from its node · gave up on {}",
            at.x,
            at.y,
            at.z,
            speed * 3.6,
            if gone { " FALLEN" } else { "" },
            pilot.passed(),
            pilot.seen(),
            pilot.covered(),
            moved_at[k],
            strayed[k],
            pilot.given_up().len()
        );
        if let Some((t, at, wp)) = lost[k] {
            println!(
                "            kursu bıraktı: t={t:>6.1}s · waypoint {wp:>3} · ({:>7.0},{:>6.0},{:>7.0})",
                at.x, at.y, at.z
            );
        } else if entered[k] {
            println!("            kursu hiç bırakmadı");
        } else {
            let off = corridor.locate(at).map_or(f32::INFINITY, |x| x.distance);
            println!(
                "            kursa hiç girmedi — grid koridorun dışında, şu an {off:.0} m uzakta"
            );
        }
        // The approach, in the pilot's own terms. Read down the `koridora` column for the moment it
        // passes 12 and then look left: what the wheel was being asked for, whether the pedal ever
        // came off, and where the aim point was while it happened.
        if let Some((t, at)) = stuck[k] {
            println!(
                "            {} t={t:>6.1}s · ({:>7.0},{:>7.0})",
                if lost[k].is_some() { "önce takıldı:" } else { "takıldı:      " },
                at.x,
                at.z
            );
        }
        if let Some(v) = &around[k] {
            println!(
                "            iz — koridor yarı genişliği {} m · 20 Hz",
                city::COURSE_HALF_WIDTH
            );
            // **What the car actually held**, as opposed to what the pilot asked of it. Worked out
            // here rather than carried in the sample, because the positions are stored at full
            // precision and only *printed* rounded — and the printed metres are far too coarse to
            // differentiate a heading over 50 ms. Lateral acceleration is `v · dψ/dt` from three
            // consecutive samples; it is the number `GRIP` is guessing at, and a pilot that asks
            // for more than the car ever demonstrates is asking to leave the road.
            let lat = |i: usize| -> Option<f32> {
                let (a, b, c) = (v.get(i.checked_sub(1)?)?, v.get(i)?, v.get(i + 1)?);
                let d1 = Vec3::new(b.at.x - a.at.x, 0.0, b.at.z - a.at.z);
                let d2 = Vec3::new(c.at.x - b.at.x, 0.0, c.at.z - b.at.z);
                let dt = c.t - b.t;
                if d1.length() < 0.2 || d2.length() < 0.2 || dt <= 0.0 {
                    return None;
                }
                let turn = d1.normalize().cross(d2.normalize()).y.asin();
                Some((b.speed.abs() * turn / dt).abs())
            };
            for (i, m) in v.iter().enumerate() {
                let aim = m.aim.map_or("          —".to_string(), |(d, a)| {
                    format!("{d:>4.0} m {a:>5.0}°")
                });
                let node = m
                    .node
                    .map_or("     —".to_string(), |(i, d)| format!("{i:>4} {d:>4.0} m"));
                let gw = m.goal_at.map_or("          —".to_string(), |(d, a)| {
                    format!("{d:>4.0} m {a:>5.0}°")
                });
                let nw = m.next_at.map_or("   —".to_string(), |d| format!("{d:>4.0}"));
                println!(
                    "              t={:>6.1} ({:>7.0},{:>7.0}) {:>4.0} km/h · koridora {:>5.1} m \
                     · direksiyon {:>5.2} · gaz {:>4.2} fren {:>4.2} · nişan {aim} · düğüm {node} \
                     · hedef {:>4} {gw} · sonraki {nw} m · yanal {} · vazgeçti {:>2}{}",
                    m.t,
                    m.at.x,
                    m.at.z,
                    m.speed * 3.6,
                    m.off,
                    m.steer,
                    m.throttle,
                    m.brake,
                    m.goal,
                    lat(i).map_or("   —".to_string(), |a| format!("{a:>4.1}")),
                    m.given_up,
                    if m.escaping { " · KAÇIŞ" } else { "" }
                );
            }
        }
    }
    // **Why** they fell, which is not the same question as how many. Each fallen car is traced back
    // to the last step its wheels had anything under them, and the city is then asked what was out
    // there in the direction the car was going. Three answers, and they are three different bugs:
    // the road ran out (the shipped geometry has holes and no barrier chunk to fence them), the
    // road went on without it (the car passed through a triangle that was there — physics, not
    // geometry), or it never touched anything at all (the grid place is over nothing).
    if fallen > 0 {
        println!("\nwhy {fallen} fell:");
    }
    let (mut edge, mut through, mut nowhere) = (0usize, 0usize, 0usize);
    for (k, (rig, _)) in field.iter().enumerate() {
        if falls[k].below <= nfsu2::rig::FALL_DEPTH {
            continue;
        }
        let at = rig.pose(&world).map_or(Vec3::ZERO, |p| p.position);
        let Some(c) = falls[k].last.filter(|_| falls[k].ever) else {
            nowhere += 1;
            println!("  car {k}: never stood anywhere — its grid place has not been shown to be ground");
            continue;
        };
        let verdict = match ground.gap_along(c.at, c.at + c.going * PROBE, PROBE_SLACK, 3.0) {
            Some(d) => {
                edge += 1;
                format!("the road runs out {d:.0} m ahead")
            }
            None => {
                through += 1;
                format!("the road goes on {PROBE:.0} m — it went THROUGH the surface")
            }
        };
        // How far it has travelled sideways since letting go. A car that went through what it was
        // standing on drops more or less where it stood; one that drove off an edge is still
        // carrying the speed that took it there.
        let over = (Vec3::new(at.x, 0.0, at.z) - Vec3::new(c.at.x, 0.0, c.at.z)).length();
        println!(
            "  car {k}: let go at t={:>5.1}s ({:>7.0},{:>6.1},{:>7.0}) doing {:>4.0} km/h, \
             {:>4.1} m of air · {:>4.0} m off the course for {:>4.1} s · \
             now {:>5.0} m down and {over:>4.0} m away{} · {verdict}",
            c.t,
            c.at.x,
            c.at.y,
            c.at.z,
            c.speed * 3.6,
            falls[k].peak,
            c.off,
            c.off_for,
            falls[k].below,
            if bounds.contains(at) { ", inside the map" } else { ", outside the map" },
        );
        // NFS_FALLMAP=<half-width in metres>: draw what the city has around the place it left.
        if let Some(half) = std::env::var("NFS_FALLMAP").ok().and_then(|v| v.parse::<f32>().ok()) {
            print!("{}", ground_map(&ground, c.at, c.going, half, 2.0));
        }
    }
    // **Why they stopped** — the question the `stopped before t=30` column raises and cannot
    // answer. Three different failures wear the same number and want completely different work: a
    // car pinned by the fence, a car queued behind another car, and a car stuck against the city.
    let early: Vec<usize> = (0..field.len()).filter(|k| moved_at[*k] < 30.0).collect();
    // NFS_ARM=<node>: what one junction actually offers, arm by arm.
    //
    // **The question left after four refuted routing rules.** On `Paths4121` every car steps from
    // node 110 to node 111, which is 50.9 m off the racing line, and forcing it to take one of the
    // two arms that stay on the line makes the route three times worse. So the on-line continuation
    // is not drivable in some way the graph does not record, and the only way to find out which way
    // is to ask the city about each arm: how far, which side, on the line or not, road under the
    // whole link or a hole in it, and anything standing across it at car height.
    if let Ok(id) = std::env::var("NFS_ARM").unwrap_or_default().parse::<u32>() {
        if let Some(j) = net.node(id) {
            let near = |p: Vec3| {
                waypoints
                    .iter()
                    .map(|w| Vec3::new(w.x - p.x, 0.0, w.z - p.z).length())
                    .fold(f32::INFINITY, f32::min)
            };
            println!(
                "\ndüğüm {id} · ({:.0},{:.0},{:.0}) · hat {} · yarış hattına {:.0} m · {} kol",
                j.at.x,
                j.at.y,
                j.at.z,
                j.path,
                near(j.at),
                j.links.len()
            );
            for &l in &j.links {
                let Some(n) = net.node(l) else { continue };
                let d = Vec3::new(n.at.x - j.at.x, 0.0, n.at.z - j.at.z);
                // The two questions `drop_walled` and the pilot's own sight ask, on this link.
                let gap = ground.gap_along(j.at, n.at, PROBE_SLACK, 2.0);
                let wall = walls.across_hit(&ground, j.at, n.at, 0.5, 3.0);
                println!(
                    "   → {l:>4} · hat {:>3} · {:>5.0} m · yarış hattına {:>5.0} m {} · zemin {} \
                     · {}",
                    n.path,
                    d.length(),
                    near(n.at),
                    if near(n.at) <= WAYPOINT_STEP { "(HATTA)" } else { "       " },
                    match gap {
                        None => "tam".to_string(),
                        Some(g) => format!("{g:.0} m'de bitiyor"),
                    },
                    match &wall {
                        None => "önü açık".to_string(),
                        Some(h) => {
                            let (lo, hi) = h.over_floor();
                            format!(
                                "{:.0} m'de {:.1}..{:.1} m engel{}",
                                h.along,
                                lo,
                                hi,
                                if h.flat_too { "" } else { " (kat değişimi)" }
                            )
                        }
                    }
                );
            }
        } else {
            println!("\ndüğüm {id} yok");
        }
    }

    // NFS_CURVE=1: the course's own speed limit, from its own geometry.
    //
    // **Why this and not another pilot sweep.** The corner that takes most of `Paths4121` has now
    // been attacked through the brake, the steering rate, the aim distance and the lock, and every
    // one of those asks "what should the driver do". None of them asks the prior question: *how
    // fast can anything go round here at all.* The circle through three consecutive waypoints gives
    // the racing line's own radius, and `v = sqrt(a · r)` with the lateral acceleration the field
    // has actually been measured holding turns that into a speed. Where that speed is below what
    // the cars arrive at, no steering rule can save them and the brake is the only lever; where it
    // is above, the corner is not the problem.
    if std::env::var("NFS_CURVE").is_ok() {
        // Measured, not chosen: the 90th percentile of `v · dψ/dt` over the field's own cornering
        // above 28 km/h is 5.2 m/s² and the 95th is 5.6 (`ROADMAP.md`, 2026-08-20). This is what a
        // 240SX on Bayview's tarmac in this build actually holds, whatever a tyre datasheet says.
        const HELD: f32 = 5.2;
        let radius = |i: usize| -> Option<f32> {
            let n = waypoints.len();
            if n < 3 {
                return None;
            }
            let (a, b, c) = (
                waypoints[(i + n - 1) % n],
                waypoints[i % n],
                waypoints[(i + 1) % n],
            );
            let (u, v) = (
                Vec3::new(b.x - a.x, 0.0, b.z - a.z),
                Vec3::new(c.x - b.x, 0.0, c.z - b.z),
            );
            let (lu, lv) = (u.length(), v.length());
            // Two waypoints on top of each other, or a straight: no circle worth reporting.
            if lu < 1.0 || lv < 1.0 {
                return None;
            }
            let turn = u.normalize().cross(v.normalize()).y.clamp(-1.0, 1.0).asin().abs();
            (turn > 1e-3).then(|| 0.5 * (lu + lv) / turn)
        };
        let mut worst: Vec<(f32, usize)> = (0..waypoints.len())
            .filter_map(|i| radius(i).map(|r| (r, i)))
            .collect();
        worst.sort_by(|a, b| a.0.total_cmp(&b.0));
        println!(
            "\nkursun kendi hız sınırı — {} waypoint, tutulabilen yanal ivme {HELD} m/s²:",
            waypoints.len()
        );
        for (r, i) in worst.iter().take(10) {
            let w = waypoints[*i];
            println!(
                "   waypoint {i:>4} · yarıçap {r:>6.0} m · en fazla {:>5.0} km/h · ({:>7.0},{:>7.0})",
                (HELD * r).sqrt() * 3.6,
                w.x,
                w.z
            );
        }
        // `NFS_CURVE=x,z` walks the waypoints near a place. **The place is what you have**: the
        // per-car report names where a car lost the course, and its "waypoint N" is the *count* it
        // had driven past, not an index — so a coordinate is the only handle on "the corner where
        // they keep going off".
        let spec = std::env::var("NFS_CURVE").unwrap_or_default();
        if let (Some(cx), Some(cz)) = {
            let mut it = spec.split(',').filter_map(|v| v.trim().parse::<f32>().ok());
            (it.next(), it.next())
        } {
            let here = Vec3::new(cx, 0.0, cz);
            println!("   ({cx:.0},{cz:.0}) çevresindeki waypoint'ler:");
            for (j, w) in waypoints.iter().enumerate() {
                let d = Vec3::new(w.x - here.x, 0.0, w.z - here.z).length();
                if d > 80.0 {
                    continue;
                }
                match radius(j) {
                    Some(r) => println!(
                        "     {j:>4} · {d:>4.0} m ötede · yarıçap {r:>6.0} m · en fazla {:>5.0} km/h",
                        (HELD * r).sqrt() * 3.6
                    ),
                    None => println!("     {j:>4} · {d:>4.0} m ötede · düz"),
                }
            }
        }
        if let Ok(i) = spec.parse::<usize>() {
            println!("   waypoint {i} çevresi:");
            let hi = (i + 4).min(waypoints.len().saturating_sub(1));
            for (j, w) in waypoints.iter().enumerate().take(hi + 1).skip(i.saturating_sub(4)) {
                match radius(j) {
                    Some(r) => println!(
                        "     {j:>4} · yarıçap {r:>6.0} m · en fazla {:>5.0} km/h · ({:>7.0},{:>7.0})",
                        (HELD * r).sqrt() * 3.6,
                        w.x,
                        w.z
                    ),
                    None => println!("     {j:>4} · düz · ({:>7.0},{:>7.0})", w.x, w.z),
                }
            }
        }
        // **And whether the line is a road at all.** `densify` lerps between the event outline's
        // corners — 17 points over 6 km, median step 425 m — so the waypoints between two corners
        // are a straight chord across whatever is there. This asks the city and the graph about
        // each one, which is the question every routing rule tried today assumed away.
        let (mut off_net, mut no_ground, mut off_cor) = (0usize, 0usize, 0usize);
        for w in &waypoints {
            let near = (0..net.len() as u32)
                .filter_map(|i| net.node(i))
                .map(|n| Vec3::new(n.at.x - w.x, 0.0, n.at.z - w.z).length())
                .fold(f32::INFINITY, f32::min);
            off_net += usize::from(near > WAYPOINT_STEP);
            no_ground += usize::from(ground.heights_at(w.x, w.z).is_empty());
            off_cor += usize::from(
                corridor.locate(*w).map_or(f32::INFINITY, |x| x.distance) > city::COURSE_HALF_WIDTH,
            );
        }
        println!(
            "   hattın kendisi: {} waypoint'in {} tanesi en yakın düğümden {WAYPOINT_STEP} m'den \
             uzak, {} tanesinin altında hiç zemin yok, {} tanesi koridorun dışında",
            waypoints.len(),
            off_net,
            no_ground,
            off_cor
        );
        // **What the ring does with itself.** Being on the road is necessary and not sufficient: a
        // ring can be entirely on tarmac and still double back, or visit one street twice, or reach
        // its next corner by a detour. Those are the shapes that would explain why the walked ring
        // wins on `Paths4081` and loses on 4001 and 4121, and none of them shows up in the
        // on-the-road counts above.
        {
            let plan = |a: Vec3, b: Vec3| Vec3::new(b.x - a.x, 0.0, b.z - a.z);
            let total: f32 = waypoints.windows(2).map(|w| plan(w[0], w[1]).length()).sum();
            // A step that turns more than 150° is the ring folding back on itself.
            let folds = waypoints
                .windows(3)
                .filter(|w| {
                    let (u, v) = (plan(w[0], w[1]), plan(w[1], w[2]));
                    u.length() > 0.5
                        && v.length() > 0.5
                        && u.normalize().dot(v.normalize()) < -0.87
                })
                .count();
            // And a place the ring comes back to: within 10 m of a point it left more than five
            // waypoints ago. A lap would be one; a street driven twice is many.
            let revisits = waypoints
                .iter()
                .enumerate()
                .filter(|(i, w)| {
                    waypoints
                        .iter()
                        .enumerate()
                        .any(|(j, o)| j + 5 < *i && plan(**w, *o).length() < 10.0)
                })
                .count();
            println!(
                "   halkanın şekli: {:.0} m uzunluk · {folds} yerde kendi üstüne katlanıyor \
                 · {revisits} waypoint daha önce geçilmiş bir yere dönüyor",
                total
            );
        }
        let under = |kmh: f32| {
            worst.iter().filter(|(r, _)| (HELD * r).sqrt() * 3.6 < kmh).count()
        };
        println!(
            "   {} waypoint 40 km/h'nin, {} tanesi 60'ın, {} tanesi 80'in altında bir sınır dayatıyor",
            under(40.0),
            under(60.0),
            under(80.0)
        );
    }

    // NFS_BLOCKED=1: walk the racing line itself and ask whether anything stands in it.
    // Everything else here measures the cars; this measures the road. A building sitting in the
    // course would look, from the cars' side, exactly like six of them running wide at one
    // corner — and a player reports hitting buildings on the roads.
    //
    // It sat inside `if !early.is_empty()` and had no business being there: a measurement **of the
    // road** that only runs when a car happened to stop in the first thirty seconds is a
    // measurement whose absence says nothing, and on a route where every car keeps moving the
    // question would go silently unasked.
    if std::env::var("NFS_BLOCKED").is_ok() {
        // Before anything about walls: do the route's own nodes agree with the ground they are
        // supposed to sit on? Every height query in this file walks from the node's y, so if
        // the graph and the collision surface disagree the walls answer is about the wrong
        // floor. This is the prior question and it had not been asked.
        {
            let (mut n, mut off2, mut off5, mut none) = (0usize, 0usize, 0usize, 0usize);
            let mut worst = 0.0f32;
            for i in 0..net.len() as u32 {
                let Some(j) = net.node(i) else { continue };
                if corridor.locate(j.at).map_or(f32::INFINITY, |x| x.distance)
                    > city::COURSE_HALF_WIDTH
                {
                    continue;
                }
                n += 1;
                let hs = ground.heights_at(j.at.x, j.at.z);
                match hs
                    .into_iter()
                    .min_by(|a, b| (a - j.at.y).abs().total_cmp(&(b - j.at.y).abs()))
                {
                    None => none += 1,
                    Some(h) => {
                        let d = (h - j.at.y).abs();
                        worst = worst.max(d);
                        if d > 2.0 {
                            off2 += 1;
                        }
                        if d > 5.0 {
                            off5 += 1;
                        }
                    }
                }
            }
            println!(
                "\nrota dugumleri zeminle uyusuyor mu: {n} dugumun {off2}'si 2 m'den, {off5}'i 5 m'den \
                 uzak · {none} tanesinin altinda hic zemin yok · en kotu {worst:.1} m"
            );
        }
        // **How the walk's own evidence is read.** The bool this scan used to ask was
        // withdrawn as a finding: 32 of the 36 walls it reported on 4002 stood in multi-level
        // places, where the walk follows the ground onto a deck and then answers about the road
        // underneath it. `across_hit` hands back what it met and how the floor behaved on the
        // way, and these constants are what turn that into a verdict instead of a suspicion.
        //
        // **The step no road takes.** A carriageway climbs, and the block above measures the
        // route's nodes as sitting on the ground to 0.0 m, so the course's own gradient is
        // whatever the nodes say. Two and a half metres inside a single three-metre step is a
        // 40° face; that is a change of surface, not a gradient.
        const DECK_STEP: f32 = 2.5;
        // **How far the walk may drift from the course's own height.** The nodes sit on the
        // ground exactly, so the line between two of them is the course's elevation to within
        // the road's curvature over at most 45 m. Three metres is above that and well under the
        // nine-metre deck separation that produced the false finding.
        const DECK_OFF: f32 = 3.0;
        // A kerb's top over the road it stands on. Below this the wheels ride over it.
        const KERB: f32 = 0.4;
        // And the other end of the car: geometry whose **lowest** point clears this stands over the
        // road rather than in it. Not the soffit under a bridge — a soffit is horizontal, which
        // makes it `Surface::Drivable`, which keeps it out of `Walls` altogether — but the
        // near-vertical things that come with one: a parapet, a deck's edge fascia, the underside
        // rail of a gantry. The walk rides 0.5 m up and its step tilts with the floor, so it can
        // meet one; a car 1.3 m to the roof cannot.
        const CAR_TOP: f32 = 1.3;
        // How many rejected hits an edge is allowed before the scan stops asking about it. Every
        // retry advances at least 1.5 m along an edge of at most 45 m, so this is a guard against a
        // pathological edge rather than a limit anything real reaches.
        const RETRIES: usize = 8;

        println!("\nyaris hatti boyunca engel taramasi:");
        let (mut blocked, mut checked) = (0usize, 0usize);
        // The three ways a hit can be the walk's own doing rather than the road's, counted apart
        // because they fail differently: `flat_too` is about the step that hit, `climb` about the
        // worst step before it, and the offset about where the walk ended up.
        let (mut no_flat, mut climbed, mut off_course) = (0usize, 0usize, 0usize);
        // Hits thrown out, and edges by the hit that decided them.
        let (mut deck, mut deck_multi) = (0usize, 0usize);
        let (mut kerb, mut soffit, mut wall) = (0usize, 0usize, 0usize);
        let (mut wall_multi, mut wall_wide) = (0usize, 0usize);
        // What the second and later questions bought: edges that came back clear once the walk was
        // resumed past a rejected hit, and edges whose real blocker was only found by resuming.
        let (mut cleared, mut recovered, mut gave_up) = (0usize, 0usize, 0usize);
        let mut shown = 0usize;
        for i in 0..net.len() as u32 {
            let Some(a) = net.node(i) else { continue };
            for &l in &a.links {
                if l <= i {
                    continue;
                }
                let Some(b) = net.node(l) else { continue };
                let ina = corridor.locate(a.at).map_or(f32::INFINITY, |x| x.distance);
                let inb = corridor.locate(b.at).map_or(f32::INFINITY, |x| x.distance);
                if ina > city::COURSE_HALF_WIDTH || inb > city::COURSE_HALF_WIDTH {
                    continue;
                }
                // Long links are the graph's shortcuts between distant nodes; a straight line
                // between them crosses buildings because the road curves, and that is not a
                // defect. Only adjacent-node edges say anything about the road itself.
                //
                // The cut is on the **full** length rather than the plan-view one, because that is
                // what the withdrawn scan cut on and the whole point of this run is that its
                // denominator is the same. `run` below is the walk's own ground-plane run and is a
                // different number on a slope.
                if (b.at - a.at).length() > 45.0 {
                    continue;
                }
                let run = Vec3::new(b.at.x - a.at.x, 0.0, b.at.z - a.at.z).length();
                checked += 1;
                // **Rejecting a hit is not the same as clearing the way.** `across_hit` returns at
                // the first thing it meets and the rest of the walk never runs, so an edge whose
                // first hit is the walk's own change of deck has not been shown to be clear — only
                // that *that* blocker was not real. Without asking again the scan would trade one
                // wrong answer for a blank one. So it resumes from just past the rejected hit.
                let dir = Vec3::new(b.at.x - a.at.x, 0.0, b.at.z - a.at.z).normalize_or_zero();
                let mut from = a.at;
                let mut thrown = 0usize;
                let mut bailed = false;
                let real = loop {
                    let Some(hit) = walls.across_hit(&ground, from, b.at, 0.5, 3.0) else {
                        break None;
                    };
                    if thrown == 0 {
                        blocked += 1;
                    }
                    // Where the course itself is, at the point the walk stopped. Measured from the
                    // edge's own start rather than from where this attempt resumed, so the number
                    // means the same thing on the first question and the fourth.
                    let along =
                        Vec3::new(hit.at.x - a.at.x, 0.0, hit.at.z - a.at.z).length();
                    let course_y = a.at.y + (b.at.y - a.at.y) * (along / run.max(1e-3)).clamp(0.0, 1.0);
                    let off = hit.walk_y - course_y;
                    let (f_flat, f_climb, f_off) =
                        (!hit.flat_too, hit.climb > DECK_STEP, off.abs() > DECK_OFF);
                    if f_flat || f_climb || f_off {
                        no_flat += usize::from(f_flat);
                        climbed += usize::from(f_climb);
                        off_course += usize::from(f_off);
                        deck += 1;
                        deck_multi += usize::from(hit.layers > 1);
                        thrown += 1;
                        if thrown > RETRIES {
                            gave_up += 1;
                            bailed = true;
                            break None;
                        }
                        // Half a walk step past it, so the same face cannot answer twice, and along
                        // the edge rather than along whatever the walk's own drift was doing.
                        from = hit.at + dir * 1.5;
                        if (b.at - from).dot(dir) <= 0.0 {
                            break None;
                        }
                        continue;
                    }
                    break Some((hit, off, along));
                };
                let Some((hit, off, along)) = real else {
                    // Given up on is not cleared: the edge has no verdict either way.
                    cleared += usize::from(thrown > 0 && !bailed);
                    continue;
                };
                recovered += usize::from(thrown > 0);
                let (under, over) = hit.over_floor();
                if over < KERB {
                    kerb += 1;
                    continue;
                }
                if under >= CAR_TOP {
                    soffit += 1;
                    continue;
                }
                wall += 1;
                wall_multi += usize::from(hit.layers > 1);
                // **The one alternative explanation left, and it has to be measured too.** The walk
                // goes node to node in a straight line and the road bends between them: on a curve
                // the chord leaves the carriageway, and a building on the outside of the bend is
                // then "in the way" of a line no car would drive. The corridor knows where the road
                // actually is, so ask it about the hit point itself rather than about the nodes.
                let off_line = corridor.locate(hit.at).map_or(f32::INFINITY, |x| x.distance);
                wall_wide += usize::from(off_line > 6.0);
                if shown < 12 {
                    shown += 1;
                    println!(
                        "   DUVAR {i:>4} -> {l:>4}  ({:>7.0},{:>7.0})  kenarin {along:>4.0} m'sinde \
                         · yolun {under:>5.1}..{over:>5.1} m arasini kesiyor · zemin {:>2} kat \
                         · kot farki {off:>5.1} m · yol ekseninden {off_line:>5.1} m",
                        hit.at.x, hit.at.z, hit.layers
                    );
                }
            }
        }
        println!(
            "   hattin {blocked} / {checked} kenarinda ilk yurumede onunu kesen bir sey var ({:.1}%)",
            100.0 * blocked as f32 / checked.max(1) as f32
        );
        println!(
            "   {deck} vurus elendi: {no_flat} tanesinde duz adim hicbir seye degmiyor, {climbed} \
             tanesi tek adimda {DECK_STEP} m'den fazla kat degistirmis, {off_course} tanesi kursun \
             kotundan {DECK_OFF} m'den uzakta"
        );
        println!(
            "   elenen vurustan sonra yurume devam etti: {cleared} kenar temiz cikti, {recovered} \
             kenarda gercek engel ancak devam edince bulundu, {gave_up} kenarda {RETRIES} denemede \
             karar verilemedi"
        );
        println!(
            "   {kerb} kenar bordur boyunda (<{KERB} m), {soffit} kenarda engel arabanin ustunden \
             geciyor (en alti {CAR_TOP} m'den yuksek)"
        );
        println!(
            "   geriye {wall} gercek duvar kaliyor ({:.1}%) — yolun kendi kotunda, arabanin \
             carpacagi yukseklikte",
            100.0 * wall as f32 / checked.max(1) as f32
        );
        println!(
            "   cok katli yerde: duvarlarin {wall_multi}/{wall} tanesi, elenen vuruslarin \
             {deck_multi}/{deck} tanesi — yani cok katli olmak tek basina eleme sebebi degil"
        );
        println!(
            "   duvarlarin {wall_wide}/{wall} tanesi yol ekseninden 6 m'den uzakta: iki dugum \
             arasindaki duz cizgi virajda karsiya tasiyor olabilir, geri kalan {} tanesi \
             dogrudan yarisin surdugu cizgide",
            wall - wall_wide
        );
    }

    // **Is there anywhere to go?** Three ways of choosing a different *node* have now been
    // refuted (blacklist, shun the heading, expire the list), and the conclusion was that the
    // answer has to come from outside the node machine — from the city's own geometry. Before
    // writing that, the question it assumes has to be asked: does a stuck car actually have
    // open ground around it that it is failing to use, or is it genuinely boxed in? Twelve
    // directions, drivable ground at the car's own height, nothing standing across the way.
    if !early.is_empty() {
        // NFS_HOLE=x,z: is there ground there? A car that stops or falls at a particular place is
        // asking a question about the world, not about its driver, and the cheapest honest answer
        // is a grid of height queries around the spot.
        if let Ok(spec) = std::env::var("NFS_HOLE") {
            let mut it = spec.split(',').filter_map(|v| v.trim().parse::<f32>().ok());
            if let (Some(cx), Some(cz)) = (it.next(), it.next()) {
                println!("\nground coverage around ({cx:.0}, {cz:.0}), 8 m steps:");
                for iz in -6..=6 {
                    let z = cz + iz as f32 * 8.0;
                    let row: String = (-6..=6)
                        .map(|ix| {
                            let x = cx + ix as f32 * 8.0;
                            match ground.heights_at(x, z).into_iter().next() {
                                Some(_) => '#',
                                None => '.',
                            }
                        })
                        .collect();
                    println!("   z={z:>7.0}  {row}");
                }
                println!("   ('#' = zemin var, '.' = yok · orta sütun/satır sorulan nokta)");
                // How far is the racing line from here? A hole beside the course and a hole in it
                // are different findings: the first blames whatever pushed the car off the line,
                // the second blames the world.
                let here = Vec3::new(cx, 0.0, cz);
                let mut best = (f32::INFINITY, 0u32);
                for i in 0..net.len() as u32 {
                    if let Some(n) = net.node(i) {
                        let d = (Vec3::new(n.at.x, 0.0, n.at.z) - here).length();
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                }
                if let Some(n) = net.node(best.1) {
                    println!(
                        "   en yakın rota düğümü: {:.0} m ötede, ({:.0},{:.0},{:.0})",
                        best.0, n.at.x, n.at.y, n.at.z
                    );
                }
                let on_course = corridor.locate(here).map(|x| x.distance);
                println!("   koridora uzaklık: {on_course:?} (yarı genişlik {})", city::COURSE_HALF_WIDTH);
                // The branches on offer here, and which of them the race is actually on. `path` is
                // which of the file's paths a node belongs to, so a junction whose links span
                // several paths is exactly where "follow the network" and "follow the race" part
                // company.
                if let Some(j) = net.node(best.1) {
                    println!("   kavşak {}: hat {} · {} kol", best.1, j.path, j.links.len());
                    for &l in &j.links {
                        if let Some(n) = net.node(l) {
                            let d = corridor
                                .locate(n.at)
                                .map_or(f32::INFINITY, |x| x.distance);
                            let on = if d <= city::COURSE_HALF_WIDTH { "YARIŞ HATTI" } else { "yan yol" };
                            println!(
                                "     → düğüm {l:>4} · hat {:>3} · ({:>7.0},{:>7.0}) · koridora {d:>6.1} m · {on}",
                                n.path, n.at.x, n.at.z, d = d
                            );
                        }
                    }
                }
            }
        }
        if wrongway {
            println!(
                "\nyürüyüş yarış hattından nerede çıktı (waypoint'e {WAYPOINT_STEP} m'den uzak), \
                 ve başka kolu var mıydı:"
            );
            let (mut had, mut none) = (0usize, 0usize);
            for (t, k, a, b, off, kept) in &strayed_at {
                if *kept > 0 {
                    had += 1;
                } else {
                    none += 1;
                }
                if had + none <= 12 {
                    println!(
                        "   t={t:>6.1} · araba {k} · düğüm {a} → {b} · yeni düğüm koridordan \
                         {off:>5.1} m · kursta kalan kol: {kept}"
                    );
                }
            }
            println!(
                "   toplam {} çıkış · {had} tanesinde kursta kalan bir kol VARDI · {none} tanesinde yoktu",
                strayed_at.len()
            );
        }
        println!("\nwhat the stuck cars have around them:");
        for &k in &early {
            let Some((rig, _)) = field.get(k) else { continue };
            let Some(p) = rig.pose(&world) else { continue };
            let mut open = 0usize;
            let mut best = f32::MIN;
            let mut best_deg = 0.0f32;
            for i in 0..12 {
                let a = i as f32 * std::f32::consts::TAU / 12.0;
                let dir = Vec3::new(a.cos(), 0.0, a.sin());
                // How far the ground holds along that heading, up to 20 m.
                let reach = ground
                    .gap_along(p.position, p.position + dir * 20.0, PROBE_SLACK, 2.0)
                    .unwrap_or(20.0);
                // And whether anything stands across it at car height.
                let clear = !walls.across(&ground, p.position, p.position + dir * reach, 0.5, 3.0);
                if reach > 8.0 && clear {
                    open += 1;
                }
                if clear && reach > best {
                    best = reach;
                    best_deg = a.to_degrees();
                }
            }
            let nose = p.rotation * Vec3::NEG_Z;
            let facing = nose.z.atan2(nose.x).to_degrees().rem_euclid(360.0);
            // The ground straight ahead, metre by metre. The twelve-ray probe walks its segments
            // three metres at a time and only knows the surfaces that made it into `Walls`, so a
            // kerb, a bollard or a low barrier half a metre in front of the bumper passes through
            // that sieve untouched and the direction still reads "open". A car pushing 3600 N into
            // something it cannot climb is exactly what that blind spot would look like, and a
            // one-metre profile of its own nose line is what settles it.
            let ahead = Vec3::new(nose.x, 0.0, nose.z).normalize_or_zero();
            let under = |q: Vec3, from: f32| {
                ground
                    .heights_at(q.x, q.z)
                    .into_iter()
                    .min_by(|x, y| (x - from).abs().total_cmp(&(y - from).abs()))
            };
            let mut here = under(p.position, p.position.y).unwrap_or(p.position.y);
            let mut profile = Vec::new();
            for m in 1..=10 {
                let q = p.position + ahead * m as f32;
                match under(q, here) {
                    Some(h) => {
                        profile.push(format!("{:+.1}", h - here));
                        here = h;
                    }
                    None => profile.push("  ?".into()),
                }
            }
            println!("      ground ahead, metre by metre: {}", profile.join(" "));
            println!(
                "  car {k}: {open}/12 directions open past 8 m · best {best:>5.1} m at \
                 {best_deg:>5.0}° · car faces {facing:>5.0}°"
            );
        }
    }

    if !early.is_empty() {
        println!("\nwhy {} stopped gaining before t=30:", early.len());
        for k in early {
            let t = ticks[k].max(1);
            let s = 100.0 * still[k] as f32 / t as f32;
            let q = if still[k] > 0 { 100.0 * queued[k] as f32 / still[k] as f32 } else { 0.0 };
            println!(
                "  car {k}: still for {s:>3.0}% of the race · {q:>3.0}% of that behind another car · \
                 on its side for {:>3.0}% · fence held it {:>3} times · \
                 four wheels down for {:>3.0}% of the standing, {:>3.1} wheels on average \
                 carrying {:>4.2} of its own weight · \
                 in gear {:>4.1} ({:>3.0}% of it neutral) at {:>5.0} rpm delivering {:>6.0} Nm \
                 into wheels turning \
                 {:>5.1} rad/s · \
                 asked for {:>4.2} throttle and {:>4.2} brake while standing, told nothing \
                 at all {:>3.0}% of it · \
                 reversed for {:>4.1}s and went {:>5.1} m along its own nose doing it · \
                 escaped {:>3} times and moved {:>6.1} m doing it · \
                 gave up {:>3} times and {:>3} of those pointed it the same way again, out of \
                 {:>4} ways on of which {:>4} pointed elsewhere and {:>4} were not already \
                 given up on · \
                 last gained at {:>4.1}s",
                100.0 * rolled[k] as f32 / t as f32,
                fenced[k],
                if still[k] > 0 { 100.0 * footed[k] as f32 / still[k] as f32 } else { 0.0 },
                if touched[k] > 0 { touching[k] as f32 / touched[k] as f32 } else { 0.0 },
                if touched[k] > 0 { loaded[k] / touched[k] as f32 } else { 0.0 },
                if touched[k] > 0 { geared[k] / touched[k] as f32 } else { 0.0 },
                if touched[k] > 0 { 100.0 * neutral[k] as f32 / touched[k] as f32 } else { 0.0 },
                if touched[k] > 0 { revved[k] / touched[k] as f32 } else { 0.0 },
                if touched[k] > 0 { torqued[k] / touched[k] as f32 } else { 0.0 },
                if touched[k] > 0 { spun[k] / touched[k] as f32 } else { 0.0 },
                if still[k] > 0 { asked[k].0 / still[k] as f32 } else { 0.0 },
                if still[k] > 0 { asked[k].1 / still[k] as f32 } else { 0.0 },
                if still[k] > 0 { 100.0 * silent[k] as f32 / still[k] as f32 } else { 0.0 },
                back_ticks[k] as f32 * FIXED_DT,
                back_move[k],
                field.get(k).map_or(0, |(_, p)| p.escapes().0),
                field.get(k).map_or(0.0, |(_, p)| p.escapes().1),
                field.get(k).map_or(0, |(_, p)| p.swaps().0),
                field.get(k).map_or(0, |(_, p)| p.swaps().1),
                field.get(k).map_or(0, |(_, p)| p.swap_choice().0),
                field.get(k).map_or(0, |(_, p)| p.swap_choice().1),
                field.get(k).map_or(0, |(_, p)| p.swap_choice().2),
                moved_at[k]
            );
        }
    }
    // What being on its side costs, in the column it corrupts. A rolled car cannot move and its
    // pilot does not know that, so it goes on walking the graph — which lands in `junctions`, a
    // column quoted in every sweep in `ROADMAP.md`. Waypoints driven past cannot be inflated this
    // way, which is exactly why that is the column decisions are made on.
    let ever_rolled: Vec<usize> = (0..field.len()).filter(|k| rolled[*k] > 0).collect();
    if !ever_rolled.is_empty() {
        let junk: usize =
            ever_rolled.iter().filter_map(|k| field.get(*k)).map(|(_, p)| p.passed()).sum();
        println!(
            "\n{} of {} cars spent time on their side; between them they hold {junk} of the \
             {junctions} junctions counted",
            ever_rolled.len(),
            field.len()
        );
    }
    if rescue {
        let total: usize = rescued.iter().sum();
        println!(
            "the field was caught {total} times ({} of {} cars needed it at least once)",
            rescued.iter().filter(|n| **n > 0).count(),
            field.len()
        );
    }
    if aim_lift > 0.0 {
        let seen: usize = aim_seen.iter().sum();
        let walled: usize = aim_walled.iter().sum();
        let each: Vec<String> = aim_seen
            .iter()
            .zip(&aim_walled)
            .map(|(s, w)| {
                if *s > 0 {
                    format!("{:.0}", 100.0 * *w as f32 / *s as f32)
                } else {
                    "-".to_string()
                }
            })
            .collect();
        println!(
            "\naim: {walled} of {seen} sampled steps had something standing between the car and the \
             point it was steering at — {:.1}% overall, per car {}",
            if seen > 0 { 100.0 * walled as f32 / seen as f32 } else { 0.0 },
            each.join(" ")
        );
    }
    println!(
        "SUMMARY held={held} away={away} junctions={junctions} waypoint={best_waypoint} furthest={furthest:.0}          fallen={fallen} edge={edge} through={through} nowhere={nowhere} cars={} seconds={seconds:.0}",
        field.len()
    );
}
