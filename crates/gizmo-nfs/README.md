# gizmo-nfs

A pure, **engine-agnostic** parser for [Need for Speed: Underground 2](https://en.wikipedia.org/wiki/Need_for_Speed:_Underground_2) (2004) asset files, part of the Gizmo Engine workspace.

It reads NFSU2's binary asset containers and hands back plain CPU-side data
(`Vec<f32>` / `Vec<u32>` / `Vec<u8>`) — **no `wgpu`, no `glam`, no renderer coupling**.
Turning that data into engine meshes/materials is the job of a separate integration
layer (a demo binary or an optional `gizmo-nfs-engine` crate), not this crate.

## Status (phased)

| Area | Module | Status |
|------|--------|--------|
| Bounds-checked byte reader | `reader` | ✅ done |
| FourCC helper | `fourcc` | ✅ done |
| Chunk tree (`BinSectionHeader`, high-bit = container) | `chunk` | ✅ done |
| RefPack / QFS decompression | `compression::refpack` | ✅ done |
| JDLZ decompression | `compression::jdlz` | ✅ done — validated byte-exact against a real golden pair |
| BIGF / VIV archive reader | `viv` | ✅ done |
| Output data contract | `types` | ✅ defined |
| `GEOMETRY.BIN` car models | `geometry` | ✅ done — stride-36 vertices (pos/normal/uv) + u16 indices, validated on real cars |
| TPK textures → RGBA8 images | `texture` | 🟡 mostly done — each 24-byte descriptor decoded to its own image: whole-file offset → JDLZ blob → embedded `OldTextureInfo` (width/height/format) → DXT1/3/5 or raw RGBA. **HUFF-compressed** textures are listed in `entries` but not decoded. |
| glTF (`.glb`) + OBJ/MTL + PNG output | `export` | ✅ done — pure text/bytes, no filesystem; shared by `ug2 export` and STRUKT |
| `GLOBALB.LZC` global data | `global` | 🔜 later |
| World / city (`STREAM*.BUN`, `L4RA.BUN`) | `world` | 🔴 research-frontier |

Several NFSU2 sub-formats have **no clean public byte-level spec**; those modules are
built defensively and their exact offsets are locked empirically using the `ug2` tool
(`ug2 dump` / `ug2 probe`) against a legally-owned game install — never by assuming
unconfirmed constants.

## Legal / asset hygiene

This crate ships **no copyrighted game data**. All tests use synthetic byte buffers.
Reading real assets is done at runtime from a user-provided install path. You must own
your copy of the game.

## The `ug2` command-line tool

```bash
cargo run -p gizmo-nfs --features tools --bin ug2 -- <command>
```

| command | what it answers |
|---|---|
| `ug2 info CARS/240SX` | what this car is: parts, the variants it ships (`--kit`/`--hood`/`--light`/`--wide`), dimensions, and its `GLOBALB` wheel record |
| `ug2 parts CARS/240SX [--selected --kit 3]` | every part grouped by customization namespace, or just the ones a configuration selects |
| `ug2 export CARS/240SX -o out/ [--kit 3 --wide 1] [--format glb\|obj\|both]` | the car as a self-contained `.glb` and/or OBJ + MTL + PNG — importable anywhere |
| `ug2 textures CARS/240SX` | the texture table, and which material run resolves to which image |
| `ug2 dump FILE` | the chunk tree of any asset file (or a BIGF/VIV archive's contents) |
| `ug2 probe CARS/240SX [--matrices]` | the raw solid view: declared counts vs. buffer sizes, mesh-header words, matrix classification |
| `ug2 globalb GLOBALB.BUN` | wheel mounts, radius and mass per car |

`dump` and `probe` are the reverse-engineering levers: every unconfirmed offset in this crate
was locked with them against a legally-owned install, never by assuming a constant.

Exports use NFSU2's own coordinates (x = length, y = width, z = height, Z-up — what Blender
reads natively) with each solid's placement applied, so no axis fixup is invented on the way out.
