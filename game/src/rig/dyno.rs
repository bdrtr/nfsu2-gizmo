//! What a car can do, measured on the car.
//!
//! A rival on rails ([`super::rail`]) is never steered and never pushed back, so nothing in the
//! physics stops it doing the impossible: it goes exactly as fast as it is told. What it has to be
//! told is what the **player's** car can do, or the race is between a car and a number somebody
//! typed.
//!
//! So the numbers are measured, not modelled. Modelling was the first idea and the engine is why
//! it is not the answer: the chassis carries a `linear_damping` of 0.1 s⁻¹ — on its own −5 m/s² at
//! 180 km/h — on top of aero drag, a nine-point torque curve, an automatic box with a shift
//! cooldown and a tyre model with combined slip. A formula that agreed with all of that would be
//! the engine rewritten. Driving the engine is shorter and cannot disagree with it.
//!
//! The same [`spawn_car`](super::spawn_car) the race uses builds the same chassis, on a flat
//! asphalt plane in a world of its own, and three runs are timed at the race's own [`FIXED_DT`]:
//!
//! - **full throttle from rest** until the speed stops rising: acceleration at every m/s, and the
//!   top speed;
//! - **full brake** from there to rest: the braking deceleration;
//! - **a skidpad**: a fixed steering input and a slowly rising speed, until the circle opens up —
//!   the most lateral acceleration the tyres hold.
//!
//! The runs are deterministic, as the sim is, so a car measures the same every time.

use super::drive::{Controls, FIXED_DT};
use super::{spawn_car, CarRig, Placement};
use crate::geom::add_transform;
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use gizmo::renderer::Renderer;

/// How long the car stands before anything is asked of it, so the suspension has settled and the
/// first sample of the acceleration run is from rest rather than from a drop.
const SETTLE_S: f32 = 2.0;
/// The longest an acceleration run may take, in seconds. Generous: it ends on its own when the car
/// stops gaining speed.
const ACCEL_FOR_S: f32 = 150.0;
/// The acceleration run ends when the car has gained less than this, in m/s, over the window below:
/// that is the top speed, to within the gain.
const TOP_GAIN: f32 = 0.2;
const TOP_WINDOW_S: f32 = 8.0;
/// Below this the car counts as stopped at the end of the braking run, in m/s.
const STOPPED: f32 = 0.5;
/// The skidpad's steering input, as a fraction of the lock. Half lock is about an 11 m circle at
/// walking pace, so the tyres reach their limit at a speed the run gets to quickly.
const PAD_STEER: f32 = 0.5;
/// How fast the skidpad's target speed rises, in m/s per second. Slow enough that the car is in a
/// steady circle at every speed it passes through.
const PAD_RAMP: f32 = 0.25;
/// How long the skidpad may run, in seconds.
const PAD_FOR_S: f32 = 90.0;
/// The window lateral acceleration is averaged over, so a single step of tyre noise is not a
/// limit.
const PAD_WINDOW_S: f32 = 1.0;
/// The plane's half-size and thickness. The acceleration run covers a few kilometres in a straight
/// line, and running off the edge would end it as a fall.
const PLANE_HALF: f32 = 20_000.0;
const PLANE_THICK: f32 = 2.0;

/// What one car can do, as measured by [`Limits::measure`].
#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    /// Full-throttle acceleration in m/s², one entry per whole m/s from rest: `accel[k]` is the
    /// average while the car went from `k` to `k + 1` m/s.
    pub accel: Vec<f32>,
    /// The highest speed reached on the flat, in m/s.
    pub top: f32,
    /// Braking deceleration from the top speed to rest, averaged, in m/s².
    pub brake: f32,
    /// The most lateral acceleration held on the skidpad, in m/s².
    pub grip: f32,
}

impl Limits {
    /// Acceleration available at speed `v`, in m/s²: the measured table, linearly interpolated, and
    /// nothing at or above the top speed.
    #[must_use]
    pub fn accel_at(&self, v: f32) -> f32 {
        if v >= self.top || self.accel.is_empty() {
            return 0.0;
        }
        let x = v.max(0.0);
        let k = x.floor() as usize;
        let a = |i: usize| self.accel.get(i).copied().unwrap_or(0.0);
        let f = x - k as f32;
        a(k) * (1.0 - f) + a(k + 1).max(0.0) * f
    }

