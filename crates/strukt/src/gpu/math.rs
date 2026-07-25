//! The small amount of 4×4 maths the preview needs.
//!
//! Hand-rolled rather than pulled from a crate: this is a viewer whose only other dependency is
//! the parser, and a perspective matrix and a look-at are sixty lines. Column-major, the layout
//! WGSL expects.

/// A 4×4 matrix in column-major order — `m[col][row]`, which is what a `mat4x4<f32>` binding reads.
pub type M4 = [[f32; 4]; 4];

/// The identity.
#[must_use]
pub fn identity() -> M4 {
    let mut m = [[0.0; 4]; 4];
    for (i, col) in m.iter_mut().enumerate() {
        col[i] = 1.0;
    }
    m
}

/// `a * b`, applied right-to-left as usual (`proj * view` transforms by the view first).
#[must_use]
pub fn mul(a: M4, b: M4) -> M4 {
    let mut out = [[0.0; 4]; 4];
    for (c, col) in out.iter_mut().enumerate() {
        for (r, cell) in col.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

/// A right-handed perspective projection with the 0..1 depth range wgpu uses.
#[must_use]
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> M4 {
    let f = 1.0 / (fov_y * 0.5).tan();
    let mut m = [[0.0; 4]; 4];
    m[0][0] = f / aspect.max(1e-6);
    m[1][1] = f;
    m[2][2] = far / (near - far);
    m[2][3] = -1.0;
    m[3][2] = near * far / (near - far);
    m
}

/// A right-handed view matrix looking from `eye` at `target`, with `up` as the world's up axis.
#[must_use]
pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> M4 {
    let f = normalize(sub(target, eye));
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    let mut m = identity();
    m[0] = [s[0], u[0], -f[0], 0.0];
    m[1] = [s[1], u[1], -f[1], 0.0];
    m[2] = [s[2], u[2], -f[2], 0.0];
    m[3] = [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0];
    m
}

/// Where an orbit camera sits, given its angles and distance around `target`.
///
/// NFSU2 is **Z-up** and the exports keep that frame, so the orbit is around Z: pitch raises the
/// eye along Z rather than Y, and "up" is Z.
#[must_use]
pub fn orbit_eye(target: [f32; 3], yaw: f32, pitch: f32, distance: f32) -> [f32; 3] {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    [
        target[0] + distance * cp * cy,
        target[1] + distance * cp * sy,
        target[2] + distance * sp,
    ]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt();
    if len < 1e-9 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: M4, v: [f32; 4]) -> [f32; 4] {
        let mut out = [0.0; 4];
        for (r, cell) in out.iter_mut().enumerate() {
            *cell = (0..4).map(|c| m[c][r] * v[c]).sum();
        }
        out
    }

    #[test]
    fn identity_is_neutral_on_both_sides() {
        let m = perspective(1.0, 1.5, 0.1, 100.0);
        assert_eq!(mul(m, identity()), m);
        assert_eq!(mul(identity(), m), m);
    }

    #[test]
    fn a_point_in_front_of_the_camera_lands_inside_the_clip_cube() {
        // The eye is 5 m out along +X looking back at the origin; the origin must project to the
        // centre of the image with a depth inside 0..1. If the handedness or the depth range were
        // wrong, the model would be behind the camera or clipped away — the classic silent failure.
        let view = look_at([5.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let clip = mul(perspective(0.9, 1.0, 0.1, 100.0), view);
        let p = apply(clip, [0.0, 0.0, 0.0, 1.0]);
        assert!(p[3] > 0.0, "in front of the camera");
        let ndc = [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
        assert!(ndc[0].abs() < 1e-5 && ndc[1].abs() < 1e-5, "centred: {ndc:?}");
        assert!((0.0..=1.0).contains(&ndc[2]), "depth in wgpu's 0..1 range: {}", ndc[2]);
    }

    #[test]
    fn the_orbit_is_around_the_z_axis_because_the_format_is_z_up() {
        let eye = orbit_eye([0.0; 3], 0.0, 0.0, 4.0);
        assert!((eye[0] - 4.0).abs() < 1e-5 && eye[2].abs() < 1e-5);
        // Pitching up must raise the eye along Z, not Y.
        let up = orbit_eye([0.0; 3], 0.0, std::f32::consts::FRAC_PI_2, 4.0);
        assert!((up[2] - 4.0).abs() < 1e-4, "{up:?}");
    }
}
