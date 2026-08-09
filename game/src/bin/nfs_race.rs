//! # NFSU2 Car — RACE on a procedural track (M3)
//!
//! The same car [`nfsu2::rig`] builds for `nfs_drive`, given a real course: a procedurally
//! generated closed loop with elevation, carrying a **triangle-mesh collider**
//! (`Collider::trimesh`) the vehicle suspension raycasts against, plus checkpoints and lap timing.
//! The track is what this binary is; the car is the shared rig.
//!
//! Controls: **W/↑** accelerate · **S/↓** reverse · **A/D or ←/→** steer · **Space** brake ·
//! **R** reset to the start line · **T** auto-shift · hold **right mouse** to orbit.
//!
//! ```bash
//! cargo run --release -p nfsu2 --bin nfs_race -- "/path/to/CARS/240SX/GEOMETRY.BIN"
//! ```

use gizmo::egui;
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use gizmo::renderer::gpu_types::Vertex;
use nfsu2::geom::add_transform;
use nfsu2::rig::{spawn_car, CarRig, ChaseCamera, Driver, Placement, Rescue};
use nfsu2::scene;

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";
const N_CHECKPOINTS: usize = 12;
const CP_RADIUS: f32 = 9.0;

struct RaceState {
    rig: CarRig,
    driver: Driver,
    camera: ChaseCamera,
    autodrive: bool,
    // Track / lap state.
    checkpoints: Vec<Vec3>,
    next_cp: usize,
    lap: u32,
    cur_time: f32,
    best_time: f32,
    /// `NFS_DIAG` throttle: the last whole second a line was printed for.
    diag_tick: i32,
}

/// A closed oval track ribbon (visual geometry + centerline for checkpoints).
struct Track {
    visual: Vec<Vertex>,
    centerline: Vec<Vec3>,
    tangents: Vec<Vec3>,
}

fn build_track(a: f32, b: f32, width: f32, hill: f32, n: usize) -> Track {
    use std::f32::consts::TAU;
    let mut centerline = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / n as f32 * TAU;
        let y = hill * 0.5 * (1.0 - (2.0 * t).cos()); // 0 when flat
        centerline.push(Vec3::new(a * t.cos(), y, b * t.sin()));
    }
    let mut tangents = Vec::with_capacity(n);
    for i in 0..n {
        let prev = centerline[(i + n - 1) % n];
        let next = centerline[(i + 1) % n];
        tangents.push((next - prev).normalize());
    }

    // Ribbon edge vertices, then two triangles per segment.
    let mut edges = Vec::with_capacity(n * 2);
    for i in 0..n {
        let normal = Vec3::Y.cross(tangents[i]).normalize();
        edges.push(centerline[i] + normal * (width * 0.5));
        edges.push(centerline[i] - normal * (width * 0.5));
    }
    let mut visual = Vec::with_capacity(n * 6);
    for i in 0..n {
        let j = (i + 1) % n;
        let quad = [edges[2 * i], edges[2 * i + 1], edges[2 * j + 1], edges[2 * i], edges[2 * j + 1], edges[2 * j]];
        for tri in quad.chunks_exact(3) {
            let mut nrm = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
            if nrm.y < 0.0 {
                nrm = -nrm;
            }
            for p in tri {
                visual.push(Vertex {
                    position: [p.x, p.y, p.z],
                    normal: [nrm.x, nrm.y, nrm.z],
                    ..Default::default()
                });
            }
        }
    }

    Track { visual, centerline, tangents }
}

fn main() {
    gizmo::app::setup_panic_hook();
    App::<RaceState>::new("Gizmo — NFSU2 240SX Race", 1500, 850)
        .add_plugin(gizmo::plugins::TransformPlugin)
        .set_setup(setup_scene)
        .set_update(update)
        .set_ui(ui)
        .set_render(|world, _s, encoder, view, renderer, _t| {
            renderer.gpu_fluid = None;
            renderer.gpu_particles = None;
            renderer.ssr = None;
            renderer.ssgi = None;
            renderer.volumetric = None;
            renderer.taa = None;
            gizmo::systems::default_render_pass(world, encoder, view, renderer);
        })
        .run()
        .expect("failed to run app");
}

