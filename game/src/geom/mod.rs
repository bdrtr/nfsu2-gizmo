//! Turning parsed CPU parts into Gizmo GPU meshes and placing them in the world.
//!
//! This is the engine-coupled counterpart to [`crate::parts`]: it depends on `gizmo` and
//! `wgpu`, expands the parser's *indexed* geometry into the flat vertex list the renderer
//! wants, and applies the NFSU2 → Gizmo coordinate remap.
//!
//! Split three ways: [`frame`] is the coordinate system, [`place`] decides what a part's file
//! matrix means, and [`build`] does the GPU work.

mod build;
mod frame;
mod place;

pub use build::{add_transform, build_box, build_mesh, build_mesh_inflated, build_mesh_items};
pub use frame::{bbox, remap};
