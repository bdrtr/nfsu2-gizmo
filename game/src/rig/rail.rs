//! Rivals on rails: placed on the race line, never steered at it.
//!
//! ## Why this exists
//!
//! M4's plan said it in so many words — *kinematic rivals, not simulated* — and the open question
//! beside it recorded that all three lenses recommended it. The project built a simulated pilot
//! instead, because a real `VehicleController` on every rival is what would prove the engine's car
//! model. Ten days and some forty refuted rules later (`ROADMAP.md`, 2026-08-12 to 08-22) no rival
//! finishes a race: the field reaches about 2 km of a 15 km circuit, and the last decision on the
//! record is that the best package of constants **does not generalise** — +19 % on the eight
//! routes it was chosen on, −8 % to +0.5 % on 24 it had never seen.
//!
//! What the pilot work did establish is that most of the difficulty is the *steering at* a line,
//! not the line: a car that aims at waypoints from where it is fights its own lag, the course's
//! kinks and every junction it passes. A rail has none of that. It is **placed** at a distance along
//! a line, so it cannot leave it, and the only question left is how fast it may go there — which is
//! arithmetic over the line's curvature and what the car can do.
//!
//! The pilot stays. It is the instrument for the engine's car model, and `nfs_sim` keeps driving it.
//!
//! ## What a rail is honest about
//!
//! - **The car's limits are the player's car's.** [`Limits`] is measured by driving that car on a
//!   flat plane ([`super::dyno`]), not typed in: a rail accelerates no faster, brakes no harder and
//!   corners no tighter than the car the player is in.
//! - **The line is the roads.** It is the walked ring ([`crate::world::race_ring`]), smoothed only
//!   as far as the road under it allows, standing on the road surface it passes over.
//! - **What it does not model** is grade (a hill neither slows it nor speeds it), weight transfer,
//!   and the interplay of braking and cornering — it brakes in a straight line before a corner and
//!   takes the corner at the limit. Those are the simplifications of every racing-line solver, and
//!   they make a rail slightly faster than a perfect driver, which [`PACE`] pays back.

use super::dyno::Limits;
use crate::world::Ground;
use gizmo::math::Mat3;
use gizmo::prelude::*;

/// Spacing of the line's samples, in metres.
pub const SAMPLE: f32 = 2.0;

/// How far smoothing may move a sample off the walked ring, in metres.
///
/// The ring is the network's nodes, which sit on the road's middle and turn at a junction the way a
/// map does — a right angle, not a curve. Smoothing rounds those corners, and this bounds by how
/// much: at a 90° corner a shift of `d` allows a radius of about `2.4 d`. Whatever it allows, the
/// road mask has the last word — a sample smoothing carries off the road goes back onto it.
/// `NFS_RAILSHIFT=<m>` overrides it.
///
/// **Measured against 8 m over the eight sweep routes**, and 8 is faster but dirtier: laps 7-11 %
/// shorter (4001 3:45 → 3:19), while the places the line steps steeper than 50 % go 11 → 25 and the
/// steps with a wall between two samples 117 → 132. The road mask stops a sample leaving the road;
/// it does not stop one sliding onto a kerb or across a reservation, and more room is more of that.
const SHIFT_MAX: f32 = 4.0;
/// A turn sharper than this between two consecutive 2 m steps is a cusp — the line going back the
/// way it came — and no road does that. `cos 120°`.
const CUSP_COS: f32 = -0.5;
/// How far back an open line is extended before its first point when no grid says where to, in
/// metres; and how far behind the grid's last row the lead-in starts when one does.
///
/// A sprint's line begins at the outline's first corner, and the grid stands somewhere behind it —
/// on `Paths4102` every slot projected onto the line's very first metre, and eight cars started in
/// one place. A fixed extension was the first fix and it was not enough: on `Paths4107` the grid is
/// 134-140 m behind the first corner, on `Paths4105` 53 m behind and 15 m to the side. So the lead-in
/// is drawn from the grid itself.
const LEAD_IN: f32 = 80.0;
const LEAD_BEHIND: f32 = 20.0;
/// How near the interpolated height a drivable surface must be for a sample with no road under it to
/// stand on it instead, in metres.
const SETTLE_WITHIN: f32 = 2.0;
/// What leaving a sample out of the height solve costs, in metres of climb.
///
/// A road under a bridge is often missing from the road mask exactly where the deck is over it, so
/// at those samples the only road on offer is the deck, 10-20 m up. Made to take it, the solve
/// climbs onto the deck and back down within a few metres — measured on `Paths4121`, grades of
/// 85-142 % under two bridges. Allowed to skip, it bridges the gap at the height it was at, for a
/// cost this small per sample against the tens of metres the jump would cost.
const SKIP_COST: f32 = 0.5;
/// The longest run of samples the height solve may skip, in samples — 60 m, wider than a deck.
const SKIP_MAX: usize = 30;
/// What leaving out a sample at either *end* of the line costs. Far more than in the middle: a gap
/// in the middle still climbs from one side to the other, but one at an end climbs nothing, and
/// priced like the middle the cheapest line would be one that stands on almost no samples at all.
const END_SKIP_COST: f32 = 1000.0;
/// How far either side a cusp is judged over, in samples: a turn-round that steps sideways on the
/// way is two right angles a sample apart, and only a 4 m baseline sees it as the reversal it is.
const CUSP_SPAN: usize = 2;
/// Smoothing passes per round, and rounds of putting back on the road what smoothing took off it.
const SMOOTH_PASSES: usize = 200;
const REPAIR_ROUNDS: usize = 4;
/// Half the baseline curvature is read over, in samples. Eight metres each side: long enough that
/// the road surface's tessellation is not a corner, short enough that a junction still is.
const CURVE_SPAN: usize = 4;
/// A grade steeper than this between two samples is a step, not a road, and [`LineReport::steps`]
/// counts it. Half: no street in Bayview is half as steep as this.
const STEP_GRADE: f32 = 0.5;
/// Half-window of the height smoothing, in samples.
const HEIGHT_SPAN: usize = 3;
/// How far either side of a sample the tangent is read over, in metres.
const TANGENT_SPAN: f32 = 3.0;

/// The fraction of the line's limit speed a rival drives at, front of the grid first.
///
/// **A choice, not a reading**, and kept here so the next person meets it. A rail at 1.0 drives the
/// theoretical limit of the car on the line — braking at the last metre, every corner at the tyre's
/// edge, never a mistake — which no person does. The spread is what makes a race of it: the field
/// separates instead of running nose to tail, and the order changes because faster cars start
/// further back.
pub const PACE: [f32; 8] = [0.90, 0.92, 0.89, 0.93, 0.91, 0.94, 0.88, 0.95];

/// How much a rival's pace breathes over a lap, as a fraction of it: ±2.5 %, each car on its own
/// rhythm ([`Rail::wobble`]). A rail at a fixed fraction of the limit is a metronome — it gains or
/// loses on the car beside it at exactly the same rate on every straight, and a pass that has not
/// happened by the first corner never does. People are faster here and slower there.
const WOBBLE: f32 = 0.025;
/// The wavelength of that breathing is between these, metres along the line.
const WOBBLE_MIN: f32 = 600.0;
const WOBBLE_SPREAD: f32 = 500.0;

/// The catch-up ("lastik bant"): a rival this far behind the player drives up to [`CATCH_UP`]
/// faster, one this far ahead up to [`CATCH_DOWN`] slower, linear in between, metres.
///
/// What every arcade racer does and NFSU2 did: without it a race is decided by the first corner —
/// a player who gets away is alone for ten minutes, one who falls behind never sees the field
/// again. Small on purpose: at 300 m it is a few km/h, which closes a gap over a lap and does not
/// teleport anybody.
const CATCH_SPAN: f32 = 300.0;
const CATCH_UP: f32 = 0.05;
const CATCH_DOWN: f32 = 0.05;
/// Whatever pace, wobble and catch-up add up to, a rail never drives closer to the line's limit
/// than this — 1.0 is the car's theoretical best on the line, which no person drives.
const PACE_CEIL: f32 = 0.98;

/// How far the body leans per g of cornering and pitches per g of braking or acceleration, radians.
///
/// **Read off the player's own car on the bench**, not chosen: 1.42° at 1.23 g on the circle,
/// 1.38° of dive under a 13.8 m/s² stop (`ROADMAP.md`, 2026-11-02) — so a rival leans exactly as
/// much as the same car does when the player drives it. Small, because the game's own anti-roll bars
/// are stiff; a road car would lean three times as far.
const ROLL_PER_G: f32 = 1.15 * std::f32::consts::PI / 180.0;
const PITCH_PER_G: f32 = 0.98 * std::f32::consts::PI / 180.0;
/// How quickly the body follows, seconds: a sprung mass does not lean the instant the load arrives.
const ATTITUDE_LAG: f32 = 0.25;
const G: f32 = 9.81;

/// How long a car pushed by a collision rolls on before it brakes back down to its pace, seconds,
/// and how fast it slows meanwhile, m/s² — rolling resistance and drag, no brakes.
const DAZED: f32 = 0.8;
const COAST: f32 = 1.0;

/// How fast a sideways shove from a collision dies away, per second: a car knocked across slides
/// for about a second and is then back under the lane logic.
const SHOVE_DECAY: f32 = 3.0;
/// The furthest a collision may knock a rail off its line, metres either side. Past this it would be
/// off the road the line was laid on, and a rail has no idea how to drive there.
const SHOVE_LANE: f32 = 4.0;

