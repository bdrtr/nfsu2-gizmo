//! The camera that follows a car.
//!
//! Three behaviours over one piece of state, because they are three answers to the same question
//! and switching between them mid-drive must not jump:
//!
//! * **Chase** — trailing the car's own forward at a fixed distance, lerped toward rather than
//!   snapped to, so a corner swings the camera around instead of teleporting it.
//! * **Orbit** — while the right mouse button is held, a fixed-radius look from wherever the mouse
//!   has dragged the view to. This is the one that inspects the car.
//! * **Shot** (`NFS_SHOTCAM`) — a fixed offset **in the car's own frame**, for screenshots and for
//!   watching the suspension work. `nfs_drive` used to place this one in *world* space, which
//!   framed a front three-quarter view only until the car turned; after that it was whichever side
//!   happened to face north.
//!
//! **Speed is felt through the chase camera.** It used to see the same at 20 km/h and at 200: a
//! fixed 45° view, a fixed seat. Two things now move with the car's speed, in chase mode only (the
//! orbit inspects the car and the shot frames it, and both want to hold still):
//!
//! * the vertical field of view opens from [`FOV_REST`] to [`FOV_FAST`] — the road ahead stretches
//!   and the edges of the screen run past faster, which is most of what "fast" looks like;
//! * past [`SHAKE_FROM`] the view trembles by a fraction of a degree, a few pixels, the way a
//!   camera bolted to something fast does.
//!
//! The pull-back with speed was already there and stays: the chase *lerps* toward its seat, so at
//! speed `v` it settles `v / lag` metres further back than [`ChaseCamera::distance`] — 2 m at
//! 90 km/h, 4 m at 180.

use gizmo::prelude::*;

/// How high above the car's origin the chase camera aims.
const CHASE_LOOK_Y: f32 = 0.7;
/// The same, for the shot camera — lower, because it is framing the car rather than the road ahead.
const SHOT_LOOK_Y: f32 = 0.5;
/// Where the orbit pivot sits relative to the car's origin, and how far the camera stands off it.
const ORBIT_LOOK_Y: f32 = 1.2;
const ORBIT_DISTANCE: f32 = 9.0;
/// Radians of yaw/pitch per pixel of mouse movement.
const MOUSE_SENSITIVITY: f32 = 0.005;

/// The vertical field of view standing still: 45°, what every binary had before.
pub const FOV_REST: f32 = std::f32::consts::FRAC_PI_4;
/// …and at [`FOV_SPEED`] and above: 62°.
pub const FOV_FAST: f32 = 62.0 * std::f32::consts::PI / 180.0;
/// Below this the view does not widen at all (m/s, 36 km/h): town speed should look like town.
pub const FOV_FROM: f32 = 10.0;
/// Where the widening is complete (m/s, 252 km/h) — a little past what the stock cars reach, so
/// the view keeps opening all the way to their top speed.
pub const FOV_SPEED: f32 = 70.0;
/// How fast the view follows its target, per second. Slow enough that a hard brake *closes* the
/// view over a second rather than snapping it, which is half of what a brake feels like.
const FOV_RATE: f32 = 2.5;
/// Speed the tremble starts at (m/s, 90 km/h), and its amplitude at [`FOV_SPEED`], radians.
pub const SHAKE_FROM: f32 = 25.0;
const SHAKE_MAX: f32 = 0.0035;

/// The field of view the chase is heading for at `speed` (m/s, either sign).
///
/// Smoothstep between [`FOV_FROM`] and [`FOV_SPEED`]: no kink where it starts, none where it ends.
#[must_use]
pub fn speed_fov(speed: f32) -> f32 {
    let f = ((speed.abs() - FOV_FROM) / (FOV_SPEED - FOV_FROM)).clamp(0.0, 1.0);
    FOV_REST + (FOV_FAST - FOV_REST) * f * f * (3.0 - 2.0 * f)
}

/// How hard the view trembles at `speed`, radians of amplitude; zero below [`SHAKE_FROM`].
#[must_use]
pub fn shake_amplitude(speed: f32) -> f32 {
    let f = ((speed.abs() - SHAKE_FROM) / (FOV_SPEED - SHAKE_FROM)).clamp(0.0, 1.0);
    SHAKE_MAX * f
}

