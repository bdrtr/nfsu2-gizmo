//! Assembling a complete, material-grouped car from parsed parts.
//!
//! This ties [`crate::part_groups`] (which part goes in which group) to a look for each
//! group, the wheel-placement maths, and the GPU-mesh building in [`crate::mesh`]. The one
//! GPU-resource it does *not* own is the `Material`: the caller passes a factory closure
//! ([`build_car_visuals`]) turning a [`PbrLook`] into an engine `Material`, so texture and
//! bind-group setup stays in the app while the assembly stays here and testable in spirit.

use crate::mesh::{bbox, build_box, build_mesh};
use crate::part_groups::{group_of, select_stock_car, Grp};
use gizmo::prelude::*;
use gizmo_nfs::{AssetHash, NfsMeshPart, NfsTexture, Tpk};
use std::collections::HashMap;

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
}

impl PbrLook {
    const fn new(rgb: [f32; 3], roughness: f32, metallic: f32) -> Self {
        Self { rgb, roughness, metallic }
    }
}

/// The body material groups, their mesh labels, and their looks for a given signature
/// paint colour. Wheels are placed separately (see [`fit_wheel`]) and so are not listed.
#[must_use]
pub fn body_palette(paint: [f32; 3]) -> [(Grp, &'static str, PbrLook); 7] {
    [
        (Grp::Paint, "nfs_paint", PbrLook::new(paint, 0.30, 0.55)),
        (Grp::Glass, "nfs_glass", PbrLook::new([0.02, 0.03, 0.05], 0.08, 0.25)),
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

/// Wheel size and corner offsets derived from the wheel part's bounds and the car body.
#[derive(Clone, Copy, Debug)]
pub struct WheelFit {
    /// Wheel radius.
    pub radius: f32,
    /// Half the wheelbase (front↔rear corner offset along the car's length).
    pub half_wheelbase: f32,
    /// Half the track width (left↔right corner offset across the car).
    pub half_track: f32,
}

/// Fit the four wheel corners from one wheel part's bounds (`wmin`/`wmax`, Gizmo frame) and
/// the car `center`/`width`/`length`. The `max(...)` floors keep a sane stance even when a
/// car's single modelled wheel sits unusually close to the centreline.
#[must_use]
pub fn fit_wheel(wmin: Vec3, wmax: Vec3, center: Vec3, width: f32, length: f32) -> WheelFit {
    let wcenter = (wmin + wmax) * 0.5;
    WheelFit {
        radius: ((wmax.y - wmin.y).max(wmax.z - wmin.z) * 0.5).clamp(0.18, 0.55),
        half_wheelbase: (wcenter.z - center.z).abs().max(length * 0.30),
        half_track: (wcenter.x - center.x).abs().max(width * 0.46),
    }
}

/// Load and decode the `TEXTURES.BIN` sitting next to a car's `GEOMETRY.BIN`, if present.
/// Returns `None` (untextured car) when the file is absent or unparseable.
#[must_use]
pub fn load_tpk_beside(geometry_path: &str) -> Option<Tpk> {
    let dir = std::path::Path::new(geometry_path).parent()?;
    let bytes = std::fs::read(dir.join("TEXTURES.BIN")).ok()?;
    Tpk::parse(&bytes).ok()
}

/// Parse a `"r,g,b"` (each `0..1`) colour from an environment variable, else the default.
#[must_use]
pub fn env_color(var: &str, default: [f32; 3]) -> [f32; 3] {
    std::env::var(var)
        .ok()
        .and_then(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (v.len() == 3).then(|| [v[0], v[1], v[2]])
        })
        .unwrap_or(default)
}

/// One built body material group: its category, GPU mesh, and material.
pub struct GroupVisual {
    /// Which material group this mesh is.
    pub group: Grp,
    /// The merged GPU mesh for every part in the group, recentered to the car centre.
    pub mesh: Mesh,
    /// The material to render it with.
    pub material: Material,
}

/// A fully assembled car ready to spawn: the body groups, its dimensions, and the (single,
/// centre-relative) wheel mesh plus the fit describing where to instance it at four corners.
pub struct CarVisuals {
    /// Body material groups (paint, glass, …) for parts with no resolvable texture; never
    /// includes wheels.
    pub groups: Vec<GroupVisual>,
    /// Per-texture parts: parts whose `material_ref` resolved to a decoded TPK texture,
    /// merged by texture. The caller uploads each `texture` and builds a `Material` from it.
    pub textured: Vec<TexturedPart>,
    /// Car centre in the Gizmo frame (all meshes are recentered by this).
    pub center: Vec3,
    /// Body width (X).
    pub width: f32,
    /// Body height (Y).
    pub height: f32,
    /// Body length (Z).
    pub length: f32,
    /// The wheel mesh (recentered on its own hub) and its material, if the car models one.
    pub wheel: Option<(Mesh, Material)>,
    /// Wheel radius and corner offsets.
    pub wheel_fit: WheelFit,
    /// A dark box filling the cabin so the camera can't see through the glass-less window
    /// openings into the hollow body shell. Sized to sit inside the greenhouse; the caller
    /// gives it a near-black matte material.
    pub interior: Mesh,
}

/// A merged mesh sharing one decoded texture, ready for the caller to upload and material-ise.
pub struct TexturedPart {
    /// The merged, recentered GPU mesh (carries the parts' UVs).
    pub mesh: Mesh,
    /// The decoded RGBA8 texture to use as this mesh's albedo.
    pub texture: NfsTexture,
    /// Base-colour tint the texture is multiplied by: the paint colour for body panels
    /// (whose texture is a neutral detail atlas the paint shader would tint), white for
    /// detail parts (whose texture is their full colour).
    pub tint: [f32; 3],
    /// Suggested PBR roughness (from the part's material group).
    pub roughness: f32,
    /// Suggested PBR metallic (from the part's material group).
    pub metallic: f32,
}

/// Minimum shared-prefix length for a part name to match a texture's DebugName — long
/// enough to reach past the shared `CAR_KIT00_` prefix into the component word.
const NAME_MATCH_MIN: usize = 16;

fn common_prefix_len(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

/// The decoded texture whose DebugName best matches `part_name` (longest common prefix,
/// at least [`NAME_MATCH_MIN`] characters), or `None` if nothing matches closely.
fn texture_for_name<'a>(part_name: &str, tpk: &'a Tpk) -> Option<&'a NfsTexture> {
    tpk.textures
        .values()
        .map(|t| (common_prefix_len(part_name, &t.name), t))
        .filter(|(cpl, _)| *cpl >= NAME_MATCH_MIN)
        .max_by_key(|(cpl, _)| *cpl)
        .map(|(_, t)| t)
}

/// Roughness/metallic to pair with a texture for a given material group.
fn group_pbr(group: Grp) -> (f32, f32) {
    body_palette([0.0; 3])
        .into_iter()
        .find(|(g, _, _)| *g == group)
        .map(|(_, _, look)| (look.roughness, look.metallic))
        .unwrap_or((0.4, 0.2))
}

/// Assemble the default car's renderable visuals from parsed parts.
///
/// Selects the showroom configuration ([`select_stock_car`]), merges each material group
/// into one recentered mesh, and builds the single wheel mesh + [`WheelFit`] the caller
/// instances at four corners. `make_material` turns each group's [`PbrLook`] into an engine
/// `Material`, keeping GPU-texture ownership with the caller (and letting a viewer add
/// double-sided rendering while a driver does not).
pub fn build_car_visuals<F>(
    device: &wgpu::Device,
    all: &[NfsMeshPart],
    tpk: Option<&Tpk>,
    paint: [f32; 3],
    make_material: F,
) -> CarVisuals
where
    F: Fn(PbrLook) -> Material,
{
    let stock = select_stock_car(all);
    let body_like: Vec<&NfsMeshPart> =
        stock.iter().copied().filter(|p| group_of(&p.name) != Grp::Wheel).collect();
    let paint_parts: Vec<&NfsMeshPart> =
        body_like.iter().copied().filter(|p| group_of(&p.name) == Grp::Paint).collect();

    // Bounds/centre from the painted panels (fallback: all body parts).
    let (lo, hi) = bbox(if paint_parts.is_empty() { &body_like } else { &paint_parts });
    let center = (lo + hi) * 0.5;
    let (width, height, length) = (hi.x - lo.x, hi.y - lo.y, hi.z - lo.z);

    // Resolve each part to a texture. Primary link is by *name*: every texture carries a
    // DebugName (`240SX_KIT00_HEADLIGHT`, `240SX_KIT00_BRAKELIGHT`, …) that shares a long
    // prefix with the part it dresses — far more reliable than the material-hash list, which
    // for most detail parts holds a shader hash that indirects to the texture elsewhere. The
    // hash list is kept as a fallback. Painted body panels are excluded: their texture is a
    // shared detail atlas that only reads right through the paint shader, and in-game the
    // body is the player's flat paint colour anyway.
    let resolve = |p: &NfsMeshPart| -> Option<AssetHash> {
        if group_of(&p.name) == Grp::Paint {
            return None;
        }
        let tpk = tpk?;
        if let Some(tex) = texture_for_name(&p.name, tpk) {
            return Some(tex.hash);
        }
        p.material_refs.iter().copied().find(|h| tpk.texture(*h).is_some())
    };
    let mut by_texture: HashMap<AssetHash, Vec<&NfsMeshPart>> = HashMap::new();
    let mut untextured: Vec<&NfsMeshPart> = Vec::new();
    for p in body_like.iter().copied() {
        match resolve(p) {
            Some(hash) => by_texture.entry(hash).or_default().push(p),
            None => untextured.push(p),
        }
    }

    let mut groups = Vec::new();
    for (group, label, look) in body_palette(paint) {
        let parts: Vec<&NfsMeshPart> =
            untextured.iter().copied().filter(|p| group_of(&p.name) == group).collect();
        if let Some(mesh) = build_mesh(device, &parts, center, label) {
            groups.push(GroupVisual { group, mesh, material: make_material(look) });
        }
    }

    let mut textured = Vec::new();
    for (hash, parts) in by_texture {
        let Some(mesh) = build_mesh(device, &parts, center, "nfs_textured") else { continue };
        let Some(texture) = tpk.and_then(|t| t.texture(hash)).cloned() else { continue };
        let group = group_of(&parts[0].name);
        let (roughness, metallic) = group_pbr(group);
        // Body panels carry a neutral detail atlas → tint by paint; details are their own
        // colour → white.
        let tint = if group == Grp::Paint { paint } else { [1.0, 1.0, 1.0] };
        textured.push(TexturedPart { mesh, texture, tint, roughness, metallic });
    }

    let mut wheel = None;
    let mut wheel_fit =
        WheelFit { radius: 0.31, half_wheelbase: length * 0.36, half_track: width * 0.42 };
    if let Some(wp) = stock.iter().copied().find(|p| group_of(&p.name) == Grp::Wheel) {
        let (wl, wh) = bbox(std::slice::from_ref(&wp));
        wheel_fit = fit_wheel(wl, wh, center, width, length);
        if let Some(mesh) = build_mesh(device, &[wp], (wl + wh) * 0.5, "nfs_wheel") {
            wheel = Some((mesh, make_material(wheel_look())));
        }
    }

    // A dark cabin filler occupying the greenhouse cavity (recentered frame: origin = car
    // centre, +Y = up). Spans from just below the beltline up toward the roof, narrow enough
    // to stay inside the pillars so it only shows through the window openings.
    let interior = build_box(
        device,
        Vec3::new(-width * 0.27, -height * 0.05, -length * 0.14),
        Vec3::new(width * 0.27, height * 0.34, length * 0.13),
        "nfs_interior",
    );

    CarVisuals { groups, textured, center, width, height, length, wheel, wheel_fit, interior }
}
