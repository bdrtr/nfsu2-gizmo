//! Checking a file against what the format says it should be.
//!
//! The parser's job is to read; this module's job is to doubt. It runs the checks a person would
//! run by hand after decoding something — is the stride a plausible number, is the car a few
//! metres long, are the normals unit length, do the indices stay inside the vertex list, does
//! every chunk fit inside its parent — and reports each one with the chunk it is about.
//!
//! Two design rules, both learned the hard way on this project:
//!
//! * **A rule records what it examined.** "No findings" and "nobody looked" are different answers,
//!   and a viewer that shows a green tick on a chunk no rule ever read is teaching the user to
//!   trust something it does not know. [`RuleResult::examined`] is what keeps that honest.
//! * **Checks read the raw buffers**, through the same offset map the parser uses, rather than
//!   the parsed output. A solid the parser *refuses* (a packed vertex layout) is exactly the one
//!   worth flagging, and it never appears in `parse_geometry`'s result.

use crate::chunk::ChunkNode;
use crate::geometry::{format, mesh_field, skip_leading_filler, VERTEX_STRIDE};

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// A rule's stable identifier — the key a UI translates and a test asserts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleId(pub &'static str);

/// One check, and the expression that makes it checkable by hand.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Rule {
    pub id: RuleId,
    /// Short English title; a UI may translate it by `id`.
    pub title: &'static str,
    /// The arithmetic the rule applies, shown verbatim so a reader can redo it.
    pub expr: &'static str,
}

/// Every rule, in the order a report presents them.
pub static RULES: &[Rule] = &[
    Rule {
        id: RuleId("stride"),
        title: "Stride sanity",
        expr: "vertex_buffer_size / vertex_count ≈ 36",
    },
    Rule { id: RuleId("bbox"), title: "Bounding box", expr: "a few metres per axis" },
    Rule { id: RuleId("normals"), title: "Normals", expr: "|n| ≈ 1" },
    Rule { id: RuleId("index_range"), title: "Index range", expr: "max_index < vertex_count" },
    Rule { id: RuleId("bounds"), title: "Chunk size", expr: "offset + 8 + size ≤ parent end" },
];

/// One thing a rule noticed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Finding {
    pub rule: RuleId,
    pub severity: Severity,
    /// Header offset of the chunk this is about — the key a viewer selects it by.
    pub chunk_offset: usize,
    pub chunk_id: u32,
    /// The part name when there is one, else the chunk's own label.
    pub subject: String,
    /// What was found, in English, with the numbers in it.
    pub message: String,
}

/// What one rule concluded.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RuleResult {
    pub rule: RuleId,
    pub findings: Vec<Finding>,
    /// How many chunks this rule actually read. Zero means the rule had nothing to say *because
    /// it looked at nothing* — which is not the same as a pass.
    pub examined: usize,
}

impl RuleResult {
    /// The worst severity among the findings, or `None` when the rule looked and found nothing.
    #[must_use]
    pub fn status(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.severity).max()
    }
}

/// What a viewer shows against one chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[non_exhaustive]
pub enum ChunkStatus {
    /// No rule read this chunk. Deliberately not a tick.
    #[default]
    Unchecked,
    Ok,
    Warn,
    Error,
}

/// Everything the validation pass concluded.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Report {
    pub results: Vec<RuleResult>,
    /// Per-chunk status, sorted by offset.
    by_chunk: Vec<(usize, ChunkStatus)>,
}

impl Report {
    /// The worst severity anywhere in the file.
    #[must_use]
    pub fn worst(&self) -> Option<Severity> {
        self.results.iter().filter_map(RuleResult::status).max()
    }

    /// Every finding, in rule order.
    pub fn findings(&self) -> impl Iterator<Item = &Finding> {
        self.results.iter().flat_map(|r| r.findings.iter())
    }

    /// What to show against the chunk whose header sits at `offset`.
    #[must_use]
    pub fn status_of(&self, offset: usize) -> ChunkStatus {
        self.by_chunk
            .binary_search_by_key(&offset, |(o, _)| *o)
            .map_or(ChunkStatus::Unchecked, |i| self.by_chunk[i].1)
    }
}

