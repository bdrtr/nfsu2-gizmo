//! Reading an *unknown* chunk the way a person would try to: pick a stride, say what the columns
//! are, and look at whether the numbers make sense.
//!
//! [`inspect`](crate::inspect) reads chunks this crate already understands. This module is the
//! other half — the one that exists because the format never will be fully decoded. It carries no
//! knowledge of any particular chunk: it takes a [`Schema`] a person typed and reads bytes through
//! it, plus the two arithmetic tricks that actually crack layouts in this format:
//!
//! * **[`stride_candidates`]** — the strides that divide the payload exactly. The project learned
//!   the hard way that header sizes are not constant; a stride that leaves no remainder is worth
//!   more than any assumption about a header.
//! * **[`stride_for`]** — the stride implied by a count read elsewhere (`size / n`), which is how
//!   the geometry parser derives every layout it trusts.
//!
//! Nothing here allocates from a size field or indexes without a bounds check: a schema is user
//! input, and user input is as untrusted as the file.

/// What one column holds. The width is fixed by the kind — this is a byte layout, not a format
/// string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    /// A `u32` better read in hex: an id, a hash, a bitfield.
    Hex,
    F32,
    /// Four bytes read as characters — a FourCC, or the start of a name.
    Char4,
    /// Four bytes nobody has claimed yet, shown as they are.
    Raw4,
}

impl Kind {
    /// How many bytes the column occupies.
    #[must_use]
    pub fn width(self) -> usize {
        match self {
            Kind::U8 | Kind::I8 => 1,
            Kind::U16 | Kind::I16 => 2,
            Kind::U32 | Kind::I32 | Kind::Hex | Kind::F32 | Kind::Char4 | Kind::Raw4 => 4,
        }
    }

    /// The short label a column header shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Kind::U8 => "u8",
            Kind::I8 => "i8",
            Kind::U16 => "u16",
            Kind::I16 => "i16",
            Kind::U32 => "u32",
            Kind::I32 => "i32",
            Kind::Hex => "hex",
            Kind::F32 => "f32",
            Kind::Char4 => "char4",
            Kind::Raw4 => "raw",
        }
    }

    /// The kinds a viewer can cycle a column through, in a sensible order.
    #[must_use]
    pub fn all() -> &'static [Kind] {
        &[
            Kind::U32,
            Kind::I32,
            Kind::Hex,
            Kind::F32,
            Kind::U16,
            Kind::I16,
            Kind::U8,
            Kind::I8,
            Kind::Char4,
            Kind::Raw4,
        ]
    }

    /// The next kind, for a header that cycles on click.
    #[must_use]
    pub fn next(self) -> Kind {
        let all = Kind::all();
        let i = all.iter().position(|k| *k == self).unwrap_or(0);
        all[(i + 1) % all.len()]
    }
}

/// How to read a chunk's payload: skip `header` bytes, then `stride` bytes per record, read as
/// `columns` from the start of each record.
///
/// The columns need not fill the stride — a half-understood record is the normal case, and the
/// unclaimed tail is reported by [`Shape::unclaimed`] rather than hidden.
#[derive(Debug, Clone, Default)]
pub struct Schema {
    /// Bytes before the first record.
    pub header: usize,
    /// Bytes per record. Zero means "not decided yet" and yields no rows.
    pub stride: usize,
    pub columns: Vec<Kind>,
}

impl Schema {
    /// The byte offset of column `i` inside a record, and its kind.
    #[must_use]
    pub fn column_offset(&self, i: usize) -> usize {
        self.columns.iter().take(i).map(|k| k.width()).sum()
    }

    /// Bytes the columns account for.
    #[must_use]
    pub fn columns_width(&self) -> usize {
        self.columns.iter().map(|k| k.width()).sum()
    }
}

/// One value read through a schema, kept typed so a viewer renders it and a test asserts on it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Cell {
    Uint(u64),
    Int(i64),
    Hex(u32),
    Float(f32),
    Text(String),
    Bytes([u8; 4]),
    /// The record ran out before this column did.
    Missing,
}

/// What a schema makes of a payload, before any row is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    /// Whole records the payload holds.
    pub records: usize,
    /// Bytes after the last whole record — the number that says a stride is wrong.
    pub remainder: usize,
    /// Bytes of each record the columns do not describe.
    pub unclaimed: usize,
}

/// What `schema` makes of a payload of `len` bytes.
#[must_use]
pub fn shape(len: usize, schema: &Schema) -> Shape {
    let body = len.saturating_sub(schema.header);
    if schema.stride == 0 {
        return Shape { records: 0, remainder: body, unclaimed: 0 };
    }
    Shape {
        records: body / schema.stride,
        remainder: body % schema.stride,
        unclaimed: schema.stride.saturating_sub(schema.columns_width()),
    }
}

