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

/// A car configuration: which aftermarket parts replace the stock ones. `0` in any field means
/// "stock" (kit slot `KIT00`), so [`CarConfig::stock`] (all zero) is the default showroom car.
/// The NFSU2 geometry only holds body kits, hood/light styles and widebody sets per car; the
/// universal aftermarket spoilers and rims live in a separate global bundle, so they are not
/// configured here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarConfig {
    /// Body kit `KIT##` sourcing the front bumper, rear bumper and side skirt (`0` = stock).
    pub body_kit: u8,
    /// Hood design `STYLE##` (`0` = stock `KIT00` hood).
    pub hood_style: u8,
    /// Head- and tail-light design `STYLE##` (`STYLE01..14`; `0` = stock).
    pub light_style: u8,
    /// Widebody kit `KITW##` replacing the body and doors (`0` = stock body).
    pub widebody: u8,
}

impl CarConfig {
    /// The stock showroom car (all slots `KIT00`).
    #[must_use]
    pub const fn stock() -> Self {
        Self { body_kit: 0, hood_style: 0, light_style: 0, widebody: 0 }
    }

    /// Build a config from `NFS_KIT` / `NFS_STYLE_HOOD` / `NFS_STYLE_LIGHT` / `NFS_WIDE`
    /// environment variables (each a decimal part number; absent or unparsable = stock).
    #[must_use]
    pub fn from_env() -> Self {
        let n = |k: &str| std::env::var(k).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        Self {
            body_kit: n("NFS_KIT"),
            hood_style: n("NFS_STYLE_HOOD"),
            light_style: n("NFS_STYLE_LIGHT"),
            widebody: n("NFS_WIDE"),
        }
    }
}

impl Default for CarConfig {
    fn default() -> Self {
        Self::stock()
    }
}

/// The customization namespace a part name belongs to, parsed from its `KIT##` / `KITW##` /
/// `STYLE##` token (or `BASE`). Survives NFSU2's 27-char name truncation, which only clips the
/// trailing LOD/side suffix, never the leading namespace token.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ns {
    /// The shared `BASE` part (greenhouse, interior, trim).
    Base,
    /// Kit slot `KIT##` (`00` = stock; `01+` aftermarket body kits).
    Kit(u8),
    /// Purchasable `STYLE##` (hoods, lights, engine bays).
    Style(u8),
    /// Widebody kit `KITW##`.
    Wide(u8),
    /// No customization token (miscellaneous / decals).
    Other,
}

