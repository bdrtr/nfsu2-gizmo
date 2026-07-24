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
pub fn wheel_material<F>(surface: WheelSurface, tex: &mut Textures<'_>, textured: F) -> Material
where
    F: Fn(Arc<wgpu::BindGroup>) -> Material,
{
    match surface {
        WheelSurface::Flat(m) => m,
        WheelSurface::Textured(t) => {
            let key = format!("nfs_wheel_{:08X}", t.hash.0);
            match tex.upload(&key, &t.rgba, t.width, t.height) {
                Some(bg) => textured(bg),
                None => Material::new(tex.assets.create_white_texture(tex.device, tex.queue, tex.layout)),
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
