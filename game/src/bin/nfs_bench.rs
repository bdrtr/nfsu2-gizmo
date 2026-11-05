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
    // `NFS_BENCH_TRACE=1`: what the drivetrain delivers on the way up, twice a second — the line
    // that says whether a car that will not move is given no drive or cannot use it.
    let trace = std::env::var("NFS_BENCH_TRACE").is_ok();
    let mut next = start;
    while b.forward_speed() < speed - 0.3 && b.t - start < 60.0 {
        b.step(&Controls { throttle: 1.0, ..Controls::default() });
        if trace && b.t >= next && b.t - start < 6.0 {
            next += 0.5;
            let (gear, rpm, torque, spin) = b.drivetrain();
            println!(
                "   iz t={:.1} · {:.1} km/h · vites {gear} · {rpm:.0} rpm · tork {torque:.0} N·m · dönüş {spin:.1} rad/s · yerde {}/4",
                b.t - start,
                b.forward_speed() * 3.6,
                b.wheels().iter().filter(|w| w.0).count()
            );
        }
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
    /// Each wheel's suspension force over the same last three seconds, newtons, in the rig's
    /// order — the front pair first. Averaged like the rest: a single sample at the end caught the
    /// suspension mid-oscillation, which read as a side-to-side difference of several per cent.
    loads: Vec<f32>,
    /// The car's weight, newtons.
    weight: f32,
    /// Body roll over the last three seconds, radians, positive leaning right
    /// ([`Bench::attitude`]).
    roll: f32,
    /// Each strut's compression at the end, metres, in the rig's order.
    travel: Vec<f32>,
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
    let mut roll = 0.0f32;
    let mut loads = vec![0.0f32; 4];
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
            roll += b.attitude().0;
            for (sum, w) in loads.iter_mut().zip(b.wheels()) {
                *sum += w.1;
            }
        }
    }
    let n = acc.3.max(1) as f32;
    Outcome::Done(Circle {
        lat: acc.0 / n,
        slip: acc.1 / n,
        speed: acc.2 / n,
        angle: b.steer_angle(),
        loads: loads.iter().map(|x| x / n).collect(),
        weight: b.mass() * 9.81,
        roll: roll / n,
        travel: b.travel(),
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

/// A steady corner at `kmh` through the player's steering (assist and catch, a full key held), then
/// the throttle floored for 2.5 s with the key still held.
fn power(renderer: &Renderer, assets: &mut AssetManager, car: &str, kmh: f32) {
    let want = kmh / 3.6;
    let Some(mut b) = up_to(renderer, assets, car, want) else {
        println!("   {kmh:>4.0} km/h: hıza çıkamadı");
        return;
    };
    let mut key = 0.0f32;
    let t0 = b.t;
    let (mut corner, mut peak, mut yaw, mut spun) = (0.0f32, 0.0f32, 0.0f32, false);
    while b.t - t0 < 6.5 {
        let t = b.t - t0;
        key = (key + 6.0 * FIXED_DT).min(1.0);
        let throttle = if t < 4.0 { hold(&b, want).0 } else { 1.0 };
        let steer = b.caught(key);
        b.step(&Controls { throttle, steer, ..Controls::default() });
        let slip = b.sideslip().to_degrees().abs();
        if t < 4.0 {
            corner = slip;
        } else {
            peak = peak.max(slip);
            yaw = yaw.max(b.yaw_rate().abs().to_degrees());
            spun |= slip > 60.0;
        }
    }
    println!(
        "   {kmh:>4.0} km/h · viraj {corner:>4.1}° · en çok {peak:>5.1}° · yalpa {yaw:>5.1}°/s · {} · {:>3.0} km/h",
        if spun { "DÖNDÜ" } else { "     " },
        b.speed() * 3.6
    );
}

/// What the driver does once the handbrake has broken the tail loose.
#[derive(Clone, Copy, PartialEq)]
enum Catch {
    /// Lets go of the key.
    LetGo,
    /// Steers into the slide through the assist alone (`NFS_CATCH=0`'s game).
    Bare,
    /// Steers into the slide through the assist and the game's counter-steer.
    Game,
    /// Keeps the left key held.
    Hold,
}

/// Straight at `kmh`, then half a second of handbrake with the left key ramping in, then four
/// seconds of three-quarter throttle and `way`. Steering into the slide is the key towards the side
/// the car is travelling to — ramped like [`nfsu2::rig::Driver`]'s — let go once the slide is under
/// 3°.
fn drift(renderer: &Renderer, assets: &mut AssetManager, car: &str, kmh: f32, name: &str, way: Catch) {
    let Some(mut b) = up_to(renderer, assets, car, kmh / 3.6) else {
        println!("   {kmh:>4.0} km/h · {name}: hıza çıkamadı");
        return;
    };
    let mut key = 0.0f32;
    let t0 = b.t;
    let (mut peak, mut at, mut settled, mut spun) = (0.0f32, [0.0f32; 3], None, false);
    while b.t - t0 < 4.5 {
        let t = b.t - t0;
        let slip = b.sideslip();
        let (target, throttle) = if t < 0.5 {
            (1.0, 0.3)
        } else {
            let into = if slip.to_degrees().abs() > 3.0 { -slip.signum() } else { 0.0 };
            let target = match way {
                Catch::LetGo => 0.0,
                Catch::Bare | Catch::Game => into,
                Catch::Hold => 1.0,
            };
            (target, 0.75)
        };
        key = if target == 0.0 {
            key * (-15.0 * FIXED_DT).exp()
        } else {
            (key + (target - key).clamp(-6.0 * FIXED_DT, 6.0 * FIXED_DT)).clamp(-1.0, 1.0)
        };
        b.handbrake(t < 0.5);
        let steer = if way == Catch::Game { b.caught(key) } else { b.assisted(key) };
        b.step(&Controls { throttle, steer, ..Controls::default() });
        let slip = b.sideslip().to_degrees().abs();
        if t >= 0.5 {
            peak = peak.max(slip);
            spun |= slip > 90.0;
            if settled.is_none() && peak > 5.0 && slip < 5.0 {
                settled = Some(t - 0.5);
            }
        }
        for (i, s) in [1.5f32, 2.5, 3.5].iter().enumerate() {
            if (t - s).abs() < FIXED_DT * 0.5 {
                at[i] = slip;
            }
        }
    }
    let style = b.style().map_or(String::new(), |s| {
        format!(" · stil {:.0}{}", s.total, s.last.map_or(String::new(), |(n, p, _)| format!(" ({n} +{p:.0})")))
    });
    println!(
        "   {kmh:>4.0} km/h · {name:<17} · en çok {peak:>5.1}° · {:>5.1}° {:>5.1}° {:>5.1}° · iniş {} · {} · {:>3.0} km/h{style}",
        at[0],
        at[1],
        at[2],
        settled.map_or("  —  ".into(), |s| format!("{s:>4.1} s")),
        if spun { "DÖNDÜ" } else { "     " },
        b.speed() * 3.6
    );
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

    // `NFS_BENCH=crash`: only the collision test — the player coasting into the back of a rival on
    // rails, resolved the game's way and, beside it, the old way (the rail a kinematic wall in the
    // engine's solver).
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "mirror") {
        mirror(&renderer, &mut assets, &car);
        return;
    }
    // `NFS_BENCH=kerb`: a kerb struck at speed and at an angle, coasting — whether the car rolls.
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "kerb") {
        println!("== kaldırım: yükseklik · hız · açı → en büyük yalpa · yalpa hızı · havada · en yüksek sıçrama · ters döndü mü");
        for height in [0.12f32, 0.2, 0.3] {
            for speed in [15.0f32, 25.0] {
                for angle in [30.0f32, 60.0, 75.0] {
                    kerb(&renderer, &mut assets, &car, height, speed, angle);
                }
            }
        }
        return;
    }
    // `NFS_BENCH=trip`: sliding sideways into a kerb — the tripped roll-over.
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "trip") {
        println!("== yana kayarak kaldırıma: yükseklik · yan hız → en büyük yalpa · yalpa hızı · ters döndü mü");
        for height in [0.12f32, 0.2, 0.3] {
            for lateral in [4.0f32, 7.0, 10.0, 14.0] {
                trip(&renderer, &mut assets, &car, height, lateral);
            }
        }
        return;
    }
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "wall") {
        println!("== duvar: hız · açı → sekme (duvara dik hız, önce → sonra) · duvar boyunca kalan · en büyük sapma · en büyük yalpa · ters döndü mü");
        for speed in [15.0f32, 25.0] {
            for angle in [0.0f32, 30.0, 60.0] {
                wall(&renderer, &mut assets, &car, speed, angle);
            }
        }
        return;
    }
    // `NFS_BENCH=steer`: full key in a steady circle, three ways — the bare lock, our speed-scaled
    // assist, and the game's own tables (`rig::steer`, at the throttle the circle holds).
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "steer") {
        println!("== direksiyon: tam tuş, sabit daire · açı · yanal ivme · kayma");
        for kmh in [40.0f32, 70.0, 100.0, 130.0, 160.0] {
            let mut row = format!("   {kmh:>4.0} km/h");
            for (name, way) in [("çıplak", 0u8), ("yardım", 1), ("oyunun", 2)] {
                let o = circle(&renderer, &mut assets, &car, kmh, |b| match way {
                    0 => 1.0,
                    1 => b.assisted(1.0),
                    _ => b.game_steered(1.0, hold(b, kmh / 3.6).0, 0.0),
                });
                row += &match o {
                    Outcome::Unreached => format!(" · {name}: —"),
                    Outcome::Spun => format!(" · {name}: DÖNDÜ"),
                    Outcome::Done(c) => format!(
                        " · {name}: {:>4.1}° {:>4.1} m/s² kayma {:>4.1}°",
                        c.angle.to_degrees(),
                        c.lat.abs(),
                        c.slip.abs().to_degrees()
                    ),
                };
            }
            println!("{row}");
        }
        return;
    }
    // `NFS_BENCH=power`: a steady corner, then the throttle floored — whether the tail steps out.
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "power") {
        println!("== gazla çıkış: sabit virajda tam tuş, sonra 2,5 s tam gaz · viraj kayması · en büyük kayma · en büyük yalpa · dönüş · son hız");
        for kmh in [40.0f32, 60.0, 90.0] {
            power(&renderer, &mut assets, &car, kmh);
        }
        return;
    }
    // `NFS_BENCH=drift`: a slide provoked with the handbrake, then caught (or not) four ways.
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "drift") {
        println!("== drift: 0,5 s el freni + sol tuş, sonra · en büyük kayma · 1/2/3 s'deki kayma · 5°'ye iniş · dönüş · son hız");
        for kmh in [70.0f32, 90.0, 110.0] {
            for (name, way) in [("bırak", Catch::LetGo), ("karşı, yardımsız", Catch::Bare), ("karşı, oyunun", Catch::Game), ("sola tut", Catch::Hold)] {
                drift(&renderer, &mut assets, &car, kmh, name, way);
            }
        }
        return;
    }
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "launch") {
        launch(&renderer, &mut assets, &car);
        return;
    }
    if std::env::var("NFS_BENCH").is_ok_and(|v| v == "crash") {
        println!("== çarpışma: oyuncu → rakip · hızlar m/s önce → sonra · temas süresi · en derin · oyuncunun en düşük hızı · momentum korunan · en büyük sapma");
        for (player, rail) in [(20.0f32, 15.0f32), (25.0, 10.0), (15.0, 0.0)] {
            for solver in [true, false] {
                let Some(c) = Bench::crash(&renderer, &mut assets, &car, player, rail, 3.0, solver) else { continue };
                println!(
                    "   {:<7} oyuncu {:>5.1} → {:>5.1} · rakip {:>5.1} → {:>5.1} · {:.2} s · {:.2} m · en düşük {:>5.1} · momentum %{:.0} · sapma {:.2} rad/s",
                    if solver { "eski" } else { "oyunun" },
                    c.player_before,
                    c.player_after,
                    c.rail_before,
                    c.rail_after,
                    c.touching,
                    c.overlap,
                    c.player_lowest,
                    c.momentum_kept() * 100.0,
                    c.yaw,
                );
            }
        }
        return;
    }

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
        // Where the eye sees the wheels: at the mount (the old visual) or at the end of the strut.
        let cm = |v: &[f32]| v.iter().map(|x| (x * 1000.0).round() / 10.0).collect::<Vec<_>>();
        let bottoms = b.wheel_bottoms();
        println!(
            "   tekerlek tabanı düzlemin üstünde, cm: bağlantıda {:?} · şerit ucunda {:?} · sıkışma {:?} (raydaki araba için hesaplanan {:?}) · yalpa {:.2}° yunuslama {:.2}°",
            cm(&bottoms.iter().map(|x| x.0).collect::<Vec<_>>()),
            cm(&bottoms.iter().map(|x| x.1).collect::<Vec<_>>()),
            cm(&b.travel()),
            cm(&b.settle()),
            b.attitude().0.to_degrees(),
            b.attitude().1.to_degrees(),
        );
    }
    // And in a steady 70 km/h circle at half input.
    match circle(&renderer, &mut assets, &car, 70.0, |_| 0.5) {
        Outcome::Unreached => {}
        Outcome::Spun => println!("   70 km/h dairede: DÖNDÜ (kayma > 29°)"),
        Outcome::Done(c) => println!(
            "   70 km/h dairede: yükler {:?} N · toplam/ağırlık {:.2} · yanal {:.1} m/s² · yalpa {:.2}° · sıkışma {:?} cm",
            c.loads.iter().map(|x| x.round()).collect::<Vec<_>>(),
            c.loads.iter().sum::<f32>() / c.weight,
            c.lat.abs(),
            c.roll.to_degrees(),
            c.travel.iter().map(|x| (x * 1000.0).round() / 10.0).collect::<Vec<_>>(),
        ),
    }
    // Pitch: full brake from 100 km/h and full throttle from rest, the most the nose dips and
    // squats, and how far the struts move doing it.
    if let Some(mut b) = up_to(&renderer, &mut assets, &car, 100.0 / 3.6) {
        let (mut pitch, mut front) = (0.0f32, 0.0f32);
        let t0 = b.t;
        while b.forward_speed() > 1.0 && b.t - t0 < 10.0 {
            b.step(&Controls { brake: 1.0, ..Controls::default() });
            pitch = pitch.max(b.attitude().1);
            let t = b.travel();
            front = front.max((t[0] + t[1]) / 2.0 - (t[2] + t[3]) / 2.0);
        }
        println!(
            "   100 km/h'den tam fren: burun en çok {:.2}° aşağı · ön şerit arkadan {:.1} cm fazla sıkışık",
            pitch.to_degrees(),
            front * 100.0
        );
    }
    {
        let mut b = Bench::new(&renderer, &mut assets, &car);
        b.run(2.0, |_, _| Controls::default());
        let base = b.attitude().1;
        let mut squat = 0.0f32;
        let t0 = b.t;
        while b.t - t0 < 3.0 {
            b.step(&Controls { throttle: 1.0, ..Controls::default() });
            squat = squat.min(b.attitude().1 - base);
        }
        println!("   duruştan tam gaz: burun en çok {:.2}° yukarı", -squat.to_degrees());
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

    mirror(&renderer, &mut assets, &car);
}

