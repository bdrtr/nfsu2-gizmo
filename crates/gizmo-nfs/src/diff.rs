//! Two files, chunk by chunk: what is only in one, and what is in both but different.
//!
//! The question this answers is the modder's one — *why does this file work and that one not* —
//! and the honest answer is usually structural: a chunk that is missing, a buffer that grew, a
//! header word that changed. So the comparison is over the chunk tree rather than over the bytes:
//! a byte diff of two `GEOMETRY.BIN`s is a wall of noise, because every offset after the first
//! change has shifted.
//!
//! **How chunks are paired.** By position among siblings that share an id: the *n*-th `0x00134011`
//! on the left is compared with the *n*-th on the right. This format's trees are ordered — a car's
//! solids are a list — and any cleverer pairing (by name, by size) would silently re-order parts
//! and report differences that are really a mismatch of the tool's own making. When the two sides
//! hold different numbers of a chunk, the extras are reported as present on one side only.

use crate::chunk::ChunkNode;

/// What a comparison made of one chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Status {
    /// Same size, same bytes, and every descendant the same.
    Same,
    /// Same size, different bytes — the interesting case: a field was edited in place.
    Changed,
    /// Different payload size.
    Resized,
    /// Present on the left only.
    OnlyLeft,
    /// Present on the right only.
    OnlyRight,
}

impl Status {
    /// Whether this is a difference at all.
    #[must_use]
    pub fn differs(self) -> bool {
        self != Status::Same
    }
}

/// Where a chunk sits in one of the two files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Side {
    /// Absolute offset of the chunk header — the same key selection uses.
    pub offset: usize,
    /// Payload size.
    pub size: u32,
}

/// One row of a comparison: a chunk, and what became of it.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Entry {
    /// Depth in the tree, for indentation.
    pub depth: usize,
    pub id: u32,
    pub status: Status,
    pub left: Option<Side>,
    pub right: Option<Side>,
    /// For a leaf whose bytes differ: the first differing byte, relative to the payload. The
    /// number that turns "this chunk changed" into somewhere to look.
    pub first_difference: Option<usize>,
    /// Differing bytes, for a leaf that kept its size.
    pub differing_bytes: usize,
}

/// The comparison as a whole.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Report {
    /// Every chunk of both files, in the left file's order, with the right-only ones interleaved
    /// where they were found.
    pub entries: Vec<Entry>,
}

impl Report {
    /// How many chunks fall in each state: (same, changed, resized, only left, only right).
    #[must_use]
    pub fn tally(&self) -> [usize; 5] {
        let mut counts = [0usize; 5];
        for e in &self.entries {
            let i = match e.status {
                Status::Same => 0,
                Status::Changed => 1,
                Status::Resized => 2,
                Status::OnlyLeft => 3,
                Status::OnlyRight => 4,
            };
            counts[i] += 1;
        }
        counts
    }

    /// Whether the two files are the same tree with the same bytes.
    #[must_use]
    pub fn identical(&self) -> bool {
        self.entries.iter().all(|e| e.status == Status::Same)
    }
}

/// Compare two parsed files.
///
/// Each tree must have been parsed from the buffer passed beside it; the payloads are read back
/// through those buffers.
#[must_use]
pub fn compare(
    left: &[ChunkNode],
    left_bytes: &[u8],
    right: &[ChunkNode],
    right_bytes: &[u8],
) -> Report {
    let mut report = Report::default();
    walk(left, left_bytes, right, right_bytes, 0, &mut report);
    report
}

