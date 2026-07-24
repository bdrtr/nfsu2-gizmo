//! Dump a car's TPK textures and how each geometry material run maps to them — the ground truth
//! for wiring body-detail (doorline/badging) and tyre textures onto the mesh.
//!
//! Usage: cargo run -p gizmo-nfs --features tools --example nfs_textures -- CAR_DIR [substr]
//! where CAR_DIR holds GEOMETRY.BIN + TEXTURES.BIN. Optional substr filters parts by name.

use gizmo_nfs::{parse_geometry, Tpk};
use std::path::Path;

fn load(dir: &Path, name: &str) -> Vec<u8> {
    let raw = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"));
    match gizmo_nfs::compression::detect(&raw) {
        gizmo_nfs::compression::Codec::None => raw,
        _ => gizmo_nfs::compression::decompress(&raw).expect("decompress"),
    }
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: nfs_textures CAR_DIR [substr]");
    let filter = std::env::args().nth(2).unwrap_or_default();
    let dir = Path::new(&dir);
    let parts = parse_geometry(&load(dir, "GEOMETRY.BIN")).expect("parse geometry");
    let tpk = Tpk::parse(&load(dir, "TEXTURES.BIN")).expect("parse tpk");

    println!("=== {} textures ===", tpk.textures.len());
    let mut texs: Vec<_> = tpk.textures.values().collect();
    texs.sort_by_key(|t| t.name.clone());
    for t in &texs {
        // opaque% and mean luminance say whether a map is an alpha overlay (mostly transparent)
        // or a full-coverage image — the difference that decides if it can be composited over
        // the paint as a detail layer.
        let n = (t.rgba.len() / 4).max(1);
        let opaque = t.rgba.chunks_exact(4).filter(|px| px[3] > 200).count() * 100 / n;
        let lum: u32 = t.rgba.chunks_exact(4).map(|px| (px[0] as u32 + px[1] as u32 + px[2] as u32) / 3).sum();
        println!(
            "  {:#010x}  {:>4}x{:<4}  {:?}  opaque={:>3}%  lum={:>3}  {}",
            t.hash.0, t.width, t.height, t.source_format, opaque, lum as usize / n, t.name
        );
    }

    println!("\n=== stock body parts: material runs (shader | tex-hash → resolved texture) ===");
    for p in parts.iter().filter(|p| {
        (p.name.contains("_BASE") || p.name.contains("_KIT00"))
            && !p.name.contains("TRUNK_AUDIO")
            && !p.name.contains("DECAL")
            && p.name.contains(filter.as_str())
    }) {
        println!("• {} ({} runs, {} material_refs)", p.name, p.materials.len(), p.material_refs.len());
        for m in &p.materials {
            let resolved = tpk.texture(m.hash).map(|t| t.name.as_str()).unwrap_or("—");
            println!(
                "    shader={:#010x}  tex={:#010x} → {:<32}  tris={}",
                m.shader.0, m.hash.0, resolved, m.index_count / 3
            );
        }
    }
}
