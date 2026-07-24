//! Pure-CPU structural survey of a car's GEOMETRY.BIN — no GPU, no engine — emitting one JSON
//! object so a caller can categorise *why* different cars render so differently under the same
//! parser + material routing. Reproduces the game's `place_point` (row-vector `v·M`, applied only
//! for a proper transform, det > 0) and the handful of NFSU2 shader hashes the router keys off.
//!
//! Usage: cargo run -p gizmo-nfs --features tools --example nfs_survey -- GEOMETRY.BIN [CarName]

use gizmo_nfs::{parse_geometry, Mat4, NfsMeshPart};

// NFSU2 shader hashes (0x00134013), mirrored from the game's `car::shader`.
const CARSKIN: u32 = 0xd6d6_080a; // painted body panels
const PLAINNOTHING: u32 = 0x010c_b64a; // unshaded filler (interior tub on BASE, wheel-well filler on kits)

fn det3(m: &Mat4) -> f32 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Mirror of the game's `mesh::should_place`: apply a matrix only when it is a proper transform
/// (det > 0) that either carries a real translation OR belongs to a part modelled at the origin
/// (an oriented instanced detail — wheels, brakes). A reflection (det < 0), or a rotation/scale
/// with no translation on an *off-origin* part (open-door / raised-hood articulation), is left
/// as the vertices are already baked.
fn should_place(m: &Mat4, centroid: &[f32; 3]) -> bool {
    if det3(m) <= 1e-6 {
        return false;
    }
    if m[3][0].abs() + m[3][1].abs() + m[3][2].abs() > 1e-4 {
        return true;
    }
    (centroid[0] * centroid[0] + centroid[1] * centroid[1] + centroid[2] * centroid[2]).sqrt() < 0.35
}

/// Local (pre-transform) centroid of a part's vertices — how far it is modelled from the origin.
fn local_centroid(p: &NfsMeshPart) -> [f32; 3] {
    let n = p.positions.len().max(1) as f64;
    let mut s = [0.0f64; 3];
    for v in &p.positions {
        for k in 0..3 {
            s[k] += v[k] as f64;
        }
    }
    [(s[0] / n) as f32, (s[1] / n) as f32, (s[2] / n) as f32]
}

/// The game's placement: row-vector `v·M` with translation in the last row, gated by
/// [`should_place`] against the part's local centroid.
fn place_point(m: &Mat4, p: [f32; 3], apply: bool) -> [f32; 3] {
    if !apply {
        return p;
    }
    [
        p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0] + m[3][0],
        p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1] + m[3][1],
        p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2] + m[3][2],
    ]
}

/// A part is part of the default showroom car (shared BASE + kit slot 00), excluding the
/// duplicate audio-trunk shell and window decals — the same gate the game's `select_stock_car`
/// applies, kept loose (all LODs) since we only need structural facts, not the exact LOD pick.
fn is_stock(name: &str) -> bool {
    (name.contains("_BASE") || name.contains("_KIT00")) && !name.contains("TRUNK_AUDIO") && !name.contains("DECAL")
}

