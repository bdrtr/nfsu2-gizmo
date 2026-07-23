//! Assembling a complete, material-grouped car from parsed parts.
//!
//! This ties [`crate::part_groups`] (which part goes in which group) to a look for each
//! group, the wheel-placement maths, and the GPU-mesh building in [`crate::mesh`]. The one
//! GPU-resource it does *not* own is the `Material`: the caller passes a factory closure
//! ([`build_car_visuals`]) turning a [`PbrLook`] into an engine `Material`, so texture and
//! bind-group setup stays in the app while the assembly stays here and testable in spirit.

use crate::mesh::{bbox, build_box, build_mesh, build_mesh_items};
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

/// How to surface the (four-times-instanced) wheel mesh.
pub enum WheelSurface {
    /// A pre-built flat material — dark rubber, for a wheel with no resolvable texture.
    Flat(Material),
    /// The tire/rim texture (its UVs cover both). The caller uploads it, once, then instances
    /// the wheel at the four corners with the resulting material.
    Textured(NfsTexture),
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
        half_track: (wcenter.x - center.x).abs().max(width * 0.40),
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
    /// The wheel mesh (recentered on its own hub) and how to surface it (flat rubber or the
    /// tire/rim texture), if the car models a wheel. The caller instances it at four corners.
    pub wheel: Option<(Mesh, WheelSurface)>,
    /// Wheel radius and corner offsets.
    pub wheel_fit: WheelFit,
    /// A dark box filling the cabin so the camera can't see through the glass-less window
    /// openings into a hollow body shell — `None` when the car models its own interior (that
    /// geometry is textured instead), so a heuristic filler box doesn't poke out through the
    /// roof. The caller gives it a near-black matte material.
    pub interior: Option<Mesh>,
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
        // Break prefix-length ties deterministically. NFSU2 ships lens textures in pairs that
        // share one DebugName (the two `240SX_KIT00_BRAKELIGHT_` maps, or `_HEADLIGHT_G`/`_O`)
        // alongside `_MASK` alpha companions; plain `max_by_key(cpl)` otherwise let `HashMap`
        // iteration order pick one at random per run. Prefer a non-`_MASK` map, then the larger
        // image (the detailed diffuse over its lower-res companion), then the lowest hash so the
        // pick is fully stable across runs.
        .max_by_key(|(cpl, t)| {
            (*cpl, !t.name.ends_with("_MASK"), t.width * t.height, std::cmp::Reverse(t.hash.0))
        })
        .map(|(_, t)| t)
}

