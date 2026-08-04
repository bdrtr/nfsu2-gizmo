//! Cutting the city into cells.
//!
//! One mesh for the whole city has the whole city's bounding box, so nothing can ever cull it and
//! every triangle is submitted every frame. Cells are what give the renderer something to reject.
//!
//! 256 m is chosen for *batching*, and deliberately not shared with anything else. It is large
//! enough that a cell's meshes merge into a handful of draws and small enough that a cell is a
//! meaningful unit of visibility at street level, where the view is blocked at every corner. A
//! ground query wants a much smaller cell — tens of metres — and giving both jobs one grid makes
//! each of them worse; that one belongs with the physics work, not here.

use gizmo::prelude::*;

/// Cell edge, in metres. Not a physics or streaming figure — see the module note.
pub const CELL_SIZE: f32 = 256.0;

/// Which cell a point falls in, on the ground plane (Gizmo's X/Z).
///
/// Height is deliberately not part of the key. Bayview is stacked — overpasses, multi-level car
/// parks — and splitting a cell by altitude would cut a single road surface into pieces that then
/// merge separately and draw separately, for no visibility gained.
#[must_use]
pub fn cell_of(p: Vec3) -> (i32, i32) {
    (
        (p.x / CELL_SIZE).floor() as i32,
        (p.z / CELL_SIZE).floor() as i32,
    )
}

/// The centre of a cell, which is what its meshes are recentred on.
#[must_use]
pub fn cell_centre((cx, cz): (i32, i32)) -> Vec3 {
    Vec3::new(
        (cx as f32 + 0.5) * CELL_SIZE,
        0.0,
        (cz as f32 + 0.5) * CELL_SIZE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_falls_in_the_cell_that_contains_it() {
        assert_eq!(cell_of(Vec3::new(0.0, 0.0, 0.0)), (0, 0));
        assert_eq!(cell_of(Vec3::new(255.9, 0.0, 255.9)), (0, 0));
        assert_eq!(cell_of(Vec3::new(256.0, 0.0, 0.0)), (1, 0));
    }

    /// Negative coordinates are half the city, so flooring has to be flooring and not truncation —
    /// `as i32` alone would put −1.0 and +1.0 in the same cell and fold the map onto itself.
    #[test]
    fn negative_coordinates_floor_rather_than_truncate() {
        assert_eq!(cell_of(Vec3::new(-1.0, 0.0, -1.0)), (-1, -1));
        assert_eq!(cell_of(Vec3::new(-256.0, 0.0, 0.0)), (-1, 0));
        assert_eq!(cell_of(Vec3::new(-256.1, 0.0, 0.0)), (-2, 0));
        assert_ne!(cell_of(Vec3::new(-1.0, 0.0, 0.0)), cell_of(Vec3::new(1.0, 0.0, 0.0)));
    }

    /// Height is not in the key: an overpass and the road under it merge into one cell on purpose.
    #[test]
    fn altitude_does_not_split_a_cell() {
        assert_eq!(cell_of(Vec3::new(10.0, 0.0, 10.0)), cell_of(Vec3::new(10.0, 40.0, 10.0)));
    }

    #[test]
    fn a_cells_centre_is_inside_it() {
        for c in [(0, 0), (3, -7), (-2, -2)] {
            assert_eq!(cell_of(cell_centre(c)), c);
        }
    }
}
