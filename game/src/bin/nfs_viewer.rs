//! # NFSU2 Car Viewer — the first NFSU2 car inside the real Gizmo engine
//!
//! Loads a Need for Speed: Underground 2 `GEOMETRY.BIN` with the pure-data `gizmo-nfs`
//! parser, expands each part's indexed geometry into the engine's flat `Vertex` format,
//! uploads it as a GPU `Mesh`, gives each part a semantic-colored PBR `Material`, and
//! spawns it into the ECS. The `with_simple_scene` helper provides an orbit camera
//! (drag to rotate, scroll to zoom) and the render loop.
//!
//! Usage:
//! ```bash
//! cargo run -p demo --bin nfs_viewer -- "/path/to/CARS/240SX/GEOMETRY.BIN"
//! # or set NFSU2_CAR=/path/to/GEOMETRY.BIN
//! ```
//!
//! NFSU2 geometry is Z-up (X=length, Y=width, Z=height); Gizmo is Y-up, so positions and
//! normals are rotated -90° about X: (x, y, z) -> (x, z, -y). Materials are double-sided
//! so face winding differences never hide geometry. Real game textures are a later step;
//! parts are colored by name for now.

use gizmo::prelude::*;
use gizmo::renderer::gpu_types::Vertex;
use gizmo::simple::{SimpleAppExt, SimpleSceneState};
use gizmo_nfs::{parse_geometry, NfsMeshPart};

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";

fn main() {
    let path = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .unwrap_or_else(|| DEFAULT_CAR.to_string());

    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let all = parse_geometry(&bytes).expect("failed to parse GEOMETRY.BIN");

    // Keep the complete stock car: the base shell + KIT00 exterior parts, highest LOD,
    // excluding internal engine/under panels.
    let stock: Vec<NfsMeshPart> = all
        .into_iter()
        .filter(|p| {
            p.name.ends_with("_A")
                && (p.name.contains("BASE") || p.name.contains("KIT00"))
                && !p.name.contains("ENGINE")
                && !p.name.contains("UNDER")
                && !p.name.contains("FULLROOF") // duplicate of ROOF → z-fighting
                && !p.name.contains("TRUNK_AUDIO") // duplicate of TRUNK
        })
        .collect();
    let part_count = stock.len();
    let tri_total: usize = stock.iter().map(|p| p.triangle_count()).sum();
    println!("loaded {part_count} stock parts, {tri_total} triangles from {path}");

    // Centre of the (Y-up remapped) car, for camera framing.
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in &stock {
        for v in &p.positions {
            let g = to_gizmo(*v);
            for k in 0..3 {
                lo[k] = lo[k].min(g[k]);
                hi[k] = hi[k].max(g[k]);
            }
        }
    }
    let center = Vec3::new((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5);
    let radius = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt()
        * 0.5;
    let eye = center + Vec3::new(radius * 1.5, radius * 0.75, radius * 1.7);

    App::<SimpleSceneState>::new("Gizmo — NFSU2 240SX Viewer", 1400, 820)
        .with_simple_scene(move |scene, state| {
            scene.spawn_camera(state, eye, center);

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

            let white = scene.asset_manager.create_white_texture(
                &scene.renderer.device,
                &scene.renderer.queue,
                &scene.renderer.scene.texture_bind_group_layout,
            );

            for p in &stock {
                let verts = to_vertices(p);
                if verts.is_empty() {
                    continue;
                }
                let mesh =
                    Mesh::from_vertices(&scene.renderer.device, &verts, format!("nfs:{}", p.name));
                let (col, rough, metal) = part_material(&p.name);
                // Moderate roughness + double-sided reads cleanly. NOTE: a glossier clear-coat
                // + low roughness exposes z-fighting between the 240SX's overlapping BASE and
                // KIT00_BODY shells (both cover the greenhouse), so we keep it matte-ish.
                let mat = Material::new(white.clone())
                    .with_pbr(col, rough, metal)
                    .with_double_sided(true);
                scene.world.spawn_bundle((
                    Transform::default(),
                    GlobalTransform::default(),
                    mesh,
                    mat,
                    MeshRenderer::new(),
                ));
            }
        })
        .run()
        .expect("failed to run app");
}

/// NFSU2 Z-up (x, y, z) -> Gizmo Y-up (x, z, -y).
#[inline]
fn to_gizmo(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[2], -p[1]]
}

/// Expand a part's indexed geometry into the engine's flat, non-indexed `Vertex` list,
/// remapping coordinates to Y-up along the way.
fn to_vertices(p: &NfsMeshPart) -> Vec<Vertex> {
    let has_normals = !p.normals.is_empty();
    let mut out = Vec::with_capacity(p.indices.len());
    for &idx in &p.indices {
        let i = idx as usize;
        let Some(&pos) = p.positions.get(i) else { continue };
        let n = if has_normals { p.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]) } else { [0.0, 1.0, 0.0] };
        let uv = p.uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        out.push(Vertex {
            position: to_gizmo(pos),
            normal: to_gizmo(n),
            tex_coords: uv,
            ..Vertex::default()
        });
    }
    out
}

/// Semantic per-part material (albedo, roughness, metallic) until real textures are decoded.
fn part_material(name: &str) -> (Vec4, f32, f32) {
    if name.contains("GLASS") || name.contains("WINDOW") {
        (Vec4::new(0.05, 0.08, 0.13, 1.0), 0.05, 0.0)
    } else if name.contains("BRAKELIGHT") || name.contains("TAILLIGHT") {
        (Vec4::new(0.75, 0.05, 0.05, 1.0), 0.3, 0.0)
    } else if name.contains("HEADLIGHT") {
        (Vec4::new(0.9, 0.92, 0.98, 1.0), 0.1, 0.1)
    } else if name.contains("TIRE") {
        (Vec4::new(0.03, 0.03, 0.035, 1.0), 0.9, 0.0)
    } else if name.contains("WHEEL") || name.contains("RIM") {
        (Vec4::new(0.6, 0.62, 0.68, 1.0), 0.25, 0.9)
    } else if name.contains("EXHAUST") {
        (Vec4::new(0.5, 0.5, 0.55, 1.0), 0.25, 0.9)
    } else {
        // Body paint: metallic orange (moderate roughness hides BASE/BODY shell z-fighting).
        (Vec4::new(0.85, 0.32, 0.06, 1.0), 0.35, 0.35)
    }
}
