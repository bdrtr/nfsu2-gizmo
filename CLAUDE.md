# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Recreating **Need for Speed: Underground 2 (2004)** inside the [Gizmo engine](https://github.com/bdrtr/Gizmo) by parsing the original game's own asset files. A Rust workspace with two crates:

- **`crates/gizmo-nfs`** — a pure, engine-agnostic NFSU2 asset parser. Depends on **no** `gizmo-*` crate and no GPU/graphics types (`wgpu`, `glam`). It reads NFSU2 binary containers and returns plain CPU data (`NfsCar`, `NfsMeshPart`, `NfsTexture` — `Vec<f32>`/`Vec<u32>`/`Vec<u8>`). Publishable standalone.
- **`game/`** (package `nfsu2`) — the playable integration layer. Turns parsed CPU data into Gizmo meshes/materials and drives it with the engine's raycast `VehicleController`. The reusable logic lives in a **library** (`game/src/`), with the binaries (`game/src/bin/*.rs`) as thin orchestrators over it:
  - `parts` — **pure, engine-free** part policy. `group` (`group_of`: name → material group), `name` (`component_key`, the `KIT##`/`KITW##`/`STYLE##` namespace and the slot a part fills), `config` (`CarConfig`), `select` (`select_car`: assemble a car in a given config at the highest available LOD; `select_stock_car` is the all-stock wrapper). Unit-tested; no `gizmo`/`wgpu` types, so **keep it that way**.
  - `geom` — engine-coupled geometry: `frame` (the NFSU2 → Gizmo `remap` and `bbox`), `place` (what a part's file matrix means — placement vs. baked pose), `build` (indexed parts → GPU `Mesh`, `add_transform`).
  - `car` — the car as a whole: `look` (per-group `PbrLook`), `shader` (routing a material run by its shader hash), `skin` (texture matching + the doorline overlay), `wheel` (`fit_wheel`, `wheel_mount`), and `build_car_visuals` tying them together.
  - `assets` — the I/O edge: `load_tpk_beside`, `load_cartypeinfo_beside`, `env_color`. Everything else is a pure function of bytes already in memory.
  - `scene` — the engine-side scaffolding the binaries share: `spawn_body` (one entity per group/textured mesh, compositing the doorline overlay), `wheel_mounts`/`wheel_mirror`/`wheel_material`, `add_lights`, `car_path`.

## Critical setup: the engine dependency

`game/` depends on the Gizmo engine via a **local path dependency to a sibling checkout**:

```
code/
├─ Gizmo-engine/     ← the engine (github.com/bdrtr/Gizmo)
└─ nfsu2-gizmo/      ← this repo
```

`Gizmo-engine/` **must be checked out next to this repo**. The game relies on unreleased engine features (`Collider::trimesh`, later a box-vs-trimesh narrowphase fix), so crates.io `gizmo-engine 0.8.0` is insufficient. This is not portable — building `game/` requires the sibling checkout.

## Build & run

```bash
# Parser only — fast, pure, no engine checkout needed:
cargo test -p gizmo-nfs

# A single test / test file:
cargo test -p gizmo-nfs --test golden_assets
cargo test -p gizmo-nfs geometry_bin_parses     # by name substring

# The game binaries — ALWAYS use --release (debug physics is CPU-bound and unusable):
export NFSU2_ROOT="/path/to/Need for Speed Underground 2"
cargo run --release -p nfsu2 --bin nfs_drive  -- "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
cargo run --release -p nfsu2 --bin nfs_viewer -- "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
cargo run --release -p nfsu2 --bin nfs_race
```

Binaries: `nfs_viewer` (M1, in-engine car viewer), `nfs_drive` (M2, drivable car, default binary), `nfs_race` (M3, oval track + lap timing).

All of them read the car's configuration from the environment (`0`/absent = stock, an
unavailable part number silently falls back to stock):
`NFS_KIT` (body kit `KIT##`: front + rear bumper + skirt), `NFS_STYLE_HOOD` (`STYLE##` hood),
`NFS_STYLE_LIGHT` (`STYLE##` head/tail lights), `NFS_WIDE` (widebody `KITW##`: body + doors).
Use `nfs info <car>` to see which numbers a given car actually ships.

### The `nfs` CLI

One tool over the whole parser — inspect a car, or export it. Read-only, ships no game data:

```bash
N="cargo run -p gizmo-nfs --features tools --bin nfs --"
$N info   "$NFSU2_ROOT/CARS/240SX"                 # parts, variants, dimensions, GLOBALB record
$N parts  "$NFSU2_ROOT/CARS/240SX" --selected --kit 3
$N export "$NFSU2_ROOT/CARS/240SX" -o out/ --kit 3 --wide 1   # OBJ + MTL + PNG
$N dump   "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"    # chunk tree / VIV listing
$N probe  "$NFSU2_ROOT/CARS/SENTRA" --matrices     # raw solids: counts, buffers, matrices
$N textures "$NFSU2_ROOT/CARS/240SX"
$N globalb  "$NFSU2_ROOT/CARS/240SX"
```

`dump` and `probe` are the workhorses for locking an unconfirmed format (they replaced the
old `nfs_dump`/`nfs_vfmt`/`nfs_survey` examples). `export` writes NFSU2's own coordinates
(x = length, y = width, z = height, Z-up — Blender reads it natively) with each solid's
placement applied.

### RAM-limited builds

