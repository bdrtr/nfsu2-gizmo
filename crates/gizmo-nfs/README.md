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
| TPK textures → RGBA (DXT/P8) | `texture` | 🚧 in progress — 24-byte descriptors (73 textures) mapped; width/height/format decode next |
| `GLOBALB.LZC` global data | `global` | 🔜 later |
| World / city (`STREAM*.BUN`, `L4RA.BUN`) | `world` | 🔴 research-frontier |

Several NFSU2 sub-formats have **no clean public byte-level spec**; those modules are
built defensively and their exact offsets are locked empirically using the `nfs_dump`
example against a legally-owned game install — never by assuming unconfirmed constants.

## Legal / asset hygiene

This crate ships **no copyrighted game data**. All tests use synthetic byte buffers.
Reading real assets is done at runtime from a user-provided install path. You must own
your copy of the game.

## Reverse-engineering tool

```bash
cargo run -p gizmo-nfs --features tools --example nfs_dump -- /path/to/FILE
```

Detects the compression codec, decompresses if needed, and prints the chunk tree (or
lists a BIGF/VIV archive's contents) — the workhorse for locking the unconfirmed formats.