/// Into a wall at `speed` m/s, `angle` degrees off square (0 = head on, 60 = a glancing scrape),
/// coasting from 10 m before it: how much the car bounces back off the wall's normal, how much of
/// its speed along the wall it keeps, and whether the hit spins it or rolls it.
fn wall(renderer: &Renderer, assets: &mut AssetManager, car: &str, speed: f32, angle: f32) {
    const AHEAD: f32 = 220.0;
    let mut b = Bench::new(renderer, assets, car);
    b.add_wall(AHEAD, angle);
    b.run(2.0, |_, _| Controls::default());
    // The wall's normal, pointing back at the car.
    let normal = Quat::from_rotation_y(angle.to_radians()) * Vec3::Z;
    let mut t = 0.0;
    while t < 60.0 {
        let ahead = AHEAD - b.travelled();
        if ahead < 10.0 {
            break;
        }
        let throttle = if b.forward_speed() < speed { 1.0 } else { 0.0 };
        let steer = 0.0;
        b.step(&Controls { throttle, steer, ..Controls::default() });
        t += FIXED_DT;
    }
    let before = b.velocity_world();
    let (into_before, along_before) = (-before.dot(normal), before.length());
    let (mut out, mut yaw, mut roll, mut flipped) = (0.0f32, 0.0f32, 0.0f32, false);
    let mut along_after = 0.0;
    let mut s = 0.0;
    while s < 3.0 {
        b.step(&Controls::default());
        s += FIXED_DT;
        let v = b.velocity_world();
        out = out.max(v.dot(normal));
        yaw = yaw.max(b.yaw_rate().abs());
        roll = roll.max(b.attitude().0.abs());
        flipped |= b.up().y < 0.2;
        along_after = (v - normal * v.dot(normal)).length();
    }
    println!(
        "   {speed:>4.0} m/s · {angle:>3.0}° → sekme {into_before:>5.1} → {out:>4.1} m/s (e≈{:.2}) · duvar boyunca {along_after:>5.1} / {along_before:>5.1} · sapma {yaw:>4.2} rad/s · yalpa {:>5.1}° · {}",
        if into_before > 0.1 { out / into_before } else { 0.0 },
        roll.to_degrees(),
        if flipped { "TERS DÖNDÜ" } else { "tekerlekleri üstünde" },
    );
}

