# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Recreating **Need for Speed: Underground 2 (2004)** inside the [Gizmo engine](https://github.com/bdrtr/Gizmo)
by parsing the original game's own asset files. This repository is the **game**:

- **`game/`** (package `nfsu2`) — the playable integration layer. Turns parsed CPU data into Gizmo meshes/materials and drives it with the engine's raycast `VehicleController`. The reusable logic lives in a **library** (`game/src/`), with the binaries (`game/src/bin/*.rs`) as thin orchestrators over it:
  - `parts` — **pure, engine-free** part policy. `group` (`group_of`: name → material group), `name` (`component_key`, the `KIT##`/`KITW##`/`STYLE##` namespace and the slot a part fills), `config` (`CarConfig`), `select` (`select_car`: assemble a car in a given config at the highest available LOD; `select_stock_car` is the all-stock wrapper). Unit-tested; no `gizmo`/`wgpu` types, so **keep it that way**.
  - `geom` — engine-coupled geometry: `frame` (the NFSU2 → Gizmo `remap` and `bbox`), `place` (what a part's file matrix means — placement vs. baked pose), `build` (indexed parts → GPU `Mesh`, `add_transform`).
  - `car` — the car as a whole: `look` (per-group `PbrLook`), `shader` (routing a material run by its shader hash), `skin` (texture matching + the doorline overlay), `wheel` (`fit_wheel`, `wheel_mount`), and `build_car_visuals` tying them together.
  - `assets` — the I/O edge: `load_tpk_beside`, `load_cartypeinfo_beside`, `env_color`. Everything else is a pure function of bytes already in memory.
  - `scene` — the engine-side scaffolding the binaries share: `spawn_body` (one entity per group/textured mesh, compositing the doorline overlay), `wheel_mounts`/`wheel_mirror`/`wheel_material`, `add_lights`, `car_path`.

### The parser lives elsewhere

Reading the files is not this repo's job. `gizmo-nfs` — the pure, engine-agnostic NFSU2 parser,
its `ug2` CLI and the PryHUB inspector built on it — is its own project:
**[PryHUB](https://github.com/bdrtr/PryHUB)**, checked out as a sibling.

```
code/
├─ Gizmo/            ← the engine (github.com/bdrtr/Gizmo) — read it freely, but the build no
│                      longer uses it: the engine is pinned by commit (see below)
├─ PryHUB/           ← the parser + asset toolkit (github.com/bdrtr/PryHUB) — required sibling
└─ nfsu2-gizmo/      ← this repo (the game)
```

`game/Cargo.toml` takes `gizmo-nfs` as a git dependency, and the workspace `[patch]` redirects it
to `../PryHUB/crates/gizmo-nfs` so a parser change reaches the game without a push. Consequences
worth keeping in mind:

- **Format work belongs there, not here.** A new chunk, a decoded field, a validation rule: it goes
  in `gizmo-nfs`, gets a test there, and arrives here as data. This repo turns that data into
  meshes, materials and physics — nothing more.
- When something renders wrong, the first question is *which side*. `ug2 dump` / `ug2 probe` /
  `ug2 textures` (in PryHUB) say what the file contains; if the file is right, the bug is here.
- The two repos are versioned independently. A parser change that breaks this repo's build shows up
  as a compile error after a `cargo update -p gizmo-nfs`, not as a silent behaviour change.

## Critical setup: the engine dependency is PINNED

`game/` builds against a **frozen engine commit**, not the sibling checkout:

```toml
gizmo = { package = "gizmo-engine", git = "https://github.com/bdrtr/Gizmo",
          rev = "48ac99e8bc4f27f9943e1d282a112e13cc1efaf0", ... }
```

The game is being developed against a fixed engine version while Gizmo evolves independently.
crates.io `gizmo-engine 0.8.0` is still insufficient (`Collider::trimesh`, cached trimesh AABB,
`VehicleTuning::torque_curve`); the pinned commit has all of it. Rules that follow from this:

- **Do not switch the dependency back to a path just to pick up an engine change.** If the game
  needs something newer, that is a decision to *move the pin* — follow the checklist in
  `MOTOR-NOTLARI.md`, don't drift.
- **An engine gap found while working belongs in `MOTOR-NOTLARI.md`**, in engine-general language,
  together with whatever workaround the game took. That file is the queue replayed when the pin
  moves; a workaround nobody wrote down is a workaround nobody removes.
- **This file is not the authority on the pin** — `game/Cargo.toml` is, with `MOTOR-NOTLARI.md`'s
  Pin table alongside it. The commit above is a copy and copies go stale: it sat two pins behind
  for two days and was read as current once. Check the manifest.
- The root `Cargo.toml` has a commented-out `[patch."https://github.com/bdrtr/Gizmo"]` block for
  temporarily building against the local tree. It overrides the `rev`. Comment it back when done —
  `cargo tree -p gizmo-engine` says which source is live.
- **Never `git checkout` in `../Gizmo`** (or `../PryHUB`): other sessions work in those trees. To
  find out whether the engine has a commit, ask git — `git -C ../Gizmo log --oneline 4d1a8cb..main`.

The parser is *not* pinned: `gizmo-nfs` stays patched to the sibling `../PryHUB` checkout, because
the parser and the game still move together.

## Build & run

```bash
# ALWAYS use --release (debug physics is CPU-bound and unusable):
export NFSU2_ROOT="/path/to/Need for Speed Underground 2"
cargo run --release -p nfsu2 --bin nfs_drive  -- "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
cargo run --release -p nfsu2 --bin nfs_viewer -- "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
cargo run --release -p nfsu2 --bin nfs_race

# The game's own tests (pure part-selection policy and geometry maths):
cargo test -p nfsu2
```

Binaries: `nfs_viewer` (M1, in-engine car viewer), `nfs_drive` (M2, drivable car, default binary),
`nfs_race` (M3, oval track + lap timing).

All of them read the car's configuration from the environment (`0`/absent = stock, an
unavailable part number silently falls back to stock):
`NFS_KIT` (body kit `KIT##`: front + rear bumper + skirt), `NFS_STYLE_HOOD` (`STYLE##` hood),
`NFS_STYLE_LIGHT` (`STYLE##` head/tail lights), `NFS_WIDE` (widebody `KITW##`: body + doors).
Use `ug2 info <car>` (PryHUB) to see which numbers a given car actually ships.

### RAM-limited builds

`.cargo/config.toml` caps `jobs = 4` and disables LTO because the dev machine has 13 GB RAM (each `rustc` uses ~1–2 GB). Do not remove this unless building on a higher-memory machine.

## Asset hygiene (important)

**No copyrighted game data ships in this repo.** `.gitignore` blocks `*.BIN`/`*.VIV`/`*.bun`/`*.lzc`
and `game-data/`. Tests are synthetic or read a real install only when `NFSU2_ROOT` is set. Never
commit real assets or hardcode a working install path into committed code (the `DEFAULT_CAR` const
in `nfs_drive.rs` is a local convenience default).

## What the parser hands over (`gizmo_nfs::types`)

Pure-data structs, no `glam`/`wgpu`. Geometry is **indexed** and transforms are stored
**as-in-file** (row-major, original handedness). Expanding indices to a flat vertex list and any
coordinate-system fixup are deliberately **this** layer's job: `geom::remap()` converts NFSU2's
Z-up frame to Gizmo's, and `geom::place` decides what a solid's matrix means via
`gizmo_nfs::placement`. If a fixup starts to look like format knowledge, it belongs in PryHUB.

## Conventions

- Code comments and Cargo.toml notes are frequently in **Turkish** — match the surrounding language when editing a file.
- **Part names are truncated** to a fixed-length field in `GEOMETRY.BIN`. Long names lose their tail: `..._HEADLIGHT_LEFT_LOD_A` arrives as `..._HEADLIGHT_LEFT_` (LOD letter gone, so two LODs share a name — disambiguate by triangle count) and `..._SIDE_MIRROR` as `..._MIRRO`/`..._MIRR`. Match shortened stems (`MIRR`), never assume the full word survives. A truncated name can be *recovered* rather than guessed: PryHUB's `gizmo_nfs::hash` hashes a candidate and the file's own hash says whether it is right. `NfsTexture::name_is_whole()` asks whether the stored name survived the cut, and `NfsTexture::is_mask()` answers "is this another texture's `_MASK` companion" **through** the truncation — one install hides 56 such masks across 29 cars, all fully opaque, and binding one as a diffuse map is a black panel. The game's skin matching (`car::skin`) filters on that proof rather than on how transparent an image happens to be. Cars also carry `STYLE00..STYLE14` purchasable part variants and `KIT01+` body kits alongside the default `BASE`/`KIT00`; render only the default set or variants overlap.
- Status is milestone-phased (M1–M3 done). The TPK texture format is decoded (per-texture DXT1/3/5 + RGBA); what is left there is **HUFF-compressed** textures, which are listed but not decoded. See PryHUB's `crates/gizmo-nfs/README.md` for the per-format status table.
