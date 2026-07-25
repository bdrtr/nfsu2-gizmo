//! The "Modernist" design tokens, carried over to egui.
//!
//! The values here are the design system's own — the ramps were generated in OKLCH on one shared
//! lightness scale, so a step of any role matches the others in visual value; don't retune them
//! one at a time. Two properties of that system make the port unusually faithful: **every radius
//! is 0** and surfaces are flat with 2 px dividers, so nothing depends on CSS gradients or
//! shadows.
//!
//! The one thing egui cannot do is `letter-spacing`, which the design uses on its small uppercase
//! labels. Those are separated by size and colour here instead.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Style, Visuals};

/// Parse a `#rrggbb` literal at compile-ish time — keeps the palette below readable as the CSS it
/// came from.
const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// The design tokens, verbatim from the system's `styles.css`.
///
/// The whole ramp is here even where no panel has reached for a step yet: the values are one
/// generated scale, and pruning it to "what is used today" would make the next screen guess.
#[allow(dead_code)]
pub mod token {
    use super::{hex, Color32};

    pub const BG: Color32 = hex(0xf3f2f2);
    pub const SURFACE: Color32 = hex(0xeae9e9);
    pub const TEXT: Color32 = hex(0x201e1d);
    pub const ACCENT: Color32 = hex(0xec3013);
    pub const ACCENT_2: Color32 = hex(0xe15b47);

    pub const NEUTRAL_100: Color32 = hex(0xf8f4f4);
    pub const NEUTRAL_200: Color32 = hex(0xeae7e7);
    pub const NEUTRAL_300: Color32 = hex(0xd7d3d3);
    pub const NEUTRAL_400: Color32 = hex(0xbab6b6);
    pub const NEUTRAL_500: Color32 = hex(0x9b9797);
    pub const NEUTRAL_600: Color32 = hex(0x7d7979);
    pub const NEUTRAL_700: Color32 = hex(0x605d5d);
    pub const NEUTRAL_800: Color32 = hex(0x444141);
    pub const NEUTRAL_900: Color32 = hex(0x2d2b2b);

    pub const ACCENT_100: Color32 = hex(0xfff2ef);
    pub const ACCENT_200: Color32 = hex(0xffe0d9);
    pub const ACCENT_300: Color32 = hex(0xffc4b8);
    pub const ACCENT_400: Color32 = hex(0xff9783);
    pub const ACCENT_500: Color32 = hex(0xff563c);
    pub const ACCENT_600: Color32 = hex(0xdd2b0f);
    pub const ACCENT_700: Color32 = hex(0xae1800);
    pub const ACCENT_800: Color32 = hex(0x7c1405);

    /// `--color-divider` is the ink at 40 % *over whatever is behind it*. A translucent stroke
    /// would land on two different colours over `BG` and `SURFACE`, and egui's premultiplied
    /// blend does not reproduce a browser's anyway — so both are baked.
    pub const DIVIDER_ON_BG: Color32 = hex(0x9f9d9d);
    pub const DIVIDER_ON_SURFACE: Color32 = hex(0x999898);
    /// The one to reach for when the background is the page itself.
    pub const DIVIDER: Color32 = DIVIDER_ON_BG;

    /// The design's spacing scale (`--space-1` … `--space-8`).
    pub const SPACE_1: f32 = 4.0;
    pub const SPACE_2: f32 = 8.0;
    pub const SPACE_3: f32 = 12.0;
    pub const SPACE_4: f32 = 16.0;
}

/// `--color-text` mixed with transparency, the way the design's `color-mix(… N%, transparent)`
/// reads: a muted ink rather than a separate grey.
#[must_use]
pub fn muted(percent: u8) -> Color32 {
    token::TEXT.gamma_multiply(f32::from(percent) / 100.0)
}

/// How tightly the interface is packed. The design ships this as a user setting, and in egui it is
/// a cleaner switch than in CSS: one spacing struct drives every panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Density {
    /// The design's default — a data tool is read, not browsed.
    #[default]
    Compact,
    Balanced,
    Roomy,
}

