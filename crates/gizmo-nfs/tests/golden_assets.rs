//! Golden tests against a real, legally-owned NFSU2 install.
//!
//! These are **skipped unless `NFSU2_ROOT` is set** (to the game's install directory), so
//! CI and other machines stay asset-free. Run locally with, e.g.:
//!
//! ```bash
//! NFSU2_ROOT="/path/to/Need for Speed Underground 2" \
//!   cargo test -p gizmo-nfs --test golden_assets
//! ```

use std::path::PathBuf;

fn root() -> Option<PathBuf> {
    std::env::var_os("NFSU2_ROOT").map(PathBuf::from)
}

/// The decisive JDLZ validation: `InGameCommon.lzc` (JDLZ-compressed) must decompress
/// byte-for-byte to `InGameCommon.bun` (the same bundle, uncompressed) shipped alongside it.
#[test]
fn jdlz_matches_ingamecommon_bun() {
    let Some(root) = root() else {
        eprintln!("NFSU2_ROOT unset — skipping golden JDLZ test");
        return;
    };
    let lzc = std::fs::read(root.join("GLOBAL/InGameCommon.lzc")).expect("read InGameCommon.lzc");
    let bun = std::fs::read(root.join("GLOBAL/InGameCommon.bun")).expect("read InGameCommon.bun");

    let out = gizmo_nfs::compression::jdlz::decompress(&lzc).expect("jdlz decompress");
    assert_eq!(out.len(), bun.len(), "decompressed length {} != bun length {}", out.len(), bun.len());
    if let Some(i) = out.iter().zip(bun.iter()).position(|(a, b)| a != b) {
        let lo = i.saturating_sub(4);
        panic!(
            "first mismatch at byte {i}: got {:02X?} want {:02X?}",
            out.get(lo..i + 4).unwrap_or(&[]),
            bun.get(lo..i + 4).unwrap_or(&[]),
        );
    }
}

/// A real car's `GEOMETRY.BIN` must parse as a chunk tree without error, with a top-level
/// `0x80134000` "solid list" container.
#[test]
fn geometry_bin_parses_as_chunk_tree() {
    let Some(root) = root() else {
        eprintln!("NFSU2_ROOT unset — skipping geometry parse test");
        return;
    };
    let bytes = std::fs::read(root.join("CARS/240SX/GEOMETRY.BIN")).expect("read GEOMETRY.BIN");
    let tree = gizmo_nfs::chunk::ChunkNode::parse(&bytes).expect("parse chunk tree");
    assert!(!tree.is_empty(), "expected at least one top-level chunk");
    assert_eq!(tree[0].header.id, 0x8013_4000, "expected solid-list container at top");
}

/// A real car's `TEXTURES.BIN` must parse with a top-level `0xB3300000` TPK container.
#[test]
fn textures_bin_is_a_tpk_container() {
    let Some(root) = root() else {
        eprintln!("NFSU2_ROOT unset — skipping textures parse test");
        return;
    };
    let bytes = std::fs::read(root.join("CARS/240SX/TEXTURES.BIN")).expect("read TEXTURES.BIN");
    let tree = gizmo_nfs::chunk::ChunkNode::parse(&bytes).expect("parse chunk tree");
    assert!(!tree.is_empty());
    assert_eq!(tree[0].header.id, 0xB330_0000, "expected TPK container at top");
}

/// The full geometry parser on a real car: many parts, the base body present with the
/// exact known counts, and every part's indices in range (the decisive layout check).
#[test]
fn geometry_parser_extracts_valid_parts() {
    let Some(root) = root() else {
        eprintln!("NFSU2_ROOT unset — skipping geometry parser test");
        return;
    };
    let bytes = std::fs::read(root.join("CARS/240SX/GEOMETRY.BIN")).expect("read GEOMETRY.BIN");
    let parts = gizmo_nfs::parse_geometry(&bytes).expect("parse geometry");
    assert!(parts.len() > 100, "240SX has hundreds of solids, got {}", parts.len());

    // Every part must have consistent, in-range, triangle-list geometry.
    for p in &parts {
        assert!(p.indices_in_range(), "part {} has out-of-range indices", p.name);
        assert_eq!(p.indices.len() % 3, 0, "part {} indices not a triangle list", p.name);
        assert_eq!(p.positions.len(), p.normals.len());
        assert_eq!(p.positions.len(), p.uvs.len());
    }

    // The base body, highest LOD, has the counts we locked during reverse-engineering.
    let base_a = parts.iter().find(|p| p.name == "240SX_BASE_A").expect("240SX_BASE_A present");
    assert_eq!(base_a.positions.len(), 483, "base_a vertex count");
    assert_eq!(base_a.triangle_count(), 496, "base_a triangle count");
    assert!(matches!(base_a.role, gizmo_nfs::PartRole::Body));
    assert!(matches!(base_a.lod, gizmo_nfs::LodLevel::A));
    // Normals are unit length.
    let n = base_a.normals[0];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    assert!((len - 1.0).abs() < 0.02, "normal not unit length: {len}");
}

/// The TPK parser on a real car: the known descriptor count, and every JDLZ-compressed
/// texture decoded to a correctly-sized RGBA8 image whose embedded header hash matches.
#[test]
fn tpk_parser_decodes_textures() {
    let Some(root) = root() else {
        eprintln!("NFSU2_ROOT unset — skipping TPK parser test");
        return;
    };
    let bytes = std::fs::read(root.join("CARS/240SX/TEXTURES.BIN")).expect("read TEXTURES.BIN");
    let tpk = gizmo_nfs::texture::Tpk::parse(&bytes).expect("parse TPK");

    // 240SX ships 73 textures; every one decodes now that both JDLZ and HUFF are supported.
    assert_eq!(tpk.entries.len(), 73, "descriptor count");
    assert_eq!(tpk.textures.len(), 73, "all 73 textures decode (JDLZ + HUFF)");
    assert_eq!(
        tpk.entries.iter().map(|e| e.header_from_end).collect::<std::collections::HashSet<_>>(),
        std::iter::once(0x100).collect(),
        "every descriptor's header_from_end is the constant 0x100"
    );

    let mut dxt1 = 0;
    for tex in tpk.textures.values() {
        // Dimensions are powers of two and the RGBA buffer is exactly W*H*4.
        assert!(tex.width.is_power_of_two() && tex.height.is_power_of_two(), "dims power of two");
        assert_eq!(tex.rgba.len(), tex.width as usize * tex.height as usize * 4, "tight RGBA8");
        assert_eq!(tex.format, gizmo_nfs::PixelFormat::Rgba8);
        if matches!(tex.source_format, gizmo_nfs::TexFormat::Dxt1) {
            dxt1 += 1;
        }
    }
    assert!(dxt1 >= 5, "expected several DXT1 textures, got {dxt1}");

    // The embedded DebugNames are recovered and carry the expected part-linked names.
    let names: Vec<&str> = tpk.textures.values().map(|t| t.name.as_str()).collect();
    assert!(names.iter().any(|n| n.starts_with("240SX_KIT00_HEADLIGHT")), "headlight texture named");
    assert!(names.iter().any(|n| n.starts_with("240SX_KIT00_BRAKELIGHT")), "brakelight texture named");
}