/// Run every rule over a parsed chunk tree.
///
/// Takes the tree rather than the bytes alone so a caller that already walked the file (a viewer,
/// which needs the tree anyway) does not walk it twice.
#[must_use]
pub fn validate(roots: &[ChunkNode], bytes: &[u8]) -> Report {
    let mut results = Vec::new();
    let solids = collect(roots, format::SOLID);
    results.push(stride_rule(&solids, bytes));
    results.push(bbox_rule(&solids, bytes));
    results.push(normals_rule(&solids, bytes));
    results.push(index_rule(&solids, bytes));
    results.push(bounds_rule(roots, bytes));

    // Per-chunk status: a chunk a rule examined is `Ok` unless a finding names it.
    let mut by_chunk: std::collections::BTreeMap<usize, ChunkStatus> = std::collections::BTreeMap::new();
    for r in &results {
        for f in &r.findings {
            let status = match f.severity {
                Severity::Error => ChunkStatus::Error,
                Severity::Warn => ChunkStatus::Warn,
                Severity::Info => ChunkStatus::Ok,
            };
            let slot = by_chunk.entry(f.chunk_offset).or_insert(status);
            *slot = (*slot).max(status);
        }
    }
    for node in &solids {
        by_chunk.entry(node.offset).or_insert(ChunkStatus::Ok);
    }
    Report { results, by_chunk: by_chunk.into_iter().collect() }
}

/// Every descendant with `id`, roots included.
fn collect(roots: &[ChunkNode], id: u32) -> Vec<&ChunkNode> {
    let mut out = Vec::new();
    for r in roots {
        if r.header.id == id {
            out.push(r);
        }
        out.extend(r.find_all(id));
    }
    out
}

/// What a solid declares: its name, vertex/triangle counts, and its buffers.
struct Solid<'a> {
    node: &'a ChunkNode,
    name: String,
    verts: usize,
    tris: usize,
    vbuf: Option<&'a ChunkNode>,
    ibuf: Option<&'a ChunkNode>,
}

fn read_solid<'a>(node: &'a ChunkNode, bytes: &[u8]) -> Option<Solid<'a>> {
    let header = node.find(format::MESH_HEADER)?;
    let body = skip_leading_filler(header.data(bytes));
    let tris = mesh_field(body, format::MESH_TRI_COUNT_FIELD).ok()? as usize;
    let verts = mesh_field(body, format::MESH_VERT_COUNT_FIELD).ok()? as usize;
    let name = node
        .find(format::SOLID_HEADER)
        .map(|h| crate::geometry::part_name(h.data(bytes)))
        .unwrap_or_default();
    Some(Solid {
        node,
        name,
        verts,
        tris,
        vbuf: node.find(format::VERTEX_BUFFER),
        ibuf: node.find(format::INDEX_BUFFER),
    })
}

fn subject(s: &Solid<'_>) -> String {
    if s.name.is_empty() {
        format!("{:#010x}", s.node.header.id)
    } else {
        s.name.clone()
    }
}

/// `stride` — the vertex buffer must hold `vertex_count` records of the layout this crate decodes.
///
/// This is the check that finds the solids the parser silently refuses: `3000GT_KIT00_ENGINE_B`
/// declares 318 vertices in 7 700 bytes, which is ~24 bytes each, not 36.
fn stride_rule(solids: &[&ChunkNode], bytes: &[u8]) -> RuleResult {
    let mut findings = Vec::new();
    let mut examined = 0;
    for node in solids {
        let Some(s) = read_solid(node, bytes) else { continue };
        let Some(vbuf) = s.vbuf else { continue };
        examined += 1;
        let len = vbuf.data(bytes).len();
        if s.verts == 0 {
            continue;
        }
        let needed = s.verts.saturating_mul(VERTEX_STRIDE);
        if needed > len {
            findings.push(Finding {
                rule: RuleId("stride"),
                severity: Severity::Error,
                chunk_offset: node.offset,
                chunk_id: node.header.id,
                subject: subject(&s),
                message: format!(
                    "{} × {VERTEX_STRIDE} = {needed} B needed, buffer holds {len} B ({:.1} B/vertex) — \
                     a packed layout this crate does not decode, so the solid is skipped",
                    s.verts,
                    len as f64 / s.verts as f64
                ),
            });
        } else if len - needed > len / 2 {
            // Leading alignment padding is normal and routinely runs to a hundred bytes or more —
            // the parser reads the *last* `count × stride` bytes precisely because of it, so
            // flagging ordinary slack would bury the real findings under hundreds of false ones
            // (3000GT alone produced 392). Only complain when the declared count explains less
            // than half the buffer, which means the count and the buffer disagree about content,
            // not about padding.
            findings.push(Finding {
                rule: RuleId("stride"),
                severity: Severity::Warn,
                chunk_offset: node.offset,
                chunk_id: node.header.id,
                subject: subject(&s),
                message: format!(
                    "{} vertices explain only {needed} of {len} B — the count and the buffer disagree",
                    s.verts
                ),
            });
        }
    }
    RuleResult { rule: RuleId("stride"), findings, examined }
}