impl Density {
    /// Row height for the dense list panels (tree, hex, log).
    #[must_use]
    pub fn row_height(self) -> f32 {
        match self {
            Self::Compact => 17.0,
            Self::Balanced => 20.0,
            Self::Roomy => 24.0,
        }
    }

    /// Base body size; every other size in the app is derived from the design's own scale.
    #[must_use]
    pub fn body_size(self) -> f32 {
        match self {
            Self::Compact => 12.5,
            Self::Balanced => 13.5,
            Self::Roomy => 15.0,
        }
    }

    /// Padding inside a panel, from the design's spacing scale.
    #[must_use]
    pub fn pad(self) -> f32 {
        match self {
            Self::Compact => token::SPACE_1,
            Self::Balanced => token::SPACE_2,
            Self::Roomy => token::SPACE_3,
        }
    }

    /// The next setting, for a cycling button.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Compact => Self::Balanced,
            Self::Balanced => Self::Roomy,
            Self::Roomy => Self::Compact,
        }
    }

    /// The single-letter badge the design's density switch shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "S",
            Self::Balanced => "M",
            Self::Roomy => "L",
        }
    }
}

/// Named text roles, so a panel asks for "the tree's monospace" rather than a number.
pub mod font {
    use egui::{FontFamily, FontId};

    /// The heading family (Archivo ExtraBold) — the design's signature weight.
    pub fn heading_family() -> FontFamily {
        FontFamily::Name("heading".into())
    }

    /// A heading of `size` px.
    pub fn heading(size: f32) -> FontId {
        FontId::new(size, heading_family())
    }

    /// Monospace, for everything that is bytes, offsets or identifiers.
    pub fn mono(size: f32) -> FontId {
        FontId::new(size, FontFamily::Monospace)
    }

    /// Body text of `size` px.
    pub fn body(size: f32) -> FontId {
        FontId::new(size, FontFamily::Proportional)
    }
}

/// Status marks, drawn rather than typed.
///
/// `⚠` exists in the bundled fallbacks but `✓`/`✕` do not, and a missing glyph renders as a tofu
/// box — on a validation screen, the one mark that must be unmistakable. Two line segments are
/// cheaper than shipping another font.
pub mod mark {
    use super::token;
    use egui::{Color32, Painter, Pos2, Stroke, Vec2};

    /// A tick centred on `at`, `size` across.
    pub fn ok(p: &Painter, at: Pos2, size: f32, colour: Color32) {
        let s = size * 0.5;
        let stroke = Stroke::new((size * 0.16).max(1.2), colour);
        let a = at + Vec2::new(-s, 0.0);
        let b = at + Vec2::new(-s * 0.25, s * 0.7);
        let c = at + Vec2::new(s, -s * 0.75);
        p.line_segment([a, b], stroke);
        p.line_segment([b, c], stroke);
    }

    /// A cross centred on `at`.
    pub fn error(p: &Painter, at: Pos2, size: f32, colour: Color32) {
        let s = size * 0.42;
        let stroke = Stroke::new((size * 0.16).max(1.2), colour);
        p.line_segment([at + Vec2::new(-s, -s), at + Vec2::new(s, s)], stroke);
        p.line_segment([at + Vec2::new(-s, s), at + Vec2::new(s, -s)], stroke);
    }

    /// A hollow ring — "nothing read here", which is not a pass.
    pub fn unchecked(p: &Painter, at: Pos2, size: f32) {
        p.circle_stroke(at, size * 0.34, Stroke::new(1.0_f32, token::NEUTRAL_400));
    }
}

/// Install Archivo (bundled, SIL OFL) as the proportional and heading families.
///
/// The fonts are compiled into the binary so the tool looks the same on a machine that has never
/// heard of Archivo; if egui ever fails to parse them it keeps its own defaults rather than
/// refusing to start.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "archivo".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!("../assets/Archivo-Regular.ttf"))),
    );
    fonts.font_data.insert(
        "archivo-bold".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!("../assets/Archivo-ExtraBold.ttf"))),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "archivo".to_owned());
    fonts
        .families
        .insert(FontFamily::Name("heading".into()), vec!["archivo-bold".to_owned()]);
    ctx.set_fonts(fonts);
}

