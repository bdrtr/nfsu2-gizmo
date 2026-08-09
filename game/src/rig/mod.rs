//! The car as a *machine standing in a world*: built once, driven the same way everywhere.
//!
//! [`crate::car`] assembles a car's visuals and [`crate::car::tune`] reads how it drives. What was
//! missing is the step between those and a running binary: spawn the meshes, hang them off a
//! chassis, give that chassis a rigid body and a `VehicleController` with the wheels in the right
//! places, and then every frame turn key presses into throttle, run the physics at a fixed step, and
//! put the visual entities back where the chassis went.
//!
//! That step was written twice — `nfs_drive` and `nfs_race` shared ~230 near-identical lines — and
//! the copies had already drifted in a way that changed how the game drove:
//!
//! - **`nfs_race` raced every car at 1200 kg.** It built its chassis with `RigidBody::new(1200.0)`
//!   while reading `tune.mass_kg` out of the car's own record and printing it, so the HUD line said
//!   1275 kg and the physics integrated 1200. `nfs_drive` had been fixed; the fix never crossed.
//! - **The two disagreed on the invented fallback torque** (520 vs 560 N·m) for a car with no
//!   record, for no reason either file gave.
//! - **`nfs_drive`'s reset key dropped the car at a fixed `y = 1.5`**, not at the height it spawned
//!   from, so **R** on a tall car reset it part-way through the ground.
//!
//! A third copy was about to be written for the city, which is what made this worth doing rather
//! than worth noting. There is one copy here now, and the differences that are *real* — where the
//! car starts, how far the camera trails it, what the autodriver steers toward — are parameters.
//!
//! ## What stays in the binary
//!
//! The scene. A rig knows about a car and nothing about what it is standing on: `nfs_drive` builds
//! a ground plane, `nfs_race` a track ribbon with checkpoints, and the city binary will hand
//! [`crate::world::collision_cells`] to physics. All three then spawn one car the same way.

mod chase;
mod drive;

pub use chase::ChaseCamera;
pub use drive::{Controls, Driver, FIXED_DT};

use crate::car::tune::{steering_lock, tune_from_record, CarTune, Upgrades};
use crate::car::{build_car_visuals, PbrLook, WheelFit};
use crate::geom::add_transform;
use crate::scene::{self, Textures};
use gizmo::physics::vehicle::{Axle, VehicleController, Wheel};
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use gizmo::renderer::Renderer;
use gizmo_nfs::parse_geometry;

/// The paint to fall back on when the install's palette is unreadable — the deep blue every
/// binary has always defaulted to. `NFS_PAINT` picks a real colour out of the palette instead.
const FALLBACK_PAINT: [f32; 3] = [0.10, 0.28, 0.72];

/// The engine torque given to a car whose record could not be read, in N·m.
///
/// **Invented, and the same for every car** — which is exactly what the record replaced. It stands
/// only for a `GEOMETRY.BIN` copied out of an install, where there is no `GlobalB.lzc` to read; the
/// rig says so on stdout when it uses it, because a car silently driving on a made-up number is the
/// failure that took longest to notice the first time. (`nfs_drive` said 520 here and `nfs_race`
/// said 560, which is how much thought either number had had.)
const FALLBACK_TORQUE_NM: f32 = 520.0;

/// Where to stand the car, in terms the caller has *before* the car is built.
///
/// The spawn height cannot be one of them: it depends on the body's height and the wheel radius,
/// and neither is known until the model is parsed. So the caller says where the ground is and the
/// rig adds the rest.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    /// The point on the ground the car is placed over.
    pub ground: Vec3,
    /// Heading in radians about Y. `0` faces −Z, which is the car's own forward.
    pub yaw: f32,
    /// How far the tyres start above `ground`. Small on a flat plane; larger over a surface whose
    /// exact height at that point is not known (a hilly ribbon, a city cell), where dropping the
    /// car a little is cheaper than solving for the contact patch.
    pub clearance: f32,
}

