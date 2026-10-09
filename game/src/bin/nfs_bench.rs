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
//! Six runs, each from rest on a fresh car:
//!
//! 1. **At rest, and in a 70 km/h circle**: whether the weight sits on the tyres — the wheel loads
//!    against the car's weight, the friction each wheel reads, how high the chassis box rides.
//! 2. **Steady circles**: a speed held by throttle, a fixed steering input, and once settled the
//!    steering angle, the lateral acceleration, the radius and the sideslip.
//! 3. **A keyboard tap**: straight at a speed, then the left key held for half a second the way
//!    [`nfsu2::rig::Driver`] ramps it (6/s to full lock) and let go (springing back at 15/s) — the
//!    largest sideslip and yaw rate, and whether the car spins.
//! 4. **The same through the steering assist** ([`nfsu2::rig::CarRig::steer_for_speed`]): a full
//!    key held in a steady circle, and a tap.
//! 5. **Full throttle from rest**, wheel straight: the largest sideslip, which on a rear-driven car
//!    is the tail stepping out.
//! 6. **Mirror images**: a few circles and a tap driven to the left and to the right. The car is
//!    symmetric, so the right-hand run must read as the left-hand one with its signs flipped and
//!    its wheel loads swapped side for side; anything else is a wheel on the wrong side, a sign
//!    the wrong way round, or a mount the file has off-centre.
//!
//! Every other turn is to the left.

use gizmo::prelude::*;
use gizmo::renderer::Renderer;
use nfsu2::rig::dyno::Bench;
use nfsu2::rig::{Controls, FIXED_DT};

/// Sideslip past which the car has spun rather than slid, radians (29°).
const SPUN: f32 = 0.5;

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

/// Bring a fresh car up to `speed` in a straight line — or `None`, said aloud, if a minute of full
/// throttle does not get it there.
///
/// Such a car is not measured. Run from what it reached, the line would carry the speed asked
/// for, and [`hold`] would keep the throttle down trying to get there: a rear-driven car spins
/// on power in every circle, which reads as a handling fault it does not have.
fn up_to(renderer: &Renderer, assets: &mut AssetManager, car: &str, speed: f32) -> Option<Bench> {
    let mut b = Bench::new(renderer, assets, car);
    b.run(2.0, |_, _| Controls::default());
    let start = b.t;
    while b.forward_speed() < speed - 0.3 && b.t - start < 60.0 {
        b.step(&Controls { throttle: 1.0, ..Controls::default() });
    }
    if b.forward_speed() < speed - 0.3 {
        println!(
            "   {:>4.0} km/h · çıkamadı: 60 s tam gazda {:.0} km/h — ölçülmedi",
            speed * 3.6,
            b.forward_speed() * 3.6
        );
        return None;
    }
    Some(b)
}

/// How a manoeuvre ended.
enum Outcome<T> {
    /// The car never got to the speed; [`up_to`] has said so.
    Unreached,
    /// Sideslip passed [`SPUN`].
    Spun,
    Done(T),
}

/// What a steady circle settled to, over its last three seconds. Signed, so a right-hand circle
/// can be read against a left-hand one.
struct Circle {
    /// Lateral acceleration, m/s², positive to the left.
    lat: f32,
    /// Sideslip, radians, positive when the car travels to the right of its nose.
    slip: f32,
    /// Speed in plan, m/s.
    speed: f32,
    /// The steering angle, radians, positive to the left.
    angle: f32,
    /// Each wheel's suspension force at the end, newtons, in the rig's order — the front pair
    /// first.
    loads: Vec<f32>,
    /// The car's weight, newtons.
    weight: f32,
}

/// Hold `kmh` for eight seconds, steering with what `steer` asks for at each step.
fn circle(
    renderer: &Renderer,
    assets: &mut AssetManager,
    car: &str,
    kmh: f32,
    steer: impl Fn(&Bench) -> f32,
) -> Outcome<Circle> {
    let want = kmh / 3.6;
    let Some(mut b) = up_to(renderer, assets, car, want) else { return Outcome::Unreached };
    let mut acc = (0.0f32, 0.0f32, 0.0f32, 0usize);
    for i in 0..(8.0 / FIXED_DT) as usize {
        let (throttle, brake) = hold(&b, want);
        let steer = steer(&b);
        b.step(&Controls { throttle, brake, steer, ..Controls::default() });
        if b.sideslip().abs() > SPUN {
            return Outcome::Spun;
        }
        // The last three seconds, once the circle has settled.
        if i as f32 * FIXED_DT > 5.0 {
            acc.0 += b.lateral();
            acc.1 += b.sideslip();
            acc.2 += b.speed();
            acc.3 += 1;
        }
    }
    let n = acc.3.max(1) as f32;
    Outcome::Done(Circle {
        lat: acc.0 / n,
        slip: acc.1 / n,
        speed: acc.2 / n,
        angle: b.steer_angle(),
        loads: b.wheels().iter().map(|w| w.1).collect(),
        weight: b.mass() * 9.81,
    })
}

