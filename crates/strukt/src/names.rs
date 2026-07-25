//! The hash dictionary: names the user gives to the numbers a file references its assets by.
//!
//! NFSU2 ships almost no strings, so most of what a file points at is a bare 32-bit hash. A TPK
//! leaves a `DebugName` beside a texture's hash — truncated to 23 characters — and nothing at all
//! is left beside a shader or a solid. This is where the names people work out get kept.
//!
//! **Verified vs. remembered.** `gizmo_nfs::hash` can hash a name back, so a name that reproduces
//! the file's number is *proof*, and one that does not is somebody's note. The dictionary keeps
//! both and never conflates them: the screen marks which is which, because a tool that presents a
//! guess as a fact is worse than one that admits it does not know. This is also how a truncated
//! name's tail comes back — type candidates until one hashes.
//!
//! The file is a tab-separated list under the user's config directory, one `hash<TAB>name` per
//! line. Plain text on purpose: it is a list of names, people will want to `grep` it, diff it, and
//! paste one another's.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// The name table, loaded from disk and written back when it changes.
#[derive(Default)]
pub struct Names {
    entries: BTreeMap<u32, String>,
    /// Set when a change has not been written yet.
    dirty: bool,
}

impl Names {
    /// Load the dictionary, or start an empty one if there is no file yet (which is not an error —
    /// everybody starts with no names).
    #[must_use]
    pub fn load() -> Self {
        Self::path().map(|p| Self::load_from(&p)).unwrap_or_default()
    }

    /// The same from a given file — what the tests read, and what an import would use.
    #[must_use]
    pub fn load_from(path: &std::path::Path) -> Self {
        let mut names = Self::default();
        let Ok(text) = std::fs::read_to_string(path) else { return names };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((hash, name)) = line.split_once('\t') else { continue };
            let hash = hash.trim().trim_start_matches("0x");
            if let Ok(hash) = u32::from_str_radix(hash, 16) {
                names.entries.insert(hash, name.trim().to_string());
            }
        }
        names
    }

    /// Where the file lives: `$XDG_CONFIG_HOME/strukt/names.tsv`, else `~/.config/…`.
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("strukt").join("names.tsv"))
    }

    /// The name for a hash, if the dictionary has one.
    #[must_use]
    pub fn get(&self, hash: u32) -> Option<&str> {
        self.entries.get(&hash).map(String::as_str)
    }

    /// Name a hash. An empty name removes the entry rather than storing a blank.
    pub fn set(&mut self, hash: u32, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.entries.remove(&hash);
        } else {
            self.entries.insert(hash, name.to_string());
        }
        self.dirty = true;
    }

    /// How many names the dictionary holds, across every file — not just the open one.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Write the file if anything changed. Returns the path written, or the failure.
    ///
    /// Called after an edit rather than at exit: a tool that loses an hour of naming because it
    /// was closed the wrong way is a tool nobody names anything in.
    pub fn save_if_dirty(&mut self) -> Option<Result<PathBuf, String>> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let path = Self::path()?;
        Some(self.write(&path).map(|()| path.clone()).map_err(|e| format!("{}: {e}", path.display())))
    }

    fn write(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text = String::from(
            "# STRUKT hash dictionary — one <hash>\\t<name> per line.\n\
             # Verified names hash back to their number (gizmo_nfs::hash::string_hash).\n",
        );
        for (hash, name) in &self.entries {
            text.push_str(&format!("{hash:08X}\t{name}\n"));
        }
        std::fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_an_empty_name_forgets_the_hash() {
        let mut names = Names::default();
        names.set(0x1234, "BODY");
        assert_eq!(names.get(0x1234), Some("BODY"));
        names.set(0x1234, "   ");
        assert_eq!(names.get(0x1234), None);
        assert_eq!(names.len(), 0);
    }

    #[test]
    fn a_saved_dictionary_reads_back_the_same() {
        let dir = std::env::temp_dir().join(format!("strukt-names-{}", std::process::id()));
        let path = dir.join("names.tsv");
        let mut names = Names::default();
        names.set(0xB3DC_27AB, "240SX_BADGING");
        names.set(0x0000_0001, "a name with spaces");
        names.write(&path).expect("write");

        let read = Names::load_from(&path);
        assert_eq!(read.get(0xB3DC_27AB), Some("240SX_BADGING"));
        assert_eq!(read.get(0x0000_0001), Some("a name with spaces"));
        assert_eq!(read.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The file is meant to be edited by hand and pasted between people, so comments, blank lines,
    /// a `0x` prefix and rubbish all have to be survivable.
    #[test]
    fn a_hand_edited_dictionary_is_read_forgivingly() {
        let dir = std::env::temp_dir().join(format!("strukt-names-hand-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("names.tsv");
        std::fs::write(
            &path,
            "# a note\n\n0xB3DC27AB\t240SX_BADGING\nnot a line at all\n4B6F3195\t240SX_ENGINE\n",
        )
        .expect("write");

        let read = Names::load_from(&path);
        assert_eq!(read.len(), 2, "the rubbish line must be skipped, not fatal");
        assert_eq!(read.get(0x4B6F_3195), Some("240SX_ENGINE"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
