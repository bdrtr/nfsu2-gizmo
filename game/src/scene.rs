//! Engine-side scaffolding the demo binaries share.
//!
//! Every binary builds the same car the same way — upload each resolved texture, spawn one
//! entity per material group and per textured mesh, then instance the wheel at four corners —
//! and differs only in what it does *with* that car afterwards. That common half lives here so
//! a binary is left with its own scene, its own camera and its own update loop.

use crate::car::{composite_over_paint, CarVisuals, WheelFit, WheelSurface};
use crate::geom::add_transform;
use gizmo::prelude::*;
use gizmo::renderer::components::{LightRole, ShadowCasting};
use gizmo::wgpu;
use gizmo_nfs::CarTypeInfo;
use std::sync::Arc;

/// How much light the city gets with the sun out of the picture, and how much it makes itself.
///
/// The engine's baked-lit path computes `(baked · shadow + ambient) · albedo · texture + emissive`,
/// and both new terms default to zero — so the arms exist and nothing was pulling them. Bayview
/// needs them pulled: it is a **night** map whose lighting is baked into vertex colours, and
/// multiplied straight through, a surface with vertex colour 64 and texture 64 lands at **2.6/255**.
/// Measured on the street, the frame's 5th percentile sat at 24/255 with the tower reading as a
/// silhouette with lit windows and nothing in between.
///
/// `ambient` goes *inside* the multiply, so it lifts the black end while the texture still decides
/// what the surface looks like — a dark-albedo wall stays darker than a pale one, which is the
/// property that makes this a lift rather than a fog. `emissive` is added *after* and is not
/// touched by the texture, so it is the wrong knob for a city and stays at zero unless asked.
///
/// Both are overridable — `NFS_AMBIENT="r,g,b"` or a single number for grey, same for
/// `NFS_EMISSIVE` — because the right value is a judgement about how a night map should read, and
/// a judgement left in a constant is one nobody can argue with.
#[must_use]
pub fn city_lift() -> (Vec3, Vec3) {
    fn read(key: &str, fallback: Vec3) -> Vec3 {
        let Ok(v) = std::env::var(key) else { return fallback };
        let parts: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        match parts[..] {
            [a] => Vec3::splat(a),
            [r, g, b] => Vec3::new(r, g, b),
            _ => fallback,
        }
    }
    (read("NFS_AMBIENT", CITY_AMBIENT), read("NFS_EMISSIVE", Vec3::ZERO))
}

/// The ambient the city is drawn with by default.
///
/// Slightly blue because Bayview's key light is street lighting and sky, not sun, and a neutral
/// grey lift reads as haze.
///
/// The magnitude comes from a sweep rather than from taste. Measured on a street-level frame of
/// `STREAML4RA`, luminance over the whole frame:
///
/// | ambient | median | p05 | p25 | p95 |
/// |---|---|---|---|---|
/// | 0.00 | 54 | 24 | 41 | 155 |
/// | 0.08 | 63 | 28 | 49 | 155 |
/// | **0.12** | **68** | **30** | **52** | **155** |
/// | 0.24 | 81 | 35 | 62 | 155 |
///
/// **p95 does not move at any setting**, which is the property that makes this safe: the bright end
/// is texture-dominated and the lift is proportionally nothing there, so neon and lit windows keep
/// their separation from the dark they sit against. 0.24 was rejected by eye — the frame reads as
/// overcast rather than as night — and 0.12 is where the road surface and the wall's relief become
/// legible without the sky-to-ground contrast collapsing.
///
/// That sweep was taken under ACES with raw vertex colours. Re-measured on 2026-11-18 under what the
/// city draws with now (PBR Neutral, decoded vertex colours — `city_tonemap`), same street:
///
/// | ambient × | p05 | p25 | median | p95 |
/// |---|---|---|---|---|
/// | **1** | **14** | **24** | **30** | **76** |
/// | ½ | 12 | 20 | 29 | 66 |
/// | ¼ | 11 | 18 | 28 | 62 |
/// | 0 | 9 | 17 | 27 | 58 |
///
/// The lift now works only where it was meant to, at the dark end (p05 9 → 14), and the frame the
/// user chose was taken with it, so it stays.
pub const CITY_AMBIENT: Vec3 = Vec3::new(0.10, 0.11, 0.14);

