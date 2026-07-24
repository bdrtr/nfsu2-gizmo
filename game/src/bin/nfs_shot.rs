//! Headless one-frame screenshot of a car for visual QA — renders the same visuals as
//! `nfs_viewer` into an offscreen target and writes the framebuffer as raw bytes (convert to
//! PNG with any tool). No window, so it works over SSH / in CI.
//!
//! Usage: nfs_shot GEOMETRY.BIN OUT.raw [W H]
//! Env: NFS_COLOR="r,g,b" paint, NFS_EYE="x,y,z" camera eye (else a 3/4 auto-frame).
//! Prints one line: `WxH format=<TextureFormat>` so the reader knows the channel order.

use gizmo::prelude::*;
use gizmo::renderer::Renderer;
use gizmo::wgpu;
use gizmo_nfs::parse_geometry;
use nfsu2::assets::{env_color, load_cartypeinfo_beside, load_tpk_beside};
use nfsu2::car::build_car_visuals;
use nfsu2::scene::{self, Textures};

fn main() {
    let path = std::env::args().nth(1).expect("usage: nfs_shot GEOMETRY.BIN OUT.raw [W H]");
    let out = std::env::args().nth(2).expect("usage: nfs_shot GEOMETRY.BIN OUT.raw [W H]");
    let w: u32 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1024);
    let h: u32 = std::env::args().nth(4).and_then(|s| s.parse().ok()).unwrap_or(768);
    pollster::block_on(run(&path, &out, w, h));
}

