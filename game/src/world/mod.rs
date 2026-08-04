//! The city, turned into Gizmo meshes.
//!
//! This sits **beside** [`crate::car`], never through it. The car path looks reusable and is not:
//! `parts::group_of` keyword-matches car vocabulary, so a building's `ROOF` and `DOOR` would come
//! out painted the car's palette colour and its `WINDOW` translucent at alpha 0.32; `skin`'s
//! longest-common-prefix matching would bind `TRN_ROADA_CHOP_*` to unrelated meshes citywide; and
//! `placement::should_place` refuses every `det < 0`, which is right for pre-mirrored car solids
//! and would drop the city's 106 mirrored buildings on the origin. World data is not ambiguous:
//! identity means the vertices are already in world space, anything else is a transform to apply.
//!
//! The one car mechanism that does generalise is the link itself — a `0x00134012` slot key looked
//! up in a texture pack — and it generalises because it is a byte fact rather than a heuristic.
//!
//! ## What this does, and why in this order
//!
//! 1. **Dedup before anything downstream sees the geometry.** 5,364 of the city's 28,985 objects
//!    are exact duplicates stacked at the same place. Drawn, they are invisible except as z-fighting
//!    and doubled draw calls, and diagnosing that as a depth problem costs a week.
//! 2. **Bucket into cells.** A city-sized mesh has a city-sized bounding box, so a renderer's
//!    frustum cull can never reject it and the whole world is submitted every frame.
//! 3. **Merge per (cell, texture).** This is what collapses thousands of solids into a few draws:
//!    batching keyed on the buffer does nothing when every scenery chunk is its own buffer.
//! 4. **Recentre each cell on its own centroid.** Building world-space vertices and spawning at the
//!    origin — which is what the car path does, correctly, for a car — would give every cell the
//!    whole city's bounds again and undo step 2.

mod build;
mod cell;

pub use build::{build_region, CityMesh, CityVisuals};
pub use cell::{cell_centre, cell_of, CELL_SIZE};

use gizmo::prelude::*;
use gizmo_nfs::world::{WorldMesh, WorldSolidHeader};

/// NFSU2 world space (Z-up) → Gizmo (Y-up), the same conversion [`crate::geom::remap`] applies to a
/// car. Kept here rather than called there because that one is documented in car terms ("length →
/// −Z (forward)") and the city has no forward; the arithmetic is identical and its determinant is
/// `+1`, so triangle winding survives and the city can be backface-culled.
#[inline]
#[must_use]
pub fn remap(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
}

/// Apply a world object's own transform to one of its vertices.
///
/// The matrix is stored as in the file: row-vector convention, basis in rows 0–2 and translation in
/// row 3. So a point goes `x·row0 + y·row1 + z·row2 + row3`, with no transposition — the layout is
/// byte-identical to a column-major OpenGL matrix, which is why applying it as-is is right and
/// transposing it puts every object at the origin.
///
/// Identity is not a special case that needs detecting for correctness — it is the identity — but
/// it is checked anyway because 12,914 of the city's objects carry it and skipping nine multiplies
/// and three adds per vertex on 44 % of the world is worth one comparison per object.
#[must_use]
pub fn place(header: &WorldSolidHeader, p: [f32; 3]) -> [f32; 3] {
    if !header.is_placed() {
        return p;
    }
    let m = &header.matrix;
    [
        p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0] + m[3][0],
        p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1] + m[3][1],
        p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2] + m[3][2],
    ]
}

/// A vertex's final position in the Gizmo frame.
#[inline]
#[must_use]
pub fn world_point(header: &WorldSolidHeader, p: [f32; 3]) -> Vec3 {
    remap(place(header, p))
}

/// Drop objects that are exact duplicates of one already seen.
///
/// The key is the object's own key plus its counts and its bounding box, so an instanced prop
/// placed somewhere else keeps its own entry — its box differs — while a mesh genuinely emitted
/// twice does not. Measured city-wide this removes 5,364 of 28,985; L4RD alone has 2,652.
///
/// It runs before cells, textures or meshes, because every one of those costs more once and the
/// duplicates are indistinguishable from the originals by then.
#[must_use]
pub fn dedup(meshes: Vec<WorldMesh>) -> Vec<WorldMesh> {
    let mut seen = std::collections::HashSet::new();
    meshes
        .into_iter()
        .filter(|m| {
            let bits = |v: [f32; 3]| [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()];
            seen.insert((
                m.header.hash.0,
                m.positions.len(),
                m.indices.len(),
                bits(m.header.bbox_min),
                bits(m.header.bbox_max),
            ))
        })
        .collect()
}

