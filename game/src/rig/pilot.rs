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
//! No racing line and no braking points: a rival drives the middle of the road at a speed the corner
//! allows. It does now keep a following distance — see `TRAFFIC_LOOK_PER_SPEED` — because having no
//! traffic model at all turned out to be measurable rather than merely untidy, but it will not go
//! round a slower car, so a queue behind one stays a queue.

use super::drive::Controls;
use crate::world::{Ground, Network, Walls};
use gizmo::prelude::*;

/// How high the question "is something standing in the way" is asked at, and how finely the walk
/// between the car and its aim follows the ground under it.
///
/// The same half metre and the same 3 m as everywhere else this city is walked: half a metre
/// clears a kerb and every crest that is road, and 3 m is the spacing at which a walk still sees
/// the ground it is walking on.
const WALL_LIFT: f32 = 0.5;
const WALL_STEP: f32 = 3.0;
/// How many ticks a way-round decision is kept before it is worked out again.
const WALL_EVERY: u8 = 10;
/// How many multiples of the shift the search will widen to before giving up.
///
/// It gives up rather than going wider because past a point the aim is no longer on the same road,
/// and a pilot that has run out of ways round is exactly the case the stall-and-reverse rule below
/// already handles.
const WALL_WIDEST: f32 = 4.0;
/// How long a geometric escape steers the car, in seconds.
///
/// **Swept twice, and the first sweep is why the second one works.** Steering at the most
/// open heading refuted: 799 waypoints with the rule off against 781 / 757 / 742 / 737 at
/// 1, 2, 3.5 and 6 s — monotonically worse, while `away` went 63/64 to **64/64**. The
/// mechanism was freeing every car and then driving it off the course. Weighting the
/// opening by the direction the car actually needs to go turns the same sweep round:
/// **802 / 827 / 833 / 838**, still 64/64 away, distinct nodes 1247 → 1329.
///
/// 3.5 is the middle of the 2-6 s plateau and the best of it on the tiebreaker: most
/// distinct nodes (1329), fewest cars stopped before t=30 (15 against 18), one car off the
/// world. Six seconds covers five more waypoints and gives back both of those.
const ESCAPE_FOR: f32 = 3.5;
/// Headings probed for a way out, how far each is probed, and the height window the ground
/// is allowed to move through along one — the same 8 m `drop_walled` settled on, for the
/// same reason: a straight probe over a crest is not a hole.
const ESCAPE_RAYS: usize = 12;
const ESCAPE_REACH: f32 = 20.0;
const ESCAPE_SLACK: f32 = 8.0;
/// How much clear ground a heading needs before it counts as a way out, and how near the
/// escape point counts as having arrived.
const ESCAPE_MIN: f32 = 8.0;
const ESCAPE_ARRIVED: f32 = 4.0;
/// How far sideways the aim moves per step of that search, in metres.
///
/// Swept over eight routes, and **seven of the eight settings beat having no rule at all** on
/// waypoints driven past — 615, 630, 653, 663, 584, 642, 636 at 2, 4, 5, 6, 7, 8 and 10 m against
/// **605** with the rule off. So the gain is the rule and not the number.
///
/// Five rather than the peak. Six covers most (663) but its neighbour at seven is a hole (584,
/// below the baseline), and the peak beside a hole is what a tuned number looks like. Five is
/// within 1.5 % of it, takes the **most junctions of the whole sweep (1,358) over the most
/// distinct nodes (980)** — which is the tiebreaker between real progress and a car going round in
/// circles — drops nobody off the world, and has good values on both sides of it (630 at four,
/// 663 at six). The hole at seven is two routes going badly at once rather than a trend:
/// `Paths4001` falls 141 → 117 and `Paths4002` 27 → 22, and both recover at eight.
const AIM_CLEAR: f32 = 5.0;

