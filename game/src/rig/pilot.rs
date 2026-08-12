//! The driver that is not a keyboard.
//!
//! [`Driver`](super::Driver) turns key presses into [`Controls`]; this turns a **course** into the
//! same thing, so a rival goes through the identical `CarRig::drive` the player does. Nothing here
//! touches physics: a pilot that could push its car would be a different car from the one being
//! raced against, and the first thing anyone would ask is whether it was cheating.
//!
//! ## What it follows, and why not a line
//!
//! There is no race line in the data — `ROADMAP.md` carries the two readings that were tried for
//! one and refuted. There is a **network** ([`Network`]) and there are **waypoints**: the event
//! outline, far too coarse to drive at a median step of 425 m, but with road under 14 of its 17
//! corners and in lap order.
//!
//! So a pilot walks the network toward the next waypoint. At every junction it takes the branch
//! that gets it nearer, and it refuses to turn round. That drives the course the event describes
//! over roads the file says exist, which is a weaker claim than "the race line" and the one the
//! data supports.
//!
//! ## Pure pursuit over that
//!
//! Steering is the oldest rule there is: aim at a point some way ahead and turn at it. The state
//! that matters is *which node* the car is at, advanced only when the car has passed it, so a
//! route that crosses itself — Bayview is a city, its routes cross constantly — cannot hand the
//! driver the wrong branch when two roads pass within a few metres.
//!
//! ## What is deliberately not modelled
//!
//! No racing line, no braking points, no awareness of the other cars. A rival drives the middle of
//! the road at a speed the corner allows and will happily drive into the back of another one.

use super::drive::Controls;
use crate::world::Network;
use gizmo::prelude::*;

/// How far ahead to aim, as a multiple of speed in m/s, and the bounds that keeps it inside.
///
/// Speed-proportional because a fixed distance is wrong at both ends: short enough to be accurate
/// at 30 km/h is a violent oscillation at 150, and long enough to be smooth at 150 cuts every
/// corner at 30.
const LOOKAHEAD_PER_SPEED: f32 = 0.9;
const LOOKAHEAD_MIN: f32 = 12.0;
const LOOKAHEAD_MAX: f32 = 40.0;

/// How near a waypoint counts as taken. Larger than [`REACHED`] because a waypoint is a corner of
/// a 6 km outline, not a place on the road.
const WAYPOINT_REACHED: f32 = 60.0;

/// Steering lock the pilot will ask for, as a fraction of the controller's own.
const STEER_LIMIT: f32 = 0.85;

/// Below this, in m/s, a car asking for throttle is not driving.
const STALL_SPEED: f32 = 0.7;
/// How long it has to be true before the pilot tries something else.
const STALL_FOR: f32 = 1.5;
/// How long a car is left alone at the start before it can be called stuck.
const SETTLE: f32 = 3.0;
/// How long a reversing manoeuvre lasts.
const BACK_FOR: f32 = 1.2;
/// The step the pilot assumes between calls. It is called once per frame and the physics runs at
/// sixty, so this is right where it matters and generous where it does not.
const TICK: f32 = 1.0 / 60.0;

/// How much steering costs throttle — the whole of the speed policy.
const CORNER_LIFT: f32 = 0.75;

/// One rival's driver.
#[derive(Clone, Debug, Default)]
pub struct Pilot {
    /// Where it is on the network.
    at: Option<u32>,
    /// Where it came from, so it does not turn round at a junction.
    from: Option<u32>,
    /// Which waypoint of the course it is heading for.
    goal: usize,
    /// Smoothed steering, the same field [`Driver`](super::Driver) keeps and for the same reason.
    steer: f32,
    /// How many junctions it has taken. The one number that says a pilot is getting somewhere.
    passed: usize,
    /// How long this pilot has been driving. Only used to give a car a moment to settle before
    /// it is judged stuck.
    age: f32,
    /// How long the car has been asking for throttle and not moving.
    ///
    /// A pilot with nothing else to try holds full lock and a fifteen-percent throttle against
    /// whatever it is touching, for ever. Three of eight rivals do exactly that.
    ///
    /// **This does not fix them, and the number says so**: 166 junctions without the manoeuvre and
    /// 165 with, the same five cars away and the same three left behind — they reverse, and then
    /// they are stuck again facing the same thing. It is kept because a driver that tries to back
    /// off what it has hit is a driver and one that grinds for ever is not, and because the three
    /// tries that came before it are worth having written down: spreading the grid apart (5/8, 6/8,
    /// 5/8 at one, two and four times the spacing), staggering the start (5/8, 4/8, 3/8, 6/8 at
    /// nought to two seconds), and gating recovery on progress rather than time, which locked it
    /// away from the only cars that needed it.
    stalled: f32,
    /// How long is left of a reversing manoeuvre.
    backing: f32,
    /// Seconds still to wait before pulling away.
    ///
    /// A grid is four abreast and two deep with 3.5 m across and 5.7 m between the rows, against a
    /// car 1.64 m wide and 4.39 m long — so 1.3 m of clear air front to back. Eight cars given full
    /// throttle at once drive into each other, and a pilot with no traffic model cannot get out of
    /// that: measured, three of eight never leave the grid, and which three changes with the
    /// spacing rather than staying with a slot.
    hold: f32,
}

impl Pilot {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Put the pilot on the network at the node nearest a point.
    pub fn place(&mut self, at: Vec3, net: &Network) {
        self.at = net.nearest(at);
        self.from = None;
        self.passed = 0;
    }

