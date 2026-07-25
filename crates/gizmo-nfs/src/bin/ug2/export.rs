//! `ug2 export` — a car (or one configuration of it) as glTF and/or OBJ + MTL + PNG.
//!
//! Both formats by default: the `.glb` is one self-contained file with its textures inside it,
//! the OBJ is what the older tools around this game read. Neither is a conversion of the other —
//! they are two renderings of the same [`MaterialPlan`].

use crate::paths::{Car, Result};
use gizmo_nfs::export::{self, MaterialPlan};
use gizmo_nfs::parts::{group_of, select_car, CarConfig, Grp};
use gizmo_nfs::{NfsMeshPart, NfsTexture};
use std::path::Path;

pub fn run(
    car: &Path,
    out: &Path,
    config: CarConfig,
    all: bool,
    want_textures: bool,
    format: &str,
) -> Result<()> {
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
    let (want_obj, want_glb) = (format != "glb", format != "obj");
    let mtl_name = format!("{}.mtl", car.name);
    let obj_name = format!("{}.obj", car.name);
    let glb_name = format!("{}.glb", car.name);

    // ── Materials: one per (texture, shader) pair a run resolves to ──
    let plan = MaterialPlan::build(&selected, tpk.as_ref());

    if want_glb {
        let glb = export::write_glb(&selected, tpk.as_ref()).map_err(|e| format!("{glb_name}: {e}"))?;
        write(&out.join(&glb_name), &glb)?;
    }

    let mut written = 0usize;
    if want_obj {
        let obj_text = export::write_obj(&selected, &mtl_name, |p, run| plan.name_for(p, run));
        let mtl_text = export::write_mtl(&plan.materials);
        write(&out.join(&obj_name), obj_text.as_bytes())?;
        write(&out.join(&mtl_name), mtl_text.as_bytes())?;

        // Only the OBJ needs the images beside it; the `.glb` carries its own.
        if let Some(tpk) = &tpk {
            let dir = out.join("tex");
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            for hash in &plan.textures {
                if let Some(t) = tpk.texture(*hash) {
                    write_png(&dir.join(export::png_name(t)), t)?;
                    written += 1;
                }
            }
        }
    }

    let tris: usize = selected.iter().map(|p| p.triangle_count()).sum();
    outln!(
        "{}: {} parts, {tris} triangles, {} materials, {} textures",
        car.name,
        selected.len(),
        plan.materials.len(),
        plan.textures.len()
    );
    if want_glb {
        outln!("  {}", out.join(&glb_name).display());
    }
    if want_obj {
        outln!("  {}", out.join(&obj_name).display());
        outln!("  {}", out.join(&mtl_name).display());
    }
    if written > 0 {
        outln!("  {}/*.png", out.join("tex").display());
    }
    Ok(())
}

fn write_png(path: &Path, t: &NfsTexture) -> Result<()> {
    let bytes = export::png_bytes(t).map_err(|e| format!("{}: {e}", path.display()))?;
    write(path, &bytes)
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}
