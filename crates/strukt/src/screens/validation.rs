//! The validation screen: one card per rule, with what it found and what it read.
//!
//! The rules are the parser's ([`gizmo_nfs::validate`]) and run once when the file opens; this
//! screen is a read of that report. The count of chunks each rule examined is shown next to its
//! findings on purpose — a rule with nothing to report because it read nothing is not a pass, and
//! the difference is the whole reason to trust the other four.

use crate::app::Strukt;
use crate::theme::{self, token};
use egui::RichText;
use gizmo_nfs::validate::{RuleResult, Severity, RULES};

/// Draw the screen; returns a chunk offset if a finding was clicked.
pub fn show(app: &mut Strukt, ui: &mut egui::Ui) -> Option<usize> {
    let t = app.lang.strings();
    let mut select = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::same(16)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t.nav_validation).font(theme::font::heading(25.0)));
                if let Some(doc) = &app.doc {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(doc.file_name())
                                .font(theme::font::mono(11.0))
                                .color(theme::muted(50)),
                        );
                    });
                }
            });
            ui.add_space(theme::token::SPACE_2);

            let Some(doc) = &app.doc else {
                ui.label(RichText::new(t.no_file).color(theme::muted(50)));
                return;
            };
            if doc.report.worst().is_none() {
                ui.label(RichText::new(t.val_no_findings).color(token::NEUTRAL_600));
                ui.add_space(theme::token::SPACE_2);
            }

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for rule in RULES {
                    let Some(result) = doc.report.results.iter().find(|r| r.rule == rule.id) else {
                        continue;
                    };
                    if let Some(offset) = card(app, ui, rule, result) {
                        select = Some(offset);
                    }
                    ui.add_space(theme::token::SPACE_2);
                }
            });
        });
    select
}

/// One rule's card: title, the expression it applies, what it read, and its findings.
fn card(
    app: &Strukt,
    ui: &mut egui::Ui,
    rule: &gizmo_nfs::validate::Rule,
    result: &RuleResult,
) -> Option<usize> {
    let mut select = None;
    egui::Frame::new()
        .fill(token::SURFACE)
        .stroke(egui::Stroke::new(1.0_f32, token::DIVIDER))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Drawn, not typed: a missing glyph would render as a box on the one mark that
                // has to be unmistakable.
                let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                let (p, at) = (ui.painter(), rect.center());
                match result.status() {
                    Some(Severity::Error) => theme::mark::error(p, at, 12.0, token::ACCENT),
                    Some(Severity::Warn) => {
                        p.text(at, egui::Align2::CENTER_CENTER, "⚠", theme::font::body(12.0), token::ACCENT_2);
                    }
                    _ if result.examined == 0 => theme::mark::unchecked(p, at, 12.0),
                    _ => theme::mark::ok(p, at, 12.0, token::NEUTRAL_600),
                }
                ui.label(RichText::new(rule.title).font(theme::font::heading(14.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(rule.expr).font(theme::font::mono(11.0)).color(theme::muted(50)));
                });
            });
            ui.add_space(theme::token::SPACE_1);

            if result.examined == 0 {
                // Said plainly rather than dressed as a pass.
                ui.label(
                    RichText::new(app.lang.strings().val_unread).size(11.5).color(theme::muted(50)),
                );
                return;
            }
            ui.label(
                RichText::new(format!(
                    "{} {} · {} {}",
                    result.examined,
                    app.lang.strings().val_examined,
                    result.findings.len(),
                    app.lang.strings().val_findings
                ))
                .size(11.0)
                .color(theme::muted(45)),
            );

            for f in &result.findings {
                ui.add_space(2.0);
                let resp = ui
                    .horizontal_wrapped(|ui| {
                        let colour = match f.severity {
                            Severity::Error => token::ACCENT,
                            Severity::Warn => token::ACCENT_2,
                            _ => theme::muted(45),
                        };
                        ui.label(RichText::new("•").color(colour).size(11.0));
                        ui.label(RichText::new(&f.subject).font(theme::font::mono(11.5)));
                        ui.label(RichText::new(&f.message).size(11.5).color(theme::muted(80)));
                        ui.label(
                            RichText::new(format!("{:#010x}", f.chunk_id))
                                .font(theme::font::mono(10.0))
                                .color(theme::muted(40)),
                        );
                    })
                    .response
                    .interact(egui::Sense::click());
                if resp.clicked() {
                    select = Some(f.chunk_offset);
                }
            }
        });
    select
}
