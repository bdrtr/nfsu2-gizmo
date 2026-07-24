//! Assembling a complete, material-grouped car from parsed parts.
//!
//! This is where the pieces meet: [`crate::parts`] decides *which* parts make up the car and
//! which group each belongs to, [`look`] says what a group looks like, [`shader`] routes a
//! material run, [`skin`] resolves its texture, [`wheel`] sizes the wheels, and
//! [`crate::geom`] turns the result into GPU meshes.
//!
//! The one GPU resource this does *not* own is the `Material`: the caller passes a factory
//! closure ([`build_car_visuals`]) turning a [`PbrLook`] into an engine `Material`, so texture
//! and bind-group setup stays in the app while the assembly stays here.

mod look;
mod shader;
mod skin;
mod wheel;

pub use look::{body_palette, wheel_look, PbrLook};
pub use skin::composite_over_paint;
pub use wheel::{fit_wheel, wheel_mount, WheelFit, WheelSurface};

use crate::geom::{bbox, build_mesh, build_mesh_items};
use crate::parts::{group_of, select_car, CarConfig, Grp};
use gizmo::prelude::*;
use gizmo_nfs::{AssetHash, NfsMeshPart, NfsTexture, Tpk};
use look::group_pbr;
use shader::{hash, is_dropped_filler, shader_group};
use skin::{doorline_texture, resolve_whole};
use std::collections::HashMap;

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
    /// When `Some(paint)`, this texture is a **body-detail overlay** (the doorline: panel gaps,
    /// door handles and creases on an otherwise transparent field). The caller must alpha-
    /// composite it over `paint` — via [`composite_over_paint`] — before upload, so the bare
    /// paint shows through where the overlay is transparent. `None` for a detail texture that is
    /// its own full colour (light lenses, tyres, badging).
    pub composite_over: Option<[f32; 3]>,
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
    cfg: &CarConfig,
    make_material: F,
) -> CarVisuals
where
    F: Fn(PbrLook) -> Material,
{
    let stock = select_car(all, cfg);
    // When a kit or widebody body provides the outer paint, BASE's painted panels are inner
    // structure (firewall, inner fenders) or a redundant shell that only pokes out through the
    // skin. Keyed off the *selected* body (KIT00 or a KITW## widebody), not a literal KIT00.
    let has_kit_body = stock.iter().any(|p| p.name.contains("_BODY") && !p.name.contains("_BASE"));
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
    // The body-detail (doorline) texture, applied to the CARSKIN paint runs by naming
    // convention. When present, paint runs are collected here (carrying their UVs) and emitted
    // as one composited textured mesh instead of a flat colour; when absent (untextured car),
    // they stay flat as before.
    let doorline = tpk.and_then(|t| doorline_texture(t, has_kit_body));
    let mut paint_detail: Vec<(&NfsMeshPart, &[u32])> = Vec::new();

    let mut by_texture: HashMap<AssetHash, Vec<(&NfsMeshPart, &[u32])>> = HashMap::new();
    let mut by_group: HashMap<Grp, Vec<(&NfsMeshPart, &[u32])>> = HashMap::new();
    for p in body_like.iter().copied() {
        let grp = group_of(&p.name);
        if p.materials.is_empty() {
            if grp == Grp::Paint && doorline.is_some() {
                paint_detail.push((p, p.indices.as_slice()));
                continue;
            }
            match tpk.and_then(|t| resolve_whole(p, grp, t)) {
                Some(h) => by_texture.entry(h).or_default().push((p, p.indices.as_slice())),
                None => by_group.entry(grp).or_default().push((p, p.indices.as_slice())),
            }
            continue;
        }
        let base_paint = has_kit_body && p.name.contains("_BASE");
        for m in &p.materials {
            let Some(slice) = p.indices.get(m.index_offset..m.index_offset + m.index_count) else {
                continue;
            };
            // Skip BASE's painted runs when the kit body supplies the paint: they are inner
            // structure that only shows as jagged slivers poking through the outer skin.
            if base_paint && m.shader.0 == hash::CARSKIN {
                continue;
            }
            // PLAINNOTHING filler juts out as a black square on kit bodies — drop it there,
            // keep it on BASE where it is the interior tub (see `is_dropped_filler`).
            if is_dropped_filler(m.shader.0, p.name.contains("_BASE")) {
                continue;
            }
            // The shader decides the run's type. Glass/chrome/interior/trim/paint runs render
            // as their flat group — this is what turns BASE's greenhouse (glass, window frames,
            // seats, mouldings) into proper dark glass + trim instead of a bright painted mess.
            if let Some(g) = shader_group(m.shader.0) {
                // Paint runs get the doorline overlay when the car ships one; else flat colour.
                if g == Grp::Paint && doorline.is_some() {
                    paint_detail.push((p, slice));
                } else {
                    by_group.entry(g).or_default().push((p, slice));
                }
                continue;
            }
            // Everything else (head/brake-light lenses, tyres, badging) carries its own texture.
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
        textured.push(TexturedPart {
            mesh,
            texture,
            tint: [1.0, 1.0, 1.0],
            roughness,
            metallic,
            composite_over: None,
        });
    }

    // The painted body as one textured mesh: the doorline overlay composited over the paint by
    // the caller. Emitted only when the car ships a doorline (else paint stayed in `by_group`).
    if let Some(dl) = doorline {
        if let Some(mesh) = build_mesh_items(device, &paint_detail, center, |_| 0.0, "nfs_paint_detail") {
            let (roughness, metallic) = group_pbr(Grp::Paint);
            textured.push(TexturedPart {
                mesh,
                texture: dl.clone(),
                tint: [1.0, 1.0, 1.0],
                roughness,
                metallic,
                composite_over: Some(paint),
            });
        }
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
        // The parser already normalised the wheel's triangle strips to a clean list, so the whole
        // wheel builds as one mesh (its runs all share the one tire/rim atlas).
        if let Some(mesh) = build_mesh(device, &[wp], (wl + wh) * 0.5, "nfs_wheel") {
            let surface = match wheel_tex {
                Some(tex) => WheelSurface::Textured(tex),
                None => WheelSurface::Flat(make_material(wheel_look())),
            };
            wheel = Some((mesh, surface));
        }
    }

    // No cabin filler: BASE now supplies the real interior geometry (routed per-shader) and the
    // windscreen glass covers the openings, so the old hollow-shell box is obsolete.
    let interior: Option<Mesh> = None;

    CarVisuals { groups, textured, center, width, height, length, wheel, wheel_fit, interior }
}
