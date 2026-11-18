//! Engine-side scaffolding the demo binaries share.
//!
//! Every binary builds the same car the same way — upload each resolved texture, spawn one
//! entity per material group and per textured mesh, then instance the wheel at four corners —
//! and differs only in what it does *with* that car afterwards. That common half lives here so
//! a binary is left with its own scene, its own camera and its own update loop.

use crate::car::{composite_over_paint, CarVisuals, WheelFit, WheelSurface};
use crate::geom::add_transform;
use gizmo::prelude::*;
use gizmo::renderer::components::{LightRole, ShadowCasting};
use gizmo::wgpu;
use gizmo_nfs::CarTypeInfo;
use std::sync::Arc;

/// How much light the city gets with the sun out of the picture, and how much it makes itself.
///
/// The engine's baked-lit path computes `(baked · shadow + ambient) · albedo · texture + emissive`,
/// and both new terms default to zero — so the arms exist and nothing was pulling them. Bayview
/// needs them pulled: it is a **night** map whose lighting is baked into vertex colours, and
/// multiplied straight through, a surface with vertex colour 64 and texture 64 lands at **2.6/255**.
/// Measured on the street, the frame's 5th percentile sat at 24/255 with the tower reading as a
/// silhouette with lit windows and nothing in between.
///
/// `ambient` goes *inside* the multiply, so it lifts the black end while the texture still decides
/// what the surface looks like — a dark-albedo wall stays darker than a pale one, which is the
/// property that makes this a lift rather than a fog. `emissive` is added *after* and is not
/// touched by the texture, so it is the wrong knob for a city and stays at zero unless asked.
///
/// Both are overridable — `NFS_AMBIENT="r,g,b"` or a single number for grey, same for
/// `NFS_EMISSIVE` — because the right value is a judgement about how a night map should read, and
/// a judgement left in a constant is one nobody can argue with.
#[must_use]
pub fn city_lift() -> (Vec3, Vec3) {
    fn read(key: &str, fallback: Vec3) -> Vec3 {
        let Ok(v) = std::env::var(key) else { return fallback };
        let parts: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        match parts[..] {
            [a] => Vec3::splat(a),
            [r, g, b] => Vec3::new(r, g, b),
            _ => fallback,
        }
    }
    (read("NFS_AMBIENT", CITY_AMBIENT), read("NFS_EMISSIVE", Vec3::ZERO))
}

/// The ambient the city is drawn with by default.
///
/// Slightly blue because Bayview's key light is street lighting and sky, not sun, and a neutral
/// grey lift reads as haze.
///
/// The magnitude comes from a sweep rather than from taste. Measured on a street-level frame of
/// `STREAML4RA`, luminance over the whole frame:
///
/// | ambient | median | p05 | p25 | p95 |
/// |---|---|---|---|---|
/// | 0.00 | 54 | 24 | 41 | 155 |
/// | 0.08 | 63 | 28 | 49 | 155 |
/// | **0.12** | **68** | **30** | **52** | **155** |
/// | 0.24 | 81 | 35 | 62 | 155 |
///
/// **p95 does not move at any setting**, which is the property that makes this safe: the bright end
/// is texture-dominated and the lift is proportionally nothing there, so neon and lit windows keep
/// their separation from the dark they sit against. 0.24 was rejected by eye — the frame reads as
/// overcast rather than as night — and 0.12 is where the road surface and the wall's relief become
/// legible without the sky-to-ground contrast collapsing.
///
/// That sweep was taken under ACES with raw vertex colours. Re-measured on 2026-11-18 under what the
/// city draws with now (PBR Neutral, decoded vertex colours — `city_tonemap`), same street:
///
/// | ambient × | p05 | p25 | median | p95 |
/// |---|---|---|---|---|
/// | **1** | **14** | **24** | **30** | **76** |
/// | ½ | 12 | 20 | 29 | 66 |
/// | ¼ | 11 | 18 | 28 | 62 |
/// | 0 | 9 | 17 | 27 | 58 |
///
/// The lift now works only where it was meant to, at the dark end (p05 9 → 14), and the frame the
/// user chose was taken with it, so it stays.
pub const CITY_AMBIENT: Vec3 = Vec3::new(0.10, 0.11, 0.14);