/// A kerb `height` tall across the path, turned `angle` from square-on, met at `speed` m/s with
/// the throttle lifted 10 m before it; three seconds watched.
fn kerb(renderer: &Renderer, assets: &mut AssetManager, car: &str, height: f32, speed: f32, angle: f32) {
    const AHEAD: f32 = 220.0;
    let mut b = Bench::new(renderer, assets, car);
    b.add_kerb(AHEAD, angle, height);
    b.run(2.0, |_, _| Controls::default());
    let mut t = 0.0;
    while t < 60.0 && AHEAD - b.travelled() > 12.0 {
        let throttle = if b.forward_speed() < speed { 1.0 } else { 0.0 };
        b.step(&Controls { throttle, ..Controls::default() });
        t += FIXED_DT;
    }
    let y0 = b.position().y;
    let (mut roll, mut rate, mut air, mut lift, mut flipped) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, false);
    let mut s = 0.0;
    while s < 3.0 {
        b.step(&Controls::default());
        s += FIXED_DT;
        let (r, p) = b.attitude();
        roll = roll.max(r.abs()).max(p.abs());
        rate = rate.max(b.roll_rate().abs());
        if b.wheels_down() == 0 {
            air += FIXED_DT;
        }
        lift = lift.max(b.position().y - y0);
        flipped |= b.up().y < 0.2;
    }
    println!(
        "   {height:.2} m · {speed:>4.0} m/s · {angle:>3.0}° → yalpa/yunus {:>5.1}° · {:>5.1}°/s · havada {air:.2} s · sıçrama {lift:.2} m · {}",
        roll.to_degrees(),
        rate.to_degrees(),
        if flipped { "TERS DÖNDÜ" } else { "" },
    );
}

