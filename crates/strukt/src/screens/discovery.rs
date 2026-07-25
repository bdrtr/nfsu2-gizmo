//! The discovery screen: read an undecoded chunk with a layout you type, and see it at once.
//!
//! This is the screen the project plan calls the most differentiating thing the tool can have, and
//! the reason is that the format will never be finished. A fixed parser can only show what it
//! already knows; this shows what a *guess* would say, and lets the guess be wrong cheaply.
//!
//! It owns no format knowledge — [`gizmo_nfs::discover`] reads the bytes, suggests the strides
//! that divide the payload exactly, and makes the timid column guesses. What lives here is the
//! table, and the arithmetic being visible while you type: how many records the stride implies,
//! how many bytes are left over (the number that says a stride is wrong), and how much of each
//! record no column has claimed.

use crate::app::Strukt;
use crate::panels;
use crate::theme::{self, token};
use egui::RichText;
use gizmo_nfs::discover::{self, Cell, Kind, Schema};

/// The screen's own state: a schema, and which chunk it was built for.
#[derive(Default)]
pub struct State {
    /// Header offset of the chunk the schema describes, so a new selection re-seeds it.
    chunk: Option<usize>,
    pub schema: Schema,
}

/// Draw the screen; returns a chunk offset when the tree was clicked.
pub fn show(app: &mut Strukt, ui: &mut egui::Ui) -> Option<usize> {
    let mut select = None;
    egui::Panel::left("discovery_tree")
        .resizable(true)
        .default_size(280.0)
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::symmetric(6, 4)))
        .show_inside(ui, |ui| {
            panels::tree::caption(app, ui);
            if let Some(offset) = panels::tree::show(app, ui) {
                select = Some(offset);
            }
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::same(12)))
        .show_inside(ui, |ui| body(app, ui));
    select
}

fn body(app: &mut Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    ui.label(RichText::new(t.nav_discovery).font(theme::font::heading(21.0)));
    ui.label(RichText::new(t.w_card_discovery).size(11.0).color(theme::muted(55)));
    ui.add_space(theme::token::SPACE_2);

    // The payload of the selected chunk, copied out once: the schema reads it every frame, and
    // borrowing the doc through the closures below would fight the mutable app.
    let Some((chunk_at, label, bytes)) = payload(app) else {
        ui.label(RichText::new(t.nothing_selected).color(theme::muted(50)));
        return;
    };

    // A new chunk gets a fresh proposal rather than the last chunk's schema, which would be a
    // reading of the wrong file half the time.
    if app.discover.chunk != Some(chunk_at) {
        app.discover.chunk = Some(chunk_at);
        app.discover.schema = discover::propose(&bytes);
    }

    controls(app, ui, &label, &bytes);
    ui.add_space(theme::token::SPACE_2);
    table(app, ui, &bytes);
}

/// The selected chunk: where its header is, what to call it, and its payload bytes.
fn payload(app: &Strukt) -> Option<(usize, String, Vec<u8>)> {
    let doc = app.doc.as_ref()?;
    let node = app.selection.and_then(|o| doc.node_at(o))?;
    let label = format!(
        "{:#010x} · {} · {} B",
        node.header.id,
        panels::inspector::chunk_label(node.header.id),
        node.header.size
    );
    Some((node.offset, label, node.data(&doc.bytes).to_vec()))
}

