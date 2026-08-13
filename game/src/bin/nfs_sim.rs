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
use nfsu2::rig::{spawn_car, CarRig, Pilot, Placement, FIXED_DT};
use nfsu2::scene;
use nfsu2::world as city;

/// How far apart the driven waypoints are after the outline is subdivided.
///
/// Short enough that a grid is never far from one and long enough that a pilot is not chasing a
/// point under its own bumper.
const WAYPOINT_STEP: f32 = 40.0;

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

    let net = city::Network::of(&nodes, &ground);
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
    let waypoints = city::densify(&coarse, step);

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
        for (rig, pilot) in &mut field {
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
            if let Some(c) =
                pilot.drive(pose.position, pose.rotation, pose.speed, &net, &waypoints, &traffic)
            {
                rig.drive(&mut world, &c);
            }
        }
        gizmo::physics::vehicle_controller_system(&world, FIXED_DT);
        // The fence, between the forces and the integration, so the step that would have carried a
        // car over the lip is the step that does not. NFS_FENCE=0 turns it off, which is the only
        // way to say what it is worth.
        if fence {
            for (rig, _) in &mut field {
                let Some(p) = rig.pose(&world) else { continue };
                held += usize::from(rig.hold_at_edge(&mut world, p, &ground));
            }
        }
        gizmo::physics::physics_step_system(&world, FIXED_DT);

        // Where the world last held each car up — the same question `keep_in_world` asks in the
        // game, through the same code, with the rescue left out. Watching a field leave the world
        // and catching it are different jobs, and only the first one answers why.
        for (k, (rig, pilot)) in field.iter_mut().enumerate() {
            let Some(p) = rig.pose(&world) else { continue };
            let round = pilot.along(waypoints.len());
            if round > along[k] {
                along[k] = round;
                moved_at[k] = now;
            }
            if let Some(j) = pilot.node().and_then(|i| net.node(i)) {
                let d = Vec3::new(j.at.x - p.position.x, 0.0, j.at.z - p.position.z).length();
                strayed[k] = strayed[k].max(d);
            }
            let g = rig.watch_ground(&world, p, FIXED_DT);
            let f = &mut falls[k];
            f.below = g.below;
            f.ever = g.ever;
            let off = corridor.locate(p.position).map_or(f32::INFINITY, |x| x.distance);
            if off > city::COURSE_HALF_WIDTH {
                f.off_for += FIXED_DT;
            } else {
                f.off_for = 0.0;
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
    println!(
        "SUMMARY held={held} away={away} junctions={junctions} waypoint={best_waypoint} furthest={furthest:.0}          fallen={fallen} edge={edge} through={through} nowhere={nowhere} cars={} seconds={seconds:.0}",
        field.len()
    );
}