/// Read record `index`. Columns that run past the record come back [`Cell::Missing`] rather than
/// as a shorter row, so a table never silently shifts its columns.
#[must_use]
pub fn row(bytes: &[u8], schema: &Schema, index: usize) -> Vec<Cell> {
    let mut cells = Vec::with_capacity(schema.columns.len());
    let start = schema.header.saturating_add(schema.stride.saturating_mul(index));
    let mut at = start;
    for kind in &schema.columns {
        let end = at.saturating_add(kind.width());
        // A column may not read past its own record, even if the buffer continues.
        let record_end = start.saturating_add(schema.stride);
        cells.push(match bytes.get(at..end).filter(|_| end <= record_end) {
            Some(b) => cell(*kind, b),
            None => Cell::Missing,
        });
        at = end;
    }
    cells
}

/// The absolute offset of record `index` — what a viewer shows in the gutter, and what ties a row
/// back to the hex view.
#[must_use]
pub fn row_offset(schema: &Schema, index: usize) -> usize {
    schema.header.saturating_add(schema.stride.saturating_mul(index))
}

fn cell(kind: Kind, b: &[u8]) -> Cell {
    let u32_le = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    match kind {
        Kind::U8 => Cell::Uint(u64::from(b[0])),
        Kind::I8 => Cell::Int(i64::from(b[0] as i8)),
        Kind::U16 => Cell::Uint(u64::from(u16::from_le_bytes([b[0], b[1]]))),
        Kind::I16 => Cell::Int(i64::from(i16::from_le_bytes([b[0], b[1]]))),
        Kind::U32 => Cell::Uint(u64::from(u32_le(b))),
        Kind::I32 => Cell::Int(i64::from(u32_le(b) as i32)),
        Kind::Hex => Cell::Hex(u32_le(b)),
        Kind::F32 => Cell::Float(f32::from_bits(u32_le(b))),
        Kind::Char4 => Cell::Text(
            b.iter().map(|c| if c.is_ascii_graphic() || *c == b' ' { *c as char } else { '.' }).collect(),
        ),
        Kind::Raw4 => Cell::Bytes([b[0], b[1], b[2], b[3]]),
    }
}

/// A stride worth trying, and what it makes of the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub stride: usize,
    pub records: usize,
    /// 4-byte lanes the stride has (`stride / 4`).
    pub lanes: usize,
    /// Lanes whose values look like something — a float, or a small count. The rest read as
    /// arbitrary bits, which is what a wrong stride mostly produces.
    pub explained: usize,
}

/// The strides that divide `payload` exactly, largest record count last.
///
/// Only multiples of 4 up to 256: everything in this format is 4-byte aligned, and a "stride" of
/// 6 that happens to divide a buffer is noise, not a lead. A stride of 1 divides everything and
/// would drown the real candidates.
#[must_use]
pub fn stride_candidates(payload: usize) -> Vec<Candidate> {
    if payload == 0 {
        return Vec::new();
    }
    (4..=256)
        .step_by(4)
        .filter(|s| payload.is_multiple_of(*s))
        .map(|stride| Candidate {
            stride,
            records: payload / stride,
            lanes: stride / 4,
            explained: 0,
        })
        .collect()
}

/// The same candidates, each scored by how many of its lanes read as something — and ordered with
/// the most convincing first.
///
/// Dividing exactly is necessary and nowhere near sufficient — a real 17,488-byte vertex buffer is
/// divided exactly by 4, 12, 28, 36, 84, 92, 108 and 252, and only one of those is the record.
/// What separates them is whether the *lanes* hold the same kind of value in every record:
///
/// * A **divisor** of the true stride puts a different field in a lane each time round, so its
///   lanes read as nothing in particular (that buffer scores 0.33 at 12 and 0.00 at 4).
/// * The **true stride** lines every field up under itself (0.78 at 36).
/// * A **multiple** lines them up just as well (0.78 at 108) — it is the same reading, repeated.
///
/// So the ordering is: best share of explained lanes first, and among equals the *shortest*
/// stride, because the record is the smallest unit that repeats.
#[must_use]
pub fn ranked_candidates(bytes: &[u8], header: usize) -> Vec<Candidate> {
    let body = bytes.len().saturating_sub(header);
    let mut candidates = stride_candidates(body);
    for c in &mut candidates {
        // Two records prove nothing about a column; the score of a stride nobody can check is 0.
        c.explained = if c.records >= 3 {
            guess_columns(bytes, header, c.stride)
                .iter()
                .filter(|k| !matches!(k, Kind::Hex | Kind::Raw4))
                .count()
        } else {
            0
        };
    }
    // The share is compared by cross-multiplication rather than as a float: no rounding, and no
    // ordering that depends on how two ratios happen to land in binary.
    candidates.sort_by(|a, b| {
        (b.explained * a.lanes).cmp(&(a.explained * b.lanes)).then(a.stride.cmp(&b.stride))
    });
    candidates
}

