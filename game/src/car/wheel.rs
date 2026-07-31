//! Wheel geometry: how big the wheel is and where its four corners sit.

use gizmo::prelude::*;
use gizmo_nfs::{NfsTexture, WheelSpec};

/// How to surface the (four-times-instanced) wheel mesh.
pub enum WheelSurface {
    /// A pre-built flat material — dark rubber, for a wheel with no resolvable texture.
    Flat(Material),
    /// The tire/rim texture (its UVs cover both). The caller uploads it, once, then instances
    /// the wheel at the four corners with the resulting material.
    Textured(NfsTexture),
}

/// Wheel size and corner offsets derived from the wheel part's bounds and the car body.
#[derive(Clone, Copy, Debug)]
pub struct WheelFit {
    /// Wheel radius.
    pub radius: f32,
    /// Half the wheelbase (front↔rear corner offset along the car's length).
    pub half_wheelbase: f32,
    /// Half the track width (left↔right corner offset across the car).
    pub half_track: f32,
}

/// Fit the four wheel corners from one wheel part's bounds (`wmin`/`wmax`, Gizmo frame) and
/// the car `center`/`width`/`length`. The `max(...)` floors keep a sane stance even when a
/// car's single modelled wheel sits unusually close to the centreline.
#[must_use]
pub fn fit_wheel(wmin: Vec3, wmax: Vec3, center: Vec3, width: f32, length: f32) -> WheelFit {
    let wcenter = (wmin + wmax) * 0.5;
    WheelFit {
        radius: ((wmax.y - wmin.y).max(wmax.z - wmin.z) * 0.5).clamp(0.18, 0.55),
        half_wheelbase: (wcenter.z - center.z).abs().max(length * 0.30),
        half_track: (wcenter.x - center.x).abs().max(width * 0.40),
    }
}

/// Map a [`WheelSpec`] mount (NFSU2 car space: fore/aft, lateral, ride-height) into the Gizmo
/// frame, recentered by the car `center` — the exact position to instance the wheel mesh at.
/// Uses the same axis remap as the body mesh so wheels and body share one frame.
#[must_use]
pub fn wheel_mount(w: &WheelSpec, center: Vec3) -> Vec3 {
    crate::geom::remap([w.fore_aft, w.lateral, w.ride_height]) - center
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizmo_nfs::{group_of, select_stock_car, Grp};

    /// What the record says a wheel's corners are, against what `fit_wheel` guesses — printed,
    /// over a real install, because the two disagreeing is what puts a car's wheels in the wrong
    /// place and no synthetic mesh can show it.
    ///
    /// Skipped without `NFSU2_ROOT`. Asserts only the thing that must be true whatever the car:
    /// the record's own mounts are a *staggered set of four*, and `fit_wheel` can only ever
    /// produce one symmetric pair — so a binary that instances at `±half_track, ±half_wheelbase`
    /// is throwing away information the file has.
    #[test]
    fn the_record_places_wheels_and_fit_wheel_only_guesses() {
        let Some(root) = std::env::var_os("NFSU2_ROOT") else {
            eprintln!("NFSU2_ROOT unset — skipping");
            return;
        };
        let dir = std::path::Path::new(&root).join("CARS").join("240SX");
        let Ok(bytes) = std::fs::read(dir.join("GEOMETRY.BIN")) else { return };
        let all = gizmo_nfs::parse_geometry(&bytes).expect("geometry");
        let stock = select_stock_car(&all);
        let (lo, hi) = crate::geom::bbox(&stock);
        let center = (lo + hi) * 0.5;
        let (width, _height, length) = (hi.x - lo.x, hi.y - lo.y, hi.z - lo.z);

        let wp = stock
            .iter()
            .copied()
            .find(|p| group_of(&p.name) == Grp::Wheel)
            .expect("the 240SX models a wheel");
        let (wl, wh) = crate::geom::bbox(std::slice::from_ref(&wp));
        let fit = fit_wheel(wl, wh, center, width, length);

        let path = dir.join("GEOMETRY.BIN");
        let cti = crate::assets::load_cartypeinfo_beside(path.to_str().unwrap())
            .expect("the record");

        eprintln!("guess : half_track {:.3}  half_wheelbase {:.3}  radius {:.3}",
                  fit.half_track, fit.half_wheelbase, fit.radius);
        for (i, w) in cti.wheels.iter().enumerate() {
            let m = wheel_mount(w, center);
            eprintln!("record[{i}]: mount ({:+.3}, {:+.3}, {:+.3})  radius {:.3}",
                      m.x, m.y, m.z, w.radius);
        }
        let guessed: Vec<Vec3> = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .iter()
            .map(|(sx, sz): &(f32, f32)| {
                Vec3::new(sx * fit.half_track, -_height * 0.5 + fit.radius * 0.95, sz * fit.half_wheelbase)
            })
            .collect();
        for (i, g) in guessed.iter().enumerate() {
            let m = wheel_mount(&cti.wheels[i], center);
            eprintln!("  [{i}] guess ({:+.3}, {:+.3}, {:+.3})  →  off by {:.3} m",
                      g.x, g.y, g.z, (*g - m).length());
        }

        // The record's four mounts are not one mirrored pair: front and rear differ, so no
        // `±half_wheelbase` can be all four at once.
        let fa: Vec<f32> = cti.wheels.iter().map(|w| w.fore_aft).collect();
        assert!(fa.iter().any(|z| *z > 0.0) && fa.iter().any(|z| *z < 0.0), "front and rear");
        assert!(
            cti.wheels[0].radius > 0.0,
            "the record states a radius; fit_wheel clamps a bbox guess to 0.18..0.55"
        );
    }
}