impl Placement {
    /// Standing on flat ground at the origin, facing −Z, dropped 15 cm.
    #[must_use]
    pub fn origin() -> Self {
        Self { ground: Vec3::ZERO, yaw: 0.0, clearance: 0.15 }
    }

    /// Standing at `ground` facing along `heading` (a direction in the XZ plane; its Y is ignored).
    ///
    /// The yaw is computed rather than taken from `Quat::from_rotation_arc`: for a heading
    /// antiparallel to the car's own forward that function picks an arbitrary perpendicular axis
    /// and can land the car on its roof.
    #[must_use]
    pub fn facing(ground: Vec3, heading: Vec3, clearance: f32) -> Self {
        let h = Vec3::new(heading.x, 0.0, heading.z).normalize_or_zero();
        Self { ground, yaw: (-h.x).atan2(-h.z), clearance }
    }
}

/// One wheel as the *eye* sees it: its own entity, where it bolts to the chassis, and which axle
/// it is on (which decides whether it steers).
pub struct RigWheel {
    /// The visual entity. It has no physics of its own — the suspension is the controller's.
    pub id: u32,
    /// Mount point in chassis space, the same one the suspension raycasts from.
    pub local: Vec3,
    /// Front axle, so it turns with the steering.
    pub front: bool,
}

/// The chassis' state this frame: where it is, which way it faces, and how fast it is going.
///
/// Read once and passed around, because every consumer (visuals, camera, HUD, diagnostics) wants
/// the same three numbers and each read borrows two component stores.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
    /// Forward speed in **m/s**, signed — the controller reports km/h and everything downstream
    /// wants metres, so the conversion happens once, here.
    pub speed: f32,
}

/// What [`CarRig::keep_in_world`] did about a car that had left the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rescue {
    /// The car is on, or plausibly near, the ground. Nothing was done.
    None,
    /// It had left the world and was put back on the last ground it stood on.
    ToLastGround,
    /// It was put back, but nowhere it has been is known to be ground — it has not touched anything
    /// since it spawned, so the spawn point itself is over a hole. Reported once by the rig.
    NowhereSafe,
}

/// A car spawned into a world: its physics chassis, its visual entities, and the state that only
/// exists to make those two look like one object.
pub struct CarRig {
    /// The chassis entity — the only one with a rigid body. Everything visible follows it.
    pub chassis: u32,
    /// One entity per material group / textured mesh. They follow the chassis rigidly.
    pub visuals: Vec<u32>,
    /// The four wheel instances.
    pub wheels: Vec<RigWheel>,
    /// Wheel radius in metres: the record's own where there is one.
    pub radius: f32,
    /// Body width, height, length in metres.
    pub size: Vec3,
    /// Where the car was placed, so **R** can put it back exactly there.
    pub start: Transform,
    /// The last pose the car was properly *standing* at — all four wheels down, upright, held for
    /// a moment — which is where [`CarRig::keep_in_world`] puts it back. Not merely the last pose
    /// something was touched at: a car falling past the underside of the world touches plenty.
    /// Starts at [`Self::start`], so a car that leaves the world before it ever stands has
    /// somewhere to return to.
    last_safe: Transform,
    /// Seconds of unbroken air. Reset the moment any wheel touches.
    airborne_for: f32,
    /// Seconds all four wheels have been down, upright, without a break.
    grounded_for: f32,
    /// Whether a wheel has ever touched anything. While this is false there is no safe pose to
    /// return to, and a rescue is treating the spawn point as ground it was never shown to be.
    has_grounded: bool,
    /// So [`Rescue::NowhereSafe`] is reported once rather than every 2.5 s.
    warned_nowhere: bool,
    /// What the car's own record said, when there was one to read.
    pub tune: Option<CarTune>,
    /// Accumulated wheel rotation, radians. Visual only — the controller does not model wheel spin,
    /// so this is integrated from road speed.
    spin: f32,
}

