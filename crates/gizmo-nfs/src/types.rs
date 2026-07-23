//! The engine-agnostic output data contract.
//!
//! These are the plain-CPU structures the parser hands back — no `glam`, no `wgpu`. An
//! integration layer (a demo binary or an optional `gizmo-nfs-engine` crate) turns them
//! into engine meshes/materials: it expands the *indexed* geometry here into the flat,
//! non-indexed vertex list the renderer wants, and uploads each [`NfsTexture`]'s RGBA to
//! a GPU texture. Keeping this layer pure-data is what lets `gizmo-nfs` sit at the very
//! bottom of the workspace with no engine dependencies.

/// A 4x4 transform as stored in the file (row-major), with its original handedness and
/// scale. Coordinate-system fixups are the integration layer's decision — deliberately
/// not baked in here.
pub type Mat4 = [[f32; 4]; 4];

/// The identity transform.
pub const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// A 32-bit asset hash — NFSU2 keys textures/materials/strings by these. Kept opaque;
/// render it with [`crate::fourcc::FourCc`] or as hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssetHash(pub u32);

/// LOD classification derived from a part's name (`CAR_BASE_A` .. `CAR_BASE_D`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LodLevel {
    /// Highest detail.
    A,
    /// Second detail level.
    B,
    /// Third detail level.
    C,
    /// Lowest detail.
    D,
    /// Not derivable from the name.
    #[default]
    Unknown,
}

/// A part's role within the car, parsed from its name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PartRole {
    /// The base body (`CAR_BASE_*`).
    Body,
    /// A customization kit part (`CAR_KIT##_*`); `slot` is the kit number.
    Kit {
        /// The kit slot number.
        slot: u16,
    },
    /// A mount / dummy point (`_MOUNTPOINTNAME##`) — attach points, not renderable.
    MountPoint {
        /// The mount point's name.
        name: String,
    },
    /// A wheel part.
    Wheel,
    /// Unrecognised role.
    #[default]
    Unknown,
}

/// One renderable "solid": indexed geometry plus a material reference.
///
/// The attribute arrays are parallel; `normals` and/or `uvs` may be empty when the source
/// omits them. `indices` is a triangle list (any strips are normalised to a list by the
/// parser).
/// One contiguous run of a part's index buffer that shares a single material/texture.
///
/// From the mesh's `0x00134B02` material list: a solid's triangles are grouped by material,
/// and each group is a `[index_offset, index_offset + index_count)` slice of
/// [`NfsMeshPart::indices`]. `hash` resolves against a [`crate::Tpk`] — a run whose hash is a
/// texture is that region's diffuse; a run whose hash does not resolve is a shader-only
/// material (body paint, glass, chrome). Lets the integration layer split one baked mesh into
/// its textured sub-regions (headlights, brake lights, interior) instead of one flat surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NfsMaterialRange {
    /// The material/texture hash for this run.
    pub hash: AssetHash,
    /// Start index into [`NfsMeshPart::indices`].
    pub index_offset: usize,
    /// Number of indices in this run (a multiple of 3).
    pub index_count: usize,
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NfsMeshPart {
    /// The part's name as stored in the file.
    pub name: String,
    /// The part's own hash.
    pub hash: AssetHash,
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals (empty if absent).
    pub normals: Vec<[f32; 3]>,
    /// Vertex texture coordinates (empty if absent).
    pub uvs: Vec<[f32; 2]>,
    /// Triangle-list indices into the attribute arrays.
    pub indices: Vec<u32>,
    /// Asset hashes this part references (from the solid's material list). Resolve each
    /// against a [`crate::Tpk`] to find the part's texture — typically the first that
    /// resolves is the diffuse map; the rest are shader/material hashes not in the texture
    /// pack.
    pub material_refs: Vec<AssetHash>,
    /// Per-material index sub-ranges (from `0x00134B02`), in file order. Empty if the mesh
    /// carries no material list; otherwise the runs tile [`Self::indices`] end to end.
    pub materials: Vec<NfsMaterialRange>,
    /// The part's role in the car hierarchy.
    pub role: PartRole,
    /// The part's LOD level.
    pub lod: LodLevel,
    /// The part's local transform, as stored in the file.
    pub transform: Mat4,
    /// Axis-aligned bounding-box minimum.
    pub bbox_min: [f32; 3],
    /// Axis-aligned bounding-box maximum.
    pub bbox_max: [f32; 3],
}

/// The pixel layout of a decoded texture. Output is always `Rgba8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PixelFormat {
    /// 8 bits per channel, RGBA order.
    Rgba8,
}

/// The source (on-disk) format a texture was decoded from — kept for debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TexFormat {
    /// S3TC / BC1.
    Dxt1,
    /// S3TC / BC2.
    Dxt3,
    /// S3TC / BC3.
    Dxt5,
    /// 8-bit palettized.
    P8,
    /// An unrecognised format tag (value preserved for reverse-engineering).
    Unknown(u32),
}

/// A decoded texture: always RGBA8, top mip only.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NfsTexture {
    /// The texture's name.
    pub name: String,
    /// The texture's hash (its key in [`NfsCar::textures`]).
    pub hash: AssetHash,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes of RGBA8 pixels.
    pub rgba: Vec<u8>,
    /// The on-disk format this was decoded from.
    pub source_format: TexFormat,
    /// The output pixel format (always `Rgba8` for now).
    pub format: PixelFormat,
}

/// A parsed car: its renderable parts plus a hash-keyed texture table for material
/// resolution.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NfsCar {
    /// The car's renderable parts.
    pub parts: Vec<NfsMeshPart>,
    /// Decoded textures keyed by hash.
    pub textures: std::collections::HashMap<AssetHash, NfsTexture>,
}

impl NfsMeshPart {
    /// Total triangle count (`indices.len() / 3`).
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Whether every index is within range of the position array — the objective check
    /// used when reverse-engineering the vertex layout (a correct layout yields all
    /// in-range indices).
    #[must_use]
    pub fn indices_in_range(&self) -> bool {
        let n = self.positions.len() as u32;
        self.indices.iter().all(|&i| i < n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_count_and_range_check() {
        let part = NfsMeshPart {
            positions: vec![[0.0; 3], [1.0; 3], [2.0; 3]],
            indices: vec![0, 1, 2],
            ..Default::default()
        };
        assert_eq!(part.triangle_count(), 1);
        assert!(part.indices_in_range());

        let bad = NfsMeshPart { positions: vec![[0.0; 3]], indices: vec![0, 1, 2], ..Default::default() };
        assert!(!bad.indices_in_range());
    }

    #[test]
    fn defaults_are_sane() {
        let part = NfsMeshPart::default();
        assert_eq!(part.hash, AssetHash(0));
        assert_eq!(part.role, PartRole::Unknown);
        assert_eq!(part.lod, LodLevel::Unknown);
        // The Default derive yields a zeroed Mat4 (not identity); the parser always
        // overwrites it, so a zero default is fine — assert it explicitly.
        assert_eq!(part.transform, [[0.0f32; 4]; 4]);
    }
}
