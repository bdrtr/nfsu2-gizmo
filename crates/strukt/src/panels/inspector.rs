//! The inspector: the selected chunk's parsed fields.
//!
//! The reading is the parser's ([`gizmo_nfs::inspect`]) — this module only draws it. That split is
//! the point: an inspector that re-declared the offsets could disagree with the reader about what
//! a file says, and then the tool would be lying with a straight face.

use crate::app::Strukt;
use crate::theme::{self, token};
use egui::RichText;
use gizmo_nfs::inspect::Value;

/// Draw the inspector for the current selection.
pub fn show(app: &Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    let (Some(model), Some(node)) = (app.selected_model(), app.selected_node()) else {
        ui.add_space(theme::token::SPACE_3);
        ui.label(RichText::new(t.nothing_selected).color(theme::muted(50)));
        return;
    };

    ui.add_space(theme::token::SPACE_1);
    ui.label(RichText::new(chunk_label(node.header.id)).font(theme::font::heading(14.0)));
    if let Some(summary) = &model.summary {
        ui.label(RichText::new(summary).font(theme::font::mono(11.5)).color(token::ACCENT_800));
    }
    ui.label(
        RichText::new(format!("{:#010x} · {} B", node.header.id, node.header.size))
            .font(theme::font::mono(10.5))
            .color(theme::muted(50)),
    );
    ui.add_space(theme::token::SPACE_2);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for f in &model.fields {
            // A matrix is a grid, not a row — the design gives it its own block.
            if let Value::Matrix(m) = &f.value {
                matrix_grid(app, ui, m);
                continue;
            }
            ui.add_space(theme::token::SPACE_1);
            ui.horizontal(|ui| {
                ui.label(RichText::new(f.label).size(11.5).color(theme::muted(60)));
                if let Some(off) = f.offset {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{off:#x}"))
                                .font(theme::font::mono(9.0))
                                .color(theme::muted(40)),
                        );
                    });
                }
            });
            ui.label(RichText::new(render(&f.value)).font(theme::font::mono(12.0)).color(token::TEXT));
            if let Some(note) = &f.note {
                ui.label(RichText::new(note).size(10.0).color(theme::muted(48)));
            }
            ui.separator();
        }
    });
}

/// One value, as the inspector shows it.
fn render(value: &Value) -> String {
    match value {
        Value::Num(n) => n.to_string(),
        Value::Hex(h) => format!("{h:#010x}"),
        // The arithmetic, not just the answer: this is the row that catches a packed vertex
        // layout, and it is only checkable by hand if the division is visible.
        Value::Ratio { num, den, value } => format!("{num} / {den} = {value:.1}"),
        Value::Text(s) => s.clone(),
        Value::Float(f) => format!("{f:.4}"),
        Value::Float3(v) => format!("{:.3}, {:.3}, {:.3}", v[0], v[1], v[2]),
        Value::Bytes(b) => b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" "),
        Value::Matrix(_) => String::new(), // drawn as a grid
        _ => String::new(),
    }
}

/// The design's 4×4 matrix block.
fn matrix_grid(app: &Strukt, ui: &mut egui::Ui, m: &gizmo_nfs::Mat4) {
    ui.add_space(theme::token::SPACE_2);
    ui.label(
        RichText::new(app.lang.strings().matrix).font(theme::font::heading(9.5)).color(theme::muted(55)),
    );
    ui.add_space(theme::token::SPACE_1);
    egui::Grid::new("matrix").num_columns(4).spacing([6.0, 2.0]).show(ui, |ui| {
        for row in m {
            for v in row {
                // Zero and one are the matrix's furniture; dimming them leaves the numbers that
                // actually say where the part sits.
                let plain = *v == 0.0 || *v == 1.0;
                ui.label(
                    RichText::new(format!("{v:.3}"))
                        .font(theme::font::mono(10.5))
                        .color(if plain { theme::muted(35) } else { token::TEXT }),
                );
            }
            ui.end_row();
        }
    });
    ui.add_space(theme::token::SPACE_2);
}

/// The panel caption.
pub fn caption(app: &Strukt, ui: &mut egui::Ui) {
    ui.label(
        RichText::new(app.lang.strings().p_inspector)
            .font(theme::font::heading(10.0))
            .color(theme::muted(60)),
    );
}

/// A readable name for a chunk id — the tree's label and the inspector's title. An id nobody has
/// decoded stays "chunk": an invented label would be worse than a number.
#[must_use]
pub fn chunk_label(id: u32) -> &'static str {
    gizmo_nfs::inspect::type_name(id).unwrap_or("chunk")
}