/// What a keyboard tap did: the sideslip and the yaw rate furthest from zero, signed, and the
/// sideslip at the end.
struct Tap {
    /// Radians, the sign of [`Bench::sideslip`].
    slip: f32,
    /// Radians per second, positive to the left.
    yaw: f32,
    /// Sideslip four seconds after the key went down, radians.
    after: f32,
}

impl Tap {
    fn verdict(&self) -> &'static str {
        if self.slip.abs() > SPUN {
            "DÖNDÜ"
        } else if self.after.abs() < 0.05 {
            "toparladı"
        } else {
            "hâlâ kayıyor"
        }
    }
}

/// Straight at `kmh`, then the key held towards `side` (+1 left, −1 right) for half a second the
/// way [`nfsu2::rig::Driver`] ramps it, and let go — through the steering assist if `assisted`.
fn tap(
    renderer: &Renderer,
    assets: &mut AssetManager,
    car: &str,
    kmh: f32,
    side: f32,
    assisted: bool,
) -> Option<Tap> {
    let want = kmh / 3.6;
    let mut b = up_to(renderer, assets, car, want)?;
    let mut key = 0.0f32;
    let (mut slip, mut yaw) = (0.0f32, 0.0f32);
    let t0 = b.t;
    while b.t - t0 < 4.0 {
        key = if b.t - t0 < 0.5 { (key + 6.0 * FIXED_DT).min(1.0) } else { key * (-15.0 * FIXED_DT).exp() };
        let (throttle, _) = hold(&b, want);
        let steer = if assisted { b.assisted(side * key) } else { side * key };
        b.step(&Controls { throttle, steer, ..Controls::default() });
        if b.sideslip().abs() > slip.abs() {
            slip = b.sideslip();
        }
        if b.yaw_rate().abs() > yaw.abs() {
            yaw = b.yaw_rate();
        }
    }
    Some(Tap { slip, yaw, after: b.sideslip() })
}

