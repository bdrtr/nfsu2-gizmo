//! NFSU2 → Gizmo integration layer.
//!
//! The pure asset parsing lives in `gizmo-nfs`; this crate owns the *presentation policy*
//! that turns parsed CPU data into a drivable, material-grouped car in the engine. The demo
//! binaries (`nfs_viewer`, `nfs_drive`, `nfs_race`, `nfs_shot`) are thin orchestrators over
//! these modules, which are layered by what they are allowed to depend on:
//!
//! * [`parts`] — **pure, engine-free** classification: which material group a part is, what
//!   its name says about the variant it belongs to, and which parts form a given
//!   configuration of the car. No `gizmo`/`wgpu` types — keep it that way.
//! * [`geom`] — engine-coupled geometry: the NFSU2 → Gizmo coordinate remap, what a part's
//!   file matrix means, and building GPU meshes.
//! * [`car`] — the car as a whole: material looks, shader routing, texture resolution, wheel
//!   fitting, and the assembly that ties them together.
//! * [`assets`] — the filesystem/environment edge (sibling asset files, env overrides).
//! * [`scene`] — the engine-side scaffolding the demo binaries share: spawning an assembled
//!   car into a world, and the lights to see it by.

pub mod assets;
pub mod car;
pub mod geom;
pub mod parts;
pub mod scene;
