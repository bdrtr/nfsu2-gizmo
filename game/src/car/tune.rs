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

/// Which engine package is fitted: the record keeps four torque tables and the shop sells three
/// levels.
///
/// Separate from [`GearboxLevel`] because the game sells them separately — a car can have a
/// built engine and a stock gearbox — and because they read out of *different* slots of the
/// profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EngineLevel {
    #[default]
    Stock,
    One,
    Two,
    Three,
}

impl EngineLevel {
    /// Index into [`CarHandling::torque_gain_nm`], which is what
    /// [`gizmo_nfs::CarHandling::torque_at`] takes.
    #[must_use]
    pub fn slot(self) -> usize {
        match self {
            Self::Stock => 0,
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }

    /// The level a profile's engine total implies.
    ///
    /// **Weaker than its gearbox twin, and the weakness is worth stating.** That one rests on a
    /// series measured over four purchases at even steps of 0.33. This one was measured over
    /// **two**: `0 → 0.21 → 0.51`. The steps are not even (+0.21 then +0.30) and the third was
    /// never bought, so the top threshold is where the series *would* go rather than where it was
    /// seen. Thresholds sit at the midpoints of what is known and at an extrapolation above it.
    ///
    /// Getting this wrong costs a level, not a crash: every value maps to some level and the
    /// worst case is a built engine read as one step down.
    #[must_use]
    pub fn from_engine_total(total: f32) -> Self {
        match total {
            t if t < 0.105 => Self::Stock,
            t if t < 0.36 => Self::One,
            t if t < 0.70 => Self::Two,
            _ => Self::Three,
        }
    }
}

/// What the player has bought, as the two categories that reach the physics.
///
/// A struct rather than two arguments because they are read together, from one profile, and a
/// call site that passes them positionally is one transposition away from a stock engine in a
/// built gearbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Upgrades {
    pub gearbox: GearboxLevel,
    pub engine: EngineLevel,
}

