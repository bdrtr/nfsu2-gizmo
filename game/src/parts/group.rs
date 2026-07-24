//! Which material group a part is rendered with, decided purely from its name.

/// The material category a part is rendered with, decided purely from its name.
///
/// `Skip` parts are deliberately never rendered: engine-bay and underbody geometry that
/// is hidden on a grounded car, and texture-only decals that only look right once the TPK
/// texture is applied (as flat colour they z-fight the body they sit on).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Grp {
    /// Painted body panels.
    Paint,
    /// Windows / windshield.
    Glass,
    /// Chrome trim and mirrors.
    Chrome,
    /// Head-lamp lenses.
    Headlight,
    /// Brake / tail lights.
    Brakelight,
    /// Exhaust tips.
    Exhaust,
    /// Dark plastic / miscellaneous trim.
    Trim,
    /// Wheels (tire + rim), handled separately from the body.
    Wheel,
    /// Never rendered.
    Skip,
}

/// Classify a part into a [`Grp`] from its (often fixed-length-truncated) name.
///
/// Ordering matters: the `Skip` and `Wheel` keywords are tested before the broad body
/// keywords so that, e.g., `KIT00_FRONT_WHEEL_A` is a wheel and `..._HOOD_UNDER_A` is
/// skipped rather than painted.
#[must_use]
pub fn group_of(name: &str) -> Grp {
    // Decals are livery overlays — coplanar with the body, they only look right with the
    // real texture; as flat colour they z-fight, so drop them until TPK is decoded. The
    // exception is the window decals, which are the actual glass panels sitting *inside*
    // the greenhouse openings (not on a painted surface), so render them as glass.
    if name.contains("DECAL") {
        if name.contains("WINDOW") {
            return Grp::Glass;
        }
        return Grp::Skip;
    }
    // `WHEE`, not `WHEEL`: the fixed-length name field clips the tail, and on a long car name it
    // eats the L too (`IMPREZAWRX_KIT00_FRONT_WHEE`, `LANCEREVO8_KIT00_REAR_WHEE`). Matching the
    // full word left those cars' wheels classified as trim — baked into the body mesh as a dark
    // lump, with nothing left for the wheel instancing, so the car rolled on empty arches.
    if name.contains("WHEE") || name.contains("TIRE") || name.contains("RIM") {
        return Grp::Wheel;
    }
    // Hidden/internal geometry (engine bay, underbody, unlocked audio panels).
    if name.contains("ENGINE") || name.contains("UNDER") || name.contains("UNL") {
        return Grp::Skip;
    }
    if name.contains("WINDOW") || name.contains("WINDSHIELD") || name.contains("GLASS") {
        return Grp::Glass;
    }
    if name.contains("HEADLIGHT") {
        return Grp::Headlight;
    }
    if name.contains("BRAKELIGHT") || name.contains("TAILLIGHT") {
        return Grp::Brakelight;
    }
    // NFSU2 truncates long names: the side mirrors arrive as `..._MIRRO` (left) and
    // `..._MIRR` (right), so match the shortened stem rather than the full word.
    if name.contains("MIRR") {
        return Grp::Chrome;
    }
    if name.contains("EXHAUST") {
        return Grp::Exhaust;
    }
    if name.contains("BASE")
        || name.contains("BODY")
        || name.contains("HOOD")
        || name.contains("DOOR")
        || name.contains("FENDER")
        || name.contains("BUMPER")
        || name.contains("TRUNK")
        || name.contains("SKIRT")
        || name.contains("ROOF")
        || name.contains("SPOILER")
        || name.contains("QUARTER")
    {
        return Grp::Paint;
    }
    Grp::Trim
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_keyword_with_correct_precedence() {
        // A kit-prefixed wheel is a wheel, not paint.
        assert_eq!(group_of("240SX_KIT00_FRONT_WHEEL_A"), Grp::Wheel);
        // …and still a wheel when the name field clipped the `L` off (long car names).
        assert_eq!(group_of("IMPREZAWRX_KIT00_FRONT_WHEE"), Grp::Wheel);
        assert_eq!(group_of("LANCEREVO8_KIT00_REAR_WHEE"), Grp::Wheel);
        // Hidden geometry wins over the body keyword it also contains.
        assert_eq!(group_of("240SX_KIT00_HOOD_UNDER_A"), Grp::Skip);
        assert_eq!(group_of("240SX_KIT00_ENGINE_A"), Grp::Skip);
        // Window decals are the glass panels; other decals are texture-only livery.
        assert_eq!(group_of("240SX_DECAL_FRONT_WINDOW_WI"), Grp::Glass);
        assert_eq!(group_of("240SX_DECAL_LEFT_QUARTER_RE"), Grp::Skip);
        // Ordinary body / trim.
        assert_eq!(group_of("240SX_BASE_A"), Grp::Paint);
        assert_eq!(group_of("240SX_KIT00_HEADLIGHT_LEFT_"), Grp::Headlight);
        assert_eq!(group_of("240SX_KIT00_BRAKELIGHT_A"), Grp::Brakelight);
        // Both side mirrors, truncated to different lengths by the fixed-size name field.
        assert_eq!(group_of("240SX_KIT00_LEFT_SIDE_MIRRO"), Grp::Chrome);
        assert_eq!(group_of("240SX_KIT00_RIGHT_SIDE_MIRR"), Grp::Chrome);
        assert_eq!(group_of("240SX_KIT00_EXHAUST_A"), Grp::Exhaust);
        assert_eq!(group_of("240SX_KIT00_ANTENNA"), Grp::Trim);
    }
}
