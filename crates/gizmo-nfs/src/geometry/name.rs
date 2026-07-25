//! The `0x00134011` solid header: the part's name, its local matrix, and what the name says
//! about the part (its role and LOD level).

use super::format::MATRIX_OFFSET;
use crate::reader::ByteReader;
use crate::types::{LodLevel, Mat4, PartRole};

/// Read the 4x4 local matrix from a solid header (row-major, 16 f32 at [`MATRIX_OFFSET`]).
/// Falls back to identity if the header is too short.
pub fn read_matrix(header: &[u8]) -> Mat4 {
    let mut m = crate::types::IDENTITY;
    if let Ok(mut r) = ByteReader::at(header, MATRIX_OFFSET) {
        for row in &mut m {
            for cell in row.iter_mut() {
                match r.f32_le() {
                    Ok(v) => *cell = v,
                    Err(_) => return crate::types::IDENTITY,
                }
            }
        }
    }
    m
}

/// Extract the part name: the longest printable run in the header (names are ASCII with
/// `_`, e.g. `240SX_BASE_A`, `240SX_KIT01_HOOD_A`).
pub fn part_name(header: &[u8]) -> String {
    let mut best = "";
    let mut start = 0usize;
    let mut i = 0usize;
    while i <= header.len() {
        let printable = header.get(i).is_some_and(|&b| b.is_ascii_graphic());
        if !printable {
            if i - start > best.len() {
                best = std::str::from_utf8(&header[start..i]).unwrap_or("");
            }
            start = i + 1;
        }
        i += 1;
    }
    best.to_owned()
}

/// Classify what a part *is* from its name: the shared body shell, a kit slot, a wheel, or
/// something this crate does not name.
pub(super) fn classify_role(name: &str) -> PartRole {
    if name.contains("_BASE") {
        PartRole::Body
    } else if let Some(pos) = name.find("_KIT") {
        let digits: String = name[pos + 4..].chars().take_while(|c| c.is_ascii_digit()).collect();
        PartRole::Kit { slot: digits.parse().unwrap_or(0) }
    } else if name.contains("WHEE") || name.contains("TIRE") || name.contains("RIM") {
        PartRole::Wheel
    } else {
        PartRole::Unknown
    }
}

