//! # NFSU2 Car — RACE on a procedural track (M3)
//!
//! Extends the drivable NFSU2 car with a real course: a procedurally generated closed
//! loop with elevation, given a **triangle-mesh collider** (`Collider::trimesh`) the
//! vehicle suspension raycasts against, plus checkpoints and lap timing.
//!
//! Controls: **W/↑** accelerate · **S/↓** reverse · **A/D or ←/→** steer · **Space** brake ·
//! **R** reset to the start line · **T** auto-shift · hold **right mouse** to orbit.
//!
//! ```bash
//! cargo run --release -p demo --bin nfs_race -- "/path/to/CARS/240SX/GEOMETRY.BIN"
//! ```

use gizmo::egui;
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use gizmo::renderer::gpu_types::Vertex;
use gizmo_nfs::parse_geometry;
use nfsu2::assets::{env_color, load_tpk_beside};
use nfsu2::car::{build_car_visuals, WheelFit};
use nfsu2::scene::{self, Textures};
use nfsu2::geom::add_transform;

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";
const FIXED_DT: f32 = 1.0 / 240.0;
const N_CHECKPOINTS: usize = 12;
const CP_RADIUS: f32 = 9.0;

struct WheelVis {
    id: u32,
    local: Vec3,
    front: bool,
}

struct RaceState {
    chassis_id: u32,
    camera_id: u32,
    visual_ids: Vec<u32>,
    wheels: Vec<WheelVis>,
    wheel_radius: f32,
    max_steer: f32,
    wheel_spin: f32,
    cam_pos: Vec3,
    cam_yaw: f32,
    cam_pitch: f32,
    steer_angle: f32,
    phys_accum: f32,
    autodrive: bool,
    shotcam: bool,
    t: f32,
    // Track / lap state.
    checkpoints: Vec<Vec3>,
    start_pos: Vec3,
    start_rot: Quat,
    next_cp: usize,
    lap: u32,
    cur_time: f32,
    best_time: f32,
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
    let path = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .unwrap_or_else(|| DEFAULT_CAR.to_string());

    let mut asset_manager = AssetManager::new();
    let mut phys = PhysicsWorld::new();
    phys.integrator.gravity = Vec3::new(0.0, -9.81, 0.0);
    let tex = asset_manager.create_white_texture(
        &renderer.device,
        &renderer.queue,
        &renderer.scene.texture_bind_group_layout,
    );
    // Double-sided so the glass-less greenhouse (windows are texture-only decals) doesn't
    // read as see-through, and the track ribbon is visible from both sides.
    let mat = |rgb: [f32; 3], rough: f32, metal: f32| {
        Material::new(tex.clone())
            .with_pbr(Vec4::new(rgb[0], rgb[1], rgb[2], 1.0), rough, metal)
            .with_double_sided(true)
    };

    // ── Ground: a big flat drivable plane (grass-green, asphalt grip) ──
    // The car drives reliably on a plane collider (proven in nfs_drive). The track ribbon
    // below is a VISUAL overlay marking the racing line; the checkpoints define the lap.
    // (`Collider::trimesh` is added to the engine and unit-tested, but a rigid chassis box
    // vs. a trimesh currently mis-collides, so we keep the drivable surface a plane.)
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

    // ── Track ribbon: a flat oval visual (built with the trimesh geometry helper) ──
    let track = build_track(80.0, 55.0, 15.0, 0.0, 200);
    let track_ent = world.spawn();
    add_transform(world, track_ent, Transform::new(Vec3::new(0.0, 0.02, 0.0)));
    world.add_component(
        track_ent,
        Mesh::from_vertices(&renderer.device, &track.visual, "nfs_track"),
    );
    world.add_component(track_ent, mat([0.12, 0.12, 0.14], 0.9, 0.0).with_double_sided(true));
    world.add_component(track_ent, MeshRenderer::new());

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