fn walk(
    left: &[ChunkNode],
    left_bytes: &[u8],
    right: &[ChunkNode],
    right_bytes: &[u8],
    depth: usize,
    report: &mut Report,
) {
    // Pair by position among siblings of the same id, so a chunk that appears three times is
    // compared first-to-first rather than to whichever sibling happens to match.
    let mut used = vec![false; right.len()];
    for node in left {
        let mate = right
            .iter()
            .enumerate()
            .find(|(i, r)| !used[*i] && r.header.id == node.header.id)
            .map(|(i, r)| {
                used[i] = true;
                r
            });
        match mate {
            Some(mate) => compare_pair(node, left_bytes, mate, right_bytes, depth, report),
            None => {
                report.entries.push(entry(Status::OnlyLeft, node, None, depth));
                // A chunk that exists on one side only still shows its shape: an added solid is
                // more readable as a subtree than as one line saying "something appeared".
                only(&node.children, depth + 1, Status::OnlyLeft, report);
            }
        }
    }
    for (i, node) in right.iter().enumerate() {
        if !used[i] {
            report.entries.push(entry(Status::OnlyRight, node, Some(Side::of(node)), depth));
            only(&node.children, depth + 1, Status::OnlyRight, report);
        }
    }
}

fn compare_pair(
    left: &ChunkNode,
    left_bytes: &[u8],
    right: &ChunkNode,
    right_bytes: &[u8],
    depth: usize,
    report: &mut Report,
) {
    let index = report.entries.len();
    let mut e = Entry {
        depth,
        id: left.header.id,
        status: Status::Same,
        left: Some(Side::of(left)),
        right: Some(Side::of(right)),
        first_difference: None,
        differing_bytes: 0,
    };
    if left.header.size != right.header.size {
        e.status = Status::Resized;
    }
    let has_children = !left.children.is_empty() || !right.children.is_empty();
    if !has_children && e.status == Status::Same {
        let (a, b) = (left.data(left_bytes), right.data(right_bytes));
        let differing = a.iter().zip(b).filter(|(x, y)| x != y).count();
        if differing > 0 {
            e.status = Status::Changed;
            e.differing_bytes = differing;
            e.first_difference = a.iter().zip(b).position(|(x, y)| x != y);
        }
    }
    report.entries.push(e);

    if has_children {
        let before = report.entries.len();
        walk(&left.children, left_bytes, &right.children, right_bytes, depth + 1, report);
        // A container is different exactly when something inside it is. Said after the children
        // are compared, because that is when it is known — and not said by comparing the
        // container's own bytes, which would report every ancestor of one edit as changed.
        if report.entries[before..].iter().any(|c| c.status.differs()) && !e_differs(report, index)
        {
            report.entries[index].status = Status::Changed;
        }
    }
}

fn e_differs(report: &Report, index: usize) -> bool {
    report.entries.get(index).is_some_and(|e| e.status.differs())
}

/// Record a subtree that exists on one side only.
fn only(nodes: &[ChunkNode], depth: usize, status: Status, report: &mut Report) {
    for n in nodes {
        let side = Some(Side::of(n));
        let (left, right) = if status == Status::OnlyLeft { (side, None) } else { (None, side) };
        report.entries.push(Entry {
            depth,
            id: n.header.id,
            status,
            left,
            right,
            first_difference: None,
            differing_bytes: 0,
        });
        only(&n.children, depth + 1, status, report);
    }
}

fn entry(status: Status, node: &ChunkNode, right: Option<Side>, depth: usize) -> Entry {
    Entry {
        depth,
        id: node.header.id,
        status,
        left: (status == Status::OnlyLeft).then(|| Side::of(node)),
        right,
        first_difference: None,
        differing_bytes: 0,
    }
}