/// How far `b` is from mirroring `a`, as a fraction of `a`: zero when `b = −a`.
fn unmirrored(a: f32, b: f32) -> f32 {
    (a + b).abs() / a.abs().max(1e-6)
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
        b.announce();
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
    }
    // And in a steady 70 km/h circle at half input.
    match circle(&renderer, &mut assets, &car, 70.0, |_| 0.5) {
        Outcome::Unreached => {}
        Outcome::Spun => println!("   70 km/h dairede: DÖNDÜ (kayma > 29°)"),
        Outcome::Done(c) => println!(
            "   70 km/h dairede: yükler {:?} N · toplam/ağırlık {:.2} · yanal {:.1} m/s²",
            c.loads.iter().map(|x| x.round()).collect::<Vec<_>>(),
            c.loads.iter().sum::<f32>() / c.weight,
            c.lat.abs()
        ),
    }

    println!("== sabit daire: hız · direksiyon girdisi → direksiyon açısı · yanal ivme · yarıçap · kayma açısı");
    for kmh in [40.0f32, 70.0, 100.0] {
        for steer in [0.1f32, 0.25, 0.5, 1.0] {
            match circle(&renderer, &mut assets, &car, kmh, |_| steer) {
                Outcome::Unreached => {}
                Outcome::Spun => println!("   {kmh:>4.0} km/h · girdi {steer:.2} · DÖNDÜ (kayma > 29°)"),
                Outcome::Done(c) => {
                    let lat = c.lat.abs();
                    println!(
                        "   {kmh:>4.0} km/h · girdi {steer:.2} → {:>4.1}° · {lat:>4.1} m/s² ({:.2} g) · R {:>5.0} m · kayma {:>4.1}° · {:>3.0} km/h",
                        c.angle.to_degrees(),
                        lat / 9.81,
                        c.speed * c.speed / lat.max(1e-3),
                        c.slip.abs().to_degrees(),
                        c.speed * 3.6
                    );
                }
            }
        }
    }

    println!("== klavye dokunuşu: hız → en büyük kayma · en büyük sapma hızı · sonuç");
    for kmh in [60.0f32, 90.0, 120.0] {
        let Some(t) = tap(&renderer, &mut assets, &car, kmh, 1.0, false) else { continue };
        println!(
            "   {kmh:>4.0} km/h · kayma {:>4.1}° · sapma {:>5.1}°/s · {}",
            t.slip.abs().to_degrees(),
            t.yaw.abs().to_degrees(),
            t.verdict()
        );
    }

    println!("== hıza duyarlı direksiyonla: tam tuş, sabit daire · ve klavye dokunuşu");
    for kmh in [40.0f32, 70.0, 100.0, 130.0] {
        let circle = match circle(&renderer, &mut assets, &car, kmh, |b| b.assisted(1.0)) {
            Outcome::Unreached => continue,
            Outcome::Spun => "DÖNDÜ".to_string(),
            Outcome::Done(c) => {
                let lat = c.lat.abs();
                format!("tam tuş → {:>4.1}° · {lat:>4.1} m/s² ({:.2} g)", c.angle.to_degrees(), lat / 9.81)
            }
        };
        let Some(t) = tap(&renderer, &mut assets, &car, kmh, 1.0, true) else { continue };
        println!(
            "   {kmh:>4.0} km/h · {circle} · dokunuş: kayma {:.1}°, sapma {:.1}°/s",
            t.slip.abs().to_degrees(),
            t.yaw.abs().to_degrees()
        );
    }

    println!("== tam gaz kalkış, direksiyon düz");
    {
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

    // A mirror image should differ only by rounding. A few per cent is a mount the file has off
    // centre; more is something on the wrong side.
    const MIRROR_TOL: f32 = 0.03;
    println!("== ayna: aynı manevra sola (+) ve sağa (−) · sağdaki soldakinin aynası olmalı");
    for (kmh, steer) in [(40.0f32, 1.0f32), (70.0, 0.25), (70.0, 0.5), (100.0, 0.5)] {
        let left = circle(&renderer, &mut assets, &car, kmh, |_| steer);
        let right = circle(&renderer, &mut assets, &car, kmh, |_| -steer);
        let (Outcome::Done(l), Outcome::Done(r)) = (&left, &right) else {
            let says = |o: &Outcome<Circle>| match o {
                Outcome::Unreached => "çıkamadı",
                Outcome::Spun => "DÖNDÜ",
                Outcome::Done(_) => "tuttu",
            };
            println!("   {kmh:>4.0} km/h · girdi ±{steer:.2} · sol {} · sağ {}", says(&left), says(&right));
            continue;
        };
        // Swapped side for side within each axle — the front pair, then the rear — by magnitude, so
        // the check holds whatever order the record lists an axle's wheels in.
        let pair = |w: &[f32], i: usize| (w[i].min(w[i + 1]), w[i].max(w[i + 1]));
        let mut loads = 0.0f32;
        for i in [0, 2] {
            let (a, b) = (pair(&l.loads, i), pair(&r.loads, i));
            loads = loads.max((a.0 - b.0).abs().max((a.1 - b.1).abs()) / a.1.max(1.0));
        }
        let off = unmirrored(l.lat, r.lat).max(unmirrored(l.angle, r.angle)).max(loads);
        println!(
            "   {kmh:>4.0} km/h · girdi ±{steer:.2} · yanal {:+.2} / {:+.2} m/s² · kayma {:+.2}° / {:+.2}° · yükler %{:.1} · {}",
            l.lat,
            r.lat,
            l.slip.to_degrees(),
            r.slip.to_degrees(),
            loads * 100.0,
            if off <= MIRROR_TOL { "ayna" } else { "AYNA DEĞİL" }
        );
    }
    for kmh in [60.0f32, 90.0] {
        let (Some(l), Some(r)) = (
            tap(&renderer, &mut assets, &car, kmh, 1.0, false),
            tap(&renderer, &mut assets, &car, kmh, -1.0, false),
        ) else {
            continue;
        };
        let off = unmirrored(l.slip, r.slip).max(unmirrored(l.yaw, r.yaw));
        println!(
            "   {kmh:>4.0} km/h dokunuş · kayma {:+.2}° / {:+.2}° · sapma {:+.1} / {:+.1}°/s · {} / {} · {}",
            l.slip.to_degrees(),
            r.slip.to_degrees(),
            l.yaw.to_degrees(),
            r.yaw.to_degrees(),
            l.verdict(),
            r.verdict(),
            if off <= MIRROR_TOL { "ayna" } else { "AYNA DEĞİL" }
        );
    }
}
