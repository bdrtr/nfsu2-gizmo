//! # Drive an NFSU2 car through Bayview (M3)
//!
//! The city `nfs_fly` looks at, given to physics, with the car `nfs_drive` drives standing on it.
//! Both halves are the shared layers — [`nfsu2::world`] turns the region bundles into cell-sized
//! meshes and per-cell triangle soup, [`nfsu2::rig`] builds the car — so what is here is only the
//! join: one `Collider::trimesh` per cell, and a car placed on top.
//!
//! **The colliders and the visuals are built from the same objects.** `dedup` and `nearest` run
//! once, up front, and both halves are handed the result — otherwise the edge of an object budget
//! is a place where you can see a road you cannot drive on, or hit one that is not drawn.
//!
//! ```bash
//! cargo run --release -p nfsu2 --bin nfs_cruise -- "$NFSU2_ROOT/TRACKS" "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
//! ```
//!
//! Controls: **W/↑** accelerate · **S/↓** reverse · **A/D or ←/→** steer · **Space** brake ·
//! **R** back to the start · **T** auto-shift · hold **right mouse** to orbit · **F** print where
//! the car is.
//!
//! Env: `NFS_AT="x,y,z"` where to start — downtown sits near `y ≈ 27` and the airport near
//! `y ≈ -11`, so the height matters as much as the place · `NFS_BUDGET=<n>` caps objects,
//! nearest-first · `NFS_DIAG=1` prints the physics' own view once a second · plus everything
//! [`nfsu2::rig`] reads (`NFS_PAINT`, `NFS_KIT`, `NFS_ENGINE`, `NFS_SHOTCAM`, …).

use gizmo::egui;
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use gizmo_nfs::types::AssetHash;
use nfsu2::geom::add_transform;
use nfsu2::rig::{spawn_car, CarRig, ChaseCamera, Driver, Placement, Rescue};
use nfsu2::scene::{self, Textures};
// Aliased: `world` is the ECS `World` in every function here, and a module by the same name three
// characters from a variable of another type is a re-read waiting to happen.
use nfsu2::world as city;
use std::collections::HashMap;

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";

/// Where to start when nothing says otherwise: the street outside Bayview's tuning shops.
///
/// The old default, `(1354, −11, −2457)`, was chosen for having ground rather than for being
/// anywhere: it is **inside the airport**. The objects covering it are `TRN_RDP_RUNWAY_KT_CHOP_*`,
/// and a bundle-wide render of `STREAML4RB` is a runway, an apron and taxiways. Every building in
/// the city (`XB_*`) sits between `x ≈ −400` and `x ≈ 1000`, so that spawn was east of all of them,
/// on open tarmac with the skyline a kilometre off — which is why driving from it felt like being
/// outside the world rather than in it.
///
/// This one is measured the same way, by dropping the car and reading `NFS_DIAG`: it settles at
/// `y = 27.06` with four wheels down, under the elevated highway, with `XB_PERFORMANCE_SHOP`,
/// `XB_PAINTSHOPA`, `XB_AUDIOSHOPA` and `XB_BODYSHOPA` within 200 m. Note the height — the city is
/// not flat, and `y ≈ −11` is true of the airport, not of downtown.
const DEFAULT_AT: Vec3 = Vec3::new(710.0, 27.0, 888.0);

/// How far above the named point the car is dropped.
///
/// The city's surface height at a given XZ is not known without querying it — the point above is a
/// road *near* `y = -11`, not a contact patch — so the car is dropped from a little way up and the
/// suspension settles it. Too small and it spawns inside the tarmac; too large and it lands hard.
const DROP: f32 = 1.5;

struct CruiseState {
    rig: CarRig,
    driver: Driver,
    camera: ChaseCamera,
    /// The last whole second `NFS_DIAG` printed a line for.
    diag_tick: i32,
    t: f32,
    stats: CityStats,
}

/// What loading the city produced, kept for the HUD — the numbers that say whether the world under
/// the car is the world in front of it.
struct CityStats {
    objects: usize,
    meshes: usize,
    cells: usize,
    triangles: usize,
    drivable: usize,
    /// Sky shells and panorama panels — drawn, but as a backdrop rather than as world.
    backdrop: usize,
}

