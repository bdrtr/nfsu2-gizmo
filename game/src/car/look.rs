//! What each material group looks like: the PBR parameters the app turns into a `Material`.
//!
//! Pure data — no engine types — so the palette can be read and reasoned about on its own.

use crate::parts::Grp;

/// A physically-based surface look for one material group: base colour plus roughness and
/// metallic in `0..1`. The caller turns this into an engine `Material`.
#[derive(Clone, Copy, Debug)]
pub struct PbrLook {
    /// Linear RGB base colour.
    pub rgb: [f32; 3],
    /// Surface roughness (0 = mirror, 1 = fully diffuse).
    pub roughness: f32,
    /// Metalness (0 = dielectric, 1 = metal).
    pub metallic: f32,
    /// Base-colour alpha (`< 1.0` marks the group transparent — used for glass so the
    /// greenhouse blends over the interior instead of stacking opaque panels that z-fight).
    pub alpha: f32,
}

impl PbrLook {
    const fn new(rgb: [f32; 3], roughness: f32, metallic: f32) -> Self {
        Self { rgb, roughness, metallic, alpha: 1.0 }
    }
    const fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }
}

/// The body material groups, their mesh labels, and their looks for a given signature
/// paint colour. Wheels are placed separately (see [`fit_wheel`]) and so are not listed.
#[must_use]
pub fn body_palette(paint: [f32; 3]) -> [(Grp, &'static str, PbrLook); 7] {
    [
        (Grp::Paint, "nfs_paint", PbrLook::new(paint, 0.30, 0.55)),
        // Transparent, lightly-tinted, low-roughness glass: routed to the engine's forward
        // blend pass so overlapping window panels alpha-blend (soft) instead of opaque-z-fight
        // (the "greenhouse shards"), and the dark interior reads through it as it should.
        (Grp::Glass, "nfs_glass", PbrLook::new([0.05, 0.07, 0.10], 0.08, 0.0).with_alpha(0.32)),
        (Grp::Chrome, "nfs_chrome", PbrLook::new([0.80, 0.82, 0.85], 0.12, 1.00)),
        (Grp::Headlight, "nfs_head", PbrLook::new([0.90, 0.92, 0.96], 0.10, 0.30)),
        (Grp::Brakelight, "nfs_brake", PbrLook::new([0.72, 0.03, 0.03], 0.25, 0.10)),
        (Grp::Exhaust, "nfs_exhaust", PbrLook::new([0.55, 0.56, 0.60], 0.22, 0.95)),
        (Grp::Trim, "nfs_trim", PbrLook::new([0.05, 0.05, 0.06], 0.55, 0.20)),
    ]
}

/// The default dark-rubber look for the wheel mesh.
#[must_use]
pub fn wheel_look() -> PbrLook {
    PbrLook::new([0.09, 0.09, 0.10], 0.70, 0.20)
}

/// Roughness/metallic to pair with a texture for a given material group.
pub(crate) fn group_pbr(group: Grp) -> (f32, f32) {
    body_palette([0.0; 3])
        .into_iter()
        .find(|(g, _, _)| *g == group)
        .map(|(_, _, look)| (look.roughness, look.metallic))
        .unwrap_or((0.4, 0.2))
}
