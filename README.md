# nfsu2-gizmo

Recreating **Need for Speed: Underground 2 (2004)** inside the [Gizmo engine](https://github.com/bdrtr/Gizmo),
by reading the original game's own asset files. Delivered in phases, easy → hard.

> ⚠️ **No copyrighted data ships in this repo.** All parsing runs against *your own*
> installed copy of the game (`$NFSU2_ROOT`). Tests are synthetic or env-var gated.

## Layout

```
game/               The playable layer — turns parsed CPU data into Gizmo meshes/materials
                    and drives it with the engine's raycast VehicleController.
  src/bin/nfs_viewer.rs   M1 — in-engine car viewer
  src/bin/nfs_drive.rs    M2 — drivable car (VehicleController, visual wheels, per-part colours)
  src/bin/nfs_race.rs     M3 — oval track + checkpoints + lap timing
```

## Where the parser lives

Reading NFSU2's files is a separate project: **[PryHUB](https://github.com/bdrtr/PryHUB)** — the
`gizmo-nfs` parser crate, its `ug2` CLI, and a native inspector for the formats. This repo takes
`gizmo-nfs` as a dependency and turns its output into meshes, materials and physics.

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

Complete NFSU2 → Gizmo **geometry and texture** pipeline. The paragraph that used to stand here
described the TPK pool as "JDLZ-compressed RGBA8 in 512-wide pages" with per-texture width/height
"not in any identified descriptor field yet", and listed wiring textures onto materials as the work
remaining. All three were wrong or done: each texture is its **own** JDLZ- or HUFF-compressed blob,
its size comes from an `OldTextureInfo` header embedded near the blob's tail, the pixels are
DXT1/3/5, uncompressed BGRA **or palettised**, and the car has been textured for many commits. See
the parser's `README.md`; **54,873 of an install's 54,885 textures decode.**

The car now also drives off its **own record**. `GLOBALB.BUN` is read once per car and answers three
things it used to guess: the wheel mounts and radius, the game's 123-colour paint palette (NFSU2
does not texture a body — it paints it, and that bundle is the only place the colours are written
down), and `CarHandling` — rpm limits, a nine-point torque curve, four gearboxes and the drivetrain
split. Five cars, five different cars:

| car | mass | gears | final drive | peak torque | drive | red line |
|---|---|---|---|---|---|---|
| PEUGOT | 1055 kg | 5 | 3.790 | 192 N·m | front | 6500 |
| CIVIC | 1180 kg | 5 | 4.400 | 151 N·m | front | 8000 |
| 240SX | 1220 kg | 5 | 4.083 | 216 N·m | rear | 6500 |
| SUPRA | 1550 kg | 6 | 3.270 | 285 N·m | rear | 7000 |
| G35 | 1550 kg | 6 | 3.540 | 365 N·m | rear | 7000 |

Braking, aerodynamics, anti-roll and the steering lock stay at the engine's own defaults and are
marked as invented in `car/tune.rs`, because they are **not in this game's files** — measured, not
assumed: the one brake-shaped triple is exactly zero for all 15 traffic vehicles, and the only
steering angles are a global ±43° identical in all 46 records.

`NFS_PAINT=<n>` picks a palette colour; `NFS_COLOR="r,g,b"` still forces a raw one.
