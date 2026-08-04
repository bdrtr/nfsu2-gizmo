//! Headless one-frame render of a city region, for visual QA of the world path.
//!
//! `nfs_shot` does this for a car. This does it for a `TRACKS/STREAM*.BUN`: read the region's
//! objects and its texture packs, merge them per (cell, texture), and render one frame from above
//! into an offscreen target. No window, so it works over SSH and in CI.
//!
//! Usage: `nfs_city STREAML4RH.BUN OUT.raw [W H]`
//!
//! Env:
//! - `NFS_BUDGET=<n>` — take only the `n` objects nearest the region's centre. The throwaway that
//!   makes `STREAML4RA`'s 10,735 objects usable before streaming exists; delete it with the rest of
//!   the radius filter.
//! - `NFS_EYE="x,y,z"` — camera eye, relative to the loaded content's centre. Default is a high
//!   three-quarter view framing the whole of what loaded.
//!
//! Prints one line of counts before rendering, because most of what can go wrong here is visible
//! in them: an object count that does not match the manifest, a duplicate count of zero (dedup not
//! running), or an unresolved-run count in the thousands (texture packs not found).

use gizmo::prelude::*;
use gizmo::renderer::Renderer;
use gizmo::wgpu;
use gizmo_nfs::types::AssetHash;
use nfsu2::scene::{self, Textures};
use nfsu2::world::{build_region, CityVisuals};
use std::collections::HashMap;

fn main() {
    let path = std::env::args().nth(1).expect("usage: nfs_city STREAM*.BUN OUT.raw [W H]");
    let out = std::env::args().nth(2).expect("usage: nfs_city STREAM*.BUN OUT.raw [W H]");
    let w: u32 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1024);
    let h: u32 = std::env::args().nth(4).and_then(|s| s.parse().ok()).unwrap_or(768);
    pollster::block_on(run(&path, &out, w, h));
}