/// What crosses the glare threshold in this city, and how hard it glows.
///
/// The renderer's own default is a threshold of `0.85`, and **nothing in Bayview ever reaches it**:
/// the brightest surfaces are a p95 vertex colour of 183 against window textures that peak at 250,
/// which is about `0.44` in linear. So the glare pass ran every frame and extracted nothing. That
/// default is right for a scene with values above `1.0` and a baked-lit night map is not one — the
/// city's whole lighting is a byte per vertex, and a byte cannot be over-bright.
///
/// Lowering it puts the glow back on exactly the things that should have it. Measured over the
/// frame, threshold `0.85` → `0.18` moves the maximum from 202 to 221 and the share of pixels over
/// 200 from 0.00 % to 0.06 % — a small number, and the right small number: it is the lit windows
/// and the neon, and nothing else moved. The median does not shift at all, so this is glare and not
/// a brightness knob; [`city_lift`] is the brightness knob.
///
/// `NFS_BLOOM="threshold[,intensity]"` overrides both.
#[must_use]
pub fn city_glare() -> (f32, f32) {
    let mut out = (CITY_BLOOM_THRESHOLD, CITY_BLOOM_INTENSITY);
    if let Ok(v) = std::env::var("NFS_BLOOM") {
        let n: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if let Some(t) = n.first() {
            out.0 = *t;
        }
        if let Some(i) = n.get(1) {
            out.1 = *i;
        }
    }
    out
}

/// The tone curve the city is drawn with: **PBR Neutral**, not the renderer's default ACES.
///
/// Chosen by eye on 2026-11-18 from five frames of one street (ACES/Neutral/none × raw/decoded
/// vertex colours), as the answer to "everything is too bright — it does not read as night". ACES
/// lifts the middle of this city's range: its light is a byte per vertex, display-referred, and
/// ACES both darkens the dark end 4.67× (MOTOR-NOTLARI #3) and pushes the mids up, so the frame
/// came out grey and washed (median 65/255). Neutral leaves values below its knee alone — a night
/// map stays as dark as its bytes say — and only rolls off the top, where neon and lit windows sit.
///
/// It goes together with the vertex colours being decoded (`world::build`): the two were judged as
/// a pair, and either one alone is a different picture. `NFS_TONEMAP=aces|none|neutral|agx|
/// reinhard|reinhard_lum` names another curve, for looking.
#[must_use]
pub fn city_tonemap() -> gizmo::renderer::gpu_types::TonemapCurve {
    use gizmo::renderer::gpu_types::TonemapCurve;
    match std::env::var("NFS_TONEMAP").as_deref() {
        Ok("aces") => TonemapCurve::Aces,
        Ok("none") => TonemapCurve::None,
        Ok("agx") => TonemapCurve::AgX,
        Ok("reinhard") => TonemapCurve::Reinhard,
        Ok("reinhard_lum") => TonemapCurve::ReinhardLuminance,
        _ => TonemapCurve::PbrNeutral,
    }
}

/// How much of the night's light reaches the screen: the camera's exposure, before the tone curve
/// ([`set_exposure`]). [`CITY_EXPOSURE`] unless `NFS_EXPOSURE=<number>` says otherwise.
#[must_use]
pub fn city_exposure() -> f32 {
    std::env::var("NFS_EXPOSURE").ok().and_then(|v| v.trim().parse::<f32>().ok()).filter(|e| *e > 0.0).unwrap_or(CITY_EXPOSURE)
}

/// The night's exposure: see [`city_exposure`].
///
/// Chosen by the user on 2026-11-20 from four frames of the race start, with the lamps and the headlights
/// on, as the answer to "it is night, the surroundings should be darker": exposure 1, 0.7 and 0.5, and 0.5
/// with half the [`CITY_AMBIENT`]. At 1 the frame's median was 33/255, at 0.5 it is 12 — the lit windows,
/// the signs and the lamps still stand out, being where the tone curve's top is.
pub const CITY_EXPOSURE: f32 = 0.5;

/// The night's eye: the engine's auto exposure (`Renderer::auto_exposure`), metered by the frame's
/// log-average luminance, keyed so that the race start reads as the user chose it ([`CITY_EXPOSURE`]) —
/// the camera's exposure is then its compensation — and held to [`CITY_EYE_RANGE`] of that, so that a lit
/// square darkens and a dark road opens up without the night turning into day. Returns whether the
/// renderer has one (the browser does not).
pub fn enable_night_eye(renderer: &mut gizmo::renderer::Renderer) -> bool {
    let Some(eye) = renderer.auto_exposure.as_mut() else { return false };
    eye.enabled = true;
    eye.settings.key = CITY_EYE_KEY;
    eye.settings.min_exposure = CITY_EYE_RANGE.0;
    eye.settings.max_exposure = CITY_EYE_RANGE.1;
    true
}

/// The log-average luminance the night's eye exposes to ([`enable_night_eye`]): what the race start
/// measures, with the lamps and the headlights on — 0.051–0.065 dry and 0.050–0.056 wet over its first
/// eight seconds (2026-11-20) — so that the eye is about 1 there and the start is the frame the user chose.
pub const CITY_EYE_KEY: f32 = 0.055;
/// How far the night's eye may move the exposure, as a factor on [`CITY_EXPOSURE`]: down 1.5 stops for a
/// bright square, up 0.6 for a dark road — enough to see by, not enough for a night to turn into dusk.
pub const CITY_EYE_RANGE: (f32, f32) = (0.35, 1.5);