/// Header offset, stride, the strides that divide exactly, and what the arithmetic says.
fn controls(app: &mut Strukt, ui: &mut egui::Ui, label: &str, bytes: &[u8]) {
    let t = app.lang.strings();
    ui.label(RichText::new(label).font(theme::font::mono(11.0)).color(theme::muted(55)));
    ui.add_space(theme::token::SPACE_1);

    ui.horizontal(|ui| {
        ui.label(RichText::new(t.d_header).size(11.0).color(theme::muted(60)));
        ui.add(egui::DragValue::new(&mut app.discover.schema.header).range(0..=bytes.len()).speed(1.0));
        ui.add_space(theme::token::SPACE_2);
        ui.label(RichText::new(t.d_stride).size(11.0).color(theme::muted(60)));
        ui.add(egui::DragValue::new(&mut app.discover.schema.stride).range(0..=4096).speed(1.0));
        ui.add_space(theme::token::SPACE_2);
        if ui.button(t.d_guess).on_hover_text(t.d_guess_hint).clicked() {
            let s = &mut app.discover.schema;
            s.columns = discover::guess_columns(bytes, s.header, s.stride);
        }
    });

    // Ranked against the header the user typed, so the suggestions follow the edit rather than
    // the file — moving the header is how someone tests a filler theory.
    let candidates = discover::ranked_candidates(bytes, app.discover.schema.header);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(t.d_candidates).size(11.0).color(theme::muted(60)));
        if candidates.is_empty() {
            ui.label(RichText::new("—").size(11.0).color(theme::muted(40)));
        }
        for c in candidates.iter().filter(|c| c.records >= 3).take(10) {
            let on = c.stride == app.discover.schema.stride;
            let text = RichText::new(format!("{}×{} · {}/{}", c.stride, c.records, c.explained, c.lanes))
                .font(theme::font::mono(10.5))
                .color(if on { token::ACCENT } else { theme::muted(60) });
            if ui.add(egui::Button::new(text).frame(on)).on_hover_text(t.d_candidate_hint).clicked() {
                app.discover.schema.stride = c.stride;
                app.discover.schema.columns = discover::guess_columns(bytes, app.discover.schema.header, c.stride);
            }
        }
    });

    let shape = discover::shape(bytes.len(), &app.discover.schema);
    ui.horizontal(|ui| {
        let mono = theme::font::mono(11.0);
        ui.label(RichText::new(format!("{} {}", shape.records, t.d_records)).font(mono.clone()));
        // A remainder is the whole point: it is what says the stride is wrong, so it is loud.
        let (colour, weight) = if shape.remainder == 0 {
            (theme::muted(45), false)
        } else {
            (token::ACCENT, true)
        };
        let left = RichText::new(format!("· {} B {}", shape.remainder, t.d_left))
            .font(mono.clone())
            .color(colour);
        ui.label(if weight { left.strong() } else { left });
        if shape.unclaimed > 0 {
            ui.label(
                RichText::new(format!("· {} B {}", shape.unclaimed, t.d_unclaimed))
                    .font(mono)
                    .color(theme::muted(45)),
            );
        }
    });
}

/// The decoded table: a header row of column-type buttons over the records.
fn table(app: &mut Strukt, ui: &mut egui::Ui, bytes: &[u8]) {
    let t = app.lang.strings();
    let shape = discover::shape(bytes.len(), &app.discover.schema);
    let mono = theme::font::mono(11.0);
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace).max(14.0);

    // Every cell is padded to a fixed number of characters and drawn in the mono font, so the
    // header buttons line up with the body without a table widget.
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{:<9}", t.d_offset)).font(mono.clone()).color(theme::muted(45)),
        );
        let mut cycle = None;
        for (i, kind) in app.discover.schema.columns.iter().enumerate() {
            let text = RichText::new(format!("{:^width$}", kind.label(), width = cell_width(*kind)))
                .font(mono.clone())
                .color(token::ACCENT_600);
            if ui.add(egui::Button::new(text).frame(false)).on_hover_text(t.d_cycle).clicked() {
                cycle = Some(i);
            }
        }
        if let Some(i) = cycle {
            if let Some(k) = app.discover.schema.columns.get_mut(i) {
                *k = k.next();
            }
        }
        ui.add_space(theme::token::SPACE_2);
        if ui.button(RichText::new(" + ").font(mono.clone())).on_hover_text(t.d_add).clicked() {
            app.discover.schema.columns.push(Kind::Hex);
        }
        if ui.button(RichText::new(" − ").font(mono.clone())).on_hover_text(t.d_remove).clicked() {
            app.discover.schema.columns.pop();
        }
    });

    let schema = &app.discover.schema;
    egui::ScrollArea::both().auto_shrink([false, false]).show_rows(
        ui,
        row_height,
        shape.records,
        |ui, range| {
            for index in range {
                let mut line = format!("{:08X} ", discover::row_offset(schema, index));
                for (cell, kind) in discover::row(bytes, schema, index).iter().zip(&schema.columns) {
                    line.push_str(&format!(
                        "{:>width$}",
                        render(cell),
                        width = cell_width(*kind)
                    ));
                }
                ui.label(RichText::new(line).font(mono.clone()));
            }
        },
    );
}

/// Characters a column occupies, including the space that separates it from the next.
fn cell_width(kind: Kind) -> usize {
    match kind {
        Kind::U8 | Kind::I8 => 5,
        Kind::U16 | Kind::I16 => 7,
        Kind::Char4 => 6,
        Kind::Raw4 => 13,
        Kind::F32 => 13,
        _ => 12,
    }
}

fn render(cell: &Cell) -> String {
    match cell {
        Cell::Uint(v) => format!("{v} "),
        Cell::Int(v) => format!("{v} "),
        Cell::Hex(v) => format!("{v:08X} "),
        Cell::Float(v) => format!("{v:.4} "),
        Cell::Text(s) => format!("{s} "),
        Cell::Bytes(b) => format!("{:02X}{:02X}{:02X}{:02X} ", b[0], b[1], b[2], b[3]),
        Cell::Missing => "· ".to_string(),
        // `Cell` is `#[non_exhaustive]`: a kind added to the parser must show as something rather
        // than stop this crate compiling.
        _ => "? ".to_string(),
    }
}