/// How far ahead to aim, as a multiple of speed in m/s, and the bounds that keeps it inside.
///
/// Speed-proportional because a fixed distance is wrong at both ends: short enough to be accurate
/// at 30 km/h is a violent oscillation at 150, and long enough to be smooth at 150 cuts every
/// corner at 30.
/// Seconds of travel the pilot aims ahead.
///
/// **Swept over eight routes and 0.9 holds.** Longer looked promising and was not: 1.5 gives 724
/// waypoints driven past and 1.8 gives 858, against **869** here, with distinct nodes agreeing
/// (1 122 and 1 276 against 1 364).
///
/// The way it looked promising is worth more than the result. A four-route screen — 4001, 4121,
/// 4041, 4102 — had 1.8 winning by 11 % (587 against 528), and the curve across 0.6…5.0 was
/// cleanly unimodal with its peak right there. It was not noise: 1.8 really does gain **+59** on
/// those four and really does lose **−70** on the four that were not screened. The subset was
/// unrepresentative, which a subset is free to be. Screening on half the routes to save time has
/// now produced a false positive twice in a row (see the steering rate above); it is not a cheap
/// version of the sweep, it is a different and unreliable instrument.
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
/// Backwards speed, in m/s, above which a car is held to be moving rather than stalled. 0 turns
/// the exemption off, which is the shape the stall rule was measured in before it existed.
///
/// **Swept over eight routes and every setting beats having no exemption at all** on waypoints
/// driven past: 840, 869, 860 and 851 at 0.2, 0.5, 1.0 and 2.0 m/s against **833** with it off,
/// and distinct nodes agree (1345 / 1364 / 1342 / 1335 against 1329). The curve rises to 0.5 and
/// falls away either side, which is what a threshold should look like when it is finding a real
/// boundary rather than being tuned: too small and a car still crawling backwards is condemned,
/// too large and one genuinely stuck against something gets to hide behind a trickle of reverse.
const STALL_BACK: f32 = 0.5;
/// How near the waypoint being held counts as having reached it, in metres.
///
/// Swept over eight routes and **all eight settings beat having no second arm at all** on waypoints
/// driven past: 707, 703, 780, 799, 788, 678, 697 at 8, 12, 15, 18, 20, 25 and 40 m against **653**
/// with the arm off. 15-20 is a plateau (780 / 799 / 788) bounded by 703 below and 678 above, and
/// eighteen is its middle as well as the best of it — most course covered, furthest driven, most
/// distinct nodes, and the cars that stop before t=30 s down from 30 of 64 to 18.
///
/// **The ceiling has a reason, which is why the plateau ends where it does.** Waypoints are 40 m
/// apart — the step the course is densified to, see [`crate::world::densify`] — so at *half* of that the two arms
/// meet: on an evenly spaced course, being within 20 m of the one you hold already means the next
/// one is further, which is the first arm's own test. Past half the rule stops saying "reached" and
/// starts saying "skip", and the numbers follow — 678 at 25 m, and 40 m drops four cars off the
/// world. Eighteen is just inside that. **It is tied to the spacing, not absolute**: change
/// `NFS_WPSTEP` and this wants moving with it.
const REACHED: f32 = 18.0;
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
///
/// **Ramping this in with speed is refuted, and the reasoning that led there is worth keeping
/// because it was half right.** At full lock the term takes the pedal to 1 − 0.85·0.75 = 0.36,
/// which from rest is about 0.29 m/s²; in the 1.5 s of [`STALL_FOR`] that reaches 0.44 m/s
/// against a [`STALL_SPEED`] of 0.7, so a car that has to turn hard to get going cannot clear its
/// own stall threshold. That arithmetic is correct and it is not the reason the cars were stuck —
/// [`STALL_BACK`] was. Charging the lift only above 4 or 8 m/s, measured on top of that fix,
/// gives **821** and **690** waypoints driven past against **869** with it charged flat. Full
/// pedal at full lock from rest does not rescue a car, it launches one; the two mechanisms looked
/// alike from the trace and only the sweep told them apart.
const CORNER_LIFT: f32 = 0.75;