/// The night's local exposure (`Renderer::local_exposure`): each pixel exposed for its own
/// neighbourhood, so that what headlights put on a car's bumper five metres ahead does not burn white
/// while the street around it stays dark. Highlights' contrast at [`CITY_LOCAL_CONTRAST`] above
/// [`CITY_LOCAL_PIVOT`], unless `NFS_LOCAL="contrast[,pivot]"` says otherwise (`off` leaves it off).
/// Returns whether it is on.
pub fn enable_local_exposure(renderer: &mut gizmo::renderer::Renderer) -> bool {
    let (mut contrast, mut pivot) = (CITY_LOCAL_CONTRAST, CITY_LOCAL_PIVOT);
    match std::env::var("NFS_LOCAL").as_deref() {
        Ok("off") => return false,
        Ok(v) => {
            let n: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            contrast = n.first().copied().unwrap_or(contrast);
            pivot = n.get(1).copied().unwrap_or(pivot);
        }
        Err(_) => {}
    }
    let Some(local) = renderer.local_exposure.as_mut() else { return false };
    local.enabled = true;
    local.settings.highlight_contrast = contrast;
    local.settings.pivot = pivot;
    true
}

/// How much of the highlights' contrast local exposure keeps ([`enable_local_exposure`]), and above what
/// exposed luminance ([`CITY_LOCAL_PIVOT`]). Chosen on 2026-11-20 from the race start's bumper — the car
/// ahead lit by the player's headlights five metres off — and a city frame: with none the bumper was a
/// beige blank (the frame's 99th percentile 223); at 0.5 / 0.5 and 0.35 / 0.3 it barely moved (219, 207),
/// being bright rather than burnt — 0.8–1 exposed, not far over those pivots; at 0.2 / 0.15 its shape and
/// the tail lights' colour come back (157) and the city's lit windows keep their place (178 → 131), the
/// frame's median unmoved at 20.
pub const CITY_LOCAL_CONTRAST: f32 = 0.2;
/// The exposed luminance above which local exposure compresses: see [`CITY_LOCAL_CONTRAST`].
pub const CITY_LOCAL_PIVOT: f32 = 0.15;

/// The night's haze: the engine's volumetric light (`Renderer::volumetric`) scattering the street lamps'
/// and the headlights' light in the air — a halo round every lamp, a beam ahead of every car — thicker in
/// the rain. Only the local lights: the scene's sun is the car's moon, and it would fog the whole frame.
/// [`CITY_HAZE`] dry and [`CITY_HAZE_WET`] wet, unless `NFS_HAZE=<scatter>` says otherwise (`off` leaves
/// the air clear and the pass out).
pub fn set_haze(renderer: &mut gizmo::renderer::Renderer, wet: bool) {
    let scatter = match std::env::var("NFS_HAZE").as_deref() {
        Ok("off") => None,
        Ok(v) => v.trim().parse::<f32>().ok(),
        Err(_) => Some(if wet { CITY_HAZE_WET } else { CITY_HAZE }),
    };
    let gizmo::renderer::Renderer { volumetric, device, deferred, .. } = renderer;
    match (scatter, volumetric.as_mut(), deferred.as_ref()) {
        (Some(scatter), Some(air), Some(deferred)) if scatter > 0.0 => {
            air.params.bulb_scatter = scatter;
            air.params.sun_scatter = 0.0;
            // The engine integrates each lamp's air once a ray, over its whole chord, so the halos do not
            // need the 16 steps a sun's shafts do; and past 70 m the city's fog has the air.
            air.params.steps = HAZE_STEPS;
            air.params.max_distance = HAZE_REACH;
            air.set_march_divisor(device, deferred, haze_march_divisor());
        }
        _ => *volumetric = None,
    }
}

/// How much faster than the clock the game runs ([`game_speed`]): 1.25. The city is at its true scale —
/// a lane 3.78 m, the 240SX 4.39 m — and the speedometer is the car's own m/s ×3.6, so at "100 km/h" the
/// road went by at a real 100 km/h, 28 m a second, which from 6 m behind looked too slow ("100 km/h'de
/// daha fazla yol almalıyız gibi", the user, 2026-11-20). Running the whole game a quarter faster than
/// the clock keeps every number honest in the game's own time — the speedometer, the physics, the laps —
/// and puts 35 m of road a second under the car at "100". The user's choice over reading the speedometer
/// low or giving the car more power, which would not have changed the road per "km/h".
pub const GAME_SPEED: f32 = 1.25;

/// The game's speed against the clock: [`GAME_SPEED`], or `NFS_GAME_SPEED=<k>` (`1` is real time),
/// kept within 0.25–4. Read once.
#[must_use]
pub fn game_speed() -> f32 {
    static SPEED: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SPEED.get_or_init(|| game_speed_from(std::env::var("NFS_GAME_SPEED").ok().as_deref()))
}

/// [`game_speed`] from what `NFS_GAME_SPEED` says, if anything.
fn game_speed_from(setting: Option<&str>) -> f32 {
    setting
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|k| k.is_finite() && *k > 0.0)
        .map_or(GAME_SPEED, |k| k.clamp(0.25, 4.0))
}

