//! Finding a car's files from whatever the user pointed at, and reading them.
//!
//! Every command takes "a car": either the directory (`CARS/240SX`) or its `GEOMETRY.BIN`.
//! From either, the sibling `TEXTURES.BIN` and the game's `GLOBAL/GLOBALB.BUN` are derivable,
//! so the user never has to spell out three paths.

use std::path::{Path, PathBuf};

/// A human-readable failure. The CLI reports these and exits non-zero; it never panics on a
/// path that does not exist or a file it cannot parse.
pub type Error = String;
pub type Result<T> = std::result::Result<T, Error>;

/// The files a car is made of, resolved from one user-supplied path.
pub struct Car {
    /// The car's directory (`CARS/240SX`).
    pub dir: PathBuf,
    /// Its `GEOMETRY.BIN`.
    pub geometry: PathBuf,
    /// The car's folder name (`240SX`) — the key `GLOBALB.BUN` lists it under.
    pub name: String,
}

impl Car {
    /// Resolve from either the car directory or the `GEOMETRY.BIN` inside it.
    pub fn resolve(path: &Path) -> Result<Self> {
        let (dir, geometry) = if path.is_dir() {
            (path.to_path_buf(), path.join("GEOMETRY.BIN"))
        } else {
            let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            (dir, path.to_path_buf())
        };
        if !geometry.is_file() {
            return Err(format!("no GEOMETRY.BIN at {}", geometry.display()));
        }
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "CAR".to_string());
        Ok(Self { dir, geometry, name })
    }

    /// Sibling `GEOMETRY_<NAME>.BIN` files, if this directory holds a *set* of models rather
    /// than one car. `CARS/WHEELS` is the case that matters: its `GEOMETRY.BIN` is a 64-byte
    /// stub and the actual rims are one file per brand, so an empty result there is data, not
    /// a failure — say which files to point at instead.
    pub fn siblings(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                (n.starts_with("GEOMETRY_") && n.ends_with(".BIN")).then_some(n)
            })
            .collect();
        names.sort();
        names
    }

    /// The car's parsed parts.
    pub fn parts(&self) -> Result<Vec<gizmo_nfs::NfsMeshPart>> {
        let bytes = read(&self.geometry)?;
        gizmo_nfs::parse_geometry(&bytes).map_err(|e| format!("{}: {e}", self.geometry.display()))
    }

    /// The car's decoded textures, if it ships a `TEXTURES.BIN` that parses.
    pub fn textures(&self) -> Option<gizmo_nfs::Tpk> {
        let bytes = read(&self.dir.join("TEXTURES.BIN")).ok()?;
        gizmo_nfs::Tpk::parse(&bytes).ok()
    }

    /// This car's `CarTypeInfo`, if the game's global bundle is reachable from here.
    pub fn car_type_info(&self) -> Option<gizmo_nfs::CarTypeInfo> {
        let bytes = read(&globalb_beside(&self.dir)?).ok()?;
        gizmo_nfs::globalb::find_car(&bytes, &self.name)
    }
}

/// `GLOBAL/GLOBALB.BUN` relative to a car directory (`<root>/CARS/<car>`), if it exists.
pub fn globalb_beside(car_dir: &Path) -> Option<PathBuf> {
    let root = car_dir.parent()?.parent()?;
    let p = root.join("GLOBAL").join("GLOBALB.BUN");
    p.is_file().then_some(p)
}

/// Read a file and decompress it if it carries a recognised codec.
pub fn read(path: &Path) -> Result<Vec<u8>> {
    let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match gizmo_nfs::compression::detect(&raw) {
        gizmo_nfs::compression::Codec::None => Ok(raw),
        _ => gizmo_nfs::compression::decompress(&raw)
            .map_err(|e| format!("{}: {e}", path.display())),
    }
}
