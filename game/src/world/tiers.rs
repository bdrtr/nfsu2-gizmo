//! Where a texture key is looked for, and in what order.
//!
//! 98.17 % of the city's 70,439 texture slots resolve in their own bundle's packs. The other 1,289
//! do not, and they are not stray: they are the skydome, the start line, the neon strips, the
//! headlight glow — the things every region shares, kept once. Without them a downtown junction
//! renders with 589 runs in the "unresolved" grey.
//!
//! **The order is policy and lives here**, not in the parser. So does the fact that the tiers are
//! not even one format:
//!
//! | tier | shape | reader |
//! |---|---|---|
//! | the region's own `STREAM*.BUN` | track variant, `0x33310004` | [`gizmo_nfs::world::packs`] |
//! | `TRACKS/LOC4DYNTEX.BIN` | **car** variant, `0x33310003` | [`gizmo_nfs::texture::Tpk`] |
//! | `GLOBAL/*.BUN`, `GLOBAL/GlobalTextures*.bin` | track variant | [`gizmo_nfs::world::packs`] |
//!
//! `GLOBAL/DYNTEX.BIN` and `GLOBAL/HUD_CustomTextures_00..10.bin` are the car variant again, so a
//! resolver cannot assume a directory implies a shape — it has to try both readers per file. That
//! is measured, not defensive: the two shapes genuinely interleave in one folder.
//!
//! 111 references resolve in none of them, in the whole install. Those stay unresolved and are
//! meant to: a fallback that quietly substitutes some other texture makes a missing tier look like
//! a working one.

use gizmo_nfs::types::{AssetHash, NfsTexture};
use std::collections::HashMap;
use std::path::Path;

/// Every shared texture, decoded once, by key.
///
/// Decoded eagerly rather than on demand because the shared tiers are small — a few hundred images
/// against the city's 5,087 — and because the alternative is holding another set of borrowed pixel
/// pools alive for the life of the window.
#[derive(Default)]
pub struct SharedTextures {
    by_key: HashMap<AssetHash, NfsTexture>,
    /// Files that yielded nothing, so a caller can tell "no textures here" from "never looked".
    pub empty_files: Vec<String>,
}

impl SharedTextures {
    /// Load `TRACKS/LOC4DYNTEX.BIN` and the `GLOBAL/` packs beneath a game root.
    ///
    /// A file that does not exist, does not parse, or holds no textures is skipped rather than
    /// fatal: this is a fallback tier, and a game that will not start because one shared pack is
    /// missing is worse than one with a few grey walls.
    #[must_use]
    pub fn load(root: &Path) -> Self {
        let mut out = Self::default();
        let mut paths: Vec<std::path::PathBuf> = vec![root.join("TRACKS/LOC4DYNTEX.BIN")];
        if let Ok(dir) = std::fs::read_dir(root.join("GLOBAL")) {
            let mut global: Vec<_> = dir
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bun") || e.eq_ignore_ascii_case("bin"))
                })
                .collect();
            global.sort();
            paths.extend(global);
        }

        for path in paths {
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let before = out.by_key.len();
            out.take_track_variant(&bytes);
            out.take_car_variant(&bytes);
            if out.by_key.len() == before {
                out.empty_files.push(path.display().to_string());
            }
        }
        out
    }

    /// The track variant: `0x33310004` records, raw pixels, format stated in the record.
    fn take_track_variant(&mut self, bytes: &[u8]) {
        let Ok(packs) = gizmo_nfs::world::packs(bytes) else { return };
        for pack in &packs {
            for tex in &pack.textures {
                if self.by_key.contains_key(&tex.key) {
                    continue;
                }
                if let Ok(image) = pack.decode(tex) {
                    self.by_key.insert(tex.key, image);
                }
            }
        }
    }

    /// The car variant: a `0x33310003` descriptor table over compressed, self-describing blobs.
    fn take_car_variant(&mut self, bytes: &[u8]) {
        let Ok(entries) = gizmo_nfs::texture::Tpk::directory(bytes) else { return };
        for entry in &entries {
            if self.by_key.contains_key(&entry.hash) {
                continue;
            }
            if let Ok(image) = gizmo_nfs::texture::Tpk::decode_one(bytes, entry) {
                self.by_key.insert(entry.hash, image);
            }
        }
    }

    /// The image behind a key, if any tier has it.
    #[must_use]
    pub fn get(&self, key: AssetHash) -> Option<&NfsTexture> {
        self.by_key.get(&key)
    }

    /// How many shared textures were found.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }
}