/// Bytes of `0x11` alignment filler at the head of a payload.
///
/// This is the single fact that makes discovery work on this format rather than mislead: the
/// filler is not part of the records, it is not a constant length, and a stride computed without
/// skipping it divides the wrong number. The parser reads every buffer this way; so does anyone
/// trying to work one out by hand.
#[must_use]
pub fn leading_filler(bytes: &[u8]) -> usize {
    bytes.len() - crate::geometry::skip_leading_filler(bytes).len()
}

/// A first reading of a payload nobody has decoded: filler skipped, the most convincing stride
/// taken, its lanes guessed.
///
/// It is a proposal, not an answer — which is why the screen that shows it also shows the other
/// candidates and the remainder.
#[must_use]
pub fn propose(bytes: &[u8]) -> Schema {
    let header = leading_filler(bytes);
    let stride = ranked_candidates(bytes, header).first().map_or(4, |c| c.stride);
    Schema { header, stride, columns: guess_columns(bytes, header, stride) }
}

/// The stride a known record count implies — `size / n`, the arithmetic the geometry parser
/// trusts over any assumed header size. `None` when the division is not exact.
#[must_use]
pub fn stride_for(payload: usize, records: usize) -> Option<usize> {
    (records > 0 && payload.is_multiple_of(records)).then(|| payload / records)
}