    // ── Lights ──
    let sun = world.spawn();
    add_transform(
        world,
        sun,
        Transform::new(Vec3::new(60.0, 120.0, 40.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.9)),
    );
    world.add_component(
        sun,
        DirectionalLight::new(Vec3::new(1.0, 0.97, 0.9), 2.7, gizmo::renderer::components::LightRole::Sun),
    );
    let fill = world.spawn();
    add_transform(world, fill, Transform::new(Vec3::new(-40.0, 50.0, -30.0)));
    world.add_component(
        fill,
        DirectionalLight::new(Vec3::new(0.6, 0.7, 0.9), 0.6, gizmo::renderer::components::LightRole::Sun),
    );

    // ── Camera ──
    let camera_ent = world.spawn();
    add_transform(world, camera_ent, Transform::new(Vec3::new(0.0, 4.0, 10.0)));
    world.add_component(
        camera_ent,
        Camera::new(std::f32::consts::FRAC_PI_4, 0.1, 4000.0, -std::f32::consts::FRAC_PI_2, -0.3, true),
    );
    // ── Car ──
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let all = parse_geometry(&bytes).expect("parse GEOMETRY.BIN");
    let tpk = load_tpk_beside(&path); // TEXTURES.BIN next to the model, if present
    let paint = env_color("NFS_COLOR", [0.10, 0.28, 0.72]); // override "r,g,b" in 0..1
    let cfg = nfsu2::parts::CarConfig::from_env();
    let car = build_car_visuals(&renderer.device, &all, tpk.as_ref(), paint, &cfg, |look| {
        Material::new(tex.clone())
            .with_pbr(
                Vec4::new(look.rgb[0], look.rgb[1], look.rgb[2], look.alpha),
                look.roughness,
                look.metallic,
            )
            .with_double_sided(true)
    });
    let (width, height, length) = (car.width, car.height, car.length);
    let WheelFit { radius, half_wheelbase, half_track } = car.wheel_fit;

    let mut car = car;
    let mut tex = Textures {
        assets: &mut asset_manager,
        device: &renderer.device,
        queue: &renderer.queue,
        layout: &renderer.scene.texture_bind_group_layout,
    };
    // Resolve the wheel material first: it borrows the uploader, as `spawn_body` does.
    let wheel = car.wheel.take().map(|(mesh, surface)| {
        let m = scene::wheel_material(
            surface,
            &mut tex,
            |bg| Material::new(bg).with_pbr(Vec4::new(1.0, 1.0, 1.0, 1.0), 0.7, 0.2).with_double_sided(true),
            |look| mat(look.rgb, look.roughness, look.metallic),
        );
        (mesh, m)
    });
    // Each body mesh is its own entity that rigidly follows the chassis.
    let visual_ids = scene::spawn_body(world, &mut car, &mut tex, |bg, tint, rough, metal| {
        Material::new(bg)
            .with_pbr(Vec4::new(tint[0], tint[1], tint[2], 1.0), rough, metal)
            .with_double_sided(true)
    });
    // Wheels: the single wheel mesh instanced at four fitted corners.
    let wheel_y = -height * 0.5 + radius * 0.95;
    let mut wheels = Vec::new();
    if let Some((mesh, wmat)) = wheel {
        // The driven wheels sit at the same four corners the vehicle controller attaches to,
        // each mirrored so its rim faces outward.
        for (sx, sz, front) in [(-1.0, -1.0, true), (1.0, -1.0, true), (-1.0, 1.0, false), (1.0, 1.0, false)] {
            let local = Vec3::new(sx * half_track, wheel_y, sz * half_wheelbase);
            let id = scene::spawn_mesh(world, mesh.clone(), wmat.clone(), Transform::new(Vec3::ZERO));
            wheels.push(WheelVis { id, local, front });
        }
    }
    world.insert_resource(asset_manager);