fn setup_scene(world: &mut World, renderer: &gizmo::renderer::Renderer) -> RaceState {
    let path = scene::car_path(DEFAULT_CAR);

    let mut assets = AssetManager::new();
    let mut phys = PhysicsWorld::new();
    phys.integrator.gravity = Vec3::new(0.0, -9.81, 0.0);
    let white = assets.create_white_texture(
        &renderer.device,
        &renderer.queue,
        &renderer.scene.texture_bind_group_layout,
    );
    // Double-sided so the track ribbon is visible from both sides.
    let mat = |rgb: [f32; 3], rough: f32, metal: f32| {
        Material::new(white.clone())
            .with_pbr(Vec4::new(rgb[0], rgb[1], rgb[2], 1.0), rough, metal)
            .with_double_sided(true)
    };

    // ── Ground: the grass around the circuit ──
    // A plane, and only the *surroundings*: the track itself is a real triangle-mesh collider below.
    // It stays because a car that leaves the ribbon should land on grass rather than fall out of the
    // world.
    let ground = world.spawn();
    add_transform(world, ground, Transform::new(Vec3::ZERO));
    world.add_component(ground, AssetManager::create_plane(&renderer.device, 600.0));
    world.add_component(ground, mat([0.10, 0.22, 0.09], 0.95, 0.0));
    world.add_component(ground, MeshRenderer::new());
    world.add_component(ground, RigidBody::new_static());
    world.add_component(ground, Velocity::default());
    world.add_component(ground, Collider::plane(Vec3::Y, 0.0));
    world.add_component(ground, gizmo::physics::components::PhysicsMaterial::ASPHALT);
    phys.add_body(
        gizmo::physics::BodyHandle::from_id(ground.id()),
        RigidBody::new_static(),
        Transform::new(Vec3::ZERO),
        Velocity::default(),
        Collider::plane(Vec3::Y, 0.0),
    );

    // ── Track ribbon: a banked oval that is actually **driven on** ──
    //
    // This used to be a flat visual overlay marking the racing line, with the note that a rigid
    // chassis box against a trimesh mis-collided. It did, and the reason was not the box: a
    // `TriMesh` pair fell through to GJK+EPA, which is a *convex* algorithm, so the engine was
    // colliding against the mesh's convex hull — and the hull of a closed oval is a filled disc.
    // A car inside the ring was inside the hull. `NarrowPhase::shape_trimesh` now dispatches
    // per-triangle, so the ribbon can carry the surface, and `hill` can stop being 0.
    let hill = std::env::var("NFS_HILL").ok().and_then(|s| s.parse().ok()).unwrap_or(3.0_f32);
    let track = build_track(80.0, 55.0, 15.0, hill, 200);
    let track_at = Vec3::new(0.0, 0.02, 0.0);
    let track_ent = world.spawn();
    add_transform(world, track_ent, Transform::new(track_at));
    world.add_component(
        track_ent,
        Mesh::from_vertices(&renderer.device, &track.visual, "nfs_track"),
    );
    world.add_component(track_ent, mat([0.12, 0.12, 0.14], 0.9, 0.0));
    world.add_component(track_ent, MeshRenderer::new());
    // The same triangles the eye sees, handed to physics. `build_track` emits an unindexed
    // triangle soup, so the index list is just its own order — no welding, because two ribbon
    // segments meeting at a shared edge are still two triangles to a per-triangle narrowphase.
    let tri_verts: Vec<Vec3> =
        track.visual.iter().map(|v| Vec3::new(v.position[0], v.position[1], v.position[2])).collect();
    let tri_indices: Vec<u32> = (0..tri_verts.len() as u32).collect();
    let track_collider = Collider::trimesh(tri_verts, tri_indices);
    world.add_component(track_ent, RigidBody::new_static());
    world.add_component(track_ent, Velocity::default());
    world.add_component(track_ent, track_collider.clone());
    world.add_component(track_ent, gizmo::physics::components::PhysicsMaterial::ASPHALT);
    phys.add_body(
        gizmo::physics::BodyHandle::from_id(track_ent.id()),
        RigidBody::new_static(),
        Transform::new(track_at),
        Velocity::default(),
        track_collider,
    );

    // Checkpoints along the centerline.
    let n = track.centerline.len();
    let checkpoints: Vec<Vec3> = (0..N_CHECKPOINTS)
        .map(|k| track.centerline[k * n / N_CHECKPOINTS])
        .collect();
    // Start line marker (a thin bright slab across checkpoint 0).
    {
        let start_c = track.centerline[0];
        let start = world.spawn();
        add_transform(
            world,
            start,
            Transform::new(start_c + Vec3::new(0.0, 0.06, 0.0)).with_scale(Vec3::new(7.5, 0.05, 0.5)),
        );
        world.add_component(start, AssetManager::create_cube(&renderer.device));
        world.add_component(start, mat([0.95, 0.95, 0.98], 0.5, 0.0));
        world.add_component(start, MeshRenderer::new());
    }

    scene::add_lights(
        world,
        Transform::new(Vec3::new(60.0, 120.0, 40.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.9)),
        2.7,
        Vec3::new(-40.0, 50.0, -30.0),
    );

    // ── Car: on the start line, facing along the track ──
    //
    // Dropped from further up than on flat ground: the ribbon is banked and hilly, so its surface
    // under the start line is not at the centerline's own height.
    let rig = spawn_car(
        world,
        renderer,
        &mut assets,
        &mut phys,
        &path,
        Placement::facing(track.centerline[0], track.tangents[0], 0.4),
    );
    world.insert_resource(assets);
    world.insert_resource(phys);

    let camera =
        ChaseCamera::spawn(world, rig.start.position + Vec3::new(0.0, 4.0, 10.0), 0.1, 4000.0)
            .trailing(7.0, 2.2, 10.0);

    println!(
        "race ready: track {} tris, {} checkpoints",
        track.visual.len() / 3,
        N_CHECKPOINTS
    );

    RaceState {
        rig,
        driver: Driver::new(),
        camera,
        autodrive: std::env::var("NFS_AUTODRIVE").is_ok(),
        checkpoints,
        next_cp: 1,
        lap: 0,
        cur_time: 0.0,
        best_time: 0.0,
        diag_tick: -1,
    }
}