async fn run(path: &str, out: &str, w: u32, h: u32) {
    assert!(Renderer::headless_adapter_available().await, "no GPU adapter for headless render");
    let mut renderer = Renderer::new_headless(w, h, None).await;
    let mut world = World::new();
    let mut assets = AssetManager::new();
    let white =
        assets.create_white_texture(&renderer.device, &renderer.queue, &renderer.scene.texture_bind_group_layout);
    // NFS_CULL → single-sided (backface culled); default double-sided.
    let double = std::env::var("NFS_CULL").is_err();
    let two = |m: Material| if double { m.with_double_sided(true) } else { m };
    let mat = |rgb: [f32; 3], rough: f32, metal: f32| {
        two(Material::new(white.clone()).with_pbr(Vec4::new(rgb[0], rgb[1], rgb[2], 1.0), rough, metal))
    };
    let spawn = |world: &mut World, m: Mesh, material: Material, t: Transform| {
        let e = world.spawn();
        world.add_component(e, t);
        world.add_component(e, GlobalTransform { matrix: t.local_matrix });
        world.add_component(e, m);
        world.add_component(e, material);
        world.add_component(e, MeshRenderer::new());
    };

    // ── Car ──
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut all = parse_geometry(&bytes).expect("parse GEOMETRY.BIN");
    // Diagnostic filters (comma-separated substrings): NFS_ONLY keeps parts matching ANY,
    // NFS_DROP removes parts matching ANY.
    if let Ok(only) = std::env::var("NFS_ONLY") {
        let pats: Vec<String> = only.split(',').map(|s| s.trim().to_string()).collect();
        all.retain(|p| pats.iter().any(|s| p.name.contains(s.as_str())));
    }
    if let Ok(drop) = std::env::var("NFS_DROP") {
        let pats: Vec<String> = drop.split(',').map(|s| s.trim().to_string()).collect();
        all.retain(|p| !pats.iter().any(|s| p.name.contains(s.as_str())));
    }
    // NFS_SHADER=0xXXXX renders only the material runs with that shader hash (diagnostic).
    if let Ok(sh) = std::env::var("NFS_SHADER") {
        let sh = u32::from_str_radix(sh.trim_start_matches("0x"), 16).unwrap_or(0);
        for p in &mut all {
            if p.materials.is_empty() {
                p.indices.clear();
            } else {
                p.materials.retain(|m| m.shader.0 == sh);
                if p.materials.is_empty() {
                    p.indices.clear();
                }
            }
        }
    }
    // NFS_SHADER_DROP=0xAAAA,0xBBBB removes those shaders' runs — the inverse of NFS_SHADER, for
    // asking "which shader draws this artefact?" by elimination rather than in isolation.
    if let Ok(list) = std::env::var("NFS_SHADER_DROP") {
        let drop: Vec<u32> = list
            .split(',')
            .filter_map(|s| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .collect();
        for p in &mut all {
            p.materials.retain(|m| !drop.contains(&m.shader.0));
        }
    }
    if std::env::var("NFS_MATLIST").is_ok() {
        for p in &all {
            for m in &p.materials {
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for &idx in p.indices.get(m.index_offset..m.index_offset + m.index_count).unwrap_or(&[]) {
                    if let Some(pos) = p.positions.get(idx as usize) {
                        for k in 0..3 {
                            lo[k] = lo[k].min(pos[k]);
                            hi[k] = hi[k].max(pos[k]);
                        }
                    }
                }
                // Positions are NFSU2 Z-up: center[2] is height (high = greenhouse/roof).
                eprintln!(
                    "{:<30} sh={:#010x} tris={:<5} centerZ={:.2} center=[{:.2},{:.2},{:.2}]",
                    p.name,
                    m.shader.0,
                    m.index_count / 3,
                    (lo[2] + hi[2]) / 2.0,
                    (lo[0] + hi[0]) / 2.0,
                    (lo[1] + hi[1]) / 2.0,
                    (lo[2] + hi[2]) / 2.0
                );
            }
        }
    }
    if std::env::var("NFS_LIST").is_ok() {
        for p in nfsu2::parts::select_stock_car(&all) {
            eprintln!(
                "{:<36} tris={:<6} grp={:?}",
                p.name,
                p.triangle_count(),
                nfsu2::parts::group_of(&p.name)
            );
        }
    }
    let tpk = load_tpk_beside(path);
    let paint = env_color("NFS_COLOR", [0.10, 0.28, 0.72]);
    let cfg = nfsu2::parts::CarConfig::from_env();
    let car = build_car_visuals(&renderer.device, &all, tpk.as_ref(), paint, &cfg, |look| {
        two(Material::new(white.clone()).with_pbr(
            Vec4::new(look.rgb[0], look.rgb[1], look.rgb[2], look.alpha),
            look.roughness,
            look.metallic,
        ))
    });
    let (cw, ch, cl) = (car.width, car.height, car.length);
    let fit = car.wheel_fit;
    let car_center = car.center;
    // Exact wheel mounts (asymmetric wheelbase, per-car track & ride height) from GLOBALB's
    // CarTypeInfo when the game bundle is reachable; else the symmetric fit_wheel corners.
    let cti = load_cartypeinfo_beside(path);

    let mut car = car;
    let mut tex = Textures {
        assets: &mut assets,
        device: &renderer.device,
        queue: &renderer.queue,
        layout: &renderer.scene.texture_bind_group_layout,
    };
    scene::spawn_body(&mut world, &mut car, &mut tex, |bg, tint, rough, metal| {
        two(Material::new(bg).with_pbr(Vec4::new(tint[0], tint[1], tint[2], 1.0), rough, metal))
    });
    if let Some(interior) = car.interior.take() {
        spawn(&mut world, interior, mat([0.02, 0.02, 0.025], 0.9, 0.0), Transform::new(Vec3::ZERO));
    }
    if let Some((wheel_mesh, surface)) = car.wheel.take() {
        let wmat = scene::wheel_material(surface, &mut tex, |bg| {
            two(Material::new(bg).with_pbr(Vec4::new(1.0, 1.0, 1.0, 1.0), 0.7, 0.2))
        });
        for mount in scene::wheel_mounts(cti.as_ref(), fit, car_center, ch) {
            let t = Transform::new(mount).with_rotation(scene::wheel_mirror(mount));
            spawn(&mut world, wheel_mesh.clone(), wmat.clone(), t);
        }
    }

    scene::add_lights(
        &mut world,
        Transform::new(Vec3::new(30.0, 80.0, 40.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.8)),
        2.6,
        Vec3::new(-30.0, 40.0, -20.0),
    );

    // ── Camera: 3/4 auto-frame on the origin (meshes are recentered there) ──
    let radius = (cw * cw + ch * ch + cl * cl).sqrt() * 0.5;
    let eye = env_color("NFS_EYE", [radius * 1.5, radius * 0.75, radius * 1.7]);
    let eye = Vec3::new(eye[0], eye[1], eye[2]);
    let dir = (-eye).normalize();
    let yaw = dir.z.atan2(dir.x);
    let pitch = dir.y.asin();
    // Tight near/far around the framed car: the default 0.1..4000 range wrecks depth-buffer
    // precision so a wheel rim's near-coplanar front/back faces z-fight into shard noise. A
    // near ≈ eye_dist − 2·radius and far ≈ eye_dist + 2·radius restores precision.
    let eye_dist = eye.length();
    let near = (eye_dist - radius * 2.0).max(radius * 0.05).max(0.05);
    let far = eye_dist + radius * 3.0;
    let cam = world.spawn();
    world.add_component(cam, Transform::new(eye));
    world.add_component(cam, GlobalTransform::default());
    world.add_component(cam, Camera::new(std::f32::consts::FRAC_PI_4, near, far, yaw, pitch, true));

    // ── Render one frame into an offscreen target, then read it back ──
    let format = renderer.config.format;
    let bpp = 4u32;
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("nfs-shot-target"),
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
    gizmo::systems::default_render_pass(&mut world, &mut encoder, &view, &mut renderer);

    // 256-align bytes_per_row for the copy.
    let unpadded = w * bpp;
    let align = 256;
    let padded = unpadded.div_ceil(align) * align;
    let staging = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("nfs-shot-readback"),
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

    // Drop row padding → tight w*h*4 buffer.
    let mut tight = Vec::with_capacity((unpadded * h) as usize);
    for y in 0..h {
        let start = (y * padded) as usize;
        tight.extend_from_slice(&data[start..start + unpadded as usize]);
    }
    std::fs::write(out, &tight).expect("write raw");
    println!("{w}x{h} format={format:?} -> {out}");
}