/// What crosses the glare threshold in this city, and how hard it glows.
///
/// The renderer's own default is a threshold of `0.85`, and **nothing in Bayview ever reaches it**:
/// the brightest surfaces are a p95 vertex colour of 183 against window textures that peak at 250,
/// which is about `0.44` in linear. So the glare pass ran every frame and extracted nothing. That
/// default is right for a scene with values above `1.0` and a baked-lit night map is not one — the
/// city's whole lighting is a byte per vertex, and a byte cannot be over-bright.
///
/// Lowering it puts the glow back on exactly the things that should have it. Measured over the
/// frame, threshold `0.85` → `0.18` moves the maximum from 202 to 221 and the share of pixels over
/// 200 from 0.00 % to 0.06 % — a small number, and the right small number: it is the lit windows
/// and the neon, and nothing else moved. The median does not shift at all, so this is glare and not
/// a brightness knob; [`city_lift`] is the brightness knob.
///
/// `NFS_BLOOM="threshold[,intensity]"` overrides both.
#[must_use]
pub fn city_glare() -> (f32, f32) {
    let mut out = (CITY_BLOOM_THRESHOLD, CITY_BLOOM_INTENSITY);
    if let Ok(v) = std::env::var("NFS_BLOOM") {
        let n: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if let Some(t) = n.first() {
            out.0 = *t;
        }
        if let Some(i) = n.get(1) {
            out.1 = *i;
        }
    }
    out
}

/// The tone curve the city is drawn with: **PBR Neutral**, not the renderer's default ACES.
///
/// Chosen by eye on 2026-11-18 from five frames of one street (ACES/Neutral/none × raw/decoded
/// vertex colours), as the answer to "everything is too bright — it does not read as night". ACES
/// lifts the middle of this city's range: its light is a byte per vertex, display-referred, and
/// ACES both darkens the dark end 4.67× (MOTOR-NOTLARI #3) and pushes the mids up, so the frame
/// came out grey and washed (median 65/255). Neutral leaves values below its knee alone — a night
/// map stays as dark as its bytes say — and only rolls off the top, where neon and lit windows sit.
///
/// It goes together with the vertex colours being decoded (`world::build`): the two were judged as
/// a pair, and either one alone is a different picture. `NFS_TONEMAP=aces|none|neutral|agx|
/// reinhard|reinhard_lum` names another curve, for looking.
#[must_use]
pub fn city_tonemap() -> gizmo::renderer::gpu_types::TonemapCurve {
    use gizmo::renderer::gpu_types::TonemapCurve;
    match std::env::var("NFS_TONEMAP").as_deref() {
        Ok("aces") => TonemapCurve::Aces,
        Ok("none") => TonemapCurve::None,
        Ok("agx") => TonemapCurve::AgX,
        Ok("reinhard") => TonemapCurve::Reinhard,
        Ok("reinhard_lum") => TonemapCurve::ReinhardLuminance,
        _ => TonemapCurve::PbrNeutral,
    }
}

/// The glare threshold the city is drawn with. See [`city_glare`] for where it comes from.
pub const CITY_BLOOM_THRESHOLD: f32 = 0.18;
/// How hard what crosses [`CITY_BLOOM_THRESHOLD`] glows.
pub const CITY_BLOOM_INTENSITY: f32 = 1.2;

/// The four handles every texture upload needs, bundled so call sites stay one line.
pub struct Textures<'a> {
    /// The asset manager owning the GPU texture cache.
    pub assets: &'a mut AssetManager,
    /// The device to create textures on.
    pub device: &'a wgpu::Device,
    /// The queue to upload with.
    pub queue: &'a wgpu::Queue,
    /// The material bind-group layout to bind them into.
    pub layout: &'a wgpu::BindGroupLayout,
}

impl Textures<'_> {
    /// Upload one decoded RGBA8 image, returning its bind group (`None` if the upload failed —
    /// a texture that will not upload is skipped, never fatal).
    pub fn upload(&mut self, key: &str, rgba: &[u8], width: u32, height: u32) -> Option<Arc<wgpu::BindGroup>> {
        self.assets
            .install_decoded_material_texture(self.device, self.queue, self.layout, key, rgba, width, height)
            .ok()
    }
}

