//! Assembling a complete, material-grouped car from parsed parts.
//!
//! This ties [`crate::part_groups`] (which part goes in which group) to a look for each
//! group, the wheel-placement maths, and the GPU-mesh building in [`crate::mesh`]. The one
//! GPU-resource it does *not* own is the `Material`: the caller passes a factory closure
//! ([`build_car_visuals`]) turning a [`PbrLook`] into an engine `Material`, so texture and
//! bind-group setup stays in the app while the assembly stays here and testable in spirit.

use crate::mesh::{bbox, build_mesh, build_mesh_items};
use crate::part_groups::{group_of, select_car, CarConfig, Grp};
use gizmo::prelude::*;
use gizmo_nfs::{AssetHash, CarTypeInfo, NfsMeshPart, NfsTexture, Tpk, WheelSpec};
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

/// Load this car's [`CarTypeInfo`] (exact wheel mounts, radius, mass) from the game's global
/// bundle, resolved relative to a `CARS/<name>/GEOMETRY.BIN` path: up two directories to the
/// game root, then `GLOBAL/GLOBALB.BUN`, looked up by the car's folder name. `None` if the
/// bundle is missing or the car isn't listed.
#[must_use]
pub fn load_cartypeinfo_beside(geometry_path: &str) -> Option<CarTypeInfo> {
    let geo = std::path::Path::new(geometry_path);
    let car_dir = geo.parent()?; // CARS/<name>
    let name = car_dir.file_name()?.to_str()?;
    let root = car_dir.parent()?.parent()?; // up past CARS/
    let raw = std::fs::read(root.join("GLOBAL").join("GLOBALB.BUN")).ok()?;
    let bytes = match gizmo_nfs::compression::detect(&raw) {
        gizmo_nfs::compression::Codec::None => raw,
        _ => gizmo_nfs::compression::decompress(&raw).ok()?,
    };
    gizmo_nfs::globalb::find_car(&bytes, name)
}

/// Map a [`WheelSpec`] mount (NFSU2 car space: fore/aft, lateral, ride-height) into the Gizmo
/// frame, recentered by the car `center` — the exact position to instance the wheel mesh at.
/// Uses the same axis remap as the body mesh so wheels and body share one frame.
#[must_use]
pub fn wheel_mount(w: &WheelSpec, center: Vec3) -> Vec3 {
    crate::mesh::remap([w.fore_aft, w.lateral, w.ride_height]) - center
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
    /// When `Some(paint)`, this texture is a **body-detail overlay** (the doorline: panel gaps,
    /// door handles and creases on an otherwise transparent field). The caller must alpha-
    /// composite it over `paint` — via [`composite_over_paint`] — before upload, so the bare
    /// paint shows through where the overlay is transparent. `None` for a detail texture that is
    /// its own full colour (light lenses, tyres, badging).
    pub composite_over: Option<[f32; 3]>,
}