/// The car set sliding sideways at `lateral` m/s towards a kerb `height` tall 4 m away; three
/// seconds watched, no controls.
fn trip(renderer: &Renderer, assets: &mut AssetManager, car: &str, height: f32, lateral: f32) {
    let mut b = Bench::new(renderer, assets, car);
    b.add_kerb(5.5, 0.0, height);
    b.run(2.0, |_, _| Controls::default());
    b.slide_sideways(lateral);
    if std::env::var("NFS_TRIP_TRACE").is_ok() {
        eprintln!("trip: at {:?} v {:?} up {:?}", b.position(), b.velocity_world(), b.up());
        b.step(&Controls::default());
        eprintln!("trip: after one step at {:?} v {:?}", b.position(), b.velocity_world());
    }
    let (mut roll, mut rate, mut flipped) = (0.0f32, 0.0f32, false);
    let mut s = 0.0;
    while s < 3.0 {
        b.step(&Controls::default());
        s += FIXED_DT;
        roll = roll.max(b.attitude().0.abs());
        rate = rate.max(b.roll_rate().abs());
        flipped |= b.up().y < 0.2;
    }
    println!(
        "   {height:.2} m · {lateral:>4.0} m/s → yalpa {:>5.1}° · {:>5.1}°/s · {}",
        roll.to_degrees(),
        rate.to_degrees(),
        if flipped { "TERS DÖNDÜ" } else { "" },
    );
}

