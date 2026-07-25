//! The workspace: five regions over one open file.
//!
//! Panel registration order is significant in egui — the bottom log is registered before the side
//! panels so it spans the full width beneath them, matching the design's layout.

use crate::app::{Strukt, Tab};
use crate::panels;
use crate::theme::{self, token};
use egui::RichText;

pub fn show(app: &mut Strukt, ui: &mut egui::Ui) {
    // Actions the panels report; applied after drawing so no panel holds a mutable borrow of the
    // app while it draws.
    let mut select: Option<usize> = None;
    let mut select_byte: Option<usize> = None;

    egui::Panel::bottom("log")
        .resizable(true)
        .default_size(120.0)
        .frame(panel_frame())
        .show_inside(ui, |ui| {
            panels::log::caption(app, ui);
            hairline(ui);
            if let Some(offset) = panels::log::show(app, ui) {
                select = Some(offset);
            }
        });

    egui::Panel::left("tree")
        .resizable(true)
        .default_size(300.0)
        .frame(panel_frame())
        .show_inside(ui, |ui| {
            panels::tree::caption(app, ui);
            hairline(ui);
            if let Some(offset) = panels::tree::show(app, ui) {
                select = Some(offset);
            }
        });

    egui::Panel::right("inspector")
        .resizable(true)
        .default_size(300.0)
        .frame(panel_frame())
        .show_inside(ui, |ui| {
            panels::inspector::caption(app, ui);
            hairline(ui);
            panels::inspector::show(app, ui);
        });

    egui::CentralPanel::default().frame(panel_frame()).show_inside(ui, |ui| {
        tabs(app, ui);
        hairline(ui);
        match app.tab {
            Tab::Hex => {
                if let Some(byte) = panels::hex::show(app, ui) {
                    select_byte = Some(byte);
                }
            }
            Tab::ThreeD => panels::viewport3d::show(app, ui),
            Tab::Texture => panels::texture::show(app, ui),
        }
    });

    // A tree/log click selects directly; a hex click selects whichever chunk owns that byte.
    if let Some(offset) = select {
        app.select(offset);
    } else if let Some(byte) = select_byte {
        if let Some(owner) = app.doc.as_ref().and_then(|d| d.owner_of_byte(byte)).map(|n| n.offset) {
            // Deliberately not `select`: the hex view is already showing this byte, and yanking
            // it to a new scroll position under the cursor would fight the user.
            app.selection = Some(owner);
        }
    } else {
        // The scroll request lives exactly one frame — otherwise the hex view would refuse to be
        // scrolled by hand.
        app.scroll_hex_to = None;
    }
}

/// The centre area's tab bar: 3D · HEX · TEXTURE, with the design's accent underline.
fn tabs(app: &mut Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    ui.horizontal(|ui| {
        for (tab, label) in
            [(Tab::ThreeD, t.tab_3d), (Tab::Hex, t.tab_hex), (Tab::Texture, t.tab_tex)]
        {
            let on = app.tab == tab;
            let colour = if on { token::ACCENT } else { theme::muted(65) };
            let text = RichText::new(label).font(theme::font::heading(11.0)).color(colour);
            let resp = ui.add(egui::Button::new(text).frame(false));
            if on {
                ui.painter().hline(
                    resp.rect.x_range(),
                    resp.rect.bottom() + 1.0,
                    egui::Stroke::new(2.0_f32, token::ACCENT),
                );
            }
            if resp.clicked() {
                app.tab = tab;
            }
        }
        if let Some(node) = app.doc.as_ref().zip(app.selection).and_then(|(d, o)| d.node_at(o)) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{:#010x} · {}",
                        node.header.id,
                        panels::inspector::chunk_label(node.header.id)
                    ))
                    .font(theme::font::mono(10.5))
                    .color(theme::muted(45)),
                );
            });
        }
    });
}


fn panel_frame() -> egui::Frame {
    egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::symmetric(6, 4))
}

/// The design's 2 px divider, used between a panel's caption and its content.
fn hairline(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, egui::Stroke::new(2.0_f32, token::DIVIDER));
}
