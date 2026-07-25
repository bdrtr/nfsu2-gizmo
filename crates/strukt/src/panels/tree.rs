//! The chunk tree.
//!
//! Rows are pre-flattened in [`crate::doc::Doc::rows`], so drawing is a filtered pass over a flat
//! list rather than a recursive walk each frame. The largest car file is ~7 500 nodes, which is
//! small enough to draw whole — the hex view is where virtualization is needed, not here.
//!
//! Each row carries what the design asks for: a caret for containers, a status dot, the chunk's
//! label, badges, and the id in hex.

use crate::app::Strukt;
use crate::theme::{self, token};
use egui::{Color32, RichText, Sense};
use gizmo_nfs::validate::ChunkStatus;

/// Draw the tree; returns the offset the user clicked, if any.
pub fn show(app: &Strukt, ui: &mut egui::Ui) -> Option<usize> {
    let Some(doc) = &app.doc else { return None };
    let mut clicked = None;
    let row_h = app.density.row_height();

    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        // Skip anything nested inside a collapsed container: `hidden_below` is the depth at which
        // we stopped drawing, cleared as soon as we come back up to it.
        let mut hidden_below: Option<usize> = None;
        for row in &doc.rows {
            match hidden_below {
                Some(d) if row.depth > d => continue,
                Some(_) => hidden_below = None,
                None => {}
            }
            let selected = app.selection == Some(row.offset);
            let collapsed = app.collapsed.contains(&row.offset);
            if collapsed && row.has_children {
                hidden_below = Some(row.depth);
            }

            let (rect, resp) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), row_h),
                Sense::click(),
            );
            if resp.clicked() {
                clicked = Some(row.offset);
            }
            if selected {
                ui.painter().rect_filled(rect, 0.0, token::ACCENT.gamma_multiply(0.18));
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, 0.0, token::SURFACE);
            }

            let p = ui.painter().clone();
            let indent = 6.0 + row.depth as f32 * 11.0;
            let mid = rect.center().y;
            let mono = theme::font::mono(app.density.body_size() - 1.5);

            // Caret — only containers have one, and it says whether it is open.
            if row.has_children {
                p.text(
                    egui::pos2(rect.left() + indent - 9.0, mid),
                    egui::Align2::LEFT_CENTER,
                    if collapsed { "▸" } else { "▾" },
                    mono.clone(),
                    theme::muted(55),
                );
            }
            // Status dot — the design's at-a-glance "is there something wrong here". An unchecked
            // chunk deliberately gets the neutral dot rather than a tick: a green mark on
            // something no rule read would be the tool vouching for what it does not know.
            let status = doc.report.status_of(row.offset);
            p.circle_filled(egui::pos2(rect.left() + indent + 4.0, mid), 2.5, dot_colour(status, row.container));
            if matches!(status, ChunkStatus::Warn | ChunkStatus::Error) {
                p.text(
                    egui::pos2(rect.left() + indent - 3.0, mid),
                    egui::Align2::RIGHT_CENTER,
                    "⚠",
                    theme::font::mono(app.density.body_size() - 2.5),
                    if status == ChunkStatus::Error { token::ACCENT } else { token::ACCENT_2 },
                );
            }
            let label = crate::panels::inspector::chunk_label(row.id);
            let x = rect.left() + indent + 13.0;
            let w = p.text(
                egui::pos2(x, mid),
                egui::Align2::LEFT_CENTER,
                label,
                mono.clone(),
                if selected { token::ACCENT_800 } else { token::TEXT },
            )
            .width();
            // The id, right-aligned, always monospace — this is what you match against a spec.
            let id_font = theme::font::mono(app.density.body_size() - 2.5);
            let id_w = p
                .text(
                    egui::pos2(rect.right() - 6.0, mid),
                    egui::Align2::RIGHT_CENTER,
                    format!("{:#010x}", row.id),
                    id_font.clone(),
                    theme::muted(42),
                )
                .width();
            // The part's own name, which is what makes 610 solids navigable. It is elided rather
            // than allowed to run under the id column — two strings on top of each other read as
            // neither.
            if let Some(name) = &row.name {
                let name_font = theme::font::mono(app.density.body_size() - 2.0);
                let start = x + w + 8.0;
                let room = rect.right() - 12.0 - id_w - start;
                if room > 20.0 {
                    p.text(
                        egui::pos2(start, mid),
                        egui::Align2::LEFT_CENTER,
                        elide(ui, name, &name_font, room),
                        name_font,
                        theme::muted(58),
                    );
                }
            }
        }
    });
    clicked
}

/// Shorten `text` until it fits `room`, ending in an ellipsis.
fn elide(ui: &egui::Ui, text: &str, font: &egui::FontId, room: f32) -> String {
    let width = |s: &str| {
        ui.painter().layout_no_wrap(s.to_owned(), font.clone(), token::TEXT).size().x
    };
    if width(text) <= room {
        return text.to_owned();
    }
    // Names are ASCII in this format, but char boundaries are respected anyway.
    let mut cut: Vec<char> = text.chars().collect();
    while !cut.is_empty() {
        cut.pop();
        let candidate: String = cut.iter().collect::<String>() + "…";
        if width(&candidate) <= room {
            return candidate;
        }
    }
    String::new()
}

/// The row's status dot: what the checks concluded, falling back to a neutral shade that only
/// says container-or-leaf when nothing examined this chunk.
fn dot_colour(status: ChunkStatus, container: bool) -> Color32 {
    match status {
        ChunkStatus::Error => token::ACCENT,
        ChunkStatus::Warn => token::ACCENT_2,
        ChunkStatus::Ok => token::NEUTRAL_600,
        ChunkStatus::Unchecked | _ if container => token::NEUTRAL_500,
        _ => token::NEUTRAL_300,
    }
}

/// The panel's caption row: title plus the open file's name.
pub fn caption(app: &Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    ui.horizontal(|ui| {
        ui.label(RichText::new(t.p_tree).font(theme::font::heading(10.0)).color(theme::muted(60)));
        if let Some(doc) = &app.doc {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(doc.file_name()).font(theme::font::mono(10.0)).color(theme::muted(45)),
                );
            });
        }
    });
}
