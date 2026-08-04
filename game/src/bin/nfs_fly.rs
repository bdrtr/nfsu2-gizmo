//! Fly through Bayview.
//!
//! A window over the same city `nfs_city` renders one frame of, with a free camera, so the world
//! can be looked at rather than screenshotted. This is the thing that tells you what a still cannot:
//! whether the frame rate holds when you turn a corner, where the geometry has holes, and which
//! textures did not resolve.
//!
//! ```bash
//! cargo run --release -p nfsu2 --bin nfs_fly -- "$NFSU2_ROOT/TRACKS"
//! ```
//!
//! Controls: **W/S** forward/back · **A/D** strafe · **E/Q** up/down · **Shift** ×6 ·
//! hold **right mouse** to look · **F** print where the camera is.
//!
//! Env: `NFS_BUDGET=<n>` caps objects (nearest first); `NFS_AT="x,y,z"` where to start.

use gizmo::egui;
use gizmo::prelude::*;
use gizmo_nfs::types::AssetHash;
use nfsu2::scene::{self, Textures};
use nfsu2::world::build_region;
use std::collections::HashMap;

/// Metres per second at a walk, before the Shift multiplier. A city block is ~100 m, so this
/// crosses one in about two seconds — fast enough to get somewhere, slow enough to look.
const SPEED: f32 = 45.0;

struct FlyState {
    camera_id: u32,
    yaw: f32,
    pitch: f32,
    meshes: usize,
    objects: usize,
}

fn main() {
    gizmo::app::setup_panic_hook();
    App::<FlyState>::new("Gizmo — Bayview", 1600, 900)
        .add_plugin(gizmo::plugins::TransformPlugin)
        .set_setup(setup)
        .set_update(update)
        .set_ui(ui)
        .set_render(|world, _s, encoder, view, renderer, _t| {
            // The city is baked-lit and opaque; the screen-space passes cost frame time and have
            // nothing to work with here.
            renderer.gpu_fluid = None;
            renderer.gpu_particles = None;
            renderer.ssr = None;
            renderer.ssgi = None;
            renderer.volumetric = None;
            gizmo::systems::default_render_pass(world, encoder, view, renderer);
        })
        .run()
        .expect("failed to run app");
}

fn setup(world: &mut World, renderer: &gizmo::renderer::Renderer) -> FlyState {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::var("NFSU2_ROOT").map(|r| format!("{r}/TRACKS")).expect("usage: nfs_fly <TRACKS>")
    });

    let files = bundles(&path);
    assert!(!files.is_empty(), "{path}: no STREAM*.BUN here");
    // The bytes outlive the packs that borrow their pixel pools, so they are leaked on purpose:
    // this is the whole city and it stays loaded for the life of the window.
    let loaded: &'static [Vec<u8>] = Box::leak(
        files
            .iter()
            .map(|f| std::fs::read(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display())))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let mut meshes = Vec::new();
    let mut packs = Vec::new();
    for bytes in loaded {
        meshes.extend(gizmo_nfs::world::meshes(bytes).expect("meshes"));
        packs.extend(gizmo_nfs::world::packs(bytes).expect("packs"));
    }
    meshes.retain(|m| !nfsu2::world::is_backdrop(&m.header.name));

    let start = std::env::var("NFS_AT")
        .ok()
        .and_then(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
        })
        .unwrap_or(Vec3::new(640.0, 40.0, -2816.0));
    let budget = std::env::var("NFS_BUDGET").ok().and_then(|s| s.parse::<usize>().ok());
    let city = build_region(&renderer.device, meshes, &packs, start, budget);

    let white = {
        let mut a = AssetManager::new();
        a.create_white_texture(&renderer.device, &renderer.queue, &renderer.scene.texture_bind_group_layout)
    };
    let mut assets = AssetManager::new();
    let mut tex = Textures {
        assets: &mut assets,
        device: &renderer.device,
        queue: &renderer.queue,
        layout: &renderer.scene.texture_bind_group_layout,
    };
    let mut bound: HashMap<AssetHash, _> = HashMap::new();
    for key in city.meshes.iter().filter_map(|m| m.texture) {
        if bound.contains_key(&key) {
            continue;
        }
        let Some((pack, rec)) = packs.iter().find_map(|p| p.get(key).map(|r| (p, r))) else { continue };
        let Ok(img) = pack.decode(rec) else { continue };
        if let Some(bg) = tex.upload(&format!("city_{:08X}", key.0), &img.rgba, img.width, img.height) {
            bound.insert(key, bg);
        }
    }

    for m in &city.meshes {
        let material = match m.texture.and_then(|k| bound.get(&k)) {
            Some(bg) => Material::new(bg.clone()).with_baked_lit(Vec4::new(1.0, 1.0, 1.0, 1.0)),
            None => Material::new(white.clone()).with_baked_lit(Vec4::new(0.35, 0.35, 0.38, 1.0)),
        };
        scene::spawn_mesh(world, m.mesh.clone(), material, Transform::new(m.origin));
    }

    scene::add_lights(
        world,
        Transform::new(start + Vec3::new(0.0, 400.0, 0.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.9)),
        1.6,
        start,
    );

    let cam = world.spawn();
    world.add_component(cam, Transform::new(start));
    world.add_component(cam, GlobalTransform::default());
    // near 0.5 / far 20 000: the ratio is what costs depth precision, and a city needs the far
    // plane. A near tied to the framed radius — right for a car viewer — clips whatever the camera
    // is standing next to.
    world.add_component(cam, Camera::new(std::f32::consts::FRAC_PI_4, 0.5, 20_000.0, -1.6, -0.35, true));

    println!("{} objects, {} meshes — flying at {start:?}", city.objects, city.meshes.len());
    FlyState {
        camera_id: cam.id(),
        yaw: -1.6,
        pitch: -0.35,
        meshes: city.meshes.len(),
        objects: city.objects,
    }
}

