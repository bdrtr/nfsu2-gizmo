//! A player meeting a rival on rails, resolved the way NFSU2 resolves two cars.
//!
//! A rail is a kinematic body, and to the engine's solver a kinematic body has infinite mass: the
//! player hit a rival as he would a wall and bounced off it, and the rival did not so much as
//! twitch. So the pair is taken out of the solver (the rival's collider collides with nothing) and
//! resolved here, with the rules read out of `SPEED2.EXE` (`ROADMAP.md`, 2026-11-03):
//!
//! | rule | where | value |
//! |---|---|---|
//! | two-body impulse, `j = −(1+e)·vₙ / (1/m_A + 1/m_B + n·((I_A⁻¹(r_A×n))×r_A) + …)` | `0x5af0e0` | — |
//! | restitution between two cars | `0x5a0c90` | [`RESTITUTION`] 0.1 |
//! | one contact moves the lighter car by at most | `0x5af4c0` | [`STEP_CAP`] 0.3 m/s per game step |
//! | a car takes an impulse as if it weighed at least | `0x5a0b00` | [`RESPONSE_MASS`] 1 850 kg |
//! | the impulse's share that turns the car | `0x5a0b9f` | [`SPIN_SHARE`] 0.4 |
//!
//! The cap is the heart of it: a car does not *bounce* off another, it shoves it, a fraction of a
//! metre per second per step for as long as they touch. The game steps at 1/60 s ([`GAME_STEP`]: the
//! constant the car's own sub-step loop adds to its clock, `0x5b0839`); this one steps at 1/240, so
//! the cap is scaled to the step it is given and the shove per *second* is the game's.
//!
//! Not from the binary: where the two boxes touch. The game has a general narrowphase; a rival on
//! rails and a car on a road meet side to side, so this is the plan view — two rectangles, the
//! separating axis that overlaps least, and the corner that went deepest. And the overlap is undone
//! by moving both cars apart, shared by the same masses, because a capped impulse alone would let a
//! fast car drive half through the one in front before the two had traded their speeds.
//!
//! The rival never turns from a hit (a rail has no angle of its own to keep) and takes only the
//! linear half; the player takes both.

use gizmo::prelude::*;

/// Restitution between two cars (`0x5a0c90`).
pub const RESTITUTION: f32 = 0.1;
/// The most one contact may change the lighter car's speed in one game step, m/s (`0x5af4c0`:
/// `j ≤ 0.3 / max(1/m_A, 1/m_B)`).
pub const STEP_CAP: f32 = 0.3;
/// The game's physics step, seconds.
pub const GAME_STEP: f32 = 1.0 / 60.0;
/// A car responds to an impulse as if it weighed at least this, kg (`0x5a0b00`: the impulse is
/// scaled by `min(1, m / 1.85 t)` before it is applied).
pub const RESPONSE_MASS: f32 = 1850.0;
/// The share of the impulse's moment that reaches the car's spin (`0x5a0b9f`). A flag on the body
/// makes it 1.0 instead; which bodies carry it is not known, so both sides get the default.
pub const SPIN_SHARE: f32 = 0.4;
/// Overlap left alone, metres: separating to exactly zero makes the pair touch and part every
/// other step.
const SLOP: f32 = 0.01;

/// One car as the contact sees it.
#[derive(Debug, Clone, Copy)]
pub struct Body {
    /// The centre of its collision box, world space.
    pub centre: Vec3,
    pub rotation: Quat,
    /// Half the box: across, up, along.
    pub half: Vec3,
    pub velocity: Vec3,
    pub spin: Vec3,
    /// Kilograms.
    pub mass: f32,
    /// The body-local diagonal of its inertia tensor, kg·m²; zero for a body that does not turn.
    pub inertia: Vec3,
}

impl Body {
    /// The velocity of the body's material at `p`.
    fn velocity_at(&self, p: Vec3) -> Vec3 {
        self.velocity + self.spin.cross(p - self.centre)
    }