/// `bbox` — a car part is a few metres across, not a few kilometres or a few microns.
fn bbox_rule(solids: &[&ChunkNode], bytes: &[u8]) -> RuleResult {
    let mut findings = Vec::new();
    let mut examined = 0;
    for node in solids {
        let Some(s) = read_solid(node, bytes) else { continue };
        let Some(vbuf) = s.vbuf else { continue };
        let Some(bounds) = positions_bounds(vbuf.data(bytes), s.verts) else { continue };
        examined += 1;
        let (lo, hi) = bounds;
        let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        if ext.iter().any(|e| !e.is_finite()) {
            findings.push(Finding {
                rule: RuleId("bbox"),
                severity: Severity::Error,
                chunk_offset: node.offset,
                chunk_id: node.header.id,
                subject: subject(&s),
                message: "non-finite vertex positions".into(),
            });
        } else if ext.iter().any(|e| *e > 30.0) {
            findings.push(Finding {
                rule: RuleId("bbox"),
                severity: Severity::Warn,
                chunk_offset: node.offset,
                chunk_id: node.header.id,
                subject: subject(&s),
                message: format!(
                    "{:.2} × {:.2} × {:.2} m — larger than any car part, so the layout is probably \
                     being read wrong",
                    ext[0], ext[1], ext[2]
                ),
            });
        }
    }
    RuleResult { rule: RuleId("bbox"), findings, examined }
}

/// `normals` — sampled, not exhaustive: a few thousand vertices answer the question, and a whole
/// 8 MB file's worth would be paid for on every open.
fn normals_rule(solids: &[&ChunkNode], bytes: &[u8]) -> RuleResult {
    const SAMPLES: usize = 64;
    let mut findings = Vec::new();
    let mut examined = 0;
    for node in solids {
        let Some(s) = read_solid(node, bytes) else { continue };
        let Some(vbuf) = s.vbuf else { continue };
        let data = vbuf.data(bytes);
        if s.verts == 0 || s.verts * VERTEX_STRIDE > data.len() {
            continue;
        }
        examined += 1;
        let start = data.len() - s.verts * VERTEX_STRIDE;
        let step = (s.verts / SAMPLES).max(1);
        let mut checked = 0usize;
        let mut bad = 0usize;
        for i in (0..s.verts).step_by(step) {
            let base = start + i * VERTEX_STRIDE + 12; // position[3] then normal[3]
            let Some(n) = read_f32x3(data, base) else { continue };
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            checked += 1;
            if !(0.99..=1.01).contains(&len) {
                bad += 1;
            }
        }
        if bad > 0 {
            findings.push(Finding {
                rule: RuleId("normals"),
                severity: Severity::Warn,
                chunk_offset: node.offset,
                chunk_id: node.header.id,
                subject: subject(&s),
                message: format!("{bad} of {checked} sampled normals are not unit length"),
            });
        }
    }
    RuleResult { rule: RuleId("normals"), findings, examined }
}

/// `index_range` — every index must name a vertex that exists.
///
/// Read from the raw buffer rather than the parsed part: an out-of-range index makes
/// `parse_geometry` reject the whole file, so by the time there is a parsed part there is nothing
/// left to check.
fn index_rule(solids: &[&ChunkNode], bytes: &[u8]) -> RuleResult {
    let mut findings = Vec::new();
    let mut examined = 0;
    for node in solids {
        let Some(s) = read_solid(node, bytes) else { continue };
        let Some(ibuf) = s.ibuf else { continue };
        let data = ibuf.data(bytes);
        let start = leading_filler(data);
        let want = s.tris.saturating_mul(6);
        if s.verts == 0 || want == 0 || start + want > data.len() {
            continue;
        }
        examined += 1;
        let max = data[start..start + want]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            .max()
            .unwrap_or(0);
        if max >= s.verts {
            findings.push(Finding {
                rule: RuleId("index_range"),
                severity: Severity::Error,
                chunk_offset: node.offset,
                chunk_id: node.header.id,
                subject: subject(&s),
                message: format!("index {max} names a vertex outside the {} in the buffer", s.verts),
            });
        }
    }
    RuleResult { rule: RuleId("index_range"), findings, examined }
}

