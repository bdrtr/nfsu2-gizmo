//! `ug2 info` — one screen that answers "what is this car, and what can I build from it?".

use crate::paths::{Car, Result};
use gizmo_nfs::parts::{group_of, select_car, CarConfig, Grp};
use gizmo_nfs::placement::{place_point, part_centroid, should_place};
use gizmo_nfs::NfsMeshPart;
use std::collections::BTreeSet;
use std::path::Path;

pub fn run(car: &Path, config: CarConfig) -> Result<()> {
    let car = Car::resolve(car)?;
    let parts = car.parts()?;
    let tpk = car.textures();
    let selected = select_car(&parts, &config);

    outln!("{} — {} parts, {} textures", car.name, parts.len(), tpk.as_ref().map_or(0, |t| t.textures.len()));
    outln!("  {}", car.geometry.display());

    // ── The configuration this call selected ──
    let tris: usize = selected.iter().map(|p| p.triangle_count()).sum();
    let (lo, hi) = bounds(&selected);
    outln!(
        "\nconfiguration {config:?}\n  {} parts, {tris} triangles, {:.2} x {:.2} x {:.2} m (length x width x height)",
        selected.len(),
        hi[0] - lo[0],
        hi[1] - lo[1],
        hi[2] - lo[2],
    );
    let mut by_group: Vec<(Grp, usize)> = Vec::new();
    for p in &selected {
        let g = group_of(&p.name);
        match by_group.iter_mut().find(|(k, _)| *k == g) {
            Some((_, n)) => *n += 1,
            None => by_group.push((g, 1)),
        }
    }
    by_group.sort_by_key(|(g, _)| format!("{g:?}"));
    let groups: Vec<String> = by_group.iter().map(|(g, n)| format!("{g:?}×{n}")).collect();
    outln!("  groups: {}", groups.join(", "));

    // ── What else the car ships ──
    let (kits, wides, hoods, lights) = variants(&parts);
    outln!("\nvariants this car ships");
    outln!("  body kits  --kit  : {}", fmt_set(&kits));
    outln!("  widebodies --wide : {}", fmt_set(&wides));
    outln!("  hoods      --hood : {}", fmt_set(&hoods));
    outln!("  lights     --light: {}", fmt_set(&lights));

    // ── The game's own numbers for it, when the bundle is reachable ──
    match car.car_type_info() {
        Some(c) => {
            let (fl, rr) = (c.wheels[0], c.wheels[2]);
            outln!(
                "\nGLOBALB record\n  wheelbase {:.2} m, track {:.2} m, wheel radius {:.3} m, mass {:.0} kg",
                (fl.fore_aft - rr.fore_aft).abs(),
                fl.lateral.abs() * 2.0,
                fl.radius,
                c.mass_kg
            );
            for (label, w) in ["front-left", "front-right", "rear-left", "rear-right"].iter().zip(&c.wheels) {
                outln!("    {label:<12} fore/aft {:+.2}  lateral {:+.2}  ride height {:+.2}", w.fore_aft, w.lateral, w.ride_height);
            }
        }
        None => outln!("\nGLOBALB record: not found (wheel mounts fall back to the fitted stance)"),
    }
    Ok(())
}

/// The variant numbers a car actually ships, per configurable slot. Kit numbering is sparse —
/// no car has every number — so this is what makes `--kit 19` a knowable mistake.
fn variants(parts: &[NfsMeshPart]) -> (BTreeSet<u8>, BTreeSet<u8>, BTreeSet<u8>, BTreeSet<u8>) {
    let (mut kits, mut wides, mut hoods, mut lights) = Default::default();
    let push = |set: &mut BTreeSet<u8>, n: u8| {
        if n != 0 {
            set.insert(n);
        }
    };
    for p in parts {
        let name = &p.name;
        if let Some(n) = num_after(name, "KITW") {
            push(&mut wides, n);
        } else if let Some(n) = num_after(name, "STYLE") {
            if name.contains("HOOD") {
                push(&mut hoods, n);
            } else if name.contains("HEADLIGHT") || name.contains("BRAKELIGHT") {
                push(&mut lights, n);
            }
        } else if let Some(n) = num_after(name, "KIT") {
            if name.contains("BUMPER") || name.contains("SKIRT") {
                push(&mut kits, n);
            }
        }
    }
    (kits, wides, hoods, lights)
}

fn fmt_set(s: &BTreeSet<u8>) -> String {
    if s.is_empty() {
        return "none".into();
    }
    let list: Vec<String> = s.iter().map(u8::to_string).collect();
    format!("{} ({})", list.join(","), s.len())
}

fn num_after(name: &str, tag: &str) -> Option<u8> {
    let i = name.find(tag)? + tag.len();
    name[i..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

/// Bounds in NFSU2's own frame (x = length, y = width, z = height), placement applied.
fn bounds(parts: &[&NfsMeshPart]) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let mut any = false;
    for p in parts {
        let apply = should_place(&p.transform, &part_centroid(p));
        for v in &p.positions {
            let g = place_point(&p.transform, *v, apply);
            for k in 0..3 {
                lo[k] = lo[k].min(g[k]);
                hi[k] = hi[k].max(g[k]);
            }
            any = true;
        }
    }
    if any {
        (lo, hi)
    } else {
        ([0.0; 3], [0.0; 3])
    }
}
