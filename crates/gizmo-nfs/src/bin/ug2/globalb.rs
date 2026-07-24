//! `ug2 globalb` — the per-car records in the game's global bundle.

use crate::paths::{self, Result};
use gizmo_nfs::globalb::parse_cartypeinfos;
use std::path::{Path, PathBuf};

pub fn run(path: &Path, filter: Option<&str>) -> Result<()> {
    let file = locate(path)?;
    let bytes = paths::read(&file)?;
    let cars = parse_cartypeinfos(&bytes);
    outln!("{} CarTypeInfo records in {}\n", cars.len(), file.display());
    outln!("{:<14} {:>9} {:>7} {:>7} {:>8}   front-left mount", "car", "wheelbase", "track", "radius", "mass");
    for c in cars.iter().filter(|c| filter.is_none_or(|f| c.name.contains(f))) {
        let (fl, rr) = (c.wheels[0], c.wheels[2]);
        outln!(
            "{:<14} {:>8.2}m {:>6.2}m {:>6.3}m {:>6.0}kg   fa={:+.2} lat={:+.2} rh={:+.2}",
            c.name,
            (fl.fore_aft - rr.fore_aft).abs(),
            fl.lateral.abs() * 2.0,
            fl.radius,
            c.mass_kg,
            fl.fore_aft,
            fl.lateral,
            fl.ride_height
        );
    }
    Ok(())
}

/// Accept the bundle itself, a car directory, or the game root.
fn locate(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    if let Some(p) = paths::globalb_beside(path) {
        return Ok(p); // a car directory
    }
    let direct = path.join("GLOBAL").join("GLOBALB.BUN");
    if direct.is_file() {
        return Ok(direct); // the game root
    }
    Err(format!("no GLOBALB.BUN at or under {}", path.display()))
}
