//! Writing parts as a self-contained binary glTF (`.glb`).
//!
//! Why GLB rather than OBJ: it carries the material graph and the images *inside one file*, so a
//! car imports with its textures already attached instead of as geometry plus a folder the
//! importer has to be pointed at. That is the difference the project plan asks for.
//!
//! # Two conversions this writer does on purpose
//!
//! The parser's contract is "as in file", and `ug2 export`'s OBJ keeps NFSU2's own frame. glTF is
//! not free to do that: the format *defines* +Y as up and −Z as forward, and every viewer applies
//! it. So the vertices are rotated here — `(x, y, z)ₙ → (−y, z, −x)` — which is a right-handed
//! change of basis (determinant +1), not a mirror. Normals go through the same rotation.
//!
//! Texture coordinates, by contrast, are *not* flipped: glTF puts the UV origin at the top left,
//! which is where DirectX — and therefore NFSU2 — already has it. The OBJ writer flips V because
//! OBJ is the odd one out, not because the file is.
//!
//! # What ends up in the file
//!
//! One mesh per part, one primitive per material run, one node per mesh. Placement is **baked
//! into the vertices** rather than put on the node: a solid's matrix is sometimes a placement and
//! sometimes a pose already applied to the vertices (see [`crate::placement`]), so a file where
//! half the nodes carried a transform and half did not would be a trap for whoever imported it.

use crate::export::{png_bytes, png_name, MaterialPlan};
use crate::placement::{part_centroid, place_dir, place_point, should_place};
use crate::texture::Tpk;
use crate::types::{AssetHash, NfsMeshPart};
use crate::NfsResult;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// glTF component types.
const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;
/// glTF buffer-view targets.
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// Write `parts` as one `.glb`, resolving material textures against `tpk` and embedding them.
///
/// # Errors
/// Only from encoding a texture ([`png_bytes`]); the geometry path cannot fail.
pub fn write_glb(parts: &[&NfsMeshPart], tpk: Option<&Tpk>) -> NfsResult<Vec<u8>> {
    let plan = MaterialPlan::build(parts, tpk);
    let mut b = Builder::default();

    // ── Images first: a material can only reference a texture that already has an index ──
    let mut texture_index: BTreeMap<u32, usize> = BTreeMap::new();
    for hash in &plan.textures {
        let Some(tex) = tpk.and_then(|t| t.texture(*hash)) else { continue };
        let png = png_bytes(tex)?;
        let view = b.push_view(&png, None);
        let image = b.images.len();
        b.images.push(format!(
            "{{\"name\":{},\"mimeType\":\"image/png\",\"bufferView\":{view}}}",
            json_string(&png_name(tex))
        ));
        texture_index.insert(hash.0, image);
    }

    // ── Materials, in the plan's order so a name in the OBJ means the same thing here ──
    for m in &plan.materials {
        // A material's map is `tex/<png_name>`; find the image that was written for it.
        let image = m.map.as_ref().and_then(|map| {
            plan.textures.iter().find_map(|h| {
                let tex = tpk?.texture(*h)?;
                (format!("tex/{}", png_name(tex)) == *map).then(|| texture_index.get(&h.0))?
            })
        });
        b.materials.push(material_json(&m.name, m.diffuse, image.copied(), m.map_is_cutout));
    }

    // ── One mesh per part ──
    for p in parts {
        let apply = should_place(&p.transform, &part_centroid(p));
        let positions: Vec<[f32; 3]> =
            p.positions.iter().map(|v| to_gltf(place_point(&p.transform, *v, apply))).collect();
        if positions.is_empty() {
            continue;
        }
        let pos_acc = b.push_f32x3(&positions, true);
        let normal_acc = (p.normals.len() == p.positions.len()).then(|| {
            let n: Vec<[f32; 3]> =
                p.normals.iter().map(|v| to_gltf(place_dir(&p.transform, *v, apply))).collect();
            b.push_f32x3(&n, false)
        });
        // The UV list is trusted only when it covers every vertex — a short one would make the
        // accessor claim vertices it does not have.
        let uv_acc = (p.uvs.len() == p.positions.len()).then(|| b.push_f32x2(&p.uvs));

        let mut primitives: Vec<String> = Vec::new();
        let runs: Vec<(usize, &[u32])> = if p.materials.is_empty() {
            vec![(0, p.indices.as_slice())]
        } else {
            p.materials
                .iter()
                .enumerate()
                .filter_map(|(i, m)| {
                    p.indices.get(m.index_offset..m.index_offset + m.index_count).map(|s| (i, s))
                })
                .collect()
        };
        for (run, indices) in runs {
            if indices.len() < 3 {
                continue;
            }
            let idx_acc = b.push_u32(indices);
            let material = plan
                .materials
                .iter()
                .position(|m| m.name == plan.name_for(p, run))
                .unwrap_or_default();
            let mut attrs = format!("\"POSITION\":{pos_acc}");
            if let Some(a) = normal_acc {
                let _ = write!(attrs, ",\"NORMAL\":{a}");
            }
            if let Some(a) = uv_acc {
                let _ = write!(attrs, ",\"TEXCOORD_0\":{a}");
            }
            primitives.push(format!(
                "{{\"attributes\":{{{attrs}}},\"indices\":{idx_acc},\"material\":{material}}}"
            ));
        }
        if primitives.is_empty() {
            continue;
        }
        let mesh = b.meshes.len();
        b.meshes.push(format!(
            "{{\"name\":{},\"primitives\":[{}]}}",
            json_string(&p.name),
            primitives.join(",")
        ));
        b.nodes.push(format!("{{\"name\":{},\"mesh\":{mesh}}}", json_string(&p.name)));
    }

    Ok(b.finish())
}

