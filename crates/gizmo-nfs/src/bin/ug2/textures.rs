//! `ug2 textures` — the car's texture table, and which material run uses which image.

use crate::paths::{Car, Result};
use std::path::Path;

pub fn run(car: &Path, filter: Option<&str>) -> Result<()> {
    let car = Car::resolve(car)?;
    let parts = car.parts()?;
    let Some(tpk) = car.textures() else {
        return Err(format!("{}: no readable TEXTURES.BIN", car.dir.display()));
    };

    outln!("== {} textures ==", tpk.textures.len());
    let mut texs: Vec<_> = tpk.textures.values().collect();
    texs.sort_by(|a, b| a.name.cmp(&b.name));
    for t in &texs {
        // opaque% and mean luminance say whether a map is an alpha overlay (mostly transparent)
        // or a full-coverage image — the difference that decides whether it can be composited
        // over the paint as a detail layer.
        let texels = (t.rgba.len() / 4).max(1);
        let opaque = t.rgba.chunks_exact(4).filter(|px| px[3] > 200).count() * 100 / texels;
        let lum: usize =
            t.rgba.chunks_exact(4).map(|px| (px[0] as usize + px[1] as usize + px[2] as usize) / 3).sum();
        outln!(
            "  {:#010x}  {:>4}x{:<4}  {:?}  opaque={opaque:>3}%  lum={:>3}  {}",
            t.hash.0,
            t.width,
            t.height,
            t.source_format,
            lum / texels,
            t.name
        );
    }

    outln!("\n== material runs (shader | texture hash → resolved texture) ==");
    for p in parts.iter().filter(|p| filter.is_none_or(|f| p.name.contains(f))) {
        if p.materials.is_empty() {
            continue;
        }
        outln!("• {} ({} runs)", p.name, p.materials.len());
        for m in &p.materials {
            let resolved = tpk.texture(m.hash).map(|t| t.name.as_str()).unwrap_or("—");
            outln!(
                "    shader={:#010x}  tex={:#010x} → {:<28}  tris={}",
                m.shader.0,
                m.hash.0,
                resolved,
                m.index_count / 3
            );
        }
    }
    Ok(())
}
