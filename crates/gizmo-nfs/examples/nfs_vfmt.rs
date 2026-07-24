//! Vertex-format probe: for each renderable solid print its declared vertex/triangle counts, the
//! raw 0x00134B01 vertex-buffer byte length, the implied bytes-per-vertex (vbuf_len / vert_count),
//! and the first N u32 words of the 0x00134900 mesh header (to spot a stride/format field). The
//! parser hard-codes stride 36; a car whose implied bpv is not ~36 is the one that explodes or
//! throws "vertex buffer smaller than count*stride".
//!
//! Usage: cargo run -p gizmo-nfs --features tools --example nfs_vfmt -- GEOMETRY.BIN [name-substr]

use gizmo_nfs::chunk::ChunkNode;
use gizmo_nfs::compression;

const SOLID: u32 = 0x8013_4010;
const SOLID_HEADER: u32 = 0x0013_4011;
const MESH_HEADER: u32 = 0x0013_4900;
const VERTEX_BUFFER: u32 = 0x0013_4B01;
const INDEX_BUFFER: u32 = 0x0013_4B03;

/// Longest run of ASCII-graphic bytes — the same way the parser recovers a part name.
fn longest_ascii(d: &[u8]) -> String {
    let (mut best, mut start) = (0..0, 0usize);
    for i in 0..=d.len() {
        if !d.get(i).is_some_and(|b| b.is_ascii_graphic()) {
            if i - start > best.len() {
                best = start..i;
            }
            start = i + 1;
        }
    }
    String::from_utf8_lossy(&d[best]).into_owned()
}

fn u32_at(d: &[u8], w: usize) -> u32 {
    let b = w * 4;
    if b + 4 > d.len() {
        return 0;
    }
    u32::from_le_bytes([d[b], d[b + 1], d[b + 2], d[b + 3]])
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: nfs_vfmt GEOMETRY.BIN [substr]");
    let filt = std::env::args().nth(2).unwrap_or_default();
    let raw = std::fs::read(&path).expect("read");
    let bytes = match compression::detect(&raw) {
        compression::Codec::None => raw.clone(),
        _ => compression::decompress(&raw).expect("decompress"),
    };
    let roots = ChunkNode::parse(&bytes).expect("chunks");
    println!("{:<32} {:>6} {:>6} {:>8} {:>7}  mesh-header u32[0..16]", "name", "verts", "tris", "vbuf", "bpv");
    for top in &roots {
        for solid in top.find_all(SOLID) {
            let (Some(mesh), Some(vbuf)) = (solid.find(MESH_HEADER), solid.find(VERTEX_BUFFER)) else {
                continue;
            };
            let md = mesh.data(&bytes);
            let tris = u32_at(md, 9) as usize;
            let verts = u32_at(md, 13) as usize;
            let name = solid.find(SOLID_HEADER).map(|h| longest_ascii(h.data(&bytes))).unwrap_or_default();
            if !filt.is_empty() && !name.contains(&filt) {
                continue;
            }
            let vlen = vbuf.data(&bytes).len();
            let bpv = if verts > 0 { vlen as f64 / verts as f64 } else { 0.0 };
            let hdr: Vec<String> = (0..16).map(|w| format!("{}", u32_at(md, w))).collect();
            // The parser's failure condition: it slices the LAST verts*36 bytes, so verts*36 > vbuf
            // throws "vertex buffer smaller than count*stride".
            let flag = if verts * 36 > vlen { " <== verts*36 > vbuf (PARSE FAIL)" } else { "" };
            println!("{name:<28} {verts:>6} {tris:>6} {vlen:>8} {bpv:>7.1}  [{}]{flag}", hdr.join(","));
            let _ = INDEX_BUFFER;
        }
    }
}
