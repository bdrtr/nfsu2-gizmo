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

/// Steering lock the pilot will ask for, as a fraction of the controller's own.
const STEER_LIMIT: f32 = 0.85;

/// Below this, in m/s, a car asking for throttle is not driving.
const STALL_SPEED: f32 = 0.7;
/// How long it has to be true before the pilot tries something else.
const STALL_FOR: f32 = 1.5;
/// How long a car is left alone at the start before it can be called stuck.
const SETTLE: f32 = 3.0;
/// How near a waypoint counts as having driven past it, for [`Pilot::covered`].
///
/// The waypoints are 40 m apart after `route::densify`, so this is one step: near enough that a car
/// on the road claims every one it passes, far enough that it does not have to clip the exact point.
const COVERED_WITHIN: f32 = 40.0;
/// How long a reversing manoeuvre lasts.
const BACK_FOR: f32 = 1.2;
/// The step the pilot assumes between calls. It is called once per frame and the physics runs at
/// sixty, so this is right where it matters and generous where it does not.
const TICK: f32 = 1.0 / 60.0;

/// The speed, in m/s, at which full lock is as much as the car will hold. Above it, braking.
///
/// Swept over eight routes on all three measures at once — cars that got away, cars that fell off
/// the world, and distance covered by the ones still on it. No brake gives 41/64 away, 20 fallen
/// and 2,404 m; 20 m/s gives 41, 18 and 2,418; 14 gives 41, 17 and 2,354; **9 gives 44, 13 and
/// 2,594**; 6 gives 41, 14 and 2,242. Nine wins on every column, which is rarer than it sounds and
/// is why it is the default.
const BRAKE_SPEED: f32 = 9.0;
/// How hard the brake comes on past that.
const BRAKE_GAIN: f32 = 1.5;

/// How much steering costs throttle.
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
    /// Completed laps. Counted where the waypoint index wraps, which is the only place a closed
    /// course says a lap ended.
    laps: u32,
    /// Where the pilot started, so a lap is measured from the grid rather than from waypoint zero
    /// — those are different points, and using the wrong one credits a lap before the first is
    /// driven.
    line: usize,
    /// Smoothed steering, the same field [`Driver`](super::Driver) keeps and for the same reason.
    steer: f32,
    /// How many junctions it has taken. The one number that says a pilot is getting somewhere.
    passed: usize,
    /// How many *distinct* nodes it has been on. Against [`Self::passed`] this is the difference
    /// between a car making its way round a course and one going round in circles.
    seen: std::collections::BTreeSet<u32>,
    /// How long this pilot has been driving. Only used to give a car a moment to settle before
    /// it is judged stuck.
    age: f32,
    /// How long the car has been asking for throttle and not moving.
    ///
    /// A pilot with nothing else to try holds full lock and a fifteen-percent throttle against
    /// whatever it is touching, for ever. Three of eight rivals do exactly that.
    ///
    /// **It is close to neutral and the numbers say so.** Held against eight routes with the
    /// manoeuvre disabled it wins on two and loses on one: 4041 goes 5/8 to 2/8 with it, 4061 1/8
    /// to 0/8, and 4102 7/8 to 8/8. It is kept because a driver that tries to back off what it has
    /// hit is a driver and one that grinds for ever is not, not because it moved a number.
    ///
    /// Three tries that came before it are worth having written down: spreading the grid apart
    /// (5/8, 6/8, 5/8 at one, two and four times the spacing), staggering the start (5/8, 4/8, 3/8,
    /// 6/8 at nought to two seconds), and gating recovery on progress rather than time, which
    /// locked it away from the only cars that needed it.
    stalled: f32,
    /// How long is left of a reversing manoeuvre.
    backing: f32,
    /// Nodes this pilot has tried to reach and could not.
    ///
    /// **Observation rather than prediction, and that is the whole idea.** Four rules that tried to
    /// tell in advance which links a car could drive were swept and thrown out — a gradient limit,
    /// a placement cone, a speed-scaled steering lock and a kerb-step test — and each cut real roads
    /// faster than it cut obstacles, because any test sharp enough to catch a half-metre kerb also
    /// catches every crest that is road. A car that has actually failed to get somewhere has
    /// evidence no geometric test has.
    ///
    /// Small and unbounded on purpose: a route has a few dozen junctions and a pilot that gave up
    /// on a dozen of them has bigger problems than the memory.
    ///
    /// Measured over eight routes it is the largest single gain the drivers have had: cars that got
    /// away 32/64 → **41/64**, distance covered 6.5 km → **7.9 km**, and it lands hardest exactly
    /// where the geometric rules could not — the route where nothing moved at all goes from 2 cars
    /// and 40 m to 5 and 170, and another from 0 and 29 m to 2 and 264.
    blocked: Vec<u32>,
    /// Which waypoints of the course the car has actually been near.
    ///
    /// **A progress measure a resync cannot inflate.** `along` is the waypoint *index*, so a pilot
    /// that re-finds the course by jumping its index forward is credited with everything in
    /// between; measured, that turns 5.5 waypoints of real progress into a reported 122.8. This can
    /// only be earned by having been there. Proximity is refuted as a rule for *advancing* — a car
    /// that strays never enters the radius, so the counter stops while the car drives on — and is
    /// exactly right for *counting*, for the same reason read the other way round.
    covered: std::collections::BTreeSet<usize>,
    /// Seconds still to wait before pulling away.
    ///
    /// A grid is four abreast and two deep with 3.5 m across and 5.7 m between the rows, against a
    /// car 1.64 m wide and 4.39 m long — so 1.3 m of clear air front to back. Eight cars given full
    /// throttle at once drive into each other, and a pilot with no traffic model cannot get out of
    /// that: measured, three of eight never leave the grid, and which three changes with the
    /// spacing rather than staying with a slot.
    hold: f32,
    /// The point the steering is actually aimed at, kept only so a harness can see it.
    ///
    /// Where a car is going and which node it holds are different facts, and reading one for the
    /// other cost a session: a trace showing the held node stuck and the car curving away from it
    /// looks like a car ignoring its pilot, and is not — the aim walks *past* the held node, so the
    /// two are supposed to disagree.
    aim: Option<Vec3>,
}

