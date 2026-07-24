//! `nfs dump` — the chunk tree of any asset file, or the contents of a VIV archive.

use crate::paths::Result;
use gizmo_nfs::chunk::{dump, DumpOptions};
use gizmo_nfs::compression::{self, Codec};
use gizmo_nfs::viv::VivArchive;
use std::path::Path;

pub fn run(file: &Path, max_depth: u32, hex: usize) -> Result<()> {
    let raw = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let codec = compression::detect(&raw);
    println!("== {} ({} bytes) ==", file.display(), raw.len());
    println!("codec: {codec:?}");

    let bytes = match codec {
        Codec::None => raw,
        _ => match compression::decompress(&raw) {
            Ok(b) => {
                println!("decompressed: {} bytes", b.len());
                b
            }
            Err(e) => {
                println!("(could not decompress: {e}) — reading the raw bytes instead");
                raw
            }
        },
    };

    if bytes.get(..4) == Some(b"BIGF".as_slice()) {
        return list_archive(&bytes);
    }

    println!("chunk tree:");
    let mut out = String::new();
    let opts = DumpOptions { max_depth, hex_leaf_bytes: hex };
    dump(&bytes, &mut out, opts).map_err(|e| format!("{}: {e}", file.display()))?;
    print!("{out}");
    Ok(())
}

fn list_archive(bytes: &[u8]) -> Result<()> {
    let viv = VivArchive::parse(bytes).map_err(|e| format!("BIGF: {e}"))?;
    println!("BIGF archive, {} entries (big_endian_size={}):", viv.entries.len(), viv.big_endian_size);
    for e in viv.iter() {
        let sub = viv.data(e).map(compression::detect).unwrap_or(Codec::None);
        println!("  {:<24} offset={:<10} size={:<10} codec={sub:?}", e.name, e.offset, e.size);
    }
    Ok(())
}
