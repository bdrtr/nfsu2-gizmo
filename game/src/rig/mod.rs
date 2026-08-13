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
mod pilot;

use crate::world::Ground;

pub use chase::ChaseCamera;
pub use drive::{Controls, Driver, FIXED_DT};
pub use pilot::Pilot;

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

/// How long all four wheels must stay down before that pose counts as ground worth returning to.
///
/// Not a formality. A car falling through a hole clips the *underside* of the terrain on the way
/// past — one or two frames, one or two wheels — and recording that pose is how a rescue puts the
/// player somewhere below the world on an empty plain. **Ground is where the car stayed, not
/// everything it touched.**
pub const GROUND_SETTLE: f32 = 0.25;

/// How far below the last standing pose is too far, whatever the wheels report.
///
/// The airborne timer alone is not enough, and the way it fails is not hypothetical: a car dropping
/// through a hole clips things on the way down — the underside of a road, a building's foundation, a
/// kerb edge — and each graze puts a wheel on the ground for a frame and resets the timer. The fall
/// then never "counts", and the car keeps going. Depth below the last place it actually *stood*
/// cannot be reset that way. 60 m is far deeper than any drop Bayview's roads set up (they sit
/// within a few metres of `y ≈ −11`) and far shallower than a fall out of the world.
///
/// **Below the last standing pose, not below anything absolute.** A world floor was tried and is the
/// wrong instrument, and so is a depth below the starting grid: `Paths4061` starts on an elevated
/// section at `y = 323` and its route descends sixty metres, so "fifty metres below the grid" called
/// three cars that had driven down a hill and parked, upright and stationary, fallen out of the
/// world. Depth below where the car itself last stood is local, needs no geometry, and means the
/// same thing on every route.
pub const FALL_DEPTH: f32 = 60.0;

/// How far ahead of the chassis centre the fence asks its question, in metres.
///
/// **Swept, over eight routes, against a field that had ten cars leave the world with no fence at
/// all** (3,652 m covered, 848 junctions):
///
/// | lead | off the world | distance | junctions |
/// |---|---:|---:|---:|
/// | 0 m | 5 | 3,934 m | 865 |
/// | 1.0 | 3 | 3,946 | 851 |
/// | 2.2 | 2 | 3,746 | 813 |
/// | 2.6 | 2 | 4,019 | 833 |
/// | **3.0** | **1** | **3,881** | **846** |
/// | 3.4 | 1 | 3,623 | 822 |
/// | 3.8 | 2 | 3,252 | 783 |
/// | 4.5 | 0 | 3,482 | 771 |
///
/// Falls fall away monotonically with the lead and the distance is flat until about 3.4 m, after
/// which the fence starts refusing legitimate road: at 4.5 m no car leaves the world and the field
/// covers 170 m *less* than with no fence at all. Three metres is the middle of that plateau — one
/// car lost instead of ten, more ground covered than unfenced, and junctions untouched (846 v 848).
///
/// **The principled value was tried and lost.** Probing from the front axle — the contact point that
/// actually loses the ground first, taken from the car's own wheel mounts — is the number with a
/// reason behind it, and it measures worse on every column than the swept one: 3 cars lost, 3,686 m,
/// 819 junctions. Written down so it is not re-derived as an improvement.
const FENCE_LEAD: f32 = 3.0;

/// What the car's own suspension says about its footing — see [`CarRig::watch_ground`].
#[derive(Clone, Copy, Debug)]
pub struct Standing {
    /// The last pose the car properly stood at: all four wheels down, upright, held for
    /// [`GROUND_SETTLE`]. Starts at the spawn pose, so there is always somewhere to name.
    pub last: Transform,
    /// Whether this step's pose was the one just recorded into [`Self::last`]. The moment a caller
    /// wanting to remember *anything else* about standing — speed, heading, the time — must catch,
    /// because one step later the pose is history and the state that went with it is gone.
    pub stood: bool,
    /// Whether the car has ever stood anywhere. While false, [`Self::last`] is still the spawn pose
    /// and nothing has shown it to be ground.
    pub ever: bool,
    /// All four wheels touching, right now.
    pub on_all_four: bool,
    /// Upright, right now — so a car on its roof is never mistaken for one on its wheels.
    pub upright: bool,
    /// Unbroken seconds with no wheel touching anything.
    pub airborne_for: f32,
    /// How far below [`Self::last`] the car is now. Negative above it.
    pub below: f32,
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
    /// It was on its side and has been set upright again. A different report from the other two on
    /// purpose: a car that rolled never left the world, and saying it did would be a lie the next
    /// person reading a log has to un-learn.
    Righted,
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
    /// Seconds spent past level, unbroken. Reset the moment the car is upright again.
    rolled_for: f32,
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
        rolled_for: 0.0,
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