`.cargo/config.toml` caps `jobs = 4` and disables LTO because the dev machine has 13 GB RAM (each `rustc` uses ~1–2 GB). Do not remove this unless building on a higher-memory machine.

## Asset hygiene (important)

**No copyrighted game data ships in this repo.** `.gitignore` blocks `*.BIN`/`*.VIV`/`*.bun`/`*.lzc` and `game-data/`. All unit tests use synthetic byte buffers. Golden tests (`tests/golden_assets.rs`) read a real install and are **skipped unless `NFSU2_ROOT` is set**, so CI and other machines stay asset-free. Never commit real assets or hardcode a working install path into committed code (the `DEFAULT_CAR` const in `nfs_drive.rs` is a local convenience default).

## Parser architecture (`crates/gizmo-nfs`)

Layered bottom-up; each layer is `&[u8]`-based and independently testable:

1. **`reader`** — `ByteReader`, a bounds-checked byte cursor. The panic-free foundation; every read returns `NfsResult`.
2. **`fourcc`** — printable rendering of 32-bit chunk IDs.
3. **`chunk`** — the universal NFSU2 chunk tree. Almost every asset is a stream of 8-byte-headed sections (`BinSectionHeader { id, size }`, both LE; `size` = bytes *after* the header). Classification by `id`: **high bit set → container** (recurse), **high bit clear → leaf** (payload), **`id == 0` → padding** (skip). Two consumption styles share one core: zero-alloc `walk()` visitor, and a materialized `ChunkNode` tree (`parse`/`find`/`find_all`) whose leaves borrow from the root buffer.
4. **`compression`** — `detect()` picks the codec **by magic bytes, never by extension** (a `.LZC` may be either). RefPack/QFS (magic `10 FB`) and JDLZ (magic `"JDLZ"`).
5. **`viv`** — BIGF/VIV archive extraction.
6. **`geometry`** — `parse_geometry()`: `GEOMETRY.BIN` → `Vec<NfsMeshPart>`. Solids without a mesh (mount/dummy points) are skipped.
7. **`texture`** — `Tpk::parse()`: `TEXTURES.BIN` (TPK) → an RGBA8 pixel pool + per-texture descriptors. The pool is the file's **JDLZ-compressed RGBA8** blocks concatenated (it is *not* DXT), laid out in `PAGE_WIDTH` (512)-wide pages; each descriptor gives hash + `pool_offset` (→ x,y) + format code. **Per-texture width/height are not yet decoded** — the module deliberately exposes the pool and origins, not cropped `NfsTexture`s. See its module docs for the descriptor table.
8. **`placement`** — what a solid's local matrix *means*: a placement to apply, or a pose already baked into the vertices (`should_place`). Format semantics, so every consumer (engine layer, CLI exporter) decides it the same way.
9. **`parts`** — **pure policy**: which material group a name is (`group_of`), what its `KIT##`/`KITW##`/`STYLE##` token says, and which parts make up a configuration (`select_car`). Lives here so the `nfs` CLI and the game select identically; the game re-exports it as `nfsu2::parts`.
10. **`types`** — the engine-agnostic output contract (see below).

The top-level `decompress_file()` is one of the few functions that touch the filesystem; everything downstream is pure `&[u8]`.

### Two hard invariants

- **Panic-free parsing.** The crate is `#![forbid(unsafe_code)]`. Input is always untrusted: every read is bounds-checked and returns an `NfsError`; no parse path may panic, `unwrap`, or allocate from an unchecked size field. `tests/no_panic.rs` enforces this with proptest against arbitrary/adversarial bytes — **any new parser must uphold it.**
- **Empirically-locked formats.** Several NFSU2 sub-formats have no public byte-level spec. Their exact offsets/constants are locked *empirically* using `nfs dump`/`nfs probe` against a legally-owned install, never by assuming unconfirmed constants. When touching format code, document offsets the way `geometry/mod.rs` does (chunk-ID map + stride/field-index constants) and validate against a real car. The objective correctness check for vertex layouts is `NfsMeshPart::indices_in_range()` (a correct layout yields all in-range indices).

### Output contract (`types`)

Pure-data structs, no `glam`/`wgpu`. Geometry is **indexed** and transforms are stored **as-in-file** (row-major, original handedness). Expanding indices to a flat vertex list and any coordinate-system fixups are deliberately the **integration layer's** job, not the parser's — e.g. the game's `geom::remap()` converts NFSU2's Z-up frame to Gizmo's (and `nfs export` deliberately does not, writing the file's own frame). `serde` derives on all output types are gated behind the optional `serde` feature.

## Conventions

- Code comments and Cargo.toml notes are frequently in **Turkish** — match the surrounding language when editing a file.
- **Part names are truncated** to a fixed-length field in `GEOMETRY.BIN`. Long names lose their tail: `..._HEADLIGHT_LEFT_LOD_A` arrives as `..._HEADLIGHT_LEFT_` (LOD letter gone, so two LODs share a name — disambiguate by triangle count) and `..._SIDE_MIRROR` as `..._MIRRO`/`..._MIRR`. Match shortened stems (`MIRR`), never assume the full word survives. Cars also carry `STYLE00..STYLE14` purchasable part variants and `KIT01+` body kits alongside the default `BASE`/`KIT00`; render only the default set or variants overlap.
- Status is milestone-phased (M1–M3 done). The open blocker for visually-faithful cars is the **TPK texture format** (data at the descriptor offset is not raw DXT; no public spec). See `crates/gizmo-nfs/README.md` for the per-format status table.
