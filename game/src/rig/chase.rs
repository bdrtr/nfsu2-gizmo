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
}

impl ChaseCamera {
    /// Spawn the camera entity looking down −X at `at`, with the trailing defaults.
    ///
    /// `far` is the far plane: a car on a plane needs a couple of kilometres, a car in Bayview needs
    /// twenty. The near plane stays at 0.1 — what costs depth precision is the *ratio*, and moving
    /// the near plane out is what clips whatever the camera is standing next to.
    pub fn spawn(world: &mut World, at: Vec3, far: f32) -> Self {
        let (yaw, pitch) = (-std::f32::consts::FRAC_PI_2, -0.3);
        let camera = world.spawn();
        crate::geom::add_transform(world, camera, Transform::new(at));
        world.add_component(camera, Camera::new(std::f32::consts::FRAC_PI_4, 0.1, far, yaw, pitch, true));
        Self {
            id: camera.id(),
            position: at,
            yaw,
            pitch,
            distance: 6.5,
            height: 1.9,
            lag: 12.0,
            shot: std::env::var("NFS_SHOTCAM").is_ok().then_some(Vec3::new(4.2, 1.4, -5.6)),
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
            c.yaw = self.yaw;
            c.pitch = self.pitch;
        }
    }
}
