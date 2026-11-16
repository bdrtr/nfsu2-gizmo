//! What turns a field of cars into a race: a countdown, a running order, and a finish.
//!
//! Everything here is arithmetic over what the pilots already track. It holds no cars, steps no
//! physics and decides no controls — a race that could push a car would be a race whose result
//! nobody should believe.
//!
//! ## What the file says a race is
//!
//! `0x0003414C` gives an event's outline and one flag: [`RaceEvent::circuit`], set exactly when the
//! outline is closed, in 4,067 records with no disagreement. So the two shapes are the file's own:
//!
//! - a **circuit** is laps of a closed course, and finishing means completing them;
//! - a **sprint** is one pass along an open one, and finishing means reaching the end.
//!
//! How many laps a circuit runs is in the game's event records but not in anything decoded here,
//! so it is a caller's choice and [`Race::LAPS`] is a default rather than a fact.
//!
//! ## Units
//!
//! A race counts in whatever its field counts in, and says so through [`Runner`]. A field of
//! simulated [`Pilot`]s counts **waypoints** driven past; a field of [`Rail`]s counts **metres**
//! along the race line. [`Race::new`]'s `course` is the length of one lap in the same unit.

use crate::rig::rail::Rail;
use crate::rig::Pilot;

/// Anything whose place in a race can be read: completed laps, and how far it has got since the
/// line in the race's own unit.
pub trait Runner {
    /// Completed laps.
    fn laps(&self) -> u32;
    /// How far since the line, laps included, in the unit `course` is in.
    fn along(&self, course: usize) -> usize;
}

impl Runner for Pilot {
    fn laps(&self) -> u32 {
        Pilot::laps(self)
    }
    fn along(&self, course: usize) -> usize {
        Pilot::along(self, course)
    }
}

/// A rail counts in metres, and knows its own lap length, so `course` is only the unit's promise.
impl Runner for Rail {
    fn laps(&self) -> u32 {
        Rail::laps(self)
    }
    fn along(&self, _course: usize) -> usize {
        Rail::along(self)
    }
}

/// One car's place in the running order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    /// Which car, as an index into the field.
    pub car: usize,
    /// Completed laps.
    pub laps: u32,
    /// How far since the line, laps included, in the field's unit (see [`Runner`]) — what the
    /// order sorts on.
    pub along: usize,
    /// Whether this car has finished.
    pub done: bool,
}

/// A race in progress.
#[derive(Debug, Clone)]
pub struct Race {
    /// How many waypoints the course has.
    course: usize,
    /// Laps to run, or `None` for a sprint.
    laps: Option<u32>,
    /// Seconds left of the countdown.
    countdown: f32,
    /// The order cars crossed the line in, first finisher first.
    finished: Vec<usize>,
    /// The human's car, if the race has one: whose win starts the [`Race::GRACE`] clock.
    player: Option<usize>,
    /// Seconds left before the race closes on whoever has not finished, once the clock is running.
    closing: Option<f32>,
}

impl Race {
    /// Laps a circuit runs when nothing says otherwise.
    ///
    /// Three, because that is what NFSU2's circuits mostly are — and it is a choice, not a reading.
    /// The game does have the number: `SPEED2.EXE` takes it from the career event's record
    /// (`0x5322E0`, `DAT_0085EF48[2]`) or the quick-race mode's (`0x83AACC + mode·0x14`, second
    /// `u16`) into `DAT_0089E7BC`. Where the career record comes from on disc is not found, so
    /// nothing this crate decodes carries it yet (`SPEED2-ATLAS.md` §9).
    pub const LAPS: u32 = 3;

    /// How long the field is held on the line.
    pub const COUNTDOWN: f32 = 3.0;

    /// Seconds the rest of the field gets once the human has **won**; then the race closes and the
    /// unfinished keep the places their progress gives them.
    ///
    /// `SPEED2.EXE`'s finish routine `0x606CB0` starts this clock — once, `0x401820` guards it —
    /// when a car finishes a normal race (`reason == 1`, race kind 1) **in first place**
    /// (`+0xC`, the place it has just been given) **and** is human (`0x5EFA00`); the limit is
    /// `0x7A5690` = 30 s. The per-frame update `0x6076D0` then finishes everyone left with reason
    /// `0xB`, "time ran out". A human who does not win starts nothing: the race runs until the
    /// field is home. Drift races use a different limit and are not modelled here.
    pub const GRACE: f32 = 30.0;

    /// Set up a race over a course of `course` waypoints.
    ///
    /// `circuit` comes from the event's own flag; a sprint is one pass and ends at the last
    /// waypoint.
    #[must_use]
    pub fn new(course: usize, circuit: bool) -> Self {
        Self {
            course,
            laps: circuit.then_some(Self::LAPS),
            countdown: Self::COUNTDOWN,
            finished: Vec::new(),
            player: None,
            closing: None,
        }
    }

    /// Name the human's car, so that their win can start the [`Race::GRACE`] clock.
    #[must_use]
    pub fn with_player(mut self, car: usize) -> Self {
        self.player = Some(car);
        self
    }

    /// Seconds left before the race closes on the unfinished, once a human win has started it.
    #[must_use]
    pub fn closing(&self) -> Option<f32> {
        self.closing
    }

