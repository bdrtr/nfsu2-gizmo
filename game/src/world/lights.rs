//! The city's street lamps as the engine's lights.
//!
//! `gizmo_nfs::world::lights` reads what each streaming section places: 2,234 lamps in `STREAML4RA`,
//! each a point that reaches 30 m. The city does not need them — its light, theirs included, is baked
//! into its vertex colours, and the engine's baked-lit path draws a dry surface by that alone. What
//! does need them is everything the bake cannot hold: the car's paint passing under a lamp, and the
//! rain on the road, which reflects every lamp along the street (the engine's `water_glints`).
//!
//! The engine ranks them against the camera each frame and keeps the nearest 256 that reach the view,
//! so all of them are spawned.

use super::remap;
use gizmo::prelude::*;

/// One lamp, in Gizmo's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lamp {
    pub position: Vec3,
    /// Linear RGB, the file's colour as it stands — `0x96` grey for most, warm for the orange
    /// fills, cool for some of the `XO_StreetLightC`/`K`.
    pub colour: Vec3,
    /// Where it reaches nothing, metres (the file's falloff end).
    pub reach: f32,
}

/// A light reaching further than this is not a lamp: `EL_NightA_Moon` stands 3 km up and reaches
/// 10 km. The night's own light is the scene's sun.
const LAMP_REACH: f32 = 100.0;

/// How bright a lamp is, per unit of its colour: a `0x96` lamp 13 m above the road (the
/// `EL_StreetlightAtoE` at the race start stand that high) puts about the scene's moon
/// (`scene::add_lights`' key, 2.2) on a surface facing it — 1300 × 0.305 / 13² × the engine's window
/// at 13 of 30 m, 0.93. Measured on the car parked under one: at 450 the paint barely changed (the
/// mean of its pixels 92 → 96), at 2,600 the rear window went white, at 1,300 the roof and the boot
/// light up (→ 102) and the wet road's reflections of the lamps stay short of glare.
pub const LAMP_INTENSITY: f32 = 1300.0;

/// The lamps of one stream bundle.
#[must_use]
pub fn lamps(bundle: &[u8]) -> Vec<Lamp> {
    let srgb = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    gizmo_nfs::world::lights::lights(bundle)
        .unwrap_or_default()
        .into_iter()
        .filter(|l| l.falloff[1] > 0.0 && l.falloff[1] <= LAMP_REACH)
        .map(|l| Lamp {
            position: remap(l.position),
            colour: Vec3::new(srgb(l.colour[0]), srgb(l.colour[1]), srgb(l.colour[2])),
            reach: l.falloff[1],
        })
        .collect()
}

/// A [`PointLight`] each, at [`LAMP_INTENSITY`]. Returns how many.
pub fn spawn(world: &mut World, lamps: &[Lamp]) -> usize {
    for l in lamps {
        let e = world.spawn();
        world.add_component(e, Transform::new(l.position));
        world.add_component(e, PointLight::new(l.colour, LAMP_INTENSITY, l.reach));
    }
    lamps.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bundle of one pack: a header and `lights`, each `(x, reach, colour)`.
    fn bundle(lights: &[(f32, f32, [u8; 4])]) -> Vec<u8> {
        let chunk = |id: u32, body: Vec<u8>| {
            let mut c = id.to_le_bytes().to_vec();
            c.extend((body.len() as u32).to_le_bytes());
            c.extend(body);
            c
        };
        // The header's payload starts at 16, on its boundary; the records' at 56, 8 bytes short of one.
        let mut header = vec![0u8; 0x20];
        header[0x1C..0x20].copy_from_slice(&(lights.len() as u32).to_le_bytes());
        let mut records = vec![0x11u8; 8];
        for (x, reach, colour) in lights {
            let mut r = vec![0u8; 0x60];
            r[0x0C..0x10].copy_from_slice(colour);
            r[0x10..0x14].copy_from_slice(&x.to_le_bytes());
            r[0x34..0x38].copy_from_slice(&reach.to_le_bytes());
            records.extend(r);
        }
        let mut pack = chunk(0x0013_5001, header);
        pack.extend(chunk(0x0013_5003, records));
        chunk(0x8013_5000, pack)
    }

    #[test]
    fn lamps_come_into_gizmo_s_frame_and_the_moon_stays_out() {
        let b = bundle(&[(10.0, 30.0, [0x96, 0x96, 0x96, 0xFF]), (0.0, 10_000.0, [0x28, 0x28, 0x37, 0xFF])]);
        let l = lamps(&b);
        assert_eq!(l.len(), 1, "the moon is not a lamp: {l:?}");
        assert_eq!(l[0].position, remap([10.0, 0.0, 0.0]));
        assert_eq!(l[0].reach, 30.0);
        assert!((l[0].colour.x - 0.305).abs() < 0.002, "0x96 is about 0.3 linear: {:?}", l[0].colour);
        assert!(l[0].colour.x == l[0].colour.y && l[0].colour.y == l[0].colour.z);
    }
}