/// How far either side of the line a rival settles, in metres. Two lanes, so two cars can be side
/// by side and one can pass another.
pub const LANE: f32 = 1.6;
/// How fast a rival moves across, in m/s.
const LANE_RATE: f32 = 1.2;
/// The space a car takes on the line, in metres: its length, and the width two cars need to pass.
const CAR_LEN: f32 = 4.6;
const CAR_WIDE: f32 = 2.0;
/// The gap a rival keeps to the car in front, in metres: a fixed margin plus this many seconds of
/// its own speed.
const FOLLOW_MIN: f32 = 2.0;
const FOLLOW_S: f32 = 0.4;
/// How far ahead a rival looks for traffic, in metres.
const TRAFFIC_LOOK: f32 = 80.0;
/// How far either side of a car's last place [`RaceLine::track`] looks for it, and how far from the
/// line it may be and still be on it, in metres.
const TRACK_WINDOW: f32 = 80.0;
const TRACK_OFF: f32 = 30.0;
/// How long a rival sits behind a slower car before it tries the other lane, in seconds.
const PASS_AFTER: f32 = 1.0;
/// How fast a car that is already too close drops back, in m/s per metre it is inside the gap.
const DROP_BACK: f32 = 1.0;

/// How far before the end of a sprint's line its finish is, in metres.
///
/// The line ends where the data does, and there is no road on it past that point. A rail stops at
/// the end; if the finish were there too, the first car home would stand on the line and every car
/// behind it would queue short of it and never finish. Eighty metres holds a whole grid stopped in
/// two lanes with room to spare.
pub const SPRINT_RUNOUT: f32 = 80.0;

/// What [`RaceLine::build`] did to the ring, and what the result is like — the numbers that say
/// whether a rival on it is on a road.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LineReport {
    /// Samples on the line.
    pub samples: usize,
    /// Length in metres.
    pub length: f32,
    /// Samples smoothing had to put back towards the ring because it had carried them off the road.
    pub repaired: usize,
    /// Samples with no road under them in plan, after repair. The ring's own chord legs — where no
    /// road joined two corners — are most of these; smoothing never makes one.
    pub off_road: usize,
    /// Samples with no road surface under them, whose height is interpolated from the nearest
    /// samples that have one.
    pub bridged: usize,
    /// Places the line climbs or drops steeper than [`STEP_GRADE`] between two samples — where the
    /// height solve chose between decks badly.
    pub steps: usize,
    /// The steepest grade between two samples, as rise over run.
    pub steepest: f32,
    /// The tightest radius on the line, in metres.
    pub tightest: f32,
    /// The slowest the line can be driven anywhere, in m/s.
    pub slowest: f32,
    /// One lap (or the whole sprint) at the limit, from a flying start, in seconds.
    pub lap_time: f32,
}

/// The race's ring as a line a car can be *on*: evenly sampled, smoothed within the road, standing
/// on the road surface, with the fastest the car can go at every point of it.
#[derive(Debug, Clone)]
pub struct RaceLine {
    /// The samples, in the Gizmo frame.
    at: Vec<Vec3>,
    /// Distance from the first sample to each, in metres.
    s: Vec<f32>,
    /// The fastest the car can be at each sample, in m/s.
    speed: Vec<f32>,
    /// Curvature at each sample, 1/m, unsigned.
    curve: Vec<f32>,
    /// Whether the line closes on itself — a circuit.
    closed: bool,
    /// Total length, including the closing segment of a circuit.
    length: f32,
    /// How it was built.
    pub report: LineReport,
}

impl RaceLine {
    /// Turn a ring of waypoints into a line, for a car with `limits`.
    ///
    /// `roads` is what smoothing may not leave and what heights are read from first; `ground` is
    /// asked where the roads have nothing. `closed` is the event's own circuit flag.
    #[must_use]
    ///
    /// `grid` is the starting grid and the way it faces. An open line is extended back to behind its
    /// last row, so every car on it stands on the line; a circuit passes its grid anyway.
    pub fn build(
        ring: &[Vec3],
        closed: bool,
        grid: Option<(&[Vec3], Vec3)>,
        roads: &Ground,
        ground: &Ground,
        limits: &Limits,
    ) -> Option<Self> {
        let mut ring = ring.to_vec();
        if !closed {
            let first = ring.first().copied();
            let next = ring.iter().skip(1).find(|p| first.is_some_and(|f| (**p - f).length() > 0.5)).copied();
            let lead = match grid {
                Some((slots, heading)) if !slots.is_empty() => {
                    let h = Vec3::new(heading.x, 0.0, heading.z).normalize_or_zero();
                    // Only when the line starts *ahead* of the last row. On most sprints the
                    // outline's first corner is behind the grid — 110 m on `Paths4101`, 316 m on
                    // `Paths4211` — and the grid already stands on the first leg.
                    slots
                        .iter()
                        .copied()
                        .min_by(|a, b| a.dot(h).total_cmp(&b.dot(h)))
                        .filter(|rear| first.is_some_and(|f| (f - *rear).dot(h) > 0.0))
                        .map(|rear| rear - h * LEAD_BEHIND)
                }
                _ => first.zip(next).map(|(a, b)| a + Vec3::new(a.x - b.x, 0.0, a.z - b.z).normalize_or_zero() * LEAD_IN),
            };
            if let Some(p) = lead {
                ring.insert(0, p);
            }
        }
        let orig = resample(&ring, closed);
        if orig.len() < 3 {
            return None;
        }
        let on_road = |p: Vec3| !roads.heights_at(p.x, p.z).is_empty();
        let (mut at, repaired) = smooth(&orig, closed, on_road);
        let off_road = at.iter().filter(|p| !on_road(**p)).count();
        let bridged = stand(&mut at, closed, roads, ground);
        let mut line = Self::from_points(at, closed, limits)?;
        line.report.repaired = repaired;
        line.report.off_road = off_road;
        line.report.bridged = bridged;
        Some(line)
    }

    /// A line from samples that already stand where they should — the arithmetic half of
    /// [`Self::build`], and what the tests drive.
    #[must_use]
    pub fn from_points(at: Vec<Vec3>, closed: bool, limits: &Limits) -> Option<Self> {
        let n = at.len();
        if n < 3 {
            return None;
        }
        let segs = if closed { n } else { n - 1 };
        let seg = |i: usize| (at[(i + 1) % n] - at[i]).length();
        let mut s = Vec::with_capacity(n);
        let mut acc = 0.0;
        for i in 0..n {
            s.push(acc);
            if i < segs {
                acc += seg(i);
            }
        }
        let length = acc;

        // Curvature: the circle through a sample and the samples CURVE_SPAN either side of it, in
        // plan. A circuit wraps; an open line's ends read as straight.
        let pick = |i: isize| -> Option<Vec3> {
            if closed {
                Some(at[i.rem_euclid(n as isize) as usize])
            } else if i < 0 || i >= n as isize {
                None
            } else {
                Some(at[i as usize])
            }
        };
        let k = CURVE_SPAN as isize;
        let curve: Vec<f32> = (0..n as isize)
            .map(|i| match (pick(i - k), pick(i), pick(i + k)) {
                (Some(a), Some(b), Some(c)) => menger(a, b, c),
                _ => 0.0,
            })
            .collect();

        // The fastest the tyres allow at each sample, then what the engine and the brakes allow on
        // the way in and out — the classic two passes. A circuit goes round twice so the line's
        // start is not a place the car arrives at from rest.
        let mut speed: Vec<f32> = curve
            .iter()
            .map(|c| if *c > 1e-6 { (limits.grip / c).sqrt().min(limits.top) } else { limits.top })
            .collect();
        let rounds = if closed { 2 } else { 1 };
        for _ in 0..rounds {
            for i in 0..segs {
                let j = (i + 1) % n;
                let ds = seg(i);
                let reach = (speed[i] * speed[i] + 2.0 * limits.accel_at(speed[i]) * ds).sqrt();
                speed[j] = speed[j].min(reach);
            }
        }
        for _ in 0..rounds {
            for i in (0..segs).rev() {
                let j = (i + 1) % n;
                let ds = seg(i);
                let reach = (speed[j] * speed[j] + 2.0 * limits.brake * ds).sqrt();
                speed[i] = speed[i].min(reach);
            }
        }

        let mut steepest = 0.0f32;
        let mut steps = 0usize;
        let mut lap_time = 0.0f32;
        for i in 0..segs {
            let j = (i + 1) % n;
            let run = Vec3::new(at[j].x - at[i].x, 0.0, at[j].z - at[i].z).length();
            if run > 0.1 {
                let grade = (at[j].y - at[i].y).abs() / run;
                steepest = steepest.max(grade);
                steps += usize::from(grade > STEP_GRADE);
            }
            let v = 0.5 * (speed[i] + speed[j]);
            if v > 0.0 {
                lap_time += seg(i) / v;
            }
        }
        let tightest = curve.iter().copied().fold(0.0f32, f32::max);
        let report = LineReport {
            samples: n,
            length,
            steepest,
            steps,
            tightest: if tightest > 0.0 { 1.0 / tightest } else { f32::INFINITY },
            slowest: speed.iter().copied().fold(f32::INFINITY, f32::min),
            lap_time,
            ..LineReport::default()
        };
        Some(Self { at, s, speed, curve, closed, length, report })
    }