/// `bounds` — every chunk must fit inside its parent, and the file should be covered by its
/// top-level chunks.
///
/// The walk that produced the tree is tolerant on purpose (a viewer must still open a broken
/// file); this rule is where what it swallowed gets said out loud.
fn bounds_rule(roots: &[ChunkNode], bytes: &[u8]) -> RuleResult {
    let mut findings = Vec::new();
    let mut examined = 0;
    fn walk(nodes: &[ChunkNode], parent_end: usize, findings: &mut Vec<Finding>, examined: &mut usize) {
        for n in nodes {
            *examined += 1;
            let end = n.data_offset + n.header.size as usize;
            if end > parent_end {
                findings.push(Finding {
                    rule: RuleId("bounds"),
                    severity: Severity::Error,
                    chunk_offset: n.offset,
                    chunk_id: n.header.id,
                    subject: format!("{:#010x}", n.header.id),
                    message: format!("ends at {end:#x}, past its parent's {parent_end:#x}"),
                });
            }
            walk(&n.children, end, findings, examined);
        }
    }
    walk(roots, bytes.len(), &mut findings, &mut examined);

    // Bytes after the last top-level chunk are what a tolerant walk quietly stopped at.
    if let Some(last) = roots.last() {
        let end = last.data_offset + last.header.size as usize;
        let tail = bytes.len().saturating_sub(end);
        if tail >= 8 {
            findings.push(Finding {
                rule: RuleId("bounds"),
                severity: Severity::Warn,
                chunk_offset: end.min(bytes.len().saturating_sub(1)),
                chunk_id: 0,
                subject: "tail".into(),
                message: format!("{tail} B after the last chunk are not part of the tree"),
            });
        }
    }
    RuleResult { rule: RuleId("bounds"), findings, examined }
}

