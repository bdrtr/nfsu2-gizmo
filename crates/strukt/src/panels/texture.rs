//! The texture tab: what the car is painted with.
//!
//! A grid of thumbnails and one large preview. The thumbnails are downscaled on the CPU and only
//! the selected image is uploaded at full size — a car ships 57–76 textures and several are
//! 512×512, so uploading them all would cost tens of megabytes of GPU memory to show a contact
//! sheet.
//!
//! Every image keeps its own aspect ratio, in the grid as well as in the preview. A contact sheet
//! that squares everything off would show a badge strip as a badge, and the shape of a texture is
//! part of reading it.
//!
//! Textures in this format carry no names, only hashes; what names there are come from the
//! `DebugName` the compiler left in the pool, which is why the label falls back to the hash
//! rather than to something invented.

use crate::app::Strukt;
use crate::theme::{self, token};
use egui::{ColorImage, RichText, TextureHandle};
use gizmo_nfs::{AssetHash, NfsTexture};

/// Thumbnail edge, in points.
const THUMB: usize = 96;

/// Draw the tab.
pub fn show(app: &mut Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    let Some(doc) = &mut app.doc else {
        note(ui, t.no_file);
        return;
    };
    let Some(tpk) = doc.textures() else {
        note(ui, t.no_textures);
        return;
    };

    // A stable order, so the grid does not reshuffle between frames (the table is a HashMap).
    let mut entries: Vec<(&AssetHash, &NfsTexture)> = tpk.textures.iter().collect();
    entries.sort_by(|a, b| a.1.name.cmp(&b.1.name).then(a.0 .0.cmp(&b.0 .0)));
    let undecoded = tpk.entries.len().saturating_sub(tpk.textures.len());

    // The preview opens on the first image rather than empty — the sheet is there to be read, and
    // one image already up says what kind of file this is.
    if app.texture_selection.is_none() {
        app.texture_selection = entries.first().map(|(hash, _)| **hash);
    }
    let selected = app.texture_selection.filter(|h| tpk.texture(*h).is_some());
    let mut pick = None;
    let mut save_one = None;

    egui::Panel::right("texture_preview")
        .resizable(true)
        .default_size(260.0)
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::same(8)))
        .show_inside(ui, |ui| {
            // A panel only keeps the width it was given if its contents claim it; an image and a
            // few short rows do not, and the panel would collapse to its 96 px minimum.
            ui.set_min_width(ui.max_rect().width());
            let Some(tex) = selected.and_then(|h| tpk.texture(h)) else {
                ui.label(RichText::new(t.pick_texture).color(theme::muted(50)));
                return;
            };
            let handle = upload(ui.ctx(), &mut app.texture_cache, tex, None);
            // Fill the pane, upscaling a small image rather than leaving it postage-stamp sized —
            // the size it is drawn at is stated underneath, and the point of a preview is to see
            // the texels. The fitted size is computed here so no `ImageFit` rule can distort it.
            let side = ui.available_width();
            let natural = egui::vec2(tex.width.max(1) as f32, tex.height.max(1) as f32);
            let fitted = natural * (side / natural.x.max(natural.y));
            ui.add(egui::Image::new(&handle).fit_to_exact_size(fitted));
            ui.add_space(theme::token::SPACE_2);
            ui.label(RichText::new(label_of(tex)).font(theme::font::mono(12.0)));
            for (k, v) in [
                ("hash", format!("{:#010x}", tex.hash.0)),
                ("size", format!("{} × {}", tex.width, tex.height)),
                ("format", format!("{:?}", tex.source_format)),
                ("opaque", format!("{}%", opaque_percent(tex))),
            ] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(k).size(11.0).color(theme::muted(60)));
                    ui.label(RichText::new(v).font(theme::font::mono(11.5)));
                });
            }
            ui.add_space(theme::token::SPACE_2);
            // Just this one image. The toolbar's export writes the whole pack; someone who came
            // for a single texture should not have to take 73.
            if ui.button("PNG").on_hover_text(t.export_hint).clicked() {
                save_one = Some(tex.hash);
            }
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(token::BG).inner_margin(egui::Margin::same(8)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} {}", entries.len(), t.textures_count))
                        .size(11.0)
                        .color(theme::muted(55)),
                );
                if undecoded > 0 {
                    // Said out loud: a contact sheet that quietly omits what it could not read
                    // would have the user believe the car has fewer textures than it does.
                    ui.label(
                        RichText::new(format!("· {undecoded} {}", t.textures_undecoded))
                            .size(11.0)
                            .color(token::ACCENT_2),
                    );
                }
            });
            ui.add_space(theme::token::SPACE_1);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (hash, tex) in &entries {
                        let handle = upload(ui.ctx(), &mut app.texture_cache, tex, Some(THUMB));
                        let resp = cell(ui, &handle, &label_of(tex), selected == Some(**hash));
                        if resp.clicked() {
                            pick = Some(**hash);
                        }
                        resp.on_hover_text(format!(
                            "{}\n{} × {} · {:?}",
                            label_of(tex),
                            tex.width,
                            tex.height,
                            tex.source_format
                        ));
                    }
                });
            });
        });

    if let Some(hash) = pick {
        app.texture_selection = Some(hash);
    }
    if let Some(hash) = save_one {
        let result = crate::export::one_texture(app, hash);
        app.report_export(result);
    }
}