/// NFSU2's frame (x = length, y = width, z = height, Z-up) in glTF's (+Y up, −Z forward).
fn to_gltf(v: [f32; 3]) -> [f32; 3] {
    [-v[1], v[2], -v[0]]
}

/// One material, as glTF's metallic-roughness model.
///
/// Cars are lit by the viewer, not by us: metallic is 0 and roughness a plausible mid value, which
/// reads as painted bodywork rather than as a mirror. A cutout texture drives `MASK` so a grille's
/// holes are holes and not black.
fn material_json(name: &str, diffuse: [f32; 3], image: Option<usize>, cutout: bool) -> String {
    let mut pbr = String::from("\"pbrMetallicRoughness\":{");
    match image {
        // A tinted texture would double-apply the group colour; the map is the colour.
        Some(i) => {
            let _ = write!(
                pbr,
                "\"baseColorFactor\":[1,1,1,1],\"baseColorTexture\":{{\"index\":{i}}}"
            );
        }
        None => {
            let c = diffuse.map(srgb_to_linear);
            let _ =
                write!(pbr, "\"baseColorFactor\":[{:.4},{:.4},{:.4},1]", c[0], c[1], c[2]);
        }
    }
    let _ = write!(pbr, ",\"metallicFactor\":0,\"roughnessFactor\":0.55}}");
    let alpha = if cutout { ",\"alphaMode\":\"MASK\",\"alphaCutoff\":0.5" } else { "" };
    format!("{{\"name\":{},{pbr}{alpha},\"doubleSided\":true}}", json_string(name))
}

/// glTF colour factors are linear; the group colours are written the way a person reads them.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The JSON string form of `s`, with the characters JSON does not allow raw.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Accumulates the binary chunk and the JSON arrays that point into it.
#[derive(Default)]
struct Builder {
    bin: Vec<u8>,
    views: Vec<String>,
    accessors: Vec<String>,
    meshes: Vec<String>,
    nodes: Vec<String>,
    materials: Vec<String>,
    images: Vec<String>,
}