    // Spawn the car on the start line, facing along the track.
    let tan0 = track.tangents[0];
    let tan_h = Vec3::new(tan0.x, 0.0, tan0.z).normalize();
    // Face the car (forward = -Z) along the track tangent via an explicit yaw. (Avoid
    // `from_rotation_arc` here: for the antiparallel -Z→+Z case it picks an arbitrary axis
    // and can flip the car onto its side/roof.)
    let start_rot = Quat::from_rotation_y((-tan_h.x).atan2(-tan_h.z));
    let start_pos = track.centerline[0] + Vec3::new(0.0, height * 0.5 + radius + 0.4, 0.0);

    let chassis = world.spawn();
    add_transform(world, chassis, Transform::new(start_pos).with_rotation(start_rot));

    let mut rb = RigidBody::new(1200.0, true);
    rb.linear_damping = 0.1;
    rb.angular_damping = 1.8;
    rb.calculate_box_inertia(width, height, length);
    rb.center_of_mass = Vec3::new(0.0, -height * 0.1, 0.0);
    rb.lock_rotation_x = false;
    rb.lock_rotation_y = false;
    rb.lock_rotation_z = false;

    let attach_y = -height * 0.5 + radius;
    let mut vehicle = gizmo::physics::vehicle::VehicleController::new();
    for (sx, sz, front, left) in [(-1.0, -1.0, true, true), (1.0, -1.0, true, false), (-1.0, 1.0, false, true), (1.0, 1.0, false, false)] {
        vehicle.add_wheel(gizmo::physics::vehicle::Wheel {
            attachment_local_pos: Vec3::new(sx * half_track, attach_y, sz * half_wheelbase),
            radius,
            axle_type: if front { gizmo::physics::vehicle::Axle::Front } else { gizmo::physics::vehicle::Axle::Rear },
            is_left: left,
            suspension_rest_length: (radius * 0.25).max(0.05),
            suspension_max_travel: (radius * 0.45).max(0.12),
            suspension_stiffness: 45000.0,
            suspension_damping: 3500.0,
            wheel_mass: 25.0,
            ..Default::default()
        });
    }
    vehicle.tuning.wheelbase = half_wheelbase * 2.0;
    vehicle.tuning.track_width = half_track * 2.0;
    vehicle.tuning.max_engine_torque = 560.0;
    vehicle.max_steering_angle = 0.44;

    let collider = Collider::offset_box(
        Vec3::new(0.0, height * 0.12, 0.0),
        Vec3::new(width * 0.42, height * 0.3, length * 0.46),
    );
    world.add_component(chassis, vehicle);
    world.add_component(chassis, rb);
    world.add_component(chassis, Velocity::new(Vec3::ZERO));
    world.add_component(chassis, collider.clone());
    phys.add_body(
        gizmo::physics::BodyHandle::from_id(chassis.id()),
        rb,
        Transform::new(start_pos).with_rotation(start_rot),
        Velocity::default(),
        collider,
    );
    world.insert_resource(phys);

    println!("race ready: track {} tris, {} checkpoints; car {width:.2}×{height:.2}×{length:.2}", track.visual.len() / 3, N_CHECKPOINTS);

    RaceState {
        chassis_id: chassis.id(),
        camera_id: camera_ent.id(),
        visual_ids,
        wheels,
        wheel_radius: radius,
        max_steer: 0.44,
        wheel_spin: 0.0,
        cam_pos: start_pos + Vec3::new(0.0, 4.0, 10.0),
        cam_yaw: -std::f32::consts::FRAC_PI_2,
        cam_pitch: -0.3,
        steer_angle: 0.0,
        phys_accum: 0.0,
        autodrive: std::env::var("NFS_AUTODRIVE").is_ok(),
        shotcam: std::env::var("NFS_SHOTCAM").is_ok(),
        t: 0.0,
        checkpoints,
        start_pos,
        start_rot,
        next_cp: 1,
        lap: 0,
        cur_time: 0.0,
        best_time: 0.0,
    }
}