/// Whether an object is backdrop rather than city: the sky shell and the painted panorama.
///
/// This is a **render category, not a parser fact** — the file does not label them, and what to do
/// with them is a decision about how the world is drawn. It lives here because the decision needs
/// making somewhere and the alternative is each binary inventing its own substring list.
///
/// Two families, found by measuring the largest objects in the loaded city rather than by guessing:
///
/// - `SKYDOME` — two per region, 40 across the eight bundles. Their texture keys live in
///   `TRACKS/LOC4DYNTEX.BIN`, not in the region's own packs.
/// - `PAN_*` and `*PANARAMA*` — the distant backdrop: `PAN_SKYLINE`, `PAN_OCEAN`, `PAN_HILLRIDGE`,
///   `PAN_HILLS_C98`, `TRN_PANARAMA_AIRPORT`. (The game spells it "PANARAMA".) These span 12,000 to
///   18,000 units against a district's few thousand, so drawn as ordinary geometry from inside the
///   city they are a wall across the whole frame.
///
/// Both want the same treatment a real renderer gives a backdrop — drawn first, camera-locked,
/// depth writes off — and until something does that, drawing them at all is worse than not.
#[must_use]
pub fn is_backdrop(name: &str) -> bool {
    name.contains("SKYDOME") || name.starts_with("PAN_") || name.contains("PANARAMA")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizmo_nfs::types::{AssetHash, IDENTITY};

    fn header(hash: u32, placed: bool) -> WorldSolidHeader {
        let mut matrix = IDENTITY;
        if placed {
            matrix[3] = [10.0, 20.0, 30.0, 1.0];
        }
        WorldSolidHeader {
            hash: AssetHash(hash),
            name: String::new(),
            bbox_min: [0.0; 3],
            bbox_max: [1.0; 3],
            matrix,
        }
    }

    #[test]
    fn an_identity_object_is_already_in_world_space() {
        let h = header(1, false);
        assert_eq!(place(&h, [3.0, 4.0, 5.0]), [3.0, 4.0, 5.0]);
    }

    /// The translation is in row 3 and is applied as-is. Reading the transpose instead is the
    /// classic failure here: it yields zero translation for every object, so the whole city piles
    /// up at the origin while every individual matrix still looks plausible.
    #[test]
    fn a_placed_object_takes_its_translation_from_row_three() {
        let h = header(2, true);
        assert_eq!(place(&h, [0.0, 0.0, 0.0]), [10.0, 20.0, 30.0]);
        assert_eq!(place(&h, [1.0, 2.0, 3.0]), [11.0, 22.0, 33.0]);
    }

    /// Z-up to Y-up, and the winding must survive it or the city renders inside out.
    #[test]
    fn the_frame_conversion_preserves_winding() {
        assert_eq!(remap([1.0, 0.0, 0.0]), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(remap([0.0, 1.0, 0.0]), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(remap([0.0, 0.0, 1.0]), Vec3::new(0.0, 1.0, 0.0));
        // det = +1: the mapped basis is right-handed, so triangle order is unchanged.
        let (x, y, z) = (remap([1.0, 0.0, 0.0]), remap([0.0, 1.0, 0.0]), remap([0.0, 0.0, 1.0]));
        assert!((x.cross(y).dot(z) - 1.0).abs() < 1e-6, "handedness flipped");
    }

    fn mesh(hash: u32, verts: usize, bbox_max: f32) -> WorldMesh {
        let mut m = WorldMesh {
            header: header(hash, false),
            positions: vec![[0.0; 3]; verts],
            normals: Vec::new(),
            colours: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
            groups: Vec::new(),
            texture_slots: Vec::new(),
        };
        m.header.bbox_max = [bbox_max; 3];
        m
    }

    /// The names are the ones measured in the install, not a guess at a naming scheme.
    #[test]
    fn the_backdrop_families_are_recognised_and_the_city_is_not() {
        for name in [
            "SKYDOME", "SKYDOME_A", "PAN_SKYLINE", "PAN_OCEAN", "PAN_HILLRIDGE", "PAN_HILLS_C98",
            "PAN_HILLSWEST", "TRN_PANARAMAB_01", "TRN_PANARAMA_AIRPORT",
        ] {
            assert!(is_backdrop(name), "{name} is backdrop");
        }
        for name in [
            "TRN_ROADA_CHOP_A2_R14", "TRN_TERRAINA_01", "XB_CC_SPANISHCONDOB", "OBJ_PYLON",
            "RDP_PARKING_NL_AA_KT", "TRN_GRASSC",
        ] {
            assert!(!is_backdrop(name), "{name} is city");
        }
    }

    #[test]
    fn dedup_drops_a_repeat_and_keeps_an_instance_placed_elsewhere() {
        let objects = vec![
            mesh(7, 100, 1.0),
            mesh(7, 100, 1.0), // byte-for-byte the same object again
            mesh(7, 100, 2.0), // same key, different extent — a real second instance
            mesh(8, 100, 1.0),
        ];
        let kept = dedup(objects);
        assert_eq!(kept.len(), 3, "only the exact repeat goes");
    }
}