impl Builder {
    /// Append `bytes` as a buffer view, 4-byte aligned so every accessor inside it is aligned too.
    fn push_view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        let target = target.map(|t| format!(",\"target\":{t}")).unwrap_or_default();
        self.views.push(format!(
            "{{\"buffer\":0,\"byteOffset\":{offset},\"byteLength\":{}{target}}}",
            bytes.len()
        ));
        self.views.len() - 1
    }

    fn push_f32x3(&mut self, data: &[[f32; 3]], with_bounds: bool) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 12);
        for v in data {
            for c in v {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, Some(ARRAY_BUFFER));
        // POSITION must carry min/max — a viewer frames the model from them.
        let bounds = if with_bounds {
            let mut lo = [f32::INFINITY; 3];
            let mut hi = [f32::NEG_INFINITY; 3];
            for v in data {
                for i in 0..3 {
                    lo[i] = lo[i].min(v[i]);
                    hi[i] = hi[i].max(v[i]);
                }
            }
            format!(
                ",\"min\":[{:.6},{:.6},{:.6}],\"max\":[{:.6},{:.6},{:.6}]",
                lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
            )
        } else {
            String::new()
        };
        self.accessors.push(format!(
            "{{\"bufferView\":{view},\"componentType\":{FLOAT},\"count\":{},\"type\":\"VEC3\"{bounds}}}",
            data.len()
        ));
        self.accessors.len() - 1
    }

    fn push_f32x2(&mut self, data: &[[f32; 2]]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 8);
        for v in data {
            for c in v {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, Some(ARRAY_BUFFER));
        self.accessors.push(format!(
            "{{\"bufferView\":{view},\"componentType\":{FLOAT},\"count\":{},\"type\":\"VEC2\"}}",
            data.len()
        ));
        self.accessors.len() - 1
    }

    fn push_u32(&mut self, data: &[u32]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 4);
        for i in data {
            bytes.extend_from_slice(&i.to_le_bytes());
        }
        let view = self.push_view(&bytes, Some(ELEMENT_ARRAY_BUFFER));
        self.accessors.push(format!(
            "{{\"bufferView\":{view},\"componentType\":{UNSIGNED_INT},\"count\":{},\"type\":\"SCALAR\"}}",
            data.len()
        ));
        self.accessors.len() - 1
    }

    /// The finished `.glb`: a 12-byte header, the JSON chunk, then the binary chunk.
    fn finish(self) -> Vec<u8> {
        let scene_nodes: Vec<String> = (0..self.nodes.len()).map(|i| i.to_string()).collect();
        let sampler = if self.images.is_empty() {
            String::new()
        } else {
            // One sampler for everything: repeat, trilinear. NFSU2 wraps its maps.
            format!(
                ",\"samplers\":[{{\"magFilter\":9729,\"minFilter\":9987,\"wrapS\":10497,\"wrapT\":10497}}],\"images\":[{}],\"textures\":[{}]",
                self.images.join(","),
                (0..self.images.len())
                    .map(|i| format!("{{\"sampler\":0,\"source\":{i}}}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let json = format!(
            concat!(
                "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"gizmo-nfs\"}}",
                ",\"scene\":0,\"scenes\":[{{\"nodes\":[{}]}}]",
                ",\"nodes\":[{}],\"meshes\":[{}],\"materials\":[{}]",
                ",\"accessors\":[{}],\"bufferViews\":[{}]",
                ",\"buffers\":[{{\"byteLength\":{}}}]{}}}"
            ),
            scene_nodes.join(","),
            self.nodes.join(","),
            self.meshes.join(","),
            self.materials.join(","),
            self.accessors.join(","),
            self.views.join(","),
            self.bin.len(),
            sampler,
        );

        let mut json = json.into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' '); // the JSON chunk pads with spaces, the binary one with zeroes
        }
        let mut bin = self.bin;
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }

        let total = 12 + 8 + json.len() + 8 + bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }
}