    /// Total length in metres.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.length
    }

    /// Whether the line is a circuit.
    #[must_use]
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// The samples, for anything that wants to draw the line.
    #[must_use]
    pub fn points(&self) -> &[Vec3] {
        &self.at
    }

    /// A distance along the line brought onto it: wrapped round a circuit, clamped to an open
    /// line's ends.
    fn wrap(&self, d: f32) -> f32 {
        if self.closed {
            d.rem_euclid(self.length.max(1e-3))
        } else {
            d.clamp(0.0, self.length)
        }
    }

    /// The segment a distance falls in, and how far along it.
    fn locate(&self, d: f32) -> (usize, usize, f32) {
        let d = self.wrap(d);
        let n = self.at.len();
        let i = match self.s.binary_search_by(|x| x.total_cmp(&d)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
        .min(n - 1);
        let j = if self.closed { (i + 1) % n } else { (i + 1).min(n - 1) };
        let end = if j == 0 { self.length } else { self.s[j] };
        let span = end - self.s[i];
        let f = if span > 1e-6 { ((d - self.s[i]) / span).clamp(0.0, 1.0) } else { 0.0 };
        (i, j, f)
    }

    /// The point on the line at a distance along it.
    #[must_use]
    pub fn point(&self, d: f32) -> Vec3 {
        let (i, j, f) = self.locate(d);
        self.at[i].lerp(self.at[j], f)
    }

    /// The fastest the car can be at a distance along the line, in m/s.
    #[must_use]
    pub fn speed_at(&self, d: f32) -> f32 {
        let (i, j, f) = self.locate(d);
        self.speed[i] + (self.speed[j] - self.speed[i]) * f
    }

    /// The curvature at a distance along the line, 1/m.
    #[must_use]
    pub fn curve_at(&self, d: f32) -> f32 {
        let (i, j, f) = self.locate(d);
        self.curve[i] + (self.curve[j] - self.curve[i]) * f
    }

    /// How hard the line turns at a distance along it, 1/m, **signed**: positive to the left — the
    /// sign the vehicle controller gives a steering angle.
    #[must_use]
    pub fn turn_at(&self, d: f32) -> f32 {
        const SPAN: f32 = 4.0;
        let (a, b) = (self.tangent(d - SPAN), self.tangent(d + SPAN));
        let (a, b) = (Vec3::new(a.x, 0.0, a.z).normalize_or_zero(), Vec3::new(b.x, 0.0, b.z).normalize_or_zero());
        a.cross(b).y.atan2(a.dot(b)) / (2.0 * SPAN)
    }

    /// The unit direction of travel at a distance along the line, pitch included.
    #[must_use]
    pub fn tangent(&self, d: f32) -> Vec3 {
        let ahead = self.point(d + TANGENT_SPAN);
        let behind = self.point(d - TANGENT_SPAN);
        let t = (ahead - behind).normalize_or_zero();
        if t == Vec3::ZERO {
            Vec3::NEG_Z
        } else {
            t
        }
    }

    /// How far a race over this line counts to, in metres, for a race that starts at `from`: one
    /// lap of a circuit, or a sprint from its start to its finish — [`SPRINT_RUNOUT`] short of the
    /// end of the line, so the cars home first have somewhere to stop.
    #[must_use]
    pub fn course(&self, from: f32) -> usize {
        if self.closed {
            self.length.round() as usize
        } else {
            (self.length - SPRINT_RUNOUT - from).max(1.0).round() as usize
        }
    }

    /// Where a car that is *not* on the line is along it: `at` projected onto the stretch within
    /// [`TRACK_WINDOW`] of where it was last (`last`, unwrapped), as an unwrapped distance and how
    /// far to the right. `None` when the car is further than [`TRACK_OFF`] from that stretch — it
    /// has left the race's line, and its last place is kept rather than guessed.
    ///
    /// Looked for near where it was rather than anywhere, because a circuit crosses itself and a
    /// city runs streets side by side: the nearest piece of line overall is often another lap's.
    #[must_use]
    pub fn track(&self, at: Vec3, last: f32) -> Option<(f32, f32)> {
        let (s, lateral) = self.project(at, None, Some((self.wrap(last), TRACK_WINDOW)))?;
        let plan = |p: Vec3| Vec3::new(p.x, 0.0, p.z);
        if (plan(self.point(s)) - plan(at)).length() > TRACK_OFF {
            return None;
        }
        // Unwrap round a circuit: of the laps `s` could be on, the one nearest `last`.
        let s = if self.closed {
            let base = last - self.wrap(last);
            [s + base - self.length, s + base, s + base + self.length]
                .into_iter()
                .min_by(|a, b| (a - last).abs().total_cmp(&(b - last).abs()))
                .unwrap_or(s)
        } else {
            s
        };
        Some((s, lateral))
    }

    /// The places worth looking at: every sample where the line steps steeper than
    /// [`STEP_GRADE`] to the next, or curves tighter than `radius` metres. `(sample, grade, radius)`.
    #[must_use]
    pub fn flags(&self, radius: f32) -> Vec<(usize, f32, f32)> {
        let n = self.at.len();
        let segs = if self.closed { n } else { n - 1 };
        (0..segs)
            .filter_map(|i| {
                let j = (i + 1) % n;
                let (a, b) = (self.at[i], self.at[j]);
                let run = Vec3::new(b.x - a.x, 0.0, b.z - a.z).length();
                let grade = if run > 0.1 { (b.y - a.y).abs() / run } else { 0.0 };
                let r = if self.curve[i] > 1e-6 { 1.0 / self.curve[i] } else { f32::INFINITY };
                (grade > STEP_GRADE || r < radius).then_some((i, grade, r))
            })
            .collect()
    }

    /// Consecutive samples with a wall between them at car height — where a rail would drive
    /// *through* something. The measure that tells a line through a junction the city did not name
    /// as road (harmless) from a line through a building (not).
    #[must_use]
    pub fn through_walls(&self, walls: &crate::world::Walls, ground: &Ground) -> Vec<usize> {
        let n = self.at.len();
        let segs = if self.closed { n } else { n - 1 };
        (0..segs).filter(|&i| walls.across(ground, self.at[i], self.at[(i + 1) % n], 0.5, 3.0)).collect()
    }

    /// Where on the line a point is: the distance along it and how far to the right of it, in
    /// metres. `heading`, when given, refuses a stretch running the other way — a circuit passes
    /// the same street twice and a grid is on only one of them. `near` limits the search to a
    /// window of the line.
    #[must_use]
    pub fn project(&self, at: Vec3, heading: Option<Vec3>, near: Option<(f32, f32)>) -> Option<(f32, f32)> {
        let n = self.at.len();
        let segs = if self.closed { n } else { n - 1 };
        let mut best: Option<(f32, f32, f32)> = None; // (plan distance, s, lateral)
        for i in 0..segs {
            let j = (i + 1) % n;
            let (a, b) = (self.at[i], self.at[j]);
            if let Some((centre, half)) = near {
                let mut gap = (self.s[i] - centre).abs();
                if self.closed {
                    gap = gap.min(self.length - gap);
                }
                if gap > half {
                    continue;
                }
            }
            let d = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
            let len2 = d.length_squared();
            if len2 < 1e-8 {
                continue;
            }
            if let Some(h) = heading {
                let h = Vec3::new(h.x, 0.0, h.z).normalize_or_zero();
                if d.normalize().dot(h) < 0.3 {
                    continue;
                }
            }
            let rel = Vec3::new(at.x - a.x, 0.0, at.z - a.z);
            let f = (rel.dot(d) / len2).clamp(0.0, 1.0);
            let off = rel - d * f;
            let dist = off.length();
            if best.is_none_or(|b| dist < b.0) {
                // Right of travel: forward × up, with forward along d.
                let right = d.normalize().cross(Vec3::Y);
                best = Some((dist, self.s[i] + f * len2.sqrt(), off.dot(right)));
            }
        }
        best.map(|(_, s, lat)| (s, lat))
    }
}

/// The ring in the order the race drives it: the outline's, or the outline's turned round.
/// Returns the ring and whether it was reversed.
///
/// **The grid's heading is the reliable half, and the outline's order is not.** `ROADMAP.md` left
/// the grid's front and back unsolved, and the first version of this trusted the outline instead
/// and turned grids round to match it. Measured over the 42 sprints in the install, that has it
/// backwards:
///
/// - **39** have their grid near the outline's first corner (8-320 m) and the file's heading along
///   the ring's way out of it (+0.63 to +1.00);
/// - **3** — `Paths4104`, `4126` and `4127` — have their grid 5-42 m from the outline's **last**
///   corner, and the heading exactly against the way the ring arrives there (−0.97 to −1.00). On
///   4126 the first corner is 2.4 km away; no race starts 2.4 km from its grid.
///
/// So the heading is right in all 42, and three outlines are written finish first. A sprint starts
/// at whichever end its grid stands at. A circuit has no ends: it is turned round when the ring
/// runs against the grid where it passes it, unless the ring also passes that street the other way
/// within [`GRID_SAME_STREET`].
#[must_use]
pub fn orient_ring(ring: &[Vec3], closed: bool, slots: &[Vec3], heading: Vec3) -> (Vec<Vec3>, bool) {
    let n = ring.len();
    if n < 2 || slots.is_empty() {
        return (ring.to_vec(), false);
    }
    let plan = |a: Vec3, b: Vec3| Vec3::new(b.x - a.x, 0.0, b.z - a.z);
    let centre = slots.iter().copied().sum::<Vec3>() / slots.len() as f32;
    let reversed = if closed {
        let h = Vec3::new(heading.x, 0.0, heading.z).normalize_or_zero();
        let mut near: Vec<(f32, f32)> = Vec::with_capacity(n); // (plan distance, direction · heading)
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let d = plan(a, b);
            let len2 = d.length_squared();
            if len2 < 1e-6 {
                continue;
            }
            let rel = plan(a, centre);
            let f = (rel.dot(d) / len2).clamp(0.0, 1.0);
            near.push(((rel - d * f).length(), d.normalize().dot(h)));
        }
        let nearest = near.iter().map(|x| x.0).fold(f32::INFINITY, f32::min);
        !near.iter().any(|(dist, dot)| *dist <= nearest + GRID_SAME_STREET && *dot > 0.3)
    } else {
        plan(centre, ring[n - 1]).length() < plan(centre, ring[0]).length()
    };
    let mut out = ring.to_vec();
    if reversed {
        out.reverse();
    }
    (out, reversed)
}

