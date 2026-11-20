//! The spray a car's tyres throw off a wet road: a mist behind every wheel, thicker the faster it turns.
//!
//! Each wheel is followed from frame to frame by where its visual entity stands — the same for the
//! player's car, a rival on rails and one a pilot drives, and no physics read — and past
//! [`SPRAY_FROM`] it throws puffs at a rate that grows with its speed ([`puffs`]). A puff leaves the
//! tyre's contact patch with part of the car's velocity and a lift ([`puff`]), slows in the air,
//! barely falls, swells and fades. They are the engine's GPU particles, lit: the street lamps, the
//! headlights behind and the brake lights in front light them, thrown forward as water's fine droplets
//! throw light — the spray of the car ahead glows in your own beams.
//!
//! The particles are the spray's alone: dry, the cruise has none (`renderer.gpu_particles` stays
//! `None`).

use crate::ecs::ById;
use gizmo::prelude::*;
use gizmo::renderer::gpu_particles::{GpuParticle, GpuParticleSystem};
use std::collections::HashMap;

/// Below this a tyre throws up nothing worth seeing, m/s: 30 km/h.
pub const SPRAY_FROM: f32 = 30.0 / 3.6;
/// Puffs a wheel throws a second for every m/s past [`SPRAY_FROM`], on a soaked road: at 100 km/h,
/// eighty. Small, faint and spread wide, so that they run together into a mist: fewer, bigger, brighter
/// ones stood apart in a car's headlights as round discs (2026-11-21), and a hundred and twenty of the
/// first kind were too thick (the user, 2026-11-20).
pub const SPRAY_RATE: f32 = 4.0;
/// The spray's own particle ring. At most some 32 wheels × 120 puffs a second (at 200 km/h) × 1.1 s are
/// alive, about 4,200. The renderer's default ring is 100,000, and the whole of a ring is simulated and drawn —
/// twice, with the volumetric light's veil — while anything in it lives: 0.85 ms of GPU for a few hundred
/// puffs (1080p, AMD 780M, 2026-11-20).
pub const SPRAY_RING: u32 = 8192;
/// The most puffs a wheel throws in one frame, whatever the frame's length.
const MAX_PER_FRAME: u32 = 8;
/// A wheel that moved further than this between frames was put there, not driven: no spray.
const JUMP: f32 = 5.0;
/// How strongly the spray's droplets throw light forward (`GpuParticleSystem::scatter_anisotropy`).
pub const SPRAY_SCATTER: f32 = 0.5;

/// The spray's state between frames, a resource of the world: where each wheel was, and the part of a
/// puff it owes.
#[derive(Default)]
pub struct Spray {
    last: HashMap<u32, Vec3>,
    owed: HashMap<u32, f32>,
    seed: u32,
    configured: bool,
}

