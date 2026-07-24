//! Dump the wheel mounts, radius and mass of every car in a `GLOBALB.BUN`.
//!
//! Usage: cargo run -p gizmo-nfs --features tools --example nfs_globalb -- GLOBALB.BUN [CAR]

use gizmo_nfs::globalb::parse_cartypeinfos;

fn main() {
    let path = std::env::args().nth(1).expect("usage: nfs_globalb GLOBALB.BUN [CAR]");
    let filter = std::env::args().nth(2);
    let raw = std::fs::read(&path).expect("read");
    let bytes = match gizmo_nfs::compression::detect(&raw) {
        gizmo_nfs::compression::Codec::None => raw,
        _ => gizmo_nfs::compression::decompress(&raw).expect("decompress"),
    };
    let cars = parse_cartypeinfos(&bytes);
    println!("{} CarTypeInfo records\n", cars.len());
    for c in cars.iter().filter(|c| filter.as_deref().is_none_or(|f| c.name.contains(f))) {
        let fl = c.wheels[0];
        let rr = c.wheels[2];
        let wheelbase = (fl.fore_aft - rr.fore_aft).abs();
        let track = fl.lateral.abs() * 2.0;
        println!(
            "{:<13} wheelbase={:.2}m track={:.2}m r={:.3}m mass={:.0}kg  [FL fa={:+.2} lat={:+.2} rh={:+.2}]",
            c.name, wheelbase, track, fl.radius, c.mass_kg, fl.fore_aft, fl.lateral, fl.ride_height
        );
    }
}