/// How much further than the nearest ring segment a segment facing the grid's way may be and still
/// count as the street the grid is on, in metres.
const GRID_SAME_STREET: f32 = 15.0;

/// The circle through three points in plan, as a curvature in 1/m.
fn menger(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let (ab, bc, ca) = (b - a, c - b, a - c);
    let plan = |v: Vec3| (v.x * v.x + v.z * v.z).sqrt();
    let cross = ab.x * bc.z - ab.z * bc.x;
    let denom = plan(ab) * plan(bc) * plan(ca);
    if denom < 1e-6 {
        0.0
    } else {
        2.0 * cross.abs() / denom
    }
}

/// Re-sample the ring at [`SAMPLE`], dropping a circuit's repeated last point and any spur.
fn resample(ring: &[Vec3], closed: bool) -> Vec<Vec3> {
    let mut pts = ring.to_vec();
    let plan = |a: Vec3, b: Vec3| (b.x - a.x).hypot(b.z - a.z);
    if closed && pts.len() > 2 {
        // The outline of a circuit ends where it began; close it explicitly either way so the last
        // segment is sampled like every other, and then drop the copy of the first point.
        if plan(pts[0], pts[pts.len() - 1]) > 0.5 {
            pts.push(pts[0]);
        }
    }
    let mut out = crate::world::respace(&pts, SAMPLE);
    if closed && out.len() > 2 && plan(out[0], out[out.len() - 1]) < SAMPLE * 0.5 {
        out.pop();
    }
    if unspur(&mut out, closed) > 0 {
        // Removing a spur leaves its two sides' points crowded where they met; even them out.
        if closed {
            out.push(out[0]);
        }
        out = crate::world::respace(&out, SAMPLE);
        if closed && out.len() > 2 && plan(out[0], out[out.len() - 1]) < SAMPLE * 0.5 {
            out.pop();
        }
    }
    out
}

/// Take out every place the line goes back the way it came. Returns how many samples went.
///
/// **The ring has them, and they are the tightest corners on it.** Where one leg of the walk ends at
/// a node a little past the node the next leg starts from, the ring runs on, turns round, comes back
/// over the same metres and turns round again: on `Paths4001` an 11 m out-and-back at
/// (858, 531) that smoothing folded into a 1.6 m radius — a 14 km/h corner on a straight street.
/// Removing the tip of a cusp makes its neighbours the new tip, so this repeats until none is left,
/// and the spur is gone from both sides.
fn unspur(pts: &mut Vec<Vec3>, closed: bool) -> usize {
    let mut removed = 0usize;
    loop {
        let n = pts.len();
        if n < 4 {
            return removed;
        }
        let dir = |a: Vec3, b: Vec3| Vec3::new(b.x - a.x, 0.0, b.z - a.z).normalize_or_zero();
        let mut keep = vec![true; n];
        let mut any = false;
        for i in 0..n {
            // CUSP_SPAN either side where the line has them, fewer near an open line's ends.
            let k = CUSP_SPAN.min(if closed { CUSP_SPAN } else { i.min(n - 1 - i) });
            if k == 0 {
                continue;
            }
            let (a, c) = if closed { ((i + n - k) % n, (i + k) % n) } else { (i - k, i + k) };
            // Never two neighbours in one pass, so a spur shortens from its tip inward.
            if i > 0 && !keep[i - 1] {
                continue;
            }
            let (d0, d1) = (dir(pts[a], pts[i]), dir(pts[i], pts[c]));
            if d0 == Vec3::ZERO || d1 == Vec3::ZERO || d0.dot(d1) < CUSP_COS {
                keep[i] = false;
                any = true;
            }
        }
        if !any {
            return removed;
        }
        let mut k = 0;
        pts.retain(|_| {
            k += 1;
            keep[k - 1]
        });
        removed += keep.iter().filter(|x| !**x).count();
    }
}

/// Round the ring's corners, never by more than [`SHIFT_MAX`] and never off a road it was on.
///
/// Laplacian smoothing in plan, bounded to a disc round each original sample; then every sample the
/// smoothing carried from road to no road is moved back towards where it was until it has road
/// under it again, and pinned there for the next round. Returns the samples and how many were
/// pinned.
fn smooth(orig: &[Vec3], closed: bool, on_road: impl Fn(Vec3) -> bool) -> (Vec<Vec3>, usize) {
    let shift_max: f32 =
        std::env::var("NFS_RAILSHIFT").ok().and_then(|v| v.parse().ok()).unwrap_or(SHIFT_MAX);
    let n = orig.len();
    let mut p = orig.to_vec();
    let mut pinned = vec![false; n];
    let neighbours = |i: usize| -> Option<(usize, usize)> {
        if closed {
            Some(((i + n - 1) % n, (i + 1) % n))
        } else if i == 0 || i == n - 1 {
            None
        } else {
            Some((i - 1, i + 1))
        }
    };
    for _ in 0..REPAIR_ROUNDS {
        for _ in 0..SMOOTH_PASSES {
            let prev = p.clone();
            for i in 0..n {
                if pinned[i] {
                    continue;
                }
                let Some((a, b)) = neighbours(i) else { continue };
                let q = (prev[a] + prev[b]) * 0.5;
                let mut d = Vec3::new(q.x - orig[i].x, 0.0, q.z - orig[i].z);
                let len = d.length();
                if len > shift_max {
                    d *= shift_max / len;
                }
                p[i] = Vec3::new(orig[i].x + d.x, orig[i].y, orig[i].z + d.z);
            }
        }
        let mut moved = 0usize;
        for i in 0..n {
            if pinned[i] || on_road(p[i]) || !on_road(orig[i]) {
                continue;
            }
            moved += 1;
            pinned[i] = true;
            let mut t = 0.75f32;
            loop {
                let q = orig[i] + (p[i] - orig[i]) * t;
                if t <= 0.0 || on_road(q) {
                    p[i] = q;
                    break;
                }
                t -= 0.25;
            }
        }
        if moved == 0 {
            break;
        }
    }
    let repaired = pinned.iter().filter(|x| **x).count();
    (p, repaired)
}

/// Stand every sample on the road under it, and smooth the result. Returns how many samples had
/// no road under them and took their height from their neighbours.
///
/// **The whole line is solved at once, not sample by sample.** The first version followed the
/// surface nearest the last sample's height and held that height where nothing was near it — and a
/// hold never ends, because once the road has gone on climbing or falling nothing is near the held
/// height again. On `Paths4001` it lost the road at sample 1,800 and stood the rest of the lap,
/// 2.5 km, at 52 m with the road between 7 and 30 m under it: 43 % of the line in the air.
///
/// So this is [`least_climb`]: the sequence of road surfaces that climbs least in total, chosen over
/// the whole line, which is how the route's own paths are stood on the city
/// ([`crate::world::route::follow`]) and for the same reason — an overpass is a choice no single
/// sample can make. Roads only: over every
/// drivable triangle least-climb is degenerate, because the flat shelf under the city climbs by
/// nothing at all. Samples with no road under them are bridged by [`crate::world::route::fill`].
/// Only when the line has no road under it anywhere — a venue whose ground is not named as road — is
/// the drivable ground asked instead.
fn stand(at: &mut [Vec3], closed: bool, roads: &Ground, ground: &Ground) -> usize {
    let n = at.len();
    let ask = |g: &Ground| -> Vec<Vec<f32>> { at.iter().map(|p| g.heights_at(p.x, p.z)).collect() };
    let mut cands = ask(roads);
    if cands.iter().all(Vec::is_empty) {
        cands = ask(ground);
    }
    let mut h = least_climb(&cands);
    let solved: Vec<bool> = h.iter().map(Option::is_some).collect();
    let bridged = crate::world::route::fill(&mut h);
    if h.iter().all(Option::is_none) {
        // Nothing anywhere: keep the ring's own heights, which came from the network's solve.
        return n;
    }
    let mut h: Vec<f32> = h.into_iter().map(Option::unwrap_or_default).collect();
    // **A bridged sample stands on the ground where there is ground at about its height.** Venues
    // name their surface otherwise — the airport is `TRN_RDP_*`, which has no `ROAD` in it — and on
    // `STREAML4RB` 86 % of the line has no road under it. Interpolated across a whole venue, a line
    // floats over every dip and cuts into every rise. Only a surface near the interpolation is
    // taken, so the shelf under the city, tens of metres down, never is.
    //
    // **And only where it keeps the line continuous from both sides.** Taken alone, a surface 2 m
    // off the interpolation next to a sample on the road is a 2 m step in 2 m: over the 105 races
    // the places the line steps steeper than 50 % went 70 → 128 when the first version of this went
    // in. So a forward pass and a backward pass each accept a surface only within a gentle step of
    // the sample before it, and a sample moves only where the two agree. `NFS_RAILSETTLE=0` turns
    // the whole thing off.
    if std::env::var("NFS_RAILSETTLE").map_or(true, |v| v != "0") {
        let max_step = STEP_GRADE * SAMPLE * 0.8;
        let near = |i: usize, want: f32| -> Option<f32> {
            ground
                .heights_at(at[i].x, at[i].z)
                .into_iter()
                .filter(|y| (y - want).abs() <= SETTLE_WITHIN)
                .min_by(|a, b| (a - want).abs().total_cmp(&(b - want).abs()))
        };
        let mut fwd = h.clone();
        for i in 0..n {
            if solved[i] {
                continue;
            }
            if let Some(y) = near(i, h[i]) {
                if i == 0 || (y - fwd[i - 1]).abs() <= max_step {
                    fwd[i] = y;
                }
            }
        }
        let mut bwd = h.clone();
        for i in (0..n).rev() {
            if solved[i] {
                continue;
            }
            if let Some(y) = near(i, h[i]) {
                if i + 1 == n || (y - bwd[i + 1]).abs() <= max_step {
                    bwd[i] = y;
                }
            }
        }
        for i in 0..n {
            if !solved[i] && fwd[i] == bwd[i] {
                h[i] = fwd[i];
            }
        }
    }
    let k = HEIGHT_SPAN as isize;
    for (i, p) in at.iter_mut().enumerate() {
        let mut sum = 0.0;
        let mut count = 0.0;
        for d in -k..=k {
            let j = i as isize + d;
            let j = if closed {
                j.rem_euclid(n as isize)
            } else if j < 0 || j >= n as isize {
                continue;
            } else {
                j
            };
            sum += h[j as usize];
            count += 1.0;
        }
        p.y = sum / count;
    }
    bridged
}

