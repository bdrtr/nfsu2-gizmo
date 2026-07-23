//! # NFSU2 Car — DRIVABLE with visual wheels & per-part colours (M2+)
//!
//! Loads a Need for Speed: Underground 2 car with `gizmo-nfs`, assembles its default
//! (showroom) configuration via [`nfsu2`], splits it into material groups (paint / glass /
//! chrome / head- & brake-lights / exhaust / trim) plus four wheels, and drives it with the
//! engine's canonical `VehicleController`. The visual entities are invisible-chassis
//! followers: each frame they copy the chassis transform (wheels add spin + steer), so
//! per-group materials work despite the deferred shader ignoring vertex colour.
//!
//! Controls: **W/↑** accelerate · **S/↓** reverse · **A/D or ←/→** steer · **Space** brake ·
//! **R** reset · **T** auto-shift · hold **right mouse** to orbit.
//!
//! ```bash
//! cargo run --release -p demo --bin nfs_drive -- "/path/to/CARS/240SX/GEOMETRY.BIN"
//! ```

use gizmo::egui;
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use gizmo_nfs::parse_geometry;
use nfsu2::car::{build_car_visuals, env_color, load_tpk_beside, WheelFit};
use nfsu2::mesh::add_transform;

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";
const FIXED_DT: f32 = 1.0 / 240.0;

struct WheelVis {
    id: u32,
    local: Vec3,
    front: bool,
}

struct DriveState {
    chassis_id: u32,
    camera_id: u32,
    visual_ids: Vec<u32>, // one per material group — all follow the chassis rigidly
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
}

