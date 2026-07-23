//! NFSU2 → Gizmo integration layer.
//!
//! The pure asset parsing lives in `gizmo-nfs`; this crate owns the *presentation policy*
//! that turns parsed CPU data into a drivable, material-grouped car in the engine. The demo
//! binaries (`nfs_viewer`, `nfs_drive`, `nfs_race`) are thin orchestrators over these three
//! modules:
//!
//! * [`part_groups`] — pure, engine-free classification: which material group a part is,
//!   and which parts form the default showroom car.
//! * [`mesh`] — engine-coupled geometry: the NFSU2 → Gizmo coordinate remap, bounds, and
//!   building GPU meshes from parts.
//! * [`car`] — the car as a whole: the per-group material looks and wheel placement.

pub mod car;
pub mod mesh;
pub mod part_groups;