/// The sequence of candidate heights that climbs least in total, where a sample may also be left
/// out for [`SKIP_COST`] — up to [`SKIP_MAX`] in a row — and filled from its neighbours afterwards.
///
/// [`crate::world::route::follow`] with one more choice. That one must take a surface at every
/// node that has one, which is right for a route's nodes 29 m apart; on a line sampled every 2 m
/// the road mask has holes exactly under bridge decks, and the deck is then the only surface on
/// offer. See [`SKIP_COST`].
fn least_climb(cands: &[Vec<f32>]) -> Vec<Option<f32>> {
    // Only samples with a road under them take part; the rest are gaps for `fill` whatever the solve
    // does. Counting them as skips would make a long roadless stretch — a chord leg — break the
    // chain, and the line before it would be left with no heights at all.
    let live: Vec<usize> = (0..cands.len()).filter(|&i| !cands[i].is_empty()).collect();
    let m = live.len();
    let c = |p: usize| &cands[live[p]];
    // best[p][k]: the cheapest way to stand live sample p on candidate k; from[p][k] where it came
    // from, as (live index, candidate).
    let mut best: Vec<Vec<f32>> = Vec::with_capacity(m);
    let mut from: Vec<Vec<Option<(usize, usize)>>> = Vec::with_capacity(m);
    for p in 0..m {
        let (row, came): (Vec<f32>, Vec<Option<(usize, usize)>>) = c(p)
            .iter()
            .map(|h| {
                let mut b = END_SKIP_COST * p as f32;
                let mut f = None;
                for back in 1..=SKIP_MAX.min(p) {
                    let q = p - back;
                    let skip = SKIP_COST * (back - 1) as f32;
                    for (j, (g, cost_g)) in c(q).iter().zip(&best[q]).enumerate() {
                        let cost = cost_g + (h - g).abs() + skip;
                        if cost < b {
                            b = cost;
                            f = Some((q, j));
                        }
                    }
                }
                (b, f)
            })
            .unzip();
        best.push(row);
        from.push(came);
    }
    // The cheapest end, paying for whatever live samples it leaves unvisited after it.
    let mut end: Option<(usize, usize)> = None;
    let mut end_cost = f32::INFINITY;
    for (p, row) in best.iter().enumerate() {
        for (k, b) in row.iter().enumerate() {
            let cost = b + END_SKIP_COST * (m - 1 - p) as f32;
            if cost < end_cost {
                end_cost = cost;
                end = Some((p, k));
            }
        }
    }
    let mut out = vec![None; cands.len()];
    while let Some((p, k)) = end {
        out[live[p]] = Some(c(p)[k]);
        end = from[p][k];
    }
    out
}

/// One rival's place on a [`RaceLine`].
#[derive(Debug, Clone)]
pub struct Rail {
    /// Distance along the line, in metres, unwrapped: a circuit's laps add to it.
    s: f32,
    /// Speed along the line, in m/s.
    v: f32,
    /// How far right of the line it is now, and where it is going, in metres.
    lane: f32,
    lane_goal: f32,
    /// The fraction of the line's limit it drives at — see [`PACE`].
    pace: f32,
    /// Where the race starts, as a distance along the line: progress is measured from here.
    from: f32,
    /// The line's length and shape, kept so progress can be read without the line.
    length: f32,
    closed: bool,
    /// Seconds spent held up by the car in front.
    held_for: f32,
    /// The speed traffic allows this step, if anything in front is slower.
    cap: Option<f32>,
    /// Whether a car alongside stops it moving across this step.
    boxed: bool,
    /// How fast it is moving across, m/s, positive to the right — what turns its nose a little
    /// into a lane change instead of sliding it sideways.
    lane_v: f32,
    /// This car's pace rhythm: wavelength (m) and phase (rad) — see [`WOBBLE`].
    wobble: (f32, f32),
    /// The catch-up multiplier [`catch_up`] last set, 1.0 when nothing has.
    catch_up: f32,
    /// The body's lean, radians: roll (positive = left side down) and pitch (positive = nose up),
    /// each lagging the load that causes it by [`ATTITUDE_LAG`].
    roll: f32,
    pitch: f32,
    /// Sideways speed a collision left it with, m/s, positive to the right; dies away at
    /// [`SHOVE_DECAY`] while the lane logic steers it back.
    shove: f32,
    /// Seconds left before a car a collision pushed faster than it wants to go starts braking back
    /// down — see [`DAZED`].
    dazed: f32,
}

/// Something on the line a rail must not drive into that is not itself a rail — the player.
#[derive(Debug, Clone, Copy)]
pub struct Obstacle {
    /// Distance along the line, metres.
    pub s: f32,
    /// How far right of the line, metres.
    pub lane: f32,
    /// Speed along the line, m/s.
    pub v: f32,
}

impl Rail {
    /// Put a rival on the line where a grid slot is.
    ///
    /// `from` is where the race starts — the front of the grid, projected onto the line — and the
    /// slot is looked for within a hundred metres of it, so a circuit that comes back past its own
    /// grid cannot place a car on the wrong lap of it.
    #[must_use]
    pub fn place(line: &RaceLine, slot: Vec3, heading: Vec3, from: f32, pace: f32) -> Self {
        let (s, lateral) = line
            .project(slot, Some(heading), Some((from, 100.0)))
            .or_else(|| line.project(slot, None, Some((from, 100.0))))
            .unwrap_or((from, 0.0));
        // Unwrapped relative to the start, so a car on the row behind is a few metres *before* it
        // rather than a lap after it.
        let mut rel = s - from;
        if line.closed && rel > line.length * 0.5 {
            rel -= line.length;
        }
        Self {
            s: from + rel,
            v: 0.0,
            lane: lateral,
            lane_goal: if lateral < 0.0 { -LANE } else { LANE },
            pace,
            from,
            length: line.length,
            closed: line.closed,
            held_for: 0.0,
            cap: None,
            boxed: false,
            lane_v: 0.0,
            // From the pace, which is per-car: any per-car number would do, and this one is already
            // in hand without widening the signature.
            wobble: (
                WOBBLE_MIN + (pace * 104_729.0).fract() * WOBBLE_SPREAD,
                (pace * 7_919.0).fract() * std::f32::consts::TAU,
            ),
            catch_up: 1.0,
            roll: 0.0,
            pitch: 0.0,
            shove: 0.0,
            dazed: 0.0,
        }
    }

    /// What a collision did to this car ([`super::bump`]): a change of velocity `dv` and a
    /// displacement `moved`, both world space. Along the line they become speed and distance;
    /// across it, a sideways drift that dies away and a step out of its lane it then steers back
    /// from.
    pub fn bump(&mut self, line: &RaceLine, dv: Vec3, moved: Vec3) {
        let t = line.tangent(self.s);
        let along = Vec3::new(t.x, 0.0, t.z).normalize_or_zero();
        let right = along.cross(Vec3::Y);
        self.s += moved.dot(along);
        self.lane = (self.lane + moved.dot(right)).clamp(-SHOVE_LANE, SHOVE_LANE);
        self.v = (self.v + dv.dot(along)).max(0.0);
        self.shove += dv.dot(right);
        self.dazed = DAZED;
    }

