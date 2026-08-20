//! Straight-line, flat-ground, full-throttle acceleration — the drivetrain with no course in it.
//!
//! The eight-route sweep says the field tops out around 103 km/h in third of five, and the city
//! cannot say why: a car on a race line is cornering, and cornering costs longitudinal force
//! through the friction circle, so what looks like a drivetrain ceiling may be the road. This
//! removes the road. One car, one flat box for ground, the wheel straight, the pedal down.
//!
//! The car is built from the same numbers the sim prints for a 240SX — mass, torque curve, gear
//! ratios, final drive, redline, wheel radius — rather than from `GEOMETRY.BIN`, so it needs no
//! install and cannot drift from the record without this file being edited.
//!
//! Usage: `nfs_top [seconds]`

use gizmo::physics::vehicle::{update_vehicle, Axle, VehicleController, Wheel};
use gizmo::physics::BodyHandle;
use gizmo::prelude::*;

/// The 240SX as the sim reports it. Kept beside the numbers rather than parsed, because the point
/// is to isolate the drivetrain and a parser in the loop is one more thing to be wrong.
const MASS: f32 = 1220.0;
const WHEEL_R: f32 = 0.31;
const FINAL_DRIVE: f32 = 4.083;
const REDLINE: f32 = 6500.0;
const PEAK_NM: f32 = 216.0;
/// `[reverse, neutral, 1st..5th]`, exactly as `tune_from_record` builds it.
const RATIOS: [f32; 7] = [-3.657, 0.0, 3.321, 1.902, 1.308, 1.0, 0.9];

/// The fully built gearbox: six forward gears on a 3.900 final drive.
const BUILT_RATIOS: [f32; 8] = [-3.657, 0.0, 3.321, 1.902, 1.308, 1.09, 0.92, 0.77];

/// The fully built engine's curve, same nine rpm points.
const BUILT_CURVE: [(f32, f32); 9] = [
    (800.0, 175.0),
    (1575.0, 188.0),
    (2350.0, 200.0),
    (3125.0, 225.0),
    (3900.0, 250.0),
    (4675.0, 270.0),
    (5450.0, 254.0),
    (6225.0, 213.0),
    (7000.0, 188.0),
];

/// The nine `(rpm, N·m)` points the sim prints for this car, straight off `GLOBALB`.
const CURVE: [(f32, f32); 9] = [
    (800.0, 140.0),
    (1575.0, 150.0),
    (2350.0, 160.0),
    (3125.0, 180.0),
    (3900.0, 200.0),
    (4675.0, 216.0),
    (5450.0, 203.0),
    (6225.0, 170.0),
    (7000.0, 150.0),
];

