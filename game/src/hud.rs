//! Sürücünün göstergesi: hız, vites, devir — yarışta ekranda kalan tek şey.
//!
//! Eskiden sağ altta yalnız `"{hız} km/h"` yazıyordu, sol üstte de hücre ve üçgen sayıları. Bu
//! modül oyuncunun gördüğü kısmı: büyük hız, vites harfi, ve vites değiştirme devrinden sonrası
//! kırmızı olan bir devir çubuğu. Teknik sayılar binary'lerde, bir düğmenin arkasında kalıyor.
//!
//! Değerler motorun kendi `VehicleController`'ından okunuyor; burada hesaplanan bir şey yok, yalnız
//! `current_gear`'ın indeksi okunur bir harfe çevriliyor ([`gear_label`]).

use gizmo::egui;
use gizmo::physics::vehicle::VehicleController;

/// Göstergenin bir karede çizdiği her şey.
#[derive(Clone, Debug, PartialEq)]
pub struct Dash {
    /// Hız, km/h, işaretsiz.
    pub speed_kmh: f32,
    /// Motor tarafı; araba bir rayda giderken (`NFS_DEMO`) motor çalışmıyor ve yalnız hız var.
    pub engine: Option<Engine>,
    /// Nitro deposu, takılıysa: doluluk `0..=2` ve şu an yanıyor mu.
    pub nitro: Option<(f32, bool)>,
    /// Stil puanları ([`crate::rig::style`]): bankadaki toplam, şu an biriken olay (adı, puanı) ve
    /// son bankaya geçen (adı, puanı, kaç saniye önce).
    pub style: Option<Style>,
}

/// Göstergenin stil satırı.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub total: f32,
    pub running: Option<(String, f32)>,
    pub last: Option<(String, f32, f32)>,
}

/// Devir ve vites, motorun kendi alanlarından.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Engine {
    /// `VehicleController::current_gear`: 0 geri, 1 boş, 2.. ileri vitesler.
    pub gear: usize,
    pub rpm: f32,
    /// Çubuğun tamamı.
    pub max_rpm: f32,
    /// Otomatik şanzımanın vites büyüttüğü devir; çubuğun kırmızı bölgesi buradan başlıyor.
    pub shift_rpm: f32,
}

impl Dash {
    /// Motorun sürdüğü bir arabanın göstergesi.
    #[must_use]
    pub fn of(v: &VehicleController) -> Self {
        Self {
            speed_kmh: v.current_speed_kmh.abs(),
            engine: Some(Engine {
                gear: v.current_gear,
                rpm: v.engine_rpm,
                max_rpm: v.tuning.max_rpm,
                shift_rpm: v.tuning.upshift_rpm,
            }),
            nitro: None,
            style: None,
        }
    }

    /// Aynısı, şasinin nitrosu da okunarak.
    #[must_use]
    pub fn of_car(world: &gizmo::prelude::World, chassis: u32) -> Option<Self> {
        let mut d = Self::of(world.borrow::<VehicleController>().get(chassis)?);
        d.nitro = world
            .borrow::<crate::rig::powertrain::Powertrain>()
            .get(chassis)
            .filter(|p| p.spec.nitro_tank_s > 0.0)
            .map(|p| (p.nitro.fill, p.nitro.firing));
        Some(d)
    }

    /// Motoru olmayan bir hız: rayın kendi hızı.
    #[must_use]
    pub fn speed_only(speed_kmh: f32) -> Self {
        Self { speed_kmh: speed_kmh.abs(), engine: None, nitro: None, style: None }
    }
}

/// `current_gear`'ın ekrandaki adı: `R`, `N`, `1`, `2`, …
#[must_use]
pub fn gear_label(gear: usize) -> String {
    match gear {
        0 => "R".to_string(),
        1 => "N".to_string(),
        n => (n - 1).to_string(),
    }
}

