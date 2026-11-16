//! The city's breakable props, as things a car can knock over.
//!
//! `gizmo_nfs::world::smokeable` reads what the game says about them — a kind with a mass for each,
//! and where each stands. This turns that into bodies: the prop's own solid in its own frame, a
//! position and a rotation in Gizmo's, a box to collide with and a mass. The scenery copies the
//! streaming sections place at the same spots are dropped by the loader, so a prop is drawn once,
//! by its body.

use super::remap;
use gizmo::prelude::*;
use gizmo_nfs::types::IDENTITY;
use gizmo_nfs::world::WorldMesh;

/// One prop, ready to become a body.
#[derive(Clone)]
pub struct Prop {
    /// The kind's name (`ZCS_Barrel_A`).
    pub name: String,
    /// The solid, in its own frame (identity matrix): its vertices go through [`super::remap`]
    /// only, and the body's transform puts them in the world.
    pub mesh: WorldMesh,
    /// Where the body stands, Gizmo frame.
    pub position: Vec3,
    /// Its rotation, Gizmo frame.
    pub rotation: Quat,
    /// Kilograms (the file states tonnes).
    pub mass: f32,
    /// The collision box: centre in the body's frame, and half extents.
    pub centre: Vec3,
    pub half: Vec3,
}

/// The rotation a file matrix applies, in Gizmo's frame — `None` when it mirrors or scales, which
/// a rigid body cannot carry.
fn rotation_of(m: &gizmo_nfs::types::Mat4) -> Option<Quat> {
    // A Gizmo-frame direction `e` is the file-frame `p` with `remap(p) = e`; the matrix carries `p`
    // to `p·R`, and that goes back through `remap`.
    let unmap = |g: Vec3| [-g.z, -g.x, g.y];
    let image = |g: Vec3| {
        let p = unmap(g);
        let w = [
            p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0],
            p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1],
            p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2],
        ];
        remap(w)
    };
    let cols = [image(Vec3::X), image(Vec3::Y), image(Vec3::Z)];
    if cols.iter().any(|c| (c.length() - 1.0).abs() > 0.05) {
        return None;
    }
    let basis = gizmo::math::Mat3::from_cols(cols[0].normalize(), cols[1].normalize(), cols[2].normalize());
    (basis.determinant() > 0.0).then(|| Quat::from_mat3(&basis).normalize())
}

/// The props of one stream bundle: its placements, with kinds from the region file beside it and
/// solids from `meshes` (the bundle's own, before [`super::instance`] takes the instanced models
/// out). Kinds with no mass — the fixed ones — and placements that mirror or scale are skipped:
/// they stay scenery.
#[must_use]
pub fn props(bundle: &[u8], region: &[u8], meshes: &[WorldMesh]) -> Vec<Prop> {
    let (Ok(kinds), Ok(placed)) = (
        gizmo_nfs::world::smokeable::kinds(region),
        gizmo_nfs::world::smokeable::placements(bundle),
    ) else {
        return Vec::new();
    };
    let solid: std::collections::HashMap<u32, &WorldMesh> =
        meshes.iter().map(|m| (m.header.hash.0, m)).collect();
    placed
        .iter()
        .filter_map(|p| {
            let kind = kinds.iter().find(|k| k.hash == p.kind)?;
            if kind.mass <= 0.0 {
                return None;
            }
            let model = solid.get(&p.model.0)?;
            let rotation = rotation_of(&p.matrix)?;
            let mut mesh = (*model).clone();
            mesh.header.matrix = IDENTITY;
            let (lo, hi) = (remap(mesh.header.bbox_min), remap(mesh.header.bbox_max));
            let (lo, hi) = (lo.min(hi), lo.max(hi));
            Some(Prop {
                name: kind.name.clone(),
                position: remap([p.matrix[3][0], p.matrix[3][1], p.matrix[3][2]]),
                rotation,
                mass: kind.mass * 1000.0,
                centre: (lo + hi) * 0.5,
                half: ((hi - lo) * 0.5).max(Vec3::splat(0.05)),
                mesh,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file rotation of 90° about its up axis (Z) is a rotation about Gizmo's up (Y), and the
    /// identity stays the identity.
    #[test]
    fn a_file_yaw_is_a_gizmo_yaw() {
        assert!(rotation_of(&IDENTITY).unwrap().angle_between(Quat::IDENTITY) < 1e-4);
        let yaw = [[0.0, 1.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        let q = rotation_of(&yaw).unwrap();
        let up = q * Vec3::Y;
        assert!((up - Vec3::Y).length() < 1e-4, "{up:?}");
        // And it moves a file vertex the way the matrix does.
        let p = [1.0, 0.0, 0.0];
        let file = [p[0] * yaw[0][0] + p[1] * yaw[1][0], p[0] * yaw[0][1] + p[1] * yaw[1][1], 0.0];
        assert!((q * remap(p) - remap(file)).length() < 1e-4);
    }

    /// A mirror or a scale is not a rotation.
    #[test]
    fn mirrors_and_scales_are_refused() {
        let mut m = IDENTITY;
        m[0][0] = -1.0;
        assert!(rotation_of(&m).is_none());
        m[0][0] = 2.0;
        assert!(rotation_of(&m).is_none());
    }
}