impl Pilot {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Put the pilot on the network at the node nearest the car.
    ///
    /// `facing` is taken and not used, and that is deliberate. Every car on a starting grid is
    /// within a few metres of every other, so nearest puts all eight pilots on the **same** node —
    /// and that node is off to one side of a grid four cars wide. The five that got away were 4.5
    /// to 11.0 m from it and the three that did not were 13.5, 14.3 and 16.4: the failure sorts
    /// perfectly by distance, which looks like a diagnosis and is not one.
    ///
    /// Restricting the choice to a cone in front of the car was the obvious cure and is **refuted**
    /// — swept over half-angles from 90° to 25°, it took the field from five cars away to three,
    /// three, two and two. The correlation is real and the intervention it suggests is wrong, so
    /// the parameter is gone rather than left at a value that reads as tuned.
    pub fn place(&mut self, at: Vec3, facing: Vec3, net: &Network, course: &[Vec3]) {
        let _ = facing;
        self.at = net.nearest(at);
        self.from = None;
        self.passed = 0;
        self.age = 0.0;

        // **Start at the waypoint nearest the grid, not at waypoint zero.** An event outline is a
        // closed ring drawn from wherever its author began, and nothing puts that beginning near
        // the start line. Measured over eight races, how well the field drives sorts *perfectly* by
        // how far waypoint 0 happens to be from the grid: 13 m and 10 m on the two that worked
        // (8/8 and 5/8 cars away), 33 m and 75 m on the two that half worked, and 160, 493, 785 and
        // 844 m on the four where nothing moved at all. A pilot aiming at a point half a kilometre
        // off the course drives off the course, from the first frame.
        self.goal = course
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let d = |p: &Vec3| (p.x - at.x).powi(2) + (p.z - at.z).powi(2);
                d(a.1).total_cmp(&d(b.1))
            })
            .map_or(0, |(i, _)| i);
        // And then the *next* one, because the nearest is the one the grid sits on: aiming at where
        // you already are is a car that turns in place.
        if !course.is_empty() {
            self.goal = (self.goal + 1) % course.len();
        }
        self.line = self.goal;
        self.laps = 0;
        self.blocked.clear();
        self.seen.clear();
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

    /// How many distinct nodes it has visited.
    #[must_use]
    pub fn seen(&self) -> usize {
        self.seen.len()
    }

    /// Which node of the network it is on.
    #[must_use]
    pub fn node(&self) -> Option<u32> {
        self.at
    }

    /// The point the steering is aimed at, once it has driven a frame.
    #[must_use]
    pub fn aim(&self) -> Option<Vec3> {
        self.aim
    }

    /// Which waypoint it is heading for.
    #[must_use]
    pub fn goal(&self) -> usize {
        self.goal
    }

    /// The nodes this pilot has given up on — see the [`Self::blocked`] field.
    ///
    /// Reported because the harvest turned out to be worth counting and worth **not** trusting: 369
    /// nodes over eight routes, 63 of 64 cars leaving at least one, and sharing the ones several
    /// cars agree on is refuted (`ROADMAP.md`).
    #[must_use]
    pub fn given_up(&self) -> &[u32] {
        &self.blocked
    }

    /// How many of the course's waypoints the car has actually driven past.
    #[must_use]
    pub fn covered(&self) -> usize {
        self.covered.len()
    }

    /// Completed laps.
    #[must_use]
    pub fn laps(&self) -> u32 {
        self.laps
    }

    /// How far round the course it is, as waypoints driven since the line — the number a running
    /// order is sorted on.
    ///
    /// Counted from the start rather than from waypoint zero, and added to the lap count, so it
    /// rises monotonically through a race instead of falling back to zero at every lap.
    #[must_use]
    pub fn along(&self, course: usize) -> usize {
        if course == 0 {
            return 0;
        }
        let round = (self.goal + course - self.line) % course;
        self.laps as usize * course + round
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
        // Which way the car is pointing. Wanted by the aim walk and by the steering itself, so it
        // is worked out once here rather than at each.
        let f = flat(facing * Vec3::NEG_Z).normalize_or_zero();

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

        // Advance while the **next** waypoint is nearer than the one held — the same self-limiting
        // rule the network walk uses, and for the same reason.
        //
        // Proximity was the first rule and it is wrong in a way that hides: a car that strays from
        // the course never comes within the radius, so its counter stops while the car keeps
        // driving. Measured, one field covered 1,843 m with the counter reading five waypoints of
        // forty metres — the standings said it had gone 200 m. A running order built on that is
        // fiction.
        if !course.is_empty() {
            let d = |i: usize| flat(course[i % course.len()] - at).length();
            // **"Advance while the one it holds is behind" is refuted here too**, and it was worth
            // trying because the lock is real: over eight routes 39 of 64 cars stop gaining course
            // progress before t=30 s and the field averages 5.2 waypoints of 126. A car that strays
            // past its waypoint sideways never gets nearer the next one than the one it holds, the
            // counter stops, and since the counter is what the pilot steers the network by it then
            // drives at a point behind it for the rest of the race.
            //
            // The reason it fails is the reason it failed on the node walk, and now it is clear:
            // **"behind" cannot terminate on a cycle.** A course is a closed ring, so a car pointed
            // away from it has a whole arc of it behind — the counter walks that arc, wraps, counts
            // a lap, and the early waypoints are behind too. Measured: 92 laps and 5,999 waypoints
            // driven for one car in ninety seconds, and two cars "FINISHED". Whatever the answer to
            // a car that has lost the course is, advancing is not it. See [`Self::lost`].
            for _ in 0..3 {
                let next = (self.goal + 1) % course.len();
                if d(next) >= d(self.goal) {
                    break;
                }
                if next == self.line {
                    self.laps += 1;
                }
                self.goal = next;
            }
        }
        // **Re-finding the course when the counter stops is refuted too.** The lock is real and the
        // idea is the obvious one left after advancing failed: a pilot that has gained no waypoint
        // for a while asks the grid's own question again — nearest waypoint, nearest node — which is
        // bounded by construction and cannot run away. It loses anyway, at every interval swept, on
        // the one measure it cannot inflate. Waypoints actually driven past, over eight routes: 462
        // with it off, **369 at 4 s, 412 at 8 s, 435 at 16 s**; distance 4,129 m → 2,955 / 3,878 /
        // 3,640; cars away 62/64 → 53 / 59 / 62. The only column it wins is junctions, which is
        // exactly the column that re-picking a node inflates.
        //
        // It was worth doing for what it exposed: `along` is the waypoint *index*, so a pilot that
        // jumps its index forward is credited with everything in between, and the resync looked like
        // a twenty-two-fold gain (5.5 → 122.8) until it was measured with [`Self::covered`].
        // What it has actually driven past, as opposed to what its counter says.
        for (i, w) in course.iter().enumerate() {
            if flat(*w - at).length() <= COVERED_WITHIN {
                self.covered.insert(i);
            }
        }

        let toward = course.get(self.goal).copied().unwrap_or(at + facing * Vec3::NEG_Z * 1000.0);

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
        // **"Or the car has driven past it" is refuted, and by a factor of a thousand.** The
        // nearer-than rule visibly cannot let go of a node the car is standing on — no neighbour of
        // a node 1.2 m away is nearer to the car than that — so adding "advance also when the held
        // node is behind" looks like the missing half, and it is not: over eight routes junctions go
        // from 848 to **854,536** and distance covered *falls*, 3,652 → 2,699 m. It is the same
        // runaway the paragraph above records, arrived at from the other side. A node behind the car
        // is usually replaced by another node behind the car, so the loop advances to its cap every
        // frame for the rest of the race. Whatever fixes the sticky node, it is not this.
        let dist = |i: u32| net.node(i).map_or(f32::MAX, |j| flat(j.at - at).length());
        for _ in 0..3 {
            let Some(next) = net.step_avoiding(self.at?, self.from, toward, &self.blocked) else { break };
            if dist(next) >= dist(self.at?) {
                break;
            }
            self.from = self.at;
            self.at = Some(next);
            self.passed += 1;
            self.seen.insert(next);
        }
        let _ = here;

        // Aim: walk the network forward from the held node until far enough away, so the aim point
        // follows the road round a corner instead of cutting across it.
        //
        // **An aim point behind the car is not an aim point.** `walked` is seeded with the distance
        // from the car to its held node, so once a car has drifted further than one lookahead from
        // that node the loop breaks on its first test and the aim is left *on the node* — which by
        // then is behind. Pure pursuit at a point behind the car asks for full lock, and full lock
        // held is a circle: traced on `Paths4041`, car 3 arrives within 1.2 m of node 163, cannot
        // advance off it (no neighbour is nearer to the car than a node it is standing on), watches
        // the aim collapse back onto it, and drives a smooth arc off the edge of the world at 39
        // km/h. Nothing recovers it — the stall manoeuvre wants speed under 0.7 m/s and the car is
        // doing eleven.
        //
        // So the walk keeps going while the aim is behind, not only while it is near.
        //
        // **It is a trade and the trade is worth it.** Over eight routes: cars that got away
        // **52/64 → 62/64**, distance **2,594 → 3,652 m**, junctions **718 → 848**, cars off the
        // world unchanged at 10. Three routes gain heavily (4081 2/8 away and 21 junctions → 8/8 and
        // 159, 4061 4/8 and 237 m → 8/8 and 735, 4121 142 m → 724) and three lose (4002 172 → 51 m,
        // 4102 452 → 333, 4021 388 → 348). Kept because it wins every headline measure at once and
        // because the ratio of junctions to *distinct* nodes does not move (1.29 → 1.30), so the
        // extra junctions are new road rather than a car going round in circles — on 4102, where the
        // distance fell, the ratio goes 1.28 → 1.00 and every junction it takes is somewhere new.
        let look = (speed * LOOKAHEAD_PER_SPEED).clamp(LOOKAHEAD_MIN, LOOKAHEAD_MAX);
        let (mut cur, mut prev) = (self.at?, self.from);
        let mut aim = net.node(cur)?.at;
        let mut walked = flat(aim - at).length();
        for _ in 0..8 {
            if walked >= look && flat(aim - at).dot(f) > 0.0 {
                break;
            }
            let Some(next) = net.step_avoiding(cur, prev, toward, &self.blocked) else { break };
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
            // Give up on where it was going, and take the next best way out of where it came from.
            // Reversing alone only buys another run at the same obstacle.
            if let Some(bad) = self.at {
                if !self.blocked.contains(&bad) {
                    self.blocked.push(bad);
                }
                let from = self.from.unwrap_or(bad);
                if let Some(other) = net.step_avoiding(from, None, toward, &self.blocked) {
                    self.at = Some(other);
                    self.from = Some(from);
                }
            }
        }

        self.aim = Some(aim);
        let to = flat(aim - at).normalize_or_zero();
        let angle = f.cross(to).y.atan2(f.dot(to));
        // A lock that grows with speed was tried here and is **refuted across routes**. The
        // reasoning was good — a raycast vehicle turns by generating lateral force and there is
        // none at rest, and the trace showed stuck cars sitting on full lock — and on the route it
        // was found on it worked: five cars away became six, 165 junctions became 198. On four
        // routes it is not an improvement but a trade: 4041 went 2/8 to 6/8 and 4102 went **8/8 to
        // 5/8**, 275 junctions down to 111. So it is gone rather than kept at a value that reads
        // as tuned.
        let want = (angle * 2.0 / std::f32::consts::PI).clamp(-1.0, 1.0) * STEER_LIMIT;
        self.steer += (want - self.steer) * 0.35;
        let throttle = (1.0 - self.steer.abs() * CORNER_LIFT).max(0.15);

        // **Brake.** Lifting the throttle was the whole speed policy and it is not enough: with no
        // brake a car carries 90 km/h into a corner, runs wide and leaves the road — measured, 20
        // of 64 cars across eight routes ended off the world, falling.
        //
        // The rule is the crudest one that is about the corner rather than about the moment: how
        // hard the car is turning, times how fast it is going, against what it could hold.
        let bs: f32 =
            std::env::var("NFS_BRAKE").ok().and_then(|v| v.parse().ok()).unwrap_or(BRAKE_SPEED);
        let over = (speed / bs) * self.steer.abs();
        let brake = ((over - 1.0) * BRAKE_GAIN).clamp(0.0, 1.0);

        Some(Controls { throttle, brake, steer: self.steer, toggle_auto_shift: false })
    }
}