#[cfg(test)]
mod game_speed_tests {
    use super::*;

    #[test]
    fn the_game_runs_a_quarter_faster_unless_told_otherwise() {
        assert_eq!(game_speed_from(None), GAME_SPEED);
        assert_eq!(GAME_SPEED, 1.25);
        assert_eq!(game_speed_from(Some("1")), 1.0, "real time");
        assert_eq!(game_speed_from(Some(" 1.5 ")), 1.5);
        assert_eq!(game_speed_from(Some("10")), 4.0, "kept within reason");
        assert_eq!(game_speed_from(Some("0.1")), 0.25);
        for nonsense in ["0", "-1", "fast", "", "NaN", "inf"] {
            assert_eq!(game_speed_from(Some(nonsense)), GAME_SPEED, "{nonsense:?}");
        }
        // At "100 km/h" the road under the car, metres a second of the clock.
        assert!((100.0 / 3.6 * GAME_SPEED - 34.7).abs() < 0.1);
    }
}

/// How much of the rain a car soaks up (`Material::porosity`): none. Paint, glass, chrome, rubber and the
/// lights are sealed — the rain lies on them as a film, smooth and rippling, and neither darkens them like
/// the road nor stands a puddle on a roof. Porous, as the engine's default is, the 240SX's paint went as
/// dark as the asphalt in the rain, and a puddle of the world's noise slid over its boot as it drove
/// (`nfs_city` from above the start, `NFS_WET=1 NFS_PUDDLES=1`, 2026-11-20).
pub const CAR_POROSITY: f32 = 0.0;

/// `NFS_WET=<0..1>` (`1` = soaked): the city in the rain, `NFS_PUDDLES=<0..1>` of the flat ground
/// under water (0.3 by default). `None` is dry. Every binary that draws the city reads it here, so that
/// the headless stills (`nfs_city`) show what `nfs_cruise` drives through.
#[must_use]
pub fn wet_streets() -> Option<gizmo::renderer::Wetness> {
    let level: f32 = std::env::var("NFS_WET").ok()?.parse().ok()?;
    if level <= 0.0 {
        return None;
    }
    let puddles = std::env::var("NFS_PUDDLES").ok().and_then(|v| v.parse().ok()).unwrap_or(0.3);
    Some(gizmo::renderer::Wetness::new(level).with_puddles(puddles))
}

/// Rain falling in the wet city (`Renderer::rainfall`): streaks round the camera, lit by the lamps and the
/// headlights it falls through, and splashes on the road and the cars where it lands. The engine's own
/// rain, slanted a little by a breeze; `NFS_RAIN=off` stops it, `NFS_RAIN=<drops>` sets how many fall
/// round the camera at once.
pub fn set_rain(renderer: &mut gizmo::renderer::Renderer, wet: bool) {
    let drops = match std::env::var("NFS_RAIN").as_deref() {
        Ok("off") => None,
        Ok(v) => v.trim().parse::<u32>().ok(),
        Err(_) => wet.then_some(CITY_RAIN_DROPS),
    };
    renderer.rainfall = drops.map(|drops| {
        let mut rain = gizmo::renderer::rain::Rainfall::default();
        rain.drops = drops;
        rain.wind = CITY_RAIN_WIND;
        rain
    });
}

/// How many drops fall round the camera at once in the wet city ([`set_rain`]).
pub const CITY_RAIN_DROPS: u32 = 20_000;
/// The breeze that slants the rain, m/s ([`set_rain`]).
pub const CITY_RAIN_WIND: [f32; 3] = [1.2, 0.0, 0.6];

/// The frame's size over the haze's march, each way ([`set_haze`]): 4, a sixteenth of the frame's pixels.
/// The haze is soft and the engine upsamples it by depth, so a lamp's halo keeps off the post in front of
/// it; at the engine's 2 a drive down a lamp-lit street spent 4.3 ms of GPU on it (1080p, AMD 780M,
/// 2026-11-21). `NFS_HAZE_RES=<n>` sets it.
pub const HAZE_MARCH_DIVISOR: u32 = 4;

/// [`HAZE_MARCH_DIVISOR`], or `NFS_HAZE_RES`.
fn haze_march_divisor() -> u32 {
    std::env::var("NFS_HAZE_RES").ok().and_then(|v| v.parse().ok()).unwrap_or(HAZE_MARCH_DIVISOR)
}

/// The haze's march: steps a ray, and how far it goes ([`set_haze`]). Five: the engine integrates each
/// step's stretch of a light's air by quadrature, so a long stretch is as smooth as a short one; at 8
/// the wet haze cost 2.8 ms of GPU, at 5 1.9 (AMD 780M, 2026-11-20).
pub const HAZE_STEPS: f32 = 5.0;
/// See [`HAZE_STEPS`].
pub const HAZE_REACH: f32 = 70.0;