/// How far ahead to look for another car, as a multiple of speed in m/s, and the floor under it.
///
/// Speed-proportional for the same reason the steering lookahead is: a following distance that is
/// right at 30 km/h is tailgating at 120. The floor is what makes a **standing** grid file out
/// instead of shunting — with no floor, a car doing nothing looks nothing up.
///
/// Swept over eight routes. The measure that decides it is **waypoints actually driven past**,
/// because that is what having no traffic model was costing and because it cannot be inflated:
///
/// | multiple | away | off the world | distance | waypoints driven past | stopped before t=30 |
/// |---|---:|---:|---:|---:|---:|
/// | none | 62/64 | 0 | 4,129 m | 462 | 39 |
/// | 0.6 | 63/64 | 2 | 3,965 | 584 | 33 |
/// | 1.2 | 63/64 | 3 | 3,401 | 526 | 36 |
/// | **2.0** | **63/64** | **0** | **3,611** | **589** | **31** |
/// | 3.0 | 63/64 | 3 | 3,612 | 587 | 34 |
/// | 4.5 | 63/64 | 3 | 3,787 | 572 | 28 |
///
/// Coverage jumps as soon as there is any following distance at all and then sits on a plateau;
/// two is the best of it. **The distance column goes the other way and that is not a surprise** —
/// `furthest` is the leading car of each route, and a leader stuck behind someone is what a
/// following distance costs. The column that says the *field* is driving the course went up 27 %.
///
/// The falls column is chaos rather than signal at this scale (0, 2, 3, 0, 3, 3); two landing on
/// zero is luck, not a property of the number.
const TRAFFIC_LOOK_PER_SPEED: f32 = 2.0;
const TRAFFIC_LOOK_MIN: f32 = 8.0;

/// How far to either side of its own line a car counts as being in the way, in metres.
///
/// The body is 1.64 m wide, so this is a car and a half: wide enough that a rival drifting in the
/// next lane registers before it is a collision, narrow enough that the car on the other
/// carriageway does not.
const TRAFFIC_HALF_WIDTH: f32 = 2.2;