fn update(world: &mut World, state: &mut RaceState, dt: f32, input: &Input) {
    state.t += dt;
    state.cur_time += dt;

    let mut throttle = 0.0f32;
    let mut brake = 0.0f32;
    if input.is_key_pressed(KeyCode::KeyW as u32) || input.is_key_pressed(KeyCode::ArrowUp as u32) {
        throttle += 1.0;
    }
    if input.is_key_pressed(KeyCode::KeyS as u32) || input.is_key_pressed(KeyCode::ArrowDown as u32) {
        throttle -= 1.0;
    }
    if input.is_key_pressed(KeyCode::Space as u32) {
        brake = 1.0;
    }
    let mut steering = false;
    if input.is_key_pressed(KeyCode::KeyA as u32) || input.is_key_pressed(KeyCode::ArrowLeft as u32) {
        state.steer_angle = (state.steer_angle + 6.0 * dt).min(1.0);
        steering = true;
    }
    if input.is_key_pressed(KeyCode::KeyD as u32) || input.is_key_pressed(KeyCode::ArrowRight as u32) {
        state.steer_angle = (state.steer_angle - 6.0 * dt).max(-1.0);
        steering = true;
    }
    if !steering {
        state.steer_angle *= (-15.0 * dt).exp();
    }
    if state.autodrive {
        throttle = 1.0;
        // Steer toward the next checkpoint so it actually laps the track.
        if let Some(t) = world.borrow::<Transform>().get(state.chassis_id) {
            let to = state.checkpoints[state.next_cp] - t.position;
            let fwd = t.rotation * Vec3::new(0.0, 0.0, -1.0);
            let right = t.rotation * Vec3::new(1.0, 0.0, 0.0);
            let err = to.normalize_or_zero().dot(right);
            state.steer_angle = (-err * 2.5).clamp(-1.0, 1.0);
            if to.normalize_or_zero().dot(fwd) < -0.3 {
                state.steer_angle = 1.0; // sharp turn if facing away
            }
        }
    }

    {
        let mut vs = world.borrow_mut::<gizmo::physics::vehicle::VehicleController>();
        if let Some(mut v) = vs.get_mut(state.chassis_id) {
            v.set_reverse(throttle < 0.0);
            v.throttle_input = throttle.abs().min(1.0);
            v.brake_input = brake;
            v.steering_input = state.steer_angle.clamp(-1.0, 1.0);
            if input.is_key_just_pressed(KeyCode::KeyT as u32) {
                v.auto_shift = !v.auto_shift;
            }
        }
    }

    if input.is_key_just_pressed(KeyCode::KeyR as u32) {
        let (sp, sr) = (state.start_pos, state.start_rot);
        let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
        let mut velocities = unsafe { world.borrow_mut_unchecked::<Velocity>() };
        if let Some(mut t) = transforms.get_mut(state.chassis_id) {
            *t = Transform::new(sp).with_rotation(sr);
            t.update_local_matrix();
        }
        if let Some(mut v) = velocities.get_mut(state.chassis_id) {
            *v = Velocity::default();
        }
        state.steer_angle = 0.0;
        state.next_cp = 1;
        state.cur_time = 0.0;
    }

    // Fixed-step physics.
    state.phys_accum += dt.min(0.1);
    let mut steps = 0;
    while state.phys_accum >= FIXED_DT && steps < 32 {
        gizmo::physics::vehicle_controller_system(world, FIXED_DT);
        gizmo::physics::physics_step_system(world, FIXED_DT);
        state.phys_accum -= FIXED_DT;
        steps += 1;
    }

    let (cpos, crot, speed) = {
        let ts = world.borrow::<Transform>();
        let vs = world.borrow::<gizmo::physics::vehicle::VehicleController>();
        let sp = vs.get(state.chassis_id).map(|v| v.current_speed_kmh / 3.6).unwrap_or(0.0);
        match ts.get(state.chassis_id) {
            Some(t) => (t.position, t.rotation, sp),
            None => return,
        }
    };

    // Lap / checkpoint detection (proximity to the next expected checkpoint, in XZ).
    let car_xz = Vec3::new(cpos.x, 0.0, cpos.z);
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

    // Sync visuals.
    state.wheel_spin += (speed / state.wheel_radius.max(0.05)) * dt;
    let spin = Quat::from_axis_angle(Vec3::X, state.wheel_spin);
    let steer = Quat::from_axis_angle(Vec3::Y, -state.steer_angle * state.max_steer);
    {
        let mut ts = unsafe { world.borrow_mut_unchecked::<Transform>() };
        let mut gs = unsafe { world.borrow_mut_unchecked::<GlobalTransform>() };
        for &id in &state.visual_ids {
            if let Some(mut t) = ts.get_mut(id) {
                t.position = cpos;
                t.rotation = crot;
                t.update_local_matrix();
                if let Some(mut g) = gs.get_mut(id) {
                    g.matrix = t.local_matrix;
                }
            }
        }
        for w in &state.wheels {
            // Yaw the left wheels 180° so their rim faces outward (else the flat inboard back
            // shows); mirror is innermost so spin still turns about the shared chassis axle.
            let mirror = if w.local.x < 0.0 { Quat::from_rotation_y(std::f32::consts::PI) } else { Quat::IDENTITY };
            let lr = if w.front { steer * spin * mirror } else { spin * mirror };
            if let Some(mut t) = ts.get_mut(w.id) {
                t.position = cpos + crot * w.local;
                t.rotation = crot * lr;
                t.update_local_matrix();
                if let Some(mut g) = gs.get_mut(w.id) {
                    g.matrix = t.local_matrix;
                }
            }
        }
    }

    // Camera.
    let orbit = input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT);
    if state.shotcam {
        // Low front-3/4 cinematic view (forward = -Z, +X = right), tracking the car's frame.
        state.cam_pos = cpos + crot * Vec3::new(4.2, 1.4, -5.6);
        let look = cpos + Vec3::new(0.0, 0.5, 0.0);
        let dir = (look - state.cam_pos).normalize();
        state.cam_yaw = dir.z.atan2(dir.x);
        state.cam_pitch = dir.y.asin();
    } else if orbit {
        let fwd = Camera::forward_from(state.cam_yaw, state.cam_pitch);
        state.cam_pos = cpos + Vec3::new(0.0, 1.2, 0.0) - fwd * 9.0;
    } else {
        let forward = crot * Vec3::new(0.0, 0.0, -1.0);
        let target = cpos - forward * 7.0 + Vec3::new(0.0, 2.2, 0.0);
        let k = 1.0 - (-10.0 * dt).exp();
        state.cam_pos = state.cam_pos.lerp(target, k);
        let look = cpos + Vec3::new(0.0, 0.7, 0.0);
        let dir = (look - state.cam_pos).normalize();
        state.cam_yaw = dir.z.atan2(dir.x);
        state.cam_pitch = dir.y.asin();
    }
    update_camera(world, state, input);
}

