//! `ug2 fields` — what a chunk says, read back as labelled fields.
//!
//! The same model STRUKT's inspector draws ([`gizmo_nfs::inspect`]), printed. Having it in the CLI
//! is not a convenience: it is how the reading gets checked without a window, and it keeps the
//! decoders honest by giving them a second consumer.

use crate::paths::{Car, Result};
use gizmo_nfs::chunk::{ChunkNode, WalkOptions};
use gizmo_nfs::geometry::format;
use gizmo_nfs::inspect::{self, Value};
use std::path::Path;

pub fn run(car: &Path, at: Option<String>, filter: Option<&str>) -> Result<()> {
    let car = Car::resolve(car)?;
    let bytes = crate::paths::read(&car.geometry)?;
    let opts = WalkOptions { stop_on_overrun: true, ..WalkOptions::default() };
    let roots = ChunkNode::parse_with(&bytes, opts).map_err(|e| format!("{}: {e}", car.geometry.display()))?;

    let wanted = at
        .map(|s| {
            let s = s.trim().trim_start_matches("0x");
            usize::from_str_radix(s, 16).map_err(|e| format!("--at {s}: {e}"))
        })
        .transpose()?;

    let mut shown = 0usize;
    for root in &roots {
        walk(root, None, &bytes, wanted, filter, &mut shown);
    }
    if shown == 0 {
        return Err("no chunk matched".into());
    }
    Ok(())
}

/// Depth-first, carrying the solid a chunk belongs to (a vertex buffer's stride needs its
/// sibling's vertex count).
fn walk(
    node: &ChunkNode,
    solid: Option<&ChunkNode>,
    bytes: &[u8],
    wanted: Option<usize>,
    filter: Option<&str>,
    shown: &mut usize,
) {
    let here = if node.header.id == format::SOLID { Some(node) } else { solid };
    let hit = match wanted {
        Some(off) => node.offset == off,
        None => filter.is_none_or(|f| {
            inspect::type_name(node.header.id).is_some_and(|n| n.contains(f))
                || format!("{:#010x}", node.header.id).contains(f)
        }),
    };
    if hit {
        print_model(node, here, bytes);
        *shown += 1;
    }
    for child in &node.children {
        walk(child, here, bytes, wanted, filter, shown);
    }
}

fn print_model(node: &ChunkNode, solid: Option<&ChunkNode>, bytes: &[u8]) {
    let m = inspect::model(node, solid, bytes);
    let name = m.type_name.unwrap_or("chunk");
    outln!(
        "\n== {name}  {:#010x}  @{:#x}  {} B{}",
        node.header.id,
        node.offset,
        node.header.size,
        m.summary.map(|s| format!("  — {s}")).unwrap_or_default()
    );
    for f in &m.fields {
        let at = f.offset.map(|o| format!("{o:#x}")).unwrap_or_else(|| "—".into());
        outln!("  {:<18} {:>10}  {}", f.label, at, render(&f.value));
        if let Some(note) = &f.note {
            outln!("  {:<18} {:>10}  ({note})", "", "");
        }
    }
}

fn render(value: &Value) -> String {
    match value {
        Value::Num(n) => n.to_string(),
        Value::Hex(h) => format!("{h:#010x}"),
        Value::Ratio { num, den, value } => format!("{num} / {den} = {value:.1}"),
        Value::Text(s) => s.clone(),
        Value::Float(f) => format!("{f:.4}"),
        Value::Float3(v) => format!("{:.3}, {:.3}, {:.3}", v[0], v[1], v[2]),
        Value::Bytes(b) => b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" "),
        Value::Matrix(m) => m
            .iter()
            .map(|r| r.iter().map(|v| format!("{v:.2}")).collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(" | "),
        _ => String::new(),
    }
}
