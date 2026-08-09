//! # NFSU2 Car — DRIVABLE with visual wheels & per-part colours (M2+)
//!
//! A Need for Speed: Underground 2 car on a flat plane, driven by the engine's raycast
//! `VehicleController`. Everything about the car itself — parsing it, assembling its default
//! (showroom) configuration, hanging its material groups off a chassis, reading its handling
//! record — is [`nfsu2::rig`]; what is here is the *scene*: a ground plane, two lights, and the
//! HUD.
//!
//! Controls: **W/↑** accelerate · **S/↓** reverse · **A/D or ←/→** steer · **Space** brake ·
//! **R** reset · **T** auto-shift · hold **right mouse** to orbit.
//!
//! ```bash
//! cargo run --release -p nfsu2 --bin nfs_drive -- "/path/to/CARS/240SX/GEOMETRY.BIN"
//! ```

use gizmo::egui;
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use nfsu2::geom::add_transform;
use nfsu2::rig::{spawn_car, CarRig, ChaseCamera, Driver, Placement};
use nfsu2::scene;

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";

struct DriveState {
    rig: CarRig,
    driver: Driver,
    camera: ChaseCamera,
    autodrive: bool,
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
    let path = scene::car_path(DEFAULT_CAR);

    let mut assets = AssetManager::new();
    let mut phys = PhysicsWorld::new();
    phys.integrator.gravity = Vec3::new(0.0, -9.81, 0.0);
    let white = assets.create_white_texture(
        &renderer.device,
        &renderer.queue,
        &renderer.scene.texture_bind_group_layout,
    );

    // ── Ground ──
    let ground = world.spawn();
    add_transform(world, ground, Transform::new(Vec3::ZERO));
    world.add_component(ground, AssetManager::create_plane(&renderer.device, 400.0));
    world.add_component(
        ground,
        Material::new(white).with_pbr(Vec4::new(0.13, 0.14, 0.16, 1.0), 0.95, 0.0).with_double_sided(true),
    );
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

    scene::add_lights(
        world,
        Transform::new(Vec3::new(30.0, 80.0, 40.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.8)),
        2.6,
        Vec3::new(-30.0, 40.0, -20.0),
    );

    let camera = ChaseCamera::spawn(world, Vec3::new(0.0, 4.0, 10.0), 0.1, 2000.0);
    let rig = spawn_car(world, renderer, &mut assets, &mut phys, &path, Placement::origin());
    world.insert_resource(assets);
    world.insert_resource(phys);

    DriveState {
        rig,
        driver: Driver::new(),
        camera,
        autodrive: std::env::var("NFS_AUTODRIVE").is_ok(),
        t: 0.0,
    }
}

fn update(world: &mut World, state: &mut DriveState, dt: f32, input: &Input) {
    state.t += dt;

    let mut controls = state.driver.read(input, dt);
    if state.autodrive {
        // A slow weave with the throttle down: enough to watch the suspension work and the wheels
        // steer without a hand on the keyboard, which is what the screenshot passes want.
        controls.throttle = 1.0;
        state.driver.steer = (state.t * 0.6).sin() * 0.5;
        controls.steer = state.driver.steer;
    }
    state.rig.drive(world, &controls);

    if input.is_key_just_pressed(KeyCode::KeyR as u32) {
        state.rig.reset(world);
        state.driver.reset();
    }

    state.driver.step_physics(world, dt);

    let Some(pose) = state.rig.pose(world) else { return };
    state.rig.sync_visuals(world, pose, dt, controls.steer);
    state.camera.update(world, input, pose, dt);
}

fn ui(world: &mut World, state: &mut DriveState, ctx: &egui::Context) {
    let speed = world
        .borrow::<gizmo::physics::vehicle::VehicleController>()
        .get(state.rig.chassis)
        .map(|v| v.current_speed_kmh.abs())
        .unwrap_or(0.0);
    egui::Area::new(egui::Id::new("hud"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-30.0, -30.0))
        .show(ctx, |ui| {
            ui.heading(format!("{speed:.0} km/h"));
            ui.label("W/S sür · A/D direksiyon · Space fren · R reset");
        });
}