fn main() {
    gizmo::app::setup_panic_hook();
    App::<CruiseState>::new("Gizmo — NFSU2 Bayview", 1600, 900)
        .add_plugin(gizmo::plugins::TransformPlugin)
        .set_setup(setup)
        .set_update(update)
        .set_ui(ui)
        .set_render(|world, _s, encoder, view, renderer, _t| {
            // The city is baked-lit and opaque; the screen-space passes cost frame time and have
            // nothing to work with here.
            renderer.gpu_fluid = None;
            renderer.gpu_particles = None;
            renderer.ssr = None;
            renderer.ssgi = None;
            renderer.volumetric = None;
            // TAA stays on, unlike the small-world binaries: `nfs_fly` keeps it and this is the
            // same city at the same distances.
            gizmo::systems::default_render_pass(world, encoder, view, renderer);
        })
        .run()
        .expect("failed to run app");
}

fn setup(world: &mut World, renderer: &gizmo::renderer::Renderer) -> CruiseState {
    let tracks = city::tracks_path(std::env::args().nth(1));
    let car_path = std::env::args()
        .nth(2)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .unwrap_or_else(|| DEFAULT_CAR.to_string());
    let at = city::start_at(DEFAULT_AT);
    let budget = std::env::var("NFS_BUDGET").ok().and_then(|s| s.parse::<usize>().ok());

    let city::Bundles { files, meshes, packs, shared } = city::load(&tracks);
    println!(
        "{} bundle(s), {} objects, {} packs, {} shared textures",
        files.len(),
        meshes.len(),
        packs.len(),
        shared.len()
    );

    // One preparation, feeding both halves — and neither of them gets the backdrop, because a sky
    // shell handed to physics is a dome the car would drive into.
    //
    // The backdrop comes out first and is kept, not dropped. `is_drawn` excludes two families for
    // two different reasons: the LOD proxy is a coarse stand-in that sits across the real streets
    // and has nothing to replace it with, while the sky shell and panorama panels were only ever a
    // problem because *ordinary* geometry that large is a wall across the frame. Drawn as a skybox
    // they cannot be — `sky.wgsl` pins their depth to the far plane — so the reason to drop them
    // does not apply and the world stops ending at a flat grey horizon.
    let (backdrop, rest): (Vec<_>, Vec<_>) =
        meshes.into_iter().partition(|m| city::is_backdrop(&m.header.name));
    let mut drawn = rest;
    drawn.retain(|m| city::is_drawn(&m.header.name));
    let declared = drawn.len();
    let mut objects = city::dedup(drawn);
    println!("{declared} drawn objects, {} after dedup", objects.len());
    city::nearest(&mut objects, at, budget);

    let mut phys = PhysicsWorld::new();
    phys.integrator.gravity = Vec3::new(0.0, -9.81, 0.0);

    // ── The city as something to hit ──
    //
    // One static trimesh body per cell. Not one for the whole city (its bounding box would pair
    // with every dynamic body every step) and not one per solid (14,000 bodies). The cell is the
    // unit that is neither, and it is the same 256 m cell the visuals merge into, so the collider
    // under the car and the mesh in front of it come from the same objects.
    let colliders = city::collision_cells(&objects);
    let mut stats = CityStats {
        objects: objects.len(),
        meshes: 0, // the visuals are built below
        cells: colliders.len(),
        triangles: colliders.iter().map(city::CityCollider::triangles).sum(),
        drivable: colliders.iter().map(city::CityCollider::drivable).sum(),
        backdrop: 0, // the backdrop is built below
    };
    for cell in colliders {
        // Vertices are already relative to the cell's own origin, so the body goes there and the
        // collider's bounds are the cell's rather than the city's.
        let at_cell = Transform::new(cell.origin);
        let entity = world.spawn();
        add_transform(world, entity, at_cell);
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
    println!(
        "collision: {} cells, {} triangles ({} drivable)",
        stats.cells, stats.triangles, stats.drivable
    );

    // ── The city as something to look at ──
    // `None` for the budget: `nearest` has already run, and running it again on an already-trimmed
    // list would be a second sort for nothing.
    let visuals = city::build_region(&renderer.device, objects, &packs, Some(&shared), at, None);
    stats.meshes = visuals.meshes.len();

    // The sky and the painted panorama, through the same builder — they are ordinary geometry, and
    // only their *material* is unusual. `at` for the budget centre is irrelevant here: `None` keeps
    // all of them, and there are 40 sky shells and a handful of panels across the eight bundles.
    let sky = city::build_region(&renderer.device, backdrop, &packs, Some(&shared), at, None);
    stats.backdrop = sky.meshes.len();
    // Whether the sky is the game's or a white stand-in is not a thing to judge by eye — an
    // untextured shell and an overcast sky texture are the same pale grey on screen.
    println!(
        "backdrop: {} meshes, {} textured, {} unresolved runs",
        sky.meshes.len(),
        sky.meshes.iter().filter(|m| m.texture.is_some()).count(),
        sky.unresolved_runs
    );

    let mut assets = AssetManager::new();
    let white = assets.create_white_texture(
        &renderer.device,
        &renderer.queue,
        &renderer.scene.texture_bind_group_layout,
    );
    {
        let mut tex = Textures {
            assets: &mut assets,
            device: &renderer.device,
            queue: &renderer.queue,
            layout: &renderer.scene.texture_bind_group_layout,
        };
        let mut bound: HashMap<AssetHash, _> = HashMap::new();
        for key in visuals.meshes.iter().chain(&sky.meshes).filter_map(|m| m.texture) {
            if bound.contains_key(&key) {
                continue;
            }
            // The region's own packs first, then the shared tiers — the same order the mesh's key
            // was resolved in, so a key that resolved shared uploads from shared.
            let own = packs.iter().find_map(|p| p.get(key).and_then(|r| p.decode(r).ok()));
            let Some(img) = own.or_else(|| shared.get(key).cloned()) else { continue };
            if let Some(bg) = tex.upload(&format!("city_{:08X}", key.0), &img.rgba, img.width, img.height) {
                bound.insert(key, bg);
            }
        }
        for m in &visuals.meshes {
            // The city's lighting is baked into its vertex colours; `BakedLit` multiplies them in
            // rather than relighting a static world that was never drawn to be relit.
            let material = match m.texture.and_then(|k| bound.get(&k)) {
                Some(bg) => Material::new(bg.clone()).with_baked_lit(Vec4::new(1.0, 1.0, 1.0, 1.0)),
                None => Material::new(white.clone()).with_baked_lit(Vec4::new(0.35, 0.35, 0.38, 1.0)),
            };
            scene::spawn_mesh(world, m.mesh.clone(), material, Transform::new(m.origin));
        }

        // Double-sided, because a sky shell is seen from the inside and its triangles face out —
        // single-sided it culls to nothing, which looks exactly like not drawing it at all.
        for m in &sky.meshes {
            let material = match m.texture.and_then(|k| bound.get(&k)) {
                Some(bg) => Material::new(bg.clone()).with_skybox().with_double_sided(true),
                // The sky shells' own keys live in `TRACKS/LOC4DYNTEX.BIN`, which this binary does
                // not open, so most resolve nowhere. White here rather than the city's grey: the
                // shell's baked vertex colour *is* the sky gradient, and white lets it through.
                None => Material::new(white.clone()).with_skybox().with_double_sided(true),
            };
            scene::spawn_mesh(world, m.mesh.clone(), material, Transform::new(m.origin));
        }
    }

    // The sun is for the *car*: its materials are PBR and the city's are baked-lit, so this lights
    // one object in the frame — and casts the shadow that puts it on the road rather than above it.
    scene::add_lights(
        world,
        Transform::new(at + Vec3::new(0.0, 400.0, 0.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.9)),
        2.2,
        at + Vec3::new(-200.0, 200.0, -100.0),
    );

    let rig = spawn_car(
        world,
        renderer,
        &mut assets,
        &mut phys,
        &car_path,
        Placement { ground: at, yaw: 0.0, clearance: DROP },
    );
    world.insert_resource(assets);
    world.insert_resource(phys);

    // near 0.5 / far 20 000 — the pair `nfs_fly` measured for this city. The ratio is what costs
    // depth precision, and a city needs the far plane, so the near plane is what has to give.
    let camera =
        ChaseCamera::spawn(world, rig.start.position + Vec3::new(0.0, 4.0, 10.0), 0.5, 20_000.0);

    println!("cruising at {:?} — {} meshes drawn", rig.start.position, stats.meshes);
    CruiseState { rig, driver: Driver::new(), camera, diag_tick: -1, t: 0.0, stats }
}

fn update(world: &mut World, state: &mut CruiseState, dt: f32, input: &Input) {
    state.t += dt;

    let controls = state.driver.read(input, dt);
    state.rig.drive(world, &controls);

    if input.is_key_just_pressed(KeyCode::KeyR as u32) {
        state.rig.reset(world);
        state.driver.reset();
    }

    state.driver.step_physics(world, dt);

    let Some(pose) = state.rig.pose(world) else { return };

    // The city has holes the shipped geometry never covered; a car that finds one must come back,
    // not fall for ever. One frame is skipped rather than driving the camera to a pose that no
    // longer exists.
    match state.rig.keep_in_world(world, pose, dt) {
        Rescue::None => {}
        Rescue::ToLastGround => {
            state.driver.reset();
            println!("out of the world at {:?} — back on the last ground", pose.position);
            return;
        }
        // The rig has already said why, once. Repeating it every 2.5 s would bury the diagnostics.
        Rescue::NowhereSafe => {
            state.driver.reset();
            return;
        }
    }

    if input.is_key_just_pressed(KeyCode::KeyF as u32) {
        println!("NFS_AT=\"{:.0},{:.0},{:.0}\"", pose.position.x, pose.position.y, pose.position.z);
    }
    diagnose(world, state, pose);

    state.rig.sync_visuals(world, pose, dt, controls.steer);
    state.camera.update(world, input, pose, dt);
}

/// `NFS_DIAG=1`: once a second, whether the car is standing on the city or falling through it.
///
/// The one question this binary exists to answer. A car that never grounds is a collider that was
/// not built where the road is; a car that grounds and then sinks is a narrowphase problem. Both
/// look the same from the outside and differ in these numbers.
fn diagnose(world: &World, state: &mut CruiseState, pose: nfsu2::rig::Pose) {
    if std::env::var("NFS_DIAG").is_err() {
        return;
    }
    let tick = state.t as i32;
    if tick == state.diag_tick {
        return;
    }
    state.diag_tick = tick;
    let vehicles = world.borrow::<gizmo::physics::vehicle::VehicleController>();
    let Some(v) = vehicles.get(state.rig.chassis) else { return };
    let grounded: Vec<bool> = v.wheels.iter().map(|w| w.is_grounded).collect();
    let vel = world.borrow::<Velocity>().get(state.rig.chassis).map_or(Vec3::ZERO, |v| v.linear);
    println!(
        "diag  pos ({:+.1},{:+.2},{:+.1})  vel ({:+.2},{:+.2},{:+.2})  speed {:+.1} km/h  cell {:?}  grounded {grounded:?}",
        pose.position.x, pose.position.y, pose.position.z,
        vel.x, vel.y, vel.z,
        v.current_speed_kmh,
        city::cell_of(pose.position),
    );
}

fn ui(world: &mut World, state: &mut CruiseState, ctx: &egui::Context) {
    let speed = world
        .borrow::<gizmo::physics::vehicle::VehicleController>()
        .get(state.rig.chassis)
        .map(|v| v.current_speed_kmh.abs())
        .unwrap_or(0.0);
    let s = &state.stats;
    egui::Area::new(egui::Id::new("city"))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(24.0, 24.0))
        .show(ctx, |ui| {
            ui.label(format!("{} obje · {} mesh · {} backdrop", s.objects, s.meshes, s.backdrop));
            ui.label(format!("{} hücre · {} üçgen ({} sürülebilir)", s.cells, s.triangles, s.drivable));
            ui.label("W/S · A/D · Space · R · F konumu yazdırır");
        });
    egui::Area::new(egui::Id::new("spd"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-30.0, -30.0))
        .show(ctx, |ui| {
            ui.heading(format!("{speed:.0} km/h"));
        });
}
