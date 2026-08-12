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

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";

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
    let objects: Vec<_> =
        meshes.into_iter().filter(|m| city::is_drawn(&m.header.name)).collect();
    let objects = city::lod::keep_finest(objects);
    let colliders = city::collision_cells(&objects);
    let ground = city::Ground::of(&colliders);

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

    // Fixed steps, not frames: a simulation has no reason to pretend it is being watched, and a
    // fixed step is what makes two runs of the same parameters give the same answer.
    // NFS_TRACE=<n>: print the field every n simulated seconds. A summary says where everyone
    // ended; this says when they stopped, which is a different question and usually the useful one.
    let trace: f32 = std::env::var("NFS_TRACE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let mut next_trace = trace;
    let watch: Option<usize> = std::env::var("NFS_WATCH").ok().and_then(|v| v.parse().ok());
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
                        println!(
                            "    watch {k} t={now:>5.1}s ({:>7.1},{:>6.2},{:>7.1}) {:>5.1} km/h · \
                             up {up:>5.2} · under {under:?} · \
                             node {:?} ({:>7.1},{:>6.2},{:>7.1}) {gap:>5.1} m · {} junctions",
                            q.position.x,
                            q.position.y,
                            q.position.z,
                            q.speed * 3.6,
                            pilot.node(),
                            nx,
                            ny,
                            nz,
                            pilot.passed()
                        );
                    }
                }
            }
        }
        // Controls first for the whole field, then one physics step: every car sees the same
        // world state, which a loop that stepped physics per car would not give.
        for (rig, pilot) in &mut field {
            let Some(pose) = rig.pose(&world) else { continue };
            if let Some(c) =
                pilot.drive(pose.position, pose.rotation, pose.speed, &net, &waypoints)
            {
                rig.drive(&mut world, &c);
            }
        }
        gizmo::physics::vehicle_controller_system(&world, FIXED_DT);
        gizmo::physics::physics_step_system(&world, FIXED_DT);
    }

    println!("\nafter {seconds:.0} s:");
    let mut junctions = 0;
    let mut best_waypoint = 0;
    // How far the field actually got, which is the one measure that survives a change to how the
    // waypoints are counted or how many junctions a route happens to have. Straight-line from the
    // grid: crude, and crude in the same way for every run, which is what a comparison needs.
    let mut furthest = 0.0f32;
    let line = slots[0];
    for (k, (rig, pilot)) in field.iter().enumerate() {
        let p = rig.pose(&world);
        let (at, speed) = p.map_or((Vec3::ZERO, 0.0), |p| (p.position, p.speed));
        junctions += pilot.passed();
        best_waypoint = best_waypoint.max(pilot.goal());
        furthest = furthest.max((Vec3::new(at.x, 0.0, at.z) - Vec3::new(line.x, 0.0, line.z)).length());
        println!(
            "  car {k}: ({:>7.0},{:>7.0}) {:>4.0} km/h · {:>4} junctions · waypoint {}",
            at.x,
            at.z,
            speed * 3.6,
            pilot.passed(),
            pilot.goal()
        );
    }
    println!(
        "SUMMARY junctions={junctions} waypoint={best_waypoint} furthest={furthest:.0}          cars={} seconds={seconds:.0}",
        field.len()
    );
}