/// How far sideways the aim point is pushed to get round a car in the way, in metres.
///
/// Swept over eight routes on top of the following distance:
///
/// | offset | away | off the world | distance | waypoints driven past | stopped before t=30 |
/// |---|---:|---:|---:|---:|---:|
/// | none | 63/64 | 0 | 3,611 m | 589 | 31 |
/// | **1.5** | **63/64** | **0** | **4,187** | **605** | 32 |
/// | 2.0 | 63/64 | 3 | 4,260 | 605 | 28 |
/// | 2.5 | 63/64 | 4 | 3,559 | 577 | 27 |
/// | 3.0 | 63/64 | 2 | 3,866 | 617 | 27 |
/// | 5.0 | 62/64 | 3 | 4,029 | 609 | 30 |
///
/// Coverage sits on a plateau of 605-617 for anything from 1.5 m up, so the wider offsets buy at
/// most a dozen waypoints — and cost two to four cars off the world for them. One and a half metres
/// is the only setting that moves anything without that, and it is the one with a reading behind it
/// as well as a measurement: a little over half a car's width, which is the smallest shift that
/// actually clears a car in the same lane.
///
/// It also gives back what the following distance cost. Distance covered goes 3,611 → 4,187 m,
/// above even the 4,129 the field managed with no traffic model at all — the queue was the price,
/// and going round is what stops paying it.
const PASS_OFFSET: f32 = 1.5;

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
    /// The sideways shift the aim is carrying to get round something standing in the way, in
    /// metres, positive to the car's left.
    ///
    /// Held between evaluations rather than worked out every step. The search costs a handful of
    /// segment tests, but that is not the reason: a decision that flips side from one frame to the
    /// next is a steering wobble rather than a way round, and the thing being gone round is a
    /// hundred metres of concrete that will still be there in a tenth of a second.
    wall_shift: f32,
    /// Ticks left before the shift is worked out again.
    wall_due: u8,
    /// How many times giving up on a node has re-picked another, and how many of those pointed
    /// within 45° of the one abandoned — that is, sent the car the same way again.
    swaps: usize,
    swaps_same: usize,
    /// Where to drive while escaping, and for how long — a point in the world, not a node.
    ///
    /// **The graph has no answer here and that is measured, not assumed.** Three ways of
    /// choosing a different *node* are refuted in `ROADMAP.md` (blacklist the node, shun its
    /// heading, expire the list), and the reason they all fail is the same: they pick from
    /// branches that all lead back to the obstruction, and 92 % of the alternatives are
    /// already crossed off. Meanwhile the stuck car is standing in open ground — probed at
    /// the moment it gives up, **six to ten of twelve directions are clear for the full
    /// twenty metres of the probe**. So the escape stops asking the graph and asks the city.
    escape: Option<(Vec3, f32)>,
    /// How many escapes were started, and how far the car actually moved during them.
    ///
    /// The rule cut the early-stopper count 18 → 15 and the field's coverage 799 → 833, but
    /// the "nothing else explains it" bucket stayed at eight cars — so for those it is either
    /// not firing or firing and failing, and those want opposite fixes.
    escapes: usize,
    escape_moved: f32,
    /// Where the current escape started, so its displacement can be measured when it ends.
    escape_from: Option<Vec3>,
    /// Links the node being re-picked from had, summed over give-ups, and how many of them pointed
    /// away from the abandoned one. The difference between "no way out" and "no way that is
    /// different".
    swap_links: usize,
    swap_options: usize,
    swap_free: usize,
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
    ///
    /// **Making it forget is refuted too, and that is the surprise.** At a give-up the node being
    /// re-picked from has 4.22 ways out, 1.56 of them point somewhere other than the one abandoned,
    /// and only **0.12** of those are not already on this list — so 92 % of a car's alternatives are
    /// nodes it crossed off earlier, and the list looked like the thing strangling the escape. It is
    /// not: expiring entries after 3, 6, 12 and 25 s loses on every column at every setting —
    /// waypoints 799 → 785 / 758 / 787 / 773, cars away 63/64 → 60-61, distance 5,394 → 5,074-5,218
    /// — while junctions leap to 2,038-2,551 against 1,566 with distinct nodes unmoved. That pairing
    /// is this project's signature for oscillation rather than progress: a car returns to a node it
    /// gave up on, fails there again, and gives up again. The permanence is load-bearing.
    #[must_use]
    pub fn given_up(&self) -> &[u32] {
        &self.blocked
    }

    /// How many times it gave up on a node and took another, and how many of those took it the
    /// **same way again** — the cost of the escape and its yield, side by side.
    ///
    /// Measured because the second number turned out to be almost the first: 94-100 % of the
    /// re-picks point within 45° of the node just abandoned, so a car backs off and drives at the
    /// same thing again. It is structural rather than bad luck — `Network::step_avoiding` ranks by
    /// nearness to the same waypoint, and a blacklist takes away one *node* while the direction
    /// stays exactly as attractive as it was.
    #[must_use]
    pub fn swaps(&self) -> (usize, usize) {
        (self.swaps, self.swaps_same)
    }

    /// How many geometric escapes were started, and the total ground covered during them.
    ///
    /// The pair is the point. An escape that never fires and one that fires constantly and gets
    /// nowhere are the same zero in every summary, and they want opposite fixes — the first a
    /// looser trigger, the second an entirely different mechanism. Measured on 4002 the answer was
    /// 44-126 escapes a car for 0.0-5.5 m, which is how the search moved off the pilot's steering
    /// and onto why the cars could not move at all.
    #[must_use]
    pub fn escapes(&self) -> (usize, f32) {
        (self.escapes, self.escape_moved)
    }

    /// Ways out of the node re-picked from, summed over give-ups, and how many of them pointed
    /// away from the one abandoned.
    #[must_use]
    pub fn swap_choice(&self) -> (usize, usize, usize) {
        (self.swap_links, self.swap_options, self.swap_free)
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
        traffic: &[Vec3],
        sight: Option<(&Walls, &Ground)>,
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
            // **The second way to be done with a waypoint: standing on it.**
            //
            // `Paths4081` needs this and nothing here provides it. Its grid is 8 m from waypoint
            // #124, so every pilot starts holding #125 with the car already on top of it, and the
            // first arm can never fire: the held one is at arm's length and the next — #0, the ring
            // wrapping — is 33 m off, so "is the next nearer" is permanently no. Measured, all
            // eight cars gain **nothing** in ninety seconds while taking seven junctions each,
            // steering at a point that ends up 128 m behind them, and finally reversing. Where the
            // grid is far from a waypoint this never comes up: `Paths4061` starts 42 m out and
            // walks 46 → 47 → 50 → 51 → 53 → 55 → 56 in the same time.
            //
            // **This is not the refuted resync above**, and the difference is the whole of it: that
            // one re-picks the nearest waypoint and can put the goal anywhere, including behind.
            // This only ever steps forward by one, only when the goal being left has been reached,
            // and at most three times a tick — so it cannot walk an arc of the ring the way
            // "advance while the held one is behind" did.
            let reached: f32 =
                std::env::var("NFS_REACHED").ok().and_then(|v| v.parse().ok()).unwrap_or(REACHED);
            for _ in 0..3 {
                let next = (self.goal + 1) % course.len();
                if d(next) >= d(self.goal) && d(self.goal) > reached {
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

        // Which way is sideways. Wanted by the two rules below that move the aim off the line, so
        // it is worked out once.
        let side = Vec3::new(-f.z, 0.0, f.x);

        // **Not through that.** Both ways of dealing with a barrier through the *graph* were swept
        // and refuted — cutting the link and pricing it, `Network::drop_walled` carries the numbers
        // — and the measurement that closed that door opened this one. Asked of the unchanged
        // field: on `Paths4002` something stands between a car and the point it is steering at on
        // **72 %** of the first twenty seconds' samples, against **zero** on the other seven
        // routes. (Over ninety seconds 4081 reads 54 % and 4001 13 %, and both of those are zero
        // early — that is cars that are already lost aiming across buildings, an effect and not a
        // cause. Only 4002 does it from the grid.)
        //
        // So the rule is the same shape as going round a car in front, with one thing it cannot
        // assume: which side. A central reservation has good road on both sides — that is why the
        // file links across it in the first place — so the side is *searched* rather than guessed,
        // widening until the way is clear. Nothing else changes, so the car returns to the line by
        // itself once the concrete has run out.
        let clear: f32 =
            std::env::var("NFS_AIMCLEAR").ok().and_then(|v| v.parse().ok()).unwrap_or(AIM_CLEAR);
        if clear > 0.0 {
            if let Some((walls, ground)) = sight {
                if self.wall_due == 0 {
                    self.wall_due = WALL_EVERY;
                    self.wall_shift = 0.0;
                    if walls.across(ground, at, aim, WALL_LIFT, WALL_STEP) {
                        let mut off = clear;
                        'search: while off <= clear * WALL_WIDEST {
                            for s in [1.0f32, -1.0] {
                                let try_at = aim + side * s * off;
                                if !walls.across(ground, at, try_at, WALL_LIFT, WALL_STEP) {
                                    self.wall_shift = s * off;
                                    break 'search;
                                }
                            }
                            off += clear;
                        }
                    }
                } else {
                    self.wall_due -= 1;
                }
                aim += side * self.wall_shift;
            }
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
        // **A car rolling backwards is not stalled — it is obeying.** `speed` is signed, so a car
        // that has just finished its 1.2 s of reverse is carrying negative speed, and `speed <
        // STALL_SPEED` counts that as a stall from the first tick. Getting from about −1.5 m/s back
        // up through +0.7 takes something over two seconds even at full pedal, and [`STALL_FOR`] is
        // 1.5: the manoeuvre therefore *guarantees* the next stall that starts it again. Measured
        // on 4002, that is what the cars were doing — 35 s of a 90 s race spent in reverse and 115
        // escapes, none of which could ever have completed. Rolling backwards is not standing
        // still, and only standing still is a stall.
        let stall_back: f32 = std::env::var("NFS_STALLBACK")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(STALL_BACK);
        let moving_back = stall_back > 0.0 && speed <= -stall_back;
        if self.age > SETTLE && speed < STALL_SPEED && !moving_back && self.hold <= 0.0 {
            self.stalled += TICK;
        } else {
            self.stalled = 0.0;
        }
        if self.stalled >= STALL_FOR {
            self.stalled = 0.0;
            self.backing = BACK_FOR;
            // **Pick a way out of the city, not out of the node list.** Twelve headings, the
            // ground asked how far it holds along each and the walls asked whether anything
            // stands across it; the longest clear one wins and the car drives at it for a
            // bounded time, graph ignored. When it expires the pilot resumes exactly where it
            // was — nothing about the node walk is disturbed, which is why this can be tried
            // without unpicking any of the three refuted rules.
            let far: f32 =
                std::env::var("NFS_ESCAPE").ok().and_then(|v| v.parse().ok()).unwrap_or(ESCAPE_FOR);
            if far > 0.0 {
                if let Some((walls, ground)) = sight {
                    let mut best = (0.0f32, Vec3::ZERO);
                    for i in 0..ESCAPE_RAYS {
                        let a = i as f32 * std::f32::consts::TAU / ESCAPE_RAYS as f32;
                        let dir = Vec3::new(a.cos(), 0.0, a.sin());
                        let reach = ground
                            .gap_along(at, at + dir * ESCAPE_REACH, ESCAPE_SLACK, 2.0)
                            .unwrap_or(ESCAPE_REACH);
                        // **Hedefe doğru en açık, en açık değil.** İlk sürüm yalnız en uzun
                        // temiz yönü seçiyordu ve ölçüm onu çürüttü: araba kurtuluyor (2 sn'den
                        // itibaren 64/64 kavşak alıyor) ama parkurdan uzaklaşıyor, kapsama
                        // 799'dan 737'ye iniyor. Açıklığı, gidilmesi gereken yönle ağırlıklamak
                        // ikisini birden ister: yarısı mesafe, yarısı yön.
                        let want = flat(toward - at).normalize_or_zero();
                        let score = reach * (0.5 + 0.5 * dir.dot(want));
                        if score > best.0
                            && !walls.across(ground, at, at + dir * reach, WALL_LIFT, WALL_STEP)
                        {
                            best = (score, dir * reach);
                        }
                    }
                    // `best.1` artık yönün kendisi değil, o yöndeki tam vektör.
                    if best.1.length() > ESCAPE_MIN {
                        self.escape = Some((at + best.1, far));
                        self.escapes += 1;
                        self.escape_from = Some(at);
                    }
                }
            }
            // Give up on where it was going, and take the next best way out of where it came from.
            // Reversing alone only buys another run at the same obstacle.
            if let Some(bad) = self.at {
                if !self.blocked.contains(&bad) {
                    self.blocked.push(bad);
                }
                let from = self.from.unwrap_or(bad);
                if let Some(other) = net.step_avoiding(from, None, toward, &self.blocked) {
                    // **Whether giving up actually points anywhere else.** Blacklisting the node
                    // and re-picking is only worth the reverse it costs if the replacement lies in
                    // a different direction from the car; a node the same way on is the same
                    // obstacle with another name, and the field spends a quarter to a half of a
                    // race backing off and returning. Counted rather than assumed, and 45° is the
                    // line: past that the car has genuinely been sent somewhere else.
                    let bearing = |i: u32| {
                        net.node(i).map_or(Vec3::ZERO, |j| flat(j.at - at).normalize_or_zero())
                    };
                    self.swaps += 1;
                    if bearing(bad).dot(bearing(other)) > 0.7 {
                        self.swaps_same += 1;
                    }
                    // **How much choice there was**, as opposed to what was taken. Shunning the
                    // abandoned heading was refuted with a fallback rate that said the escape
                    // almost never has anywhere else to go (`ROADMAP.md`), but that was inferred
                    // rather than measured, and two very different worlds produce it: a node with
                    // one way out, or a node with four ways out that all point the same way. The
                    // next mechanism depends on which, so it is counted here.
                    if let Some(j) = net.node(from) {
                        let away = bearing(bad);
                        self.swap_links += j.links.len();
                        self.swap_options +=
                            j.links.iter().filter(|l| bearing(**l).dot(away) <= 0.7).count();
                        // And of those, the ones not already given up on. The difference between
                        // these two is the whole question: a graph that offers no alternative and
                        // a graph whose alternatives this car has already crossed off are the same
                        // to the escape and want opposite fixes.
                        self.swap_free += j
                            .links
                            .iter()
                            .filter(|l| bearing(**l).dot(away) <= 0.7 && !self.blocked.contains(l))
                            .count();
                    }
                    self.at = Some(other);
                    self.from = Some(from);
                }
            }
        }

        // **The car in front.** Until now a rival would drive into the back of another one, which
        // the module said out loud and which turned out to cost more than it looked: part of the
        // field never leaves a grid that is four abreast with 1.3 m of clear air, and when the
        // pilots' blacklists were harvested for graph truth the nodes several cars agreed on sat a
        // median 7.6-47.7 m from the start line — they were giving up on each other, not on the
        // road.
        //
        // The crudest rule that is about the right thing: anything in a corridor of its own width,
        // ahead, inside a following distance. `traffic` may contain this car's own position — its
        // own `along` is zero, which is not ahead — so the caller does not have to exclude it.
        let look_mul: f32 = std::env::var("NFS_TRAFFIC")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(TRAFFIC_LOOK_PER_SPEED);
        let mut blocking: Option<(f32, f32)> = None;
        let mut blocked_side = 0.0f32;
        if look_mul > 0.0 {
            let look = (speed * look_mul).max(TRAFFIC_LOOK_MIN);
            let near = traffic
                .iter()
                .filter_map(|o| {
                    let d = flat(*o - at);
                    let (fwd, lat) = (d.dot(f), d.dot(side));
                    (fwd > 0.0 && fwd <= look && lat.abs() <= TRAFFIC_HALF_WIDTH).then_some((fwd, lat))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((g, lat)) = near {
                blocking = Some((g, look));
                blocked_side = lat;
            }
        }

        // **Going round it.** Queueing is honest but it is also the whole field stopping behind one
        // stuck car, and the aim point is the cheap place to say "not through that". Shift it away
        // from whichever side the obstacle is on, by more the closer it is; nothing else changes, so
        // the car comes back to the line by itself as soon as the way is clear.
        let pass: f32 =
            std::env::var("NFS_PASS").ok().and_then(|v| v.parse().ok()).unwrap_or(PASS_OFFSET);
        if pass > 0.0 {
            if let Some((g, look)) = blocking {
                let urgency = (1.0 - g / look).clamp(0.0, 1.0);
                aim -= side * blocked_side.signum() * pass * urgency;
            }
        }

        // An escape overrides the graph's aim for its duration, and nothing else: the held
        // node, the blacklist and the waypoint are all left exactly as they were, so the
        // pilot resumes mid-stride when it expires.
        if let Some((to, left)) = self.escape {
            let left = left - TICK;
            if left <= 0.0 || flat(to - at).length() < ESCAPE_ARRIVED {
                if let Some(from) = self.escape_from.take() {
                    self.escape_moved += flat(at - from).length();
                }
                self.escape = None;
            } else {
                self.escape = Some((to, left));
                aim = to;
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
        // How fast the wheel catches up with what the pilot wants.
        //
        // **Swept and refuted, and the way it failed is the point.** Six of eight cars on 4121
        // leave the course at one corner, and a lagging wheel at 65 km/h through 50° is exactly
        // what running wide looks like — so raising this was the obvious fix. On that route it
        // looked like one: 0.5 took the cars losing the course there from **6 of 8 to 3 of 8**,
        // and the number never leaving it from 1 to 3.
        //
        // It did not survive the field. Over eight routes, 0.5 gives **773** waypoints driven past
        // against **869**, and 1 149 distinct nodes against 1 364 — while the corner metric it was
        // supposed to fix barely moves (53 of 64 cars leave the course either way, 11 never leave
        // against 12). The one-route result was the route, not the rule. This is what the sweep is
        // for, and it is worth remembering that a 2× improvement on a single route can mean
        // nothing at all.
        self.steer += (want - self.steer) * 0.35;
        let mut throttle = (1.0 - self.steer.abs() * CORNER_LIFT).max(0.15);

        // **Brake.** Lifting the throttle was the whole speed policy and it is not enough: with no
        // brake a car carries 90 km/h into a corner, runs wide and leaves the road — measured, 20
        // of 64 cars across eight routes ended off the world, falling.
        //
        // The rule is the crudest one that is about the corner rather than about the moment: how
        // hard the car is turning, times how fast it is going, against what it could hold.
        let bs: f32 =
            std::env::var("NFS_BRAKE").ok().and_then(|v| v.parse().ok()).unwrap_or(BRAKE_SPEED);
        let over = (speed / bs) * self.steer.abs();
        let mut brake = ((over - 1.0) * BRAKE_GAIN).clamp(0.0, 1.0);

        // **The car in front.** Until now a rival would drive into the back of another one, which
        // the module said out loud and which turned out to cost more than it looked: three of eight
        // never leave a grid that is four abreast with 1.3 m of clear air, and when the pilots'
        // blacklists were harvested for graph truth the nodes several cars agreed on sat a median
        // 7.6-47.7 m from the start line — they were giving up on each other, not on the road.
        //
        // The crudest rule that is about the right thing: anything in a corridor of its own width,
        // ahead, inside a following distance. `traffic` may contain this car's own position — its
        // own `along` is zero, which is not ahead — so the caller does not have to exclude it.
        if let Some((g, look)) = blocking {
            // Nothing at the far end of the look, everything at nose to tail.
            let close = (1.0 - g / look).clamp(0.0, 1.0);
            throttle *= 1.0 - close;
            brake = brake.max((close - 0.5).max(0.0) * 2.0);
        }

        Some(Controls { throttle, brake, steer: self.steer, toggle_auto_shift: false })
    }
}