/// Apply the design system to a context: palette, zero radii, hairline strokes, and the text
/// sizes for the given density.
pub fn apply(ctx: &egui::Context, density: Density) {
    let mut visuals = Visuals::light();

    visuals.panel_fill = token::BG;
    visuals.window_fill = token::SURFACE;
    visuals.extreme_bg_color = token::SURFACE;
    visuals.faint_bg_color = token::NEUTRAL_200;
    visuals.override_text_color = Some(token::TEXT);
    visuals.hyperlink_color = token::ACCENT;
    visuals.selection.bg_fill = token::ACCENT.gamma_multiply(0.25);
    visuals.selection.stroke = Stroke::new(1.0_f32, token::ACCENT);
    visuals.window_stroke = Stroke::new(2.0_f32, token::DIVIDER);

    // Every radius in the system is 0 — the flat, squared-off look is the design's signature.
    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = CornerRadius::ZERO;
    }
    visuals.window_corner_radius = CornerRadius::ZERO;
    visuals.menu_corner_radius = CornerRadius::ZERO;

    visuals.widgets.noninteractive.bg_fill = token::BG;
    visuals.widgets.noninteractive.weak_bg_fill = token::BG;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, token::DIVIDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, token::TEXT);

    visuals.widgets.inactive.bg_fill = token::SURFACE;
    visuals.widgets.inactive.weak_bg_fill = token::SURFACE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, token::DIVIDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, token::TEXT);

    // Hover is the design's `color-mix(text 7%)` wash, not a colour change.
    visuals.widgets.hovered.bg_fill = token::NEUTRAL_200;
    visuals.widgets.hovered.weak_bg_fill = token::NEUTRAL_200;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, token::ACCENT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, token::TEXT);

    visuals.widgets.active.bg_fill = token::ACCENT;
    visuals.widgets.active.weak_bg_fill = token::ACCENT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, token::ACCENT_700);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, token::BG);

    let mut style = Style { visuals, ..Default::default() };
    let body = density.body_size();
    style.text_styles = [
        (egui::TextStyle::Heading, font::heading(19.0)),
        (egui::TextStyle::Body, font::body(body)),
        (egui::TextStyle::Monospace, font::mono(body - 0.5)),
        (egui::TextStyle::Button, font::body(body)),
        (egui::TextStyle::Small, font::body(body - 2.0)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(token::SPACE_2, density.pad());
    style.spacing.button_padding = egui::vec2(token::SPACE_2, density.pad());
    style.spacing.window_margin = egui::Margin::same(0);
    style.spacing.interact_size.y = density.row_height();
    style.spacing.scroll.bar_width = 9.0; // the design's own scrollbar width
    ctx.set_global_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_cycles_through_every_setting() {
        let mut d = Density::Compact;
        let mut seen = vec![d];
        for _ in 0..2 {
            d = d.next();
            seen.push(d);
        }
        assert_eq!(seen, [Density::Compact, Density::Balanced, Density::Roomy]);
        assert_eq!(d.next(), Density::Compact, "the cycle closes");
    }

    #[test]
    fn denser_settings_are_actually_denser() {
        assert!(Density::Compact.row_height() < Density::Balanced.row_height());
        assert!(Density::Balanced.row_height() < Density::Roomy.row_height());
        assert!(Density::Compact.body_size() < Density::Roomy.body_size());
    }

    #[test]
    fn muted_ink_stays_on_the_text_hue() {
        // The design mixes the ink with transparency rather than switching to a grey, so a muted
        // label must keep the text colour's own hue.
        let m = muted(55);
        assert_eq!((m.r() > m.b(), m.g() > m.b()), (true, true));
        assert!(m.a() < token::TEXT.a() || m != token::TEXT);
    }
}
