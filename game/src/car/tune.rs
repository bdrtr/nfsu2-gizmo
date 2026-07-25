//! The car as a *machine*: turning `GLOBALB.BUN`'s per-car record into engine vehicle tuning.
//!
//! Every car in this game used to drive identically, off two constants — 1200 kg and 520 N·m — and
//! the file has had the real numbers all along. `gizmo_nfs::CarHandling` carries rpm limits, a
//! nine-point torque curve, four complete gearboxes and the drivetrain split; this maps them onto
//! [`VehicleTuning`].
//!
//! **What the file supplies and what it does not** is the whole of the design here, and it was
//! measured rather than assumed (see `gizmo_nfs::globalb`). Braking, aerodynamics and anti-roll are
//! *not in this game's data*: the one brake-shaped triple is exactly zero for all 15 traffic
//! vehicles, and the only steering angles are a global ±43 identical in all 46 records — a field fed
//! from either would give a bus and a Skyline the same number. Those stay at the engine's defaults
//! and are marked here as invented, so nobody later reads them as NFSU2's.
//!
//! One field lines up so exactly it is worth writing down: `VehicleTuning::gear_ratios` wants
//! `[reverse, neutral, forward…]`, and that **is** the file's own eight-slot layout — reverse always
//! negative, neutral always exactly `0.0`, then the forward gears. Nothing is rearranged.

use gizmo::physics::vehicle::VehicleTuning;
use gizmo_nfs::{CarHandling, CarTypeInfo};

/// Which gearbox a car is running: the file stores four, stock plus three upgrade levels, and they
/// are the same four the game's tuning screen offers.
///
/// It is the only part of the record that changes with what the player has bought — everything else
/// the record stores once — so it is the one thing a save can select. The 240SX **gains a sixth
/// gear** at level 3; the G35 ships with six.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GearboxLevel {
    #[default]
    Stock,
    One,
    Two,
    Three,
}

impl GearboxLevel {
    /// Index into [`CarHandling::gearbox`].
    #[must_use]
    pub fn slot(self) -> usize {
        match self {
            Self::Stock => 0,
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }

    /// The level a profile's transmission total implies.
    ///
    /// The per-category `f32` is the **sum of what is fitted**, measured at steps of 0.33 for this
    /// category — `0 → 0.33 → 0.66 → 0.99 → 1.32` over four purchases. Three products coexist there
    /// (gearbox, flywheel, differential), so the total is not a level; it is rounded down to one,
    /// which is a *reading* of the number rather than something the file states.
    #[must_use]
    pub fn from_transmission_total(total: f32) -> Self {
        match (total / 0.33).round() as i32 {
            i32::MIN..=0 => Self::Stock,
            1 => Self::One,
            2 => Self::Two,
            _ => Self::Three,
        }
    }
}

/// How the car is driven, from the record's rear-drive fraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drivetrain {
    Front,
    Rear,
    All,
}

impl Drivetrain {
    /// `0.0` is front, `1.0` is rear, anything between is all-wheel. The fraction partitions the
    /// playable cars exactly by their real drivetrains — PEUGOT and CIVIC read 0.0, the 240SX, G35
    /// and SUPRA read 1.0 — so the thresholds here only have to separate those three cases.
    #[must_use]
    pub fn of(rear_drive: f32) -> Self {
        if rear_drive <= 0.01 {
            Self::Front
        } else if rear_drive >= 0.99 {
            Self::Rear
        } else {
            Self::All
        }
    }
}

/// What a car's own record says about how it should drive.
#[derive(Clone, Debug)]
pub struct CarTune {
    /// Ready for `VehicleController::tuning`.
    pub tuning: VehicleTuning,
    /// Mass in kilograms, for the chassis rigid body.
    pub mass_kg: f32,
    /// Which axle is driven.
    pub drivetrain: Drivetrain,
    /// Body length, width, height in metres, as the record states them.
    pub body_m: [f32; 3],
    /// How many forward gears the selected gearbox has, for a HUD that wants to say so.
    pub gears: usize,
}