/// Build a car from its `GEOMETRY.BIN` and stand it in the world at `place`.
///
/// Spawns the visual entities, the four wheel instances, and one chassis carrying the rigid body,
/// the collider and a `VehicleController` whose wheels sit at the same mounts the visuals use — so
/// the wheel the eye sees is the wheel the suspension raycasts from.
///
/// `assets` is borrowed rather than owned because the caller usually has other textures to upload
/// (a city, a track) and one `AssetManager` is what shares the cache between them; insert it into
/// the world afterwards.
pub fn spawn_car(
    world: &mut World,
    renderer: &Renderer,
    assets: &mut AssetManager,
    phys: &mut PhysicsWorld,
    path: &str,
    place: Placement,
) -> CarRig {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let all = parse_geometry(&bytes).expect("parse GEOMETRY.BIN");
    let tpk = crate::assets::load_tpk_beside(path); // TEXTURES.BIN next to the model, if present
    // One read of the install's bundle answers all three: where the wheels go, how the car drives,
    // and which colours it may be painted. Absent, each falls back on its own.
    let gb = crate::assets::load_globalb_beside(path);
    let paint = crate::assets::paint_from_palette(&gb.palette, FALLBACK_PAINT);
    let cfg = crate::parts::CarConfig::from_env();

    // Double-sided throughout: the greenhouse has no glass geometry (windows are texture-only
    // decals), so single-sided rendering lets the camera see through the empty openings into the
    // dark cabin and out the far side of the body.
    let white = renderer_white(renderer, assets);
    let flat = |look: PbrLook| {
        Material::new(white.clone())
            .with_pbr(Vec4::new(look.rgb[0], look.rgb[1], look.rgb[2], look.alpha), look.roughness, look.metallic)
            .with_double_sided(true)
    };
    let mut car = build_car_visuals(&renderer.device, &all, tpk.as_ref(), paint, &cfg, flat);

    let size = Vec3::new(car.width, car.height, car.length);
    let WheelFit { half_wheelbase, half_track, .. } = car.wheel_fit;
    // Kept before `car` is borrowed apart below; both feed `scene::wheel_mounts`.
    let (fit, center) = (car.wheel_fit, car.center);
    // The record states the radius; `fit_wheel`'s is a bbox guess with a clamp on it.
    let radius = scene::wheel_radius(gb.info.as_ref(), fit);

    let mut tex = Textures {
        assets,
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
            flat,
        );
        (mesh, m)
    });
    let visuals = scene::spawn_body(world, &mut car, &mut tex, |bg, tint, rough, metal| {
        Material::new(bg)
            .with_pbr(Vec4::new(tint[0], tint[1], tint[2], 1.0), rough, metal)
            .with_double_sided(true)
    });

    // ── Wheels: the single wheel mesh instanced at the four corners **the record names** ──
    //
    // Not at `fit_wheel`'s guess — one symmetric pair derived from the modelled wheel's bounding
    // box with `.max()` floors under it — while `GLOBALB` states all four mounts outright. Measured
    // on a 240SX the guess put the fronts 0.14 m out and the rears **1.56 m** out, with the rear
    // pair on the wrong sides: the record's order is front-left, front-right, **rear-right**,
    // rear-left, and the sign table read `(-1,-1), (1,-1), (-1,1), (1,1)`.
    let mounts = scene::wheel_mounts(gb.info.as_ref(), fit, center, size.y);
    let mut wheels = Vec::new();
    if let Some((mesh, wmat)) = wheel {
        for (i, &local) in mounts.iter().enumerate() {
            let id = scene::spawn_mesh(world, mesh.clone(), wmat.clone(), Transform::new(Vec3::ZERO));
            // The first two are the front pair — the record's own order, and the one the vehicle
            // controller's axles are built in below.
            wheels.push(RigWheel { id, local, front: i < 2 });
        }
    }

    // ── Chassis (physics only; everything above follows it) ──
    let start = Transform::new(place.ground + Vec3::Y * (size.y * 0.5 + radius + place.clearance))
        .with_rotation(Quat::from_rotation_y(place.yaw));
    let chassis = world.spawn();
    add_transform(world, chassis, start);

    // The car's own record, where there is one: mass, rpm limits, the gearbox, the final drive, the
    // whole nine-point torque curve on its own rpm axis, and the drivetrain. Without it the engine's
    // defaults stand, which is what every car in this game used to drive on.
    let tune = gb.info.as_ref().zip(gb.handling.as_ref()).map(|(info, h)| {
        tune_from_record(info, h, Upgrades::from_env(), half_wheelbase * 2.0, half_track * 2.0)
    });

    // The record's own mass. 1200 kg was a stand-in for every car in the game, and `nfs_race` was
    // still integrating it while printing the real one.
    let mass = tune.as_ref().map_or(1200.0, |t| t.mass_kg);
    let mut rb = RigidBody::new(mass, true);
    rb.linear_damping = 0.1;
    rb.angular_damping = 1.8;
    rb.calculate_box_inertia(size.x, size.y, size.z);
    rb.center_of_mass = Vec3::new(0.0, -size.y * 0.1, 0.0);
    rb.lock_rotation_x = false;
    rb.lock_rotation_y = false;
    rb.lock_rotation_z = false;

    let mut vehicle = VehicleController::new();
    // The suspension hangs its wheel *below* the attachment, so the bolt goes one rest-length above
    // the mount and the wheel settles where the record puts it.
    let rest = (radius * 0.25).max(0.05);
    for (i, &mount) in mounts.iter().enumerate() {
        let (front, left) = (i < 2, mount.x < 0.0);
        vehicle.add_wheel(Wheel {
            attachment_local_pos: mount + Vec3::new(0.0, rest, 0.0),
            radius,
            axle_type: if front { Axle::Front } else { Axle::Rear },
            is_left: left,
            suspension_rest_length: rest,
            suspension_max_travel: (radius * 0.45).max(0.12),
            suspension_stiffness: 45000.0,
            suspension_damping: 3500.0,
            wheel_mass: 25.0,
            ..Default::default()
        });
    }
    match &tune {
        Some(t) => vehicle.tuning = t.tuning.clone(),
        None => {
            // No record reachable (a `GEOMETRY.BIN` copied out of an install). Geometry is still
            // known — it comes from the model — so only the engine is invented, and it says so.
            vehicle.tuning.wheelbase = half_wheelbase * 2.0;
            vehicle.tuning.track_width = half_track * 2.0;
            vehicle.tuning.max_engine_torque = FALLBACK_TORQUE_NM;
        }
    }
    vehicle.max_steering_angle = steering_lock();

    let collider = Collider::offset_box(
        Vec3::new(0.0, size.y * 0.12, 0.0),
        Vec3::new(size.x * 0.42, size.y * 0.3, size.z * 0.46),
    );
    world.add_component(chassis, vehicle);
    world.add_component(chassis, rb);
    world.add_component(chassis, Velocity::new(Vec3::ZERO));
    world.add_component(chassis, collider.clone());
    phys.add_body(
        gizmo::physics::BodyHandle::from_id(chassis.id()),
        rb,
        start,
        Velocity::default(),
        collider,
    );

    let rig = CarRig {
        chassis: chassis.id(),
        visuals,
        wheels,
        radius,
        size,
        start,
        last_safe: start,
        airborne_for: 0.0,
        grounded_for: 0.0,
        has_grounded: false,
        warned_nowhere: false,
        tune,
        spin: 0.0,
    };
    rig.announce();
    rig
}

