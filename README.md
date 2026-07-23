# nfsu2-gizmo

Recreating **Need for Speed: Underground 2 (2004)** inside the [Gizmo engine](https://github.com/bdrtr/Gizmo),
by reading the original game's own asset files. Delivered in phases, easy → hard.

> ⚠️ **No copyrighted data ships in this repo.** All parsing runs against *your own*
> installed copy of the game (`$NFSU2_ROOT`). Tests are synthetic or env-var gated.

## Layout

```
crates/gizmo-nfs/   Pure, engine-agnostic NFSU2 asset parser (no wgpu/glam/gizmo deps).
                    BIGF/VIV archives, RefPack/QFS + JDLZ compression, chunked containers,
                    GEOMETRY.BIN → indexed CPU meshes. Publishable standalone.
game/               The playable layer — turns parsed CPU data into Gizmo meshes/materials
                    and drives it with the engine's raycast VehicleController.
  src/bin/nfs_viewer.rs   M1 — in-engine car viewer
  src/bin/nfs_drive.rs    M2 — drivable car (VehicleController, visual wheels, per-part colours)
  src/bin/nfs_race.rs     M3 — oval track + checkpoints + lap timing
```

## The engine dependency (important)

`game/` depends on the Gizmo engine via a **local path dependency** to a sibling checkout:

```toml
gizmo = { package = "gizmo-engine", path = "../../Gizmo-engine/crates/gizmo", ... }
```

So this repo expects **`Gizmo-engine/` checked out next to `nfsu2-gizmo/`**:

```
code/
├─ Gizmo-engine/     ← the engine (github.com/bdrtr/Gizmo)
└─ nfsu2-gizmo/      ← this repo
```

The game currently relies on unreleased engine features (`Collider::trimesh`, and later a
box-vs-trimesh narrowphase fix), so the crates.io `gizmo-engine 0.8.0` is not enough yet.
When the engine stabilises, switch to a git-pinned or crates.io dependency.

## Build & run

```bash
# Parser only (fast, pure — env-gated golden tests need $NFSU2_ROOT):
cargo test -p gizmo-nfs

# The game — ALWAYS --release (physics debug build is CPU-bound):
export NFSU2_ROOT="/path/to/Need for Speed Underground 2"
cargo run --release -p nfsu2 --bin nfs_drive  -- "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
cargo run --release -p nfsu2 --bin nfs_viewer -- "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
cargo run --release -p nfsu2 --bin nfs_race
```

## Status

Complete NFSU2 → Gizmo **geometry** pipeline (M1–M3 done). The **TPK texture format** is now
largely decoded: the pixel pool is **JDLZ-compressed RGBA8** (not DXT) laid out in 512-wide
pages, and the 24-byte descriptors (hash + pool offset → x,y + format) are parsed by
`gizmo-nfs::texture`. Remaining before textured cars: per-texture **width/height** (not in any
identified descriptor field yet) and wiring textures onto the car's materials. See the
parser's `README.md` for format details.
