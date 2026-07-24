//! Reading a part's name: the LOD suffix, the customization namespace token, and the slot the
//! part fills. All of it has to survive NFSU2's fixed-length name field, which clips the tail.

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

/// The customization namespace a part name belongs to, parsed from its `KIT##` / `KITW##` /
/// `STYLE##` token (or `BASE`). Survives NFSU2's 27-char name truncation, which only clips the
/// trailing LOD/side suffix, never the leading namespace token.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Ns {
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
pub(super) fn num_after(name: &str, tag: &str) -> Option<u8> {
    let i = name.find(tag)? + tag.len();
    let digits: String = name[i..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

pub(super) fn namespace(name: &str) -> Ns {
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
pub(super) enum Slot {
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

pub(super) fn slot_of(name: &str) -> Slot {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_key_collapses_lod_suffix_only() {
        assert_eq!(component_key("240SX_KIT00_BODY_A"), "240SX_KIT00_BODY");
        assert_eq!(component_key("240SX_KIT00_BODY_D"), "240SX_KIT00_BODY");
        // No trailing single A..D letter → unchanged (truncated names, or non-LOD names).
        assert_eq!(component_key("240SX_KIT00_HEADLIGHT_LEFT_"), "240SX_KIT00_HEADLIGHT_LEFT_");
        assert_eq!(component_key("240SX_KIT00_TRUNK_AUDIO_UNL"), "240SX_KIT00_TRUNK_AUDIO_UNL");
    }

    #[test]
    fn namespace_reads_the_customization_token() {
        // `KITW##` must win over `KIT##`, whose letters it contains.
        assert_eq!(namespace("240SX_KITW03_BODY_A"), Ns::Wide(3));
        assert_eq!(namespace("240SX_KIT00_BODY_A"), Ns::Kit(0));
        assert_eq!(namespace("240SX_KIT12_SKIRT_A"), Ns::Kit(12));
        assert_eq!(namespace("240SX_STYLE07_HOOD_B"), Ns::Style(7));
        assert_eq!(namespace("240SX_BASE_A"), Ns::Base);
        assert_eq!(namespace("240SX_DECAL_LEFT_QUARTER_RE"), Ns::Other);
    }

    #[test]
    fn slot_reads_the_component() {
        assert_eq!(slot_of("240SX_KIT01_FRONT_BUMPER_A"), Slot::FrontBumper);
        assert_eq!(slot_of("240SX_KIT01_REAR_BUMPER_A"), Slot::RearBumper);
        assert_eq!(slot_of("240SX_KIT01_SKIRT_A"), Slot::Skirt);
        assert_eq!(slot_of("240SX_STYLE04_HOOD_A"), Slot::Hood);
        assert_eq!(slot_of("240SX_STYLE04_HEADLIGHT_LEF"), Slot::Headlight);
        assert_eq!(slot_of("240SX_STYLE04_BRAKELIGHT_A"), Slot::Brakelight);
        assert_eq!(slot_of("240SX_KITW01_BODY_A"), Slot::Body);
        assert_eq!(slot_of("240SX_KITW01_DOOR_LEFT_A"), Slot::Door);
        assert_eq!(slot_of("240SX_KIT00_SPOILER_A"), Slot::Fixed);
    }
}