    /// The car's velocity, world space: along the line at its speed, plus however fast it is
    /// moving across.
    #[must_use]
    pub fn velocity(&self, line: &RaceLine) -> Vec3 {
        let t = line.tangent(self.s);
        let right = Vec3::new(t.x, 0.0, t.z).normalize_or_zero().cross(Vec3::Y);
        t * self.v + right * self.lane_v
    }

    /// The same rail already moving at `v` m/s — for a bench that wants a car ahead at a known
    /// speed rather than one starting from the grid.
    #[must_use]
    pub fn with_speed(mut self, v: f32) -> Self {
        self.v = v.max(0.0);
        self
    }

    /// The fraction of the line's limit this car drives at here: its [`PACE`], breathing with
    /// [`WOBBLE`], times the catch-up, never past [`PACE_CEIL`].
    #[must_use]
    pub fn pace_here(&self) -> f32 {
        let (len, phase) = self.wobble;
        let breath = 1.0 + WOBBLE * (std::f32::consts::TAU * self.s / len + phase).sin();
        (self.pace * breath * self.catch_up).min(PACE_CEIL)
    }

    /// The body's lean now, (roll, pitch) radians — see [`Self::pose`].
    #[must_use]
    pub fn attitude(&self) -> (f32, f32) {
        (self.roll, self.pitch)
    }

    /// A whole grid on the line: one rail per slot, `paces` handed out in slot order.
    ///
    /// Two things are decided here once rather than per car. **Where the race starts**: the front
    /// of the grid, projected onto the line — whichever slot lands furthest along it — so every car
    /// measures its progress from the same place and the back rows start a few metres short of it.
    /// **Which lane each car keeps**: the grid's own columns, split at their median, so two cars
    /// that start side by side keep to their own sides instead of both settling into whichever lane
    /// their offset happens to lean towards — which is two cars in one place.
    ///
    /// `None` when the grid is nowhere near the line.
    #[must_use]
    pub fn grid(line: &RaceLine, slots: &[Vec3], heading: Vec3, paces: &[f32]) -> Option<Vec<Self>> {
        let first = *slots.first()?;
        let pole = line.project(first, Some(heading), None)?.0;
        let along = |s: f32| {
            let mut d = s - pole;
            if line.closed && d > line.length * 0.5 {
                d -= line.length;
            } else if line.closed && d < -line.length * 0.5 {
                d += line.length;
            }
            pole + d
        };
        let from = slots
            .iter()
            .filter_map(|p| line.project(*p, Some(heading), Some((pole, 100.0))))
            .map(|(s, _)| along(s))
            .fold(pole, f32::max);
        let mut field: Vec<Self> = slots
            .iter()
            .enumerate()
            .map(|(k, p)| Self::place(line, *p, heading, from, paces.get(k % paces.len().max(1)).copied().unwrap_or(1.0)))
            .collect();
        let mut lateral: Vec<f32> = field.iter().map(|r| r.lane).collect();
        lateral.sort_by(f32::total_cmp);
        let middle = lateral.get(lateral.len() / 2).copied().unwrap_or(0.0);
        for r in &mut field {
            r.lane_goal = if r.lane < middle { -LANE } else { LANE };
        }
        Some(field)
    }

    /// What traffic did to this car on the last step: the speed it was capped to, whether a car
    /// alongside held it in its lane, how long it has been held up, and the lane it is heading for.
    #[must_use]
    pub fn traffic(&self) -> (Option<f32>, bool, f32, f32) {
        (self.cap, self.boxed, self.held_for, self.lane_goal)
    }

    /// Where the race starts, as a distance along the line.
    #[must_use]
    pub fn from(&self) -> f32 {
        self.from
    }

    /// Distance along the line, unwrapped.
    #[must_use]
    pub fn s(&self) -> f32 {
        self.s
    }

    /// Speed along the line, m/s.
    #[must_use]
    pub fn speed(&self) -> f32 {
        self.v
    }

    /// How far right of the line, metres.
    #[must_use]
    pub fn lane(&self) -> f32 {
        self.lane
    }

    /// Metres driven since the start, negative on the rows behind it until they reach it.
    #[must_use]
    pub fn progress(&self) -> f32 {
        self.s - self.from
    }

    /// Completed laps of a circuit; always zero on a sprint.
    #[must_use]
    pub fn laps(&self) -> u32 {
        if self.closed && self.length > 0.0 {
            (self.progress().max(0.0) / self.length).floor() as u32
        } else {
            0
        }
    }

    /// Metres since the start, the unit a race of rails counts in.
    #[must_use]
    pub fn along(&self) -> usize {
        self.progress().max(0.0) as usize
    }

    /// One step. `hold` keeps the car on its mark — the countdown.
    pub fn step(&mut self, line: &RaceLine, limits: &Limits, dt: f32, hold: bool) {
        if hold {
            self.v = 0.0;
            return;
        }
        let mut want = line.speed_at(self.s) * self.pace_here();
        if let Some(c) = self.cap {
            want = want.min(c);
        }
        if !self.closed {
            // A sprint's line ends at the finish, and there is nothing to drive on past it.
            let left = (line.length - self.s).max(0.0);
            want = want.min((2.0 * limits.brake * left).sqrt());
        }
        let v0 = self.v;
        // Pushed faster than it wants to go, it rolls on for a moment before it brakes: a driver hit
        // from behind does not stand on the brakes in the same instant, and a rail that did would
        // hand the push straight back — the shove would be a spring, and the car a wall again.
        let slowing = if self.dazed > 0.0 { COAST } else { limits.brake };
        self.dazed = (self.dazed - dt).max(0.0);
        self.v = if self.v < want {
            (self.v + limits.accel_at(self.v) * dt).min(want)
        } else {
            (self.v - slowing * dt).max(want)
        };
        // The loads the body leans under: along the line from the change of speed, across it from
        // speed² × curvature (positive = to the left, the way the line turns).
        let along = (self.v - v0) / dt.max(1e-6);
        let across = self.v * self.v * line.turn_at(self.s);
        let k = 1.0 - (-dt / ATTITUDE_LAG).exp();
        // A left turn pushes the body out to the right: right side down, which is negative roll.
        self.roll += (-ROLL_PER_G * across / G - self.roll) * k;
        // Braking dips the nose (negative pitch), accelerating lifts it.
        self.pitch += (PITCH_PER_G * along / G - self.pitch) * k;
        self.s += self.v * dt;
        if !self.closed && self.s >= line.length {
            // Stopped by the end of the line, and stopped means a speed of nothing: a car parked
            // here still reporting the speed it arrived at reads to the car behind as one that is
            // driving away, and it closes up into it.
            self.s = line.length;
            self.v = 0.0;
        }
        let before = self.lane;
        if !self.boxed {
            let reach = LANE_RATE * dt;
            self.lane += (self.lane_goal - self.lane).clamp(-reach, reach);
        }
        if self.shove != 0.0 {
            self.lane = (self.lane + self.shove * dt).clamp(-SHOVE_LANE, SHOVE_LANE);
            self.shove *= (-SHOVE_DECAY * dt).exp();
            if self.shove.abs() < 0.01 {
                self.shove = 0.0;
            }
        }
        self.lane_v = (self.lane - before) / dt.max(1e-6);
    }

    /// Where the car stands: the chassis position and rotation, with the chassis `ride` metres
    /// over the road.
    ///
    /// Forward is the line's tangent, pitch included, and up is square to it in the vertical plane
    /// through it — so a car climbs a ramp nose up. On top of that the body leans as the player's
    /// own car does under the same loads ([`ROLL_PER_G`], [`PITCH_PER_G`]): out of a corner, nose
    /// down under the brakes, back on the throttle. It used to stand bolt upright through all of
    /// it, which read as a model on a stick.
    #[must_use]
    pub fn pose(&self, line: &RaceLine, ride: f32) -> (Vec3, Quat) {
        let t = line.tangent(self.s);
        let right = t.cross(Vec3::Y).normalize_or_zero();
        let right = if right == Vec3::ZERO { Vec3::X } else { right };
        let up = right.cross(t).normalize_or_zero();
        let flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
        let at = line.point(self.s) + flat * self.lane + up * ride;
        // Columns are where the car's own axes go: X right, Y up, Z back (forward is −Z).
        let rot = Quat::from_mat3(&Mat3::from_cols(right, up, -t));
        // Into a lane change nose first: the car points where it is going, not where the line
        // does. Positive about up is a turn to the left, and moving right is a turn to the right.
        let yaw = -self.lane_v.atan2(self.v.max(1.0));
        // In the body's own frame: roll about its back axis (+Z), pitch about its right (+X).
        let lean = Quat::from_rotation_z(self.roll) * Quat::from_rotation_x(self.pitch);
        (at, Quat::from_axis_angle(up, yaw) * rot * lean)
    }

    /// The angle a car with this wheelbase would have its front wheels at to drive the line here,
    /// radians, positive to the left — for drawing the wheels, which a placed car has nothing
    /// else to turn.
    #[must_use]
    pub fn steer(&self, line: &RaceLine, wheelbase: f32) -> f32 {
        (wheelbase * line.turn_at(self.s)).atan()
    }
}

/// Sets every rail's catch-up against the player at `player_s` (distance along the same line,
/// unwrapped the way a rail's is): behind the player faster, ahead slower — see [`CATCH_SPAN`].
pub fn catch_up(field: &mut [Rail], player_s: f32) {
    for r in field {
        let gap = ((r.s - player_s) / CATCH_SPAN).clamp(-1.0, 1.0);
        r.catch_up = 1.0 - gap * if gap < 0.0 { CATCH_UP } else { CATCH_DOWN };
    }
}

