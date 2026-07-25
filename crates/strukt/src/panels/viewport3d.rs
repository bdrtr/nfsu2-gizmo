//! The 3D tab: the selected part, orbited.
//!
//! What it shows follows the selection, which is the design's rule for the whole centre area: a
//! chunk inside a solid shows that solid's mesh; anything else shows the whole file. The frame is
//! the file's own — Z-up, one unit to the metre — so the viewport agrees with the status bar and
//! with what `ug2 export` writes.

use crate::app::Strukt;
use crate::gpu::math;
use crate::gpu::preview::MeshKey;
use crate::theme::{self, token};
use egui::RichText;

/// Where the camera is looking from.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    /// Distance from the target, in metres.
    pub distance: f32,
    pub target: [f32; 3],
    /// Set when the mesh changed and the camera should re-frame it.
    pub framed: Option<MeshKey>,
}

impl Default for Camera {
    fn default() -> Self {
        // A three-quarter view from above the front — the angle the design's viewport shows, and
        // the one that reads as "a car" rather than as a silhouette.
        Self {
            yaw: 0.9,
            pitch: 0.35,
            distance: 6.0,
            target: [0.0; 3],
            framed: None,
        }
    }
}

impl Camera {
    /// Put the whole of `bounds` in view.
    fn frame(&mut self, bounds: ([f32; 3], [f32; 3])) {
        let (lo, hi) = bounds;
        let centre = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
        let extent = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        let radius = (extent[0] * extent[0] + extent[1] * extent[1] + extent[2] * extent[2]).sqrt() * 0.5;
        self.target = centre;
        self.distance = (radius * 2.4).clamp(0.4, 400.0);
    }

    /// The clip-from-world matrix for an image of `aspect`.
    fn clip_from_world(&self, aspect: f32) -> math::M4 {
        let eye = math::orbit_eye(self.target, self.yaw, self.pitch, self.distance);
        let near = (self.distance * 0.01).max(0.01);
        let far = self.distance * 10.0 + 10.0;
        math::mul(
            math::perspective(0.9, aspect, near, far),
            math::look_at(eye, self.target, [0.0, 0.0, 1.0]),
        )
    }
}

/// Draw the tab.
pub fn show(app: &mut Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    let Some(state) = app.render_state.clone() else {
        centered(ui, "no wgpu backend");
        return;
    };
    if app.preview.is_none() {
        app.preview = crate::gpu::preview::Preview::new(&state);
    }
    let Some(preview) = &mut app.preview else {
        centered(ui, "no wgpu backend");
        return;
    };

    // What to show: the solid the selection sits in, else the whole file.
    let Some(doc) = &app.doc else {
        centered(ui, t.no_file);
        return;
    };
    let solid = app.selection.and_then(|o| doc.solid_of(o));
    let key = solid.map_or(MeshKey::StockCar, |s| MeshKey::Solid(s.offset));
    if preview.mesh_key() != Some(key) {
        let parts: Vec<&gizmo_nfs::NfsMeshPart> = match solid {
            // One solid: match it by the name in its header, which is what ties a chunk in the
            // tree to a parsed part.
            Some(s) => {
                let name = s
                    .find(gizmo_nfs::geometry::format::SOLID_HEADER)
                    .map(|h| gizmo_nfs::geometry::part_name(h.data(&doc.bytes)))
                    .unwrap_or_default();
                doc.parts.iter().filter(|p| p.name == name).collect()
            }
            // Nothing solid selected: the showroom car, not every part in the file. Drawing all
            // 609 would stack two dozen bumpers and four widebodies on top of each other — true
            // to the bytes, useless as a picture.
            None => gizmo_nfs::select_stock_car(&doc.parts),
        };
        preview.upload(&state, key, &parts);
        app.camera.framed = None;
    }
    if app.camera.framed != Some(key) {
        if let Some(bounds) = preview.bounds() {
            app.camera.frame(bounds);
        }
        app.camera.framed = Some(key);
    }

    // The viewport itself.
    let available = ui.available_size();
    let (rect, response) =
        ui.allocate_exact_size(available, egui::Sense::click_and_drag());
    let drag = response.drag_delta();
    if drag != egui::Vec2::ZERO {
        app.camera.yaw -= drag.x * 0.01;
        app.camera.pitch = (app.camera.pitch + drag.y * 0.01).clamp(-1.5, 1.5);
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            app.camera.distance = (app.camera.distance * (1.0 - scroll * 0.002)).clamp(0.2, 500.0);
        }
    }

    let ppp = ui.ctx().pixels_per_point();
    let size = [(rect.width() * ppp) as u32, (rect.height() * ppp) as u32];
    let aspect = rect.width().max(1.0) / rect.height().max(1.0);
    let texture = preview.render(&state, size, app.camera.clip_from_world(aspect));
    ui.painter().image(
        texture,
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );

    overlay(app, ui, rect);
}

/// The corner readouts the design puts over the viewport: what is shown, how big it is, and how
/// to move the camera.
fn overlay(app: &Strukt, ui: &egui::Ui, rect: egui::Rect) {
    let t = app.lang.strings();
    let p = ui.painter();
    let Some(preview) = &app.preview else { return };
    let mono = theme::font::mono(10.5);

    if let Some((lo, hi)) = preview.bounds() {
        let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        let what = match preview.mesh_key() {
            Some(MeshKey::StockCar) => t.stock_car.to_string(),
            _ => app
                .selected_model()
                .and_then(|m| m.summary.clone())
                .or_else(|| app.selected_solid_name())
                .unwrap_or_default(),
        };
        p.text(
            rect.left_top() + egui::vec2(10.0, 10.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{what}   {} {:.2} × {:.2} × {:.2} m · {} ▲",
                t.bbox,
                ext[0],
                ext[1],
                ext[2],
                preview.triangles()
            ),
            mono.clone(),
            theme::muted(65),
        );
    }
    p.text(
        rect.left_bottom() + egui::vec2(10.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        t.drag_hint,
        mono,
        theme::muted(45),
    );
}

fn centered(ui: &mut egui::Ui, message: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.label(RichText::new(message).color(theme::muted(50)));
    });
    let _ = token::BG;
}