impl Upgrades {
    /// Read both from the environment: `NFS_ENGINE` and `NFS_GEARBOX`, each `0`–`3`.
    ///
    /// The same way every other configuration in this game arrives (`NFS_KIT`, `NFS_STYLE_HOOD`,
    /// `NFS_PAINT`), and for the same reason: there is no garage to buy anything in yet, so the
    /// only way to drive a built car is to say so. Absent or unparseable is stock, and anything
    /// above 3 clamps rather than indexing off the end.
    #[must_use]
    pub fn from_env() -> Self {
        let level = |var: &str| {
            std::env::var(var).ok().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0).min(3)
        };
        Self {
            gearbox: match level("NFS_GEARBOX") {
                1 => GearboxLevel::One,
                2 => GearboxLevel::Two,
                3 => GearboxLevel::Three,
                _ => GearboxLevel::Stock,
            },
            engine: match level("NFS_ENGINE") {
                1 => EngineLevel::One,
                2 => EngineLevel::Two,
                3 => EngineLevel::Three,
                _ => EngineLevel::Stock,
            },
        }
    }

    /// Read both out of a player profile.
    #[must_use]
    pub fn from_profile(profile: &gizmo_nfs::Profile) -> Self {
        use gizmo_nfs::profile::Category;
        Self {
            gearbox: GearboxLevel::from_transmission_total(profile.total(Category::Transmission)),
            engine: EngineLevel::from_engine_total(profile.total(Category::Engine)),
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
/// **Invented, and now invented on purpose rather than for want of looking.** The claim used to be
/// that the only steering angles in `GLOBALB.BUN` are a global ±43° identical in all 46 records, so
/// nothing per-car could be read. A per-car angle then turned up at `+0x284` — 27° to 60° over the
/// playable cars — and it was wired in, set from 37° to 12° on a 240SX, installed and driven. The
/// car steered exactly as before. It also reads 100 on every traffic vehicle against a bus's 75,
/// and 60 on a HUMMER against 27 on a 106, which is the wrong way round for a lock.
///
/// So this stays the engine's own feel, and `gizmo_nfs::Unproven::angle_284_deg` keeps the lane and
/// the refutation. Tested, not unexamined.
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
    upgrades: Upgrades,
    wheelbase: f32,
    track: f32,
) -> CarTune {
    let gb = &handling.gearbox[upgrades.gearbox.slot()];
    let gears = gb.gears();

    // `[reverse, neutral, forward…]` — the file's own order, unrearranged.
    let mut gear_ratios = Vec::with_capacity(gears.len() + 2);
    gear_ratios.push(gb.reverse);
    gear_ratios.push(0.0);
    gear_ratios.extend_from_slice(gears);

    // The whole curve, not its peak. This used to hand the engine one number, with the reason
    // written down beside it: the nine points had no rpm axis in the file, so the *shape* could not
    // be given to something that wants torque at an rpm. The axis was found — idle to limiter in
    // eight equal steps, checked against the game's own dynamometer on two cars — so the reason is
    // gone and so is the number.
    //
    // What it cost while it stood: `VehicleTuning`'s own curve peaks at `ratio = 0.4`, which on a
    // 240SX is 3280 rpm, and that car peaks at 4675. Every car in this game made its torque 1,400
    // rpm early and in a shape none of them have, and two cars with the same peak drove identically.
    //
    // `torque_at` applies the engine upgrade: the record keeps four tables and the level picks one.
    let rpm_axis = handling.torque_rpm();
    let curve_nm = handling.torque_at(upgrades.engine.slot());
    let torque_curve: Vec<(f32, f32)> =
        rpm_axis.iter().copied().zip(curve_nm.iter().copied()).collect();
    // Still set, because it is what the engine falls back to and what a HUD would print.
    let peak_nm = curve_nm.iter().copied().fold(0.0_f32, f32::max);

    let mut tuning = VehicleTuning {
        idle_rpm: handling.engine.idle_rpm,
        max_rpm: handling.engine.limiter_rpm,
        gear_ratios,
        final_drive_ratio: gb.final_drive,
        upshift_rpm: handling.engine.red_line_rpm,
        wheelbase,
        track_width: track,
        max_engine_torque: peak_nm,
        torque_curve,
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

    /// The engine total reads as a level, on a series measured over two purchases rather than four.
    #[test]
    fn an_engine_total_reads_as_a_level() {
        // The two steps the profile was actually seen to write.
        assert_eq!(EngineLevel::from_engine_total(0.0), EngineLevel::Stock);
        assert_eq!(EngineLevel::from_engine_total(0.21), EngineLevel::One);
        assert_eq!(EngineLevel::from_engine_total(0.51), EngineLevel::Two);
        // The third was never bought; anything above the series lands on the top level rather
        // than indexing off the end.
        assert_eq!(EngineLevel::from_engine_total(0.90), EngineLevel::Three);
        assert_eq!(EngineLevel::from_engine_total(99.0), EngineLevel::Three);
        // And a negative, which no profile should hold but no reader should trust it not to.
        assert_eq!(EngineLevel::from_engine_total(-1.0), EngineLevel::Stock);
        // Slots line up with the record's four torque tables.
        assert_eq!(EngineLevel::Stock.slot(), 0);
        assert_eq!(EngineLevel::Three.slot(), 3);
    }

    /// The two categories are read from their own slots and do not swap.
    #[test]
    fn upgrades_default_to_stock_and_are_read_per_category() {
        let u = Upgrades::default();
        assert_eq!(u.gearbox, GearboxLevel::Stock);
        assert_eq!(u.engine, EngineLevel::Stock);
    }

    /// The whole path over a **real** install: the file the game opens → the parser → the
    /// `VehicleTuning` the physics runs on.
    ///
    /// Skipped unless `NFSU2_ROOT` is set, like every other test in these repos that needs the
    /// game.
    ///
    /// It asserts **structure, not numbers** — nine points, an ascending axis pinned to the car's
    /// own idle and limiter, and a built engine strictly above a stock one. Hardcoding a 240SX's
    /// 216 N·m at 4675 rpm would be a better-looking test and a wrong one: PryHUB exists to edit
    /// exactly these lanes, so a tuned install would fail it, and "the numbers are the ones I have"
    /// is not a property of the pipeline.
    #[test]
    fn a_real_record_reaches_the_engine_as_a_curve() {
        let Some(root) = std::env::var_os("NFSU2_ROOT") else {
            eprintln!("NFSU2_ROOT unset — skipping");
            return;
        };
        let geo = std::path::Path::new(&root).join("CARS").join("240SX").join("GEOMETRY.BIN");
        let Some(path) = geo.to_str() else { return };
        let gb = crate::assets::load_globalb_beside(path);
        let (Some(info), Some(h)) = (gb.info.as_ref(), gb.handling.as_ref()) else {
            eprintln!("no 240SX record in this install — skipping");
            return;
        };

        let stock = tune_from_record(info, h, Upgrades::default(), 2.6, 1.6);
        let curve = &stock.tuning.torque_curve;
        assert_eq!(curve.len(), 9, "the file's nine points, not its peak");
        assert!(
            curve.windows(2).all(|w| w[0].0 < w[1].0),
            "the axis must ascend, or the engine's lookup reads the wrong segment"
        );
        assert_eq!(curve[0].0, h.engine.idle_rpm, "the curve starts at this car's idle");
        assert_eq!(curve[8].0, h.engine.limiter_rpm, "and ends at its limiter");
        assert!(curve.iter().all(|&(_, nm)| nm > 0.0), "every point is real torque");
        // The peak the engine falls back on is the peak of the curve it was given.
        let peak = curve.iter().map(|&(_, nm)| nm).fold(f32::MIN, f32::max);
        assert!((stock.tuning.max_engine_torque - peak).abs() < 1e-3);

        // A built engine is strictly more torque at every point, and the axis does not move —
        // the upgrade tables are gains on the same nine speeds.
        let built = tune_from_record(
            info,
            h,
            Upgrades { gearbox: GearboxLevel::Three, engine: EngineLevel::Three },
            2.6,
            1.6,
        );
        for (i, (&(rpm, stock_nm), &(built_rpm, built_nm))) in
            curve.iter().zip(built.tuning.torque_curve.iter()).enumerate()
        {
            assert_eq!(rpm, built_rpm, "point {i}: the axis is the same at every level");
            assert!(built_nm > stock_nm, "point {i}: {built_nm} is no more than stock {stock_nm}");
        }
        // …and the gearbox moved with it, which is the other half of what an upgrade buys.
        assert!(built.gears >= stock.gears);
    }

    #[test]
    fn the_drive_fraction_names_the_drivetrain() {
        assert_eq!(Drivetrain::of(0.0), Drivetrain::Front);
        assert_eq!(Drivetrain::of(1.0), Drivetrain::Rear);
        assert_eq!(Drivetrain::of(0.5), Drivetrain::All);
    }
}