/// Full throttle from rest on the flat, then the pedal lifted at speed: the drivetrain alone.
///
/// Prints each gear change (time, speed, the rpm it left at and landed on), the times to 60, 100
/// and 160 km/h and the top speed after 60 s — then 10 s off the throttle from there, which is
/// where engine braking and the game's lifted-off drag show. `NFS_POWERTRAIN=off` runs it with
/// the engine's own drivetrain, for the comparison.
fn launch(renderer: &Renderer, assets: &mut AssetManager, car: &str) {
    let mut b = Bench::new(renderer, assets, car);
    b.run(2.0, |_, _| Controls::default());
    // `NFS_BENCH_NOS=1`: nitro held from the line — the tank burns out in its six seconds.
    let nos = std::env::var("NFS_BENCH_NOS").is_ok_and(|v| v == "1");
    let full = Controls { throttle: 1.0, nitro: nos, ..Controls::default() };
    let (mut gear, mut rpm) = b.gearbox();
    let mut marks: [(f32, Option<f32>); 3] = [(60.0, None), (100.0, None), (160.0, None)];
    let mut t = 0.0f32;
    let mut shifts = Vec::new();
    while t < 60.0 {
        b.step(&full);
        t += FIXED_DT;
        let kmh = b.forward_speed() * 3.6;
        for m in &mut marks {
            if m.1.is_none() && kmh >= m.0 {
                m.1 = Some(t);
            }
        }
        let (g, r) = b.gearbox();
        if g != gear {
            shifts.push(format!("{}→{} {:.1}s {:.0}km/h {:.0}→{:.0}", gear, g, t, kmh, rpm, r));
            gear = g;
        }
        rpm = r;
    }
    let top = b.forward_speed() * 3.6;
    let fmt = |m: (f32, Option<f32>)| m.1.map_or("—".to_string(), |s| format!("{s:.1} s"));
    println!(
        "== kalkış: 0-60 {} · 0-100 {} · 0-160 {} · 60 s'de {top:.1} km/h ({}. vites, {rpm:.0} rpm)",
        fmt(marks[0]),
        fmt(marks[1]),
        fmt(marks[2]),
        gear.saturating_sub(1),
    );
    println!("   geçişler: {}", shifts.join(" · "));
    let mut coast = Vec::new();
    for k in 1..=10 {
        b.run(1.0, |_, _| Controls::default());
        if k % 2 == 0 || k == 1 {
            let (g, r) = b.gearbox();
            coast.push(format!("{k}s {:.0} km/h ({}., {r:.0})", b.forward_speed() * 3.6, g.saturating_sub(1)));
        }
    }
    println!("   gaz bırakınca: {}", coast.join(" · "));
}

