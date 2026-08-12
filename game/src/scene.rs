//! Engine-side scaffolding the demo binaries share.
//!
//! Every binary builds the same car the same way — upload each resolved texture, spawn one
//! entity per material group and per textured mesh, then instance the wheel at four corners —
//! and differs only in what it does *with* that car afterwards. That common half lives here so
//! a binary is left with its own scene, its own camera and its own update loop.

use crate::car::{composite_over_paint, CarVisuals, WheelFit, WheelSurface};
use crate::geom::add_transform;
use gizmo::prelude::*;
use gizmo::renderer::components::LightRole;
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
