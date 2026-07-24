//! Turning parsed CPU parts into Gizmo GPU meshes and placing them in the world.
//!
//! This is the engine-coupled counterpart to [`gizmo_nfs::parts`]: it depends on `gizmo` and
//! `wgpu`, expands the parser's *indexed* geometry into the flat vertex list the renderer
//! wants, and applies the NFSU2 → Gizmo coordinate remap.
//!
//! The decision of *what a part's file matrix means* is not here — that is format semantics
//! shared with the CLI exporter, and lives in [`gizmo_nfs::placement`]. What is here is the
//! engine's own frame ([`frame`]) and the GPU work ([`build`]).

mod build;
mod frame;

pub use build::{add_transform, build_box, build_mesh, build_mesh_inflated, build_mesh_items};
pub use frame::{bbox, remap};