/// Parse the decimal number immediately following `tag` in `name`, if any.
fn num_after(name: &str, tag: &str) -> Option<u8> {
    let i = name.find(tag)? + tag.len();
    let digits: String = name[i..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn namespace(name: &str) -> Ns {
    // `KITW##` before `KIT##` (the former contains the latter's letters).
    if let Some(n) = num_after(name, "KITW") {
        return Ns::Wide(n);
    }
    if let Some(n) = num_after(name, "STYLE") {
        return Ns::Style(n);
    }
    if let Some(n) = num_after(name, "KIT") {
        return Ns::Kit(n);
    }
    if name.contains("_BASE") {
        return Ns::Base;
    }
    Ns::Other
}

/// The customization slot a part fills — decides which namespace sources it (see [`select_car`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    FrontBumper,
    RearBumper,
    Skirt,
    Hood,
    Headlight,
    Brakelight,
    Body,
    Door,
    /// Everything not swapped by a config dimension (mirrors, trunk, exhaust, wheel, spoiler,
    /// roof, …) — always sourced from the stock kit.
    Fixed,
}

fn slot_of(name: &str) -> Slot {
    if name.contains("FRONT_BUMPER") {
        Slot::FrontBumper
    } else if name.contains("REAR_BUMPER") {
        Slot::RearBumper
    } else if name.contains("SKIRT") {
        Slot::Skirt
    } else if name.contains("HOOD") {
        Slot::Hood
    } else if name.contains("HEADLIGHT") {
        Slot::Headlight
    } else if name.contains("BRAKELIGHT") {
        Slot::Brakelight
    } else if name.contains("BODY") {
        Slot::Body
    } else if name.contains("DOOR") {
        Slot::Door
    } else {
        Slot::Fixed
    }
}

/// Assemble a car in configuration `cfg` from all parsed parts: the shared `BASE` greenhouse plus
/// one part per component, each sourced from the namespace `cfg` selects for its slot, picking the
/// **highest-detail** LOD of the chosen part.
///
/// The routing is per-slot: bumpers + skirt come from the body kit; the hood from the hood style;
/// head/tail lights from the light style; body + doors from the widebody kit (else stock). Because
/// a part's [`component_key`] embeds its `KIT##`/`STYLE##` token, an *unselected* namespace's part
/// is NOT collapsed away by the LOD dedup — so it must be filtered out **before** the dedup, or two
/// hoods/bumpers would render on top of each other. If the requested variant has no part for a slot
/// (kit numbering is sparse; not every car has every style), that dimension falls back to stock so
/// a bad pick degrades to `KIT00` rather than leaving a hole.
///
/// **Body exception** (unchanged from stock): a car with no paintable door skin bakes the door into
/// a lower body LOD, so its body component picks the LOD with the best [`door_zone_verts`] coverage.
#[must_use]
pub fn select_car<'a>(all: &'a [NfsMeshPart], cfg: &CarConfig) -> Vec<&'a NfsMeshPart> {
    use std::collections::HashMap;

    // A requested variant that has no part for its target slots falls back to stock (0).
    let has = |ns: Ns, slots: &[Slot]| {
        all.iter().any(|p| namespace(&p.name) == ns && slots.contains(&slot_of(&p.name)))
    };
    let bumper_slots = [Slot::FrontBumper, Slot::RearBumper, Slot::Skirt];
    let eff_kit = if cfg.body_kit != 0 && has(Ns::Kit(cfg.body_kit), &bumper_slots) { cfg.body_kit } else { 0 };
    let eff_hood = if cfg.hood_style != 0 && has(Ns::Style(cfg.hood_style), &[Slot::Hood]) { cfg.hood_style } else { 0 };
    let light_slots = [Slot::Headlight, Slot::Brakelight];
    let eff_light = if cfg.light_style != 0 && has(Ns::Style(cfg.light_style), &light_slots) { cfg.light_style } else { 0 };
    let eff_wide = if cfg.widebody != 0 && has(Ns::Wide(cfg.widebody), &[Slot::Body, Slot::Door]) { cfg.widebody } else { 0 };

    // Traffic cars (TAXI, BUS, SUV, …) and the shared prop bundles (WHEELS, SPOILER, …) carry no
    // customization token at all: every part is `NAME_BODY_A`. They have exactly one configuration,
    // so with nothing to choose between, everything is admitted — otherwise they'd render empty.
    let customizable = all.iter().any(|p| matches!(namespace(&p.name), Ns::Kit(_) | Ns::Base));

    // Whether a part is part of the configured car (before LOD dedup).
    let admit = |name: &str| -> bool {
        if !customizable {
            return true;
        }
        // The BASE greenhouse (glass/interior/trim) is always kept; window decals are the glass
        // panels; any other decal is texture-only livery, dropped until textured.
        if name.contains("_BASE") {
            return true;
        }
        if name.contains("DECAL") {
            return name.contains("WINDOW");
        }
        match namespace(name) {
            Ns::Kit(n) => match slot_of(name) {
                Slot::FrontBumper | Slot::RearBumper | Slot::Skirt => n == eff_kit,
                Slot::Hood => n == 0 && eff_hood == 0,
                Slot::Headlight | Slot::Brakelight => n == 0 && eff_light == 0,
                Slot::Body | Slot::Door => n == 0 && eff_wide == 0,
                Slot::Fixed => n == 0,
            },
            Ns::Style(n) => match slot_of(name) {
                Slot::Hood => eff_hood != 0 && n == eff_hood,
                Slot::Headlight | Slot::Brakelight => eff_light != 0 && n == eff_light,
                _ => false,
            },
            Ns::Wide(n) => match slot_of(name) {
                Slot::Body | Slot::Door => eff_wide != 0 && n == eff_wide,
                _ => false,
            },
            Ns::Base => true,
            Ns::Other => false,
        }
    };

    let fill_body_door = !has_paintable_door_skin(all);
    let mut best: HashMap<&str, &NfsMeshPart> = HashMap::new();
    for p in all {
        // Drop the TRUNK_AUDIO second shell (z-fights the TRUNK) and any hidden/decal Skip parts.
        if !admit(&p.name) || p.name.contains("TRUNK_AUDIO") || group_of(&p.name) == Grp::Skip {
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

/// The default (showroom) car — [`select_car`] with the stock [`CarConfig`]. Kept as a named
/// wrapper for the many call sites and tests that only want the stock configuration.
#[must_use]
pub fn select_stock_car(all: &[NfsMeshPart]) -> Vec<&NfsMeshPart> {
    select_car(all, &CarConfig::stock())
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

    /// A minimal named part with a triangle count — enough for selection, which only reads names
    /// and triangle counts (except for the body door-fill rule, which `body_lod` covers).
    fn part(name: &str, tris: usize) -> NfsMeshPart {
        let mut p = NfsMeshPart::default();
        p.name = name.to_string();
        p.indices = vec![0; tris * 3];
        p
    }

    /// The 240SX's real slot layout, trimmed to one LOD per component.
    fn customizable_car() -> Vec<NfsMeshPart> {
        vec![
            part("240SX_BASE_A", 496),
            part("240SX_KIT00_BODY_A", 728),
            part("240SX_KIT00_DOOR_LEFT_A", 68),
            part("240SX_KIT00_FRONT_BUMPER_A", 532),
            part("240SX_KIT00_REAR_BUMPER_A", 505),
            part("240SX_KIT00_SKIRT_A", 184),
            part("240SX_KIT00_HOOD_A", 148),
            part("240SX_KIT00_BRAKELIGHT_A", 148),
            part("240SX_KIT00_SPOILER_A", 116), // Fixed slot — never swapped
            part("240SX_KIT01_FRONT_BUMPER_A", 673),
            part("240SX_KIT01_REAR_BUMPER_A", 473),
            part("240SX_KIT01_SKIRT_A", 170),
            part("240SX_STYLE04_HOOD_A", 258),
            part("240SX_STYLE04_BRAKELIGHT_A", 208),
            part("240SX_KITW01_BODY_A", 2006),
            part("240SX_KITW01_DOOR_LEFT_A", 131),
        ]
    }

    fn names_of(sel: &[&NfsMeshPart]) -> Vec<String> {
        let mut v: Vec<String> = sel.iter().map(|p| p.name.clone()).collect();
        v.sort();
        v
    }

    #[test]
    fn stock_config_takes_only_base_and_kit00() {
        let all = customizable_car();
        let names = names_of(&select_stock_car(&all));
        assert!(names.iter().all(|n| n.contains("_BASE") || n.contains("_KIT00")), "{names:?}");
        // One part per slot: nothing from KIT01 / STYLE04 / KITW01 leaks in as a second shell.
        assert_eq!(names.len(), 9);
    }

    #[test]
    fn each_config_dimension_swaps_only_its_own_slots() {
        let all = customizable_car();
        let cfg = CarConfig { body_kit: 1, hood_style: 4, light_style: 4, widebody: 1 };
        let names = names_of(&select_car(&all, &cfg));
        assert_eq!(
            names,
            [
                "240SX_BASE_A",
                "240SX_KIT00_SPOILER_A", // Fixed slot stays stock
                "240SX_KIT01_FRONT_BUMPER_A",
                "240SX_KIT01_REAR_BUMPER_A",
                "240SX_KIT01_SKIRT_A",
                "240SX_KITW01_BODY_A",
                "240SX_KITW01_DOOR_LEFT_A",
                "240SX_STYLE04_BRAKELIGHT_A",
                "240SX_STYLE04_HOOD_A",
            ]
        );
    }

    #[test]
    fn a_variant_the_car_does_not_have_falls_back_to_stock() {
        // Kit numbering is sparse (the 240SX has no KIT19/27/28) and not every car has every
        // style — an absent pick must degrade to KIT00, not leave a hole.
        let all = customizable_car();
        let cfg = CarConfig { body_kit: 19, hood_style: 27, light_style: 9, widebody: 3 };
        let names = names_of(&select_car(&all, &cfg));
        assert!(names.iter().all(|n| n.contains("_BASE") || n.contains("_KIT00")), "{names:?}");
    }

    #[test]
    fn an_uncustomizable_model_keeps_every_part() {
        // Traffic cars and the shared prop bundles carry no KIT/STYLE token; with nothing to
        // choose between, dropping the un-namespaced parts would render them empty.
        let all = vec![part("TAXI_BODY_A", 900), part("TAXI_TIRE_FRONT_A", 120)];
        assert_eq!(names_of(&select_stock_car(&all)), ["TAXI_BODY_A", "TAXI_TIRE_FRONT_A"]);
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