/// The neutral white texture every flat material samples, from the caller's own cache (the engine
/// keys it, so asking twice is not two textures).
fn renderer_white(renderer: &Renderer, assets: &mut AssetManager) -> std::sync::Arc<gizmo::wgpu::BindGroup> {
    assets.create_white_texture(&renderer.device, &renderer.queue, &renderer.scene.texture_bind_group_layout)
}

impl CarRig {
    /// Print what this car turned out to be: its dimensions, and the handling record behind them.
    ///
    /// One place rather than two, because the two had drifted into printing different subsets — and
    /// one of them was printing a mass it was not using.
    fn announce(&self) {
        println!(
            "car ready: {} visual meshes, {} wheels; dims {:.2}×{:.2}×{:.2}, r={:.2}",
            self.visuals.len(),
            self.wheels.len(),
            self.size.x,
            self.size.y,
            self.size.z,
            self.radius,
        );
        let Some(t) = &self.tune else {
            println!("handling: no record for this car — engine defaults, {FALLBACK_TORQUE_NM:.0} N·m invented");
            return;
        };
        println!(
            "handling: {:.0} kg · {} gears · final drive {:.3} · {:.0} N·m peak · {:?} · red line {:.0}",
            t.mass_kg,
            t.gears,
            t.tuning.final_drive_ratio,
            t.tuning.max_engine_torque,
            t.drivetrain,
            t.tuning.upshift_rpm,
        );
        // The curve, not just its peak — the shape is the thing that distinguishes two cars with
        // the same peak. Nine points is short enough to read.
        let curve = &t.tuning.torque_curve;
        if !curve.is_empty() {
            let rpm: Vec<String> = curve.iter().map(|(r, _)| format!("{r:>6.0}")).collect();
            let nm: Vec<String> = curve.iter().map(|(_, n)| format!("{n:>6.0}")).collect();
            println!("  curve {}   rpm", rpm.join(""));
            println!("        {}   Nm", nm.join(""));
        }
    }