fn update(world: &mut World, state: &mut RaceState, dt: f32, input: &Input) {
    state.cur_time += dt;

    let mut controls = state.driver.read(input, dt);
    if state.autodrive {
        controls.throttle = 1.0;
        // Steer toward the next checkpoint so it actually laps the track.
        if let Some(t) = world.borrow::<Transform>().get(state.rig.chassis) {
            let to = (state.checkpoints[state.next_cp] - t.position).normalize_or_zero();
            let fwd = t.rotation * Vec3::NEG_Z;
            let right = t.rotation * Vec3::X;
            state.driver.steer = if to.dot(fwd) < -0.3 {
                1.0 // sharp turn if facing away
            } else {
                (-to.dot(right) * 2.5).clamp(-1.0, 1.0)
            };
        }
        controls.steer = state.driver.steer;
    }
    state.rig.drive(world, &controls);

    if input.is_key_just_pressed(KeyCode::KeyR as u32) {
        state.rig.reset(world);
        state.driver.reset();
        state.next_cp = 1;
        state.cur_time = 0.0;
    }

    state.driver.step_physics(world, dt);

    let Some(pose) = state.rig.pose(world) else { return };

    // The same net the city binary has, for the same reason: `rig` is where "a car cannot fall out
    // of the world" belongs, not one binary. It is rarely reached here — the ground is
    // `Collider::plane`, which is a true plane in the narrowphase — but not never: its broadphase
    // AABB is a ±10 000 m cube about the origin, so past 10 km there is nothing left to pair with.
    match state.rig.keep_in_world(world, pose, dt) {
        Rescue::None => {}
        Rescue::ToLastGround => {
            state.driver.reset();
            println!("out of the world at {:?} — back on the last ground", pose.position);
            return;
        }
        // The rig has already said why, once.
        Rescue::NowhereSafe => {
            state.driver.reset();
            return;
        }
    }

    // Lap / checkpoint detection (proximity to the next expected checkpoint, in XZ).
    let car_xz = Vec3::new(pose.position.x, 0.0, pose.position.z);
    let cp = state.checkpoints[state.next_cp];
    if car_xz.distance(Vec3::new(cp.x, 0.0, cp.z)) < CP_RADIUS {
        if state.next_cp == 0 {
            // Reached the start line after all checkpoints → a lap is complete.
            state.lap += 1;
            if state.best_time == 0.0 || state.cur_time < state.best_time {
                state.best_time = state.cur_time;
            }
            state.cur_time = 0.0;
            state.next_cp = 1;
        } else {
            state.next_cp = (state.next_cp + 1) % N_CHECKPOINTS;
        }
    }

    diagnose(world, state, pose);

    state.rig.sync_visuals(world, pose, dt, controls.steer);
    state.camera.update(world, input, pose, dt);
}

