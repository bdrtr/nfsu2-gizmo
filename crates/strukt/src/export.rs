//! `Dışa Aktar` — writing what is on screen to disk.
//!
//! STRUKT carries no format knowledge of its own: the OBJ/MTL text and the PNG bytes come from
//! [`gizmo_nfs::export`], the same code `ug2 export` runs, so the two tools cannot end up
//! disagreeing about what a car is. What lives here is only *where* the files go and *what is on
//! screen* — and the second question is answered exactly as the 3D tab answers it, so an export
//! writes the thing the viewport was showing.
//!
//! There is no file dialog (see `Cargo.toml`): the files land under `strukt-export/` in the
//! working directory and the log says the full path, the same way `ug2` prints what it wrote.

use crate::app::{Strukt, Tab};
use crate::doc::Doc;
use crate::i18n::Strings;
use gizmo_nfs::export::{self, MaterialPlan};
use gizmo_nfs::NfsMeshPart;
use std::path::{Path, PathBuf};

/// What an export produced.
pub struct Written {
    /// A one-line account of the contents, for the log.
    pub summary: String,
    /// Where it went — the first path is what the log points at.
    pub files: Vec<PathBuf>,
}

/// Write what the centre area is currently showing.
///
/// # Errors
/// Returns a human-readable message when there is nothing to write (no file open, no texture
/// selected, a car whose `GEOMETRY.BIN` holds no parts) or when a write fails.
pub fn run(app: &mut Strukt) -> Result<Written, String> {
    // Decoding is lazy, and the texture tab may never have been opened; ask first, then read the
    // result immutably so the parts and the textures can be looked at together.
    if let Some(doc) = &mut app.doc {
        let _ = doc.textures();
    }
    let t = app.lang.strings();
    let doc = app.doc.as_ref().ok_or("no file")?;
    let out = out_dir(doc)?;
    let has_images =
        doc.decoded_textures().is_some_and(|tpk| !tpk.textures.is_empty());
    match app.tab {
        Tab::Texture => textures(doc, &out, t),
        // A TPK has only its textures to give, whichever tab happens to be open — refusing to
        // export one because the hex tab was in front would be pedantry, not fidelity.
        _ if doc.parts.is_empty() && has_images => textures(doc, &out, t),
        _ => model(doc, app.selection, &out, t),
    }
}

/// One texture as a PNG — what the preview pane is showing.
///
/// # Errors
/// When the texture is no longer in the pack, cannot be encoded, or cannot be written.
pub fn one_texture(app: &Strukt, hash: gizmo_nfs::AssetHash) -> Result<Written, String> {
    let doc = app.doc.as_ref().ok_or("no file")?;
    let tpk = doc.decoded_textures().ok_or("no textures")?;
    let tex = tpk.texture(hash).ok_or("no such texture")?;
    let out = out_dir(doc)?;
    create_dir(&out)?;
    let path = out.join(export::png_name(tex));
    let bytes = export::png_bytes(tex).map_err(|e| format!("{}: {e}", path.display()))?;
    write(&path, &bytes)?;
    Ok(Written {
        summary: format!("{} × {} PNG", tex.width, tex.height),
        files: vec![path],
    })
}

/// Every decoded texture in the pack, as PNGs in one folder.
fn textures(doc: &Doc, out: &Path, t: &Strings) -> Result<Written, String> {
    let tpk = doc.decoded_textures().ok_or("no textures")?;
    if tpk.textures.is_empty() {
        return Err("no textures were decoded".into());
    }
    let dir = out.join("tex");
    create_dir(&dir)?;
    let mut files = Vec::new();
    for tex in tpk.textures.values() {
        let path = dir.join(export::png_name(tex));
        let bytes = export::png_bytes(tex).map_err(|e| format!("{}: {e}", path.display()))?;
        write(&path, &bytes)?;
        files.push(path);
    }
    // Say what was left behind: entries the parser could not decode are not written, and a folder
    // with fewer files than the pack has textures should not have to be noticed by counting.
    let undecoded = tpk.entries.len().saturating_sub(tpk.textures.len());
    let mut summary = format!("{} PNG", files.len());
    if undecoded > 0 {
        summary.push_str(&format!(" ({undecoded} {})", t.textures_undecoded));
    }
    files.sort();
    Ok(Written { summary, files })
}

