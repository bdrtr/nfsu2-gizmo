//! The dictionary screen: every hash the open file references, and what it is called.
//!
//! The file names almost nothing — a texture carries a truncated `DebugName`, a shader and a solid
//! carry nothing at all — so this screen is where names are kept. What makes it more than a notes
//! field is that a name can be **checked**: `gizmo_nfs::hash` hashes it back, and a name that
//! reproduces the file's number is proof rather than a note. It is also the only way a truncated
//! name's tail comes back: type candidates until one hashes.

use crate::app::Strukt;
use crate::theme::{self, token};
use egui::RichText;
use std::collections::BTreeMap;

/// The screen's own state.
#[derive(Default)]
pub struct State {
    /// Substring filter over hashes and names.
    pub filter: String,
    /// In-progress edits, by hash: a row is committed when it loses focus or Enter is pressed, not
    /// on every keystroke — otherwise the file would be rewritten per character typed.
    edits: BTreeMap<u32, String>,
    /// The last save, to show where the file went.
    note: Option<String>,
}

/// Where a hash was referenced from.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct Sources {
    texture: bool,
    material: bool,
    shader: bool,
    part: bool,
}

/// One row: a hash, where it came from, and the name the file gave it (if any).
struct Row {
    hash: u32,
    sources: Sources,
    /// The name in the file — a TPK `DebugName`, or a part's name. Possibly truncated.
    file_name: Option<String>,
}

/// Draw the screen.
pub fn show(app: &mut Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::same(14)))
        .show_inside(ui, |ui| {
            ui.label(RichText::new(t.nav_dict).font(theme::font::heading(21.0)));
            ui.label(RichText::new(t.dc_hint).size(11.0).color(theme::muted(55)));
            ui.add_space(theme::token::SPACE_2);

            if app.doc.is_none() {
                ui.label(RichText::new(t.no_file).color(theme::muted(50)));
                return;
            }
            let rows = collect(app);

            let named = rows.iter().filter(|r| app.names.get(r.hash).is_some()).count();
            let verified = rows
                .iter()
                .filter(|r| {
                    app.names
                        .get(r.hash)
                        .is_some_and(|n| gizmo_nfs::hash::string_hash(n) == r.hash)
                })
                .count();
            ui.horizontal(|ui| {
                let mono = theme::font::mono(11.0);
                ui.label(RichText::new(format!("{} hash", rows.len())).font(mono.clone()));
                ui.label(RichText::new("·").color(theme::muted(30)));
                ui.label(
                    RichText::new(format!("{named} {}", t.dc_named)).font(mono.clone()).color(
                        if named == 0 { theme::muted(35) } else { token::TEXT },
                    ),
                );
                ui.label(RichText::new("·").color(theme::muted(30)));
                ui.label(
                    RichText::new(format!("{verified} {}", t.dc_verified))
                        .font(mono.clone())
                        .color(if verified == 0 { theme::muted(35) } else { token::ACCENT }),
                );
                ui.label(RichText::new("·").color(theme::muted(30)));
                ui.label(
                    RichText::new(format!("{} {}", app.names.len(), t.dc_in_dictionary))
                        .font(mono.clone())
                        .color(theme::muted(50)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_sized(
                        [180.0, 22.0],
                        egui::TextEdit::singleline(&mut app.dict.filter)
                            .font(mono)
                            .hint_text(t.dc_filter),
                    );
                });
            });
            if let Some(note) = &app.dict.note {
                ui.label(RichText::new(note).size(10.5).color(theme::muted(50)));
            }
            ui.add_space(theme::token::SPACE_1);
            table(app, ui, &rows);
        });
}

/// Every hash the open file points at: textures, material runs, shaders, and the solids themselves.
fn collect(app: &mut Strukt) -> Vec<Row> {
    let mut seen: BTreeMap<u32, Row> = BTreeMap::new();
    let mut add = |hash: u32, name: Option<String>, tag: fn(&mut Sources)| {
        if hash == 0 {
            return; // an empty slot, not a reference
        }
        let row = seen.entry(hash).or_insert_with(|| Row {
            hash,
            sources: Sources::default(),
            file_name: name.clone(),
        });
        tag(&mut row.sources);
        // A name from the file is worth keeping whichever reference brought it in.
        if row.file_name.is_none() {
            row.file_name = name;
        }
    };

    // Textures first, because they are the only place the file leaves a name.
    if let Some(doc) = &mut app.doc {
        if let Some(tpk) = doc.textures() {
            for tex in tpk.textures.values() {
                let name = (!tex.name.is_empty()).then(|| tex.name.clone());
                add(tex.hash.0, name, |s| s.texture = true);
            }
            for entry in &tpk.entries {
                add(entry.hash.0, None, |s| s.texture = true);
            }
        }
    }
    if let Some(doc) = &app.doc {
        for part in &doc.parts {
            let name = (!part.name.is_empty()).then(|| part.name.clone());
            add(part.hash.0, name, |s| s.part = true);
            for run in &part.materials {
                add(run.hash.0, None, |s| s.material = true);
                add(run.shader.0, None, |s| s.shader = true);
            }
        }
    }
    seen.into_values().collect()
}