    /// `I⁻¹ · v` in world space; zero for a body that does not turn.
    fn inv_inertia(&self, v: Vec3) -> Vec3 {
        let local = self.rotation.inverse() * v;
        let inv = |i: f32| if i > 0.0 { 1.0 / i } else { 0.0 };
        self.rotation * Vec3::new(local.x * inv(self.inertia.x), local.y * inv(self.inertia.y), local.z * inv(self.inertia.z))
    }

    /// The plan-view rectangle: centre, the unit axes across and along, and the half lengths.
    fn plan(&self) -> (Vec2, [Vec2; 2], [f32; 2]) {
        let flat = |v: Vec3| Vec2::new(v.x, v.z).normalize_or_zero();
        let across = flat(self.rotation * Vec3::X);
        let along = flat(self.rotation * Vec3::Z);
        (Vec2::new(self.centre.x, self.centre.z), [across, along], [self.half.x, self.half.z])
    }
}

/// Where two cars touch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// Horizontal unit normal, pointing from `b` towards `a`.
    pub normal: Vec3,
    /// How far they overlap along it, metres.
    pub depth: f32,
    /// The point of contact, world space.
    pub point: Vec3,
}

/// The overlap of two cars' boxes in plan view, if they overlap — and only if their heights do too.
#[must_use]
pub fn contact(a: &Body, b: &Body) -> Option<Contact> {
    if (a.centre.y - b.centre.y).abs() > a.half.y + b.half.y {
        return None;
    }
    let (ca, axa, ha) = a.plan();
    let (cb, axb, hb) = b.plan();
    let radius = |axes: &[Vec2; 2], half: &[f32; 2], n: Vec2| half[0] * axes[0].dot(n).abs() + half[1] * axes[1].dot(n).abs();
    let d = ca - cb;
    let mut best: Option<(f32, Vec2)> = None;
    for n in axa.iter().chain(axb.iter()).copied() {
        if n == Vec2::ZERO {
            continue;
        }
        let overlap = radius(&axa, &ha, n) + radius(&axb, &hb, n) - d.dot(n).abs();
        if overlap <= 0.0 {
            return None;
        }
        if best.is_none_or(|(o, _)| overlap < o) {
            best = Some((overlap, if d.dot(n) < 0.0 { -n } else { n }));
        }
    }
    let (depth, n) = best?;
    // The corner of either box that went deepest into the other, along the normal: a's corner
    // furthest towards b, or b's furthest towards a, whichever reaches further.
    let corners = |c: Vec2, ax: &[Vec2; 2], h: &[f32; 2]| {
        [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)].map(|(s, t)| c + ax[0] * (h[0] * s) + ax[1] * (h[1] * t))
    };
    // Corners level with the deepest are averaged: two cars side by side touch along an edge, and
    // its middle is the contact, not whichever corner the loop met first.
    let deepest = |pts: [Vec2; 4], dir: Vec2| {
        let far = pts.iter().map(|p| p.dot(dir)).fold(f32::MIN, f32::max);
        let near: Vec<Vec2> = pts.into_iter().filter(|p| p.dot(dir) > far - 1e-3).collect();
        near.iter().copied().sum::<Vec2>() / near.len().max(1) as f32
    };
    let deepest_a = deepest(corners(ca, &axa, &ha), -n);
    let deepest_b = deepest(corners(cb, &axb, &hb), n);
    // Whichever corner sits inside the other box; with both, the midpoint.
    let inside = |p: Vec2, c: Vec2, ax: &[Vec2; 2], h: &[f32; 2]| {
        let r = p - c;
        r.dot(ax[0]).abs() <= h[0] + 1e-4 && r.dot(ax[1]).abs() <= h[1] + 1e-4
    };
    let p = match (inside(deepest_a, cb, &axb, &hb), inside(deepest_b, ca, &axa, &ha)) {
        (true, false) => deepest_a,
        (false, true) => deepest_b,
        _ => (deepest_a + deepest_b) * 0.5,
    };
    let y = (a.centre.y + b.centre.y) * 0.5;
    Some(Contact { normal: Vec3::new(n.x, 0.0, n.y), depth, point: Vec3::new(p.x, y, p.y) })
}