/// Classify a part's level of detail from the trailing `_A`..`_D` suffix of its name
/// (`A` = highest detail). Anything else — including names whose LOD letter was cut off by
/// the fixed-length name field — is [`LodLevel::Unknown`].
///
/// Empirically locked against 54 stock cars (~15k parts): a genuine LOD tag is always a
/// single `A`..`D` immediately after `_`, and truncation never forges a spurious `_A`..`_D`
/// tail (the only truncated single-letter tails observed are `L`/`R`/`Q`/`W`/`M`/`U`), so
/// this predicate has no false positives. Parts whose LOD letter was truncated away
/// (`..._HEADLIGHT_LEFT_`, `..._HEADLIGHT_L`, `..._SIDE_MIRRO`) are deliberately left
/// `Unknown` rather than reconstructed by triangle count: names that collide after
/// truncation are *not* clean per-component LOD ladders — they merge left/right panels and
/// `STYLE##` variants and carry duplicate triangle counts, so a rank-based guess would be
/// wrong. Downstream code that needs the best LOD picks it per component by triangle count
/// instead (see the game's `select_car`), which is why this field can stay honest.
///
/// The suffix test mirrors the game's `component_key`, which strips the very same `_[A-D]`.
pub(super) fn classify_lod(name: &str) -> LodLevel {
    // A LOD tag is a single A..D at the end, immediately preceded by '_'. Requiring the
    // delimiter (rather than just the last '_'-segment) means a bare letter or an
    // underscore-less name can never be misread as a LOD.
    let b = name.as_bytes();
    if b.len() >= 2 && b[b.len() - 2] == b'_' {
        return match b[b.len() - 1] {
            b'A' => LodLevel::A,
            b'B' => LodLevel::B,
            b'C' => LodLevel::C,
            b'D' => LodLevel::D,
            _ => LodLevel::Unknown,
        };
    }
    LodLevel::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_names() {
        assert_eq!(classify_role("240SX_BASE_A"), PartRole::Body);
        assert_eq!(classify_lod("240SX_BASE_A"), LodLevel::A);
        assert_eq!(classify_role("FOCUS_KIT03_HOOD_B"), PartRole::Kit { slot: 3 });
        assert_eq!(classify_lod("FOCUS_KIT03_HOOD_B"), LodLevel::B);
        assert_eq!(classify_role("FRONT_WHEEL"), PartRole::Wheel);
        // The name field clips the tail: on a long car name a wheel loses its `L` too, and the
        // `WHEE` stem still names it. (A *kit* wheel keeps its kit role — the kit test is first,
        // so `role` says which slot supplies the part; the game groups wheels by name instead.)
        assert_eq!(classify_role("FRONT_WHEE"), PartRole::Wheel);
        assert_eq!(classify_role("IMPREZAWRX_KIT00_FRONT_WHEE"), PartRole::Kit { slot: 0 });
    }

    /// Locks the empirical LOD-naming survey (54 stock cars, ~15k parts): genuine LOD tags
    /// are exactly a trailing `_[A-D]`; every truncation pattern seen in the wild must fall
    /// through to `Unknown` rather than be misread as (or fabricated into) a LOD.
    #[test]
    fn classify_lod_locks_empirical_naming() {
        // Genuine LOD suffixes at every level.
        assert_eq!(classify_lod("240SX_BASE_A"), LodLevel::A);
        assert_eq!(classify_lod("240SX_KIT00_BODY_B"), LodLevel::B);
        assert_eq!(classify_lod("240SX_KIT00_REAR_BUMPER_C"), LodLevel::C);
        assert_eq!(classify_lod("240SX_KIT00_BODY_D"), LodLevel::D);
        // Truncated away entirely (trailing '_'): two LODs collapse to one name → Unknown.
        assert_eq!(classify_lod("240SX_KIT00_HEADLIGHT_LEFT_"), LodLevel::Unknown);
        // Left/right letter survived but the LOD letter is gone.
        assert_eq!(classify_lod("MUSTANGGT_KIT00_HEADLIGHT_L"), LodLevel::Unknown);
        assert_eq!(classify_lod("MUSTANGGT_KIT00_HEADLIGHT_R"), LodLevel::Unknown);
        // Mid-word truncations observed in the survey.
        assert_eq!(classify_lod("240SX_KIT00_LEFT_SIDE_MIRRO"), LodLevel::Unknown);
        assert_eq!(classify_lod("240SX_KIT00_TRUNK_AUDIO_UNL"), LodLevel::Unknown);
        // Non-LOD single-letter tails that DO occur (`R` from `..._QUARTER_R`) must not read
        // as a LOD — only A..D are LOD letters.
        assert_eq!(classify_lod("240SX_DECAL_RIGHT_QUARTER_R"), LodLevel::Unknown);
        // Robustness: a bare letter or an underscore-less/empty name is never a LOD.
        assert_eq!(classify_lod("A"), LodLevel::Unknown);
        assert_eq!(classify_lod("BODY"), LodLevel::Unknown);
        assert_eq!(classify_lod(""), LodLevel::Unknown);
    }

    #[test]
    fn part_name_is_the_longest_printable_run() {
        let mut header = vec![0u8; 8];
        header.extend_from_slice(b"240SX_KIT00_HOOD_A");
        header.extend_from_slice(&[0, 0, b'x', b'y', 0]);
        assert_eq!(part_name(&header), "240SX_KIT00_HOOD_A");
    }

    #[test]
    fn a_short_header_falls_back_to_identity() {
        assert_eq!(read_matrix(&[0u8; 8]), crate::types::IDENTITY);
    }
}
