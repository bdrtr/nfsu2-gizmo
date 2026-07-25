//! Which material each run of a part resolves to, and the textures that implies.
//!
//! A material run names a texture by hash and a shader by hash; an exporter has to turn that pair
//! into something an importer understands. Runs that resolve to the same texture share one
//! `newmtl`, and a run with no usable texture falls back to a flat colour chosen from the part's
//! material group — so a car opens in Blender readable rather than uniformly grey.

use crate::export::{obj::Material, png_name};
use crate::parts::{group_of, Grp};
use crate::texture::Tpk;
use crate::types::{AssetHash, NfsMeshPart};
use std::collections::BTreeMap;

/// The material set an export needs, and the mapping from a part's run to a `newmtl` name.
pub struct MaterialPlan {
    /// One entry per distinct material, in the order they were first met.
    pub materials: Vec<Material>,
    /// The textures the materials reference — what an exporter has to write out.
    pub textures: Vec<AssetHash>,
    /// Material name per (part name, run index).
    by_run: BTreeMap<(String, usize), String>,
}

impl MaterialPlan {
    /// Work out the materials `parts` need, resolving texture hashes against `tpk` when there is
    /// one. A hash the TPK does not hold falls back to the group colour rather than to a `map_Kd`
    /// pointing at a file nobody will write.
    #[must_use]
    pub fn build(parts: &[&NfsMeshPart], tpk: Option<&Tpk>) -> Self {
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

    /// The `newmtl` name a run references — what [`write_obj`](crate::export::write_obj) asks for.
    #[must_use]
    pub fn name_for(&self, p: &NfsMeshPart, run: usize) -> String {
        self.by_run.get(&(p.name.clone(), run)).cloned().unwrap_or_else(|| "default".into())
    }
}

/// A plausible flat colour per material group, for the runs that carry no texture. The importer
/// gets something readable rather than uniform grey; the paint colour is deliberately neutral
/// since NFSU2 paint is a per-car choice, not part of the asset.
#[must_use]
pub fn group_colour(g: Grp) -> [f32; 3] {
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

/// The group's name as it appears in a material name.
#[must_use]
pub fn group_name(g: Grp) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NfsMaterialRange;

    fn part(name: &str, runs: &[(u32, u32)]) -> NfsMeshPart {
        NfsMeshPart {
            name: name.to_string(),
            indices: vec![0, 1, 2],
            materials: runs
                .iter()
                .map(|&(tex, shader)| NfsMaterialRange {
                    hash: AssetHash(tex),
                    shader: AssetHash(shader),
                    index_count: 3,
                    ..NfsMaterialRange::default()
                })
                .collect(),
            ..NfsMeshPart::default()
        }
    }

    // With no TPK to resolve against, every run falls back to its group's flat colour — and two
    // runs of the same group and shader share one material rather than duplicating it.
    #[test]
    fn untextured_runs_collapse_by_group_and_shader() {
        let a = part("CAR_KIT00_BODY_A", &[(1, 0xAA), (2, 0xAA)]);
        let plan = MaterialPlan::build(&[&a], None);
        assert_eq!(plan.materials.len(), 1);
        assert_eq!(plan.materials[0].name, "paint_000000AA");
        assert!(plan.textures.is_empty());
        assert_eq!(plan.name_for(&a, 1), "paint_000000AA");
    }

    // Different groups keep different colours, so an importer can tell glass from paint.
    #[test]
    fn each_group_gets_its_own_colour() {
        let body = part("CAR_KIT00_BODY_A", &[(1, 1)]);
        let glass = part("CAR_KIT00_WINDOW_FRONT_A", &[(1, 1)]);
        let plan = MaterialPlan::build(&[&body, &glass], None);
        assert_eq!(plan.materials.len(), 2);
        assert_ne!(plan.materials[0].diffuse, plan.materials[1].diffuse);
    }

    // A part with no material list at all is still one run, and still gets a name.
    #[test]
    fn a_part_without_materials_is_a_single_run() {
        let p = part("CAR_KIT00_BODY_A", &[]);
        let plan = MaterialPlan::build(&[&p], None);
        assert_eq!(plan.materials.len(), 1);
        assert_eq!(plan.name_for(&p, 0), "paint_00000000");
    }
}