/// The parts the 3D tab would show, as OBJ + MTL + the textures they reference.
fn model(doc: &Doc, selection: Option<usize>, out: &Path, t: &Strings) -> Result<Written, String> {
    let parts = shown_parts(doc, selection);
    if parts.is_empty() {
        return Err("this file holds no parts to export".into());
    }
    let tpk = doc.decoded_textures();
    let stem = stem(doc);
    let mtl_name = format!("{stem}.mtl");

    let plan = MaterialPlan::build(&parts, tpk);
    let obj_text = export::write_obj(&parts, &mtl_name, |p, run| plan.name_for(p, run));
    let mtl_text = export::write_mtl(&plan.materials);

    create_dir(out)?;
    // The `.glb` first: it is the one file someone can drag into a viewer and see the car, images
    // and all. The OBJ beside it is for the older tools around this game.
    let glb_path = out.join(format!("{stem}.glb"));
    let glb = export::write_glb(&parts, tpk).map_err(|e| format!("{}: {e}", glb_path.display()))?;
    write(&glb_path, &glb)?;
    let obj_path = out.join(format!("{stem}.obj"));
    let mtl_path = out.join(&mtl_name);
    write(&obj_path, obj_text.as_bytes())?;
    write(&mtl_path, mtl_text.as_bytes())?;
    let mut files = vec![glb_path, obj_path, mtl_path];

    if let Some(tpk) = tpk {
        let dir = out.join("tex");
        create_dir(&dir)?;
        for hash in &plan.textures {
            if let Some(tex) = tpk.texture(*hash) {
                let path = dir.join(export::png_name(tex));
                let bytes = export::png_bytes(tex).map_err(|e| format!("{}: {e}", path.display()))?;
                write(&path, &bytes)?;
                files.push(path);
            }
        }
    }

    let tris: usize = parts.iter().map(|p| p.triangle_count()).sum();
    Ok(Written {
        summary: format!(
            "GLB + OBJ · {} {} · {tris} ▲ · {} {} · {} PNG",
            parts.len(),
            t.ex_parts,
            plan.materials.len(),
            t.ex_materials,
            files.len().saturating_sub(3)
        ),
        files,
    })
}

/// What the 3D tab is showing: the solid the selection sits in, else the showroom car. Kept in
/// step with `panels::viewport3d` on purpose — an export that wrote something else would make the
/// viewport a lie.
fn shown_parts(doc: &Doc, selection: Option<usize>) -> Vec<&NfsMeshPart> {
    match selection.and_then(|o| doc.solid_of(o)) {
        Some(solid) => {
            let name = solid
                .find(gizmo_nfs::geometry::format::SOLID_HEADER)
                .map(|h| gizmo_nfs::geometry::part_name(h.data(&doc.bytes)))
                .unwrap_or_default();
            doc.parts.iter().filter(|p| p.name == name).collect()
        }
        None => gizmo_nfs::select_stock_car(&doc.parts),
    }
}

/// `strukt-export/<car>_<file>/` under the working directory. The car folder is in the name
/// because every car's geometry file is called `GEOMETRY.BIN`, and two exports must not land on
/// top of each other.
fn out_dir(doc: &Doc) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("working directory: {e}"))?;
    Ok(cwd.join("strukt-export").join(stem(doc)))
}

fn stem(doc: &Doc) -> String {
    let file = doc.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let car = doc
        .path
        .parent()
        .and_then(Path::file_name)
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = if car.is_empty() { file } else { format!("{car}_{file}") };
    name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect()
}

fn create_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}
