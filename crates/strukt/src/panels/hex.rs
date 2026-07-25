//! The hex view.
//!
//! A car file is 2–11 MB, so at 16 bytes to the row this is up to ~700 000 rows: the list is
//! virtualized with `show_rows` and reads straight out of the document's single buffer, so no
//! byte is ever copied to draw it.
//!
//! Two highlight layers, exactly as the design has them: the selected chunk's whole extent as a
//! wash, and its 8-byte header on top of that. Clicking a byte selects the chunk that owns it,
//! which is the design's "hex → tree" direction of the same synchronisation.

use crate::app::Strukt;
use crate::theme::{self, token};
use egui::{Color32, RichText};

/// Bytes per row. 16 is what every hex editor uses and what the design draws.
pub const BYTES_PER_ROW: usize = 16;

/// What a byte is, for colouring.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Outside,
    /// Inside the selected chunk's payload.
    Region,
    /// Inside the selected chunk's own 8-byte header.
    Header,
    /// A field the inspector named: a counter, a name, a matrix, the alignment filler.
    Key(gizmo_nfs::inspect::SpanRole),
}

/// The byte ranges the view highlights, derived from the selection once per frame.
struct Highlights {
    region: std::ops::Range<usize>,
    header: std::ops::Range<usize>,
    /// The inspector's own field offsets. These are the same numbers the inspector shows, so a row
    /// in the pane and a run of bytes in the grid are literally the same fact.
    key: Vec<gizmo_nfs::inspect::KeySpan>,
}

impl Highlights {
    fn class_of(&self, byte: usize) -> Class {
        if let Some(span) = self.key.iter().find(|s| byte >= s.start && byte < s.start + s.len) {
            Class::Key(span.role)
        } else if self.header.contains(&byte) {
            Class::Header
        } else if self.region.contains(&byte) {
            Class::Region
        } else {
            Class::Outside
        }
    }
}

/// Draw the hex view; returns a byte offset the user clicked, if any.
pub fn show(app: &Strukt, ui: &mut egui::Ui) -> Option<usize> {
    let Some(doc) = &app.doc else {
        empty(app, ui);
        return None;
    };

    let selected = app.selection.and_then(|o| doc.node_at(o));
    let highlights = selected.map(|n| Highlights {
        region: n.offset..n.data_offset + n.header.size as usize,
        header: n.offset..n.data_offset,
        key: app.selected_model().map(|m| m.key_spans.clone()).unwrap_or_default(),
    });

    let row_h = (app.density.body_size() + 4.0).round();
    let total_rows = doc.bytes.len().div_ceil(BYTES_PER_ROW);
    let mono = theme::font::mono(app.density.body_size() - 0.5);
    let mut clicked = None;

    let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
    // A tree click asks the hex view to follow. The row height is uniform, so the target offset
    // is exact arithmetic — no scroll-to-rect hunting through rows that were never rendered.
    if let Some(target) = app.scroll_hex_to.filter(|_| app.doc.is_some()) {
        let row = target / BYTES_PER_ROW;
        let y = row as f32 * row_h - ui.available_height() * 0.35;
        area = area.vertical_scroll_offset(y.max(0.0));
    }

    area.show_rows(ui, row_h, total_rows, |ui, rows| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for row in rows {
            let start = row * BYTES_PER_ROW;
            let end = (start + BYTES_PER_ROW).min(doc.bytes.len());
            let (rect, resp) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), row_h),
                egui::Sense::click(),
            );
            let p = ui.painter();
            // `FontsView::glyph_width` needs a mutable view that `Ui::fonts` will not hand out;
            // laying out one digit gives the same number and is cached by content.
            let char_w = ui
                .painter()
                .layout_no_wrap("0".to_owned(), mono.clone(), token::TEXT)
                .size()
                .x;
            let mid = rect.center().y;

            // Offset column.
            p.text(
                egui::pos2(rect.left() + 6.0, mid),
                egui::Align2::LEFT_CENTER,
                format!("{start:08X}"),
                mono.clone(),
                theme::muted(40),
            );
            let hex_x = rect.left() + 6.0 + char_w * 10.0;
            let ascii_x = hex_x + char_w * (BYTES_PER_ROW as f32 * 3.0 + 2.0);

            for (i, b) in doc.bytes[start..end].iter().enumerate() {
                let byte = start + i;
                let class = highlights.as_ref().map_or(Class::Outside, |h| h.class_of(byte));
                let x = hex_x + char_w * (i as f32 * 3.0);
                if class != Class::Outside {
                    p.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x - 1.0, rect.top()),
                            egui::vec2(char_w * 2.0 + 2.0, row_h),
                        ),
                        0.0,
                        wash(class),
                    );
                }
                // A zero byte is dimmed: in a multi-megabyte file the padding is most of what you
                // scroll past, and dimming it is what makes the real data findable.
                let colour = if light_type(class) {
                    token::NEUTRAL_100
                } else if *b == 0 {
                    theme::muted(28)
                } else {
                    token::TEXT
                };
                p.text(
                    egui::pos2(x, mid),
                    egui::Align2::LEFT_CENTER,
                    format!("{b:02X}"),
                    mono.clone(),
                    colour,
                );
                let ch = if b.is_ascii_graphic() { *b as char } else { '.' };
                p.text(
                    egui::pos2(ascii_x + char_w * i as f32, mid),
                    egui::Align2::LEFT_CENTER,
                    ch,
                    mono.clone(),
                    if *b == 0 { theme::muted(25) } else { theme::muted(70) },
                );
            }

            if resp.clicked() {
                // Which byte was clicked: the column under the pointer, so a click in the hex
                // grid selects the chunk that owns exactly that byte.
                if let Some(pos) = resp.interact_pointer_pos() {
                    let col = ((pos.x - hex_x) / (char_w * 3.0)).floor();
                    let col = col.clamp(0.0, (BYTES_PER_ROW - 1) as f32) as usize;
                    clicked = Some((start + col).min(doc.bytes.len().saturating_sub(1)));
                }
            }
        }
    });
    clicked
}

