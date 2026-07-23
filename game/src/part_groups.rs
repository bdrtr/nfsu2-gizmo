//! Pure, engine-free classification of NFSU2 car parts.
//!
//! This module owns the *presentation policy* for a car's geometry: which material group
//! a part belongs to, and which parts make up the default (showroom) configuration. It
//! touches no engine or GPU types — only part names and triangle counts — so it stays
//! trivially unit-testable and reusable across every demo binary.

use gizmo_nfs::NfsMeshPart;

/// The material category a part is rendered with, decided purely from its name.
///
/// `Skip` parts are deliberately never rendered: engine-bay and underbody geometry that
/// is hidden on a grounded car, and texture-only decals that only look right once the TPK
/// texture is applied (as flat colour they z-fight the body they sit on).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    if name.contains("WHEEL") || name.contains("TIRE") || name.contains("RIM") {
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

/// Strip a trailing `_A`..`_D` LOD suffix to get a logical component key, so the four LOD
/// variants of one panel collapse to a single component.
///
/// Names truncated by NFSU2's fixed-length name field (e.g. `..._HEADLIGHT_LEFT_`) have no
/// such suffix and key as-is; two of their LODs then share a key and are disambiguated by
/// triangle count in [`select_stock_car`].
#[must_use]
pub fn component_key(name: &str) -> &str {
    let b = name.as_bytes();
    if b.len() >= 2 && b[b.len() - 2] == b'_' && matches!(b[b.len() - 1], b'A'..=b'D') {
        &name[..name.len() - 2]
    } else {
        name
    }
}

/// Assemble the default (showroom) car: the shared `BASE` body plus kit slot `KIT00`,
/// picking the **highest-detail** variant of each logical component.
///
/// Highest detail = most triangles, which stays correct even when the LOD letter was
/// truncated out of the name (two LODs then share a [`component_key`]). Everything else is
/// dropped: other kits (`KIT01`+) and the `STYLE##` purchasable variants (a dozen alternate
/// headlights/spoilers/rims) would otherwise render as overlapping duplicates.
#[must_use]
pub fn select_stock_car(all: &[NfsMeshPart]) -> Vec<&NfsMeshPart> {
    use std::collections::HashMap;
    let mut best: HashMap<&str, &NfsMeshPart> = HashMap::new();
    for p in all {
        // The showroom set is the shared BASE body + kit slot 00, plus the window glass
        // (which lives on un-kitted `DECAL_*_WINDOW` parts, so admit it explicitly).
        let is_default = p.name.contains("_BASE")
            || p.name.contains("_KIT00")
            || (p.name.contains("DECAL") && p.name.contains("WINDOW"));
        // Drop the second-shell duplicates that z-fight the parts they sit on: TRUNK_AUDIO
        // over TRUNK, and the separate ROOF / FULLROOF panels over the BASE roof they overlay.
        let is_duplicate_shell = p.name.contains("TRUNK_AUDIO") || p.name.contains("ROOF");
        if !is_default || is_duplicate_shell || group_of(&p.name) == Grp::Skip {
            continue;
        }
        best.entry(component_key(&p.name))
            .and_modify(|cur| {
                if p.triangle_count() > cur.triangle_count() {
                    *cur = p;
                }
            })
            .or_insert(p);
    }
    best.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_keyword_with_correct_precedence() {
        // A kit-prefixed wheel is a wheel, not paint.
        assert_eq!(group_of("240SX_KIT00_FRONT_WHEEL_A"), Grp::Wheel);
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

    #[test]
    fn component_key_collapses_lod_suffix_only() {
        assert_eq!(component_key("240SX_KIT00_BODY_A"), "240SX_KIT00_BODY");
        assert_eq!(component_key("240SX_KIT00_BODY_D"), "240SX_KIT00_BODY");
        // No trailing single A..D letter → unchanged (truncated names, or non-LOD names).
        assert_eq!(component_key("240SX_KIT00_HEADLIGHT_LEFT_"), "240SX_KIT00_HEADLIGHT_LEFT_");
        assert_eq!(component_key("240SX_KIT00_TRUNK_AUDIO_UNL"), "240SX_KIT00_TRUNK_AUDIO_UNL");
    }
}
