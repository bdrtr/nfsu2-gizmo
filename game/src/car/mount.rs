//! The parts `GEOMETRY.BIN` models about the origin and never places, hung where its markers say.
//!
//! Every playable car in an install ships its exhaust tip and its two brake meshes modelled about
//! the origin with an identity or rotation-only matrix, and some ship the spoiler the same way
//! (MUSTANGGT). Drawn as they stand they all land at the car's origin, the middle of its floor —
//! the "tow bar" under every car. The file does say where they go, in position markers
//! (`0x0013401A`, [`gizmo_nfs::NfsMarker`]) on *other* solids:
//!
//! | part | marker | on | count |
//! |---|---|---|---|
//! | `EXHAUST_A` | `LEFT_EXHAUST`, `RIGHT_EXHAUST` | the rear bumper (or a widebody's body) | one per exit: 240SX one, 350Z two |
//! | `SPOILER_A` | `SPOILER` | `BASE` | one; a *placed* spoiler's own matrix holds exactly this translation (SKYLINE −2.17 / 0 / 0.787 against −2.17 / 0 / 0.79) |
//! | `FRONT_BRAKE_A`, `REAR_BRAKE_A` | `MARKER_BRAKE` | the wheel, in the wheel's own space | one; the brake is instanced per wheel by the caller |
//!
//! Pure: parts in, parts out. Only an *unplaced* part is touched (its placed centroid within
//! [`UNPLACED`] of the origin), so a file that does place one keeps it as it is.

use crate::parts::{group_of, Grp};
use gizmo::prelude::Vec3;
use gizmo_nfs::placement::{part_centroid, place_point, should_place};
use gizmo_nfs::NfsMeshPart;

/// How near the origin a part's placed centroid must be to count as never placed, metres. The
/// unplaced exhausts, brakes and spoilers of an install sit within 0.21 m (NAVIGATOR's exhaust);
/// a placed one is a metre or more out. Only the parts named in [`Mount`] are tested against it.
pub const UNPLACED: f32 = 0.35;

/// What a part waits to be hung on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mount {
    /// An exhaust tip: one copy per `LEFT_EXHAUST` / `RIGHT_EXHAUST` marker.
    Exhaust,
    /// A spoiler: `BASE`'s `SPOILER` marker.
    Spoiler,
    /// A brake disc, `true` for the front axle: hung inside each wheel by the caller.
    Brake(bool),
}

impl Mount {
    /// The mount `name` waits for, if it is one of the parts the file leaves at the origin. Stems,
    /// because names arrive truncated (`_FRONT_BRAK`).
    #[must_use]
    pub fn of(name: &str) -> Option<Self> {
        if name.contains("BRAKELIGHT") {
            None
        } else if name.contains("_FRONT_BRAK") {
            Some(Self::Brake(true))
        } else if name.contains("_REAR_BRAK") {
            Some(Self::Brake(false))
        } else if group_of(name) == Grp::Exhaust {
            Some(Self::Exhaust)
        } else if name.contains("_SPOILER") {
            Some(Self::Spoiler)
        } else {
            None
        }
    }

    /// The marker names that place this mount on a car.
    #[must_use]
    pub fn markers(self) -> &'static [&'static str] {
        match self {
            Self::Exhaust => &["LEFT_EXHAUST", "RIGHT_EXHAUST"],
            Self::Spoiler => &["SPOILER"],
            Self::Brake(_) => &["MARKER_BRAKE"],
        }
    }
}

/// A part's centroid where its own matrix puts it, NFSU2 frame.
fn placed_centroid(p: &NfsMeshPart) -> [f32; 3] {
    let apply = should_place(&p.transform, &part_centroid(p));
    let n = p.positions.len().max(1) as f32;
    let mut c = [0.0f32; 3];
    for v in &p.positions {
        let v = place_point(&p.transform, *v, apply);
        for i in 0..3 {
            c[i] += v[i] / n;
        }
    }
    c
}

/// Whether the file left `p` at the origin.
#[must_use]
pub fn is_unplaced(p: &NfsMeshPart) -> bool {
    !p.positions.is_empty() && Vec3::from(placed_centroid(p)).length() < UNPLACED
}