fn placed_bounds(p: &NfsMeshPart) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let apply = should_place(&p.transform, &local_centroid(p));
    let (mut lo, mut hi, mut sum) = ([f32::MAX; 3], [f32::MIN; 3], [0.0f64; 3]);
    for v in &p.positions {
        let g = place_point(&p.transform, *v, apply);
        for k in 0..3 {
            lo[k] = lo[k].min(g[k]);
            hi[k] = hi[k].max(g[k]);
            sum[k] += g[k] as f64;
        }
    }
    let n = p.positions.len().max(1) as f64;
    let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let cen = [(sum[0] / n) as f32, (sum[1] / n) as f32, (sum[2] / n) as f32];
    (ext, cen, [lo[0], lo[1], lo[2]])
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: nfs_survey GEOMETRY.BIN [CarName]");
    let car = std::env::args().nth(2).unwrap_or_else(|| path.clone());
    let raw = std::fs::read(&path).expect("read");
    let bytes = match gizmo_nfs::compression::detect(&raw) {
        gizmo_nfs::compression::Codec::None => raw.clone(),
        _ => gizmo_nfs::compression::decompress(&raw).expect("decompress"),
    };
    let parts = match parse_geometry(&bytes) {
        Ok(p) => p,
        Err(e) => {
            println!("{{\"car\":\"{car}\",\"parse_error\":\"{e}\"}}");
            return;
        }
    };

    // SURVEY_DUMP=<substr>: raw 4x4 matrix + det + local/placed bounds for matching parts.
    if let Ok(sub) = std::env::var("SURVEY_DUMP") {
        for p in parts.iter().filter(|p| p.name.contains(sub.as_str())) {
            let m = &p.transform;
            let (lo, hi) = {
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for v in &p.positions {
                    for k in 0..3 {
                        lo[k] = lo[k].min(v[k]);
                        hi[k] = hi[k].max(v[k]);
                    }
                }
                (lo, hi)
            };
            let (ext, cen, _) = placed_bounds(p);
            let d = det3(m);
            let trans = m[3][0].abs() + m[3][1].abs() + m[3][2].abs();
            let cn = local_centroid(p);
            let cdist = (cn[0] * cn[0] + cn[1] * cn[1] + cn[2] * cn[2]).sqrt();
            let ident3 = (m[0][0] - 1.0).abs() + m[0][1].abs() + m[0][2].abs() + m[1][0].abs()
                + (m[1][1] - 1.0).abs() + m[1][2].abs() + m[2][0].abs() + m[2][1].abs() + (m[2][2] - 1.0).abs();
            // Match the game's should_place, and name why each part is/ isn't applied.
            let class = if d < -1e-6 {
                "reflection" // det<0 → baked mirror, skipped
            } else if trans > 1e-4 {
                "placement " // real translation → applied
            } else if ident3 <= 1e-3 {
                "identity  " // no-op
            } else if cdist < 0.35 {
                "oriented  " // rotation on an origin-modelled detail → applied (no move)
            } else {
                "ARTICULATE" // off-origin rotation/scale, no translation → SKIPPED by the fix
            };
            eprintln!(
                "{class} {:<34} det={:+.2} |t|={:5.2} local_cen=[{:+.2},{:+.2},{:+.2}] placed_cen=[{:+.2},{:+.2},{:+.2}] placed_ext=[{:.2},{:.2},{:.2}]",
                p.name, d, trans,
                (lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0,
                cen[0], cen[1], cen[2], ext[0], ext[1], ext[2]
            );
            if std::env::var("SURVEY_MATRIX").is_ok() {
                for row in m {
                    eprintln!("       [{:+8.3} {:+8.3} {:+8.3} {:+8.3}]", row[0], row[1], row[2], row[3]);
                }
            }
        }
        return;
    }
    let stock: Vec<&NfsMeshPart> = parts.iter().filter(|p| is_stock(&p.name)).collect();

    // Whole-car placed extent + per-part explosion / displacement flags.
    let (mut clo, mut chi) = ([f32::MAX; 3], [f32::MIN; 3]);
    let mut exploded = Vec::new();
    for p in &stock {
        let (ext, cen, _) = placed_bounds(p);
        for k in 0..3 {
            clo[k] = clo[k].min(cen[k] - ext[k] / 2.0);
            chi[k] = chi[k].max(cen[k] + ext[k] / 2.0);
        }
        let maxext = ext[0].max(ext[1]).max(ext[2]);
        let far = cen[0].abs().max(cen[1].abs()).max(cen[2].abs());
        if maxext > 8.0 || far > 5.0 {
            exploded.push(format!(
                "{{\"name\":\"{}\",\"ext\":[{:.1},{:.1},{:.1}],\"cen\":[{:.1},{:.1},{:.1}]}}",
                p.name, ext[0], ext[1], ext[2], cen[0], cen[1], cen[2]
            ));
        }
    }
    // A dir with no stock parts (a shared parts bin: WHEELS/BRAKES/…) leaves the accumulators at
    // ±MAX, giving a non-finite extent that breaks JSON — clamp to 0 so the line stays parseable.
    let fin = |x: f32| if x.is_finite() { x } else { 0.0 };
    let car_ext = [fin(chi[0] - clo[0]), fin(chi[1] - clo[1]), fin(chi[2] - clo[2])];

    // Door analysis: does the exterior door skin exist as a paintable surface?
    let is_door_skin = |n: &str| n.contains("DOOR") && !n.contains("PANEL") && !n.contains("SILL");
    let door_has_carskin = stock
        .iter()
        .filter(|p| p.name.contains("DOOR"))
        .any(|p| p.materials.iter().any(|m| m.shader.0 == CARSKIN));
    let door_skin_nomat = stock.iter().any(|p| is_door_skin(&p.name) && p.materials.is_empty());
    let door_skin_parts = stock.iter().filter(|p| is_door_skin(&p.name)).count();

    // Per-door detail (skins only) so a reader can spot a door swung open by its transform.
    let mut doors = Vec::new();
    for p in stock.iter().filter(|p| is_door_skin(&p.name)) {
        let (ext, cen, _) = placed_bounds(p);
        let carskin = p.materials.iter().any(|m| m.shader.0 == CARSKIN);
        doors.push(format!(
            "{{\"name\":\"{}\",\"mats\":{},\"carskin\":{},\"det\":{:.0},\"cen\":[{:.2},{:.2},{:.2}],\"ext\":[{:.2},{:.2},{:.2}]}}",
            p.name, p.materials.len(), carskin, det3(&p.transform).signum(),
            cen[0], cen[1], cen[2], ext[0], ext[1], ext[2]
        ));
    }

    // PLAINNOTHING filler carried on non-BASE (kit) parts — the black-square source.
    let (mut pn_runs, mut pn_tris, mut zmin, mut zmax) = (0u32, 0u32, f32::MAX, f32::MIN);
    for p in stock.iter().filter(|p| !p.name.contains("_BASE")) {
        let apply = should_place(&p.transform, &local_centroid(p));
        for m in &p.materials {
            if m.shader.0 != PLAINNOTHING {
                continue;
            }
            pn_runs += 1;
            pn_tris += (m.index_count / 3) as u32;
            for &idx in p.indices.get(m.index_offset..m.index_offset + m.index_count).unwrap_or(&[]) {
                if let Some(pos) = p.positions.get(idx as usize) {
                    let z = place_point(&p.transform, *pos, apply)[2];
                    zmin = zmin.min(z);
                    zmax = zmax.max(z);
                }
            }
        }
    }
    let (zmin, zmax) = if pn_runs == 0 { (0.0, 0.0) } else { (zmin, zmax) };

    // Shader histogram (tris per shader) across the stock car — top 4.
    use std::collections::HashMap;
    let mut hist: HashMap<u32, u32> = HashMap::new();
    for p in &stock {
        for m in &p.materials {
            *hist.entry(m.shader.0).or_default() += (m.index_count / 3) as u32;
        }
    }
    let mut sh: Vec<(u32, u32)> = hist.into_iter().collect();
    sh.sort_by_key(|&(_, tris)| std::cmp::Reverse(tris));
    let shtop: Vec<String> = sh.iter().take(4).map(|(h, t)| format!("[\"{h:#010x}\",{t}]")).collect();

    println!(
        "{{\"car\":\"{car}\",\"solids\":{},\"stock\":{},\"car_ext\":[{:.2},{:.2},{:.2}],\
         \"exploded\":[{}],\"door_skin_parts\":{door_skin_parts},\"door_has_carskin\":{door_has_carskin},\
         \"door_skin_nomat\":{door_skin_nomat},\"door_ok\":{},\"doors\":[{}],\
         \"plain_nb\":{{\"runs\":{pn_runs},\"tris\":{pn_tris},\"zmin\":{:.2},\"zmax\":{:.2}}},\"shader_top\":[{}]}}",
        parts.len(),
        stock.len(),
        car_ext[0], car_ext[1], car_ext[2],
        exploded.join(","),
        door_has_carskin || door_skin_nomat,
        doors.join(","),
        zmin, zmax,
        shtop.join(",")
    );
}
