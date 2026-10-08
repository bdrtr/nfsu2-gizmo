//! # How the car handles, measured
//!
//! "It slides" is a feeling; this turns it into numbers on the dyno's own flat plane
//! ([`nfsu2::rig::dyno::Bench`]): how much grip the tyres hold at each speed and steering input,
//! how far the car's path parts from its nose — sideslip, the thing a driver feels as sliding — and
//! what a keyboard tap at speed does to it.
//!
//! ```bash
//! cargo run --release -p nfsu2 --bin nfs_bench -- "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
//! ```
//!
//! Three runs, each from rest on a fresh car:
//!
//! 1. **Steady circles**: a speed held by throttle, a fixed steering input, and once settled the
//!    lateral acceleration, the radius and the sideslip.
//! 2. **A keyboard tap**: straight at a speed, then the left key held for half a second the way
//!    [`nfsu2::rig::Driver`] ramps it (6/s to full lock) and let go (springing back at 15/s) — the
//!    largest sideslip and yaw rate, and whether the car spins.
//! 3. **Full throttle from rest**, wheel straight: the largest sideslip, which on a rear-driven car
//!    is the tail stepping out.

use gizmo::prelude::*;
use gizmo::renderer::Renderer;
use nfsu2::rig::dyno::Bench;
use nfsu2::rig::{Controls, FIXED_DT};

fn main() {
    pollster::block_on(run());
}

/// Throttle and brake that hold `want` m/s.
fn hold(bench: &Bench, want: f32) -> (f32, f32) {
    let v = bench.forward_speed();
    let throttle = ((want - v) * 0.6 + 0.25).clamp(0.0, 1.0);
    let brake = if v > want + 1.5 { 0.3 } else { 0.0 };
    (if v > want + 0.5 { 0.0 } else { throttle }, brake)
}

/// Bring a fresh car up to `speed` in a straight line.
fn up_to(renderer: &Renderer, assets: &mut AssetManager, car: &str, speed: f32) -> Bench {
    let mut b = Bench::new(renderer, assets, car);
    b.run(2.0, |_, _| Controls::default());
    let start = b.t;
    while b.forward_speed() < speed - 0.3 && b.t - start < 60.0 {
        b.step(&Controls { throttle: 1.0, ..Controls::default() });
    }
    b
}