impl Spray {
    /// A number in `[0, 1)`, a new one each call.
    fn next(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let mut x = self.seed;
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// How many puffs a wheel throws this frame, going `speed` m/s across a road `wet` (0–1) for `dt`
/// seconds, owing `owed` of a puff from frames before; and what it owes after.
#[must_use]
pub fn puffs(speed: f32, wet: f32, dt: f32, owed: f32) -> (u32, f32) {
    let rate = SPRAY_RATE * (speed - SPRAY_FROM).max(0.0) * wet.clamp(0.0, 1.0);
    if rate <= 0.0 {
        return (0, 0.0);
    }
    let due = owed + rate * dt.max(0.0);
    let n = (due.floor() as u32).min(MAX_PER_FRAME);
    (n, due - n as f32)
}

/// One puff off a tyre touching the road at `contact`, the wheel going `velocity`, on a road `wet`;
/// `r` four numbers in `[0, 1)` that make each puff its own.
#[must_use]
pub fn puff(contact: Vec3, velocity: Vec3, wet: f32, r: [f32; 4]) -> GpuParticle {
    let ahead = Vec3::new(velocity.x, 0.0, velocity.z).normalize_or_zero();
    let side = Vec3::new(-ahead.z, 0.0, ahead.x);
    // Off the back of the contact patch, a hand either side of its middle.
    let at = contact - ahead * 0.15 + side * (r[0] - 0.5) * 0.5 + Vec3::Y * 0.05;
    // Part of the car's speed — the tread throws the water forward as much as the car runs from it —
    // lifted, and spread sideways.
    let thrown = velocity * (0.3 + 0.25 * r[1]) + Vec3::Y * (0.6 + 1.6 * r[2]) + side * (r[3] - 0.5) * 3.0;
    GpuParticle {
        position: at.to_array(),
        life: 0.0,
        velocity: thrown.to_array(),
        max_life: 0.5 + 0.6 * r[2],
        color: [0.82, 0.85, 0.9, 0.03 * wet.clamp(0.0, 1.0)],
        size_start: 0.12,
        size_end: 0.35 + 0.3 * r[1],
        _padding: [0.0; 2],
    }
}

/// Throws this frame's spray off `wheels` (each wheel's visual entity and its radius), on a road as wet
/// as the renderer's rain says, for a frame as long as the world's clock says. Nothing while it is dry or
/// there is no particle system.
pub fn throw(world: &mut World, renderer: &mut gizmo::renderer::Renderer, wheels: &[(u32, f32)]) {
    // `NFS_SPRAY=off`: none, and no particle system.
    if std::env::var("NFS_SPRAY").as_deref() == Ok("off") {
        renderer.gpu_particles = None;
        return;
    }
    let wet = renderer.wetness.map_or(0.0, |w| w.level);
    let dt = world.get_resource::<gizmo::core::time::Time>().map_or(0.0, |t| t.dt());
    if world.get_resource::<Spray>().is_none() {
        world.insert_resource(Spray::default());
    }
    let positions: Vec<(u32, f32, Vec3)> = {
        let transforms = world.query::<&Transform>();
        wheels.iter().filter_map(|&(id, radius)| transforms.by_id(id).map(|t| (id, radius, t.position))).collect()
    };
    let Some(mut spray) = world.get_resource_mut::<Spray>() else { return };
    if !spray.configured || renderer.gpu_particles.is_none() {
        // A ring of the spray's own size, configured as the spray wants it.
        let mut particles = GpuParticleSystem::new(&renderer.device, SPRAY_RING, &renderer.scene.global_bind_group_layout, renderer.post.hdr_texture.format());
        configure(&mut particles, &renderer.device, &renderer.queue);
        renderer.gpu_particles = Some(particles);
        spray.configured = true;
    }
    let Some(particles) = renderer.gpu_particles.as_mut() else { return };
    let mut batch = Vec::new();
    for (id, radius, at) in positions {
        let before = spray.last.insert(id, at);
        let Some(before) = before.filter(|b| dt > 0.0 && b.distance(at) < JUMP) else {
            spray.owed.remove(&id);
            continue;
        };
        let velocity = (at - before) / dt;
        let speed = Vec3::new(velocity.x, 0.0, velocity.z).length();
        let owed = spray.owed.get(&id).copied().unwrap_or(0.0);
        let (n, left) = puffs(speed, wet, dt, owed);
        spray.owed.insert(id, left);
        let contact = at - Vec3::Y * radius * 0.9;
        for _ in 0..n {
            let r = [spray.next(), spray.next(), spray.next(), spray.next()];
            batch.push(puff(contact, velocity, wet, r));
        }
    }
    if !batch.is_empty() {
        particles.spawn_particles(&renderer.queue, &batch);
    }
}

/// The particle system as the spray wants it: lit, its light thrown forward, a soft puff for each, slowed
/// by the air and barely falling, curling as mist does.
fn configure(particles: &mut GpuParticleSystem, device: &wgpu::Device, queue: &wgpu::Queue) {
    particles.lit = true;
    particles.scatter_anisotropy = SPRAY_SCATTER;
    particles.gravity = 1.5;
    particles.drag = 2.0;
    particles.curl_strength = 0.8;
    particles.set_procedural_smoke_flipbook(device, queue);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tyre_throws_spray_past_thirty_and_more_the_faster_on_a_wetter_road() {
        let frame = 1.0 / 60.0;
        assert_eq!(puffs(SPRAY_FROM, 1.0, frame, 0.0), (0, 0.0), "at 30 km/h, none");
        assert_eq!(puffs(30.0, 0.0, frame, 0.5), (0, 0.0), "on a dry road, none, and nothing owed");
        // A second at 100 km/h: eighty puffs, a frame at a time.
        let speed = 100.0 / 3.6;
        let (mut thrown, mut owed) = (0, 0.0);
        for _ in 0..60 {
            let (n, left) = puffs(speed, 1.0, frame, owed);
            thrown += n;
            owed = left;
        }
        assert!((thrown as f32 - SPRAY_RATE * (speed - SPRAY_FROM)).abs() <= 1.0, "a second at 100 km/h: {thrown}");
        let (half, _) = puffs(speed, 0.5, 1.0, 0.0);
        let (full, _) = puffs(speed, 1.0, 1.0, 0.0);
        assert!(half <= full, "a damp road throws less than a soaked one");
        assert_eq!(puffs(200.0, 1.0, 1.0, 0.0).0, MAX_PER_FRAME, "a long frame throws no flood");
    }

    #[test]
    fn a_puff_leaves_the_back_of_the_tyre_going_the_cars_way_and_up() {
        let velocity = Vec3::new(0.0, 0.0, -30.0);
        let p = puff(Vec3::ZERO, velocity, 1.0, [0.5; 4]);
        let at = Vec3::from_array(p.position);
        let v = Vec3::from_array(p.velocity);
        assert!(at.z > 0.0, "behind the contact patch: {at}");
        assert!(v.z < 0.0 && v.z > velocity.z, "with part of the car's speed: {v}");
        assert!(v.y > 0.0, "lifted: {v}");
        assert_eq!(p.life, 0.0, "born now");
        assert!(p.size_end > p.size_start, "it swells");
        assert!(p.color[3] > 0.0 && p.color[3] < 0.3, "a thin mist: {}", p.color[3]);
        assert_eq!(puff(Vec3::ZERO, velocity, 0.0, [0.5; 4]).color[3], 0.0, "nothing on a dry road");
    }
}