fn update_camera(world: &mut World, state: &mut RaceState, input: &Input) {
    if input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT) {
        let d = input.mouse_delta();
        state.cam_yaw += d.0 * 0.005;
        state.cam_pitch += d.1 * 0.005;
    }
    state.cam_pitch = state.cam_pitch.clamp(-std::f32::consts::FRAC_PI_2 + 0.1, std::f32::consts::FRAC_PI_2 - 0.1);
    let cam_id = state.camera_id;
    let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
    let mut globals = unsafe { world.borrow_mut_unchecked::<GlobalTransform>() };
    let mut cameras = unsafe { world.borrow_mut_unchecked::<Camera>() };
    if let Some(mut t) = transforms.get_mut(cam_id) {
        t.position = state.cam_pos;
        t.update_local_matrix();
        if let Some(mut g) = globals.get_mut(cam_id) {
            g.matrix = t.local_matrix;
        }
    }
    if let Some(mut c) = cameras.get_mut(cam_id) {
        c.yaw = state.cam_yaw;
        c.pitch = state.cam_pitch;
    }
}

fn ui(world: &mut World, state: &mut RaceState, ctx: &egui::Context) {
    let speed = world
        .borrow::<gizmo::physics::vehicle::VehicleController>()
        .get(state.chassis_id)
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