/// The tremble at time `t`, as (yaw, pitch) in units of the amplitude. Three sines at frequencies
/// that never line up, per axis — a sum of incommensurate sines never repeats visibly, and unlike
/// random noise it has no frame-to-frame jumps to alias against the frame rate.
fn tremble(t: f32) -> (f32, f32) {
    let yaw = 0.5 * (t * 71.0).sin() + 0.3 * (t * 113.0 + 1.3).sin() + 0.2 * (t * 47.0 + 2.1).sin();
    let pitch = 0.5 * (t * 83.0 + 0.7).sin() + 0.3 * (t * 131.0 + 2.9).sin() + 0.2 * (t * 59.0).sin();
    (yaw, pitch)
}

/// A camera entity plus the state needed to move it smoothly behind a car.
pub struct ChaseCamera {
    /// The camera entity.
    pub id: u32,
    /// Where the camera is this frame. Carried rather than recomputed because the chase mode lerps
    /// toward its target and so depends on where it was.
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// How far behind the car the chase sits, in metres.
    pub distance: f32,
    /// How far above it.
    pub height: f32,
    /// Chase stiffness: the exponential rate the camera closes on its target at. Higher is tighter
    /// and more nauseating.
    pub lag: f32,
    /// The `NFS_SHOTCAM` offset, in the car's own frame. `None` unless the variable is set.
    pub shot: Option<Vec3>,
    /// The vertical field of view this frame, closing on [`speed_fov`] at [`FOV_RATE`].
    pub fov: f32,
    /// Seconds the camera has run, for the tremble.
    clock: f32,
    /// This frame's tremble, (yaw, pitch) radians, added on write and never fed back into the aim.
    shake: (f32, f32),
}

impl ChaseCamera {
    /// Spawn the camera entity looking down −X at `at`, with the trailing defaults.
    ///
    /// **Both planes are the caller's**, because what costs depth precision is their *ratio* and
    /// only the caller knows how big its world is. A car on a plane wants `0.1 / 2_000`; a car in
    /// Bayview wants `0.5 / 20_000`, which is 40,000:1 rather than 200,000:1 — at street level the
    /// tighter ratio is the difference between a city and a field of z-fighting. Moving the near
    /// plane out has its own cost (it clips whatever the camera is standing next to), which is why
    /// the small-world binaries do not pay it.
    pub fn spawn(world: &mut World, at: Vec3, near: f32, far: f32) -> Self {
        let (yaw, pitch) = (-std::f32::consts::FRAC_PI_2, -0.3);
        let camera = world.spawn();
        crate::geom::add_transform(world, camera, Transform::new(at));
        world.add_component(camera, Camera::new(std::f32::consts::FRAC_PI_4, near, far, yaw, pitch, true));
        Self {
            id: camera.id(),
            position: at,
            yaw,
            pitch,
            distance: 6.5,
            height: 1.9,
            lag: 12.0,
            // Empty reads as unset: a knob cleared with `NFS_SHOTCAM=` should be off, not on.
            shot: std::env::var("NFS_SHOTCAM")
                .is_ok_and(|v| !v.is_empty())
                .then_some(Vec3::new(4.2, 1.4, -5.6)),
            fov: FOV_REST,
            clock: 0.0,
            shake: (0.0, 0.0),
        }
    }

    /// Set how the chase mode trails the car.
    #[must_use]
    pub fn trailing(mut self, distance: f32, height: f32, lag: f32) -> Self {
        self.distance = distance;
        self.height = height;
        self.lag = lag;
        self
    }

