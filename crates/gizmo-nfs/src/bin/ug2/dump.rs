//! `ug2 dump` — the chunk tree of any asset file, or the contents of a VIV archive.

use crate::paths::Result;
use gizmo_nfs::chunk::{dump, DumpOptions};
use gizmo_nfs::compression::{self, Codec};
use gizmo_nfs::viv::VivArchive;
use std::path::Path;

pub fn run(file: &Path, max_depth: u32, hex: usize) -> Result<()> {
    let raw = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let codec = compression::detect(&raw);
    outln!("== {} ({} bytes) ==", file.display(), raw.len());
    outln!("codec: {codec:?}");

    let bytes = match codec {
        Codec::None => raw,
        _ => match compression::decompress(&raw) {
            Ok(b) => {
                outln!("decompressed: {} bytes", b.len());
                b
            }
            Err(e) => {
                outln!("(could not decompress: {e}) — reading the raw bytes instead");
                raw
            }
        },
    };

    if bytes.get(..4) == Some(b"BIGF".as_slice()) {
        return list_archive(&bytes);
    }

    outln!("chunk tree:");
    let mut out = String::new();
    let opts = DumpOptions { max_depth, hex_leaf_bytes: hex };
    dump(&bytes, &mut out, opts).map_err(|e| format!("{}: {e}", file.display()))?;
    outln!("{}", out.trim_end());
    Ok(())
}

fn list_archive(bytes: &[u8]) -> Result<()> {
    let viv = VivArchive::parse(bytes).map_err(|e| format!("BIGF: {e}"))?;
    outln!("BIGF archive, {} entries (big_endian_size={}):", viv.entries.len(), viv.big_endian_size);
    for e in viv.iter() {
        let sub = viv.data(e).map(compression::detect).unwrap_or(Codec::None);
        outln!("  {:<24} offset={:<10} size={:<10} codec={sub:?}", e.name, e.offset, e.size);
    }
    Ok(())
}