    /// Time from rest to `v`, in seconds, through the table. The number a person checks a car
    /// against, so it is what [`Self::measure`] prints.
    #[must_use]
    pub fn time_to(&self, v: f32) -> Option<f32> {
        if v >= self.top {
            return None;
        }
        let mut t = 0.0;
        for (k, a) in self.accel.iter().enumerate() {
            if k as f32 >= v {
                break;
            }
            let span = (v - k as f32).min(1.0);
            if *a <= 0.0 {
                return None;
            }
            t += span / a;
        }
        Some(t)
    }

    /// Drive the car at `car_path` on a flat plane and time it.
    ///
    /// Opens its own `World` and `PhysicsWorld`, so nothing the caller has spawned is touched; the
    /// textures it uploads go into the caller's cache, which is where the race's own cars will find
    /// them again.
    ///
    /// `NFS_DYNO=1` prints the acceleration run second by second — speed, gear, rpm — which is what
    /// says *why* a top speed is what it is. `NFS_DYNO_DAMPING=<x>` replaces the chassis'
    /// `linear_damping` for the bench only, so its share of the top speed can be read directly.
    pub fn measure(renderer: &Renderer, assets: &mut AssetManager, car_path: &str) -> Self {
        let mut bench = Bench::new(renderer, assets, car_path);
        let trace = std::env::var("NFS_DYNO").is_ok_and(|v| v != "0");
        bench.run(SETTLE_S, |_, _| Controls::default());

        // ── Full throttle from rest ──
        let mut crossed: Vec<f32> = Vec::new(); // time the car first reached k m/s, per k
        let mut history: Vec<(f32, f32)> = Vec::new();
        let start = bench.t;
        let mut top = 0.0f32;
        while bench.t - start < ACCEL_FOR_S {
            bench.step(&Controls { throttle: 1.0, ..Controls::default() });
            let v = bench.forward_speed();
            top = top.max(v);
            while (crossed.len() as f32) <= v {
                crossed.push(bench.t - start);
            }
            if trace && (((bench.t - start) / FIXED_DT).round() as usize).is_multiple_of(240) {
                let (gear, rpm) = bench.gearbox();
                println!(
                    "   t {:>5.1} s · {:>5.1} km/h · vites {gear} · {rpm:>5.0} rpm",
                    bench.t - start,
                    v * 3.6
                );
            }
            history.push((bench.t, v));
            let old = history.iter().rev().find(|(t, _)| bench.t - t >= TOP_WINDOW_S);
            if old.is_some_and(|(_, v_then)| v - v_then < TOP_GAIN) {
                break;
            }
        }
        let accel: Vec<f32> = crossed
            .windows(2)
            .map(|w| if w[1] > w[0] { 1.0 / (w[1] - w[0]) } else { 0.0 })
            .collect();

        // ── Full brake to rest ──
        let from = bench.forward_speed();
        let t0 = bench.t;
        while bench.forward_speed() > STOPPED && bench.t - t0 < 60.0 {
            bench.step(&Controls { brake: 1.0, ..Controls::default() });
        }
        let brake = (from - bench.forward_speed()) / (bench.t - t0).max(FIXED_DT);

        // ── Skidpad: a fixed lock, a slowly rising speed, the most lateral acceleration held ──
        let window = (PAD_WINDOW_S / FIXED_DT).round() as usize;
        let mut lateral: std::collections::VecDeque<f32> = Default::default();
        let mut grip = 0.0f32;
        let t0 = bench.t;
        while bench.t - t0 < PAD_FOR_S {
            let want = 2.0 + PAD_RAMP * (bench.t - t0);
            let v = bench.forward_speed();
            let c = Controls {
                throttle: ((want - v) * 0.5).clamp(0.0, 1.0),
                brake: if v > want + 1.0 { 0.3 } else { 0.0 },
                steer: PAD_STEER,
                ..Controls::default()
            };
            bench.step(&c);
            lateral.push_back(bench.speed() * bench.yaw_rate().abs());
            if lateral.len() > window {
                lateral.pop_front();
            }
            if lateral.len() == window {
                let mean = lateral.iter().sum::<f32>() / window as f32;
                grip = grip.max(mean);
                // The circle has opened up: the tyres are past their limit and the number only
                // falls from here.
                if mean < grip * 0.8 && want > 6.0 {
                    break;
                }
            }
        }

        let limits = Self { accel, top, brake, grip };
        println!(
            "dyno: top {:.0} km/h · 0-100 {} · fren {:.1} m/s² · yanal tutunma {:.1} m/s² · {:.0} s sürdü",
            limits.top * 3.6,
            limits
                .time_to(100.0 / 3.6)
                .map_or_else(|| "ulaşılamıyor".to_string(), |t| format!("{t:.1} s")),
            limits.brake,
            limits.grip,
            bench.t,
        );
        limits
    }
}

