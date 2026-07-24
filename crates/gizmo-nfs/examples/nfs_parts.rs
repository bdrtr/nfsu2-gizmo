//! Lists a car's parts grouped by customization namespace (`BASE` / `KIT##` / `KITW##` /
//! `STYLE##`), so the customizable slots of a given car can be read off before configuring one.
//!
//! Usage: cargo run -p gizmo-nfs --features tools --example nfs_parts -- GEOMETRY.BIN

use gizmo_nfs::parse_geometry;
use std::collections::BTreeMap;

fn num_after(name: &str, tag: &str) -> Option<u32> {
    let i = name.find(tag)? + tag.len();
    let d: String = name[i..].chars().take_while(char::is_ascii_digit).collect();
    d.parse().ok()
}

fn namespace(name: &str) -> String {
    if let Some(n) = num_after(name, "KITW") {
        format!("KITW{n:02}")
    } else if let Some(n) = num_after(name, "STYLE") {
        format!("STYLE{n:02}")
    } else if let Some(n) = num_after(name, "KIT") {
        format!("KIT{n:02}")
    } else if name.contains("_BASE") {
        "BASE".into()
    } else {
        "OTHER".into()
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: nfs_parts GEOMETRY.BIN");
    let bytes = gizmo_nfs::decompress_file(&path).expect("read/decompress");
    let parts = parse_geometry(&bytes).expect("parse GEOMETRY.BIN");

    let mut by_ns: BTreeMap<String, Vec<(&str, usize)>> = BTreeMap::new();
    for p in &parts {
        by_ns.entry(namespace(&p.name)).or_default().push((&p.name, p.indices.len() / 3));
    }
    println!("{} parts in {path}", parts.len());
    for (ns, mut ps) in by_ns {
        ps.sort_unstable();
        println!("\n== {ns}  ({} parts)", ps.len());
        for (name, tris) in ps {
            println!("   {name:<40} {tris:>6} tris");
        }
    }
}