    /// How many of the car's wheels are touching something, and how many there are.
    ///
    /// The suspension's own answer, against the real colliders — not a second opinion from a height
    /// query, which is why a caller trying to work out *why* a car left the world should start here
    /// and go to [`crate::world::Ground`] only for the city's side of the story.
    ///
    /// `(0, 0)` means there is no controller to ask. That is not "airborne": a car with no wheels
    /// has not left the ground, it has no ground to leave, and the two must not be conflated by
    /// anyone reading this.
    #[must_use]
    pub fn wheels_down(&self, world: &World) -> (usize, usize) {
        let vehicles = world.borrow::<VehicleController>();
        vehicles.get(self.chassis).map_or((0, 0), |v| {
            (v.wheels.iter().filter(|w| w.is_grounded).count(), v.wheels.len())
        })
    }

    /// Watch the car's footing for one step and remember the last place it properly stood.
    ///
    /// All the bookkeeping [`Self::keep_in_world`] needs, with none of the rescuing — so a harness
    /// that wants to *watch* cars leave the world rather than catch them (`nfs_sim`) asks exactly
    /// the same question the game asks, and cannot drift from it. Call it once per step per car.
    ///
    /// Two different predicates, deliberately not one. Falling is "no wheel is touching" — the one
    /// `NFS_DIAG` prints. Ground is "all four are, and it is upright", which is much stricter, and
    /// the strictness is the point: a car resting on its roof or wedged nose-down against a wall is
    /// not somewhere to come back to.
    ///
    /// A car with no controller is neither airborne nor standing: it has no wheels to report with,
    /// and calling that airborne would start a fall timer on something that cannot fall.
    pub fn watch_ground(&mut self, world: &World, pose: Pose, dt: f32) -> Standing {
        let (down, total) = self.wheels_down(world);
        let (airborne, on_all_four) = (total > 0 && down == 0, total > 0 && down == total);
        let upright = (pose.rotation * Vec3::Y).dot(Vec3::Y) > 0.7;

        let mut stood = false;
        if on_all_four && upright {
            self.grounded_for += dt;
            if self.grounded_for >= GROUND_SETTLE {
                self.has_grounded = true;
                self.last_safe = Transform::new(pose.position).with_rotation(pose.rotation);
                stood = true;
            }
        } else {
            self.grounded_for = 0.0;
        }
        if airborne {
            self.airborne_for += dt;
        } else {
            self.airborne_for = 0.0;
        }

        Standing {
            last: self.last_safe,
            stood,
            ever: self.has_grounded,
            on_all_four,
            upright,
            airborne_for: self.airborne_for,
            below: self.last_safe.position.y - pose.position.y,
        }
    }