    /// Read the chassis' pose and speed. `None` once the chassis is gone, which ends the frame.
    #[must_use]
    pub fn pose(&self, world: &World) -> Option<Pose> {
        let transforms = world.borrow::<Transform>();
        let vehicles = world.borrow::<VehicleController>();
        let speed = vehicles.get(self.chassis).map_or(0.0, |v| v.current_speed_kmh / 3.6);
        transforms
            .get(self.chassis)
            .map(|t| Pose { position: t.position, rotation: t.rotation, speed })
    }

    /// Hand this frame's controls to the vehicle controller.
    pub fn drive(&self, world: &mut World, c: &Controls) {
        let mut vehicles = world.borrow_mut::<VehicleController>();
        let Some(mut v) = vehicles.get_mut(self.chassis) else { return };
        v.set_reverse(c.throttle < 0.0);
        v.throttle_input = c.throttle.abs().min(1.0);
        v.brake_input = c.brake;
        v.steering_input = c.steer.clamp(-1.0, 1.0);
        if c.toggle_auto_shift {
            v.auto_shift = !v.auto_shift;
        }
    }

    /// Put the car back exactly where it was spawned, at rest.
    ///
    /// [`Self::start`] rather than a written-down constant: `nfs_drive` used to reset to a fixed
    /// `y = 1.5`, which is neither where the car started nor a height any particular car should be
    /// dropped from.
    pub fn reset(&self, world: &mut World) {
        let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
        let mut velocities = unsafe { world.borrow_mut_unchecked::<Velocity>() };
        if let Some(mut t) = transforms.get_mut(self.chassis) {
            *t = self.start;
            t.update_local_matrix();
        }
        if let Some(mut v) = velocities.get_mut(self.chassis) {
            *v = Velocity::default();
        }
    }