/// How much of the lamps' light the night air scatters, per metre, dry ([`set_haze`]): a faint halo round
/// each lamp head. Chosen on 2026-11-20 from the race start in the rain at 0.0005, 0.0008, 0.0015 and
/// 0.004 — at 0.0008 a soft glow a couple of metres round the bulb, at 0.004 the lamps balls of fog.
pub const CITY_HAZE: f32 = 0.0003;
/// And in the rain: see [`CITY_HAZE`].
pub const CITY_HAZE_WET: f32 = 0.0008;

/// Give the camera `camera` an exposure: the engine's one exposure knob is the camera's
/// (`Camera::exposure`), applied to the whole frame before the tone curve.
pub fn set_exposure(world: &mut World, camera: u32, exposure: f32) {
    let Some(e) = world.get_entity(camera) else { return };
    if let Some(mut c) = world.query_mut::<&mut gizmo::renderer::components::Camera>().get_mut(e) {
        c.exposure = exposure;
    }
}

/// The glare threshold the city is drawn with. See [`city_glare`] for where it comes from.
pub const CITY_BLOOM_THRESHOLD: f32 = 0.18;
/// How hard what crosses [`CITY_BLOOM_THRESHOLD`] glows.
pub const CITY_BLOOM_INTENSITY: f32 = 1.2;

/// The four handles every texture upload needs, bundled so call sites stay one line.
pub struct Textures<'a> {
    /// The asset manager owning the GPU texture cache.
    pub assets: &'a mut AssetManager,
    /// The device to create textures on.
    pub device: &'a wgpu::Device,
    /// The queue to upload with.
    pub queue: &'a wgpu::Queue,
    /// The material bind-group layout to bind them into.
    pub layout: &'a wgpu::BindGroupLayout,
}

impl Textures<'_> {
    /// Upload one decoded RGBA8 image, returning its bind group (`None` if the upload failed —
    /// a texture that will not upload is skipped, never fatal).
    pub fn upload(&mut self, key: &str, rgba: &[u8], width: u32, height: u32) -> Option<Arc<wgpu::BindGroup>> {
        self.assets
            .install_decoded_material_texture(self.device, self.queue, self.layout, key, rgba, width, height)
            .ok()
    }
}

/// Spawn a built car's body meshes (material groups, then textured parts) as world entities,
/// returning their ids so the caller can move them with the chassis.
///
/// `textured` turns an uploaded bind group plus the part's suggested PBR parameters into the
/// material to draw it with, so double-sidedness stays the binary's choice.
pub fn spawn_body<F>(world: &mut World, car: &mut CarVisuals, tex: &mut Textures<'_>, textured: F) -> Vec<u32>
where
    F: Fn(Arc<wgpu::BindGroup>, [f32; 3], f32, f32) -> Material,
{
    let mut ids = Vec::new();
    for gv in std::mem::take(&mut car.groups) {
        ids.push(spawn_mesh(world, gv.mesh, gv.material, Transform::new(Vec3::ZERO)));
    }
    let lamps = std::mem::take(&mut car.headlights);
    for tp in std::mem::take(&mut car.textured) {
        // A body-detail (doorline) overlay is composited over the paint; a full-colour detail
        // (light lens, badging) uploads as-is. The key carries which, so the two never collide
        // in the texture cache.
        let rgba = match tp.composite_over {
            Some(paint) => composite_over_paint(&tp.texture.rgba, paint),
            None => tp.texture.rgba.clone(),
        };
        let key = format!("nfs_tex_{:08X}_{}", tp.texture.hash.0, u8::from(tp.composite_over.is_some()));
        let Some(bg) = tex.upload(&key, &rgba, tp.texture.width, tp.texture.height) else { continue };
        let material = textured(bg, tp.tint, tp.roughness, tp.metallic);
        ids.push(spawn_mesh(world, tp.mesh, material, Transform::new(Vec3::ZERO)));
    }
    if let Some(&body) = ids.first() {
        let intensity = world.get_resource::<Headlights>().map(|h| h.intensity);
        if let Some(intensity) = intensity {
            spawn_headlights(world, body, &lamps, intensity);
        }
    }
    ids
}

/// The headlights' beam, a spot light's cookie (`SpotLight::cookie`): registered by
/// [`enable_headlights`].
pub const HEADLIGHT_COOKIE: &str = "nfs_headlight";

/// That a binary turned the cars' headlights on ([`enable_headlights`]): [`spawn_body`] gives every car
/// its two only then, at `intensity`.
pub struct Headlights {
    /// How bright each is at the top of its beam.
    pub intensity: f32,
}

/// How bright a headlight is at the top of its beam ([`headlight_cookie`]). A lamp 0.6 m up puts on the
/// road about 1.6 at 2 m, 1.1 at 5, 0.8 at 10, 0.6 at 17 and 0.13 at 25 — a street lamp puts 2.3 under
/// itself (`world::lights::LAMP_INTENSITY`). Half what it was first set to: the user, on the frames
/// that chose [`CITY_EXPOSURE`], found the headlights too bright.
pub const HEADLIGHT_INTENSITY: f32 = 4_900.0;
/// How far a headlight reaches, metres.
pub const HEADLIGHT_REACH: f32 = 45.0;
/// The cone's half-angle, radians: wide enough for the beam's spread to the sides (the cookie's), and
/// down to the road a couple of metres ahead.
pub const HEADLIGHT_CONE: f32 = 0.65;
/// Where the beam is brightest, below level, radians: a lamp 0.6 m up meets the road there 17 m ahead.
pub const HEADLIGHT_PEAK: f32 = 0.035;
/// How fast the beam fades below its peak: the intensity goes as `(peak / angle)^this`. A lamp at
/// height `h` lights the road `d` ahead by `I · h / d³` at an angle `h / d` below level, so 3 would light
/// the near road evenly; less keeps it brighter close to the car, as a low beam does.
pub const HEADLIGHT_FALL: f32 = 2.5;

/// The headlights' beam as a cookie, `size`² RGBA (sRGB, row 0 at the top) over a cone of
/// [`HEADLIGHT_CONE`]: a low beam's. Above level nothing but a faint glow; from level the beam grows
/// to its peak ([`HEADLIGHT_PEAK`]) and fades below it ([`HEADLIGHT_FALL`]); to the sides a bell about
/// a quarter-turn wide.
///
/// A plain cone was what lit the road and nothing else wrongly: the spot's inverse square put hundreds
/// of times more light on the road at the bumper than 15 m on, and the road around the car went white.
/// And the peak sits below level, not at it: a beam whose top lit the road 40 m off evenly, as the
/// grazing angle there asks, put three thousand times that on the back of a car five metres ahead —
/// a surface square to the beam takes `I / d²`, the road `I · h / d³`. Here it is the bumper's lower
/// edge that catches the peak, and the body above the lamps the glow.
#[must_use]
pub fn headlight_cookie(size: u32) -> Vec<u8> {
    let tan_outer = HEADLIGHT_CONE.tan();
    let srgb = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        let e = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        (e * 255.0).round() as u8
    };
    const GLOW: f32 = 0.002;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for j in 0..size {
        for i in 0..size {
            let across = ((i as f32 + 0.5) / size as f32 - 0.5) * 2.0 * tan_outer;
            let up = (0.5 - (j as f32 + 0.5) / size as f32) * 2.0 * tan_outer;
            let below = -up.atan();
            let vertical = if below <= 0.0 {
                GLOW
            } else if below <= HEADLIGHT_PEAK {
                GLOW + (1.0 - GLOW) * below / HEADLIGHT_PEAK
            } else {
                (HEADLIGHT_PEAK / below).powf(HEADLIGHT_FALL)
            };
            let side = across.atan() / 0.45;
            let v = srgb(vertical * (-side * side).exp());
            rgba.extend([v, v, v, 255]);
        }
    }
    rgba
}

