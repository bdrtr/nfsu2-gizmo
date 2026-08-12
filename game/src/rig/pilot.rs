//! The driver that is not a keyboard.
//!
//! [`Driver`](super::Driver) turns key presses into [`Controls`]; this turns a **route** into the
//! same thing, so a rival goes through the identical `CarRig::drive` the player does. Nothing here
//! touches physics: a pilot that could push its car would be a different car from the one being
//! raced against, and the first thing anyone would ask is whether it was cheating.
//!
//! ## Pure pursuit, and why the state is an index
//!
//! The rule is the oldest one there is: aim at a point on the line some distance ahead and steer
//! at it. What makes it work or not is *which* point, and that is a question about state rather
//! than geometry — take the nearest point on the line every frame and a route that crosses itself
//! (Bayview is a city; its routes cross themselves constantly) will hand the car the wrong branch
//! the moment the two lines pass within a few metres.
//!
//! So a pilot remembers where it is as an **index into its path** and only ever moves that index
//! forward, searching a short window. It cannot be pulled backwards by a crossing, it cannot skip
//! a lap by cutting a corner into a later part of the line, and when it is genuinely lost —
//! knocked off, spun, respawned — [`Pilot::relocate`] is the one call that lets it start again.
//!
//! ## What is deliberately not modelled
//!
//! No racing line, no braking points, no awareness of the other cars. A rival drives the centre of
//! its lane at a speed the corner allows and will happily drive into the back of another one. That
//! is the honest state of it, and the alternative — a lookahead that also solves for traffic —
//! is a much larger thing that should not be smuggled in under "the grid has drivers now".

use super::drive::Controls;
use crate::world::RoutePath;
use gizmo::prelude::*;

/// How far ahead to aim, as a multiple of speed in m/s, and the bounds that keeps it inside.
///
/// Speed-proportional because a fixed distance is wrong at both ends: short enough to be accurate
/// at 30 km/h is a violent oscillation at 150, and long enough to be smooth at 150 cuts every
/// corner at 30. The floor keeps a stopped car aiming at something.
const LOOKAHEAD_PER_SPEED: f32 = 0.9;
const LOOKAHEAD_MIN: f32 = 9.0;
const LOOKAHEAD_MAX: f32 = 34.0;

/// How many points ahead of its own index a pilot will look when advancing it.
///
/// Bounded so a route that doubles back cannot let the search jump the car to a later part of the
/// line: the window is long enough to survive one frame at any speed the car reaches (34 m of
/// lookahead over a median 29 m node step is one to two points) and far too short to skip a lap.
const ADVANCE_WINDOW: usize = 6;

/// Steering lock the pilot will ask for, as a fraction of the controller's own.
///
/// Under one because the controller's full lock is a parking manoeuvre; a rival that used it at
/// speed would spin, and the failure looks like bad physics rather than a bad driver.
const STEER_LIMIT: f32 = 0.85;

/// How much steering costs throttle.
///
/// The whole of the speed policy: a car that is turning hard is going too fast for the corner, so
/// it lifts. Crude, and it is the crudeness that keeps the pilot honest — there is no lookahead
/// curvature model here pretending to know a braking point.
const CORNER_LIFT: f32 = 0.75;

/// One rival's driver: which path it is on, how far along, and its steering position.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pilot {
    /// Index into the route's `paths`, chosen once and kept.
    path: usize,
    /// Index into that path's points. Monotone — see the module note on why it is state.
    at: usize,
    /// Smoothed steering, the same field [`Driver`](super::Driver) keeps and for the same reason.
    steer: f32,
    /// Whether [`Self::relocate`] has ever run. A pilot that has not been placed drives nothing.
    placed: bool,
}