/// Encode a linear `0..1` channel to an 8-bit sRGB value.
#[inline]
fn linear_to_srgb_u8(c: f32) -> u8 {
    let s = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Alpha-composite a body-detail overlay (`overlay`: doorline panel lines/handles on a
/// transparent field, RGBA8) over a solid `paint` colour, returning ready-to-upload opaque RGBA8
/// the same size. Transparent texels become the paint; opaque texels keep the overlay. The paint
/// is written in sRGB so an sRGB-sampling albedo texture brings it back to the intended linear
/// colour.
#[must_use]
pub fn composite_over_paint(overlay: &[u8], paint: [f32; 3]) -> Vec<u8> {
    let ps = [linear_to_srgb_u8(paint[0]), linear_to_srgb_u8(paint[1]), linear_to_srgb_u8(paint[2])];
    let mut out = Vec::with_capacity(overlay.len());
    for px in overlay.chunks_exact(4) {
        let a = px[3] as f32 / 255.0;
        for k in 0..3 {
            out.push((px[k] as f32 * a + ps[k] as f32 * (1.0 - a)).round() as u8);
        }
        out.push(255);
    }
    out
}

/// The car's body-detail (**doorline**) texture — panel gaps, handles and creases UV-mapped
/// across the paint. It is referenced by naming convention (`<CAR>_DOORLINE`), not by a geometry
/// material hash, so it is resolved by name here. Prefer the kit-body variant when a KIT00 body
/// supplies the outer paint, else the base `_DOORLINE`.
fn doorline_texture(tpk: &Tpk, has_kit_body: bool) -> Option<&NfsTexture> {
    let pick = |suffix: &str| {
        tpk.textures
            .values()
            .filter(|t| t.name.ends_with(suffix))
            // TPK DebugNames are truncated to 23 characters, so on a long car name the `_MASK`
            // companion loses its tail and ends with the same stem as the map itself
            // (`LANCEREVO8_DOORLINE_KIT_MASK` → `LANCEREVO8_DOORLINE_KIT`). A mask is fully
            // opaque and the map is mostly transparent, so pick the *most transparent* candidate
            // — with the hash as a deterministic tie-break, since `textures` is a `HashMap` whose
            // order varies per run (which made the Lancer and the Impreza render black at random).
            .min_by_key(|t| (opaque_permille(t), t.hash.0))
            // A full-coverage map is a mask or an undecoded format, not a detail overlay:
            // compositing it paints the whole car its own (near-black) colour, as it did on the
            // IS300, whose real `_DOORLINE` is opaque too. Better no detail than a black car.
            .filter(|t| opaque_permille(t) < OVERLAY_MAX_OPAQUE_PERMILLE)
    };
    if has_kit_body {
        pick("_DOORLINE_KIT").or_else(|| pick("_DOORLINE"))
    } else {
        pick("_DOORLINE")
    }
}

/// Opacity of a texture in ‰ of texels — how much of the paint an overlay would cover. Real
/// doorline maps sit at 100–200‰ (thin panel gaps on a transparent field); masks are 1000‰.
fn opaque_permille(t: &NfsTexture) -> u32 {
    let texels = (t.rgba.len() / 4).max(1);
    let opaque = t.rgba.chunks_exact(4).filter(|px| px[3] > 200).count();
    (opaque * 1000 / texels) as u32
}

/// Above this coverage a "detail overlay" is really a mask (or an undecodable map) and is dropped.
const OVERLAY_MAX_OPAQUE_PERMILLE: u32 = 900;

/// NFSU2 shares one shader set across every car, so a material's shader hash (`0x00134013`)
/// names its *type* — the reliable signal for how to render a run, independent of the
/// car-specific texture it happens to reference. Hashes are `h = 0xFFFF_FFFF; for b in
/// NAME.bytes() { h = h.wrapping_mul(33).wrapping_add(b) }` of the uppercase shader name.
mod shader {
    pub const CARSKIN: u32 = 0xd6d6_080a; // painted body panels
    pub const WINDSHIELD: u32 = 0x471a_1dca; // transparent glass (windscreen + windows)
    pub const WINDOWMASK: u32 = 0x3ed7_0c43; // opaque black window frame / frit
    pub const INTERIOR: u32 = 0x2787_edab; // seats / dash (behind the glass)
    pub const CHROME: u32 = 0x5494_9afd; // bright metal trim + mirrors
    pub const DULLPLASTIC: u32 = 0x0fed_ee40; // matte black plastic
    pub const MOLDINGS: u32 = 0x12c9_453c; // dark rubber/plastic mouldings
    pub const PLAINNOTHING: u32 = 0x010c_b64a; // unshaded filler
    pub const BOTTOM: u32 = 0x52bf_4c34; // underbody / rocker
    pub const GRILL: u32 = 0x02dd_dad9; // dark front grille
}

/// Map a shader hash to the flat material group it should render as, for the runs whose look
/// is decided by shader alone (glass, chrome, and the dark interior/trim family that would
/// otherwise paint bright). Returns `None` for shaders that instead carry a texture (head/
/// brake lights, tyres, badging) or that we don't recognise — those fall back to texture/name.
fn shader_group(shader: u32) -> Option<Grp> {
    match shader {
        shader::CARSKIN => Some(Grp::Paint),
        shader::WINDSHIELD => Some(Grp::Glass),
        shader::CHROME => Some(Grp::Chrome),
        shader::INTERIOR
        | shader::WINDOWMASK
        | shader::DULLPLASTIC
        | shader::MOLDINGS
        | shader::PLAINNOTHING
        | shader::BOTTOM
        | shader::GRILL => Some(Grp::Trim),
        _ => None,
    }
}

/// Whether a material run is [`shader::PLAINNOTHING`] filler that should be dropped rather than
/// rendered. That shader is "unshaded filler": on **BASE** it is the dark interior tub (kept, so
/// the cabin reads through the glass), but on a **kit body** it is wheel-well filler the real game
/// hides behind the wheel — drawn as an opaque flat panel it juts out past the arch as a black
/// square (was visible on RX7/RX8/GTO/GOLF/SKYLINE). Drop it only there, keyed off whether the
/// owning part is the BASE shell.
fn is_dropped_filler(shader: u32, part_is_base: bool) -> bool {
    shader == shader::PLAINNOTHING && !part_is_base
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
            if base_paint && m.shader.0 == shader::CARSKIN {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plainnothing_filler_dropped_only_off_base() {
        // The interior tub on BASE is kept (read through the glass)...
        assert!(!is_dropped_filler(shader::PLAINNOTHING, true));
        // ...but the same "unshaded filler" shader on a kit body is the wheel-well filler that
        // otherwise juts out as an opaque black square, so it is dropped.
        assert!(is_dropped_filler(shader::PLAINNOTHING, false));
    }

    #[test]
    fn other_shaders_are_never_treated_as_filler() {
        // Only PLAINNOTHING is filler; real materials are never dropped, on BASE or a kit body.
        for sh in [shader::CARSKIN, shader::WINDSHIELD, shader::CHROME, shader::INTERIOR, shader::BOTTOM] {
            assert!(!is_dropped_filler(sh, true));
            assert!(!is_dropped_filler(sh, false));
        }
    }

    #[test]
    fn composite_transparent_becomes_paint_opaque_keeps_overlay() {
        // Two texels: fully transparent, then fully opaque. Paint = white (sRGB white = 255).
        let overlay = [10u8, 20, 30, 0, 100, 150, 200, 255];
        let out = composite_over_paint(&overlay, [1.0, 1.0, 1.0]);
        assert_eq!(&out[0..4], &[255, 255, 255, 255], "transparent texel becomes the paint");
        assert_eq!(&out[4..8], &[100, 150, 200, 255], "opaque texel keeps the overlay colour");
    }

    #[test]
    fn composite_output_is_opaque_same_size() {
        let overlay = [0u8, 0, 0, 128, 0, 0, 0, 200];
        let out = composite_over_paint(&overlay, [0.0, 0.0, 0.0]);
        assert_eq!(out.len(), overlay.len());
        assert_eq!(out[3], 255);
        assert_eq!(out[7], 255);
    }

    /// A texture of `texels` pixels, `opaque` of them opaque — enough for the overlay picker,
    /// which only reads the alpha channel, the name and the hash.
    fn tex(name: &str, hash: u32, texels: usize, opaque: usize) -> NfsTexture {
        let mut rgba = Vec::with_capacity(texels * 4);
        for i in 0..texels {
            rgba.extend_from_slice(&[0, 0, 0, if i < opaque { 255 } else { 0 }]);
        }
        let mut t = NfsTexture::default();
        t.name = name.to_string();
        t.hash = AssetHash(hash);
        t.width = texels as u32;
        t.height = 1;
        t.rgba = rgba;
        t
    }

    fn tpk_of(textures: Vec<NfsTexture>) -> Tpk {
        let mut tpk = Tpk::default();
        tpk.textures = textures.into_iter().map(|t| (t.hash, t)).collect();
        tpk
    }

    #[test]
    fn overlay_pick_prefers_the_map_over_its_truncated_mask_twin() {
        // TPK DebugNames are cut at 23 chars, so `LANCEREVO8_DOORLINE_KIT_MASK` arrives ending in
        // the same stem as the map. The mask is fully opaque, the map mostly transparent — and
        // `textures` is a HashMap, so picking by iteration order rendered the car black at random.
        let tpk = tpk_of(vec![
            tex("LANCEREVO8_DOORLINE_KIT", 0x1111_1111, 100, 100), // the truncated _MASK
            tex("LANCEREVO8_DOORLINE_KIT", 0x2222_2222, 100, 19),  // the real map
        ]);
        let picked = doorline_texture(&tpk, true).expect("a doorline overlay");
        assert_eq!(picked.hash.0, 0x2222_2222);
    }

    #[test]
    fn a_full_coverage_overlay_is_refused() {
        // The IS300's own `_DOORLINE` is opaque: composited it would paint the whole car its
        // near-black colour. No overlay is better than a black car.
        let tpk = tpk_of(vec![tex("IS300_DOORLINE", 0x3333_3333, 100, 100)]);
        assert!(doorline_texture(&tpk, false).is_none());
        // A kit body with only an opaque `_DOORLINE_KIT` falls back to `_DOORLINE` — and refuses
        // that too when it is opaque as well.
        let tpk = tpk_of(vec![
            tex("IS300_DOORLINE_KIT", 0x4444_4444, 100, 100),
            tex("IS300_DOORLINE", 0x5555_5555, 100, 100),
        ]);
        assert!(doorline_texture(&tpk, true).is_none());
    }

    #[test]
    fn srgb_encode_endpoints() {
        assert_eq!(linear_to_srgb_u8(0.0), 0);
        assert_eq!(linear_to_srgb_u8(1.0), 255);
    }
}