fn main() {
    gizmo::app::setup_panic_hook();
    App::<DriveState>::new("Gizmo — NFSU2 240SX (drivable)", 1500, 850)
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

fn setup_scene(world: &mut World, renderer: &gizmo::renderer::Renderer) -> DriveState {
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
    // Double-sided: the greenhouse has no glass geometry yet (windows are texture-only
    // decals), so single-sided rendering would let the camera see straight through the
    // empty window openings to the dark cabin interior / far body.
    let mat = |rgb: [f32; 3], rough: f32, metal: f32| {
        Material::new(tex.clone())
            .with_pbr(Vec4::new(rgb[0], rgb[1], rgb[2], 1.0), rough, metal)
            .with_double_sided(true)
    };

    // ── Ground ──
    let ground_mesh = AssetManager::create_plane(&renderer.device, 400.0);
    let ground = world.spawn();
    add_transform(world, ground, Transform::new(Vec3::ZERO));
    world.add_component(ground, ground_mesh);
    world.add_component(ground, mat([0.13, 0.14, 0.16], 0.95, 0.0));
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

    // ── Lights ──
    let sun = world.spawn();
    add_transform(
        world,
        sun,
        Transform::new(Vec3::new(30.0, 80.0, 40.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.8)),
    );
    world.add_component(
        sun,
        DirectionalLight::new(Vec3::new(1.0, 0.97, 0.9), 2.6, gizmo::renderer::components::LightRole::Sun),
    );
    let fill = world.spawn();
    add_transform(world, fill, Transform::new(Vec3::new(-30.0, 40.0, -20.0)));
    world.add_component(
        fill,
        DirectionalLight::new(Vec3::new(0.6, 0.7, 0.9), 0.6, gizmo::renderer::components::LightRole::Sun),
    );

    // ── Camera ──
    let camera_ent = world.spawn();
    add_transform(world, camera_ent, Transform::new(Vec3::new(0.0, 4.0, 10.0)));
    world.add_component(
        camera_ent,
        Camera::new(std::f32::consts::FRAC_PI_4, 0.1, 2000.0, -std::f32::consts::FRAC_PI_2, -0.3, true),
    );
    // ── Parse & assemble the default (showroom) car ──
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let all = parse_geometry(&bytes).expect("parse GEOMETRY.BIN");
    let tpk = load_tpk_beside(&path); // TEXTURES.BIN next to the model, if present
    let paint = env_color("NFS_COLOR", [0.10, 0.28, 0.72]); // override "r,g,b" in 0..1
    let car = build_car_visuals(&renderer.device, &all, tpk.as_ref(), paint, |look| {
        mat(look.rgb, look.roughness, look.metallic)
    });
    let (width, height, length) = (car.width, car.height, car.length);
    let WheelFit { radius, half_wheelbase, half_track } = car.wheel_fit;

    // Each material group is its own entity that rigidly follows the chassis.
    let mut visual_ids = Vec::new();
    for gv in car.groups {
        let e = world.spawn();
        add_transform(world, e, Transform::new(Vec3::ZERO));
        world.add_component(e, gv.mesh);
        world.add_component(e, gv.material);
        world.add_component(e, MeshRenderer::new());
        visual_ids.push(e.id());
    }
    // Dark cabin filler so the glass-less windows don't read as see-through.
    {
        let e = world.spawn();
        add_transform(world, e, Transform::new(Vec3::ZERO));
        world.add_component(e, car.interior);
        world.add_component(e, mat([0.02, 0.02, 0.025], 0.9, 0.0));
        world.add_component(e, MeshRenderer::new());
        visual_ids.push(e.id());
    }
    // Textured parts: upload each decoded TPK texture and material-ise it as albedo.
    let mut textured_count = 0;
    for tp in car.textured {
        let key = format!("nfs_tex_{:08X}", tp.texture.hash.0);
        let Ok(bg) = asset_manager.install_decoded_material_texture(
            &renderer.device,
            &renderer.queue,
            &renderer.scene.texture_bind_group_layout,
            &key,
            &tp.texture.rgba,
            tp.texture.width,
            tp.texture.height,
        ) else {
            continue;
        };
        let material = Material::new(bg)
            .with_pbr(Vec4::new(tp.tint[0], tp.tint[1], tp.tint[2], 1.0), tp.roughness, tp.metallic)
            .with_double_sided(true);
        let e = world.spawn();
        add_transform(world, e, Transform::new(Vec3::ZERO));
        world.add_component(e, tp.mesh);
        world.add_component(e, material);
        world.add_component(e, MeshRenderer::new());
        visual_ids.push(e.id());
        textured_count += 1;
    }
    world.insert_resource(asset_manager);

    // ── Wheels: the single wheel mesh instanced at four fitted corners ──
    // Wheel centre near the bottom of the body so the lower half sticks out of the arch.
    let wheel_y = -height * 0.5 + radius * 0.15;
    let mut wheels = Vec::new();
    if let Some((wm, wmat)) = car.wheel {
        for (sx, sz, front) in [(-1.0, -1.0, true), (1.0, -1.0, true), (-1.0, 1.0, false), (1.0, 1.0, false)] {
            let e = world.spawn();
            add_transform(world, e, Transform::new(Vec3::ZERO));
            world.add_component(e, wm.clone());
            world.add_component(e, wmat.clone());
            world.add_component(e, MeshRenderer::new());
            wheels.push(WheelVis {
                id: e.id(),
                local: Vec3::new(sx * half_track, wheel_y, sz * half_wheelbase),
                front,
            });
        }
    }

    // ── Chassis (physics only; the visuals above follow it) ──
    let spawn = Vec3::new(0.0, height * 0.5 + radius + 0.15, 0.0);
    let chassis = world.spawn();
    add_transform(world, chassis, Transform::new(spawn));

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
    vehicle.tuning.max_engine_torque = 520.0;
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
        Transform::new(spawn),
        Velocity::default(),
        collider,
    );
    world.insert_resource(phys);

    println!(
        "car ready: {} visual groups ({textured_count} textured), {} wheels; dims {width:.2}×{height:.2}×{length:.2}, r={radius:.2}",
        visual_ids.len(),
        wheels.len()
    );

    DriveState {
        chassis_id: chassis.id(),
        camera_id: camera_ent.id(),
        visual_ids,
        wheels,
        wheel_radius: radius,
        max_steer: 0.44,
        wheel_spin: 0.0,
        cam_pos: Vec3::new(0.0, 4.0, 10.0),
        cam_yaw: -std::f32::consts::FRAC_PI_2,
        cam_pitch: -0.3,
        steer_angle: 0.0,
        phys_accum: 0.0,
        autodrive: std::env::var("NFS_AUTODRIVE").is_ok(),
        shotcam: std::env::var("NFS_SHOTCAM").is_ok(),
        t: 0.0,
    }
}