    /// Hold the car on the city, at the edge the city's own geometry has — the barrier this
    /// install's files do not carry. Returns whether it pushed back this step.
    ///
    /// `0x0003410B` is in no file here (`ROADMAP.md` §M4), so NFSU2's own fences are simply absent
    /// and a car that reaches the lip of the world goes over it. Two sources for a replacement were
    /// available and the choice was measured, not assumed: a corridor around the race line would
    /// have caught eight of the ten cars that left the world, and **two were still on the course**
    /// when the ground stopped — on `Paths4041` the race line runs along the lip. The route does not
    /// know where the city ends. [`Ground::edge_at`] does.
    ///
    /// **This is a wall, not a driving aid.** It removes the component of velocity that points off
    /// the city and leaves everything else, so a car meeting the edge slides along it and keeps its
    /// speed down the road — the behaviour a barrier has — rather than being stopped dead or
    /// steered by an invisible hand. It applies to the player exactly as it applies to a rival,
    /// which is the whole point of putting it here and not in [`Pilot`](super::Pilot).
    ///
    /// Only while the car is on its wheels. A car already in the air is
    /// [`Self::keep_in_world`]'s problem, and a jump that clears a gap is not a barrier violation.
    pub fn hold_at_edge(&mut self, world: &mut World, pose: Pose, ground: &Ground) -> bool {
        /// How far ahead to look, as a multiple of speed in m/s, and the bounds on it.
        ///
        /// Half a second of travel: long enough to have somewhere to put the car at 100 km/h, short
        /// enough that a fence never reaches across a road it is not on the edge of.
        const LOOK_PER_SPEED: f32 = 0.5;
        const LOOK_MIN: f32 = 4.0;
        const LOOK_MAX: f32 = 16.0;
        /// The height window the probes allow, the same one the network's wall filter uses.
        const SLACK: f32 = 8.0;

        let (down, total) = self.wheels_down(world);
        if total == 0 || down == 0 {
            return false;
        }

        let v = {
            let velocities = world.borrow::<Velocity>();
            velocities.get(self.chassis).map_or(Vec3::ZERO, |x| x.linear)
        };
        let flat = Vec3::new(v.x, 0.0, v.z);
        let speed = flat.length();
        if speed < 0.5 {
            return false;
        }
        let dir = flat / speed;

        // One probe in the common case. The ring of twelve behind [`Ground::edge_at`] only runs
        // once this one has already found the ground stopping in front of the car, which is rare
        // enough that the cost of the fence is a single query per car per step almost always.
        // **From the nose, not the middle.** A car is 4.4 m long and the pose is its centre, so a
        // fence measured from the centre engages when the front axle is already over the lip — and
        // by then the car goes over under gravity, which is not a horizontal velocity and not
        // something this can take back. Measured with the probe at the centre: five cars still left
        // the world, at 0, 1, 9, 13 and 27 km/h — pressed against the fence and tipping over it
        // rather than driving through it.
        let lead: f32 =
            std::env::var("NFS_FENCE_LEAD").ok().and_then(|v| v.parse().ok()).unwrap_or(FENCE_LEAD);
        let from = pose.position + dir * lead;
        let look = (speed * LOOK_PER_SPEED).clamp(LOOK_MIN, LOOK_MAX);
        if ground.gap_along(from, from + dir * look, SLACK, 2.0).is_none() {
            return false;
        }
        // **Measured again on 2026-08-13, and it is badly aimed.** The fence still earns its place
        // over eight routes — 2 cars off the world against **9** with it off — but it costs 62
        // waypoints and 491 m for that, and the cost is not where the saving is. On **four of the
        // eight routes it saves nobody**, and on two of those it is expensive: `Paths4001` loses
        // 202 m to it and `Paths4002` **340 m**, the latter while firing 122 times in a race where
        // turning the fence off drops nobody at all. Everything it actually saves is on `Paths4041`
        // (6 → 2), the route whose race line runs along the lip of the void, plus one car each on
        // three others.
        //
        // The shape of the fault is below rather than here: `n` is the **sum** of the directions
        // with no ground, so where a car is on something narrow with air on both sides those two
        // cancel and what is left points along the road. `outward` is then the whole of the car's
        // forward speed and this deletes it — which is why 4002's field ends the race at 0 km/h
        // instead of sliding along an edge the way a fence is supposed to let it.
        //
        // Why the ground looks as though it stops ahead is **not measured yet**, and the fix has to
        // start there: a straight probe inside an 8 m height window cannot tell a road that *ends*
        // from a road that *turns*, and on a raised carriageway the second is far more likely. See
        // `ROADMAP.md`.
        let Some(n) = ground.edge_at(from, look, SLACK) else { return false };
        let outward = flat.dot(n);
        if outward <= 0.0 {
            // Already leaving the edge behind. A barrier that also stopped a car driving *away*
            // from it would be a trap rather than a fence.
            return false;
        }

        let mut velocities = unsafe { world.borrow_mut_unchecked::<Velocity>() };
        if let Some(mut x) = velocities.get_mut(self.chassis) {
            x.linear -= n * outward;
        }
        true
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
        /// How long a car may lie on its side before it counts as needing the same help.
        ///
        /// **Falling out of the world was never the only terminal state and the other one had no
        /// answer at all.** Measured over eight routes: **25 of 64 cars spend time on their side**,
        /// and a car on its side is not slowed down, it is *finished* — one sat at the same
        /// coordinates for seventy-two seconds. Worse, its pilot cannot tell: it goes on walking the
        /// graph, so a motionless car banked 115 junctions in that time, and across the field cars
        /// that had rolled held **52 % of every junction counted**.
        ///
        /// Longer than the fall grace because a car can be up on two wheels through a corner and
        /// come back down, and that is driving rather than crashing.
        const ROLL_GRACE: f32 = 4.0;
        /// How far past level counts as being over: `up.y` below this is more than 60° from
        /// upright, which no amount of cornering reaches.
        const UPRIGHT: f32 = 0.5;

        let g = self.watch_ground(world, pose, dt);
        let too_deep = !g.on_all_four && g.below > FALL_DEPTH;
        self.rolled_for = if (pose.rotation * Vec3::Y).y < UPRIGHT {
            self.rolled_for + dt
        } else {
            0.0
        };
        let over = self.rolled_for >= ROLL_GRACE;

        if g.airborne_for < FALL_GRACE && !too_deep && !over {
            return Rescue::None;
        }
        self.rolled_for = 0.0;
        self.airborne_for = 0.0;
        self.grounded_for = 0.0;

        // Never having touched anything means `last_safe` is still the spawn pose, which this fall
        // has just disproved as ground. Rescuing to it again is a loop, so say so — once — and let
        // the caller decide. The bug is the spawn point, not the fall.
        let verdict = if over && g.airborne_for < FALL_GRACE && !too_deep {
            // Only if rolling is the *whole* reason. A car that rolled on its way off a cliff has
            // left the world, and that is the more important half to report.
            Rescue::Righted
        } else if g.ever {
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
    /// It takes no steering argument. It used to take the driver's input and re-derive an angle
    /// from it, which is how the wheels came to point out of the corner; the controller already
    /// knows the angle it steered each wheel with, so that is what is read.
    pub fn sync_visuals(&mut self, world: &mut World, pose: Pose, dt: f32) {
        // The front wheels take the angle the *physics* is steering with, one per wheel, rather
        // than re-deriving it from the input.
        //
        // Re-deriving it was wrong twice over. Sign: the visual used `-input * lock` about +Y,
        // and a positive rotation about +Y carries this engine's −Z forward toward −X, which is
        // left — the same direction the controller documents for a positive angle ("positive
        // steers the wheel to the left", vehicle/mod.rs:282). So the wheels pointed away from the
        // corner the car was taking. Magnitude: `update_vehicle` applies Ackermann, so the inner
        // wheel of a turn steers *more* than the outer one, and one shared `input * lock` cannot
        // express that at all.
        //
        // Reading the controller settles both and leaves no convention to get wrong: whatever the
        // physics steered with is what the eye sees.
        let angles: Vec<f32> = {
            let vehicles = world.borrow::<VehicleController>();
            vehicles
                .get(self.chassis)
                .map(|v| v.wheels.iter().map(|w| w.steering_angle).collect())
                .unwrap_or_default()
        };

        self.spin += (pose.speed / self.radius.max(0.05)) * dt;
        // About −X, not +X. The car's forward is −Z (see [`Placement::yaw`]), and by the right-hand
        // rule a positive rotation about +X carries the top of the wheel from +Y toward +Z — which
        // is backwards. Rolling forward turns the other way, so the axle is −X and `spin` keeps its
        // plain meaning: how far the wheel has rolled *forward*.
        let spin = Quat::from_axis_angle(Vec3::NEG_X, self.spin);

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
        for (i, w) in self.wheels.iter().enumerate() {
            // The wheel mesh is modelled for one side; yaw the left wheels 180° so their rim faces
            // outward (else the flat inboard back shows). The mirror is innermost, so spin still
            // turns about the shared chassis axle and both sides roll the same way.
            let mirror = scene::wheel_mirror(w.local);
            // Sign as the controller writes it: positive is left, about the chassis up axis. The
            // rig's wheels are built in the controller's order, so `i` indexes both.
            let turn = match angles.get(i) {
                Some(&a) if w.front => Quat::from_axis_angle(Vec3::Y, a) * spin * mirror,
                _ => spin * mirror,
            };
            place(w.id, pose.position + pose.rotation * w.local, pose.rotation * turn);
        }
    }
}
