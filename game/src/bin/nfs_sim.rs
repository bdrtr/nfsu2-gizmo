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
    let waypoints: Vec<Vec3> = ev
        .map(|e| e.outline.iter().map(|p| city::remap([p[0], p[1], 0.0])).collect())
        .unwrap_or_default();

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
    let mut field: Vec<(CarRig, Pilot)> = Vec::new();
    for slot in slots.iter().take(rivals) {
        let stand = ground
            .height_at(*slot + Vec3::Y * 2.0)
            .map_or(*slot, |y| Vec3::new(slot.x, y, slot.z));
        let rig = spawn_car(
            &mut world,
            &renderer,
            &mut assets,
            &mut phys,
            &car_path,
            Placement::facing(stand, heading, 1.5),
        );
        let mut pilot = Pilot::new();
        pilot.place(stand, &net);
        field.push((rig, pilot));
    }
    world.insert_resource(assets);
    world.insert_resource(phys);
    scene::add_lights(&mut world, Transform::new(slots[0] + Vec3::Y * 60.0), 2.0, slots[0]);
    println!("field: {} cars", field.len());

    // Fixed steps, not frames: a simulation has no reason to pretend it is being watched, and a
    // fixed step is what makes two runs of the same parameters give the same answer.
    let steps = (seconds / FIXED_DT) as usize;
    for _ in 0..steps {
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
    for (k, (rig, pilot)) in field.iter().enumerate() {
        let p = rig.pose(&world);
        let (at, speed) = p.map_or((Vec3::ZERO, 0.0), |p| (p.position, p.speed));
        junctions += pilot.passed();
        best_waypoint = best_waypoint.max(pilot.goal());
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
        "SUMMARY junctions={junctions} waypoint={best_waypoint} cars={} seconds={seconds:.0}",
        field.len()
    );
}
