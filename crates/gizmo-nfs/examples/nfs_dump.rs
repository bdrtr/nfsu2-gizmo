//! `nfs_dump` — a read-only reverse-engineering aid for NFSU2 asset files.
//!
//! Usage:
//! ```text
//! cargo run -p gizmo-nfs --features tools --example nfs_dump -- <FILE> [--hex N] [--max-depth D]
//! ```
//!
//! It detects the compression codec, decompresses if needed, and then either lists a
//! BIGF/VIV archive's contents or prints the chunk tree. This is the tool used to lock
//! the unconfirmed NFSU2 formats (TPK texture chunk IDs, GEOMETRY.BIN vertex layout)
//! against real, legally-owned game files.

use gizmo_nfs::chunk::{self, DumpOptions};
use gizmo_nfs::compression::{self, Codec};
use gizmo_nfs::viv::VivArchive;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = match args.next() {
        Some(p) if !p.starts_with("--") => p,
        _ => {
            eprintln!("usage: nfs_dump <FILE> [--hex N] [--max-depth D]");
            std::process::exit(2);
        }
    };

    let mut hex = 16usize;
    let mut max_depth = 64u32;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--hex" => hex = args.next().and_then(|v| v.parse().ok()).unwrap_or(hex),
            "--max-depth" => max_depth = args.next().and_then(|v| v.parse().ok()).unwrap_or(max_depth),
            other => eprintln!("(ignoring unknown flag {other})"),
        }
    }

    let raw = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            std::process::exit(1);
        }
    };

    println!("== {path} ({} bytes) ==", raw.len());
    let codec = compression::detect(&raw);
    println!("codec: {codec:?}");

    // Decompress if we can; otherwise work on the raw bytes.
    let bytes = match codec {
        Codec::None => raw.clone(),
        _ => match compression::decompress(&raw) {
            Ok(b) => {
                println!("decompressed: {} bytes", b.len());
                b
            }
            Err(e) => {
                println!("(could not decompress: {e}) — dumping raw bytes instead");
                raw.clone()
            }
        },
    };

    // If it's a BIGF archive, list its table of contents.
    if bytes.get(..4) == Some(b"BIGF".as_slice()) {
        match VivArchive::parse(&bytes) {
            Ok(viv) => {
                println!("BIGF archive, {} entries (big_endian_size={}):", viv.entries.len(), viv.big_endian_size);
                for e in viv.iter() {
                    let sub = viv.data(e).map(compression::detect).unwrap_or(Codec::None);
                    println!("  {:<20} offset={:<10} size={:<10} codec={:?}", e.name, e.offset, e.size, sub);
                }
            }
            Err(e) => eprintln!("error parsing BIGF: {e}"),
        }
        return;
    }

    // Otherwise dump the chunk tree.
    println!("chunk tree:");
    let mut out = String::new();
    let opts = DumpOptions { max_depth, hex_leaf_bytes: hex };
    match chunk::dump(&bytes, &mut out, opts) {
        Ok(()) => print!("{out}"),
        Err(e) => eprintln!("error dumping chunks: {e}"),
    }
}