    /// Put the car back on the last ground it stood on, if it has left the world. Returns whether
    /// it fired.
    ///
    /// A city assembled from the shipped geometry has holes in it. `(1800, −2535)` is one: with the
    /// containment slack set to zero, **no object in any of the eight STREAM bundles covers that
    /// point**, so a car that drives off there falls for ever — measured, not supposed, by spawning
    /// at it and watching the car pass −1900 m without touching a triangle.
    ///
    /// The game NFSU2 keeps the player off those edges with barriers. This install carries **no
    /// barrier chunk at all** (`0x0003410B` is in no file — see `ROADMAP.md` §M4), so barriers have
    /// to be *derived* from the route network, which is not read yet. Until then this is what stops
    /// the fall — and a finished game keeps it anyway, because no amount of derived barrier makes a
    /// bottomless fall an acceptable failure mode.
    ///
    /// The trigger is **how long the car has been airborne**, not how deep it has got.
    ///
    /// A world floor was tried first and is the wrong instrument: derived from the city's own
    /// lowest collider vertex it comes out at `y = −766`, because the geometry reaches that far
    /// down somewhere, so the car falls 750 m — a quarter of a minute — before anything catches it.
    /// Written down as a constant instead, it would be wrong for every other region. Airborne time
    /// is local, needs no geometry, and means the same thing everywhere.
    pub fn keep_in_world(&mut self, world: &mut World, pose: Pose, dt: f32) -> Rescue {
        /// How long a car may be off the ground before it counts as having left the world.
        ///
        /// At 9.81 m/s² this is ~30 m of fall: longer than any jump Bayview's roads set up (a kerb
        /// or a crest reattaches inside one `NFS_DIAG` tick), and short enough that nobody sits
        /// through the drop. A ramp that genuinely needs more air than this wants its own exemption,
        /// not a bigger number here.
        const FALL_GRACE: f32 = 2.5;

        /// How long all four wheels must stay down before that pose counts as ground worth
        /// returning to.
        ///
        /// Not a formality. A car falling through a hole clips the *underside* of the terrain on
        /// the way past — one or two frames, one or two wheels — and recording that pose is how a
        /// rescue puts the player somewhere below the world on an empty plain. Ground is where the
        /// car stayed, not everything it touched.
        const SETTLE: f32 = 0.25;

        // Two different questions, deliberately not the same predicate. Falling is "no wheel is
        // touching" — the one `NFS_DIAG` prints. Ground is "all four are", which is much stricter.
        let (airborne, on_all_four) = {
            let vehicles = world.borrow::<VehicleController>();
            match vehicles.get(self.chassis) {
                Some(v) => (
                    v.wheels.iter().all(|w| !w.is_grounded),
                    !v.wheels.is_empty() && v.wheels.iter().all(|w| w.is_grounded),
                ),
                None => (false, false),
            }
        };
        // And upright, so a car resting on its roof or wedged nose-down against a wall is never
        // stored as somewhere to come back to.
        let upright = (pose.rotation * Vec3::Y).dot(Vec3::Y) > 0.7;

        if on_all_four && upright {
            self.grounded_for += dt;
            if self.grounded_for >= SETTLE {
                self.has_grounded = true;
                self.last_safe = Transform::new(pose.position).with_rotation(pose.rotation);
            }
        } else {
            self.grounded_for = 0.0;
        }

        /// How far below the last standing pose is too far, whatever the wheels report.
        ///
        /// The airborne timer alone is not enough, and the way it fails is not hypothetical: a car
        /// dropping through a hole clips things on the way down — the underside of a road, a
        /// building's foundation, a kerb edge — and each graze puts a wheel on the ground for a
        /// frame and resets the timer. The fall then never "counts", and the car keeps going. Depth
        /// below the last place it actually stood cannot be reset that way. 60 m is far deeper than
        /// any drop Bayview's roads set up (they sit within a few metres of `y ≈ −11`) and far
        /// shallower than a fall out of the world.
        const FALL_DEPTH: f32 = 60.0;

        let too_deep = !on_all_four && (self.last_safe.position.y - pose.position.y) > FALL_DEPTH;

        if !airborne {
            self.airborne_for = 0.0;
        } else {
            self.airborne_for += dt;
        }
        if self.airborne_for < FALL_GRACE && !too_deep {
            return Rescue::None;
        }
        self.airborne_for = 0.0;
        self.grounded_for = 0.0;

        // Never having touched anything means `last_safe` is still the spawn pose, which this fall
        // has just disproved as ground. Rescuing to it again is a loop, so say so — once — and let
        // the caller decide. The bug is the spawn point, not the fall.
        let verdict = if self.has_grounded {
            Rescue::ToLastGround
        } else {
            if !self.warned_nowhere {
                self.warned_nowhere = true;
                println!(
                    "no ground under the spawn point {:?} — the car has never touched anything, \
                     so there is nowhere safe to return to (pick another NFS_AT)",
                    self.start.position
                );
            }
            Rescue::NowhereSafe
        };

        self.recover(world);
        verdict
    }