    /// Seconds left before the field is released, or zero once it is running.
    #[must_use]
    pub fn countdown(&self) -> f32 {
        self.countdown
    }

    /// Whether the field is still held.
    #[must_use]
    pub fn holding(&self) -> bool {
        self.countdown > 0.0
    }

    /// Advance the clock.
    pub fn tick(&mut self, dt: f32) {
        self.countdown = (self.countdown - dt).max(0.0);
        if let Some(left) = self.closing.as_mut() {
            *left = (*left - dt).max(0.0);
        }
    }

    /// How far a car has to get to finish, in waypoints driven.
    #[must_use]
    pub fn distance(&self) -> usize {
        match self.laps {
            Some(n) => n as usize * self.course,
            // A sprint ends one short of the count: the last waypoint is the finish, and a car
            // that reached it has driven every one before it.
            None => self.course.saturating_sub(1),
        }
    }

    /// The running order, leader first, and who has finished.
    ///
    /// Sorted on waypoints driven since the line rather than on the raw waypoint index, because
    /// the index wraps and the order must not. Ties keep the field's own order, which is the grid
    /// order — so a car that has not moved is behind one on the same waypoint that started behind
    /// it, and never in front.
    pub fn standings<R: Runner>(&mut self, field: &[R]) -> Vec<Standing> {
        let mut out: Vec<Standing> = field
            .iter()
            .enumerate()
            .map(|(car, p)| {
                let along = p.along(self.course);
                Standing { car, laps: p.laps(), along, done: along >= self.distance() }
            })
            .collect();
        for s in &out {
            if s.done && !self.finished.contains(&s.car) && !self.timed_out() {
                self.finished.push(s.car);
                if self.finished.len() == 1 && self.player == Some(s.car) {
                    self.closing.get_or_insert(Self::GRACE);
                }
            }
        }
        // After the clock, crossing the line no longer counts as finishing.
        for s in &mut out {
            s.done = self.finished.contains(&s.car);
        }
        // Finishers first, in the order they finished; everyone else by how far they have got.
        let place = |c: usize| self.finished.iter().position(|f| *f == c);
        out.sort_by(|a, b| match (place(a.car), place(b.car)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.along.cmp(&a.along).then(a.car.cmp(&b.car)),
        });
        out
    }

    /// Whether the race is over: every car has finished, or the clock a human win started has
    /// run out.
    #[must_use]
    pub fn over(&self, field: usize) -> bool {
        (self.finished.len() >= field && field > 0) || self.timed_out()
    }

    /// Whether the race closed on the clock rather than on the last finisher.
    #[must_use]
    pub fn timed_out(&self) -> bool {
        self.closing == Some(0.0)
    }

    /// The finishing order so far.
    #[must_use]
    pub fn finishers(&self) -> &[usize] {
        &self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sprint is over one waypoint short of the count, a circuit after its laps. Both are
    /// arithmetic and both are easy to get off by one, which is the kind of mistake that shows up
    /// as a race that never ends.
    #[test]
    fn a_sprint_ends_at_the_last_waypoint_and_a_circuit_after_its_laps() {
        assert_eq!(Race::new(100, false).distance(), 99);
        assert_eq!(Race::new(100, true).distance(), 100 * Race::LAPS as usize);
        // A course with no waypoints must not wrap into a very long race.
        assert_eq!(Race::new(0, false).distance(), 0);
    }

    struct At(usize);
    impl Runner for At {
        fn laps(&self) -> u32 {
            0
        }
        fn along(&self, _course: usize) -> usize {
            self.0
        }
    }

    /// A human win gives the rest thirty seconds, and then the race is over with them unfinished,
    /// in the order their progress puts them.
    #[test]
    fn a_human_win_closes_the_race_thirty_seconds_later() {
        let mut r = Race::new(10, false).with_player(0);
        r.tick(Race::COUNTDOWN);
        r.standings(&[At(9), At(4), At(6)]);
        assert_eq!(r.closing(), Some(Race::GRACE));
        r.tick(Race::GRACE - 1.0);
        assert!(!r.over(3));
        r.tick(1.0);
        assert!(r.over(3) && r.timed_out());
        let order: Vec<usize> = r.standings(&[At(9), At(9), At(6)]).iter().map(|s| s.car).collect();
        assert_eq!(order, [0, 1, 2], "the unfinished by progress, the winner first");
        assert_eq!(r.finishers(), [0], "nobody finishes after the clock");
        assert!(!r.standings(&[At(9), At(9), At(6)])[1].done);
    }

    /// The clock is for a win only: a human who comes in second starts nothing.
    #[test]
    fn a_human_who_does_not_win_starts_no_clock() {
        let mut r = Race::new(10, false).with_player(0);
        r.standings(&[At(4), At(9)]);
        r.standings(&[At(9), At(9)]);
        assert_eq!(r.closing(), None);
        r.tick(Race::GRACE * 2.0);
        assert!(r.over(2) && !r.timed_out());
    }

    /// The countdown runs down and stops, rather than going negative and reading as "released a
    /// long time ago" to anything that compares it.
    #[test]
    fn the_countdown_stops_at_zero() {
        let mut r = Race::new(10, true);
        assert!(r.holding());
        r.tick(Race::COUNTDOWN * 2.0);
        assert_eq!(r.countdown(), 0.0);
        assert!(!r.holding());
    }
}