/// Guess what each 4-byte lane of a record holds, by looking at up to 64 records.
///
/// A lane is called `f32` only when nine in ten samples read as a number a geometry file would
/// actually contain, and a count only when nine in ten are small; anything else stays `hex`, which
/// claims nothing. Nine in ten rather than all of them because a single odd vertex — one normal
/// that came out denormal — should not blind a whole column, and because the values this is meant
/// to reject miss by an order of magnitude, not by one sample: only about one random word in six
/// reads as a plausible float, and one in seventy thousand as a small count.
#[must_use]
pub fn guess_columns(bytes: &[u8], header: usize, stride: usize) -> Vec<Kind> {
    if stride == 0 || !stride.is_multiple_of(4) {
        return Vec::new();
    }
    let lanes = stride / 4;
    let body = bytes.len().saturating_sub(header);
    let records = (body / stride).min(64);
    if records == 0 {
        return vec![Kind::Hex; lanes];
    }
    (0..lanes)
        .map(|lane| {
            let (mut floats, mut smalls, mut chars, mut seen) = (0usize, 0usize, 0usize, 0usize);
            for r in 0..records {
                let at = header + r * stride + lane * 4;
                let Some(b) = bytes.get(at..at + 4) else { continue };
                let raw = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let f = f32::from_bits(raw);
                seen += 1;
                // Zero is a plausible float; a denormal or a huge magnitude is a hex value that
                // happens to be readable as one.
                floats += usize::from(f == 0.0 || (f.is_finite() && f.abs() > 1e-6 && f.abs() < 1e7));
                smalls += usize::from(raw < 65_536);
                chars += usize::from(b.iter().all(|c| c.is_ascii_graphic() || *c == b' ' || *c == 0));
            }
            let most = |n: usize| seen > 0 && n * 10 >= seen * 9;
            if most(chars) {
                Kind::Char4
            } else if most(smalls) {
                Kind::U32
            } else if most(floats) {
                Kind::F32
            } else {
                Kind::Hex
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn a_stride_that_does_not_divide_is_reported_as_a_remainder() {
        let schema = Schema { header: 4, stride: 12, columns: vec![Kind::U32; 3] };
        let s = shape(4 + 12 * 3 + 5, &schema);
        assert_eq!((s.records, s.remainder, s.unclaimed), (3, 5, 0));
    }

    #[test]
    fn a_zero_stride_reads_nothing_rather_than_dividing_by_zero() {
        let s = shape(100, &Schema { header: 0, stride: 0, columns: vec![Kind::U32] });
        assert_eq!((s.records, s.remainder), (0, 100));
        assert_eq!(row(&[1, 2, 3, 4], &Schema::default(), 7), Vec::<Cell>::new());
    }

    #[test]
    fn columns_read_from_the_record_they_belong_to() {
        let bytes = le(&[9, 1, 2, 3, 4]);
        // Header of one u32, then two records of two u32s.
        let schema = Schema { header: 4, stride: 8, columns: vec![Kind::U32, Kind::U32] };
        assert_eq!(row(&bytes, &schema, 0), vec![Cell::Uint(1), Cell::Uint(2)]);
        assert_eq!(row(&bytes, &schema, 1), vec![Cell::Uint(3), Cell::Uint(4)]);
        assert_eq!(row_offset(&schema, 1), 12);
    }

    /// A column wider than the stride must not read into the next record — that is how a table
    /// silently turns into fiction.
    #[test]
    fn a_column_may_not_read_past_its_own_record() {
        let bytes = le(&[1, 2, 3, 4]);
        let schema = Schema { header: 0, stride: 4, columns: vec![Kind::U32, Kind::U32] };
        assert_eq!(row(&bytes, &schema, 0), vec![Cell::Uint(1), Cell::Missing]);
    }

    #[test]
    fn a_short_buffer_yields_missing_rather_than_a_panic() {
        let bytes = [1u8, 2, 3];
        let schema = Schema { header: 0, stride: 8, columns: vec![Kind::U32, Kind::U32] };
        assert_eq!(row(&bytes, &schema, 0), vec![Cell::Missing, Cell::Missing]);
        assert_eq!(row(&bytes, &schema, usize::MAX), vec![Cell::Missing, Cell::Missing]);
    }

    #[test]
    fn candidates_are_the_exact_divisors_that_are_4_byte_aligned() {
        let c = stride_candidates(72);
        let strides: Vec<usize> = c.iter().map(|c| c.stride).collect();
        assert_eq!(strides, vec![4, 8, 12, 24, 36, 72]);
        assert!(c.iter().any(|c| c.stride == 36 && c.records == 2));
        // 6 divides 72 but is not 4-aligned, and 1 would drown everything.
        assert!(!strides.contains(&6) && !strides.contains(&1));
    }

    #[test]
    fn a_known_count_gives_the_stride_the_parser_would_derive() {
        assert_eq!(stride_for(26_280, 730), Some(36));
        assert_eq!(stride_for(26_281, 730), None);
        assert_eq!(stride_for(100, 0), None);
    }

    /// The guess has to be timid: positions read as floats, counts as counts, and a hash — which
    /// is a perfectly valid float bit pattern — must not be called a float.
    #[test]
    fn the_guess_calls_a_lane_a_float_only_when_every_sample_looks_like_one() {
        let mut bytes = Vec::new();
        for i in 0..8u32 {
            bytes.extend(1.5f32.to_le_bytes()); // a position
            bytes.extend(i.to_le_bytes()); // a small count
            bytes.extend(0xB3DC_27ABu32.to_le_bytes()); // a hash
        }
        assert_eq!(guess_columns(&bytes, 0, 12), vec![Kind::F32, Kind::U32, Kind::Hex]);
    }

    /// The ranking's whole claim, on a buffer whose record is known: a divisor of the true stride
    /// mixes the fields between lanes and must lose to the stride itself, and a multiple — the
    /// same reading twice over — must lose the tie to the shorter one.
    #[test]
    fn the_true_stride_beats_both_its_divisors_and_its_multiples() {
        let mut bytes = Vec::new();
        for i in 0..24u32 {
            bytes.extend(1.5f32.to_le_bytes());
            bytes.extend((2.5f32 + i as f32).to_le_bytes());
            bytes.extend(0xB3DC_27ABu32.to_le_bytes()); // a hash: never a plausible float
        }
        let ranked = ranked_candidates(&bytes, 0);
        assert_eq!(ranked.first().map(|c| c.stride), Some(12), "{ranked:?}");
        let share = |stride: usize| {
            ranked
                .iter()
                .find(|c| c.stride == stride)
                .map(|c| c.explained as f32 / c.lanes as f32)
                .unwrap_or_default()
        };
        assert!(share(12) > share(4), "a divisor mixes the lanes: {} vs {}", share(4), share(12));
        assert_eq!(share(24), share(12), "a multiple is the same reading, so it ties");
    }

    #[test]
    fn a_stride_that_is_not_a_multiple_of_four_has_no_lanes_to_guess() {
        assert!(guess_columns(&[0; 64], 0, 6).is_empty());
        assert!(guess_columns(&[0; 64], 0, 0).is_empty());
    }
}