impl Pilot {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Put the pilot on the nearest point of the nearest path, forgetting where it was.
    ///
    /// `facing` is what keeps it honest about direction: only paths running the same way as the car
    /// are considered. Without that the nearest point is as likely to be the opposite carriageway,
    /// which is a few metres away and on the other side of a crash barrier.
    ///
    /// The one way the index moves backwards, and it is a deliberate escape hatch rather than a
    /// fallback the pilot reaches for on its own: a car that quietly relocates every time it
    /// strays is a car that will cut the course and report a clean lap.
    pub fn relocate(&mut self, at: Vec3, facing: Vec3, route: &[RoutePath]) {
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z).normalize_or_zero();
        let f = flat(facing);
        let mut best = (f32::MAX, 0usize, 0usize);
        for (pi, p) in route.iter().enumerate() {
            for (i, q) in p.points.iter().enumerate() {
                // A path that runs the other way is the wrong path, however near it is. Bayview's
                // route network carries both carriageways of a dual road and they pass within a
                // few metres of each other — nearest-point alone put a car on the far side of the
                // barrier, aiming at a line it could not reach, and it drove into the rail.
                let ahead = match p.points.get(i + 1) {
                    Some(n) => flat(*n - *q),
                    None => match i.checked_sub(1).and_then(|k| p.points.get(k)) {
                        Some(prev) => flat(*q - *prev),
                        None => continue,
                    },
                };
                if f != Vec3::ZERO && ahead.dot(f) <= 0.0 {
                    continue;
                }
                let d = (*q - at).length_squared();
                if d < best.0 {
                    best = (d, pi, i);
                }
            }
        }
        if best.0 < f32::MAX {
            self.path = best.1;
            self.at = best.2;
            self.placed = true;
        }
    }

    /// Which path this pilot is on, and how far along it is.
    #[must_use]
    pub fn progress(&self, route: &[RoutePath]) -> Option<f32> {
        route.get(self.path).and_then(|p| p.progress.get(self.at)).copied()
    }

    /// This frame's controls.
    ///
    /// `None` before [`Self::relocate`] has placed it, or when the path it holds has run out —
    /// which is what the end of a sprint looks like, and the caller decides what that means.
    pub fn drive(&mut self, at: Vec3, facing: Quat, speed: f32, route: &[RoutePath]) -> Option<Controls> {
        if !self.placed {
            return None;
        }
        let path = route.get(self.path)?;
        if path.points.len() < 2 {
            return None;
        }
        let forward = facing * Vec3::NEG_Z;
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);

        // Advance past every point already behind the car, within the window. `> 0` and not `>= 0`
        // so a point exactly abeam is kept: it is the one the car is turning around.
        for _ in 0..ADVANCE_WINDOW {
            let Some(next) = path.points.get(self.at + 1) else { break };
            if flat(*next - at).dot(flat(forward)) > 0.0 {
                break;
            }
            self.at += 1;
        }

        // Walk forward from there until the aim point is far enough away. Distance is measured
        // along the line rather than straight to the car, so a hairpin does not put the aim point
        // behind the driver.
        let look = (speed * LOOKAHEAD_PER_SPEED).clamp(LOOKAHEAD_MIN, LOOKAHEAD_MAX);
        let mut aim = *path.points.get(self.at)?;
        let mut walked = 0.0;
        for w in path.points[self.at..].windows(2) {
            walked += flat(w[1] - w[0]).length();
            aim = w[1];
            if walked >= look {
                break;
            }
        }

        // Signed angle to the aim point, in the ground plane. The cross product's Y component is
        // the sign — positive is left — and the dot gives the magnitude through `atan2`, which
        // stays well behaved when the aim point is directly behind.
        let to = flat(aim - at).normalize_or_zero();
        let f = flat(forward).normalize_or_zero();
        let angle = f.cross(to).y.atan2(f.dot(to));
        let want = (angle * 2.0 / std::f32::consts::PI).clamp(-1.0, 1.0) * STEER_LIMIT;

        // Same recentring the keyboard gets, so a rival's steering has the same feel as a player's
        // rather than snapping between positions.
        self.steer += (want - self.steer) * 0.35;
        let throttle = (1.0 - self.steer.abs() * CORNER_LIFT).max(0.15);

        Some(Controls { throttle, brake: 0.0, steer: self.steer, toggle_auto_shift: false })
    }
}
