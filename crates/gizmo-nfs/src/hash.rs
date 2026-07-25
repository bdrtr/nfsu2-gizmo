//! The hash NFSU2 names its assets by.
//!
//! Textures, materials, shaders and solids are referenced by a 32-bit hash, not by name: the game
//! ships almost no strings. The function is Black Box's `bStringHash` — `h = h * 33 + byte` over
//! the bytes of the name, starting from `0xFFFFFFFF`.
//!
//! # How this was locked
//!
//! Not from a spec — there is none. A TPK leaves a `DebugName` beside each texture's hash, which
//! gives thousands of (name, hash) pairs from a real install. Over one install's 2,123 pairs, this
//! function reproduces the hash for **every** name that fits in the name field, and fails only for
//! names of exactly 23 characters — the ones the field truncated, whose hash was computed from the
//! full name that no longer survives. That is what makes the result decisive rather than plausible:
//! the failures are exactly the cases where the input is known to be incomplete.
//!
//! # What it is for
//!
//! Two things. It turns a name into the key the file uses ([`asset_hash`]), and — because the hash
//! is of the *whole* name — it can **confirm a guess**: a truncated `240SX_CENTRE_BRAKELIGHT`
//! whose real name was longer can be recovered by hashing candidates until one matches, which is
//! the only way those tails come back.

use crate::types::AssetHash;

/// `bStringHash`: `h = h * 33 + byte`, from `0xFFFFFFFF`, over the name's bytes.
///
/// Case matters — the names in the files are upper case, and hashing a lower-case spelling of one
/// gives a different value. Nothing here normalises on the caller's behalf.
#[must_use]
pub fn string_hash(name: &str) -> u32 {
    bytes_hash(name.as_bytes())
}

/// The same, as the [`AssetHash`] every other module speaks in.
#[must_use]
pub fn asset_hash(name: &str) -> AssetHash {
    AssetHash(string_hash(name))
}

/// The same over raw bytes, for a name read out of a file that may not be UTF-8.
#[must_use]
pub fn bytes_hash(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0xFFFF_FFFF;
    for b in bytes {
        h = h.wrapping_mul(33).wrapping_add(u32::from(*b));
    }
    h
}

/// Whether `name` is the name behind `hash`.
///
/// The point of asking it this way round: a name found in a file is often truncated, so "does this
/// candidate hash to what the file says" is the question that can actually be answered.
#[must_use]
pub fn names(name: &str, hash: AssetHash) -> bool {
    string_hash(name) == hash.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vectors taken from a real install's TPK (a name and a number — no asset data). If the
    /// constants ever drift, these are what says so.
    #[test]
    fn known_names_hash_to_the_values_the_files_use() {
        for (name, hash) in [
            ("240SX_BADGING", 0xB3DC_27ABu32),
            ("240SX_DOORLINE", 0x5805_76DB),
            ("240SX_DOORLINE_KIT", 0x3980_CD22),
            ("240SX_DOORLINE_MASK", 0x699B_6846),
            ("240SX_ENGINE", 0x4B6F_3195),
        ] {
            assert_eq!(string_hash(name), hash, "{name}");
            assert!(names(name, AssetHash(hash)));
        }
    }

    #[test]
    fn the_empty_name_is_the_seed_and_case_is_not_normalised() {
        assert_eq!(string_hash(""), 0xFFFF_FFFF);
        assert_ne!(string_hash("240sx_badging"), string_hash("240SX_BADGING"));
    }

    /// A truncated name must *not* hash to the file's value — that is the property the dictionary
    /// relies on to tell a recovered name from a guess.
    #[test]
    fn a_truncated_name_does_not_hash_to_the_full_ones_value() {
        let full = "240SX_CENTRE_BRAKELIGHT_LEFT";
        let truncated = &full[..23];
        assert_ne!(string_hash(truncated), string_hash(full));
    }

    #[test]
    fn hashing_bytes_and_hashing_a_str_agree() {
        assert_eq!(bytes_hash(b"240SX_ENGINE"), string_hash("240SX_ENGINE"));
    }
}