/// The table: one editable name per hash, with what the file said beside it.
fn table(app: &mut Strukt, ui: &mut egui::Ui, rows: &[Row]) {
    let t = app.lang.strings();
    let filter = app.dict.filter.trim().to_uppercase();
    let visible: Vec<&Row> = rows
        .iter()
        .filter(|r| {
            if filter.is_empty() {
                return true;
            }
            format!("{:08X}", r.hash).contains(&filter)
                || r.file_name.as_ref().is_some_and(|n| n.to_uppercase().contains(&filter))
                || app.names.get(r.hash).is_some_and(|n| n.to_uppercase().contains(&filter))
        })
        .collect();

    let mono = theme::font::mono(11.0);
    let height = 24.0;
    let mut commit: Option<(u32, String)> = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(
        ui,
        height,
        visible.len(),
        |ui, range| {
            for row in &visible[range] {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{:08X}", row.hash)).font(mono.clone()).color(token::TEXT),
                    );
                    ui.label(
                        RichText::new(format!("{:<26}", sources(row.sources, t)))
                            .font(mono.clone())
                            .color(theme::muted(50)),
                    );
                    // What the file itself says, and whether that name is whole: a `DebugName`
                    // that hashes back is complete, one that does not was truncated.
                    match &row.file_name {
                        Some(name) => {
                            let whole = gizmo_nfs::hash::string_hash(name) == row.hash;
                            ui.label(
                                RichText::new(format!("{name:<24}"))
                                    .font(mono.clone())
                                    .color(if whole { token::TEXT } else { theme::muted(45) }),
                            )
                            .on_hover_text(if whole { t.dc_whole } else { t.dc_truncated });
                        }
                        None => {
                            ui.label(
                                RichText::new(format!("{:<24}", "—"))
                                    .font(mono.clone())
                                    .color(theme::muted(30)),
                            );
                        }
                    }

                    let buf = app
                        .dict
                        .edits
                        .entry(row.hash)
                        .or_insert_with(|| app.names.get(row.hash).unwrap_or_default().to_string());
                    let resp = ui.add_sized(
                        [220.0, 20.0],
                        egui::TextEdit::singleline(buf).font(mono.clone()).hint_text(t.dc_your_name),
                    );
                    let typed = buf.clone();
                    if resp.lost_focus() || resp.ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                        commit = Some((row.hash, typed.clone()));
                    }
                    // The mark is the whole point of the screen, so it is *drawn*: a tick from a
                    // font the user may not have would render as a box on the one glyph that has
                    // to be unmistakable. A ring means "kept as a note", not "wrong".
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    if !typed.trim().is_empty() {
                        let ok = gizmo_nfs::hash::string_hash(typed.trim()) == row.hash;
                        let (p, at) = (ui.painter(), rect.center());
                        if ok {
                            theme::mark::ok(p, at, 12.0, token::ACCENT);
                        } else {
                            theme::mark::unchecked(p, at, 12.0);
                        }
                        resp.on_hover_text(if ok { t.dc_verified_hint } else { t.dc_unverified_hint });
                    }
                });
            }
        },
    );

    if let Some((hash, name)) = commit {
        app.names.set(hash, &name);
        if let Some(result) = app.names.save_if_dirty() {
            app.dict.note = Some(match result {
                Ok(path) => format!("{} → {}", t.dc_saved, path.display()),
                Err(e) => format!("{}: {e}", t.dc_save_failed),
            });
        }
    }
}

/// Where a hash was seen, as a compact list.
fn sources(s: Sources, t: &crate::i18n::Strings) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if s.texture {
        parts.push(t.dc_src_texture);
    }
    if s.material {
        parts.push(t.dc_src_material);
    }
    if s.shader {
        parts.push(t.dc_src_shader);
    }
    if s.part {
        parts.push(t.dc_src_part);
    }
    parts.join(" ")
}