/// The wash behind a highlighted byte — the design's own swatches: `accent-200` marks the chunk's
/// region, `accent-500` the bytes that carry its counters. (`accent-100` reads as white against the
/// page, which is what made the region invisible before.)
fn wash(class: Class) -> Color32 {
    use gizmo_nfs::inspect::SpanRole;
    match class {
        Class::Outside => Color32::TRANSPARENT,
        Class::Region => token::ACCENT_200,
        Class::Header => token::ACCENT_300,
        Class::Key(SpanRole::Counter) => token::ACCENT_500,
        Class::Key(SpanRole::Name) => token::ACCENT_2,
        Class::Key(SpanRole::Matrix) => token::ACCENT_400,
        // Filler is inert by definition: showing it as dead grey is itself the information.
        Class::Key(_) => token::NEUTRAL_300,
    }
}

/// Whether type on this wash needs to be light.
fn light_type(class: Class) -> bool {
    use gizmo_nfs::inspect::SpanRole;
    matches!(class, Class::Key(SpanRole::Counter | SpanRole::Name))
}

/// What the centre shows before a file is open.
fn empty(app: &Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.label(RichText::new(t.no_file).color(theme::muted(50)));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_wins_over_region_where_they_overlap() {
        // The header is the first 8 bytes of the chunk's own extent, so a byte can be in both;
        // the stronger layer must win or the header stops being visible.
        let h = Highlights { region: 0x100..0x200, header: 0x100..0x108, key: Vec::new() };
        assert_eq!(h.class_of(0x100), Class::Header);
        assert_eq!(h.class_of(0x108), Class::Region);
        assert_eq!(h.class_of(0x200), Class::Outside);
    }

    #[test]
    fn a_named_field_outranks_the_region_and_the_header() {
        // The counter bytes are inside the chunk's region and may sit inside its header too; the
        // whole point of the layer is that it wins, so the inspector's row and the highlighted
        // bytes are visibly the same fact.
        use gizmo_nfs::inspect::{KeySpan, SpanRole};
        let h = Highlights {
            region: 0x100..0x200,
            header: 0x100..0x108,
            key: vec![KeySpan::new(0x140, 4, SpanRole::Counter)],
        };
        assert_eq!(h.class_of(0x140), Class::Key(SpanRole::Counter));
        assert_eq!(h.class_of(0x144), Class::Region);
    }

    #[test]
    fn a_short_last_row_is_still_a_row() {
        // 17 bytes is two rows, the second holding one byte — an off-by-one here truncates the
        // end of every file.
        assert_eq!(17usize.div_ceil(BYTES_PER_ROW), 2);
        assert_eq!(16usize.div_ceil(BYTES_PER_ROW), 1);
        assert_eq!(0usize.div_ceil(BYTES_PER_ROW), 0);
    }
}