/// Devrin çubuktaki payı, 0..1. `max_rpm` sıfır ya da bozuksa çubuk boş kalır.
#[must_use]
pub fn rpm_fraction(rpm: f32, max_rpm: f32) -> f32 {
    if max_rpm > 0.0 && rpm.is_finite() {
        (rpm / max_rpm).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

const BAR_W: f32 = 240.0;
const BAR_H: f32 = 12.0;
const PANEL: egui::Color32 = egui::Color32::from_rgba_premultiplied(8, 10, 14, 190);
const INK: egui::Color32 = egui::Color32::from_rgb(235, 238, 242);
const DIM: egui::Color32 = egui::Color32::from_rgb(140, 146, 156);
const RED: egui::Color32 = egui::Color32::from_rgb(232, 52, 44);
const NOS: egui::Color32 = egui::Color32::from_rgb(64, 156, 255);
const NOS_OVER: egui::Color32 = egui::Color32::from_rgb(170, 110, 255);

/// Göstergeyi sağ alta çizer.
pub fn show(ctx: &egui::Context, dash: &Dash) {
    egui::Area::new(egui::Id::new("dash"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-28.0, -28.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(PANEL)
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| panel(ui, dash));
        });
}

fn panel(ui: &mut egui::Ui, dash: &Dash) {
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("{:>3.0}", dash.speed_kmh))
                    .size(54.0)
                    .strong()
                    .monospace()
                    .color(INK),
            )
            .extend(),
        );
        ui.vertical(|ui| {
            ui.add_space(30.0);
            ui.label(egui::RichText::new("km/h").size(14.0).color(DIM));
        });
        if let Some(e) = &dash.engine {
            ui.add_space(18.0);
            let hot = e.rpm >= e.shift_rpm && e.gear >= 2;
            ui.add(
                egui::Label::new(
                    egui::RichText::new(gear_label(e.gear))
                        .size(54.0)
                        .strong()
                        .monospace()
                        .color(if hot { RED } else { INK }),
                )
                .extend(),
            );
        }
    });
    if let Some(e) = &dash.engine {
        rpm_bar(ui, e);
        ui.label(egui::RichText::new(format!("{:.0} rpm", e.rpm)).size(12.0).color(DIM));
    }
    if let Some((fill, firing)) = dash.nitro {
        nitro_bar(ui, fill, firing);
    }
    if let Some(s) = &dash.style {
        style_line(ui, s);
    }
}

/// Stil puanı: toplam, şu an biriken olay, ve son bankaya geçen iki saniye boyunca.
fn style_line(ui: &mut egui::Ui, s: &Style) {
    let mut text = format!("STİL {:.0}", s.total);
    if let Some((name, pts)) = &s.running {
        text += &format!("  ·  {name} {pts:.0}");
    } else if let Some((name, pts, age)) = &s.last {
        if *age < 2.0 {
            text += &format!("  ·  {name} +{pts:.0}");
        }
    }
    ui.label(egui::RichText::new(text).size(13.0).strong().color(NOS));
}

/// Nitro deposu: çubuğun tamamı dolu bir depo, taşan kısım (1'in üstü, en çok 2) üstüne başka renkte
/// biniyor — oyunun kademeleri oradan geliyor.
fn nitro_bar(ui: &mut egui::Ui, fill: f32, firing: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(BAR_W, BAR_H * 0.6), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, egui::Color32::from_rgb(28, 34, 46));
    let at = |f: f32| egui::pos2(rect.left() + rect.width() * f.clamp(0.0, 1.0), rect.bottom());
    let base = if firing { INK } else { NOS };
    p.rect_filled(egui::Rect::from_min_max(rect.left_top(), at(fill)), 2.0, base);
    if fill > 1.0 {
        p.rect_filled(egui::Rect::from_min_max(rect.left_top(), at(fill - 1.0)), 2.0, NOS_OVER);
    }
    ui.label(egui::RichText::new("NOS · Shift / N").size(11.0).color(DIM));
}

/// Bin devirde bir çentikli çubuk; vites devrinden sonrası kırmızı zemin, ibre oraya girince
/// kendisi de kırmızı.
fn rpm_bar(ui: &mut egui::Ui, e: &Engine) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(BAR_W, BAR_H), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, egui::Color32::from_rgb(40, 44, 52));
    let x_at = |rpm: f32| rect.left() + rect.width() * rpm_fraction(rpm, e.max_rpm);
    let red_from = x_at(e.shift_rpm);
    p.rect_filled(
        egui::Rect::from_min_max(egui::pos2(red_from, rect.top()), rect.right_bottom()),
        3.0,
        egui::Color32::from_rgb(90, 24, 22),
    );
    let fill = x_at(e.rpm);
    let colour = if e.rpm >= e.shift_rpm { RED } else { INK };
    p.rect_filled(egui::Rect::from_min_max(rect.left_top(), egui::pos2(fill, rect.bottom())), 3.0, colour);
    let mut k = 1000.0;
    while k < e.max_rpm {
        let x = x_at(k);
        p.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.top() + BAR_H * 0.4)],
            egui::Stroke::new(1.0, PANEL),
        );
        k += 1000.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gears_read_the_way_the_controller_counts_them() {
        assert_eq!(gear_label(0), "R");
        assert_eq!(gear_label(1), "N");
        assert_eq!(gear_label(2), "1");
        assert_eq!(gear_label(7), "6");
    }

    #[test]
    fn the_bar_never_leaves_its_box() {
        assert_eq!(rpm_fraction(3500.0, 7000.0), 0.5);
        assert_eq!(rpm_fraction(9000.0, 7000.0), 1.0);
        assert_eq!(rpm_fraction(-10.0, 7000.0), 0.0);
        assert_eq!(rpm_fraction(f32::NAN, 7000.0), 0.0);
        assert_eq!(rpm_fraction(3000.0, 0.0), 0.0);
    }
}