async fn run(path: &str, out: &str, w: u32, h: u32) {
    assert!(Renderer::headless_adapter_available().await, "no GPU adapter for headless render");
    let mut renderer = Renderer::new_headless(w, h, None).await;
    let mut world = World::new();
    let mut assets = AssetManager::new();

    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut meshes = gizmo_nfs::world::meshes(&bytes).expect("read the region's meshes");

    // The skydome is a viewer decision, not a parser one, so it is filtered here. Every region
    // ships two of them, and their texture keys live in `TRACKS/LOC4DYNTEX.BIN` rather than in the
    // region's own packs — so with the shared tier not wired up yet they draw in the unresolved
    // grey, and being a sphere the size of the world they swallow the entire frame. A real
    // renderer keeps them and draws them camera-locked with depth writes off; this one is looking
    // at the ground.
    let sky = meshes.iter().filter(|m| m.header.name.contains("SKYDOME")).count();
    meshes.retain(|m| !m.header.name.contains("SKYDOME"));
    let packs = gizmo_nfs::world::packs(&bytes).expect("read the region's texture packs");
    let declared = meshes.len();

    let budget = std::env::var("NFS_BUDGET").ok().and_then(|s| s.parse::<usize>().ok());
    // Frame on the region's own centre so a budget takes a neighbourhood rather than an edge.
    let around = centre_of(&meshes);
    let city = build_region(&renderer.device, meshes, &packs, around, budget);

    println!(
        "{declared} declared, {sky} skydome, {} kept ({} duplicates), {} packs, {} merged meshes, {} unresolved runs",
        city.objects,
        city.duplicates,
        packs.len(),
        city.meshes.len(),
        city.unresolved_runs
    );

    // ── Textures: decode each key once, only the ones something actually uses ──
    let white =
        assets.create_white_texture(&renderer.device, &renderer.queue, &renderer.scene.texture_bind_group_layout);
    let mut tex = Textures {
        assets: &mut assets,
        device: &renderer.device,
        queue: &renderer.queue,
        layout: &renderer.scene.texture_bind_group_layout,
    };
    let mut bound: HashMap<AssetHash, _> = HashMap::new();
    let mut decoded = 0usize;
    for key in city.meshes.iter().filter_map(|m| m.texture) {
        if bound.contains_key(&key) {
            continue;
        }
        let Some((pack, record)) = packs.iter().find_map(|p| p.get(key).map(|r| (p, r))) else {
            continue;
        };
        let Ok(image) = pack.decode(record) else { continue };
        let name = format!("city_{:08X}", key.0);
        if let Some(bg) = tex.upload(&name, &image.rgba, image.width, image.height) {
            bound.insert(key, bg);
            decoded += 1;
        }
    }
    println!("{decoded} textures decoded and uploaded");

    // ── Spawn ──
    //
    // Baked-lit scenery: the vertex colour is the lighting, so the material is a flat white the
    // texture modulates rather than a PBR surface pretending to be lit from somewhere.
    let mut spawned = 0usize;
    for m in &city.meshes {
        let material = match m.texture.and_then(|k| bound.get(&k)) {
            Some(bg) => Material::new(bg.clone()).with_pbr(Vec4::new(1.0, 1.0, 1.0, 1.0), 0.9, 0.0),
            // A run whose texture resolved nowhere draws in a flat grey rather than vanishing —
            // a hole in the world reads as a parser bug, and this is not one.
            None => Material::new(white.clone()).with_pbr(Vec4::new(0.35, 0.35, 0.38, 1.0), 0.95, 0.0),
        };
        scene::spawn_mesh(&mut world, m.mesh.clone(), material, Transform::new(m.origin));
        spawned += 1;
    }
    println!("{spawned} entities spawned");

    // ── Camera: frame what actually loaded ──
    let (lo, hi) = bounds(&city);
    let mid = (lo + hi) * 0.5;
    let radius = ((hi - lo).length() * 0.5).max(1.0);
    let eye_off = std::env::var("NFS_EYE")
        .ok()
        .and_then(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
        })
        .unwrap_or_else(|| Vec3::new(radius * 0.9, radius * 1.1, radius * 0.9));
    let eye = mid + eye_off;

    scene::add_lights(
        &mut world,
        Transform::new(eye + Vec3::new(0.0, radius, 0.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.9)),
        2.4,
        mid,
    );

    let dir = (mid - eye).normalize();
    let cam = world.spawn();
    world.add_component(cam, Transform::new(eye));
    world.add_component(cam, GlobalTransform::default());
    // Near is derived from the eye distance rather than from the framing radius. Tying it to the
    // radius — which is what a car viewer does, correctly, because the car *is* the radius — put
    // the near plane at 95 m on a street-level shot and clipped every building the camera stood
    // next to. What matters at city scale is the near:far ratio, not the absolute near, so this
    // takes a small fraction of the eye distance and floors it at half a metre.
    let eye_dist = (mid - eye).length();
    let near = (eye_dist * 0.002).clamp(0.5, 10.0);
    let far = eye_dist + radius * 3.0;
    world.add_component(
        cam,
        Camera::new(std::f32::consts::FRAC_PI_4, near, far, dir.z.atan2(dir.x), dir.y.asin(), true),
    );
    println!("camera at {eye:?} looking at {mid:?}, near={near:.1} far={far:.1}");

    shoot(&mut world, &mut renderer, out, w, h);
}

/// The centre of every object's bounding box, in the Gizmo frame.
fn centre_of(meshes: &[gizmo_nfs::world::WorldMesh]) -> Vec3 {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for m in meshes {
        for corner in [m.header.bbox_min, m.header.bbox_max] {
            let p = nfsu2::world::world_point(&m.header, corner);
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    if lo.is_finite() && hi.is_finite() {
        (lo + hi) * 0.5
    } else {
        Vec3::ZERO
    }
}

/// Bounds of the cells that were actually built, which is what the camera frames.
fn bounds(city: &CityVisuals) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for m in &city.meshes {
        let half = Vec3::splat(nfsu2::world::CELL_SIZE * 0.5);
        lo = lo.min(m.origin - half);
        hi = hi.max(m.origin + half);
    }
    if lo.is_finite() && hi.is_finite() {
        (lo, hi)
    } else {
        (Vec3::ZERO, Vec3::ONE)
    }
}

/// Render one frame into an offscreen target and write it out, tightly packed.
fn shoot(world: &mut World, renderer: &mut Renderer, out: &str, w: u32, h: u32) {
    let format = renderer.config.format;
    let bpp = 4u32;
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("nfs-city-target"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder =
        renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    gizmo::systems::default_render_pass(world, &mut encoder, &view, renderer);

    let unpadded = w * bpp;
    let padded = unpadded.div_ceil(256) * 256;
    let staging = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("nfs-city-readback"),
        size: (padded * h) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    renderer.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
    let _ = renderer.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();

    let mut tight = Vec::with_capacity((unpadded * h) as usize);
    for y in 0..h {
        let start = (y * padded) as usize;
        tight.extend_from_slice(&data[start..start + unpadded as usize]);
    }
    std::fs::write(out, &tight).expect("write raw");
    println!("{w}x{h} format={format:?} -> {out}");
}