    /// Hold this pilot on the line for `seconds` before it pulls away.
    pub fn hold_for(&mut self, seconds: f32) {
        self.hold = seconds;
    }

    /// How many junctions it has driven through.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.passed
    }

    /// Which node of the network it is on.
    #[must_use]
    pub fn node(&self) -> Option<u32> {
        self.at
    }

    /// Which waypoint it is heading for.
    #[must_use]
    pub fn goal(&self) -> usize {
        self.goal
    }

    /// This frame's controls. `None` when it has not been placed or the network is empty.
    pub fn drive(
        &mut self,
        at: Vec3,
        facing: Quat,
        speed: f32,
        net: &Network,
        course: &[Vec3],
    ) -> Option<Controls> {
        let here = self.at?;
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);

        // Still on the line: brakes on, wheels straight, and no advance along the network — a
        // pilot that walked the graph while its car stood still would arrive already lost.
        if self.hold > 0.0 {
            self.hold -= TICK;
            return Some(Controls {
                throttle: 0.0,
                brake: 1.0,
                steer: 0.0,
                toggle_auto_shift: false,
            });
        }

        // The waypoint being driven to. With no course, the pilot still drives — it just has
        // nothing to prefer at a junction, and takes whatever is not backwards.
        let target = course.get(self.goal).copied();
        if let Some(t) = target {
            if flat(t - at).length() < WAYPOINT_REACHED {
                self.goal = (self.goal + 1) % course.len().max(1);
            }
        }
        let toward = target.unwrap_or(at + facing * Vec3::NEG_Z * 1000.0);

        // Advance while the **next** node is nearer the car than the one held. Self-limiting by
        // construction: it stops the moment the held node is the nearest, so a stopped car stops
        // advancing and a fast one keeps up.
        //
        // Both simpler rules were tried and both fail, in opposite directions. "Advance when near
        // enough" is a runaway — the node stepped to is usually near enough too, so the pilot walks
        // the graph at frame rate while the car crawls: 6,600 junctions per rival in a hundred
        // seconds, against the one a second a car at 96 km/h over 29 m nodes can drive. "Advance
        // only when the node is behind" is the opposite: a node the car cannot reach is never
        // behind it, so the pilot stops at the first one it cannot get to and the car stops at 65 m.
        let dist = |i: u32| net.node(i).map_or(f32::MAX, |j| flat(j.at - at).length());
        for _ in 0..3 {
            let Some(next) = net.step(self.at?, self.from, toward) else { break };
            if dist(next) >= dist(self.at?) {
                break;
            }
            self.from = self.at;
            self.at = Some(next);
            self.passed += 1;
        }
        let _ = here;

        // Aim: walk the network forward from the held node until far enough away, so the aim point
        // follows the road round a corner instead of cutting across it.
        let look = (speed * LOOKAHEAD_PER_SPEED).clamp(LOOKAHEAD_MIN, LOOKAHEAD_MAX);
        let (mut cur, mut prev) = (self.at?, self.from);
        let mut aim = net.node(cur)?.at;
        let mut walked = flat(aim - at).length();
        for _ in 0..8 {
            if walked >= look {
                break;
            }
            let Some(next) = net.step(cur, prev, toward) else { break };
            let p = net.node(next)?.at;
            walked += flat(p - aim).length();
            aim = p;
            prev = Some(cur);
            cur = next;
        }

        // Stuck, and doing something about it. Reverse for a moment with the lock reversed too,
        // which is what backs a car off the thing it has driven into rather than along it.
        //
        // The threshold is speed and not position because a car grinding along a wall is moving:
        // it is the *forward* speed that is gone.
        //
        // Two things this has to get right, and the first version got both wrong. A car that has
        // just been dropped on the grid is not stalled, it is settling — so the count only runs
        // once the pilot has driven at all. And a car that is *reversing* is not stalled either;
        // measuring `|speed|` made every car reverse for ever, the whole field ending sixty metres
        // behind the line facing the wrong way.
        if self.backing > 0.0 {
            self.backing -= TICK;
            self.stalled = 0.0;
            return Some(Controls {
                throttle: -0.7,
                brake: 0.0,
                steer: -self.steer,
                toggle_auto_shift: false,
            });
        }
        // Gated on *time* and not on having taken a junction: the cars that need this most are
        // exactly the ones that never took one, and gating on progress meant the pilots that were
        // stuck from the start were the only ones that could not try to get out.
        self.age += TICK;
        if self.age > SETTLE && speed < STALL_SPEED && self.hold <= 0.0 {
            self.stalled += TICK;
        } else {
            self.stalled = 0.0;
        }
        if self.stalled >= STALL_FOR {
            self.stalled = 0.0;
            self.backing = BACK_FOR;
        }

        let to = flat(aim - at).normalize_or_zero();
        let f = flat(facing * Vec3::NEG_Z).normalize_or_zero();
        let angle = f.cross(to).y.atan2(f.dot(to));
        let want = (angle * 2.0 / std::f32::consts::PI).clamp(-1.0, 1.0) * STEER_LIMIT;
        self.steer += (want - self.steer) * 0.35;
        let throttle = (1.0 - self.steer.abs() * CORNER_LIFT).max(0.15);

        Some(Controls { throttle, brake: 0.0, steer: self.steer, toggle_auto_shift: false })
    }
}
