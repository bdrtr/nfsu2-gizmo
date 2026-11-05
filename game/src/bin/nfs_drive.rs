//! # NFSU2 Car — DRIVABLE with visual wheels & per-part colours (M2+)
//!
//! A Need for Speed: Underground 2 car on a flat plane, driven by the engine's raycast
//! `VehicleController`. Everything about the car itself — parsing it, assembling its default
//! (showroom) configuration, hanging its material groups off a chassis, reading its handling
//! record — is [`nfsu2::rig`]; what is here is the *scene*: a ground plane, two lights, and the
//! HUD.
//!
//! Controls: **W/↑** accelerate · **S/↓** brake, then reverse · **A/D or ←/→** steer · **Space** handbrake ·
//! **R** reset · **T** auto-shift · hold **right mouse** to orbit.
//!
//! ```bash
//! cargo run --release -p nfsu2 --bin nfs_drive -- "/path/to/CARS/240SX/GEOMETRY.BIN"
//! ```

use gizmo::egui;
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use nfsu2::geom::add_transform;
use nfsu2::rig::{spawn_car, CarRig, ChaseCamera, Driver, Placement, Rescue, FIXED_DT};
use nfsu2::scene;

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";

struct DriveState {
    /// The engine's voice; `None` with no audio device or under `NFS_MUTE=1`.
    engine: Option<nfsu2::sound::EngineSound>,
    rig: CarRig,
    driver: Driver,
    camera: ChaseCamera,
    autodrive: bool,
    t: f32,
}

/// How wide the ground is, and how thick the slab under it.
///
/// The collider used to be a mathematical plane: infinite in the narrowphase and bounded only by
/// the +/-10 000 m box its broadphase AABB reports (`collider.rs:109`). The mesh beside it is 400
/// units across, so from +/-200 out to +/-10 000 the car drove on ground that was not there to
/// see. `world::collision_cells` promises the city will never do that — *what you hit is what you
/// see* — and there is no reason the small scenes should.
///
/// A slab rather than a plane, hung below the entity with `offset_box` so the entity and its mesh
/// stay at `y = 0`: the surface the car drives on has not moved, it just has edges now. Drive off
/// one and `CarRig::keep_in_world` catches you, which is the point — a net nothing can reach is not
/// a net.
const GROUND_SIZE: f32 = 400.0;
const GROUND_THICKNESS: f32 = 4.0;

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
    world.add_component(ground, AssetManager::create_plane(&renderer.device, GROUND_SIZE));
    world.add_component(
        ground,
        Material::new(white).with_pbr(Vec4::new(0.13, 0.14, 0.16, 1.0), 0.95, 0.0).with_double_sided(true),
    );
    world.add_component(ground, MeshRenderer::new());
    world.add_component(ground, RigidBody::new_static());
    world.add_component(ground, Velocity::default());
    world.add_component(ground, nfsu2::scene::road(Collider::offset_box(
            Vec3::new(0.0, -GROUND_THICKNESS / 2.0, 0.0),
            Vec3::new(GROUND_SIZE / 2.0, GROUND_THICKNESS / 2.0, GROUND_SIZE / 2.0),
        )));
    world.add_component(ground, gizmo::physics::components::PhysicsMaterial::ASPHALT);
    phys.add_body(
        gizmo::physics::BodyHandle::from_id(ground.id()),
        RigidBody::new_static(),
        Transform::new(Vec3::ZERO),
        Velocity::default(),
        nfsu2::scene::road(Collider::offset_box(
            Vec3::new(0.0, -GROUND_THICKNESS / 2.0, 0.0),
            Vec3::new(GROUND_SIZE / 2.0, GROUND_THICKNESS / 2.0, GROUND_SIZE / 2.0),
        )),
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
        engine: nfsu2::sound::EngineSound::start(),
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
    // The keyboard read the way NFSU2 reads it: steering scaled to the speed, Down brakes before it
    // reverses, Space is the handbrake — `CarRig::for_human`.
    controls = state.rig.for_human(world, controls);
    if state.autodrive {
        // A slow weave with the throttle down: enough to watch the suspension work and the wheels
        // steer without a hand on the keyboard, which is what the screenshot passes want.
        controls.throttle = 1.0;
        state.driver.steer = (state.t * 0.6).sin() * 0.5;
        controls.steer = state.driver.steer;
    }
    state.rig.drive(world, &controls);
    if let Some(engine) = state.engine.as_mut() {
        if let Some(v) = world.borrow::<gizmo::physics::vehicle::VehicleController>().get(state.rig.chassis) {
            engine.follow(v, dt);
        }
    }

    if input.is_key_just_pressed(KeyCode::KeyR as u32) {
        state.rig.reset(world);
        state.driver.reset();
    }

    let rig = &state.rig;
    state.driver.step_physics_with(world, dt, |w| rig.between_step(w, FIXED_DT));
    state.rig.style_tick(world, dt);

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
        Rescue::Righted => {
            state.driver.reset();
            println!("on its side at {:?} — set upright again", pose.position);
            return;
        }
        // The rig has already said why, once.
        Rescue::NowhereSafe => {
            state.driver.reset();
            return;
        }
    }

    state.rig.sync_visuals(world, pose, dt);
    state.camera.update(world, input, pose, dt);
}

fn ui(world: &mut World, state: &mut DriveState, ctx: &egui::Context) {
    if let Some(mut d) = nfsu2::hud::Dash::of_car(world, state.rig.chassis) {
        d.style = state.rig.style_hud();
        nfsu2::hud::show(ctx, &d);
    }
    egui::Area::new(egui::Id::new("keys"))
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(24.0, -24.0))
        .show(ctx, |ui| {
            ui.label("W gaz · S fren/geri · A/D direksiyon · Space el freni · Shift nitro · E/Q vites · R reset · T otomatik vites");
        });
}