/// Turn the cars' headlights on at `intensity` ([`HEADLIGHT_INTENSITY`] by default): register their beam
/// ([`headlight_cookie`]) and mark the world, so that every car [`spawn_body`] spawns after this has its
/// two. Returns whether the beam registered.
pub fn enable_headlights(world: &mut World, renderer: &mut gizmo::renderer::Renderer, intensity: f32) -> bool {
    let size = 256;
    match renderer.insert_light_cookie(HEADLIGHT_COOKIE, &headlight_cookie(size), size, size) {
        Ok(_) => {
            world.insert_resource(Headlights { intensity });
            true
        }
        Err(e) => {
            eprintln!("headlights: the beam did not register ({e:?}); no headlights");
            false
        }
    }
}

/// A spot light at each of `lamps` (centre-relative, `CarVisuals::headlights`), shining level down the
/// car's forward axis through the beam ([`HEADLIGHT_COOKIE`]), as children of the body entity `body`:
/// they move with it, which the rig places every frame. They are not `in_bake` — no bake holds a car's
/// lights — so the baked city is lit by them.
pub fn spawn_headlights(world: &mut World, body: u32, lamps: &[Vec3], intensity: f32) {
    let Some(parent) = world.get_entity(body) else { return };
    let mut children = Vec::new();
    for &at in lamps {
        let e = world.spawn();
        world.add_component(e, Transform::new(at));
        world.add_component(
            e,
            gizmo::renderer::components::SpotLight::new(
                Vec3::new(1.0, 0.93, 0.82),
                intensity,
                HEADLIGHT_REACH,
                HEADLIGHT_CONE - 0.05,
                HEADLIGHT_CONE,
            )
            .with_cookie(HEADLIGHT_COOKIE),
        );
        world.add_component(e, gizmo::core::component::Parent(parent));
        children.push(e);
    }
    if !children.is_empty() {
        world.add_component(parent, gizmo::core::component::Children(children));
    }
}

