//! The log panel: what the tool noticed, and which chunk it noticed it about.
//!
//! Clicking a row selects the chunk it names — the design's third path into the same selection,
//! after the tree and the hex view.

use crate::app::{LogFilter, Strukt};
use crate::doc::Level;
use crate::theme::{self, token};
use egui::RichText;

/// Draw the log; returns the chunk offset to select, if a row was clicked.
pub fn show(app: &Strukt, ui: &mut egui::Ui) -> Option<usize> {
    let Some(doc) = &app.doc else { return None };
    let mut clicked = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for note in doc.notes.iter().filter(|n| app.log_filter.accepts(n.level)) {
            let resp = ui
                .horizontal(|ui| {
                    ui.label(RichText::new(note.level.icon()).color(level_colour(note.level)).size(11.0));
                    ui.label(
                        RichText::new(&note.chunk_id).font(theme::font::mono(10.5)).color(theme::muted(45)),
                    );
                    ui.label(RichText::new(&note.message).size(11.5).color(token::TEXT));
                })
                .response
                .interact(egui::Sense::click());
            if resp.clicked() {
                clicked = note.chunk;
            }
        }
    });
    clicked
}

/// The caption row: title plus the four filter buttons.
pub fn caption(app: &mut Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    ui.horizontal(|ui| {
        ui.label(RichText::new(t.p_log).font(theme::font::heading(10.0)).color(theme::muted(60)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (filter, label) in [
                (LogFilter::Info, t.log_info),
                (LogFilter::Error, t.log_err),
                (LogFilter::Warn, t.log_warn),
                (LogFilter::All, t.log_all),
            ] {
                let on = app.log_filter == filter;
                let text = RichText::new(label)
                    .size(10.5)
                    .color(if on { token::ACCENT } else { theme::muted(55) });
                if ui.add(egui::Button::new(text).frame(on)).clicked() {
                    app.log_filter = filter;
                }
            }
        });
    });
}

fn level_colour(level: Level) -> egui::Color32 {
    match level {
        Level::Info => theme::muted(45),
        Level::Warn => token::ACCENT_2,
        Level::Error => token::ACCENT,
    }
}
