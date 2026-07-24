//! Routing a material run by its NFSU2 shader hash: which flat group it renders as, and the
//! one shader whose runs are dropped outright.

use crate::parts::Grp;

/// NFSU2 shares one shader set across every car, so a material's shader hash (`0x00134013`)
/// names its *type* — the reliable signal for how to render a run, independent of the
/// car-specific texture it happens to reference. Hashes are `h = 0xFFFF_FFFF; for b in
/// NAME.bytes() { h = h.wrapping_mul(33).wrapping_add(b) }` of the uppercase shader name.
pub mod hash {
    pub const CARSKIN: u32 = 0xd6d6_080a; // painted body panels
    pub const WINDSHIELD: u32 = 0x471a_1dca; // transparent glass (windscreen + windows)
    pub const WINDOWMASK: u32 = 0x3ed7_0c43; // opaque black window frame / frit
    pub const INTERIOR: u32 = 0x2787_edab; // seats / dash (behind the glass)
    pub const CHROME: u32 = 0x5494_9afd; // bright metal trim + mirrors
    pub const DULLPLASTIC: u32 = 0x0fed_ee40; // matte black plastic
    pub const MOLDINGS: u32 = 0x12c9_453c; // dark rubber/plastic mouldings
    pub const PLAINNOTHING: u32 = 0x010c_b64a; // unshaded filler
    pub const BOTTOM: u32 = 0x52bf_4c34; // underbody / rocker
    pub const GRILL: u32 = 0x02dd_dad9; // dark front grille
}

/// Map a shader hash to the flat material group it should render as, for the runs whose look
/// is decided by shader alone (glass, chrome, and the dark interior/trim family that would
/// otherwise paint bright). Returns `None` for shaders that instead carry a texture (head/
/// brake lights, tyres, badging) or that we don't recognise — those fall back to texture/name.
pub(crate) fn shader_group(shader: u32) -> Option<Grp> {
    match shader {
        hash::CARSKIN => Some(Grp::Paint),
        hash::WINDSHIELD => Some(Grp::Glass),
        hash::CHROME => Some(Grp::Chrome),
        hash::INTERIOR
        | hash::WINDOWMASK
        | hash::DULLPLASTIC
        | hash::MOLDINGS
        | hash::PLAINNOTHING
        | hash::BOTTOM
        | hash::GRILL => Some(Grp::Trim),
        _ => None,
    }
}

/// Whether a material run is [`hash::PLAINNOTHING`] filler that should be dropped rather than
/// rendered. That shader is "unshaded filler": on **BASE** it is the dark interior tub (kept, so
/// the cabin reads through the glass), but on a **kit body** it is wheel-well filler the real game
/// hides behind the wheel — drawn as an opaque flat panel it juts out past the arch as a black
/// square (was visible on RX7/RX8/GTO/GOLF/SKYLINE). Drop it only there, keyed off whether the
/// owning part is the BASE shell.
pub(crate) fn is_dropped_filler(shader: u32, part_is_base: bool) -> bool {
    shader == hash::PLAINNOTHING && !part_is_base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plainnothing_filler_dropped_only_off_base() {
        // The interior tub on BASE is kept (read through the glass)...
        assert!(!is_dropped_filler(hash::PLAINNOTHING, true));
        // ...but the same "unshaded filler" shader on a kit body is the wheel-well filler that
        // otherwise juts out as an opaque black square, so it is dropped.
        assert!(is_dropped_filler(hash::PLAINNOTHING, false));
    }

    #[test]
    fn other_shaders_are_never_treated_as_filler() {
        // Only PLAINNOTHING is filler; real materials are never dropped, on BASE or a kit body.
        for sh in [hash::CARSKIN, hash::WINDSHIELD, hash::CHROME, hash::INTERIOR, hash::BOTTOM] {
            assert!(!is_dropped_filler(sh, true));
            assert!(!is_dropped_filler(sh, false));
        }
    }

    #[test]
    fn every_routed_shader_lands_in_a_group() {
        // Paint / glass / chrome route to their own group; the dark family shares Trim.
        assert_eq!(shader_group(hash::CARSKIN), Some(Grp::Paint));
        assert_eq!(shader_group(hash::WINDSHIELD), Some(Grp::Glass));
        assert_eq!(shader_group(hash::CHROME), Some(Grp::Chrome));
        assert_eq!(shader_group(hash::INTERIOR), Some(Grp::Trim));
        // An unrecognised shader carries its own texture instead — not a group.
        assert_eq!(shader_group(0xdead_beef), None);
    }
}