/// What a contact does to the two cars.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Shove {
    /// Change of `a`'s velocity and spin.
    pub dv_a: Vec3,
    pub dw_a: Vec3,
    /// Change of `b`'s velocity (`b` does not turn).
    pub dv_b: Vec3,
    /// How far to move each apart, metres, world space.
    pub move_a: Vec3,
    pub move_b: Vec3,
}

/// Resolve one contact over a step of `dt` seconds. `b` is the rail: its spin is ignored.
#[must_use]
pub fn resolve(a: &Body, b: &Body, c: &Contact, dt: f32) -> Shove {
    let n = c.normal;
    let ra = c.point - a.centre;
    let (ma, mb) = (a.mass.max(1.0), b.mass.max(1.0));
    // The response masses: what each car's velocity actually moves by per unit of impulse.
    let (sa, sb) = ((ma / RESPONSE_MASS).min(1.0), (mb / RESPONSE_MASS).min(1.0));

    // Undo the overlap, shared the way the impulse would be.
    let (wa, wb) = (sa / ma, sb / mb);
    let push = (c.depth - SLOP).max(0.0) / (wa + wb).max(1e-9);
    let mut out = Shove { move_a: n * (push * wa), move_b: -n * (push * wb), ..Shove::default() };

    let vn = (a.velocity_at(c.point) - b.velocity).dot(n);
    if vn >= 0.0 {
        return out; // already parting
    }
    let turn = a.inv_inertia(ra.cross(n)).cross(ra).dot(n);
    let j = -(1.0 + RESTITUTION) * vn / (1.0 / ma + 1.0 / mb + turn);
    let cap = STEP_CAP * (dt / GAME_STEP) * ma.min(mb);
    let j = j.clamp(0.0, cap);
    out.dv_a = n * (j * sa / ma);
    out.dw_a = a.inv_inertia(ra.cross(n * (j * sa))) * SPIN_SHARE;
    out.dv_b = -n * (j * sb / mb);
    out
}