/// One cell of the contact sheet: the thumbnail in a square slot, its name beneath. The slot is
/// square so the grid stays a grid; the image inside it is not, so a strip still reads as a strip.
fn cell(ui: &mut egui::Ui, handle: &TextureHandle, label: &str, on: bool) -> egui::Response {
    let side = THUMB as f32;
    ui.allocate_ui(egui::vec2(side, side + 16.0), |ui| {
        ui.vertical(|ui| {
            let resp = ui.add(
                egui::Button::image(egui::Image::new(handle).max_size(egui::vec2(side, side)))
                    .min_size(egui::vec2(side, side))
                    .selected(on),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(label).font(theme::font::mono(9.0)).color(theme::muted(50)),
                )
                .truncate(),
            );
            resp
        })
        .inner
    })
    .inner
}

/// Upload a texture (or its thumbnail) once and keep the handle.
fn upload(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<(u32, bool), TextureHandle>,
    tex: &NfsTexture,
    thumb: Option<usize>,
) -> TextureHandle {
    let key = (tex.hash.0, thumb.is_some());
    if let Some(handle) = cache.get(&key) {
        return handle.clone();
    }
    let image = match thumb {
        Some(side) => downscale(tex, side),
        None => {
            let size = [tex.width as usize, tex.height as usize];
            ColorImage::from_rgba_unmultiplied(size, &tex.rgba)
        }
    };
    // Nearest at full size: the preview upscales, and a filter would smear exactly the texel edges
    // someone opens a texture to look at. Thumbnails are already downscaled to their drawn size.
    let options =
        if thumb.is_some() { egui::TextureOptions::LINEAR } else { egui::TextureOptions::NEAREST };
    let name = format!("tex{:08x}{}", tex.hash.0, u8::from(thumb.is_some()));
    let handle = ctx.load_texture(name, image, options);
    cache.insert(key, handle.clone());
    handle
}

/// Nearest-neighbour downscale into a `side`-bounded box, keeping the aspect ratio. Nearest rather
/// than a filter on purpose: these are texture atlases, and a blurred one hides the seams you are
/// looking for.
fn downscale(tex: &NfsTexture, side: usize) -> ColorImage {
    let (w, h) = (tex.width as usize, tex.height as usize);
    if w == 0 || h == 0 {
        return ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]);
    }
    let (tw, th) = if w >= h {
        (side.min(w), (side * h / w).clamp(1, h))
    } else {
        ((side * w / h).clamp(1, w), side.min(h))
    };
    let mut out = Vec::with_capacity(tw * th * 4);
    for y in 0..th {
        let sy = y * h / th;
        for x in 0..tw {
            let sx = x * w / tw;
            let i = (sy * w + sx) * 4;
            out.extend_from_slice(tex.rgba.get(i..i + 4).unwrap_or(&[0, 0, 0, 0]));
        }
    }
    ColorImage::from_rgba_unmultiplied([tw, th], &out)
}

/// A texture's label: its `DebugName` when the compiler left one, else its hash.
fn label_of(tex: &NfsTexture) -> String {
    if tex.name.is_empty() {
        format!("{:#010x}", tex.hash.0)
    } else {
        tex.name.clone()
    }
}

/// How much of the image is opaque — the number that tells an overlay from a full-coverage map.
///
/// Every fourth texel, not all of them: this runs once a frame while the preview is open, and a
/// coverage figure read off a quarter of a 512×512 image is the same figure.
fn opaque_percent(tex: &NfsTexture) -> usize {
    let sampled = tex.rgba.chunks_exact(4).step_by(4);
    let (opaque, total) =
        sampled.fold((0usize, 0usize), |(o, n), px| (o + usize::from(px[3] > 200), n + 1));
    opaque * 100 / total.max(1)
}

fn note(ui: &mut egui::Ui, message: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.label(RichText::new(message).color(theme::muted(50)));
    });
}