/// Bounds of the positions in a vertex buffer, or `None` when the layout does not fit.
fn positions_bounds(data: &[u8], verts: usize) -> Option<([f32; 3], [f32; 3])> {
    if verts == 0 || verts * VERTEX_STRIDE > data.len() {
        return None;
    }
    let start = data.len() - verts * VERTEX_STRIDE;
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for i in 0..verts {
        let p = read_f32x3(data, start + i * VERTEX_STRIDE)?;
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    Some((lo, hi))
}

fn read_f32x3(data: &[u8], at: usize) -> Option<[f32; 3]> {
    let s = data.get(at..at + 12)?;
    let f = |i: usize| f32::from_le_bytes([s[i], s[i + 1], s[i + 2], s[i + 3]]);
    Some([f(0), f(4), f(8)])
}

fn leading_filler(data: &[u8]) -> usize {
    data.iter().take_while(|&&b| b == format::FILLER_BYTE).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    /// A solid with the given counts and buffers, as a byte stream.
    fn solid(verts: u32, tris: u32, vbuf: Vec<u8>, ibuf: Vec<u8>) -> Vec<u8> {
        let mut header = vec![0u8; 16 * 4];
        header[format::MESH_TRI_COUNT_FIELD * 4..][..4].copy_from_slice(&tris.to_le_bytes());
        header[format::MESH_VERT_COUNT_FIELD * 4..][..4].copy_from_slice(&verts.to_le_bytes());
        let mut inner = chunk(format::MESH_HEADER, &header);
        inner.extend(chunk(format::VERTEX_BUFFER, &vbuf));
        inner.extend(chunk(format::INDEX_BUFFER, &ibuf));
        chunk(format::SOLID, &inner)
    }

    /// `count` vertices whose position is `p` and whose normal is `n`.
    fn vertices(count: usize, p: [f32; 3], n: [f32; 3]) -> Vec<u8> {
        let mut v = Vec::new();
        for _ in 0..count {
            for f in [p[0], p[1], p[2], n[0], n[1], n[2], -1.0e38, 0.0, 0.0] {
                v.extend_from_slice(&f.to_le_bytes());
            }
        }
        v
    }

    fn report_of(bytes: &[u8]) -> Report {
        let roots = ChunkNode::parse(bytes).unwrap();
        validate(&roots, bytes)
    }

    fn result<'a>(r: &'a Report, id: &str) -> &'a RuleResult {
        r.results.iter().find(|x| x.rule.0 == id).unwrap()
    }

    #[test]
    fn a_packed_vertex_layout_is_an_error_the_parser_never_reports() {
        // 4 vertices declared, but only 3 vertices' worth of bytes: the shape of
        // 3000GT_KIT00_ENGINE_B, which `parse_geometry` skips without a word.
        let bytes = solid(4, 1, vertices(3, [0.0; 3], [0.0, 1.0, 0.0]), vec![0u8; 6]);
        let report = report_of(&bytes);
        let stride = result(&report, "stride");
        assert_eq!(stride.examined, 1);
        assert_eq!(stride.findings.len(), 1);
        assert_eq!(stride.findings[0].severity, Severity::Error);
        assert!(stride.findings[0].message.contains("packed layout"));
    }

    #[test]
    fn ordinary_leading_padding_is_not_a_finding() {
        // Real vertex buffers carry alignment padding in front of the records — 36 to 116 bytes
        // is routine. An earlier version of this rule called that "slack" and produced 392
        // warnings on one car, which is how a validator teaches people to ignore it.
        let mut vbuf = vec![0x11u8; 116];
        vbuf.extend(vertices(4, [0.5, 0.0, 0.0], [0.0, 1.0, 0.0]));
        let bytes = solid(4, 1, vbuf, vec![0u8; 6]);
        let report = report_of(&bytes);
        assert_eq!(result(&report, "stride").findings.len(), 0);
    }

    #[test]
    fn a_count_that_explains_less_than_half_the_buffer_is_flagged() {
        let mut vbuf = vec![0u8; 400];
        vbuf.extend(vertices(2, [0.0; 3], [0.0, 1.0, 0.0]));
        let bytes = solid(2, 1, vbuf, vec![0u8; 6]);
        let report = report_of(&bytes);
        let stride = result(&report, "stride");
        assert_eq!(stride.findings.len(), 1);
        assert_eq!(stride.findings[0].severity, Severity::Warn);
    }

    #[test]
    fn a_clean_solid_produces_no_findings_but_is_still_marked_examined() {
        let bytes = solid(4, 1, vertices(4, [0.5, 0.0, 0.0], [0.0, 1.0, 0.0]), vec![0u8; 6]);
        let report = report_of(&bytes);
        for id in ["stride", "bbox", "normals", "index_range"] {
            let r = result(&report, id);
            assert_eq!(r.findings.len(), 0, "{id} should be clean");
            assert!(r.examined > 0, "{id} must record that it looked");
        }
        assert_eq!(report.worst(), None);
    }

    #[test]
    fn an_index_past_the_vertex_list_is_caught_in_the_raw_buffer() {
        // Index 9 with only 4 vertices. `parse_geometry` rejects the whole file for this, so the
        // rule has to read the buffer itself or it could never report it per-solid.
        let mut ibuf = Vec::new();
        for i in [0u16, 1, 9] {
            ibuf.extend_from_slice(&i.to_le_bytes());
        }
        let bytes = solid(4, 1, vertices(4, [0.0; 3], [0.0, 1.0, 0.0]), ibuf);
        let report = report_of(&bytes);
        let idx = result(&report, "index_range");
        assert_eq!(idx.findings.len(), 1);
        assert_eq!(idx.findings[0].severity, Severity::Error);
        assert_eq!(report.worst(), Some(Severity::Error));
    }

    #[test]
    fn normals_that_are_not_unit_length_are_flagged() {
        let bytes = solid(4, 1, vertices(4, [0.0; 3], [0.0, 3.0, 0.0]), vec![0u8; 6]);
        let report = report_of(&bytes);
        assert_eq!(result(&report, "normals").findings.len(), 1);
    }

    #[test]
    fn a_part_the_size_of_a_street_is_flagged() {
        let mut vbuf = vertices(1, [0.0; 3], [0.0, 1.0, 0.0]);
        vbuf.extend(vertices(1, [90.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
        let bytes = solid(2, 1, vbuf, vec![0u8; 6]);
        let report = report_of(&bytes);
        let bbox = result(&report, "bbox");
        assert_eq!(bbox.findings.len(), 1);
        assert!(bbox.findings[0].message.contains("larger than any car part"));
    }

    #[test]
    fn per_chunk_status_distinguishes_unchecked_from_clean() {
        let bytes = solid(4, 1, vertices(4, [0.0; 3], [0.0, 1.0, 0.0]), vec![0u8; 6]);
        let report = report_of(&bytes);
        // The solid was read by four rules → clean.
        assert_eq!(report.status_of(0), ChunkStatus::Ok);
        // A chunk no rule looked at must not wear a tick.
        assert_eq!(report.status_of(0xDEAD), ChunkStatus::Unchecked);
    }

    #[test]
    fn every_rule_appears_in_the_report_even_with_nothing_to_read() {
        let report = report_of(&chunk(0x0000_0002, &[1, 2, 3]));
        assert_eq!(report.results.len(), RULES.len());
        for r in &report.results {
            assert!(RULES.iter().any(|rule| rule.id == r.rule));
        }
    }
}