/// The steering lock, in radians.
///
/// **Invented.** The only steering angles in `GLOBALB.BUN` are a global ±43° identical in all 46
/// records, so a car-specific value cannot be read from this game's files — a row fed from it would
/// print the same number for a bus and a Skyline. This is the engine's own feel, kept where it was.
const STEERING_LOCK_RAD: f32 = 0.44;

/// Build engine tuning from a car's own record.
///
/// `wheelbase` and `track` come from the caller because they are geometry, not handling: the record
/// has no such field, and [`CarTypeInfo::wheels`] gives the four mounts the body is actually built
/// around. Passing them in keeps this function a pure map from one struct to another.
#[must_use]
pub fn tune_from_record(
    info: &CarTypeInfo,
    handling: &CarHandling,
    level: GearboxLevel,
    wheelbase: f32,
    track: f32,
) -> CarTune {
    let gb = &handling.gearbox[level.slot()];
    let gears = gb.gears();

    // `[reverse, neutral, forward…]` — the file's own order, unrearranged.
    let mut gear_ratios = Vec::with_capacity(gears.len() + 2);
    gear_ratios.push(gb.reverse);
    gear_ratios.push(0.0);
    gear_ratios.extend_from_slice(gears);

    // The curve's peak. The nine points have no rpm axis in the file, so the *shape* cannot be given
    // to an engine that wants torque at an rpm; its highest value is the one number that means the
    // same thing either way.
    let peak_nm = handling.torque_nm.iter().copied().fold(0.0_f32, f32::max);

    let mut tuning = VehicleTuning {
        idle_rpm: handling.engine.idle_rpm,
        max_rpm: handling.engine.limiter_rpm,
        gear_ratios,
        final_drive_ratio: gb.final_drive,
        upshift_rpm: handling.engine.red_line_rpm,
        wheelbase,
        track_width: track,
        max_engine_torque: peak_nm,
        ..VehicleTuning::default()
    };
    // Downshift has no counterpart in the record either. Half the red line keeps it below the
    // upshift point for every car measured, which is a rule about this field and not about NFSU2.
    tuning.downshift_rpm = handling.engine.red_line_rpm * 0.5;

    CarTune {
        tuning,
        mass_kg: info.mass_kg,
        drivetrain: Drivetrain::of(handling.rear_drive),
        body_m: handling.body_m,
        gears: gears.len(),
    }
}

/// The steering lock to give the controller. Its own function so the one invented number in this
/// module has a name and a place to be argued with.
#[must_use]
pub fn steering_lock() -> f32 {
    STEERING_LOCK_RAD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transmission_total_reads_as_a_level() {
        // The measured series for this category: 0 → 0.33 → 0.66 → 0.99 → 1.32.
        assert_eq!(GearboxLevel::from_transmission_total(0.0), GearboxLevel::Stock);
        assert_eq!(GearboxLevel::from_transmission_total(0.33), GearboxLevel::One);
        assert_eq!(GearboxLevel::from_transmission_total(0.66), GearboxLevel::Two);
        assert_eq!(GearboxLevel::from_transmission_total(0.99), GearboxLevel::Three);
        // Past the top of the measured series it stays at the top rather than indexing off the end.
        assert_eq!(GearboxLevel::from_transmission_total(1.32), GearboxLevel::Three);
        assert_eq!(GearboxLevel::from_transmission_total(99.0), GearboxLevel::Three);
        // And a negative, which no save should hold but no reader should trust it not to.
        assert_eq!(GearboxLevel::from_transmission_total(-1.0), GearboxLevel::Stock);
    }

    #[test]
    fn the_drive_fraction_names_the_drivetrain() {
        assert_eq!(Drivetrain::of(0.0), Drivetrain::Front);
        assert_eq!(Drivetrain::of(1.0), Drivetrain::Rear);
        assert_eq!(Drivetrain::of(0.5), Drivetrain::All);
    }
}