fn update(world: &mut World, state: &mut FlyState, dt: f32, input: &Input) {
    if input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT) {
        let d = input.mouse_delta();
        state.yaw += d.0 * 0.0032;
        state.pitch = (state.pitch - d.1 * 0.0032).clamp(-1.5, 1.5);
    }

    let (sy, cy) = state.yaw.sin_cos();
    let forward = Vec3::new(cy * state.pitch.cos(), state.pitch.sin(), sy * state.pitch.cos());
    let right = Vec3::new(-sy, 0.0, cy);

    let mut move_dir = Vec3::ZERO;
    let held = |k: KeyCode| input.is_key_pressed(k as u32);
    if held(KeyCode::KeyW) {
        move_dir += forward;
    }
    if held(KeyCode::KeyS) {
        move_dir -= forward;
    }
    if held(KeyCode::KeyD) {
        move_dir += right;
    }
    if held(KeyCode::KeyA) {
        move_dir -= right;
    }
    if held(KeyCode::KeyE) {
        move_dir += Vec3::Y;
    }
    if held(KeyCode::KeyQ) {
        move_dir -= Vec3::Y;
    }

    let boost = if held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight) { 6.0 } else { 1.0 };
    let step = if move_dir.length_squared() > 0.0 {
        move_dir.normalize() * SPEED * boost * dt
    } else {
        Vec3::ZERO
    };

    let cam_id = state.camera_id;
    let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
    let mut globals = unsafe { world.borrow_mut_unchecked::<GlobalTransform>() };
    let mut cameras = unsafe { world.borrow_mut_unchecked::<Camera>() };
    let mut pos = Vec3::ZERO;
    if let Some(mut t) = transforms.get_mut(cam_id) {
        t.position += step;
        pos = t.position;
        t.update_local_matrix();
        if let Some(mut g) = globals.get_mut(cam_id) {
            g.matrix = t.local_matrix;
        }
    }
    if let Some(mut c) = cameras.get_mut(cam_id) {
        c.yaw = state.yaw;
        c.pitch = state.pitch;
    }
    if input.is_key_just_pressed(KeyCode::KeyF as u32) {
        println!("NFS_AT=\"{:.0},{:.0},{:.0}\"", pos.x, pos.y, pos.z);
    }
}

fn ui(_world: &mut World, state: &mut FlyState, ctx: &egui::Context) {
    egui::Window::new("Bayview").show(ctx, |ui| {
        ui.label(format!("{} objects · {} meshes", state.objects, state.meshes));
        ui.label("W/S/A/D move · E/Q up/down · Shift x6");
        ui.label("hold right mouse to look · F prints the position");
    });
}

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