/// Step a field of rails: who is held up by whom, who moves over to pass, then everyone moves.
///
/// `others` is everything on the line that is not a rail — the player — which a rail treats as
/// traffic and nothing more: it slows for it and goes round it, and it does not know it is racing
/// it.
pub fn advance(
    field: &mut [Rail],
    others: &[Obstacle],
    line: &RaceLine,
    limits: &Limits,
    dt: f32,
    hold: bool,
) {
    let len = line.length.max(1e-3);
    let ahead = |from: f32, to: f32| -> f32 {
        if line.closed {
            (to - from).rem_euclid(len)
        } else {
            to - from
        }
    };
    // Each car with the lane it is in *and* the one it is moving to: two cars changing lanes at once
    // cross, and a check against where each is now sees neither in the other's way.
    let cars: Vec<(Obstacle, f32)> = field
        .iter()
        .map(|r| (Obstacle { s: r.s, lane: r.lane, v: r.v }, r.lane_goal))
        .chain(others.iter().map(|o| (*o, o.lane)))
        .collect();
    let overlaps = |a: f32, b: f32| (a - b).abs() < CAR_WIDE;
    // Decided for every car against the field as it stands, then applied — so no car's decision
    // sees another's from the same step.
    let decided: Vec<(Option<f32>, bool, f32, f32)> = field
        .iter()
        .enumerate()
        .map(|(i, me)| {
            // The nearest car in front in my lane, as (gap, its speed).
            let leader = cars
                .iter()
                .enumerate()
                .filter(|(j, (c, goal))| {
                    *j != i
                        && (overlaps(c.lane, me.lane)
                            || overlaps(*goal, me.lane)
                            || overlaps(c.lane, me.lane_goal))
                })
                .map(|(j, (c, _))| (j, ahead(me.s, c.s), c.v))
                // Level with each other, the lower index counts as in front, so of two cars at one
                // place exactly one waits — at the end of a sprint every car arrives at the same place.
                .filter(|(j, d, _)| (*d > 0.0 || (*d == 0.0 && *j < i)) && *d < TRAFFIC_LOOK)
                .map(|(_, d, v)| (d, v))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let cap = leader.map(|(d, v)| {
                // The speed from which the brakes still stop at the leader's speed before the gap is
                // gone — so a rail can close on a car but never into it. Already inside the gap, it
                // drops *below* the leader's speed until it is out again: matching it would hold the
                // two cars exactly as close as they are, which on the grid is overlapping.
                let room = d - CAR_LEN - FOLLOW_MIN - FOLLOW_S * me.v;
                if room >= 0.0 {
                    (v * v + 2.0 * limits.brake * room).sqrt()
                } else {
                    (v + DROP_BACK * room).max(0.0)
                }
            });
            let want = line.speed_at(me.s) * me.pace_here();
            let held = cap.is_some_and(|c| c < want * 0.97);
            let goal = me.lane_goal;
            let (s_me, v_me) = (me.s, me.v);
            // A car alongside — less than a car's length either way — that the move across would bring
            // within a car's width: wait for it. The grid is four wide and the line has two lanes, so
            // every start merges two cars into each lane, and without this they merge into each other.
            let step = (me.lane_goal - me.lane).clamp(-LANE_RATE * dt, LANE_RATE * dt);
            let next = me.lane + step;
            let boxed = step != 0.0
                && cars.iter().enumerate().any(|(j, (c, _))| {
                    if j == i {
                        return false;
                    }
                    let mut d = (c.s - me.s).abs();
                    if line.closed {
                        d = d.rem_euclid(len).min(len - d.rem_euclid(len));
                    }
                    d < CAR_LEN + 0.5
                        && (next - c.lane).abs() < CAR_WIDE
                        && (next - c.lane).abs() < (me.lane - c.lane).abs()
                });
            let mut held_for = if held { me.held_for + dt } else { 0.0 };
            let mut lane_goal = goal;
            if held_for > PASS_AFTER {
                // Try the other lane — free when every car in it is far enough away that neither has to
                // brake harder than the brakes can for the move: a car behind must be able to settle
                // to this car's speed before it is on it, and this car to the speed of one in front.
                //
                // A fixed window was the first version and it cut cars up: on `Paths4101` a car slowing
                // for the end of the line moved across 20 m in front of one closing at 30 km/h more,
                // outside a window that only knew about distance.
                let other = -goal;
                let need = |faster: f32, slower: f32, own: f32| {
                    let closing = ((faster * faster - slower * slower) / (2.0 * limits.brake)).max(0.0);
                    CAR_LEN + FOLLOW_MIN + FOLLOW_S * own + closing
                };
                let free = cars.iter().enumerate().all(|(j, (c, their_goal))| {
                    if j == i || (!overlaps(c.lane, other) && !overlaps(*their_goal, other)) {
                        return true;
                    }
                    let d = ahead(s_me, c.s);
                    let d = if line.closed && d > len * 0.5 { d - len } else { d };
                    if d >= 0.0 {
                        d >= need(v_me, c.v, v_me)
                    } else {
                        -d >= need(c.v, v_me, c.v)
                    }
                });
                if free {
                    lane_goal = other;
                    held_for = 0.0;
                }
            }
            (cap, boxed, held_for, lane_goal)
        })
        .collect();
    for (r, (cap, boxed, held_for, lane_goal)) in field.iter_mut().zip(decided) {
        r.cap = cap;
        r.boxed = boxed;
        r.held_for = held_for;
        r.lane_goal = lane_goal;
    }
    for r in field.iter_mut() {
        r.step(line, limits, dt, hold);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> Limits {
        // A plausible car: 4 m/s² falling to nothing at 50 m/s.
        let accel: Vec<f32> = (0..50).map(|k| 4.0 * (1.0 - k as f32 / 50.0)).collect();
        Limits { accel, top: 50.0, brake: 8.0, grip: 8.0 }
    }

    fn circle(r: f32, n: usize) -> Vec<Vec3> {
        (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                Vec3::new(r * a.cos(), 0.0, r * a.sin())
            })
            .collect()
    }

    /// A circle is one corner all the way round: every sample's limit is √(grip · r), and the
    /// length is the circumference. If either is off, every lap time the race prints is.
    #[test]
    fn a_circle_is_driven_at_the_tyres_limit_for_its_radius() {
        let r = 40.0;
        let line = RaceLine::from_points(circle(r, 126), true, &limits()).unwrap();
        assert!((line.length() - std::f32::consts::TAU * r).abs() < 0.5, "{}", line.length());
        let want = (8.0f32 * r).sqrt();
        for i in 0..126 {
            let v = line.speed_at(i as f32 * 2.0);
            assert!((v - want).abs() / want < 0.02, "{v} against {want}");
        }
    }

    /// The braking pass: approaching a hairpin, speed never falls faster than the brakes can take
    /// it off, and never rises faster than the engine can put it on.
    #[test]
    fn the_profile_respects_the_brakes_and_the_engine() {
        let l = limits();
        // 400 m straight, then a tight turn: a quarter circle of 10 m.
        let mut pts: Vec<Vec3> = (0..200).map(|i| Vec3::new(0.0, 0.0, -(i as f32) * 2.0)).collect();
        for i in 1..=8 {
            let a = i as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
            pts.push(Vec3::new(10.0 - 10.0 * a.cos(), 0.0, -398.0 - 10.0 * a.sin()));
        }
        for i in 1..100 {
            pts.push(Vec3::new(10.0 + i as f32 * 2.0, 0.0, -408.0));
        }
        let line = RaceLine::from_points(pts, false, &l).unwrap();
        let n = line.at.len();
        for i in 0..n - 1 {
            let ds = (line.at[i + 1] - line.at[i]).length();
            let (a, b) = (line.speed[i], line.speed[i + 1]);
            assert!(a * a - b * b <= 2.0 * l.brake * ds + 1e-2, "braked too hard at {i}");
            assert!(b * b - a * a <= 2.0 * l.accel_at(a) * ds + 1e-2, "accelerated too hard at {i}");
        }
        assert!(line.report.slowest < 12.0, "the hairpin must slow it: {}", line.report.slowest);
    }

    /// Round a circuit, progress keeps rising and laps tick over at the line — the arithmetic a
    /// running order is sorted on, which must not fall back at the start of a lap.
    #[test]
    fn a_rail_counts_laps_and_its_progress_never_falls() {
        let l = limits();
        let line = RaceLine::from_points(circle(50.0, 157), true, &l).unwrap();
        let mut rail = Rail::place(&line, Vec3::new(50.0, 0.0, 0.0), Vec3::Z, 0.0, 1.0);
        let mut last = rail.progress();
        let mut laps_seen = 0;
        for _ in 0..(60 * 120) {
            rail.step(&line, &l, 1.0 / 60.0, false);
            assert!(rail.progress() >= last);
            last = rail.progress();
            laps_seen = rail.laps();
        }
        assert!(laps_seen >= 2, "two minutes round a 314 m circle is several laps: {laps_seen}");
        assert_eq!(rail.laps(), (rail.progress() / line.length()).floor() as u32);
    }

    /// From rest a rail gains speed no faster than the table says the car can.
    #[test]
    fn a_rail_accelerates_like_the_car() {
        let l = limits();
        let pts: Vec<Vec3> = (0..2000).map(|i| Vec3::new(0.0, 0.0, -(i as f32) * 2.0)).collect();
        let line = RaceLine::from_points(pts, false, &l).unwrap();
        let mut rail = Rail::place(&line, Vec3::ZERO, Vec3::NEG_Z, 0.0, 1.0);
        let dt = 1.0 / 60.0;
        for _ in 0..600 {
            let before = rail.speed();
            rail.step(&line, &l, dt, false);
            assert!(rail.speed() - before <= l.accel_at(before) * dt + 1e-5);
        }
        assert!(rail.speed() > 20.0);
    }

    /// The car's nose points along the line and its right side to the right of it.
    #[test]
    fn the_pose_faces_along_the_line() {
        let l = limits();
        let pts: Vec<Vec3> = (0..100).map(|i| Vec3::new(i as f32 * 2.0, 0.0, 0.0)).collect();
        let line = RaceLine::from_points(pts, false, &l).unwrap();
        let rail = Rail::place(&line, Vec3::new(50.0, 0.0, 0.0), Vec3::X, 0.0, 1.0);
        let (at, rot) = rail.pose(&line, 0.5);
        let nose = rot * Vec3::NEG_Z;
        let right = rot * Vec3::X;
        assert!(nose.dot(Vec3::X) > 0.99, "{nose:?}");
        assert!(right.dot(Vec3::Z) > 0.99, "{right:?}");
        assert!((at.y - 0.5).abs() < 1e-4);
    }

    /// Under a bridge the road mask often offers only the deck. The solve skips those samples
    /// rather than climbing onto the deck and back, and the gap is bridged at the road's height;
    /// a real ramp, offered one surface at a time, is taken step by step.
    #[test]
    fn the_height_solve_skips_a_deck_and_keeps_a_ramp() {
        let under: Vec<Vec<f32>> =
            vec![vec![0.0], vec![0.0], vec![10.0], vec![10.0], vec![10.0], vec![0.0], vec![0.0]];
        let h = least_climb(&under);
        assert_eq!(h[0], Some(0.0));
        assert_eq!(h[2], None, "{h:?}");
        assert_eq!(h[6], Some(0.0));
        let ramp: Vec<Vec<f32>> = (0..6).map(|i| vec![i as f32]).collect();
        assert!(least_climb(&ramp).iter().all(Option::is_some));
        // Two decks over the whole stretch: it stays on one of them — here the flat one, which climbs
        // nothing — and never steps between them.
        let stack: Vec<Vec<f32>> = (0..6).map(|i| vec![0.1 * i as f32, 12.0]).collect();
        let h = least_climb(&stack);
        let low = h.iter().filter(|x| x.is_some_and(|y| y < 1.0)).count();
        assert!(low == 0 || low == h.len(), "switched decks: {h:?}");
    }

    /// An out-and-back spur on a straight line is taken out from both sides, and a curve is left
    /// alone — a hairpin of 4 m turns 29° per 2 m step, far short of a cusp.
    #[test]
    fn a_spur_is_removed_and_a_curve_is_not() {
        let mut pts: Vec<Vec3> = (0..20).map(|i| Vec3::new(0.0, 0.0, -2.0 * i as f32)).collect();
        // Out 10 m past the end of the street and back again, then on along it.
        let mut spur: Vec<Vec3> = (1..=5).map(|i| Vec3::new(0.0, 0.0, -38.0 - 2.0 * i as f32)).collect();
        spur.extend((1..5).rev().map(|i| Vec3::new(0.3, 0.0, -38.0 - 2.0 * i as f32)));
        pts.extend(spur);
        pts.extend((1..20).map(|i| Vec3::new(2.0 * i as f32, 0.0, -38.0)));
        let removed = unspur(&mut pts, false);
        assert!(removed >= 8, "{removed}");
        // The 10 m spur is gone; at most a sample's wiggle is left where it joined.
        assert!(pts.iter().all(|p| p.z > -41.0), "the spur's tip is still there: {pts:?}");

        let mut curve = circle(4.0, 13);
        assert_eq!(unspur(&mut curve, true), 0);
    }

    /// Tracking a car round a circuit keeps counting up through the line, and a car far off the
    /// line keeps its last place instead of jumping to whatever piece of line is nearest.
    #[test]
    fn tracking_unwraps_laps_and_ignores_a_car_off_the_line() {
        let l = limits();
        let line = RaceLine::from_points(circle(50.0, 157), true, &l).unwrap();
        let len = line.length();
        // A little way into the second lap.
        let at = line.point(12.0) + Vec3::new(0.0, 0.0, 0.0);
        let (s, _) = line.track(at, len + 5.0).unwrap();
        assert!((s - (len + 12.0)).abs() < 1.0, "{s} against {}", len + 12.0);
        // 200 m from the circle is nowhere on it.
        assert_eq!(line.track(Vec3::new(250.0, 0.0, 0.0), 5.0), None);
    }

    /// The signed curvature reads a right-hand circle as negative and at its own radius — the sign
    /// the wheels are drawn with, where getting it backwards points them out of the corner.
    #[test]
    fn a_right_hand_circle_turns_right() {
        let l = limits();
        let line = RaceLine::from_points(circle(40.0, 126), true, &l).unwrap();
        // `circle` runs from +X towards +Z, which with Y up is clockwise seen from above.
        let k = line.turn_at(30.0);
        assert!((k + 1.0 / 40.0).abs() < 0.002, "{k}");
    }

    /// A fast car behind a slow one in the same lane never drives into it, and gets past it.
    #[test]
    fn the_body_leans_out_of_a_corner_and_dips_under_the_brakes() {
        let lim = limits();
        // A left-hand circle (anticlockwise seen from above, the way `circle` runs), driven at pace.
        let line = RaceLine::from_points(circle(60.0, 180), true, &lim).expect("a line");
        let turn = line.turn_at(10.0).signum();
        let mut r = Rail::place(&line, line.point(0.0), line.tangent(0.0), 0.0, 0.9);
        for _ in 0..600 {
            r.step(&line, &lim, 1.0 / 60.0, false);
        }
        let (roll, _) = r.attitude();
        assert!(roll != 0.0 && roll.signum() == -turn, "leans away from the turn: roll {roll}, turn {turn}");
        let lateral_g = r.speed().powi(2) / 60.0 / G;
        assert!((roll.abs() - ROLL_PER_G * lateral_g).abs() < ROLL_PER_G * 0.2, "as far as the bench car does");

        // Straight line, full stop from speed: the nose goes down while it brakes.
        let straight: Vec<Vec3> = (0..400).map(|i| Vec3::new(0.0, 0.0, -(i as f32) * 2.0)).collect();
        let line = RaceLine::from_points(straight, false, &lim).expect("a line");
        let mut r = Rail::place(&line, line.point(0.0), line.tangent(0.0), 0.0, 0.9);
        let mut lifted = false;
        let mut dipped = false;
        for _ in 0..3000 {
            r.step(&line, &lim, 1.0 / 60.0, false);
            lifted |= r.attitude().1 > PITCH_PER_G * 0.05;
            dipped |= r.attitude().1 < -PITCH_PER_G * 0.3;
        }
        assert!(lifted, "the nose rises as it pulls away");
        assert!(dipped, "and dips as it stops for the end of the line");
    }

    #[test]
    fn pace_breathes_and_catch_up_pulls_towards_the_player() {
        let lim = limits();
        let line = RaceLine::from_points(circle(200.0, 400), true, &lim).expect("a line");
        let mut r = Rail::place(&line, line.point(0.0), line.tangent(0.0), 0.0, 0.92);
        let paces: Vec<f32> = (0..200)
            .map(|i| {
                r.s = i as f32 * 10.0;
                r.pace_here()
            })
            .collect();
        let (lo, hi) = paces.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
        assert!(hi - lo > 0.92 * WOBBLE, "it breathes: {lo}..{hi}");
        assert!(hi <= PACE_CEIL);

        let mut field = vec![r.clone(), r.clone()];
        field[0].s = 0.0;
        field[1].s = 1000.0;
        let base: Vec<f32> = field.iter().map(Rail::pace_here).collect();
        catch_up(&mut field, 500.0);
        assert!(field[0].pace_here() > base[0], "behind the player: faster");
        assert!(field[1].pace_here() < base[1], "ahead of the player: slower");
    }

    #[test]
    fn traffic_is_followed_and_then_passed() {
        let l = limits();
        let line = RaceLine::from_points(circle(150.0, 471), true, &l).unwrap();
        let fast = Rail::place(&line, Vec3::new(150.0, 0.0, 0.0), Vec3::Z, 0.0, 1.0);
        let mut slow = Rail::place(&line, Vec3::new(150.0, 0.0, 0.0), Vec3::Z, 0.0, 0.5);
        slow.s += 30.0;
        slow.lane = fast.lane;
        slow.lane_goal = fast.lane_goal;
        let mut field = vec![fast, slow];
        let mut passed = false;
        for _ in 0..(60 * 90) {
            advance(&mut field, &[], &line, &l, 1.0 / 60.0, false);
            let gap = (field[1].s - field[0].s).rem_euclid(line.length());
            let side_by_side = (field[0].lane - field[1].lane).abs() < CAR_WIDE;
            if side_by_side && !passed {
                assert!(gap > CAR_LEN, "drove into the car in front: gap {gap}");
            }
            if field[0].progress() > field[1].progress() + 10.0 {
                passed = true;
            }
        }
        assert!(passed, "the faster car never got by");
    }
}