fn update(world: &mut World, state: &mut DriveState, dt: f32, input: &Input) {
    state.t += dt;
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
        state.steer_angle = (state.t * 0.6).sin() * 0.5;
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
        let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
        let mut velocities = unsafe { world.borrow_mut_unchecked::<Velocity>() };
        if let Some(mut t) = transforms.get_mut(state.chassis_id) {
            *t = Transform::new(Vec3::new(0.0, 1.5, 0.0));
            t.update_local_matrix();
        }
        if let Some(mut v) = velocities.get_mut(state.chassis_id) {
            *v = Velocity::default();
        }
        state.steer_angle = 0.0;
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

    // Read chassis pose + speed.
    let (cpos, crot, speed) = {
        let ts = world.borrow::<Transform>();
        let vs = world.borrow::<gizmo::physics::vehicle::VehicleController>();
        let sp = vs.get(state.chassis_id).map(|v| v.current_speed_kmh / 3.6).unwrap_or(0.0);
        match ts.get(state.chassis_id) {
            Some(t) => (t.position, t.rotation, sp),
            None => return,
        }
    };

    // Sync visuals to the chassis (body/lights/exhaust rigid; wheels spin + steer).
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
            let lr = if w.front { steer * spin } else { spin };
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

    // Chase camera behind the car (car forward = local -Z).
    let orbit = input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT);
    if state.shotcam {
        // Fixed front-3/4 low view (front is -Z, +X is right) — shows wheels & body side.
        state.cam_pos = cpos + Vec3::new(5.0, 1.25, -3.2);
        let look = cpos + Vec3::new(0.0, 0.35, 0.0);
        let dir = (look - state.cam_pos).normalize();
        state.cam_yaw = dir.z.atan2(dir.x);
        state.cam_pitch = dir.y.asin();
    } else if orbit {
        let fwd = Camera::forward_from(state.cam_yaw, state.cam_pitch);
        state.cam_pos = cpos + Vec3::new(0.0, 1.2, 0.0) - fwd * 9.0;
    } else {
        let forward = crot * Vec3::new(0.0, 0.0, -1.0);
        let target = cpos - forward * 6.5 + Vec3::new(0.0, 1.9, 0.0);
        let k = 1.0 - (-12.0 * dt).exp();
        state.cam_pos = state.cam_pos.lerp(target, k);
        let look = cpos + Vec3::new(0.0, 0.7, 0.0);
        let dir = (look - state.cam_pos).normalize();
        state.cam_yaw = dir.z.atan2(dir.x);
        state.cam_pitch = dir.y.asin();
    }
    update_camera(world, state, input);
}

fn update_camera(world: &mut World, state: &mut DriveState, input: &Input) {
    if input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT) {
        let d = input.mouse_delta();
        state.cam_yaw += d.0 * 0.005;
        state.cam_pitch += d.1 * 0.005;
    }
    state.cam_pitch = state
        .cam_pitch
        .clamp(-std::f32::consts::FRAC_PI_2 + 0.1, std::f32::consts::FRAC_PI_2 - 0.1);
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

fn ui(world: &mut World, state: &mut DriveState, ctx: &egui::Context) {
    let speed = world
        .borrow::<gizmo::physics::vehicle::VehicleController>()
        .get(state.chassis_id)
        .map(|v| v.current_speed_kmh.abs())
        .unwrap_or(0.0);
    egui::Area::new(egui::Id::new("hud"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-30.0, -30.0))
        .show(ctx, |ui| {
            ui.heading(format!("{speed:.0} km/h"));
            ui.label("W/S sür · A/D direksiyon · Space fren · R reset");
        });
}
