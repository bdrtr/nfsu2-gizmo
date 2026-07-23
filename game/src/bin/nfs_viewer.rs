//! # NFSU2 Car Viewer — the first NFSU2 car inside the real Gizmo engine (M1)
//!
//! Loads a Need for Speed: Underground 2 `GEOMETRY.BIN` with the pure-data `gizmo-nfs`
//! parser, assembles its default (showroom) configuration via [`nfsu2`], and spawns the
//! material-grouped meshes into the ECS with an orbit camera (drag to rotate, scroll to
//! zoom). It shares the exact car-assembly pipeline with `nfs_drive`/`nfs_race`; the only
//! difference is that materials are double-sided (so winding never hides geometry) and the
//! four wheels are static. Real game textures are a later step; parts are coloured by group.
//!
//! ```bash
//! cargo run -p nfsu2 --bin nfs_viewer -- "/path/to/CARS/240SX/GEOMETRY.BIN"
//! # or set NFSU2_CAR=/path/to/GEOMETRY.BIN
//! ```

use gizmo::prelude::*;
use gizmo::simple::{SimpleAppExt, SimpleSceneState};
use gizmo_nfs::parse_geometry;
use nfsu2::car::{build_car_visuals, env_color, load_tpk_beside, PbrLook};

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";

fn main() {
    let path = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .unwrap_or_else(|| DEFAULT_CAR.to_string());

    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let all = parse_geometry(&bytes).expect("failed to parse GEOMETRY.BIN");
    let tpk = load_tpk_beside(&path); // TEXTURES.BIN next to the model, if present
    let paint = env_color("NFS_COLOR", [0.10, 0.28, 0.72]); // override "r,g,b" in 0..1

    App::<SimpleSceneState>::new("Gizmo — NFSU2 240SX Viewer", 1400, 820)
        .with_simple_scene(move |scene, state| {
            // Key light (sun) + a softer fill from the other side.
            scene.world.spawn_bundle(DirectionalLightBundle {
                rotation: Quat::from_euler(EulerRot::XYZ, -0.95, 0.6, 0.0),
                intensity: 2.6,
                color: Vec3::new(1.0, 0.97, 0.9),
                ..Default::default()
            });
            scene.world.spawn_bundle(DirectionalLightBundle {
                rotation: Quat::from_euler(EulerRot::XYZ, -0.4, -2.2, 0.0),
                intensity: 0.7,
                color: Vec3::new(0.7, 0.8, 1.0),
                ..Default::default()
            });

            // Double-sided materials read cleanly regardless of face winding.
            let white = scene.asset_manager.create_white_texture(
                &scene.renderer.device,
                &scene.renderer.queue,
                &scene.renderer.scene.texture_bind_group_layout,
            );
            let make_material = |look: PbrLook| {
                Material::new(white.clone())
                    .with_pbr(Vec4::new(look.rgb[0], look.rgb[1], look.rgb[2], 1.0), look.roughness, look.metallic)
                    .with_double_sided(true)
            };

            let car = build_car_visuals(&scene.renderer.device, &all, tpk.as_ref(), paint, make_material);
            println!(
                "loaded {} material groups + {} textured parts from {path}",
                car.groups.len(),
                car.textured.len()
            );

            // Meshes are recentered to the car centre, so frame the camera on the origin.
            let radius =
                (car.width.powi(2) + car.height.powi(2) + car.length.powi(2)).sqrt() * 0.5;
            let eye = Vec3::new(radius * 1.5, radius * 0.75, radius * 1.7);
            scene.spawn_camera(state, eye, Vec3::ZERO);

            // Body material groups.
            for gv in car.groups {
                scene.world.spawn_bundle((
                    Transform::default(),
                    GlobalTransform::default(),
                    gv.mesh,
                    gv.material,
                    MeshRenderer::new(),
                ));
            }

            // Textured parts: upload each decoded TPK texture and use it as albedo.
            for tp in car.textured {
                let key = format!("nfs_tex_{:08X}", tp.texture.hash.0);
                let Ok(bg) = scene.asset_manager.install_decoded_material_texture(
                    &scene.renderer.device,
                    &scene.renderer.queue,
                    &scene.renderer.scene.texture_bind_group_layout,
                    &key,
                    &tp.texture.rgba,
                    tp.texture.width,
                    tp.texture.height,
                ) else {
                    continue;
                };
                let material = Material::new(bg)
                    .with_pbr(Vec4::new(tp.tint[0], tp.tint[1], tp.tint[2], 1.0), tp.roughness, tp.metallic)
                    .with_double_sided(true);
                scene.world.spawn_bundle((
                    Transform::default(),
                    GlobalTransform::default(),
                    tp.mesh,
                    material,
                    MeshRenderer::new(),
                ));
            }

            // Four static wheels at the fitted corners (lower half tucked into the arch).
            let fit = car.wheel_fit;
            let wheel_y = -car.height * 0.5 + fit.radius * 0.15;
            if let Some((wm, wmat)) = car.wheel {
                for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    let t = Transform::new(Vec3::new(sx * fit.half_track, wheel_y, sz * fit.half_wheelbase));
                    scene.world.spawn_bundle((
                        t,
                        GlobalTransform { matrix: t.local_matrix },
                        wm.clone(),
                        wmat.clone(),
                        MeshRenderer::new(),
                    ));
                }
            }
        })
        .run()
        .expect("failed to run app");
}