/// Every manoeuvre to the left and to the right: the second should be the first's mirror image.
fn mirror(renderer: &Renderer, assets: &mut AssetManager, car: &str) {
    // A mirror image should differ only by rounding. It did not until 2026-11-03 (`MOTOR-NOTLARI.md`
    // #19): the controller applies each wheel's force to the chassis velocity before computing the
    // next wheel's, so the result depended on the wheel order — up to ~12 % in loads and ~5 % in
    // lateral grip. `rig::vehicle_step` now alternates each axle's pair every step and every row
    // here reads "ayna" to two decimals. `NFS_WHEELORDER=fixed` brings the old asymmetry back; an
    // "AYNA DEĞİL" without it is something new — a mount on the wrong side would be far larger.
    const MIRROR_TOL: f32 = 0.03;
    println!("== ayna: aynı manevra sola (+) ve sağa (−) · sağdaki soldakinin aynası olmalı");
    for (kmh, steer) in [(40.0f32, 1.0f32), (70.0, 0.25), (70.0, 0.5), (100.0, 0.5)] {
        let left = circle(renderer, assets, car, kmh, |_| steer);
        let right = circle(renderer, assets, car, kmh, |_| -steer);
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
        if std::env::var("NFS_BENCH_TRACE").is_ok() {
            let r0 = |v: &[f32]| v.iter().map(|x| x.round()).collect::<Vec<_>>();
            println!("      yükler sol {:?} · sağ {:?} N (sıra: ön çift, arka çift)", r0(&l.loads), r0(&r.loads));
        }
    }
    for kmh in [60.0f32, 90.0] {
        let (Some(l), Some(r)) = (
            tap(renderer, assets, car, kmh, 1.0, false),
            tap(renderer, assets, car, kmh, -1.0, false),
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