/// Resolve a whole part (one carrying no `0x00134B02` material list) to a texture the old
/// way: painted panels stay flat, else match by DebugName prefix, then fall back to the
/// material-hash list. Parts *with* a material list are resolved per-run in
/// [`build_car_visuals`] instead.
fn resolve_whole(p: &NfsMeshPart, grp: Grp, tpk: &Tpk) -> Option<AssetHash> {
    if grp == Grp::Paint {
        return None;
    }
    if let Some(tex) = texture_for_name(&p.name, tpk) {
        return Some(tex.hash);
    }
    p.material_refs.iter().copied().find(|h| tpk.texture(*h).is_some())
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

    // Split every body part into its material runs (`0x00134B02`) and route each run to a
    // texture — when its hash resolves to a decoded TPK image (headlights, brake lights,
    // interior, badging) — or to its part's flat colour group. This is what turns a mod that
    // bakes a whole car into one BODY mesh (split only by material) into textured light lenses
    // instead of one flat panel. Body paint, glass and chrome carry shader-only hashes that
    // never resolve, so they fall through to their group. A part with no material list is one
    // run resolved the old way (by DebugName, then material-hash; paint stays flat).
    let mut by_texture: HashMap<AssetHash, Vec<(&NfsMeshPart, &[u32])>> = HashMap::new();
    let mut by_group: HashMap<Grp, Vec<(&NfsMeshPart, &[u32])>> = HashMap::new();
    for p in body_like.iter().copied() {
        let grp = group_of(&p.name);
        if p.materials.is_empty() {
            match tpk.and_then(|t| resolve_whole(p, grp, t)) {
                Some(h) => by_texture.entry(h).or_default().push((p, p.indices.as_slice())),
                None => by_group.entry(grp).or_default().push((p, p.indices.as_slice())),
            }
            continue;
        }
        for m in &p.materials {
            let Some(slice) = p.indices.get(m.index_offset..m.index_offset + m.index_count) else {
                continue;
            };
            match tpk.and_then(|t| t.texture(m.hash)).map(|_| m.hash) {
                Some(h) => by_texture.entry(h).or_default().push((p, slice)),
                None => by_group.entry(grp).or_default().push((p, slice)),
            }
        }
    }

    // Flat-colour group meshes. Sink the shared BASE shell a few mm behind the kit panels: the
    // two model the same greenhouse belt near-coplanar and would otherwise z-fight.
    let mut groups = Vec::new();
    for (group, label, look) in body_palette(paint) {
        let Some(items) = by_group.get(&group) else { continue };
        let mesh = build_mesh_items(
            device,
            items,
            center,
            |p| if p.name.contains("_BASE") { -0.006 } else { 0.0 },
            label,
        );
        if let Some(mesh) = mesh {
            groups.push(GroupVisual { group, mesh, material: make_material(look) });
        }
    }

    // One textured mesh per resolved texture. A run's texture is its own full colour, so it is
    // used white (never the paint tint the old whole-part body-atlas path applied).
    let mut textured = Vec::new();
    for (hash, items) in by_texture {
        let Some(mesh) = build_mesh_items(device, &items, center, |_| 0.0, "nfs_textured") else {
            continue;
        };
        let Some(texture) = tpk.and_then(|t| t.texture(hash)).cloned() else { continue };
        let (roughness, metallic) = group_pbr(group_of(&items[0].0.name));
        textured.push(TexturedPart { mesh, texture, tint: [1.0, 1.0, 1.0], roughness, metallic });
    }

    let mut wheel = None;
    let mut wheel_fit =
        WheelFit { radius: 0.31, half_wheelbase: length * 0.36, half_track: width * 0.42 };
    if let Some(wp) = stock.iter().copied().find(|p| group_of(&p.name) == Grp::Wheel) {
        let (wl, wh) = bbox(std::slice::from_ref(&wp));
        wheel_fit = fit_wheel(wl, wh, center, width, length);
        // The wheel's texture is its first material run that resolves to a TPK image — the tire
        // + rim atlas, whose UVs cover both. The whole wheel is one mesh; a small untextured hub
        // run just samples the same atlas, which reads fine.
        let wheel_tex = wp.materials.iter().find_map(|m| tpk.and_then(|t| t.texture(m.hash)).cloned());
        if let Some(mesh) = build_mesh(device, &[wp], (wl + wh) * 0.5, "nfs_wheel") {
            let surface = match wheel_tex {
                Some(tex) => WheelSurface::Textured(tex),
                None => WheelSurface::Flat(make_material(wheel_look())),
            };
            wheel = Some((mesh, surface));
        }
    }

    // A dark cabin filler occupying the greenhouse cavity (recentered frame: origin = car
    // centre, +Y = up). Only for hollow shells: skip it when the car models a real interior
    // (its geometry is textured above), which the heuristic box would otherwise poke through.
    let has_interior = textured.iter().any(|tp| tp.texture.name.contains("INTERIOR"));
    let interior = (!has_interior).then(|| {
        build_box(
            device,
            Vec3::new(-width * 0.27, -height * 0.05, -length * 0.14),
            Vec3::new(width * 0.27, height * 0.34, length * 0.13),
            "nfs_interior",
        )
    });

    CarVisuals { groups, textured, center, width, height, length, wheel, wheel_fit, interior }
}