async fn run() {
    let car = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("NFSU2_ROOT").ok().map(|r| format!("{r}/CARS/240SX/GEOMETRY.BIN")))
        .expect("no car given and NFSU2_ROOT is unset");
    assert!(Renderer::headless_adapter_available().await, "no GPU adapter for a headless run");
    let renderer = Renderer::new_headless(64, 64, None).await;
    let mut assets = AssetManager::new();

    // At rest: does the weight sit on the tyres?
    {
        let mut b = Bench::new(&renderer, &mut assets, &car);
        b.run(3.0, |_, _| Controls::default());
        let w = b.wheels();
        let load: f32 = w.iter().map(|x| x.1).sum();
        let (y, bottom) = b.clearance();
        println!(
            "== duruş: kütle {:.0} kg · tekerlek yükü {:.0} N / ağırlık {:.0} N = {:.2} · yerde {} / {} · sürtünme {:?} · şasi y {y:.2}, kutu altı {bottom:.2}",
            b.mass(),
            load,
            b.mass() * 9.81,
            load / (b.mass() * 9.81),
            w.iter().filter(|x| x.0).count(),
            w.len(),
            w.iter().map(|x| x.2).collect::<Vec<_>>(),
        );
        // And in a steady 70 km/h circle at half input.
        let want = 70.0 / 3.6;
        let mut b = up_to(&renderer, &mut assets, &car, want);
        for _ in 0..(6.0 / FIXED_DT) as usize {
            let (throttle, brake) = hold(&b, want);
            b.step(&Controls { throttle, brake, steer: 0.5, ..Controls::default() });
        }
        let w = b.wheels();
        let load: f32 = w.iter().map(|x| x.1).sum();
        println!(
            "   70 km/h dairede: yükler {:?} N · toplam/ağırlık {:.2} · yanal {:.1} m/s²",
            w.iter().map(|x| x.1.round()).collect::<Vec<_>>(),
            load / (b.mass() * 9.81),
            b.lateral().abs()
        );
    }

    println!("== sabit daire: hız · direksiyon girdisi → tekerlek açısı · yanal ivme · yarıçap · kayma açısı");
    for kmh in [40.0f32, 70.0, 100.0] {
        for steer in [0.1f32, 0.25, 0.5, 1.0] {
            let want = kmh / 3.6;
            let mut b = up_to(&renderer, &mut assets, &car, want);
            let mut spun = false;
            let mut acc = (0.0f32, 0.0f32, 0.0f32, 0usize);
            for i in 0..(8.0 / FIXED_DT) as usize {
                let (throttle, brake) = hold(&b, want);
                b.step(&Controls { throttle, brake, steer, ..Controls::default() });
                if b.sideslip().abs() > 0.5 {
                    spun = true;
                    break;
                }
                // The last three seconds, once the circle has settled.
                if i as f32 * FIXED_DT > 5.0 {
                    acc.0 += b.lateral().abs();
                    acc.1 += b.sideslip().abs();
                    acc.2 += b.speed();
                    acc.3 += 1;
                }
            }
            if spun || acc.3 == 0 {
                println!("   {kmh:>4.0} km/h · girdi {steer:.2} · DÖNDÜ (kayma > 29°)");
                continue;
            }
            let n = acc.3 as f32;
            let (lat, slip, v) = (acc.0 / n, acc.1 / n, acc.2 / n);
            println!(
                "   {kmh:>4.0} km/h · girdi {steer:.2} → {:>4.1}° · {lat:>4.1} m/s² ({:.2} g) · R {:>5.0} m · kayma {:>4.1}° · {:>3.0} km/h",
                b.steer_angle().to_degrees(),
                lat / 9.81,
                v * v / lat.max(1e-3),
                slip.to_degrees(),
                v * 3.6
            );
        }
    }

    println!("== klavye dokunuşu: hız → en büyük kayma · en büyük sapma hızı · sonuç");
    for kmh in [60.0f32, 90.0, 120.0] {
        let want = kmh / 3.6;
        let mut b = up_to(&renderer, &mut assets, &car, want);
        let mut steer = 0.0f32;
        let (mut slip, mut yaw) = (0.0f32, 0.0f32);
        let t0 = b.t;
        while b.t - t0 < 4.0 {
            let held = b.t - t0 < 0.5;
            steer = if held { (steer + 6.0 * FIXED_DT).min(1.0) } else { steer * (-15.0 * FIXED_DT).exp() };
            let (throttle, _) = hold(&b, want);
            b.step(&Controls { throttle, steer, ..Controls::default() });
            slip = slip.max(b.sideslip().abs());
            yaw = yaw.max(b.yaw_rate().abs());
        }
        let heading_kept = b.sideslip().abs() < 0.05;
        println!(
            "   {kmh:>4.0} km/h · kayma {:>4.1}° · sapma {:>5.1}°/s · {}",
            slip.to_degrees(),
            yaw.to_degrees(),
            if slip > 0.5 { "DÖNDÜ" } else if heading_kept { "toparladı" } else { "hâlâ kayıyor" }
        );
    }

    println!("== hıza duyarlı direksiyonla: tam tuş, sabit daire · ve klavye dokunuşu");
    for kmh in [40.0f32, 70.0, 100.0, 130.0] {
        let want = kmh / 3.6;
        let mut b = up_to(&renderer, &mut assets, &car, want);
        let mut acc = (0.0f32, 0usize);
        let mut spun = false;
        for i in 0..(8.0 / FIXED_DT) as usize {
            let (throttle, brake) = hold(&b, want);
            let steer = b.assisted(1.0);
            b.step(&Controls { throttle, brake, steer, ..Controls::default() });
            if b.sideslip().abs() > 0.5 {
                spun = true;
                break;
            }
            if i as f32 * FIXED_DT > 5.0 {
                acc.0 += b.lateral().abs();
                acc.1 += 1;
            }
        }
        let circle = if spun { "DÖNDÜ".to_string() } else {
            let lat = acc.0 / acc.1.max(1) as f32;
            format!("tam tuş → {:>4.1}° · {lat:>4.1} m/s² ({:.2} g)", b.steer_angle().to_degrees(), lat / 9.81)
        };
        let mut b = up_to(&renderer, &mut assets, &car, want);
        let mut key = 0.0f32;
        let (mut slip, mut yaw) = (0.0f32, 0.0f32);
        let t0 = b.t;
        while b.t - t0 < 4.0 {
            key = if b.t - t0 < 0.5 { (key + 6.0 * FIXED_DT).min(1.0) } else { key * (-15.0 * FIXED_DT).exp() };
            let (throttle, _) = hold(&b, want);
            let steer = b.assisted(key);
            b.step(&Controls { throttle, steer, ..Controls::default() });
            slip = slip.max(b.sideslip().abs());
            yaw = yaw.max(b.yaw_rate().abs());
        }
        println!(
            "   {kmh:>4.0} km/h · {circle} · dokunuş: kayma {:.1}°, sapma {:.1}°/s",
            slip.to_degrees(),
            yaw.to_degrees()
        );
    }

    println!("== tam gaz kalkış, direksiyon düz");
    let mut b = Bench::new(&renderer, &mut assets, &car);
    b.run(2.0, |_, _| Controls::default());
    let mut slip = 0.0f32;
    let t0 = b.t;
    while b.t - t0 < 15.0 {
        b.step(&Controls { throttle: 1.0, ..Controls::default() });
        slip = slip.max(b.sideslip().abs());
    }
    println!("   15 s sonra {:.0} km/h · en büyük kayma {:.2}°", b.forward_speed() * 3.6, slip.to_degrees());
}
