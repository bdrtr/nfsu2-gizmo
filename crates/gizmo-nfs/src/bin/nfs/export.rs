//! `nfs export` — a car (or one configuration of it) as OBJ + MTL + PNG.

use crate::obj::{self, Material};
use crate::paths::{Car, Result};
use gizmo_nfs::parts::{group_of, select_car, CarConfig, Grp};
use gizmo_nfs::{AssetHash, NfsMeshPart, NfsTexture, Tpk};
use std::collections::BTreeMap;
use std::path::Path;

pub fn run(car: &Path, out: &Path, config: CarConfig, all: bool, want_textures: bool) -> Result<()> {
    let car = Car::resolve(car)?;
    let parts = car.parts()?;
    let selected: Vec<&NfsMeshPart> = if all {
        parts.iter().collect()
    } else {
        // Skip the parts that are never drawn (engine bay, underbody, livery decals): they
        // would import as geometry buried inside the body.
        select_car(&parts, &config).into_iter().filter(|p| group_of(&p.name) != Grp::Skip).collect()
    };
    if selected.is_empty() {
        let siblings = car.siblings();
        if siblings.is_empty() {
            return Err(format!("{}: nothing to export", car.name));
        }
        return Err(format!(
            "{}: its GEOMETRY.BIN holds no parts — this directory is a set, try one of: {}",
            car.name,
            siblings.join(", ")
        ));
    }
    let tpk = want_textures.then(|| car.textures()).flatten();

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mtl_name = format!("{}.mtl", car.name);
    let obj_name = format!("{}.obj", car.name);

    // ── Materials: one per (texture, shader) pair a run resolves to ──
    let plan = MaterialPlan::build(&selected, tpk.as_ref());
    let obj_text = obj::write_obj(&selected, &mtl_name, |p, run| plan.name_for(p, run));
    let mtl_text = obj::write_mtl(&plan.materials);

    write(&out.join(&obj_name), obj_text.as_bytes())?;
    write(&out.join(&mtl_name), mtl_text.as_bytes())?;

    let mut written = 0usize;
    if let Some(tpk) = &tpk {
        let dir = out.join("tex");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for hash in &plan.textures {
            if let Some(t) = tpk.texture(*hash) {
                write_png(&dir.join(png_name(t)), t)?;
                written += 1;
            }
        }
    }

    let tris: usize = selected.iter().map(|p| p.triangle_count()).sum();
    println!(
        "{}: {} parts, {tris} triangles, {} materials, {written} textures",
        car.name,
        selected.len(),
        plan.materials.len()
    );
    println!("  {}", out.join(&obj_name).display());
    println!("  {}", out.join(&mtl_name).display());
    if written > 0 {
        println!("  {}/*.png", out.join("tex").display());
    }
    Ok(())
}

/// Which material each run references, and the set of textures that implies.
struct MaterialPlan {
    materials: Vec<Material>,
    textures: Vec<AssetHash>,
    /// Material name per (part name, run index).
    by_run: BTreeMap<(String, usize), String>,
}

impl MaterialPlan {
    fn build(parts: &[&NfsMeshPart], tpk: Option<&Tpk>) -> Self {
        let mut plan =
            Self { materials: Vec::new(), textures: Vec::new(), by_run: BTreeMap::new() };
        let mut seen: BTreeMap<String, ()> = BTreeMap::new();
        for p in parts {
            let group = group_of(&p.name);
            let runs: Vec<(usize, Option<AssetHash>, u32)> = if p.materials.is_empty() {
                vec![(0, None, 0)]
            } else {
                p.materials.iter().enumerate().map(|(i, m)| (i, Some(m.hash), m.shader.0)).collect()
            };
            for (run, hash, shader) in runs {
                let tex = hash.filter(|h| tpk.is_some_and(|t| t.texture(*h).is_some()));
                let name = match tex {
                    Some(h) => format!("tex_{:08X}", h.0),
                    None => format!("{}_{shader:08X}", group_name(group)),
                };
                if seen.insert(name.clone(), ()).is_none() {
                    let image = tex.and_then(|h| tpk?.texture(h));
                    let map = image.map(|t| format!("tex/{}", png_name(t)));
                    let map_is_cutout =
                        image.is_some_and(|t| t.rgba.chunks_exact(4).any(|px| px[3] < 250));
                    if let Some(h) = tex {
                        plan.textures.push(h);
                    }
                    plan.materials.push(Material {
                        name: name.clone(),
                        map,
                        map_is_cutout,
                        diffuse: group_colour(group),
                    });
                }
                plan.by_run.insert((p.name.clone(), run), name);
            }
        }
        plan
    }

    fn name_for(&self, p: &NfsMeshPart, run: usize) -> String {
        self.by_run.get(&(p.name.clone(), run)).cloned().unwrap_or_else(|| "default".into())
    }
}

/// A plausible flat colour per material group, for the runs that carry no texture. The importer
/// gets something readable rather than uniform grey; the paint colour is deliberately neutral
/// since NFSU2 paint is a per-car choice, not part of the asset.
fn group_colour(g: Grp) -> [f32; 3] {
    match g {
        Grp::Paint => [0.55, 0.57, 0.62],
        Grp::Glass => [0.15, 0.18, 0.22],
        Grp::Chrome => [0.80, 0.82, 0.85],
        Grp::Headlight => [0.90, 0.92, 0.96],
        Grp::Brakelight => [0.72, 0.03, 0.03],
        Grp::Exhaust => [0.55, 0.56, 0.60],
        Grp::Wheel => [0.09, 0.09, 0.10],
        Grp::Trim | Grp::Skip => [0.10, 0.10, 0.11],
    }
}

fn group_name(g: Grp) -> &'static str {
    match g {
        Grp::Paint => "paint",
        Grp::Glass => "glass",
        Grp::Chrome => "chrome",
        Grp::Headlight => "headlight",
        Grp::Brakelight => "brakelight",
        Grp::Exhaust => "exhaust",
        Grp::Wheel => "wheel",
        Grp::Trim => "trim",
        Grp::Skip => "hidden",
    }
}

/// A texture's file name: its DebugName when it has a usable one, else its hash.
fn png_name(t: &NfsTexture) -> String {
    let stem: String = t.name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    if stem.is_empty() {
        format!("{:08X}.png", t.hash.0)
    } else {
        format!("{stem}_{:08X}.png", t.hash.0)
    }
}

fn write_png(path: &Path, t: &NfsTexture) -> Result<()> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), t.width, t.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| format!("{}: {e}", path.display()))?;
    w.write_image_data(&t.rgba).map_err(|e| format!("{}: {e}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}
