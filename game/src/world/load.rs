//! Getting a city off disk: which files, and what they hand over.
//!
//! Three binaries now want the same four things — the region bundles, their objects, the texture
//! packs that dress them, and the shared tiers the packs do not carry — and the order matters in a
//! way that is easy to get subtly wrong (the shared tiers are found by walking *up* from the
//! `TRACKS` directory, not from the current directory). It is written once here.

use super::SharedTextures;
use gizmo_nfs::world::{TrackPack, WorldMesh};
use std::path::{Path, PathBuf};

/// A loaded city: every region's objects and packs, plus the tiers shared between them.
pub struct Bundles {
    /// The files this came from, for a caller that wants to say how many.
    pub files: Vec<PathBuf>,
    /// Every region's objects, concatenated. Not filtered — [`super::is_drawn`] is the caller's
    /// call, because `nfs_city` counts what it is about to drop and the others do not.
    pub meshes: Vec<WorldMesh>,
    /// Every region's texture packs.
    pub packs: Vec<TrackPack<'static>>,
    /// `TRACKS/LOC4DYNTEX.BIN` and `GLOBAL/` — the keys no region carries itself. Empty rather than
    /// fatal when the install root cannot be found: that means grey walls, not a failure to start.
    pub shared: SharedTextures,
}

/// Every `STREAM*.BUN` at `path`, or `path` itself when it names one file.
///
/// Sorted, so two runs over the same directory load the same regions in the same order — a golden
/// screenshot needs that, and so does anyone comparing two runs' counts.
#[must_use]
pub fn bundles(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let Ok(dir) = std::fs::read_dir(path) else { return Vec::new() };
    let mut out: Vec<_> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|q| {
            q.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("STREAM") && n.ends_with(".BUN"))
        })
        .collect();
    out.sort();
    out
}

/// Read the whole city at `path` (a `TRACKS` directory, or one `STREAM*.BUN`).
///
/// **The bundles are not adjacent tiles.** They share one world coordinate system and overlap,
/// being per-race-route supersets of the same city, so a district's buildings and the roads under
/// them can live in different files and loading one alone leaves buildings hanging in the air.
/// Given a directory, this takes all of them.
///
/// The file bytes are **leaked on purpose**: a [`TrackPack`] borrows its pixel pool from them, and
/// this is the whole city staying loaded for the life of the process. Freeing them would mean
/// threading a lifetime through every caller to save memory at exit.
#[must_use]
pub fn load(path: &str) -> Bundles {
    let files = bundles(Path::new(path));
    assert!(!files.is_empty(), "{path}: no STREAM*.BUN here");
    let loaded: &'static [Vec<u8>] = Box::leak(
        files
            .iter()
            .map(|f| std::fs::read(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display())))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let mut meshes = Vec::new();
    let mut packs = Vec::new();
    for bytes in loaded {
        meshes.extend(gizmo_nfs::world::meshes(bytes).expect("read a region's meshes"));
        packs.extend(gizmo_nfs::world::packs(bytes).expect("read a region's texture packs"));
    }

    // The shared tiers live beside the install's `GLOBAL/`, which is above `TRACKS`. Walking up
    // from the path given is what makes this work whatever directory the binary was started in.
    let root = Path::new(path).ancestors().find(|a| a.join("GLOBAL").is_dir());
    let shared = root.map(SharedTextures::load).unwrap_or_default();

    Bundles { files, meshes, packs, shared }
}

/// Where in the city to start, from `NFS_AT="x,y,z"`, else `default`.
///
/// Bayview is 12 km across and almost all of it is not where you want to be, so every binary that
/// opens the city takes this. Anything unparseable falls back rather than failing: a typo in a
/// coordinate should put you at the default, not refuse to start.
#[must_use]
pub fn start_at(default: gizmo::prelude::Vec3) -> gizmo::prelude::Vec3 {
    std::env::var("NFS_AT")
        .ok()
        .and_then(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (v.len() == 3).then(|| gizmo::prelude::Vec3::new(v[0], v[1], v[2]))
        })
        .unwrap_or(default)
}

/// The `TRACKS` directory to read: an explicit argument, else `$NFSU2_ROOT/TRACKS`.
///
/// Its own function because the alternative is each binary inventing a different half of it, and
/// the install path is deliberately not written down anywhere in this repo.
#[must_use]
pub fn tracks_path(arg: Option<String>) -> String {
    arg.unwrap_or_else(|| {
        std::env::var("NFSU2_ROOT")
            .map(|r| format!("{r}/TRACKS"))
            .expect("no TRACKS directory given and NFSU2_ROOT is unset")
    })
}