/// The player against every car on rails, one physics step: each contact resolved and applied —
/// the player's body takes velocity, spin and a step apart, each rail its speed, drift and a step
/// off its line. Returns the contacts made, with the rail's index and the closing speed before.
///
/// The one place this happens, so the race and the bench resolve a collision the same way.
pub fn player_vs_rails(
    world: &mut World,
    player: &super::CarRig,
    rigs: &[super::CarRig],
    field: &mut [super::rail::Rail],
    line: &super::rail::RaceLine,
    dt: f32,
) -> Vec<(usize, Contact, f32)> {
    let mut out = Vec::new();
    let Some(mut me) = player.bump_body(world, None) else { return out };
    for (k, (rig, rail)) in rigs.iter().zip(field.iter_mut()).enumerate() {
        let Some(them) = rig.bump_body(world, Some(rail.velocity(line))) else { continue };
        let Some(c) = contact(&me, &them) else { continue };
        let closing = -(me.velocity_at(c.point) - them.velocity).dot(c.normal);
        let s = resolve(&me, &them, &c, dt);
        player.take_bump(world, s.dv_a, s.dw_a, s.move_a);
        me.velocity += s.dv_a;
        me.spin += s.dw_a;
        me.centre += s.move_a;
        rail.bump(line, s.dv_b, s.move_b);
        out.push((k, c, closing));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(at: Vec3, yaw: f32, velocity: Vec3) -> Body {
        Body {
            centre: at,
            rotation: Quat::from_rotation_y(yaw),
            half: Vec3::new(0.85, 0.5, 2.2),
            velocity,
            spin: Vec3::ZERO,
            mass: 1220.0,
            inertia: Vec3::new(2000.0, 2200.0, 600.0),
        }
    }

    #[test]
    fn apart_is_no_contact_and_overlapping_is_one() {
        let a = car(Vec3::new(0.0, 0.5, 0.0), 0.0, Vec3::ZERO);
        assert!(contact(&a, &car(Vec3::new(3.0, 0.5, 0.0), 0.0, Vec3::ZERO)).is_none());
        assert!(contact(&a, &car(Vec3::new(0.0, 5.0, 0.0), 0.0, Vec3::ZERO)).is_none(), "one above the other");
        // Side by side, 0.2 m into each other: the normal is across, from b to a.
        let c = contact(&a, &car(Vec3::new(1.5, 0.5, 0.0), 0.0, Vec3::ZERO)).expect("touching");
        assert!((c.normal - Vec3::NEG_X).length() < 1e-5, "{:?}", c.normal);
        assert!((c.depth - 0.2).abs() < 1e-4);
        // Nose to tail: along.
        let c = contact(&a, &car(Vec3::new(0.0, 0.5, -4.2), 0.0, Vec3::ZERO)).expect("touching");
        assert!((c.normal - Vec3::Z).length() < 1e-5, "{:?}", c.normal);
    }

    #[test]
    fn a_rear_end_shoves_rather_than_bounces() {
        // The player at 20 m/s into the back of a rival at 15, a hand's width in.
        let a = car(Vec3::new(0.0, 0.5, 4.3), 0.0, Vec3::new(0.0, 0.0, -20.0));
        let b = car(Vec3::new(0.0, 0.5, 0.0), 0.0, Vec3::new(0.0, 0.0, -15.0));
        let c = contact(&a, &b).expect("touching");
        let dt = 1.0 / 240.0;
        let s = resolve(&a, &b, &c, dt);
        // The rival is pushed on, the player held back, by equal and opposite momentum.
        assert!(s.dv_b.z < 0.0 && s.dv_a.z > 0.0);
        assert!((s.dv_a * a.mass + s.dv_b * b.mass).length() < 1e-3);
        // And by no more than the game's cap, scaled to this step and the response mass.
        let cap = STEP_CAP * (dt / GAME_STEP) * a.mass / RESPONSE_MASS;
        assert!(s.dv_b.length() <= cap + 1e-5, "{} > {cap}", s.dv_b.length());
        // A head-on rear hit is through the centres: nothing turns.
        assert!(s.dw_a.length() < 1e-4);
        // The overlap is undone, half each.
        assert!((s.move_a.z - (c.depth - SLOP) * 0.5).abs() < 1e-4);
    }

    #[test]
    fn parting_cars_are_only_separated() {
        let a = car(Vec3::new(1.6, 0.5, 0.0), 0.0, Vec3::new(1.0, 0.0, 0.0));
        let b = car(Vec3::new(0.0, 0.5, 0.0), 0.0, Vec3::ZERO);
        let c = contact(&a, &b).expect("touching");
        let s = resolve(&a, &b, &c, 1.0 / 240.0);
        assert_eq!(s.dv_a, Vec3::ZERO);
        assert_eq!(s.dv_b, Vec3::ZERO);
        assert!(s.move_a.x > 0.0 && s.move_b.x < 0.0);
    }

    #[test]
    fn a_hit_on_the_corner_turns_the_player() {
        // Sideways into the rival's flank with the player's nose: off-centre, so the player yaws.
        let a = car(Vec3::new(1.5, 0.5, 2.0), 0.0, Vec3::new(-5.0, 0.0, 0.0));
        let b = car(Vec3::new(0.0, 0.5, 0.0), 0.0, Vec3::ZERO);
        let c = contact(&a, &b).expect("touching");
        let s = resolve(&a, &b, &c, 1.0 / 240.0);
        assert!(s.dw_a.y.abs() > 0.0);
        assert!(s.dv_b.x < 0.0, "the rival is pushed away from the player");
    }
}