/// `NFS_DIAG=1`: once a second, what the physics actually thinks is happening. Three vague
/// symptoms ("doesn't move", "W goes backwards", "a square") become numbers.
fn diagnose(world: &World, state: &mut RaceState, pose: nfsu2::rig::Pose) {
    if std::env::var("NFS_DIAG").is_err() {
        return;
    }
    let tick = state.cur_time as i32;
    if tick == state.diag_tick {
        return;
    }
    state.diag_tick = tick;
    let vehicles = world.borrow::<gizmo::physics::vehicle::VehicleController>();
    let Some(v) = vehicles.get(state.rig.chassis) else { return };
    let grounded: Vec<bool> = v.wheels.iter().map(|w| w.is_grounded).collect();
    let drive: Vec<f32> = v.wheels.iter().map(|w| w.drive_torque.round()).collect();
    // The rigid body's *own* velocity, beside the controller's idea of speed. If the two disagree
    // the bug is not in the car, it is in what is being integrated.
    let vel = world.borrow::<Velocity>().get(state.rig.chassis).map_or(Vec3::ZERO, |v| v.linear);
    println!(
        "diag  pos ({:+.2},{:+.2},{:+.2})  vel ({:+.2},{:+.2},{:+.2})  fwd_speed {:+.2}  gear {}  rpm {:.0}  throttle {:.2}  grounded {grounded:?}  drive {drive:?}",
        pose.position.x, pose.position.y, pose.position.z,
        vel.x, vel.y, vel.z,
        pose.speed,
        v.current_gear, v.engine_rpm, v.throttle_input,
    );
}

fn ui(world: &mut World, state: &mut RaceState, ctx: &egui::Context) {
    let speed = world
        .borrow::<gizmo::physics::vehicle::VehicleController>()
        .get(state.rig.chassis)
        .map(|v| v.current_speed_kmh.abs())
        .unwrap_or(0.0);
    egui::Area::new(egui::Id::new("hud"))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(24.0, 24.0))
        .show(ctx, |ui| {
            ui.heading(format!("Tur {}", state.lap));
            ui.label(format!("Süre: {:>5.1} s", state.cur_time));
            if state.best_time > 0.0 {
                ui.label(format!("En iyi: {:>5.1} s", state.best_time));
            }
            ui.label(format!("Checkpoint: {}/{}", state.next_cp, N_CHECKPOINTS));
        });
    egui::Area::new(egui::Id::new("spd"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-30.0, -30.0))
        .show(ctx, |ui| {
            ui.heading(format!("{speed:.0} km/h"));
            ui.label("W/S · A/D · Space · R");
        });
}
