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

/// CARSKIN shader hash (`0x00134013`, a painted body run). Mirrors `car::shader::CARSKIN`;
/// duplicated here so part selection can tell a paintable door skin from a glass-only door
/// without reaching into the engine layer.
const CARSKIN_SHADER: u32 = 0xd6d6_080a;

/// A door "skin" is the exterior door surface — not the inner `PANEL` card or `SILL` rocker.
fn is_door_skin(name: &str) -> bool {
    name.contains("DOOR") && !name.contains("PANEL") && !name.contains("SILL")
}

/// Whether the showroom car ships a **paintable exterior door skin**: a door-skin part carrying
/// a CARSKIN run, or with no material list (painted flat by name). Most cars do (240SX's
/// `DOOR_LEFT_A`). Some — RX8, CELICA, IS300, IMPREZAWRX, COROLLA — do not: they bake the door
/// into a *lower* body LOD, so their highest-triangle body LOD has a bare door hole that exposes
/// the dark interior (a "black door"). [`select_stock_car`] keys off this.
fn has_paintable_door_skin(all: &[NfsMeshPart]) -> bool {
    all.iter()
        .filter(|p| p.name.contains("_KIT00") || p.name.contains("_BASE"))
        .any(|p| {
            is_door_skin(&p.name)
                && (p.materials.is_empty()
                    || p.materials.iter().any(|m| m.shader.0 == CARSKIN_SHADER))
        })
}

/// Vertices in a part's "door zone" — mid-length, outer-width, mid-height of its own bounding box
/// (NFSU2 raw coords: x = length, y = width, z = height). A body LOD that bakes the door in has
/// many here; a LOD with a bare door hole (one that expects a separate door skin) has few.
fn door_zone_verts(p: &NfsMeshPart) -> usize {
    let mid = |i: usize| (p.bbox_min[i] + p.bbox_max[i]) * 0.5;
    let ext = |i: usize| (p.bbox_max[i] - p.bbox_min[i]).max(1e-3);
    p.positions
        .iter()
        .filter(|v| {
            (v[0] - mid(0)).abs() < 0.30 * ext(0) // middle of the length
                && (v[1] - mid(1)).abs() > 0.35 * ext(1) // outer width (either side)
                && (v[2] - mid(2)).abs() < 0.30 * ext(2) // middle of the height
        })
        .count()
}

/// Assemble the default (showroom) car: the shared `BASE` body plus kit slot `KIT00`,
/// picking the **highest-detail** variant of each logical component.
///
/// Highest detail = most triangles, which stays correct even when the LOD letter was
/// truncated out of the name (two LODs then share a [`component_key`]). Everything else is
/// dropped: other kits (`KIT01`+) and the `STYLE##` purchasable variants (a dozen alternate
/// headlights/spoilers/rims) would otherwise render as overlapping duplicates.
///
/// **Body exception:** a car with no paintable door skin ([`has_paintable_door_skin`]) bakes the
/// door into a lower body LOD; its highest-triangle LOD has a bare door hole. For the body
/// component of such a car, pick the LOD with the best [`door_zone_verts`] coverage (the one that
/// fills the door) instead of the one with the most triangles.
#[must_use]
pub fn select_stock_car(all: &[NfsMeshPart]) -> Vec<&NfsMeshPart> {
    use std::collections::HashMap;
    let fill_body_door = !has_paintable_door_skin(all);
    let mut best: HashMap<&str, &NfsMeshPart> = HashMap::new();
    for p in all {
        // The showroom set is the shared BASE part (which carries the greenhouse: glass,
        // window masks, interior and trim) plus kit slot 00, plus the window glass on any
        // un-kitted `DECAL_*_WINDOW` parts. BASE is kept — its materials are routed per-shader,
        // so its interior/trim reads correctly instead of as a bright painted shell.
        let is_default = p.name.contains("_BASE")
            || p.name.contains("_KIT00")
            || (p.name.contains("DECAL") && p.name.contains("WINDOW"));
        // Drop only the TRUNK_AUDIO second shell that z-fights the TRUNK it sits on. (ROOF is
        // kept — it is the only roof; closing the cabin is what stops the interior spilling out.)
        let is_duplicate_shell = p.name.contains("TRUNK_AUDIO");
        if !is_default || is_duplicate_shell || group_of(&p.name) == Grp::Skip {
            continue;
        }
        let prefer_door_fill = fill_body_door && p.name.contains("_BODY");
        best.entry(component_key(&p.name))
            .and_modify(|cur| {
                let better = if prefer_door_fill {
                    // Door coverage first, triangles as the tie-break.
                    (door_zone_verts(p), p.triangle_count())
                        > (door_zone_verts(cur), cur.triangle_count())
                } else {
                    p.triangle_count() > cur.triangle_count()
                };
                if better {
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

    // NfsMeshPart / NfsMaterialRange are #[non_exhaustive], so build via Default + field set.
    fn body_lod(name: &str, tris: usize, door_verts: usize) -> NfsMeshPart {
        // bbox mid=0, ext=(4,2,2) → door zone: |x|<1.2, |y|>0.7, |z|<0.6.
        let mut positions = vec![[0.0, 0.0, 0.0]]; // one out-of-zone vertex
        positions.extend(std::iter::repeat_n([0.0, 0.9, 0.0], door_verts)); // in-zone
        let mut p = NfsMeshPart::default();
        p.name = name.to_string();
        p.positions = positions;
        p.indices = vec![0; tris * 3];
        p.bbox_min = [-2.0, -1.0, -1.0];
        p.bbox_max = [2.0, 1.0, 1.0];
        p
    }

    fn door_skin(name: &str, shader: u32) -> NfsMeshPart {
        let mut m = gizmo_nfs::types::NfsMaterialRange::default();
        m.shader = gizmo_nfs::AssetHash(shader);
        let mut p = NfsMeshPart::default();
        p.name = name.to_string();
        p.materials = vec![m];
        p
    }

    #[test]
    fn body_lod_picks_door_fill_when_no_paintable_door_skin() {
        // A car whose only door skin is glass (no CARSKIN, not empty) has a bare door hole in its
        // high-triangle body LOD → select the lower-triangle LOD that fills the door.
        let parts = vec![
            body_lod("RX8_KIT00_BODY_A", 100, 0), // most triangles, but door hole
            body_lod("RX8_KIT00_BODY_B", 50, 5),  // fewer triangles, door filled
            door_skin("RX8_KIT00_DOOR_LEFT_B", 0x471a_1dca), // glass only → not paintable
        ];
        let body = select_stock_car(&parts)
            .into_iter()
            .find(|p| p.name.contains("_BODY"))
            .expect("a body part");
        assert_eq!(body.name, "RX8_KIT00_BODY_B");
    }

    #[test]
    fn body_lod_keeps_highest_triangles_when_a_real_door_skin_exists() {
        // A car with a CARSKIN door skin keeps the max-triangle body LOD (no door-fill override),
        // so cars like the 240SX are untouched.
        let parts = vec![
            body_lod("240SX_KIT00_BODY_A", 100, 0),
            body_lod("240SX_KIT00_BODY_B", 50, 5),
            door_skin("240SX_KIT00_DOOR_LEFT_A", CARSKIN_SHADER), // paintable skin
        ];
        let body = select_stock_car(&parts)
            .into_iter()
            .find(|p| p.name.contains("_BODY"))
            .expect("a body part");
        assert_eq!(body.name, "240SX_KIT00_BODY_A");
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
