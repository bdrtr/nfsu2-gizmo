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
