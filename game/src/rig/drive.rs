//! The half of the frame between the keyboard and the physics: what the player asked for, and the
//! fixed step the simulation actually runs at.
//!
//! Neither half is about NFSU2 — a car with a steering wheel and a 240 Hz solver is the same in
//! every binary here — which is why both were written identically twice and why they belong beside
//! [`super::CarRig`] rather than inside it: a headless test drives a rig with synthetic
//! [`Controls`] and never touches a keyboard.

use gizmo::prelude::*;

/// The physics step, in seconds.
///
/// 240 Hz rather than the frame rate. A raycast suspension integrates a spring against the ground
/// every step, and at 60 Hz that spring is stiff enough relative to the step to buzz — the car sits
/// visibly vibrating on its springs. Four sub-steps per displayed frame is what stopped it.
pub const FIXED_DT: f32 = 1.0 / 240.0;

/// The most sub-steps one frame may run before giving up and letting time slip.
///
/// Without it, a frame that stalls (a texture upload, a window drag) hands the accumulator a large
/// `dt`, which runs enough steps to stall the next frame too, which is a spiral that never
/// recovers. Dropping simulated time is the survivable failure.
const MAX_STEPS: u32 = 32;

/// The longest frame the accumulator will believe, in seconds. Beyond this the frame is treated as
/// a stall rather than as elapsed time.
const MAX_FRAME: f32 = 0.1;

/// What the car is being asked to do this frame.
///
/// A struct rather than four arguments because an autodriver *overrides* some of these after
/// reading them — it wants the human's brake and its own steering — and a positional call is one
/// transposition away from full throttle into a wall.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// −1..1. Negative is reverse, which the controller wants as a separate flag plus a magnitude.
    pub throttle: f32,
    /// 0..1, the handbrake key.
    pub brake: f32,
    /// −1..1, *input* rather than an angle: the controller applies the steering lock.
    pub steer: f32,
    /// Whether the automatic gearbox was toggled this frame.
    pub toggle_auto_shift: bool,
}

/// The player's side of the car: the smoothed steering position and the physics accumulator.
#[derive(Clone, Copy, Debug, Default)]
pub struct Driver {
    /// Current steering input, −1..1. Public because an autodriver sets it directly and then wants
    /// the same recentring the keyboard gets when it lets go.
    pub steer: f32,
    /// Unsimulated time carried over from previous frames.
    accum: f32,
}

impl Driver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the keyboard into [`Controls`], advancing the smoothed steering.
    ///
    /// Steering is rate-limited rather than binary (a keyboard has no half-lock) and springs back
    /// exponentially when neither key is held, so letting go straightens the car out instead of
    /// snapping it straight.
    pub fn read(&mut self, input: &Input, dt: f32) -> Controls {
        let held = |k: KeyCode| input.is_key_pressed(k as u32);
        let mut throttle = 0.0f32;
        if held(KeyCode::KeyW) || held(KeyCode::ArrowUp) {
            throttle += 1.0;
        }
        if held(KeyCode::KeyS) || held(KeyCode::ArrowDown) {
            throttle -= 1.0;
        }
        let brake = f32::from(u8::from(held(KeyCode::Space)));

        let mut steering = false;
        if held(KeyCode::KeyA) || held(KeyCode::ArrowLeft) {
            self.steer = (self.steer + 6.0 * dt).min(1.0);
            steering = true;
        }
        if held(KeyCode::KeyD) || held(KeyCode::ArrowRight) {
            self.steer = (self.steer - 6.0 * dt).max(-1.0);
            steering = true;
        }
        if !steering {
            self.steer *= (-15.0 * dt).exp();
        }

        Controls {
            throttle,
            brake,
            steer: self.steer,
            toggle_auto_shift: input.is_key_just_pressed(KeyCode::KeyT as u32),
        }
    }

    /// Run the vehicle controller and the rigid-body solver at [`FIXED_DT`] for as much of this
    /// frame's time as has accumulated.
    pub fn step_physics(&mut self, world: &mut World, dt: f32) {
        self.step_physics_with(world, dt, |_| {});
    }

    /// The same, with a hook **between the forces and the integration**.
    ///
    /// That gap is the only place a barrier can stand. `CarRig::hold_at_edge` takes the velocity the
    /// controller has just produced and removes the part of it that points off the city, so the step
    /// which would have carried the car over the lip is the step that does not — and it has to
    /// happen once per fixed step rather than once per frame, or a slow frame steps the car over the
    /// edge in instalments the fence never sees.
    pub fn step_physics_with(
        &mut self,
        world: &mut World,
        dt: f32,
        mut between: impl FnMut(&mut World),
    ) {
        self.accum += dt.min(MAX_FRAME);
        let mut steps = 0;
        while self.accum >= FIXED_DT && steps < MAX_STEPS {
            gizmo::physics::vehicle_controller_system(world, FIXED_DT);
            between(world);
            gizmo::physics::physics_step_system(world, FIXED_DT);
            self.accum -= FIXED_DT;
            steps += 1;
        }
    }

    /// Straighten the wheels and drop any unsimulated time — what a reset key means for this half.
    pub fn reset(&mut self) {
        self.steer = 0.0;
        self.accum = 0.0;
    }
}
