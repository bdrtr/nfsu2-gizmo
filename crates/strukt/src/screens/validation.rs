//! The validation screen.
//!
//! The rules themselves belong in the parser (they are format knowledge, and the CLI wants them
//! too), so this screen is a read of that report. Until it lands, the screen shows the rule set
//! the design specifies, so the shape of the answer is visible.

use crate::app::Strukt;
use crate::theme::{self, token};
use egui::RichText;

pub fn show(app: &mut Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::same(16)))
        .show_inside(ui, |ui| {
            ui.label(RichText::new(t.nav_validation).font(theme::font::heading(25.0)));
            ui.add_space(theme::token::SPACE_2);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (title, expr) in RULES {
                    egui::Frame::new()
                        .fill(token::SURFACE)
                        .stroke(egui::Stroke::new(1.0_f32, token::DIVIDER))
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(*title).font(theme::font::heading(14.0)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(*expr)
                                            .font(theme::font::mono(11.0))
                                            .color(theme::muted(50)),
                                    );
                                });
                            });
                            ui.label(RichText::new(app.lang.strings().soon).size(11.0).color(token::ACCENT));
                        });
                    ui.add_space(theme::token::SPACE_2);
                }
            });
        });
}

/// The design's five rule groups, in its order. Each becomes a rule in the parser's validation
/// pass; the expression is shown verbatim because it is what makes a finding checkable by hand.
const RULES: &[(&str, &str)] = &[
    ("Stride tutarlılığı / Stride sanity", "stride = vb_size / n_vtx"),
    ("Sınır kutusu / Bounding box", "≈ araç ölçüsü / vehicle-sized"),
    ("Normaller / Normals", "|n| ≈ 1"),
    ("İndeks aralığı / Index range", "max_index < n_vtx"),
    ("Chunk boyutu / Chunk size", "≤ dosya sınırı / within file bounds"),
];
