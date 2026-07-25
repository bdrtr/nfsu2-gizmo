//! Writing indexed parts as Wavefront OBJ + MTL.
//!
//! Coordinates are NFSU2's own (x = length, y = width, z = height, Z-up) with each solid's
//! placement applied — the parser's contract is "as in file", and Z-up is what Blender reads
//! natively, so no axis fixup is invented here.
//!
//! One `o` group per part, one `usemtl` per material run, so a run that carries its own
//! texture stays separable in the importer.

use crate::placement::{part_centroid, place_dir, place_point, should_place};
use crate::types::NfsMeshPart;
use std::fmt::Write as _;

/// A material the OBJ references: either a texture map or a flat colour.
pub struct Material {
    /// `newmtl` name, referenced by `usemtl`.
    pub name: String,
    /// Relative path of the texture image, when the run resolved to one.
    pub map: Option<String>,
    /// Whether that image carries real transparency, and so should also drive the alpha
    /// channel (`map_d`). Binding an opaque image as an alpha map is harmless in principle but
    /// makes a whole panel vanish in an importer that reads the map's alpha loosely.
    pub map_is_cutout: bool,
    /// Diffuse colour used when there is no map.
    pub diffuse: [f32; 3],
}

/// Build the OBJ text for `parts`, naming each run's material via `material_for`.
///
/// `material_for(part, run_index)` returns the `newmtl` name to reference; a part with no
/// material list is emitted as a single run with index 0.
#[must_use]
pub fn write_obj(
    parts: &[&NfsMeshPart],
    mtl_file: &str,
    material_for: impl Fn(&NfsMeshPart, usize) -> String,
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# exported by gizmo-nfs — NFSU2 coordinates (x=length, y=width, z=height)");
    let _ = writeln!(s, "mtllib {mtl_file}");
    // OBJ indices are 1-based and global to the file, so each part appends to the running count.
    let mut base = 1usize;
    for p in parts {
        let apply = should_place(&p.transform, &part_centroid(p));
        let _ = writeln!(s, "\no {}", p.name);
        for v in &p.positions {
            let g = place_point(&p.transform, *v, apply);
            let _ = writeln!(s, "v {:.5} {:.5} {:.5}", g[0], g[1], g[2]);
        }
        for n in &p.normals {
            let g = place_dir(&p.transform, *n, apply);
            let _ = writeln!(s, "vn {:.5} {:.5} {:.5}", g[0], g[1], g[2]);
        }
        for uv in &p.uvs {
            // OBJ's V axis points up, the game's down.
            let _ = writeln!(s, "vt {:.5} {:.5}", uv[0], 1.0 - uv[1]);
        }
        let has_n = !p.normals.is_empty();
        let has_uv = !p.uvs.is_empty();
        let face = |s: &mut String, tri: &[u32]| {
            let _ = write!(s, "f");
            for &i in tri {
                let v = base + i as usize;
                match (has_uv, has_n) {
                    (true, true) => {
                        let _ = write!(s, " {v}/{v}/{v}");
                    }
                    (true, false) => {
                        let _ = write!(s, " {v}/{v}");
                    }
                    (false, true) => {
                        let _ = write!(s, " {v}//{v}");
                    }
                    (false, false) => {
                        let _ = write!(s, " {v}");
                    }
                }
            }
            let _ = writeln!(s);
        };
        if p.materials.is_empty() {
            let _ = writeln!(s, "usemtl {}", material_for(p, 0));
            for tri in p.indices.chunks_exact(3) {
                face(&mut s, tri);
            }
        } else {
            for (run, m) in p.materials.iter().enumerate() {
                let Some(slice) = p.indices.get(m.index_offset..m.index_offset + m.index_count)
                else {
                    continue;
                };
                let _ = writeln!(s, "usemtl {}", material_for(p, run));
                for tri in slice.chunks_exact(3) {
                    face(&mut s, tri);
                }
            }
        }
        base += p.positions.len();
    }
    s
}

/// Build the MTL text for a set of materials.
#[must_use]
pub fn write_mtl(materials: &[Material]) -> String {
    let mut s = String::from("# exported by gizmo-nfs\n");
    for m in materials {
        let _ = writeln!(s, "\nnewmtl {}", m.name);
        let _ = writeln!(s, "Kd {:.3} {:.3} {:.3}", m.diffuse[0], m.diffuse[1], m.diffuse[2]);
        let _ = writeln!(s, "Ka 0.000 0.000 0.000");
        let _ = writeln!(s, "d 1.0");
        let _ = writeln!(s, "illum 2");
        if let Some(map) = &m.map {
            let _ = writeln!(s, "map_Kd {map}");
            if m.map_is_cutout {
                let _ = writeln!(s, "map_d {map}");
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NfsMaterialRange;

    fn part(name: &str) -> NfsMeshPart {
        NfsMeshPart {
            name: name.to_string(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.25, 0.75]; 3],
            indices: vec![0, 1, 2],
            transform: crate::types::IDENTITY,
            ..NfsMeshPart::default()
        }
    }

    #[test]
    fn faces_are_one_based_and_run_on_after_the_first_part() {
        let (a, b) = (part("A"), part("B"));
        let obj = write_obj(&[&a, &b], "car.mtl", |_, _| "m".into());
        assert!(obj.contains("f 1/1/1 2/2/2 3/3/3"), "{obj}");
        // The second part's vertices continue the global numbering.
        assert!(obj.contains("f 4/4/4 5/5/5 6/6/6"), "{obj}");
    }

    #[test]
    fn each_material_run_gets_its_own_usemtl() {
        let mut p = part("A");
        p.indices = vec![0, 1, 2, 0, 1, 2];
        let run = |offset, count| NfsMaterialRange {
            index_offset: offset,
            index_count: count,
            ..NfsMaterialRange::default()
        };
        p.materials = vec![run(0, 3), run(3, 3)];
        let obj = write_obj(&[&p], "car.mtl", |_, run| format!("m{run}"));
        assert!(obj.contains("usemtl m0"));
        assert!(obj.contains("usemtl m1"));
    }

    #[test]
    fn v_axis_is_flipped_for_obj() {
        let obj = write_obj(&[&part("A")], "car.mtl", |_, _| "m".into());
        assert!(obj.contains("vt 0.25000 0.25000"), "{obj}");
    }

    #[test]
    fn mtl_references_its_map_when_there_is_one() {
        let mtl = write_mtl(&[
            Material {
                name: "tex".into(),
                map: Some("tex/1.png".into()),
                map_is_cutout: false,
                diffuse: [1.0; 3],
            },
            Material {
                name: "flat".into(),
                map: None,
                map_is_cutout: false,
                diffuse: [0.1, 0.2, 0.3],
            },
        ]);
        assert!(mtl.contains("newmtl tex") && mtl.contains("map_Kd tex/1.png"));
        assert!(mtl.contains("newmtl flat") && mtl.contains("Kd 0.100 0.200 0.300"));
        assert_eq!(mtl.matches("map_Kd").count(), 1);
    }
}
