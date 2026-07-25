//! Choosing and preparing the textures a car's surfaces are drawn with: matching a TPK image
//! to a part by name, picking the body-detail (doorline) overlay, and compositing that overlay
//! over the paint colour.

use crate::parts::Grp;
use gizmo_nfs::{AssetHash, NfsMeshPart, NfsTexture, Tpk};

/// Encode a linear `0..1` channel to an 8-bit sRGB value.
#[inline]
fn linear_to_srgb_u8(c: f32) -> u8 {
    let s = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Alpha-composite a body-detail overlay (`overlay`: doorline panel lines/handles on a
/// transparent field, RGBA8) over a solid `paint` colour, returning ready-to-upload opaque RGBA8
/// the same size. Transparent texels become the paint; opaque texels keep the overlay. The paint
/// is written in sRGB so an sRGB-sampling albedo texture brings it back to the intended linear
/// colour.
#[must_use]
pub fn composite_over_paint(overlay: &[u8], paint: [f32; 3]) -> Vec<u8> {
    let ps = [linear_to_srgb_u8(paint[0]), linear_to_srgb_u8(paint[1]), linear_to_srgb_u8(paint[2])];
    let mut out = Vec::with_capacity(overlay.len());
    for px in overlay.chunks_exact(4) {
        let a = px[3] as f32 / 255.0;
        for k in 0..3 {
            out.push((px[k] as f32 * a + ps[k] as f32 * (1.0 - a)).round() as u8);
        }
        out.push(255);
    }
    out
}

/// The car's body-detail (**doorline**) texture — panel gaps, handles and creases UV-mapped
/// across the paint. It is referenced by naming convention (`<CAR>_DOORLINE`), not by a geometry
/// material hash, so it is resolved by name here. Prefer the kit-body variant when a KIT00 body
/// supplies the outer paint, else the base `_DOORLINE`.
pub(crate) fn doorline_texture(tpk: &Tpk, has_kit_body: bool) -> Option<&NfsTexture> {
    let pick = |suffix: &str| {
        tpk.textures
            .values()
            .filter(|t| t.name.ends_with(suffix))
            // The `_MASK` companion is what turned cars black: TPK DebugNames are cut at 23
            // characters, so on a long car name it arrives under the map's own name
            // (`LANCEREVO8_DOORLINE_KIT_MASK` → `LANCEREVO8_DOORLINE_KIT`), fully opaque. The hash
            // is of the *whole* name, so `is_mask()` settles it outright — no longer a guess from
            // how transparent the image happens to be.
            .filter(|t| !t.is_mask())
            // What is left may still be a mask this cannot prove (a tail nobody can recover) or a
            // format the decoder does not yet read, whose alpha is meaningless. Both are caught by
            // preferring the most transparent candidate, with the hash as a deterministic
            // tie-break — `textures` is a `HashMap` whose order varies per run, which is why the
            // Lancer and the Impreza used to render black at random rather than always.
            .min_by_key(|t| (t.opaque_permille(), t.hash.0))
            // A full-coverage map is a mask or an undecoded format, not a detail overlay:
            // compositing it paints the whole car its own (near-black) colour, as it did on the
            // IS300, whose real `_DOORLINE` is opaque too. Better no detail than a black car.
            .filter(|t| t.opaque_permille() < OVERLAY_MAX_OPAQUE_PERMILLE)
    };
    if has_kit_body {
        pick("_DOORLINE_KIT").or_else(|| pick("_DOORLINE"))
    } else {
        pick("_DOORLINE")
    }
}

/// Above this coverage a "detail overlay" is really a mask (or an undecodable map) and is dropped.
const OVERLAY_MAX_OPAQUE_PERMILLE: u32 = 900;

/// Minimum shared-prefix length for a part name to match a texture's DebugName — long
/// enough to reach past the shared `CAR_KIT00_` prefix into the component word.
const NAME_MATCH_MIN: usize = 16;

fn common_prefix_len(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

/// The decoded texture whose DebugName best matches `part_name` (longest common prefix,
/// at least [`NAME_MATCH_MIN`] characters), or `None` if nothing matches closely.
pub(crate) fn texture_for_name<'a>(part_name: &str, tpk: &'a Tpk) -> Option<&'a NfsTexture> {
    tpk.textures
        .values()
        .map(|t| (common_prefix_len(part_name, &t.name), t))
        .filter(|(cpl, _)| *cpl >= NAME_MATCH_MIN)
        // Break prefix-length ties deterministically. NFSU2 ships lens textures in pairs that
        // share one DebugName (the two `240SX_KIT00_BRAKELIGHT_` maps, or `_HEADLIGHT_G`/`_O`)
        // alongside `_MASK` alpha companions; plain `max_by_key(cpl)` otherwise let `HashMap`
        // iteration order pick one at random per run. Prefer a non-`_MASK` map, then the larger
        // image (the detailed diffuse over its lower-res companion), then the lowest hash so the
        // pick is fully stable across runs.
        .max_by_key(|(cpl, t)| {
            // `is_mask()` rather than a suffix test: the same truncation hides `_MASK` here too,
            // and a mask bound as a diffuse map is a black panel.
            (*cpl, !t.is_mask(), t.width * t.height, std::cmp::Reverse(t.hash.0))
        })
        .map(|(_, t)| t)
}

/// Resolve a whole part (one carrying no `0x00134B02` material list) to a texture the old
/// way: painted panels stay flat, else match by DebugName prefix, then fall back to the
/// material-hash list. Parts *with* a material list are resolved per-run in
/// [`build_car_visuals`] instead.
pub(crate) fn resolve_whole(p: &NfsMeshPart, grp: Grp, tpk: &Tpk) -> Option<AssetHash> {
    if grp == Grp::Paint {
        return None;
    }
    if let Some(tex) = texture_for_name(&p.name, tpk) {
        return Some(tex.hash);
    }
    p.material_refs.iter().copied().find(|h| tpk.texture(*h).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same as [`tex`], but with the hash of `real_name` — a name the 23-character field cut.
    fn tex_named(stored: &str, real_name: &str, texels: usize, opaque: usize) -> NfsTexture {
        let mut t = tex(stored, 0, texels, opaque);
        t.hash = gizmo_nfs::hash::asset_hash(real_name);
        t
    }

    /// A texture of `texels` pixels, `opaque` of them opaque — enough for the overlay picker,
    /// which only reads the alpha channel, the name and the hash.
    fn tex(name: &str, hash: u32, texels: usize, opaque: usize) -> NfsTexture {
        let mut rgba = Vec::with_capacity(texels * 4);
        for i in 0..texels {
            rgba.extend_from_slice(&[0, 0, 0, if i < opaque { 255 } else { 0 }]);
        }
        let mut t = NfsTexture::default();
        t.name = name.to_string();
        t.hash = AssetHash(hash);
        t.width = texels as u32;
        t.height = 1;
        t.rgba = rgba;
        t
    }

    fn tpk_of(textures: Vec<NfsTexture>) -> Tpk {
        let mut tpk = Tpk::default();
        tpk.textures = textures.into_iter().map(|t| (t.hash, t)).collect();
        tpk
    }

#[test]
    fn composite_transparent_becomes_paint_opaque_keeps_overlay() {
        // Two texels: fully transparent, then fully opaque. Paint = white (sRGB white = 255).
        let overlay = [10u8, 20, 30, 0, 100, 150, 200, 255];
        let out = composite_over_paint(&overlay, [1.0, 1.0, 1.0]);
        assert_eq!(&out[0..4], &[255, 255, 255, 255], "transparent texel becomes the paint");
        assert_eq!(&out[4..8], &[100, 150, 200, 255], "opaque texel keeps the overlay colour");
    }

#[test]
    fn composite_output_is_opaque_same_size() {
        let overlay = [0u8, 0, 0, 128, 0, 0, 0, 200];
        let out = composite_over_paint(&overlay, [0.0, 0.0, 0.0]);
        assert_eq!(out.len(), overlay.len());
        assert_eq!(out[3], 255);
        assert_eq!(out[7], 255);
    }

#[test]
    fn srgb_encode_endpoints() {
        assert_eq!(linear_to_srgb_u8(0.0), 0);
        assert_eq!(linear_to_srgb_u8(1.0), 255);
    }

#[test]
    fn overlay_pick_prefers_the_map_over_its_truncated_mask_twin() {
        // TPK DebugNames are cut at 23 chars, so `LANCEREVO8_DOORLINE_KIT_MASK` arrives ending in
        // the same stem as the map. The mask is fully opaque, the map mostly transparent — and
        // `textures` is a HashMap, so picking by iteration order rendered the car black at random.
        let tpk = tpk_of(vec![
            tex("LANCEREVO8_DOORLINE_KIT", 0x1111_1111, 100, 100), // the truncated _MASK
            tex("LANCEREVO8_DOORLINE_KIT", 0x2222_2222, 100, 19),  // the real map
        ]);
        let picked = doorline_texture(&tpk, true).expect("a doorline overlay");
        assert_eq!(picked.hash.0, 0x2222_2222);
    }

#[test]
    fn a_proven_mask_loses_even_when_it_is_the_more_transparent_image() {
        // The hash settles what the name cannot. Here the mask is *more* transparent than the map,
        // so the old "pick the most transparent" rule would have chosen it; being able to prove it
        // is a `_MASK` is what makes the choice right rather than lucky.
        let tpk = tpk_of(vec![
            tex_named("LANCEREVO8_DOORLINE_KIT", "LANCEREVO8_DOORLINE_KIT_MASK", 100, 1),
            tex_named("LANCEREVO8_DOORLINE_KIT", "LANCEREVO8_DOORLINE_KIT", 100, 50),
        ]);
        let picked = doorline_texture(&tpk, true).expect("a doorline overlay");
        assert_eq!(picked.hash, gizmo_nfs::hash::asset_hash("LANCEREVO8_DOORLINE_KIT"));
        assert!(!picked.is_mask());
    }

#[test]
    fn a_full_coverage_overlay_is_refused() {
        // The IS300's own `_DOORLINE` is opaque: composited it would paint the whole car its
        // near-black colour. No overlay is better than a black car.
        let tpk = tpk_of(vec![tex("IS300_DOORLINE", 0x3333_3333, 100, 100)]);
        assert!(doorline_texture(&tpk, false).is_none());
        // A kit body with only an opaque `_DOORLINE_KIT` falls back to `_DOORLINE` — and refuses
        // that too when it is opaque as well.
        let tpk = tpk_of(vec![
            tex("IS300_DOORLINE_KIT", 0x4444_4444, 100, 100),
            tex("IS300_DOORLINE", 0x5555_5555, 100, 100),
        ]);
        assert!(doorline_texture(&tpk, true).is_none());
    }
}