impl Side {
    fn of(node: &ChunkNode) -> Self {
        Self { offset: node.offset, size: node.header.size }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::tests::chunk;
    use crate::chunk::CONTAINER_FLAG;

    fn tree(bytes: &[u8]) -> Vec<ChunkNode> {
        ChunkNode::parse(bytes).expect("synthetic chunk stream")
    }

    #[test]
    fn a_file_compared_with_itself_is_identical() {
        let bytes = chunk(0x0000_0001, &[1, 2, 3, 4]);
        let report = compare(&tree(&bytes), &bytes, &tree(&bytes), &bytes);
        assert!(report.identical());
        assert_eq!(report.tally(), [1, 0, 0, 0, 0]);
    }

    #[test]
    fn an_edited_field_is_a_change_with_the_offset_it_happened_at() {
        let a = chunk(0x0000_0001, &[1, 2, 3, 4]);
        let b = chunk(0x0000_0001, &[1, 2, 9, 4]);
        let report = compare(&tree(&a), &a, &tree(&b), &b);
        let e = &report.entries[0];
        assert_eq!(e.status, Status::Changed);
        assert_eq!(e.first_difference, Some(2));
        assert_eq!(e.differing_bytes, 1);
    }

    #[test]
    fn a_buffer_that_grew_is_resized_rather_than_changed() {
        let a = chunk(0x0000_0001, &[1, 2, 3, 4]);
        let b = chunk(0x0000_0001, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let report = compare(&tree(&a), &a, &tree(&b), &b);
        assert_eq!(report.entries[0].status, Status::Resized);
        assert_eq!(report.entries[0].right.map(|s| s.size), Some(8));
    }

    /// A change deep inside a container must mark its ancestors — otherwise a collapsed tree
    /// hides the one thing the user opened the screen for.
    #[test]
    fn a_container_is_changed_when_something_inside_it_is() {
        let inner_a = chunk(0x0000_0002, &[7; 4]);
        let inner_b = chunk(0x0000_0002, &[8; 4]);
        let a = chunk(CONTAINER_FLAG | 0x10, &inner_a);
        let b = chunk(CONTAINER_FLAG | 0x10, &inner_b);
        let report = compare(&tree(&a), &a, &tree(&b), &b);
        assert_eq!(report.entries[0].status, Status::Changed, "the container");
        assert_eq!(report.entries[1].status, Status::Changed, "the leaf");
        // The container's own status is not read off its bytes: those differ for every ancestor
        // of any edit, which would make the whole path look edited rather than the one chunk.
        assert_eq!(report.entries[0].differing_bytes, 0);
    }

    #[test]
    fn a_missing_chunk_is_reported_on_the_side_that_has_it() {
        let mut a = chunk(0x0000_0001, &[1; 4]);
        a.extend(chunk(0x0000_0002, &[2; 4]));
        let b = chunk(0x0000_0001, &[1; 4]);
        let report = compare(&tree(&a), &a, &tree(&b), &b);
        assert_eq!(report.tally(), [1, 0, 0, 1, 0]);
        assert_eq!(report.entries[1].status, Status::OnlyLeft);
        // And the other way round, without re-deciding what "left" means.
        let back = compare(&tree(&b), &b, &tree(&a), &a);
        assert_eq!(back.tally(), [1, 0, 0, 0, 1]);
    }

    /// Repeated ids are paired in order. Comparing the second `0x02` on the left with the first on
    /// the right would report two changes where there is one insertion.
    #[test]
    fn siblings_that_share_an_id_are_paired_in_order() {
        let mut a = chunk(0x0000_0002, &[1; 4]);
        a.extend(chunk(0x0000_0002, &[2; 4]));
        let mut b = chunk(0x0000_0002, &[1; 4]);
        b.extend(chunk(0x0000_0002, &[2; 4]));
        b.extend(chunk(0x0000_0002, &[3; 4]));
        let report = compare(&tree(&a), &a, &tree(&b), &b);
        assert_eq!(report.tally(), [2, 0, 0, 0, 1]);
    }

    /// A subtree that exists on one side only is listed in full, so an added solid reads as a
    /// solid rather than as one anonymous line.
    #[test]
    fn a_one_sided_subtree_is_listed_in_full() {
        let inner = chunk(0x0000_0002, &[7; 4]);
        let a = chunk(CONTAINER_FLAG | 0x10, &inner);
        let b: Vec<u8> = Vec::new();
        let report = compare(&tree(&a), &a, &tree(&b), &b);
        assert_eq!(report.entries.len(), 2);
        assert!(report.entries.iter().all(|e| e.status == Status::OnlyLeft));
        assert_eq!(report.entries[1].depth, 1);
    }
}