    /// Move the camera for this frame and write it to its entity.
    pub fn update(&mut self, world: &mut World, input: &Input, pose: super::Pose, dt: f32) {
        let orbiting = input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT);
        let chasing = self.shot.is_none() && !orbiting;
        self.clock += dt;
        let (want, amp) = if chasing { (speed_fov(pose.speed), shake_amplitude(pose.speed)) } else { (FOV_REST, 0.0) };
        self.fov += (want - self.fov) * (1.0 - (-FOV_RATE * dt).exp());
        let (ty, tp) = tremble(self.clock);
        self.shake = (ty * amp, tp * amp);
        match self.shot {
            // A fixed seat in the car's frame; the aim follows from where that lands.
            Some(offset) => {
                self.position = pose.position + pose.rotation * offset;
                self.aim_at(pose.position + Vec3::Y * SHOT_LOOK_Y);
            }
            None if orbiting => {
                // Stand off along whatever direction the mouse has dragged the view to, so dragging
                // swings the camera around the car rather than turning it on the spot.
                let forward = Camera::forward_from(self.yaw, self.pitch);
                self.position = pose.position + Vec3::Y * ORBIT_LOOK_Y - forward * ORBIT_DISTANCE;
            }
            None => {
                let forward = pose.rotation * Vec3::NEG_Z;
                let target = pose.position - forward * self.distance + Vec3::Y * self.height;
                // Frame-rate independent smoothing: a plain `lerp(k)` with a constant `k` closes
                // faster the more frames there are, so the camera would tighten with the frame rate.
                self.position = self.position.lerp(target, 1.0 - (-self.lag * dt).exp());
                self.aim_at(pose.position + Vec3::Y * CHASE_LOOK_Y);
            }
        }
        self.mouse_look(input);
        self.write(world);
    }

    /// Point the camera at a world point.
    fn aim_at(&mut self, target: Vec3) {
        let dir = (target - self.position).normalize_or_zero();
        if dir == Vec3::ZERO {
            return;
        }
        self.yaw = dir.z.atan2(dir.x);
        self.pitch = dir.y.asin();
    }

    /// Apply this frame's mouse drag, while the right button is held.
    ///
    /// After the mode has had its say, so orbit reads the angles it is about to be given and the
    /// other two modes are simply overridden — dragging during a chase looks around and the next
    /// frame's aim takes it back.
    fn mouse_look(&mut self, input: &Input) {
        if input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT) {
            let d = input.mouse_delta();
            self.yaw += d.0 * MOUSE_SENSITIVITY;
            self.pitch += d.1 * MOUSE_SENSITIVITY;
        }
        // Short of straight up or straight down, where the view basis degenerates.
        self.pitch = self
            .pitch
            .clamp(-std::f32::consts::FRAC_PI_2 + 0.1, std::f32::consts::FRAC_PI_2 - 0.1);
    }

    /// Write the camera state onto its entity.
    fn write(&self, world: &mut World) {
        let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
        let mut globals = unsafe { world.borrow_mut_unchecked::<GlobalTransform>() };
        let mut cameras = unsafe { world.borrow_mut_unchecked::<Camera>() };
        if let Some(mut t) = transforms.get_mut(self.id) {
            t.position = self.position;
            t.update_local_matrix();
            if let Some(mut g) = globals.get_mut(self.id) {
                g.matrix = t.local_matrix;
            }
        }
        if let Some(mut c) = cameras.get_mut(self.id) {
            c.yaw = self.yaw + self.shake.0;
            c.pitch = self.pitch + self.shake.1;
            c.fov = self.fov;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_opens_with_speed_and_only_with_speed() {
        assert_eq!(speed_fov(0.0), FOV_REST);
        assert_eq!(speed_fov(FOV_FROM), FOV_REST);
        assert!((speed_fov(FOV_SPEED) - FOV_FAST).abs() < 1e-6);
        assert!((speed_fov(150.0) - FOV_FAST).abs() < 1e-6, "no wider past the end");
        assert_eq!(speed_fov(-30.0), speed_fov(30.0), "reversing is the same speed");
        let mut last = 0.0;
        for v in 0..80 {
            let f = speed_fov(v as f32);
            assert!(f >= last, "never narrows as the car speeds up");
            last = f;
        }
    }

    #[test]
    fn the_tremble_starts_at_its_speed_and_stays_small() {
        assert_eq!(shake_amplitude(SHAKE_FROM), 0.0);
        assert_eq!(shake_amplitude(10.0), 0.0);
        assert!(shake_amplitude(FOV_SPEED) <= SHAKE_MAX);
        for i in 0..1000 {
            let (y, p) = tremble(i as f32 * 0.013);
            assert!(y.abs() <= 1.0 && p.abs() <= 1.0);
        }
    }
}