/// Spawn a built car's body meshes (material groups, then textured parts) as world entities,
/// returning their ids so the caller can move them with the chassis.
///
/// `textured` turns an uploaded bind group plus the part's suggested PBR parameters into the
/// material to draw it with, so double-sidedness stays the binary's choice.
pub fn spawn_body<F>(world: &mut World, car: &mut CarVisuals, tex: &mut Textures<'_>, textured: F) -> Vec<u32>
where
    F: Fn(Arc<wgpu::BindGroup>, [f32; 3], f32, f32) -> Material,
{
    let mut ids = Vec::new();
    for gv in std::mem::take(&mut car.groups) {
        ids.push(spawn_mesh(world, gv.mesh, gv.material, Transform::new(Vec3::ZERO)));
    }
    for tp in std::mem::take(&mut car.textured) {
        // A body-detail (doorline) overlay is composited over the paint; a full-colour detail
        // (light lens, badging) uploads as-is. The key carries which, so the two never collide
        // in the texture cache.
        let rgba = match tp.composite_over {
            Some(paint) => composite_over_paint(&tp.texture.rgba, paint),
            None => tp.texture.rgba.clone(),
        };
        let key = format!("nfs_tex_{:08X}_{}", tp.texture.hash.0, u8::from(tp.composite_over.is_some()));
        let Some(bg) = tex.upload(&key, &rgba, tp.texture.width, tp.texture.height) else { continue };
        let material = textured(bg, tp.tint, tp.roughness, tp.metallic);
        ids.push(spawn_mesh(world, tp.mesh, material, Transform::new(Vec3::ZERO)));
    }
    ids
}

/// Spawn a mesh with distance-chosen detail levels: `levels` is `(mesh, max_distance)`, finest
/// first. The first level is also the entity's own mesh, which the `LodGroup` overrides.
/// Whether the city's own meshes cast the sun's shadow: no, unless `NFS_CITY_SHADOWS=on`.
///
/// Bayview is a night map with its light baked into vertex colours, and the sun is there for the
/// car alone — but the engine's baked-lit path darkens a surface by up to 45 % where that sun is
/// shadowed, within [`SHADOW_DISTANCE`](gizmo::renderer::SHADOW_DISTANCE) (100 m) of the camera,
/// fading out at its edge. So every pole, sign and building near the camera threw a daylight
/// shadow onto the night street, and that band moved with the camera: lights that seemed to switch
/// as the car drove, signs whose faces dimmed and came back. The Bevy port of the same city, drawn
/// unlit with no sun, shows none of it. The car still casts, so it still sits on the road.
#[must_use]
pub fn city_shadows() -> ShadowCasting {
    if std::env::var("NFS_CITY_SHADOWS").as_deref() == Ok("on") { ShadowCasting::On } else { ShadowCasting::Off }
}

/// One mesh of the city: [`spawn_mesh`] with the city's shadow setting, [`city_shadows`].
pub fn spawn_city(world: &mut World, mesh: Mesh, material: Material, t: Transform) -> u32 {
    let e = world.spawn();
    add_transform(world, e, t);
    world.add_component(e, mesh);
    world.add_component(e, material);
    world.add_component(e, MeshRenderer::new().with_shadows(city_shadows()));
    e.id()
}

/// A city mesh with its LOD levels, under the city's shadow setting ([`city_shadows`]).
pub fn spawn_lod(world: &mut World, levels: &[(Mesh, f32)], material: Material, t: Transform) -> u32 {
    let e = world.spawn();
    add_transform(world, e, t);
    world.add_component(e, levels[0].0.clone());
    if levels.len() > 1 {
        let lods = levels
            .iter()
            .map(|(m, d)| gizmo::renderer::LodLevel::new(m.clone(), *d))
            .collect();
        world.add_component(e, gizmo::renderer::LodGroup::new(lods));
    }
    world.add_component(e, material);
    world.add_component(e, MeshRenderer::new().with_shadows(city_shadows()));
    e.id()
}

/// Spawn one mesh + material as an entity at `t`, returning its id.
pub fn spawn_mesh(world: &mut World, mesh: Mesh, material: Material, t: Transform) -> u32 {
    let e = world.spawn();
    add_transform(world, e, t);
    world.add_component(e, mesh);
    world.add_component(e, material);
    world.add_component(e, MeshRenderer::new());
    e.id()
}

/// Where the four wheels go, in chassis space.
///
/// Prefers the exact per-car mounts from `GLOBALB`'s [`CarTypeInfo`] (asymmetric wheelbase,
/// per-car track and ride height); falls back to the symmetric corners derived from the wheel
/// part's own bounds when the game bundle is not reachable.
#[must_use]
pub fn wheel_mounts(cti: Option<&CarTypeInfo>, fit: WheelFit, center: Vec3, body_height: f32) -> Vec<Vec3> {
    match cti {
        Some(c) => c.wheels.iter().map(|w| crate::car::wheel_mount(w, center)).collect(),
        None => {
            let y = -body_height * 0.5 + fit.radius * 0.95;
            [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .iter()
                .map(|(sx, sz)| Vec3::new(sx * fit.half_track, y, sz * fit.half_wheelbase))
                .collect()
        }
    }
}

/// The wheel radius to use: the record's own, else the one guessed from the wheel part's bounds.
///
/// `WheelSpec::radius` was never read — `fit_wheel` derived a radius from the mesh's bounding box
/// and clamped it to `0.18..0.55`, which is a guess with a guard on it where the file states the
/// number. The four mounts can differ (a staggered fitment does), so this takes the front pair's,
/// which is what sizes the visual and the suspension.
#[must_use]
pub fn wheel_radius(cti: Option<&CarTypeInfo>, fit: WheelFit) -> f32 {
    cti.map_or(fit.radius, |c| c.wheels[0].radius)
}

/// The rotation that turns a wheel instance to face its rim outward.
///
/// The wheel mesh is modelled for one side only; on the other flank it would show its flat
/// inboard back as a black slab. A 180° yaw mirrors it — a *reflection* would too, but would
/// invert the winding and the normals with it.
#[must_use]
pub fn wheel_mirror(mount: Vec3) -> Quat {
    if mount.x < 0.0 {
        Quat::from_rotation_y(std::f32::consts::PI)
    } else {
        Quat::IDENTITY
    }
}

/// Resolve the car's [`WheelSurface`] into one material shared by all four instances: the tyre
/// texture when the car ships one, else the flat rubber the assembly already built.
///
/// `flat` builds that rubber look, and is also the fallback when the tyre texture will not
/// upload — a white unlit wheel would be a worse failure than an untextured black one.
pub fn wheel_material<F, G>(surface: WheelSurface, tex: &mut Textures<'_>, textured: F, flat: G) -> Material
where
    F: Fn(Arc<wgpu::BindGroup>) -> Material,
    G: Fn(crate::car::PbrLook) -> Material,
{
    match surface {
        WheelSurface::Flat(m) => m,
        WheelSurface::Textured(t) => {
            let key = format!("nfs_wheel_{:08X}", t.hash.0);
            match tex.upload(&key, &t.rgba, t.width, t.height) {
                Some(bg) => textured(bg),
                None => flat(crate::car::wheel_look()),
            }
        }
    }
}

/// The two-light rig every demo uses: a warm key sun (with its own orientation) and a cool fill.
pub fn add_lights(world: &mut World, key: Transform, key_intensity: f32, fill_pos: Vec3) {
    let sun = world.spawn();
    add_transform(world, sun, key);
    world.add_component(sun, DirectionalLight::new(Vec3::new(1.0, 0.97, 0.9), key_intensity, LightRole::Sun));
    let fill = world.spawn();
    add_transform(world, fill, Transform::new(fill_pos));
    world.add_component(fill, DirectionalLight::new(Vec3::new(0.6, 0.7, 0.9), 0.6, LightRole::Sun));
}

/// The car model to load: the first CLI argument, else `$NFSU2_CAR`, else `default`.
#[must_use]
pub fn car_path(default: &str) -> String {
    std::env::args()
        .nth(1)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .unwrap_or_else(|| default.to_string())
}

/// A collider the cars drive on, made of what roads are made of.
///
/// **The wheel reads the collider's own material, not the `PhysicsMaterial` component.** The
/// engine's suspension raycast takes `Collider::material.dynamic_friction` from the collider it
/// hits and scales every tyre force by it over asphalt's 0.65. Every ground collider in this game
/// was built with the default material — friction 0.5 — and given a `PhysicsMaterial::ASPHALT`
/// component beside it that the wheel never looks at, so every tyre held 0.5 / 0.65 = **77 %** of
/// its grip, on every road and every test plane. Measured on the bench (`nfs_bench`): the wheels
/// reported a surface friction of 0.5 with the asphalt component in place.
#[must_use]
pub fn road(collider: Collider) -> Collider {
    collider.with_material(gizmo::physics::components::PhysicsMaterial::ASPHALT)
}