    /// Put the car back on the last ground it stood on, at rest, whatever the reason.
    ///
    /// Separate from [`Self::keep_in_world`] because falling out of the world is not the only way
    /// to end up somewhere a car should not be: leaving the mapped city is the other, and that one
    /// is a decision the caller makes from [`crate::world::Bounds`], not something the wheels can
    /// report.
    pub fn recover(&mut self, world: &mut World) {
        self.airborne_for = 0.0;
        self.grounded_for = 0.0;
        let safe = self.last_safe;
        let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
        let mut velocities = unsafe { world.borrow_mut_unchecked::<Velocity>() };
        if let Some(mut t) = transforms.get_mut(self.chassis) {
            *t = safe;
            t.update_local_matrix();
        }
        if let Some(mut v) = velocities.get_mut(self.chassis) {
            *v = Velocity::default();
        }
    }

    /// Move every visual entity onto the chassis: the body rigidly, the wheels with spin and steer.
    ///
    /// `steer` is the input in −1..1, not an angle; the visual lock is the same
    /// [`steering_lock`] the controller is given, so the wheel the eye sees turns as far as the
    /// wheel the physics steers.
    pub fn sync_visuals(&mut self, world: &mut World, pose: Pose, dt: f32, steer: f32) {
        self.spin += (pose.speed / self.radius.max(0.05)) * dt;
        // About −X, not +X. The car's forward is −Z (see [`Placement::yaw`]), and by the right-hand
        // rule a positive rotation about +X carries the top of the wheel from +Y toward +Z — which
        // is backwards. Rolling forward turns the other way, so the axle is −X and `spin` keeps its
        // plain meaning: how far the wheel has rolled *forward*.
        let spin = Quat::from_axis_angle(Vec3::NEG_X, self.spin);
        let steer = Quat::from_axis_angle(Vec3::Y, -steer.clamp(-1.0, 1.0) * steering_lock());

        let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
        let mut globals = unsafe { world.borrow_mut_unchecked::<GlobalTransform>() };
        let mut place = |id: u32, position: Vec3, rotation: Quat| {
            if let Some(mut t) = transforms.get_mut(id) {
                t.position = position;
                t.rotation = rotation;
                t.update_local_matrix();
                if let Some(mut g) = globals.get_mut(id) {
                    g.matrix = t.local_matrix;
                }
            }
        };
        for &id in &self.visuals {
            place(id, pose.position, pose.rotation);
        }
        for w in &self.wheels {
            // The wheel mesh is modelled for one side; yaw the left wheels 180° so their rim faces
            // outward (else the flat inboard back shows). The mirror is innermost, so spin still
            // turns about the shared chassis axle and both sides roll the same way.
            let mirror = scene::wheel_mirror(w.local);
            let turn = if w.front { steer * spin * mirror } else { spin * mirror };
            place(w.id, pose.position + pose.rotation * w.local, pose.rotation * turn);
        }
    }
}
