//! `ug2 parts` — what a car ships, and what a configuration of it selects.

use crate::paths::{Car, Result};
use gizmo_nfs::parts::{group_of, select_car, CarConfig};
use gizmo_nfs::NfsMeshPart;
use std::collections::BTreeMap;
use std::path::Path;

pub fn run(car: &Path, selected: bool, config: CarConfig) -> Result<()> {
    let car = Car::resolve(car)?;
    let parts = car.parts()?;
    if selected {
        list_selected(&parts, config);
        return Ok(());
    }
    list_by_namespace(&parts);
    Ok(())
}

/// Every part, grouped by the customization namespace its name carries.
fn list_by_namespace(parts: &[NfsMeshPart]) {
    let mut by_ns: BTreeMap<String, Vec<(&str, usize)>> = BTreeMap::new();
    for p in parts {
        by_ns.entry(namespace_label(&p.name)).or_default().push((&p.name, p.triangle_count()));
    }
    outln!("{} parts", parts.len());
    for (ns, mut ps) in by_ns {
        ps.sort_unstable();
        outln!("\n== {ns}  ({} parts)", ps.len());
        for (name, tris) in ps {
            outln!("   {name:<40} {tris:>6} tris");
        }
    }
}

/// Only the parts a configuration selects, with the material group each renders as.
fn list_selected(parts: &[NfsMeshPart], config: CarConfig) {
    let mut sel = select_car(parts, &config);
    sel.sort_by(|a, b| a.name.cmp(&b.name));
    outln!("{} parts selected for {config:?}\n", sel.len());
    for p in sel {
        outln!("   {:<40} {:>6} tris  {:?}", p.name, p.triangle_count(), group_of(&p.name));
    }
}

/// The namespace token a name carries, rendered for display (`KIT03`, `KITW01`, `STYLE07`,
/// `BASE`, or `—` for the parts that carry none).
fn namespace_label(name: &str) -> String {
    for (tag, fmt) in [("KITW", "KITW"), ("STYLE", "STYLE"), ("KIT", "KIT")] {
        if let Some(n) = num_after(name, tag) {
            return format!("{fmt}{n:02}");
        }
    }
    if name.contains("_BASE") {
        "BASE".into()
    } else {
        "—".into()
    }
}

fn num_after(name: &str, tag: &str) -> Option<u32> {
    let i = name.find(tag)? + tag.len();
    let digits: String = name[i..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}
