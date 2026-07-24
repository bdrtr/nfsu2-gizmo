//! The I/O edge: reading a car's sibling asset files and its environment overrides.
//!
//! Everything else in this crate is a pure function of bytes already in memory; the filesystem
//! and the environment are touched only here (and in the binaries' argument handling).

use gizmo_nfs::{CarTypeInfo, Tpk};

/// Load and decode the `TEXTURES.BIN` sitting next to a car's `GEOMETRY.BIN`, if present.
/// Returns `None` (untextured car) when the file is absent or unparseable.
#[must_use]
pub fn load_tpk_beside(geometry_path: &str) -> Option<Tpk> {
    let dir = std::path::Path::new(geometry_path).parent()?;
    let bytes = std::fs::read(dir.join("TEXTURES.BIN")).ok()?;
    Tpk::parse(&bytes).ok()
}

/// Load this car's [`CarTypeInfo`] (exact wheel mounts, radius, mass) from the game's global
/// bundle, resolved relative to a `CARS/<name>/GEOMETRY.BIN` path: up two directories to the
/// game root, then `GLOBAL/GLOBALB.BUN`, looked up by the car's folder name. `None` if the
/// bundle is missing or the car isn't listed.
#[must_use]
pub fn load_cartypeinfo_beside(geometry_path: &str) -> Option<CarTypeInfo> {
    let geo = std::path::Path::new(geometry_path);
    let car_dir = geo.parent()?; // CARS/<name>
    let name = car_dir.file_name()?.to_str()?;
    let root = car_dir.parent()?.parent()?; // up past CARS/
    let raw = std::fs::read(root.join("GLOBAL").join("GLOBALB.BUN")).ok()?;
    let bytes = match gizmo_nfs::compression::detect(&raw) {
        gizmo_nfs::compression::Codec::None => raw,
        _ => gizmo_nfs::compression::decompress(&raw).ok()?,
    };
    gizmo_nfs::globalb::find_car(&bytes, name)
}

/// Parse a `"r,g,b"` (each `0..1`) colour from an environment variable, else the default.
#[must_use]
pub fn env_color(var: &str, default: [f32; 3]) -> [f32; 3] {
    std::env::var(var)
        .ok()
        .and_then(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (v.len() == 3).then(|| [v[0], v[1], v[2]])
        })
        .unwrap_or(default)
}