/// Spawn a mesh with distance-chosen detail levels: `levels` is `(mesh, max_distance)`, finest
/// first. The first level is also the entity's own mesh, which the `LodGroup` overrides.
/// Whether the city's own meshes cast the sun's shadow: no, unless `NFS_CITY_SHADOWS=on`.
///
/// Bayview is a night map with its light baked into vertex colours, and the sun is there for the
/// car alone — but the engine's baked-lit path darkens a surface by up to 45 % where that sun is
/// shadowed, within [`SHADOW_DISTANCE`](gizmo::renderer::SHADOW_DISTANCE) (100 m) of the camera,
/// fading out at its edge. So every pole, sign and building near the camera threw a daylight
/// shadow onto the night street, and that band moved with the camera: lights that seemed to switch
/// as the car drove, signs whose faces dimmed and came back. The Bevy port of the same city, drawn
/// unlit with no sun, shows none of it. The car still casts, so it still sits on the road.
#[must_use]
pub fn city_shadows() -> ShadowCasting {
    if std::env::var("NFS_CITY_SHADOWS").as_deref() == Ok("on") { ShadowCasting::On } else { ShadowCasting::Off }
}

/// One mesh of the city: [`spawn_mesh`] with the city's shadow setting, [`city_shadows`].
pub fn spawn_city(world: &mut World, mesh: Mesh, material: Material, t: Transform) -> u32 {
    let e = world.spawn();
    add_transform(world, e, t);
    world.add_component(e, mesh);
    world.add_component(e, material);
    world.add_component(e, MeshRenderer::new().with_shadows(city_shadows()));
    e.id()
}

/// A city mesh with its LOD levels, under the city's shadow setting ([`city_shadows`]).
pub fn spawn_lod(world: &mut World, levels: &[(Mesh, f32)], material: Material, t: Transform) -> u32 {
    let e = world.spawn();
    add_transform(world, e, t);
    world.add_component(e, levels[0].0.clone());
    if levels.len() > 1 {
        let lods = levels
            .iter()
            .map(|(m, d)| gizmo::renderer::LodLevel::new(m.clone(), *d))
            .collect();
        world.add_component(e, gizmo::renderer::LodGroup::new(lods));
    }
    world.add_component(e, material);
    world.add_component(e, MeshRenderer::new().with_shadows(city_shadows()));
    e.id()
}

/// Spawn one mesh + material as an entity at `t`, returning its id.
pub fn spawn_mesh(world: &mut World, mesh: Mesh, material: Material, t: Transform) -> u32 {
    let e = world.spawn();
    add_transform(world, e, t);
    world.add_component(e, mesh);
    world.add_component(e, material);
    world.add_component(e, MeshRenderer::new());
    e.id()
}

/// Where the four wheels go, in chassis space.
///
/// Prefers the exact per-car mounts from `GLOBALB`'s [`CarTypeInfo`] (asymmetric wheelbase,
/// per-car track and ride height); falls back to the symmetric corners derived from the wheel
/// part's own bounds when the game bundle is not reachable.
#[must_use]
pub fn wheel_mounts(cti: Option<&CarTypeInfo>, fit: WheelFit, center: Vec3, body_height: f32) -> Vec<Vec3> {
    // **The record's lateral is the wheel's outer face, not its middle.** A stock 240SX's record puts
    // its wheels at ±0.821 m (front) and ±0.801 m (rear); its body is ±0.815 m wide at the axles,
    // and its wheel is 0.20 m across — so the game's wheels stand flush with the wings. Centred on the
    // mount, half of every tyre stood 0.1 m out of the body. A wheel *drawn* there goes in by half
    // its width, and the brakes, placed at the same mounts, with it. The suspension stays on the
    // record's own point (`wheel_record_mounts`): this is where the eye sees the wheel, not where
    // the car stands on it.
    let inset = if cti.is_some() { fit.half_width } else { 0.0 };
    wheel_record_mounts(cti, fit, center, body_height)
        .into_iter()
        .map(|m| m - Vec3::X * m.x.signum() * inset)
        .collect()
}

