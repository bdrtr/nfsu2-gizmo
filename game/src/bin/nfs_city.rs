//! Headless one-frame render of a city region, for visual QA of the world path.
//!
//! `nfs_shot` does this for a car. This does it for a `TRACKS/STREAM*.BUN`: read the region's
//! objects and its texture packs, merge them per (cell, texture), and render one frame from above
//! into an offscreen target. No window, so it works over SSH and in CI.
//!
//! Usage: `nfs_city <STREAM*.BUN | TRACKS-dir> OUT.raw [W H]`
//!
//! Given the `TRACKS` directory it loads every region at once. The bundles are not adjacent tiles
//! — they share one world coordinate system and overlap, being per-race-route supersets of the same
//! city — so a district's buildings and the roads under them can live in different files, and
//! loading one alone leaves the buildings hanging in the air.
//!
//! Env:
//! - `NFS_BUDGET=<n>` — take only the `n` objects nearest the region's centre. The throwaway that
//!   makes `STREAML4RA`'s 10,735 objects usable before streaming exists; delete it with the rest of
//!   the radius filter.
//! - `NFS_AT="x,y,z"` — what to look at, in world space. Defaults to the centre of what loaded,
//!   which is only useful for one region; with the whole `TRACKS` directory the centre is a point
//!   in the middle of eight overlapping districts.
//! - `NFS_EYE="x,y,z"` — camera eye, relative to the look-at point. Default is a high
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

    // Every bundle's bytes stay alive for the whole run: a pack borrows its pixel pool from them.
    let files = bundles(path);
    assert!(!files.is_empty(), "{path}: no STREAM*.BUN here");
    let loaded: Vec<Vec<u8>> = files
        .iter()
        .map(|f| std::fs::read(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display())))
        .collect();

    let mut meshes = Vec::new();
    let mut packs = Vec::new();
    for bytes in &loaded {
        meshes.extend(gizmo_nfs::world::meshes(bytes).expect("read a region's meshes"));
        packs.extend(gizmo_nfs::world::packs(bytes).expect("read a region's texture packs"));
    }
    println!("{} bundle(s), {} objects, {} packs", loaded.len(), meshes.len(), packs.len());

    // Sky and panorama are drawn by nothing yet, and drawn as ordinary geometry they are a wall
    // across the frame — see `world::is_backdrop` for what they are and how they were found.
    let sky = meshes.iter().filter(|m| nfsu2::world::is_backdrop(&m.header.name)).count();
    let lod = meshes.iter().filter(|m| nfsu2::world::is_distant_lod(&m.header.name)).count();
    meshes.retain(|m| {
        !nfsu2::world::is_backdrop(&m.header.name) && !nfsu2::world::is_distant_lod(&m.header.name)
    });
    let declared = meshes.len();

    // NFS_TOP=<n>: name the n objects with the largest extent, which is how you find out what a
    // frame-filling surface actually is instead of guessing at it.
    if let Ok(n) = std::env::var("NFS_TOP").map(|v| v.parse::<usize>().unwrap_or(20)) {
        let mut by_size: Vec<(f32, &str, usize)> = meshes
            .iter()
            .filter(|m| !m.positions.is_empty())
            .map(|m| {
                let lo = nfsu2::world::world_point(&m.header, m.header.bbox_min);
                let hi = nfsu2::world::world_point(&m.header, m.header.bbox_max);
                ((hi - lo).length(), m.header.name.as_str(), m.positions.len())
            })
            .collect();
        by_size.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (span, name, verts) in by_size.iter().take(n) {
            println!("  span {span:>10.0}  verts {verts:>7}  {name}");
        }
    }

    // The shared tiers: `TRACKS/LOC4DYNTEX.BIN` and `GLOBAL/`, found from the TRACKS directory's
    // parent. Missing means grey walls, not a failure to start.
    let root = std::path::Path::new(&path)
        .ancestors()
        .find(|a| a.join("GLOBAL").is_dir())
        .map(std::path::Path::to_path_buf);
    let shared = root.map(|r| nfsu2::world::SharedTextures::load(&r)).unwrap_or_default();
    println!("{} shared textures", shared.len());

    // NFS_NEAR=<radius>: name the objects whose box contains or nearly contains the look-at
    // point, with their resolved texture. For finding out what a surface actually is.
    if let Ok(r) = std::env::var("NFS_NEAR").map(|v| v.parse::<f32>().unwrap_or(40.0)) {
        let at = std::env::var("NFS_AT")
            .ok()
            .and_then(|s| vec3_of(&s))
            .unwrap_or(Vec3::ZERO);
        let mut near: Vec<(f32, &str, usize, String)> = meshes
            .iter()
            .filter(|m| !m.positions.is_empty())
            .filter_map(|m| {
                let lo = nfsu2::world::world_point(&m.header, m.header.bbox_min);
                let hi = nfsu2::world::world_point(&m.header, m.header.bbox_max);
                let c = (lo + hi) * 0.5;
                // Containment on the ground plane, not centre distance: a terrain plane centred a
                // kilometre away is still the thing under your feet.
                let inside = at.x >= lo.x.min(hi.x) - r
                    && at.x <= lo.x.max(hi.x) + r
                    && at.z >= lo.z.min(hi.z) - r
                    && at.z <= lo.z.max(hi.z) + r;
                let d = (c - at).length();
                inside.then(|| {
                    let keys: Vec<String> = m
                        .texture_slots
                        .iter()
                        .map(|k| {
                            let own = packs.iter().any(|p| p.get(*k).is_some());
                            let sh = shared.get(*k).is_some();
                            format!("{:08X}{}", k.0, if own { "" } else if sh { "^shared" } else { "^MISSING" })
                        })
                        .collect();
                    (d, m.header.name.as_str(), m.positions.len(), keys.join(" "))
                })
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (d, name, v, keys) in near.iter().take(16) {
            println!("  d={d:>7.0}  v={v:>6}  {name:<28} {keys}");
        }
    }

    // NFS_FIND=<substr>: how many loaded objects match, and where the first few are — for
    // answering "is the road even here" without flying around looking for it.
    if let Ok(pat) = std::env::var("NFS_FIND") {
        let hits: Vec<&gizmo_nfs::world::WorldMesh> =
            meshes.iter().filter(|m| m.header.name.contains(&pat)).collect();
        println!("{} objects match {pat:?}", hits.len());
        for m in hits.iter().take(6) {
            let lo = nfsu2::world::world_point(&m.header, m.header.bbox_min);
            let hi = nfsu2::world::world_point(&m.header, m.header.bbox_max);
            let c = (lo + hi) * 0.5;
            println!("  {:<28} centre {:.0},{:.0},{:.0}  v={}", m.header.name, c.x, c.y, c.z, m.positions.len());
        }
    }

    let budget = std::env::var("NFS_BUDGET").ok().and_then(|s| s.parse::<usize>().ok());
    // Frame on the region's own centre so a budget takes a neighbourhood rather than an edge.
    let around = centre_of(&meshes);
    let city = build_region(&renderer.device, meshes, &packs, Some(&shared), around, budget);

    println!(
        "{declared} declared, {sky} backdrop, {lod} world-LOD, {} kept ({} duplicates), {} packs, {} merged meshes, {} unresolved runs",
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
        let own = packs.iter().find_map(|p| p.get(key).and_then(|r| p.decode(r).ok()));
        let Some(image) = own.or_else(|| shared.get(key).cloned()) else { continue };
        let name = format!("city_{:08X}", key.0);
        if let Some(bg) = tex.upload(&name, &image.rgba, image.width, image.height) {
            bound.insert(key, bg);
            decoded += 1;
        }
    }
    println!("{decoded} textures decoded and uploaded");

    // NFS_ID="x,y": render every merged mesh in a colour that encodes its index, then say which
    // one covers that pixel. Four hypotheses about the flat white ground were each eliminated by
    // filtering a family out and finding the frame unchanged; this asks the frame directly instead.
    //
    // The albedo is pre-converted linear→sRGB's inverse so the byte that lands in an
    // `Rgba8UnormSrgb` target is the index back again.
    let id_probe = std::env::var("NFS_ID").ok().and_then(|s| {
        let v: Vec<u32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        (v.len() == 2).then(|| (v[0], v[1]))
    });

    // ── Spawn ──
    //
    // Baked-lit: the city's own lighting is in the vertex colour, and the one thing the file
    // cannot know is where the car in front of it is casting. `BakedLit` multiplies the baked
    // colour in the way `unlit` does and adds the sun's cascade term, for one forward draw per
    // batch instead of the eleven a deferred-lit one costs.
    let mut spawned = 0usize;
    for (idx, m) in city.meshes.iter().enumerate() {
        if id_probe.is_some() {
            let to_linear = |b: u32| {
                let v = b as f32 / 255.0;
                if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            let (r, g) = ((idx & 0xFF) as u32, ((idx >> 8) & 0xFF) as u32);
            let material = Material::new(white.clone())
                .with_unlit(Vec4::new(to_linear(r), to_linear(g), 0.0, 1.0));
            scene::spawn_mesh(&mut world, m.mesh.clone(), material, Transform::new(m.origin));
            spawned += 1;
            continue;
        }
        let material = match m.texture.and_then(|k| bound.get(&k)) {
            Some(bg) => Material::new(bg.clone()).with_baked_lit(Vec4::new(1.0, 1.0, 1.0, 1.0)),
            // A run whose texture resolved nowhere draws in a flat grey rather than vanishing —
            // a hole in the world reads as a parser bug, and this is not one.
            None => Material::new(white.clone()).with_baked_lit(Vec4::new(0.35, 0.35, 0.38, 1.0)),
        };
        scene::spawn_mesh(&mut world, m.mesh.clone(), material, Transform::new(m.origin));
        spawned += 1;
    }
    println!("{spawned} entities spawned");

    // ── Camera: frame what actually loaded ──
    let (lo, hi) = bounds(&city);
    let mid = std::env::var("NFS_AT")
        .ok()
        .and_then(|s| vec3_of(&s))
        .unwrap_or_else(|| (lo + hi) * 0.5);
    let radius = ((hi - lo).length() * 0.5).max(1.0);
    let eye_off = std::env::var("NFS_EYE")
        .ok()
        .and_then(|s| vec3_of(&s))
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

    let pixels = shoot(&mut world, &mut renderer, out, w, h);

    if let Some((px, py)) = id_probe {
        let i = ((py.min(h - 1) * w + px.min(w - 1)) * 4) as usize;
        let (r, g) = (u32::from(pixels[i]), u32::from(pixels[i + 1]));
        let idx = (g << 8 | r) as usize;
        println!("pixel ({px},{py}) = rgb({r},{g},{}) → mesh #{idx}", pixels[i + 2]);
        match city.meshes.get(idx) {
            Some(m) => {
                let name = m
                    .texture
                    .and_then(|k| packs.iter().find_map(|p| p.get(k)).map(|t| t.name.clone()))
                    .or_else(|| m.texture.map(|k| format!("shared {:08X}", k.0)))
                    .unwrap_or_else(|| "(no texture)".into());
                println!("  cell {:?}  origin {:?}  texture {name}", m.cell, m.origin);
            }
            None => println!("  no mesh with that index — the pixel is background"),
        }
    }
}

/// `"x,y,z"` → a vector, or `None` if it is not three numbers.
fn vec3_of(s: &str) -> Option<Vec3> {
    let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
}

/// One bundle, or every `STREAM*.BUN` in a directory, in name order.
fn bundles(path: &str) -> Vec<std::path::PathBuf> {
    let p = std::path::Path::new(path);
    if p.is_file() {
        return vec![p.to_path_buf()];
    }
    let Ok(dir) = std::fs::read_dir(p) else { return Vec::new() };
    let mut out: Vec<_> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|q| {
            q.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("STREAM") && n.ends_with(".BUN"))
        })
        .collect();
    out.sort();
    out
}

/// The centre of every object's bounding box, in the Gizmo frame.
fn centre_of(meshes: &[gizmo_nfs::world::WorldMesh]) -> Vec3 {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    // Objects with no geometry are skipped: the `ANM_*` animation markers carry an infinite
    // bounding box, and one of those turns the whole city's centre into a NaN.
    for m in meshes.iter().filter(|m| !m.positions.is_empty()) {
        for corner in [m.header.bbox_min, m.header.bbox_max] {
            let p = nfsu2::world::world_point(&m.header, corner);
            if p.is_finite() {
                lo = lo.min(p);
                hi = hi.max(p);
            }
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
fn shoot(world: &mut World, renderer: &mut Renderer, out: &str, w: u32, h: u32) -> Vec<u8> {
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
    tight
}
