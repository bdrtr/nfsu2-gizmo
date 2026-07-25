//! The I/O edge: reading a car's sibling asset files and its environment overrides.
//!
//! Everything else in this crate is a pure function of bytes already in memory; the filesystem
//! and the environment are touched only here (and in the binaries' argument handling).

use gizmo_nfs::{CarHandling, CarTypeInfo, Colour, Tpk};

/// Load and decode the `TEXTURES.BIN` sitting next to a car's `GEOMETRY.BIN`, if present.
/// Returns `None` (untextured car) when the file is absent or unparseable.
#[must_use]
pub fn load_tpk_beside(geometry_path: &str) -> Option<Tpk> {
    let dir = std::path::Path::new(geometry_path).parent()?;
    let bytes = std::fs::read(dir.join("TEXTURES.BIN")).ok()?;
    Tpk::parse(&bytes).ok()
}

/// Everything `GLOBAL/GLOBALB.BUN` has to say about one car, read in a single pass.
///
/// The bundle is 8 MB and holds three unrelated things this game wants — the car's placement record,
/// how it drives, and the palette its paint comes from — so reading and decompressing it once and
/// answering all three beats three separate 8 MB reads. Any field may be `None` on its own: a car
/// that is not in the record list still gets the palette, and an install without the bundle gets
/// neither rather than failing to start.
#[derive(Debug, Clone, Default)]
pub struct GlobalbCar {
    /// Wheel mounts, radius, mass, body box.
    pub info: Option<CarTypeInfo>,
    /// The rpm limits, torque curve, gearboxes and drivetrain split. Carried on the record itself;
    /// mirrored here so a caller can ask for it without unwrapping [`Self::info`] twice.
    pub handling: Option<CarHandling>,
    /// The 123 colours the game paints bodies with. Empty when the bundle would not parse.
    pub palette: Vec<Colour>,
}

/// Read `GLOBAL/GLOBALB.BUN` for the car a `CARS/<name>/GEOMETRY.BIN` path names: up two
/// directories to the game root, then the bundle, looked up by the car's folder name.
#[must_use]
pub fn load_globalb_beside(geometry_path: &str) -> GlobalbCar {
    let Some(bytes) = globalb_bytes(geometry_path) else { return GlobalbCar::default() };
    let name = std::path::Path::new(geometry_path)
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let info = gizmo_nfs::globalb::find_car(&bytes, &name);
    GlobalbCar {
        handling: info.as_ref().map(|i| i.handling),
        info,
        palette: gizmo_nfs::CarParts::parse(&bytes).map(|p| p.palette()).unwrap_or_default(),
    }
}

/// The bundle's bytes, decompressed if it is stored that way.
fn globalb_bytes(geometry_path: &str) -> Option<Vec<u8>> {
    let geo = std::path::Path::new(geometry_path);
    let root = geo.parent()?.parent()?.parent()?; // up past CARS/<name>/
    let raw = std::fs::read(root.join("GLOBAL").join("GLOBALB.BUN")).ok()?;
    match gizmo_nfs::compression::detect(&raw) {
        gizmo_nfs::compression::Codec::None => Some(raw),
        _ => gizmo_nfs::compression::decompress(&raw).ok(),
    }
}

/// Load this car's [`CarTypeInfo`] (exact wheel mounts, radius, mass) from the game's global
/// bundle. Kept for callers that want only that; [`load_globalb_beside`] answers all three
/// questions from one read.
#[must_use]
pub fn load_cartypeinfo_beside(geometry_path: &str) -> Option<CarTypeInfo> {
    load_globalb_beside(geometry_path).info
}

/// The body colour to paint with: one of the game's own, chosen by `NFS_PAINT` as an index into
/// the palette, else the first.
///
/// This replaces a hardcoded RGB triple. NFSU2 does not texture a car's body — it paints it — and
/// `GLOBALB.BUN` is the only place those colours are written down, so a car painted anything else
/// is painted a colour the game does not have. `NFS_COLOR` still overrides with a raw triple, for
/// looking at something deliberately wrong.
#[must_use]
pub fn paint_from_palette(palette: &[Colour], fallback: [f32; 3]) -> [f32; 3] {
    if let Ok(raw) = std::env::var("NFS_COLOR") {
        let v: Vec<f32> = raw.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() == 3 {
            return [v[0], v[1], v[2]];
        }
    }
    if palette.is_empty() {
        return fallback;
    }
    let i = std::env::var("NFS_PAINT").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let c = palette[i % palette.len()];
    // The palette stores 8-bit channels; the renderer wants linear 0..1.
    let to_linear = |b: u8| {
        let s = f32::from(b) / 255.0;
        if s <= 0.040_45 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    [to_linear(c.red), to_linear(c.green), to_linear(c.blue)]
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