/// The four wheel points as the record states them — the suspension's mounts. [`wheel_mounts`] is
/// where the wheels are drawn.
#[must_use]
pub fn wheel_record_mounts(cti: Option<&CarTypeInfo>, fit: WheelFit, center: Vec3, body_height: f32) -> Vec<Vec3> {
    match cti {
        Some(c) => c.wheels.iter().map(|w| crate::car::wheel_mount(w, center)).collect(),
        None => {
            let y = -body_height * 0.5 + fit.radius * 0.95;
            [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .iter()
                .map(|(sx, sz)| Vec3::new(sx * fit.half_track, y, sz * fit.half_wheelbase))
                .collect()
        }
    }
}

/// The wheel radius to use: the record's own, else the one guessed from the wheel part's bounds.
///
/// `WheelSpec::radius` was never read — `fit_wheel` derived a radius from the mesh's bounding box
/// and clamped it to `0.18..0.55`, which is a guess with a guard on it where the file states the
/// number. The four mounts can differ (a staggered fitment does), so this takes the front pair's,
/// which is what sizes the visual and the suspension.
#[must_use]
pub fn wheel_radius(cti: Option<&CarTypeInfo>, fit: WheelFit) -> f32 {
    cti.map_or(fit.radius, |c| c.wheels[0].radius)
}

/// The rotation that turns a wheel instance to face its rim outward.
///
/// The wheel mesh is modelled for one side only; on the other flank it would show its flat
/// inboard back as a black slab. A 180° yaw mirrors it — a *reflection* would too, but would
/// invert the winding and the normals with it.
#[must_use]
pub fn wheel_mirror(mount: Vec3) -> Quat {
    if mount.x < 0.0 {
        Quat::from_rotation_y(std::f32::consts::PI)
    } else {
        Quat::IDENTITY
    }
}

/// Resolve the car's [`WheelSurface`] into one material shared by all four instances: the tyre
/// texture when the car ships one, else the flat rubber the assembly already built.
///
/// `flat` builds that rubber look, and is also the fallback when the tyre texture will not
/// upload — a white unlit wheel would be a worse failure than an untextured black one.
pub fn wheel_material<F, G>(surface: WheelSurface, tex: &mut Textures<'_>, textured: F, flat: G) -> Material
where
    F: Fn(Arc<wgpu::BindGroup>) -> Material,
    G: Fn(crate::car::PbrLook) -> Material,
{
    match surface {
        WheelSurface::Flat(m) => m,
        WheelSurface::Textured(t) => {
            let key = format!("nfs_wheel_{:08X}", t.hash.0);
            match tex.upload(&key, &t.rgba, t.width, t.height) {
                Some(bg) => textured(bg),
                None => flat(crate::car::wheel_look()),
            }
        }
    }
}

/// The two-light rig every demo uses: a warm key sun (with its own orientation) and a cool fill.
pub fn add_lights(world: &mut World, key: Transform, key_intensity: f32, fill_pos: Vec3) {
    let sun = world.spawn();
    add_transform(world, sun, key);
    world.add_component(sun, DirectionalLight::new(Vec3::new(1.0, 0.97, 0.9), key_intensity, LightRole::Sun));
    let fill = world.spawn();
    add_transform(world, fill, Transform::new(fill_pos));
    // In the bake: it lights the car, not the baked city (which holds its own light).
    world.add_component(fill, DirectionalLight::new(Vec3::new(0.6, 0.7, 0.9), 0.6, LightRole::Sun).in_bake());
}

/// The car model to load: the first CLI argument, else `$NFSU2_CAR`, else `default`.
#[must_use]
pub fn car_path(default: &str) -> String {
    std::env::args()
        .nth(1)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .unwrap_or_else(|| default.to_string())
}

/// A collider the cars drive on, made of what roads are made of.
///
/// **The wheel reads the collider's own material, not the `PhysicsMaterial` component.** The
/// engine's suspension raycast takes `Collider::material.dynamic_friction` from the collider it
/// hits and scales every tyre force by it over asphalt's 0.65. Every ground collider in this game
/// was built with the default material — friction 0.5 — and given a `PhysicsMaterial::ASPHALT`
/// component beside it that the wheel never looks at, so every tyre held 0.5 / 0.65 = **77 %** of
/// its grip, on every road and every test plane. Measured on the bench (`nfs_bench`): the wheels
/// reported a surface friction of 0.5 with the asphalt component in place.
#[must_use]
pub fn road(collider: Collider) -> Collider {
    collider.with_material(gizmo::physics::components::PhysicsMaterial::ASPHALT)
}

#[cfg(test)]
mod headlight_beam_tests {
    use super::*;

    /// **The beam is a low beam's**: down the middle column, faint above level, brightest at its peak
    /// below level and fading steadily under it; dimmer to the sides than in the middle.
    #[test]
    fn the_beam_is_brightest_just_below_level_and_fades_down_and_out() {
        let size = 256u32;
        let beam = headlight_cookie(size);
        assert_eq!(beam.len(), (size * size * 4) as usize);
        let at = |i: u32, j: u32| beam[((j * size + i) * 4) as usize];
        // The row an angle `below` level falls in.
        let row = |below: f32| (((0.5 + below.tan() / (2.0 * HEADLIGHT_CONE.tan())) * size as f32) as u32).min(size - 1);
        let middle = size / 2;
        assert!(at(middle, row(HEADLIGHT_PEAK)) >= 240, "full at the peak: {}", at(middle, row(HEADLIGHT_PEAK)));
        assert!(at(middle, row(-0.1)) < 10, "a glow above level: {}", at(middle, row(-0.1)));
        let mut last = 255u8;
        for k in 1..=10 {
            let v = at(middle, row(HEADLIGHT_PEAK * (1.0 + k as f32 * 0.6)));
            assert!(v <= last && v > 0, "fading under the peak, never to nothing: {v} after {last}");
            last = v;
        }
        assert!(last < 120, "well down by eight times the peak's angle: {last}");
        assert!(u32::from(at(middle / 4, row(HEADLIGHT_PEAK))) * 4 < u32::from(at(middle, row(HEADLIGHT_PEAK))) * 3, "dimmer to the side");
    }
}
