//! The compare screen: the open file against another one, chunk by chunk.
//!
//! The comparison is [`gizmo_nfs::diff`] — paired by position among siblings of the same id, which
//! is the only pairing this format's ordered trees justify. What lives here is the second file (a
//! whole [`Doc`], so the right-hand side is as real as the left) and the reading of the report:
//! only the differences by default, since "what is different about these two cars" should not
//! arrive as seven thousand lines of *same*.

use crate::app::{Screen, Strukt};
use crate::doc::Doc;
use crate::panels;
use crate::theme::{self, token};
use egui::RichText;
use gizmo_nfs::diff::{self, Report, Status};

/// The screen's own state: the other file, and what comparing it said.
#[derive(Default)]
pub struct State {
    /// The path field — the same way the welcome screen takes a file, since there is no dialog.
    pub path_input: String,
    /// The other file.
    pub right: Option<Doc>,
    report: Option<Report>,
    /// Which left-hand file the report was computed against, so opening another one recomputes it.
    computed_for: Option<std::path::PathBuf>,
    /// Whether identical chunks are listed too.
    show_all: bool,
    /// The last failure to open the other file.
    error: Option<String>,
}

impl State {
    /// Load the other side. Called by the path field and by a file dropped on this screen.
    pub fn open_right(&mut self, path: &std::path::Path) {
        match Doc::open(path) {
            Ok(doc) => {
                self.right = Some(doc);
                self.report = None;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

/// Draw the screen; returns a chunk offset when a row was clicked (an offset in the *left* file).
pub fn show(app: &mut Strukt, ui: &mut egui::Ui) -> Option<usize> {
    let t = app.lang.strings();
    let mut select = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::same(14)))
        .show_inside(ui, |ui| {
            ui.label(RichText::new(t.nav_diff).font(theme::font::heading(21.0)));
            ui.label(RichText::new(t.w_card_diff).size(11.0).color(theme::muted(55)));
            ui.add_space(theme::token::SPACE_2);

            let Some(left) = &app.doc else {
                ui.label(RichText::new(t.no_file).color(theme::muted(50)));
                return;
            };
            let left_name = left.file_name();
            let left_path = left.path.clone();

            picker(app, ui, &left_name);

            // Recompute when either side changed. Both files are already parsed, so this is a walk
            // of two trees, not a re-read.
            let stale = app.diff.computed_for.as_deref() != Some(left_path.as_path());
            if app.diff.report.is_none() || stale {
                if let (Some(left), Some(right)) = (&app.doc, &app.diff.right) {
                    app.diff.report =
                        Some(diff::compare(&left.roots, &left.bytes, &right.roots, &right.bytes));
                    app.diff.computed_for = Some(left_path);
                }
            }

            if let Some(e) = app.diff.error.clone() {
                ui.label(RichText::new(format!("{}: {e}", t.open_failed)).color(token::ACCENT));
            }
            let Some(report) = &app.diff.report else {
                ui.add_space(theme::token::SPACE_2);
                ui.label(RichText::new(t.df_pick).color(theme::muted(50)));
                return;
            };

            ui.add_space(theme::token::SPACE_2);
            summary(app.lang.strings(), report, ui);
            ui.add_space(theme::token::SPACE_1);
            ui.checkbox(&mut app.diff.show_all, t.df_all);
            ui.add_space(theme::token::SPACE_1);
            select = table(app.lang.strings(), report, app.diff.show_all, ui);
        });
    select
}

/// The two file names, and how to load the second one.
fn picker(app: &mut Strukt, ui: &mut egui::Ui, left_name: &str) {
    let t = app.lang.strings();
    let mono = theme::font::mono(11.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(t.df_left).size(10.5).color(theme::muted(55)));
        ui.label(RichText::new(left_name).font(mono.clone()));
        ui.label(RichText::new("↔").color(theme::muted(45)));
        ui.label(RichText::new(t.df_right).size(10.5).color(theme::muted(55)));
        match &app.diff.right {
            Some(doc) => {
                ui.label(RichText::new(doc.file_name()).font(mono.clone()));
            }
            None => {
                ui.label(RichText::new("—").font(mono.clone()).color(theme::muted(40)));
            }
        }
    });
    ui.horizontal(|ui| {
        let hint = ui.available_width() - 90.0;
        ui.add_sized(
            [hint.max(120.0), 22.0],
            egui::TextEdit::singleline(&mut app.diff.path_input)
                .font(mono)
                .hint_text(t.w_drop),
        );
        if ui.button(t.m_open).clicked() {
            let path = std::path::PathBuf::from(app.diff.path_input.trim());
            app.diff.open_right(&path);
        }
    });
}