/// Every marker named in `names` across `parts`, in the car's space (each through its own part's
/// placement), duplicates dropped — a bumper's LODs carry the same markers.
#[must_use]
pub fn marker_positions(parts: &[&NfsMeshPart], names: &[&str]) -> Vec<[f32; 3]> {
    let mut out: Vec<[f32; 3]> = Vec::new();
    for p in parts {
        let apply = should_place(&p.transform, &part_centroid(p));
        for name in names {
            let h = gizmo_nfs::hash::string_hash(name);
            for m in p.markers.iter().filter(|m| m.hash.0 == h) {
                let at = place_point(&p.transform, m.position(), apply);
                if !out.iter().any(|o| (Vec3::from(*o) - Vec3::from(at)).length() < 1e-3) {
                    out.push(at);
                }
            }
        }
    }
    out
}

/// `p` moved so its origin lands on `at`, keeping whatever rotation its matrix carries.
#[must_use]
pub fn hung_at(p: &NfsMeshPart, at: [f32; 3]) -> NfsMeshPart {
    let mut q = p.clone();
    q.transform[3][0] = at[0];
    q.transform[3][1] = at[1];
    q.transform[3][2] = at[2];
    q
}

/// `p` at `at` with its own matrix's rotation dropped: the raw geometry, translated.
///
/// For the brakes. Their matrices cannot be the pose inside a wheel — a SKYLINE's rear brake's
/// turns the disc flat, face up — while the raw geometry is already in the wheel's frame (a disc
/// on the wheel's axle, as wide as the rim is), so the marker's offset is all it needs.
#[must_use]
pub fn raw_at(p: &NfsMeshPart, at: [f32; 3]) -> NfsMeshPart {
    let mut q = p.clone();
    q.transform = gizmo_nfs::types::IDENTITY;
    q.transform[3][0] = at[0];
    q.transform[3][1] = at[1];
    q.transform[3][2] = at[2];
    q
}

/// The body-side mounts: every unplaced exhaust and spoiler in `stock`, hung on its markers.
/// Returns the hung copies; the caller drops the originals ([`is_unplaced`] with a body
/// [`Mount`]). An unplaced part whose markers are missing gets no copy — not drawn at all is
/// better than drawn under the floor.
#[must_use]
pub fn hang_body_parts(stock: &[&NfsMeshPart]) -> Vec<NfsMeshPart> {
    let mut out = Vec::new();
    for p in stock {
        let Some(mount @ (Mount::Exhaust | Mount::Spoiler)) = Mount::of(&p.name) else { continue };
        if !is_unplaced(p) {
            continue;
        }
        for at in marker_positions(stock, mount.markers()) {
            // An exhaust's own rotation is dropped like a brake's: 240SX, SKYLINE and MUSTANGGT
            // model the pipe along +x with an identity matrix, and it reads right hung that way —
            // the tip at the marker, the pipe running forward under the car. 350Z's matrix (and
            // every exhaust marker's own frame) turns +x straight up, which stands the pipe on end
            // as a box hanging under the bumper. A spoiler keeps whatever its matrix says.
            out.push(if mount == Mount::Exhaust { raw_at(p, at) } else { hung_at(p, at) });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounts_are_named_through_the_truncation() {
        assert_eq!(Mount::of("IMPREZAWRX_KIT00_FRONT_BRAK"), Some(Mount::Brake(true)));
        assert_eq!(Mount::of("SKYLINE_KIT00_REAR_BRAKE_A"), Some(Mount::Brake(false)));
        assert_eq!(Mount::of("240SX_KIT00_EXHAUST_A"), Some(Mount::Exhaust));
        assert_eq!(Mount::of("MUSTANGGT_KIT00_SPOILER_A"), Some(Mount::Spoiler));
        // A brake *light* is not a brake.
        assert_eq!(Mount::of("240SX_KIT00_BRAKELIGHT_A"), None);
        assert_eq!(Mount::of("X_KIT00_REAR_BRAKELIGHT_A"), None);
        assert_eq!(Mount::of("240SX_KIT00_HOOD_A"), None);
    }
}
