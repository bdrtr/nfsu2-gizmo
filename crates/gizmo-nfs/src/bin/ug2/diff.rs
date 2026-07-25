//! `ug2 diff` — two asset files, chunk by chunk.
//!
//! The comparison itself is [`gizmo_nfs::diff`]; this prints it. Only the differences by default,
//! because the answer to "what is different about these two cars" should not arrive as seven
//! thousand lines of "same".

use crate::paths::{read, Result};
use gizmo_nfs::diff::{self, Status};
use gizmo_nfs::inspect::type_name;
use std::path::Path;

pub fn run(left: &Path, right: &Path, all: bool, max: usize) -> Result<()> {
    let (a, b) = (read(left)?, read(right)?);
    let ta = gizmo_nfs::chunk::ChunkNode::parse(&a).map_err(|e| format!("{}: {e}", left.display()))?;
    let tb =
        gizmo_nfs::chunk::ChunkNode::parse(&b).map_err(|e| format!("{}: {e}", right.display()))?;
    let report = diff::compare(&ta, &a, &tb, &b);

    outln!("== {} ↔ {} ==", left.display(), right.display());
    let [same, changed, resized, only_left, only_right] = report.tally();
    if report.identical() {
        outln!("identical: {same} chunks, same sizes, same bytes");
        return Ok(());
    }

    let mut shown = 0usize;
    let mut hidden = 0usize;
    for e in report.entries.iter().filter(|e| all || e.status.differs()) {
        if shown == max {
            hidden += 1;
            continue;
        }
        shown += 1;
        let indent = "  ".repeat(e.depth);
        let name = type_name(e.id).unwrap_or("chunk");
        let mark = match e.status {
            Status::Same => " ",
            Status::Changed => "~",
            Status::Resized => "≠",
            Status::OnlyLeft => "-",
            Status::OnlyRight => "+",
            // `Status` is `#[non_exhaustive]`: a state added to the library shows as "?" rather
            // than stopping this tool from compiling.
            _ => "?",
        };
        let sizes = match (e.left, e.right) {
            (Some(l), Some(r)) if l.size != r.size => format!("{} B → {} B", l.size, r.size),
            (Some(l), Some(_)) => format!("{} B", l.size),
            (Some(l), None) => format!("{} B", l.size),
            (None, Some(r)) => format!("{} B", r.size),
            (None, None) => String::new(),
        };
        let where_ = match (e.first_difference, e.differing_bytes) {
            (Some(at), n) => format!("  first difference at +0x{at:X} ({n} bytes differ)"),
            _ => String::new(),
        };
        let at = e.left.or(e.right).map(|s| s.offset).unwrap_or_default();
        outln!("{mark} {indent}{:#010x} {name:<16} @{at:<10} {sizes}{where_}", e.id);
    }
    if hidden > 0 {
        // A cap that quietly truncated would read as "that is all of it".
        outln!("… {hidden} more (raise --max to see them)");
    }
    outln!(
        "{changed} changed · {resized} resized · {only_left} only left · {only_right} only right · {same} same"
    );
    Ok(())
}