/// The tally, in the same five words the rows use.
fn summary(t: &crate::i18n::Strings, report: &Report, ui: &mut egui::Ui) {
    if report.identical() {
        ui.label(RichText::new(t.df_identical).color(token::NEUTRAL_600));
        return;
    }
    let [same, changed, resized, only_left, only_right] = report.tally();
    let parts = [
        (changed, t.df_changed, token::ACCENT),
        (resized, t.df_resized, token::ACCENT_2),
        (only_left, t.df_only_left, token::ACCENT_600),
        (only_right, t.df_only_right, token::ACCENT_600),
        (same, t.df_same, theme::muted(45)),
    ];
    ui.horizontal_wrapped(|ui| {
        for (i, (n, label, colour)) in parts.iter().enumerate() {
            if i > 0 {
                ui.label(RichText::new("·").color(theme::muted(30)));
            }
            ui.label(
                RichText::new(format!("{n} {label}"))
                    .font(theme::font::mono(11.0))
                    .color(if *n == 0 { theme::muted(35) } else { *colour }),
            );
        }
    });
}

/// One row per chunk, virtualised: a real car is seven thousand of them.
fn table(
    t: &crate::i18n::Strings,
    report: &Report,
    show_all: bool,
    ui: &mut egui::Ui,
) -> Option<usize> {
    let rows: Vec<&diff::Entry> =
        report.entries.iter().filter(|e| show_all || e.status.differs()).collect();
    let mono = theme::font::mono(11.0);
    let height = ui.text_style_height(&egui::TextStyle::Monospace).max(14.0);
    let mut select = None;
    egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, height, rows.len(), |ui, range| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for entry in rows[range.clone()].iter() {
            let side = entry.left.or(entry.right);
            let sizes = match (entry.left, entry.right) {
                (Some(l), Some(r)) if l.size != r.size => format!("{} → {} B", l.size, r.size),
                (Some(l), _) => format!("{} B", l.size),
                (None, Some(r)) => format!("{} B", r.size),
                (None, None) => String::new(),
            };
            let first = match entry.first_difference {
                Some(at) => format!("   {} +0x{at:X} ({} B)", t.df_first, entry.differing_bytes),
                None => String::new(),
            };
            let line = format!(
                "{} {:indent$}{:#010x} {:<16} @{:<9} {sizes}{first}",
                mark(entry.status),
                "",
                entry.id,
                panels::inspector::chunk_label(entry.id),
                side.map(|s| s.offset).unwrap_or_default(),
                indent = entry.depth * 2,
            );
            let resp = ui
                .add(egui::Label::new(RichText::new(line).font(mono.clone()).color(colour(entry.status))).sense(egui::Sense::click()))
                .on_hover_text(t.df_go);
            // Only a chunk that exists on the left can be selected there — the tree it would jump
            // to is the left file's.
            if resp.clicked() {
                if let Some(l) = entry.left {
                    select = Some(l.offset);
                }
            }
        }
    });
    select
}

fn mark(status: Status) -> &'static str {
    match status {
        Status::Same => " ",
        Status::Changed => "~",
        Status::Resized => "≠",
        Status::OnlyLeft => "−",
        Status::OnlyRight => "+",
        _ => "?",
    }
}

fn colour(status: Status) -> egui::Color32 {
    match status {
        Status::Same => theme::muted(40),
        Status::Changed => token::TEXT,
        Status::Resized => token::ACCENT_700,
        Status::OnlyLeft | Status::OnlyRight => token::ACCENT,
        _ => token::TEXT,
    }
}

/// A file dropped while this screen is showing loads the *other* side rather than replacing the
/// open file — on a compare screen, that is what dropping a file obviously means.
pub fn accepts_drop(app: &Strukt) -> bool {
    app.screen == Screen::Diff && app.doc.is_some()
}
