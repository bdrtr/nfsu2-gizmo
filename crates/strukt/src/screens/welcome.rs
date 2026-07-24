//! The welcome screen: open something, or read what the tool is for.

use crate::app::{Screen, Strukt};
use crate::theme::{self, token};
use egui::RichText;

pub fn show(app: &mut Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    let mut open: Option<std::path::PathBuf> = None;

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::same(24)))
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.label(RichText::new("STRUKT").font(theme::font::heading(42.0)));
                ui.label(
                    RichText::new(format!("{} · v0.1 · gizmo-nfs", t.brand_sub))
                        .size(11.0)
                        .color(theme::muted(50)),
                );
                ui.add_space(theme::token::SPACE_3);
                ui.horizontal_wrapped(|ui| {
                    for (text, accent) in [
                        ("Linux-native", false),
                        (t.w_open_source, false),
                        (t.w_validates, true),
                        (t.w_discovers, true),
                    ] {
                        chip(ui, text, accent);
                    }
                });
                ui.add_space(theme::token::SPACE_4);

                if let Some(err) = &app.error {
                    ui.label(RichText::new(format!("{} — {err}", t.open_failed)).color(token::ACCENT));
                    ui.add_space(theme::token::SPACE_2);
                }

                // Open: a path field plus drag-and-drop. No file-dialog dependency; the tool is
                // driven the same way `ug2` is, and a dropped file works from any screen.
                ui.label(RichText::new(t.w_open_file).font(theme::font::heading(10.0)).color(theme::muted(60)));
                ui.add_space(theme::token::SPACE_1);
                egui::Frame::new()
                    .stroke(egui::Stroke::new(1.0_f32, token::DIVIDER))
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.label(RichText::new(t.w_drop).font(theme::font::heading(14.0)));
                        ui.label(
                            RichText::new("GEOMETRY.BIN · TEXTURES.BIN · GLOBALB.BUN · *.viv")
                                .size(11.0)
                                .color(theme::muted(45)),
                        );
                        ui.add_space(theme::token::SPACE_2);
                        ui.horizontal(|ui| {
                            let field = egui::TextEdit::singleline(&mut app.path_input)
                                .hint_text(Strukt::suggested_dir())
                                .font(theme::font::mono(12.0))
                                .desired_width(ui.available_width() - 90.0);
                            let resp = ui.add(field);
                            let go = ui.button(t.m_open).clicked();
                            if go || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                open = Some(std::path::PathBuf::from(app.path_input.trim()));
                            }
                        });
                    });

                if !app.recents.is_empty() {
                    ui.add_space(theme::token::SPACE_4);
                    ui.label(RichText::new(t.w_recent).font(theme::font::heading(10.0)).color(theme::muted(60)));
                    for path in app.recents.clone() {
                        let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        let parent = path.parent().map_or_else(String::new, |p| p.display().to_string());
                        let resp = ui
                            .horizontal(|ui| {
                                ui.label(RichText::new(name).font(theme::font::mono(12.0)));
                                ui.label(RichText::new(parent).size(10.5).color(theme::muted(40)));
                            })
                            .response
                            .interact(egui::Sense::click());
                        if resp.clicked() {
                            open = Some(path);
                        }
                    }
                }

                ui.add_space(theme::token::SPACE_4);
                ui.label(RichText::new(t.w_modes).font(theme::font::heading(10.0)).color(theme::muted(60)));
                ui.add_space(theme::token::SPACE_1);
                let mut goto = None;
                for (screen, title, body) in [
                    (Screen::Validation, t.nav_validation, t.w_card_validation),
                    (Screen::Discovery, t.nav_discovery, t.w_card_discovery),
                    (Screen::Diff, t.nav_diff, t.w_card_diff),
                    (Screen::Dictionary, t.nav_dict, t.w_card_dict),
                ] {
                    if card(ui, title, body) {
                        goto = Some(screen);
                    }
                }
                if let Some(screen) = goto {
                    app.screen = screen;
                }
            });
        });

    if let Some(path) = open {
        app.open(&path);
    }
}

/// The design's tag chip.
fn chip(ui: &mut egui::Ui, text: &str, accent: bool) {
    let (fill, colour) = if accent {
        (token::ACCENT_100, token::ACCENT_800)
    } else {
        (egui::Color32::TRANSPARENT, token::ACCENT)
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, if accent { fill } else { token::ACCENT }))
        .inner_margin(egui::Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(colour));
        });
}

/// A mode card; returns true when clicked.
fn card(ui: &mut egui::Ui, title: &str, body: &str) -> bool {
    let resp = egui::Frame::new()
        .fill(token::SURFACE)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(RichText::new(title).font(theme::font::heading(15.0)));
            ui.label(RichText::new(body).size(12.0).color(theme::muted(75)));
        })
        .response
        .interact(egui::Sense::click());
    ui.add_space(theme::token::SPACE_1);
    resp.clicked()
}
