//! `ug2 probe` — the raw view of a car's solids, for locking down a format.
//!
//! Two questions this answers that the parsed output cannot: what a solid *declares* (counts,
//! buffer sizes, the mesh-header words) versus what the standard layout expects, and what its
//! local matrix means. A solid whose implied bytes-per-vertex is not ~36 is one the parser
//! skips rather than mis-decodes; a shifted mesh header shows up as leading `0x11111111` words.

use crate::paths::{Car, Result};
use gizmo_nfs::chunk::ChunkNode;
use gizmo_nfs::placement::{part_centroid, should_place};
use gizmo_nfs::types::Mat4;
use gizmo_nfs::geometry::VERTEX_STRIDE;
use std::path::Path;

const SOLID: u32 = 0x8013_4010;
const SOLID_HEADER: u32 = 0x0013_4011;
const MESH_HEADER: u32 = 0x0013_4900;
const VERTEX_BUFFER: u32 = 0x0013_4B01;

pub fn run(car: &Path, filter: Option<&str>, matrices: bool) -> Result<()> {
    let car = Car::resolve(car)?;
    let bytes = crate::paths::read(&car.geometry)?;
    let roots = ChunkNode::parse(&bytes).map_err(|e| format!("{}: {e}", car.geometry.display()))?;

    outln!("{:<28} {:>6} {:>6} {:>8} {:>6}  mesh-header u32[0..16]", "solid", "verts", "tris", "vbuf", "bpv");
    for top in &roots {
        for solid in top.find_all(SOLID) {
            let (Some(mesh), Some(vbuf)) = (solid.find(MESH_HEADER), solid.find(VERTEX_BUFFER)) else {
                continue;
            };
            let md = mesh.data(&bytes);
            let name = solid.find(SOLID_HEADER).map(|h| longest_ascii(h.data(&bytes))).unwrap_or_default();
            if filter.is_some_and(|f| !name.contains(f)) {
                continue;
            }
            // Read the counts the way the parser does: past any leading 0x11 filler words.
            let shift = leading_filler_words(md);
            let tris = u32_at(md, shift + 9);
            let verts = u32_at(md, shift + 13) as usize;
            let vlen = vbuf.data(&bytes).len();
            let bpv = if verts > 0 { vlen as f64 / verts as f64 } else { 0.0 };
            let hdr: Vec<String> = (0..16).map(|w| u32_at(md, w).to_string()).collect();
            let mut note = String::new();
            if shift > 0 {
                note.push_str(&format!("  <== header shifted by {shift} word(s)"));
            }
            if verts * VERTEX_STRIDE > vlen {
                note.push_str("  <== verts*36 > vbuf (layout not decoded, solid skipped)");
            }
            outln!("{name:<28} {verts:>6} {tris:>6} {vlen:>8} {bpv:>6.1}  [{}]{note}", hdr.join(","));
        }
    }

    if matrices {
        print_matrices(&car, filter)?;
    }
    Ok(())
}

/// Classify each part's local matrix the way every consumer must: applied placement, baked
/// pose, or a mirrored right-side part.
fn print_matrices(car: &Car, filter: Option<&str>) -> Result<()> {
    outln!("\n{:<28} {:>7} {:>7}  local centroid          class", "part", "det", "|t|");
    for p in car.parts()?.iter().filter(|p| filter.is_none_or(|f| p.name.contains(f))) {
        let m: &Mat4 = &p.transform;
        let cen = part_centroid(p);
        let det = gizmo_nfs::placement::det3(m);
        let t = m[3][0].abs() + m[3][1].abs() + m[3][2].abs();
        let class = if det < 0.0 {
            "reflection (baked mirror, skipped)"
        } else if should_place(m, &cen) {
            "placement (applied)"
        } else {
            "baked pose (skipped)"
        };
        outln!(
            "{:<28} {det:>+7.2} {t:>7.2}  [{:+.2},{:+.2},{:+.2}]  {class}",
            p.name, cen[0], cen[1], cen[2]
        );
    }
    Ok(())
}

/// How many leading `0x11111111` filler words sit in front of a chunk payload.
fn leading_filler_words(d: &[u8]) -> usize {
    let mut n = 0;
    while d.get(n * 4..n * 4 + 4) == Some(&[0x11; 4]) {
        n += 1;
    }
    n
}

fn u32_at(d: &[u8], w: usize) -> u32 {
    d.get(w * 4..w * 4 + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}

/// Longest run of ASCII-graphic bytes — the same way the parser recovers a part name.
fn longest_ascii(d: &[u8]) -> String {
    let (mut best, mut start) = (0..0, 0usize);
    for i in 0..=d.len() {
        if !d.get(i).is_some_and(u8::is_ascii_graphic) {
            if i - start > best.len() {
                best = start..i;
            }
            start = i + 1;
        }
    }
    String::from_utf8_lossy(d.get(best).unwrap_or(&[])).into_owned()
}