fn main() {
    let seconds: f32 = std::env::args().nth(1).and_then(|v| v.parse().ok()).unwrap_or(60.0);
    let veh = BodyHandle::from_id(2);
    let ground_id = BodyHandle::from_id(1);
    let ground = Collider::box_collider(Vec3::new(4000.0, 1.0, 4000.0));
    let ground_t = Transform::new(Vec3::new(0.0, -1.0, 0.0));

    let mut rb = RigidBody::new(MASS, true);
    rb.calculate_box_inertia(1.64, 1.13, 4.39);
    rb.center_of_mass = Vec3::new(0.0, 0.2, 0.0);
    let mut t = Transform::new(Vec3::new(0.0, 1.0, 0.0));
    let mut vel = Velocity::default();
    let mut vc = VehicleController::new();
    for (x, z, front) in [(0.8_f32, 1.3_f32, true), (-0.8, 1.3, true), (0.8, -1.3, false), (-0.8, -1.3, false)] {
        vc.add_wheel(Wheel {
            attachment_local_pos: Vec3::new(x, 0.2, z),
            radius: WHEEL_R,
            axle_type: if front { Axle::Front } else { Axle::Rear },
            is_left: x > 0.0,
            suspension_rest_length: 0.15,
            suspension_max_travel: 0.15,
            suspension_stiffness: 40000.0,
            suspension_damping: 3000.0,
            wheel_mass: 25.0,
            ..Default::default()
        });
    }
    // `NFS_ENGINE=3 NFS_GEARBOX=3` drives the car the game's own upgrade data builds: the sim's
    // `handling` line reports 270 N·m and six gears on a 3.900 final drive for a fully built
    // 240SX against the stock 216 and five on 4.083. Taken from that printout rather than re-read
    // here, so this file cannot disagree with the sim about what the car is.
    let built_engine = std::env::var("NFS_ENGINE").is_ok_and(|v| v.trim() == "3");
    let built_box = std::env::var("NFS_GEARBOX").is_ok_and(|v| v.trim() == "3");
    let ratios: Vec<f32> =
        if built_box { BUILT_RATIOS.to_vec() } else { RATIOS.to_vec() };
    vc.tuning.gear_ratios = ratios;
    vc.tuning.final_drive_ratio = if built_box { 3.900 } else { FINAL_DRIVE };
    vc.tuning.max_engine_torque = if built_engine { 270.0 } else { PEAK_NM };
    // **The car's own nine-point curve, not the controller's parabola.** `engine_torque()` falls
    // back to a bell curve peaking at 0.4 of the rev range only when `torque_curve` is empty, and
    // that fallback is *more* generous down low than this car really is (216 N·m at 2,600 rpm
    // against a measured 160). Leaving it empty measures the engine's default car, not a 240SX.
    vc.tuning.torque_curve =
        if built_engine { BUILT_CURVE.to_vec() } else { CURVE.to_vec() };
    vc.tuning.upshift_rpm = REDLINE;
    // `NFS_GRIPD` scales the tyre's Pacejka peak factor and `NFS_TORQUE` the engine's curve.
    // Two knobs because "the car is not quick enough" has two possible answers and they are
    // distinguishable only by moving one at a time: if grip binds, torque does nothing.
    let grip_d: f32 =
        std::env::var("NFS_GRIPD").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let torque_mul: f32 =
        std::env::var("NFS_TORQUE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    if (grip_d - 1.0).abs() > 1e-3 {
        for w in &mut vc.wheels {
            w.pacejka_long.d *= grip_d;
            w.pacejka_lat.d *= grip_d;
        }
    }
    if (torque_mul - 1.0).abs() > 1e-3 {
        for p in &mut vc.tuning.torque_curve {
            p.1 *= torque_mul;
        }
        vc.tuning.max_engine_torque *= torque_mul;
    }
    vc.tuning.wheelbase = 2.6;
    vc.tuning.track_width = 1.6;
    vc.current_gear = 2;
    vc.auto_shift = true;

    println!(
        "düz zemin, tam gaz, direksiyon sıfır · {MASS} kg · son sürüş {:.3} · tepe {:.0} Nm \
         · kırmızı çizgi {REDLINE:.0} · {} vites{}",
        vc.tuning.final_drive_ratio,
        vc.tuning.max_engine_torque,
        vc.tuning.gear_ratios.len() - 2,
        if built_engine || built_box { " · YÜKSELTİLMİŞ" } else { " · stok" }
    );
    println!("      t   hız km/h   vites   rpm   itiş N   ivme m/s²");

    let dt = 1.0 / 240.0;
    let gravity = Vec3::new(0.0, -9.81, 0.0);
    let steps = (seconds / dt) as usize;
    let mut last_v = 0.0f32;
    let mut next_report = 0.0f32;
    for i in 0..steps {
        let now = i as f32 * dt;
        vc.throttle_input = 1.0;
        vc.brake_input = 0.0;
        vc.steering_input = 0.0;
        vc.auto_shift_tick(dt);
        let colliders = [
            (ground_id, ground_t, ground.clone()),
            (veh, t, Collider::box_collider(Vec3::new(0.82, 0.56, 2.2))),
        ];
        update_vehicle(veh, &mut vc, &mut rb, &t, &mut vel, &colliders, 1.0, dt);
        vel.linear += gravity * dt;
        t.position += vel.linear * dt;
        t.rotation = (t.rotation * Quat::from_scaled_axis(vel.angular * dt)).normalize();
        let v = Vec3::new(vel.linear.x, 0.0, vel.linear.z).length();
        if now >= next_report {
            let torque = vc.engine_torque()
                * vc.tuning.gear_ratios.get(vc.current_gear).copied().unwrap_or(0.0).abs()
                * FINAL_DRIVE;
            println!(
                "   {now:>4.0}   {:>8.1}   {:>5}   {:>4.0}   {:>6.0}   {:>9.2}",
                v * 3.6,
                vc.current_gear,
                vc.engine_rpm,
                torque / WHEEL_R,
                (v - last_v) / 1.0_f32.max(1e-3)
            );
            if now < 3.0 {
                let (down, all) = (
                    vc.wheels.iter().filter(|w| w.is_grounded).count(),
                    vc.wheels.len(),
                );
                println!("        (y={:.2} · yerde {down}/{all} tekerlek)", t.position.y);
            }
            last_v = v;
            next_report = now + 1.0;
        }
    }
}
