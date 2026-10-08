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
//! How many laps a circuit runs is *not* in anything decoded, so it is a caller's choice and
//! [`Race::LAPS`] is a default rather than a fact.
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
}

impl Race {
    /// Laps a circuit runs when nothing says otherwise.
    ///
    /// Three, because that is what NFSU2's circuits mostly are — and it is a choice, not a reading:
    /// nothing decoded carries a lap count. Kept as a constant so the next person meets the
    /// assumption instead of inheriting it.
    pub const LAPS: u32 = 3;

    /// How long the field is held on the line.
    pub const COUNTDOWN: f32 = 3.0;

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
        }
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
            if s.done && !self.finished.contains(&s.car) {
                self.finished.push(s.car);
            }
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

    /// Whether every car has finished.
    #[must_use]
    pub fn over(&self, field: usize) -> bool {
        self.finished.len() >= field && field > 0
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