/// The hashes whose images a `.glb` of these parts would embed — what a caller needs to have
/// decoded before calling [`write_glb`].
#[must_use]
pub fn referenced_textures(parts: &[&NfsMeshPart], tpk: Option<&Tpk>) -> Vec<AssetHash> {
    MaterialPlan::build(parts, tpk).textures
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NfsMaterialRange, IDENTITY};

    fn part(name: &str) -> NfsMeshPart {
        NfsMeshPart {
            name: name.to_string(),
            positions: vec![[1.0, 2.0, 3.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.25, 0.75]; 3],
            indices: vec![0, 1, 2],
            transform: IDENTITY,
            materials: vec![NfsMaterialRange { index_count: 3, ..NfsMaterialRange::default() }],
            ..NfsMeshPart::default()
        }
    }

    /// The chunk lengths in the header have to agree with the bytes that follow, or no importer
    /// will read the file at all.
    #[test]
    fn the_container_is_a_well_formed_glb() {
        let p = part("A");
        let glb = write_glb(&[&p], None).expect("geometry-only glb");
        assert_eq!(&glb[..4], b"glTF");
        let u32_at = |o: usize| u32::from_le_bytes(glb[o..o + 4].try_into().unwrap()) as usize;
        assert_eq!(u32_at(4), 2, "version");
        assert_eq!(u32_at(8), glb.len(), "declared length is the real length");
        let json_len = u32_at(12);
        assert_eq!(&glb[16..20], b"JSON");
        assert_eq!(json_len % 4, 0, "chunks are 4-byte aligned");
        let bin_len = u32_at(20 + json_len);
        assert_eq!(&glb[24 + json_len..28 + json_len], b"BIN\0");
        assert_eq!(28 + json_len + bin_len, glb.len());
    }

    /// Z-up to Y-up, as a rotation: a point one metre up must come out one metre up.
    #[test]
    fn the_frame_becomes_gltfs_own() {
        assert_eq!(to_gltf([0.0, 0.0, 1.0]), [0.0, 1.0, 0.0], "up stays up");
        assert_eq!(to_gltf([1.0, 0.0, 0.0]), [0.0, 0.0, -1.0], "the nose points forward");
        // A rotation, not a mirror: the basis keeps its handedness.
        let (x, y, z) = (to_gltf([1.0, 0.0, 0.0]), to_gltf([0.0, 1.0, 0.0]), to_gltf([0.0, 0.0, 1.0]));
        let cross = [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ];
        let dot = cross[0] * z[0] + cross[1] * z[1] + cross[2] * z[2];
        assert!(dot > 0.0, "x × y must still point along z, got {dot}");
    }

    /// UVs are written through unchanged — glTF and DirectX put the origin in the same corner.
    #[test]
    fn uvs_are_not_flipped() {
        let p = part("A");
        let glb = write_glb(&[&p], None).expect("glb");
        let bin_start = glb.len() - {
            let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
            u32::from_le_bytes(glb[20 + json_len..24 + json_len].try_into().unwrap()) as usize
        };
        let floats: Vec<f32> = glb[bin_start..]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert!(floats.windows(2).any(|w| (w[0] - 0.25).abs() < 1e-6 && (w[1] - 0.75).abs() < 1e-6));
    }

    /// A part whose UV list is short must not have its TEXCOORD accessor claim vertices that are
    /// not there — that is the kind of file that crashes an importer.
    #[test]
    fn a_short_uv_list_is_left_out_rather_than_half_written() {
        let mut p = part("A");
        p.uvs.truncate(1);
        let glb = write_glb(&[&p], None).expect("glb");
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json = std::str::from_utf8(&glb[20..20 + json_len]).expect("utf8 json");
        assert!(!json.contains("TEXCOORD_0"), "{json}");
        assert!(json.contains("POSITION"));
    }
}
