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

/// The engine's vehicle step, between the game's own powertrain rules ([`super::powertrain`]):
/// shift time, the fuel cut, engine braking and the record's drag, none of which the engine's
/// controller has.
///
/// Until the pin moved past `256aa377` this also took each axle's left and right wheel in
/// alternate order on alternate steps: the controller applied each wheel's force at once
/// (Gauss-Seidel) and no fixed order of four corners is its own mirror, so a car turned harder one
/// way than the other (`MOTOR-NOTLARI.md` #19). The engine now accumulates and applies once, as
/// NFSU2 does (`0x59bb00`); with the alternation gone `nfs_bench`'s mirror runs are still exact to
/// the last digit, so it went.
pub fn vehicle_step(world: &mut World, dt: f32) {
    super::powertrain::before_step(world, dt);
    controller_step(world, dt);
    super::powertrain::after_step(world, dt);
}

fn controller_step(world: &mut World, dt: f32) {
    gizmo::physics::vehicle_controller_system(world, dt);
}

/// One rigid-body step.
///
/// `SPEED2.EXE` gives a car only **0.4** of a collision impulse's turning effect (`0x5A0B00`,
/// [`super::bump::SPIN_SHARE`]). Until the pin moved past `51fa4aef` this was done around the step,
/// as a cut of whatever the step changed beyond the damping — which also cut what the *ground* did
/// to a chassis on a kerb, and the pilot sweep lost to it (1 605 → 1 500). The engine now sizes the
/// contact impulse with the body's own share (`RigidBody::contact_spin_share`, set in the rig to
/// [`super::contact_spin_share`]), so this is the engine's step and nothing else.
pub fn physics_step(world: &mut World, dt: f32) {
    gizmo::physics::physics_step_system(world, dt);
}


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
    /// Nitro düğmesi basılı mı. Basılı tutuldukça yanıyor, depo bitene kadar
    /// ([`super::powertrain::Nitro`]).
    pub nitro: bool,
    /// El ile vites: `+1` büyüt (E), `−1` küçült (Q), bu karede basıldıysa. Basmak otomatik
    /// şanzımanı kapatıyor ([`super::CarRig::drive`]).
    pub shift: i8,
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
            nitro: held(KeyCode::ShiftLeft) || held(KeyCode::KeyN),
            shift: i8::from(input.is_key_just_pressed(KeyCode::KeyE as u32))
                - i8::from(input.is_key_just_pressed(KeyCode::KeyQ as u32)),
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
            vehicle_step(world, FIXED_DT);
            between(world);
            physics_step(world, FIXED_DT);
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