/// One car on one plane, stepped the way the race steps its cars.
///
/// Public so a measurement binary can script its own manoeuvres on the same plane the dyno uses.
pub struct Bench {
    world: World,
    rig: CarRig,
    /// Seconds simulated.
    pub t: f32,
}

impl Bench {
    /// The car at `car_path`, standing at rest on a flat asphalt plane in a world of its own.
    pub fn new(renderer: &Renderer, assets: &mut AssetManager, car_path: &str) -> Self {
        let mut world = World::new();
        let mut phys = PhysicsWorld::new();
        let plane = world.spawn();
        let at = Transform::new(Vec3::ZERO);
        add_transform(&mut world, plane, at);
        let collider = crate::scene::road(Collider::offset_box(
            Vec3::new(0.0, -PLANE_THICK / 2.0, 0.0),
            Vec3::new(PLANE_HALF, PLANE_THICK / 2.0, PLANE_HALF),
        ));
        world.add_component(plane, RigidBody::new_static());
        world.add_component(plane, Velocity::default());
        world.add_component(plane, collider.clone());
        world.add_component(plane, gizmo::physics::components::PhysicsMaterial::ASPHALT);
        phys.add_body(
            gizmo::physics::BodyHandle::from_id(plane.id()),
            RigidBody::new_static(),
            at,
            Velocity::default(),
            collider,
        );
        let rig = spawn_car(&mut world, renderer, assets, &mut phys, car_path, Placement::origin());
        world.insert_resource(phys);
        // `NFS_DYNO_DAMPING=<x>` / `NFS_DYNO_ANGDAMP=<x>`: the chassis' linear and angular damping
        // replaced, for the bench only — so each one's share of a number can be read directly.
        let knob = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<f32>().ok());
        if let Some(mut rb) = world.borrow_mut::<RigidBody>().get_mut(rig.chassis) {
            if let Some(d) = knob("NFS_DYNO_DAMPING") {
                println!("tezgâh: linear_damping {} → {d}", rb.linear_damping);
                rb.linear_damping = d;
            }
            if let Some(d) = knob("NFS_DYNO_ANGDAMP") {
                println!("tezgâh: angular_damping {} → {d}", rb.angular_damping);
                rb.angular_damping = d;
            }
        }
        Self { world, rig, t: 0.0 }
    }

    /// One physics step with these controls.
    pub fn step(&mut self, c: &Controls) {
        self.rig.drive(&mut self.world, c);
        gizmo::physics::vehicle_controller_system(&self.world, FIXED_DT);
        gizmo::physics::physics_step_system(&self.world, FIXED_DT);
        self.t += FIXED_DT;
    }

    /// Step for `seconds`, asking `controls` each step.
    pub fn run(&mut self, seconds: f32, mut controls: impl FnMut(&World, f32) -> Controls) {
        let end = self.t + seconds;
        while self.t < end {
            let c = controls(&self.world, self.t);
            self.step(&c);
        }
    }

    fn velocity(&self) -> (Vec3, Vec3, Quat) {
        let vel = self.world.borrow::<Velocity>();
        let tr = self.world.borrow::<Transform>();
        let v = vel.get(self.rig.chassis).map_or((Vec3::ZERO, Vec3::ZERO), |v| (v.linear, v.angular));
        let r = tr.get(self.rig.chassis).map_or(Quat::IDENTITY, |t| t.rotation);
        (v.0, v.1, r)
    }

    /// Speed along the car's own nose, in m/s — the car's forward is −Z.
    pub fn forward_speed(&self) -> f32 {
        let (lin, _, rot) = self.velocity();
        lin.dot(rot * Vec3::NEG_Z)
    }

    /// Speed in plan, in m/s.
    pub fn speed(&self) -> f32 {
        let (lin, _, _) = self.velocity();
        Vec3::new(lin.x, 0.0, lin.z).length()
    }

    /// The gear the box is in and the engine speed — what a trace needs to say why the car stopped
    /// gaining.
    pub fn gearbox(&self) -> (usize, f32) {
        let vehicles = self.world.borrow::<gizmo::physics::vehicle::VehicleController>();
        vehicles.get(self.rig.chassis).map_or((0, 0.0), |v| (v.current_gear, v.engine_rpm))
    }

    /// Yaw rate, in rad/s.
    pub fn yaw_rate(&self) -> f32 {
        self.velocity().1.y
    }

    /// Sideslip: the angle between where the nose points and where the car is going, in plan, in
    /// radians — positive when the car is travelling to the right of its nose. What a driver feels
    /// as the car "sliding"; near zero in a car that is gripping.
    pub fn sideslip(&self) -> f32 {
        let (lin, _, rot) = self.velocity();
        let v = Vec3::new(lin.x, 0.0, lin.z);
        if v.length() < 1.0 {
            return 0.0;
        }
        let nose = rot * Vec3::NEG_Z;
        let nose = Vec3::new(nose.x, 0.0, nose.z).normalize_or_zero();
        let right = nose.cross(Vec3::Y);
        v.dot(right).atan2(v.dot(nose))
    }

    /// Lateral acceleration felt in the car, in m/s²: speed times yaw rate.
    pub fn lateral(&self) -> f32 {
        self.speed() * self.yaw_rate()
    }

    /// Every wheel as the controller sees it: whether it touches, the suspension force it carries
    /// in newtons, and the friction of what it stands on.
    pub fn wheels(&self) -> Vec<(bool, f32, f32)> {
        let vehicles = self.world.borrow::<gizmo::physics::vehicle::VehicleController>();
        vehicles.get(self.rig.chassis).map_or_else(Vec::new, |v| {
            v.wheels.iter().map(|w| (w.is_grounded, w.suspension_force, w.surface_friction)).collect()
        })
    }

    /// The chassis' mass in kilograms, as its rigid body has it.
    pub fn mass(&self) -> f32 {
        self.world.borrow::<RigidBody>().get(self.rig.chassis).map_or(0.0, |rb| rb.mass)
    }

    /// The chassis' height over the plane, and the bottom of its collider box over the plane, in
    /// metres — a box that reaches the plane is carrying weight the tyres should be.
    pub fn clearance(&self) -> (f32, f32) {
        let y = self.world.borrow::<Transform>().get(self.rig.chassis).map_or(0.0, |t| t.position.y);
        let size = self.rig.size;
        // The rig's own box: centre `0.12 h` up, half-height `0.30 h` (see `spawn_car`).
        (y, y + size.y * 0.12 - size.y * 0.30)
    }

    /// A steering input as the player's would arrive: scaled to the speed by
    /// [`CarRig::steer_for_speed`](super::CarRig::steer_for_speed).
    #[must_use]
    pub fn assisted(&self, steer: f32) -> f32 {
        self.rig.steer_for_speed(&self.world, steer)
    }

    /// The front wheels' steering angles, radians, as the controller applied them.
    pub fn steer_angle(&self) -> f32 {
        let vehicles = self.world.borrow::<gizmo::physics::vehicle::VehicleController>();
        vehicles
            .get(self.rig.chassis)
            .and_then(|v| v.wheels.first().map(|w| w.steering_angle))
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Limits {
        Limits { accel: vec![4.0, 4.0, 2.0, 1.0], top: 4.0, brake: 8.0, grip: 7.0 }
    }

    /// The table is read between its entries and gives nothing at the top speed — a rail that read
    /// a positive number there would go on accelerating past what the car can do.
    #[test]
    fn acceleration_is_interpolated_and_ends_at_the_top_speed() {
        let l = table();
        assert_eq!(l.accel_at(0.0), 4.0);
        assert!((l.accel_at(1.5) - 3.0).abs() < 1e-6);
        assert!((l.accel_at(2.5) - 1.5).abs() < 1e-6);
        assert_eq!(l.accel_at(4.0), 0.0);
        assert_eq!(l.accel_at(9.0), 0.0);
    }

    /// Time to a speed is the sum of the per-m/s times, part of a step included.
    #[test]
    fn time_to_a_speed_sums_the_table() {
        let l = table();
        assert!((l.time_to(2.0).unwrap() - 0.5).abs() < 1e-6);
        assert!((l.time_to(2.5).unwrap() - 0.75).abs() < 1e-6);
        assert_eq!(l.time_to(5.0), None);
    }
}
