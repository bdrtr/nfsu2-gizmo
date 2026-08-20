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
    vc.tuning.gear_ratios = RATIOS.to_vec();
    vc.tuning.final_drive_ratio = FINAL_DRIVE;
    vc.tuning.max_engine_torque = PEAK_NM;
    vc.tuning.upshift_rpm = REDLINE;
    vc.tuning.wheelbase = 2.6;
    vc.tuning.track_width = 1.6;
    vc.current_gear = 2;
    vc.auto_shift = true;

    println!(
        "düz zemin, tam gaz, direksiyon sıfır · {MASS} kg · son sürüş {FINAL_DRIVE} · \
         tepe {PEAK_NM} Nm · kırmızı çizgi {REDLINE:.0} · oranlar {RATIOS:?}"
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
            let torque = PEAK_NM
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
