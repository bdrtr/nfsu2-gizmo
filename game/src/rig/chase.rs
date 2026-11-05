//! The camera that follows a car.
//!
//! Three behaviours over one piece of state, because they are three answers to the same question
//! and switching between them mid-drive must not jump:
//!
//! * **Chase** — trailing the car's own forward at a fixed distance, lerped toward rather than
//!   snapped to, so a corner swings the camera around instead of teleporting it.
//! * **Orbit** — while the right mouse button is held, a fixed-radius look from wherever the mouse
//!   has dragged the view to. This is the one that inspects the car.
//! * **Shot** (`NFS_SHOTCAM`) — a fixed offset **in the car's own frame**, for screenshots and for
//!   watching the suspension work. `nfs_drive` used to place this one in *world* space, which
//!   framed a front three-quarter view only until the car turned; after that it was whichever side
//!   happened to face north.
//!
//! **Speed is felt through the chase camera.** It used to see the same at 20 km/h and at 200: a
//! fixed 45° view, a fixed seat. Two things now move with the car's speed, in chase mode only (the
//! orbit inspects the car and the shot frames it, and both want to hold still):
//!
//! * the vertical field of view opens from [`FOV_REST`] to [`FOV_FAST`] — the road ahead stretches
//!   and the edges of the screen run past faster, which is most of what "fast" looks like;
//! * past [`SHAKE_FROM`] the view trembles by a fraction of a degree, a few pixels, the way a
//!   camera bolted to something fast does.
//!
//! **No pull-back with speed** (2026-11-03). The chase used to *lerp* toward its seat, so at speed
//! `v` it settled `v / lag` metres further back — 2 m at 90 km/h, 4 m at 180 — and a camera that
//! drifts away from the car makes the world beside it crawl: the player's verdict was "fast does
//! not look fast". Now the seat is held at [`ChaseCamera::distance`] at every speed, and what moves
//! is:
//!
//! * the camera's **heading**, which follows the car's nose at [`ChaseCamera::lag`] per second, so a
//!   corner or a slide swings the car across the screen and shows its flank before the view catches
//!   up;
//! * a **pull** of at most [`PULL_MAX`] metres under acceleration (and a little push under braking),
//!   from the change in speed rather than the speed — it is the shove, not the cruise, that the
//!   eye reads;
//! * the **aim**, [`LOOK_AHEAD`] metres in front of the car rather than at it, which puts the car low
//!   on the screen and the road ahead in the middle of it.
//!
//! Those numbers were not the game's, and they now stand only behind `NFS_CHASE=old`.
//!
//! **The game's own chase** (2026-11-05, the default; [`GameFollow`]). `SPEED2.EXE` reads each car's
//! seats from its record (`+0x720`, [`gizmo_nfs::CameraView`]), and `0x453570`/`0x44DE90` hold the
//! camera in a frame that follows the car:
//!
//! * the frame's heading goes to the nose below 8 m/s and to the direction of **travel** above 13,
//!   so a slide shows the car's flank (`0x445C20`, table `0x7EC420`; `0x5ECDB0` turns the same speed
//!   field to mph with 2.237, which is what says it is m/s);
//! * in that frame the seat is fixed, so speed alone moves nothing. Forward acceleration, averaged
//!   over four samples, slides the seat and the aim along the nose (`0x7EC610`'s curves, read at
//!   their midpoint as `0x787154` asks);
//! * the field of view does not change with speed. 71.52° (`0x7EB5B4`) is horizontal: the projection
//!   (`0x5C7EC0`) scales the half-angle by height/width, so 4:3 gives 53.64° vertically, which is
//!   what is kept here at any window shape.
//!
//! Not carried over: the horizon's lean into acceleration, since the engine's camera has no roll;
//! the heading's level-out in the air, because [`super::Pose`] does not say whether the car is
//! flying; and `0x44DE90`'s ×1.3 seat in a state not identified yet.
//!
//! **The shake is the game's too** ([`game_shake`], `0x452ED0`/`0x441340`), and far smaller than the
//! tremble above, which stands only behind `NFS_CHASE=old`. Fractal noise in pitch and yaw, growing
//! with speed; what shakes the view hard is nitro. Not carried over: its 3 mm sideways offsets, its
//! second channel, which only a rough surface drives (`0x800EFC`; this road has no surface kinds),
//! and the car's own top speed in the speed scale, of which the code's fallback of 100 is used.

use gizmo::prelude::*;

/// The game's field of view for its chase views, vertical, radians: see the module notes.
pub const GAME_FOV: f32 = GAME_HFOV * 0.75;
/// The same before the 4:3: what `0x7EB5B4` holds, horizontal.
const GAME_HFOV: f32 = 71.52 * std::f32::consts::PI / 180.0;

/// One seat from a car's record, in the car's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChaseView {
    /// The view's number in the record.
    pub pov: i16,
    /// Metres along the nose; negative is behind.
    pub forward: f32,
    /// Metres above the car's origin.
    pub up: f32,
    /// The tilt, degrees. The aim is at `up + forward · tan(pitch)` over the car's origin.
    pub pitch_deg: f32,
}

/// What a car without a record gets: the 240SX's chase views, which most of the 46 share.
pub const DEFAULT_VIEWS: [ChaseView; 3] = [
    ChaseView { pov: 0, forward: -5.8, up: 1.7, pitch_deg: 0.0 },
    ChaseView { pov: 1, forward: -4.7, up: 1.7, pitch_deg: 0.0 },
    ChaseView { pov: 5, forward: -5.8, up: 3.45, pitch_deg: 16.7 },
];

/// The chase views in a car's record, in the record's order; none if it has no record.
#[must_use]
pub fn views_of(info: Option<&gizmo_nfs::CarTypeInfo>) -> Vec<ChaseView> {
    info.map_or_else(Vec::new, |i| {
        i.cameras
            .iter()
            .filter(|c| matches!(c.pov, 0 | 1 | 5))
            .map(|c| ChaseView { pov: c.pov, forward: c.forward, up: c.up, pitch_deg: c.pitch_deg })
            .collect()
    })
}

/// One view's follow settings: `0x7EC610 + 20 · pov`, half way between its two rows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FollowTuning {
    /// Seconds the seat, the aim and each axis of the heading take to arrive.
    eye_time: f32,
    look_time: f32,
    heading_time: [f32; 3],
    /// Metres of seat per m/s² of forward acceleration, and the seat's limits.
    eye_gain: f32,
    eye_limits: (f32, f32),
    /// The same for the aim.
    look_gain: f32,
    look_limits: (f32, f32),
}

/// `pov` 5's rows are byte-for-byte `pov` 0's.
fn follow_tuning(pov: i16) -> FollowTuning {
    if pov == 1 {
        FollowTuning {
            eye_time: 1.0,
            look_time: 0.35,
            heading_time: [0.3, 0.3, 0.5],
            eye_gain: -0.125,
            eye_limits: (-1.25, 1.0),
            look_gain: -0.06,
            look_limits: (-1.25, 1.25),
        }
    } else {
        FollowTuning {
            eye_time: 1.0,
            look_time: 0.35,
            heading_time: [0.3, 0.3, 0.55],
            eye_gain: -0.15,
            eye_limits: (-1.5, 1.25),
            look_gain: -0.06,
            look_limits: (-1.25, 1.25),
        }
    }
}

/// Speed where the heading starts leaving the nose and where it has become the direction of travel.
const TRAVEL_FROM: f32 = 8.0;
const TRAVEL_FULL: f32 = 13.0;

/// `0x5749E0`: a cubic from where a value is, at its rate, to a target at rest, restarted every step.
///
/// Restarted, it is a damped spring with `ω = √6 / T` and `ζ = 2 / √6`, so `T` is a time constant
/// rather than an arrival time. `rate` is per `T`, as the game keeps it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Ease {
    value: f32,
    rate: f32,
}

impl Ease {
    fn at(value: f32) -> Self {
        Self { value, rate: 0.0 }
    }

    fn step(&mut self, target: f32, dt: f32, time: f32) {
        let t = dt / time.max(1e-3);
        let gap = target - self.value;
        let a = self.rate - 2.0 * gap;
        let b = 3.0 * gap - 2.0 * self.rate;
        self.value += ((t * a + b) * t + self.rate) * t;
        self.rate += (2.0 * b + 3.0 * t * a) * t;
    }
}

/// The game's chase, free of the engine: a car's motion in, a seat and an aim out.
#[derive(Clone, Copy, Debug)]
struct GameFollow {
    heading: [Ease; 3],
    eye: Ease,
    look: Ease,
    /// Where the car was when it last moved, and the seconds since. The physics steps at a fixed
    /// rate and the frames do not, so a frame can see no motion at all; that is not a stopped car.
    last: Option<Vec3>,
    since: f32,
    velocity: Vec3,
    /// The last four accelerations (`0x446F10`), and the next slot.
    accel: [Vec3; 4],
    slot: usize,
    filled: usize,
}

impl GameFollow {
    fn new() -> Self {
        Self {
            heading: [Ease::default(); 3],
            eye: Ease::default(),
            look: Ease::default(),
            last: None,
            since: 0.0,
            velocity: Vec3::ZERO,
            accel: [Vec3::ZERO; 4],
            slot: 0,
            filled: 0,
        }
    }

    /// Stand at the seat with nothing in motion: the first frame, a reset, a change of view.
    fn cut(&mut self, view: &ChaseView, position: Vec3, nose: Vec3) {
        *self = Self::new();
        self.heading = [Ease::at(nose.x), Ease::at(nose.y), Ease::at(nose.z)];
        self.eye = Ease::at(view.forward);
        self.look = Ease::at(0.0);
        self.last = Some(position);
    }

    /// Where the heading is going: the nose, or the direction of travel with a reversing car's
    /// reflected so the camera stays behind the nose (`0x445C20`).
    fn heading_target(velocity: Vec3, nose: Vec3) -> Vec3 {
        let speed = velocity.length();
        let along = velocity.dot(nose);
        let travel = if along < 0.0 { velocity - nose * (2.0 * along) } else { velocity };
        let travel = travel.normalize_or(nose);
        let f = ((speed - TRAVEL_FROM) / (TRAVEL_FULL - TRAVEL_FROM)).clamp(0.0, 1.0);
        travel * f + nose * (1.0 - f)
    }

    /// One frame: the seat and the point it looks at, world space.
    fn advance(&mut self, view: &ChaseView, position: Vec3, rotation: Quat, dt: f32) -> (Vec3, Vec3) {
        let nose = (rotation * Vec3::NEG_Z).normalize_or(Vec3::NEG_Z);
        if self.last.is_none_or(|at| at.distance(position) > CUT_DISTANCE) {
            self.cut(view, position, nose);
        }
        self.since += dt;
        if let Some(at) = self.last.filter(|at| *at != position) {
            let velocity = (position - at) / self.since.max(1e-4);
            self.accel[self.slot] = (velocity - self.velocity) / self.since.max(1e-4);
            self.slot = (self.slot + 1) % self.accel.len();
            self.filled = (self.filled + 1).min(self.accel.len());
            self.velocity = velocity;
            self.last = Some(position);
            self.since = 0.0;
        }
        let tuning = follow_tuning(view.pov);
        let want = Self::heading_target(self.velocity, nose);
        for (axis, (ease, time)) in self.heading.iter_mut().zip(tuning.heading_time).enumerate() {
            ease.step(want[axis], dt, time);
        }
        let forward = Vec3::new(self.heading[0].value, self.heading[1].value, self.heading[2].value)
            .normalize_or(nose);
        let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = right.cross(forward);

        let shove = self.accel[..self.filled].iter().copied().sum::<Vec3>().dot(forward)
            / self.filled.max(1) as f32;
        let (lo, hi) = tuning.eye_limits;
        self.eye.step(view.forward + (shove * tuning.eye_gain).clamp(lo, hi), dt, tuning.eye_time);
        let (lo, hi) = tuning.look_limits;
        self.look.step((shove * tuning.look_gain).clamp(lo, hi), dt, tuning.look_time);

        let aim_up = view.up + view.forward * view.pitch_deg.to_radians().tan();
        (
            position + forward * self.eye.value + up * view.up,
            position + forward * self.look.value + up * aim_up,
        )
    }
}

/// `0x4411B0`: value noise in one dimension, −1..1, from the integer hash the game uses.
fn noise(x: f32) -> f32 {
    let hash = |n: u32| {
        let n = (n << 13) ^ n;
        let h = n
            .wrapping_mul(n)
            .wrapping_mul(0x3D73)
            .wrapping_add(0xC_0AE5)
            .wrapping_mul(n)
            .wrapping_add(0xD208_DD0D)
            & 0x7FFF_FFFF;
        1.0 - h as f32 / 1_073_741_824.0
    };
    let whole = x as i32;
    let f = x - whole as f32;
    hash((whole as u32).wrapping_add(1)) * f + (1.0 - f) * hash(whole as u32)
}

/// `0x441270`: six octaves of [`noise`], each twice the frequency and half the weight.
fn fractal(x: f32) -> f32 {
    (0..6).map(|o| noise(x * (1u32 << o) as f32) / (1u32 << o) as f32).sum()
}

/// `0x7EC430`: amplitude and frequency against speed, five rows over speed / top speed.
const SHAKE_ROWS: [(f32, f32); 5] = [(0.0, 1.0), (0.1, 1.2), (0.3, 1.5), (0.6, 2.0), (1.0, 3.0)];
/// The fallback in `0x452ED0` for the car's top speed, m/s.
const SHAKE_TOP_SPEED: f32 = 100.0;
/// What the game scales the amplitude by outside its two debug modes (`0x7870B8`).
const SHAKE_SCALE: f32 = 0.2;
/// Added to the amplitude and multiplied into the frequency while nitro burns (`0x7870B0`, `0x7870B4`).
const SHAKE_NITRO_AMP: f32 = 0.2;
const SHAKE_NITRO_FREQ: f32 = 1.4;
/// Pitch and yaw: frequency and degrees of amplitude (`0x7EC488`, `0x7EC498`).
const SHAKE_PITCH: (f32, f32) = (6.125, 0.7);
const SHAKE_YAW: (f32, f32) = (5.0, 0.5);
/// The field of view the amplitude is written for (`0x7EC1F0`), radians: 60°.
const SHAKE_REF_FOV: f32 = std::f32::consts::FRAC_PI_3;

/// This frame's shake as (yaw, pitch), radians, for a car at `speed` m/s `t` seconds in, seen
/// through a horizontal field of view of `hfov` radians.
#[must_use]
pub fn game_shake(speed: f32, nitro: bool, t: f32, hfov: f32) -> (f32, f32) {
    let at = (speed.abs() / SHAKE_TOP_SPEED * 4.0).clamp(0.0, 4.0);
    let row = (at as usize).min(3);
    let f = at - row as f32;
    let (a0, b0) = SHAKE_ROWS[row];
    let (a1, b1) = SHAKE_ROWS[row + 1];
    let mut amp = (a0 + (a1 - a0) * f) * SHAKE_SCALE;
    let mut freq = b0 + (b1 - b0) * f;
    // `0x5ECDB0`'s other half: not below 5 mph.
    if nitro && speed.abs() * 2.237 >= 5.0 {
        amp += SHAKE_NITRO_AMP;
        freq *= SHAKE_NITRO_FREQ;
    }
    // `0x4412E0`: an angle is widened with the view, so it covers the same part of the screen.
    let widen = (hfov * 0.5).sin() / (SHAKE_REF_FOV * 0.5).sin();
    let angle = |(rate, degrees): (f32, f32)| {
        let a = fractal(rate * freq * t) * degrees * amp;
        (a.to_radians().sin() * widen).clamp(-1.0, 1.0).asin()
    };
    (angle(SHAKE_YAW), angle(SHAKE_PITCH))
}

/// How high above the car's origin the chase camera aims.
const CHASE_LOOK_Y: f32 = 0.9;
/// How far in front of the car the chase aims, metres.
pub const LOOK_AHEAD: f32 = 4.0;
/// The most the camera is pushed back by acceleration, metres; braking pulls it in by up to half.
pub const PULL_MAX: f32 = 0.8;
/// Metres of pull per m/s² of acceleration.
const PULL_PER_MPS2: f32 = 0.12;
/// How fast the pull follows the acceleration, per second.
const PULL_RATE: f32 = 3.0;
/// How fast the camera's height follows the car's, per second — kerbs and bumps are softened, a
/// hill is not.
const HEIGHT_RATE: f32 = 10.0;
/// A jump in the car's position bigger than this in one frame is a reset, not driving: the camera
/// cuts to its seat instead of swinging there.
const CUT_DISTANCE: f32 = 15.0;
/// The same, for the shot camera — lower, because it is framing the car rather than the road ahead.
const SHOT_LOOK_Y: f32 = 0.5;
/// Where the orbit pivot sits relative to the car's origin, and how far the camera stands off it.
const ORBIT_LOOK_Y: f32 = 1.2;
const ORBIT_DISTANCE: f32 = 9.0;
/// Radians of yaw/pitch per pixel of mouse movement.
const MOUSE_SENSITIVITY: f32 = 0.005;

/// The vertical field of view standing still: 45°, what every binary had before.
pub const FOV_REST: f32 = std::f32::consts::FRAC_PI_4;
/// …and at [`FOV_SPEED`] and above: 72°. (It was 62° at 252 km/h, which at 150 km/h was 54° —
/// too little to read.)
pub const FOV_FAST: f32 = 72.0 * std::f32::consts::PI / 180.0;
/// Below this the view does not widen at all (m/s, 29 km/h): town speed should look like town.
pub const FOV_FROM: f32 = 8.0;
/// Where the widening is complete (m/s, 216 km/h); 150 km/h is 66°.
pub const FOV_SPEED: f32 = 60.0;
/// How fast the view follows its target, per second. Slow enough that a hard brake *closes* the
/// view over a second rather than snapping it, which is half of what a brake feels like.
const FOV_RATE: f32 = 2.5;
/// Speed the tremble starts at (m/s, 90 km/h), and its amplitude at [`FOV_SPEED`], radians.
pub const SHAKE_FROM: f32 = 25.0;
const SHAKE_MAX: f32 = 0.005;

/// The field of view the chase is heading for at `speed` (m/s, either sign).
///
/// Smoothstep between [`FOV_FROM`] and [`FOV_SPEED`]: no kink where it starts, none where it ends.
#[must_use]
pub fn speed_fov(speed: f32) -> f32 {
    let f = ((speed.abs() - FOV_FROM) / (FOV_SPEED - FOV_FROM)).clamp(0.0, 1.0);
    FOV_REST + (FOV_FAST - FOV_REST) * f * f * (3.0 - 2.0 * f)
}

/// How hard the view trembles at `speed`, radians of amplitude; zero below [`SHAKE_FROM`].
#[must_use]
pub fn shake_amplitude(speed: f32) -> f32 {
    let f = ((speed.abs() - SHAKE_FROM) / (FOV_SPEED - SHAKE_FROM)).clamp(0.0, 1.0);
    SHAKE_MAX * f
}

/// The tremble at time `t`, as (yaw, pitch) in units of the amplitude. Three sines at frequencies
/// that never line up, per axis — a sum of incommensurate sines never repeats visibly, and unlike
/// random noise it has no frame-to-frame jumps to alias against the frame rate.
fn tremble(t: f32) -> (f32, f32) {
    let yaw = 0.5 * (t * 71.0).sin() + 0.3 * (t * 113.0 + 1.3).sin() + 0.2 * (t * 47.0 + 2.1).sin();
    let pitch = 0.5 * (t * 83.0 + 0.7).sin() + 0.3 * (t * 131.0 + 2.9).sin() + 0.2 * (t * 59.0).sin();
    (yaw, pitch)
}

/// A camera entity plus the state needed to move it smoothly behind a car.
pub struct ChaseCamera {
    /// The camera entity.
    pub id: u32,
    /// Where the camera is this frame. Carried rather than recomputed because the chase mode lerps
    /// toward its target and so depends on where it was.
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// How far behind the car the chase sits, in metres.
    pub distance: f32,
    /// How far above it.
    pub height: f32,
    /// How fast the camera's heading follows the car's nose, per second. Higher is tighter; lower
    /// swings wider through corners and shows more of a slide.
    pub lag: f32,
    /// The `NFS_SHOTCAM` offset, in the car's own frame. `None` unless the variable is set.
    pub shot: Option<Vec3>,
    /// The vertical field of view this frame, closing on [`speed_fov`] at [`FOV_RATE`].
    pub fov: f32,
    /// Seconds the camera has run, for the tremble.
    clock: f32,
    /// This frame's tremble, (yaw, pitch) radians, added on write and never fed back into the aim.
    shake: (f32, f32),
    /// The direction the chase looks along, in plan; zero until the first chase frame.
    heading: Vec3,
    /// The acceleration pull this frame, metres (negative under braking).
    pull: f32,
    /// The car's speed and position last frame, for the pull and for spotting a reset.
    last: Option<(f32, Vec3)>,
    /// The camera's height, smoothed.
    seat_y: f32,
    /// `NFS_CHASE=old`: the chase this file had before the game's was read.
    old_chase: bool,
    /// Whether nitro is burning this frame ([`game_shake`]); the caller's to set.
    pub nitro: bool,
    /// The seats on offer, and which one is in use. C goes to the next.
    views: Vec<ChaseView>,
    view: usize,
    follow: GameFollow,
}

impl ChaseCamera {
    /// Spawn the camera entity looking down −X at `at`, with the trailing defaults.
    ///
    /// **Both planes are the caller's**, because what costs depth precision is their *ratio* and
    /// only the caller knows how big its world is. A car on a plane wants `0.1 / 2_000`; a car in
    /// Bayview wants `0.5 / 20_000`, which is 40,000:1 rather than 200,000:1 — at street level the
    /// tighter ratio is the difference between a city and a field of z-fighting. Moving the near
    /// plane out has its own cost (it clips whatever the camera is standing next to), which is why
    /// the small-world binaries do not pay it.
    pub fn spawn(world: &mut World, at: Vec3, near: f32, far: f32) -> Self {
        let (yaw, pitch) = (-std::f32::consts::FRAC_PI_2, -0.3);
        let camera = world.spawn();
        crate::geom::add_transform(world, camera, Transform::new(at));
        world.add_component(camera, Camera::new(std::f32::consts::FRAC_PI_4, near, far, yaw, pitch, true));
        // `NFS_CAM="uzaklık,yükseklik,dönüş"`: kovalamanın oturduğu yer, deneme için.
        let cam: Vec<f32> = std::env::var("NFS_CAM")
            .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
            .unwrap_or_default();
        let (distance, height, lag) = match cam[..] {
            [d, h, l] => (d, h, l),
            _ => (5.2, 1.6, 6.0),
        };
        let old_chase = std::env::var("NFS_CHASE").is_ok_and(|v| v == "old");
        Self {
            id: camera.id(),
            position: at,
            yaw,
            pitch,
            distance,
            height,
            lag,
            // Empty reads as unset: a knob cleared with `NFS_SHOTCAM=` should be off, not on.
            shot: std::env::var("NFS_SHOTCAM")
                .is_ok_and(|v| !v.is_empty())
                .then_some(Vec3::new(4.2, 1.4, -5.6)),
            fov: if old_chase { FOV_REST } else { GAME_FOV },
            clock: 0.0,
            shake: (0.0, 0.0),
            heading: Vec3::ZERO,
            pull: 0.0,
            last: None,
            seat_y: at.y,
            old_chase,
            nitro: false,
            views: DEFAULT_VIEWS.to_vec(),
            view: 0,
            follow: GameFollow::new(),
        }
    }

    /// Take the car's own seats (`CarRig::cameras`), starting on `NFS_POV` if that names one of them
    /// and on the record's first otherwise. Empty leaves the defaults. `NFS_CAM` still overrides the
    /// seat, for trying one out.
    pub fn set_views(&mut self, views: &[ChaseView]) {
        if !views.is_empty() {
            self.views = views.to_vec();
        }
        if std::env::var("NFS_CAM").is_ok() {
            for v in &mut self.views {
                v.forward = -self.distance;
                v.up = self.height;
                v.pitch_deg = 0.0;
            }
        }
        let pov = std::env::var("NFS_POV").ok().and_then(|v| v.parse::<i16>().ok());
        self.view = pov.and_then(|p| self.views.iter().position(|v| v.pov == p)).unwrap_or(0);
        self.follow = GameFollow::new();
    }

    /// Set how the chase mode trails the car.
    #[must_use]
    pub fn trailing(mut self, distance: f32, height: f32, lag: f32) -> Self {
        self.distance = distance;
        self.height = height;
        self.lag = lag;
        self
    }

    /// Move the camera for this frame and write it to its entity.
    pub fn update(&mut self, world: &mut World, input: &Input, pose: super::Pose, dt: f32) {
        let orbiting = input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT);
        let chasing = self.shot.is_none() && !orbiting;
        self.clock += dt;
        if chasing && input.is_key_just_pressed(KeyCode::KeyC as u32) {
            self.view = (self.view + 1) % self.views.len();
            self.follow = GameFollow::new();
        }
        let (want, amp) = match (chasing, self.old_chase) {
            (true, true) => (speed_fov(pose.speed), shake_amplitude(pose.speed)),
            (true, false) => (GAME_FOV, 0.0),
            (false, _) => (FOV_REST, 0.0),
        };
        self.fov += (want - self.fov) * (1.0 - (-FOV_RATE * dt).exp());
        let (ty, tp) = tremble(self.clock);
        self.shake = if chasing && !self.old_chase {
            game_shake(pose.speed, self.nitro, self.clock, GAME_HFOV)
        } else {
            (ty * amp, tp * amp)
        };
        match self.shot {
            // A fixed seat in the car's frame; the aim follows from where that lands.
            Some(offset) => {
                self.position = pose.position + pose.rotation * offset;
                self.aim_at(pose.position + Vec3::Y * SHOT_LOOK_Y);
            }
            None if orbiting => {
                // Stand off along whatever direction the mouse has dragged the view to, so dragging
                // swings the camera around the car rather than turning it on the spot.
                let forward = Camera::forward_from(self.yaw, self.pitch);
                self.position = pose.position + Vec3::Y * ORBIT_LOOK_Y - forward * ORBIT_DISTANCE;
            }
            None if !self.old_chase => {
                let view = self.views[self.view];
                let (seat, aim) = self.follow.advance(&view, pose.position, pose.rotation, dt);
                self.position = seat;
                self.aim_at(aim);
            }
            None => {
                let nose = pose.rotation * Vec3::NEG_Z;
                let nose = Vec3::new(nose.x, 0.0, nose.z).normalize_or(Vec3::NEG_Z);
                let cut = self.heading == Vec3::ZERO
                    || self.last.is_none_or(|(_, at)| at.distance(pose.position) > CUT_DISTANCE);
                // Frame-rate independent smoothing: a plain `lerp(k)` with a constant `k` closes
                // faster the more frames there are, so the camera would tighten with the frame rate.
                let k = |rate: f32| 1.0 - (-rate * dt).exp();
                if cut {
                    self.heading = nose;
                    self.pull = 0.0;
                    self.seat_y = pose.position.y + self.height;
                } else {
                    self.heading = self.heading.lerp(nose, k(self.lag)).normalize_or(nose);
                    let accel = self.last.map_or(0.0, |(v, _)| (pose.speed - v) / dt.max(1e-4));
                    let want = (accel * PULL_PER_MPS2).clamp(-0.5 * PULL_MAX, PULL_MAX);
                    self.pull += (want - self.pull) * k(PULL_RATE);
                    self.seat_y += (pose.position.y + self.height - self.seat_y) * k(HEIGHT_RATE);
                }
                self.last = Some((pose.speed, pose.position));
                let behind = pose.position - self.heading * (self.distance + self.pull);
                self.position = Vec3::new(behind.x, self.seat_y, behind.z);
                self.aim_at(pose.position + self.heading * LOOK_AHEAD + Vec3::Y * CHASE_LOOK_Y);
            }
        }
        self.mouse_look(input);
        self.write(world);
    }

    /// Point the camera at a world point.
    fn aim_at(&mut self, target: Vec3) {
        let dir = (target - self.position).normalize_or_zero();
        if dir == Vec3::ZERO {
            return;
        }
        self.yaw = dir.z.atan2(dir.x);
        self.pitch = dir.y.asin();
    }

    /// Apply this frame's mouse drag, while the right button is held.
    ///
    /// After the mode has had its say, so orbit reads the angles it is about to be given and the
    /// other two modes are simply overridden — dragging during a chase looks around and the next
    /// frame's aim takes it back.
    fn mouse_look(&mut self, input: &Input) {
        if input.is_mouse_button_pressed(gizmo::core::input::mouse::RIGHT) {
            let d = input.mouse_delta();
            self.yaw += d.0 * MOUSE_SENSITIVITY;
            self.pitch += d.1 * MOUSE_SENSITIVITY;
        }
        // Short of straight up or straight down, where the view basis degenerates.
        self.pitch = self
            .pitch
            .clamp(-std::f32::consts::FRAC_PI_2 + 0.1, std::f32::consts::FRAC_PI_2 - 0.1);
    }

    /// Write the camera state onto its entity.
    fn write(&self, world: &mut World) {
        let mut transforms = unsafe { world.borrow_mut_unchecked::<Transform>() };
        let mut globals = unsafe { world.borrow_mut_unchecked::<GlobalTransform>() };
        let mut cameras = unsafe { world.borrow_mut_unchecked::<Camera>() };
        if let Some(mut t) = transforms.get_mut(self.id) {
            t.position = self.position;
            t.update_local_matrix();
            if let Some(mut g) = globals.get_mut(self.id) {
                g.matrix = t.local_matrix;
            }
        }
        if let Some(mut c) = cameras.get_mut(self.id) {
            c.yaw = self.yaw + self.shake.0;
            c.pitch = self.pitch + self.shake.1;
            c.fov = self.fov;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: f32 = 1.0 / 60.0;

    /// Drive `follow` along `velocity` for `seconds`, nose along −Z unless `heading` turns it.
    fn drive(follow: &mut GameFollow, view: &ChaseView, at: &mut Vec3, velocity: Vec3, rot: Quat, seconds: f32) -> (Vec3, Vec3) {
        let mut out = (Vec3::ZERO, Vec3::ZERO);
        for _ in 0..(seconds / STEP) as usize {
            *at += velocity * STEP;
            out = follow.advance(view, *at, rot, STEP);
        }
        out
    }

    #[test]
    fn the_ease_is_the_spring_its_cubic_comes_to() {
        let mut e = Ease::at(0.0);
        let mut peak = 0.0f32;
        for _ in 0..600 {
            e.step(1.0, STEP, 1.0);
            peak = peak.max(e.value);
        }
        assert!((e.value - 1.0).abs() < 1e-3, "arrives: {}", e.value);
        // ζ = 0.816 overshoots by exp(−πζ/√(1−ζ²)) ≈ 1.2 %.
        assert!(peak < 1.02, "and barely overshoots: {peak}");
        let mut half = Ease::at(0.0);
        for _ in 0..30 {
            half.step(1.0, STEP, 1.0);
        }
        assert!(half.value > 0.3 && half.value < 0.8, "half a time constant is part of the way: {}", half.value);
    }

    #[test]
    fn standing_still_is_the_seat_in_the_record() {
        let view = DEFAULT_VIEWS[0];
        let (seat, aim) = GameFollow::new().advance(&view, Vec3::new(3.0, 1.0, 2.0), Quat::IDENTITY, STEP);
        assert!((seat - Vec3::new(3.0, 2.7, 7.8)).length() < 1e-4, "{seat}");
        assert!((aim - Vec3::new(3.0, 2.7, 2.0)).length() < 1e-4, "{aim}");
    }

    #[test]
    fn speed_alone_does_not_move_the_camera_off_the_seat() {
        let view = DEFAULT_VIEWS[0];
        let mut follow = GameFollow::new();
        let mut at = Vec3::ZERO;
        let (seat, _) = drive(&mut follow, &view, &mut at, Vec3::NEG_Z * 70.0, Quat::IDENTITY, 8.0);
        assert!(((at - seat).z.abs() - 5.8).abs() < 0.01, "{}", (at - seat).z);
    }

    #[test]
    fn hard_acceleration_slides_the_seat_back_to_its_limit_and_no_further() {
        let view = DEFAULT_VIEWS[0];
        let mut follow = GameFollow::new();
        let mut at = Vec3::ZERO;
        let mut speed = 0.0;
        let mut seat = Vec3::ZERO;
        for _ in 0..360 {
            speed += 30.0 * STEP;
            at += Vec3::NEG_Z * speed * STEP;
            seat = follow.advance(&view, at, Quat::IDENTITY, STEP).0;
        }
        let back = (seat - at).z;
        assert!(back > 5.8 + 1.4 && back < 5.8 + 1.5 + 0.02, "{back}");
    }

    #[test]
    fn a_frame_without_a_physics_step_is_not_a_stopped_car() {
        let view = DEFAULT_VIEWS[0];
        let mut follow = GameFollow::new();
        let mut at = Vec3::ZERO;
        drive(&mut follow, &view, &mut at, Vec3::NEG_Z * 40.0, Quat::IDENTITY, 2.0);
        let velocity = follow.velocity;
        follow.advance(&view, at, Quat::IDENTITY, STEP);
        assert_eq!(follow.velocity, velocity);
    }

    #[test]
    fn the_heading_is_the_nose_when_slow_and_the_travel_when_fast() {
        let nose = Vec3::NEG_Z;
        let sideways = Vec3::X;
        assert_eq!(GameFollow::heading_target(sideways * TRAVEL_FROM, nose), nose);
        assert_eq!(GameFollow::heading_target(sideways * 13.5, nose), sideways);
        let half = GameFollow::heading_target(sideways * 10.5, nose);
        assert!((half - (sideways + nose) * 0.5).length() < 1e-5, "{half}");
    }

    #[test]
    fn a_reversing_car_keeps_the_camera_behind_its_nose() {
        let target = GameFollow::heading_target(Vec3::Z * 20.0, Vec3::NEG_Z);
        assert!((target - Vec3::NEG_Z).length() < 1e-5, "{target}");
    }

    #[test]
    fn the_noise_stays_in_its_range_and_does_not_jump() {
        let mut last = fractal(0.0);
        for i in 1..20_000 {
            let v = fractal(i as f32 * 1e-3);
            assert!(v.abs() <= 2.0, "{v}");
            assert!((v - last).abs() < 0.5, "at {i}: {last} → {v}");
            last = v;
        }
        // At a whole number every octave is exactly one hash: the game's first, worked by hand.
        let n = 1u32;
        let n = (n << 13) ^ n;
        let h = n.wrapping_mul(n).wrapping_mul(0x3D73).wrapping_add(0xC_0AE5).wrapping_mul(n).wrapping_add(0xD208_DD0D) & 0x7FFF_FFFF;
        assert_eq!(noise(1.0), 1.0 - h as f32 / 1_073_741_824.0);
    }

    fn largest_shake(speed: f32, nitro: bool) -> f32 {
        (0..20_000)
            .map(|i| game_shake(speed, nitro, i as f32 * 0.01, GAME_HFOV).1.abs())
            .fold(0.0, f32::max)
            .to_degrees()
    }

    #[test]
    fn a_stopped_car_does_not_shake_and_a_fast_one_barely_does() {
        assert_eq!(game_shake(0.0, false, 12.3, GAME_HFOV), (0.0, 0.0));
        let cruising = largest_shake(60.0, false);
        // Row 2.4 of the table: amplitude 0.42 · 0.2, on 0.7° and a noise that peaks under 2.
        assert!(cruising > 0.01 && cruising < 0.14, "{cruising}");
    }

    #[test]
    fn nitro_is_what_shakes_the_view_and_not_when_standing_still() {
        let burning = largest_shake(60.0, true);
        assert!(burning > largest_shake(60.0, false) * 2.0, "{burning}");
        assert!(burning < 0.6, "{burning}");
        assert_eq!(game_shake(1.0, true, 5.0, GAME_HFOV), game_shake(1.0, false, 5.0, GAME_HFOV));
    }

    #[test]
    fn the_sky_view_aims_where_the_chase_views_do() {
        let sky = DEFAULT_VIEWS[2];
        let aim = sky.up + sky.forward * sky.pitch_deg.to_radians().tan();
        assert!((aim - DEFAULT_VIEWS[0].up).abs() < 0.02, "{aim}");
    }

    #[test]
    fn the_view_opens_with_speed_and_only_with_speed() {
        assert_eq!(speed_fov(0.0), FOV_REST);
        assert_eq!(speed_fov(FOV_FROM), FOV_REST);
        assert!((speed_fov(FOV_SPEED) - FOV_FAST).abs() < 1e-6);
        assert!((speed_fov(150.0) - FOV_FAST).abs() < 1e-6, "no wider past the end");
        assert_eq!(speed_fov(-30.0), speed_fov(30.0), "reversing is the same speed");
        let mut last = 0.0;
        for v in 0..80 {
            let f = speed_fov(v as f32);
            assert!(f >= last, "never narrows as the car speeds up");
            last = f;
        }
    }

    #[test]
    fn the_tremble_starts_at_its_speed_and_stays_small() {
        assert_eq!(shake_amplitude(SHAKE_FROM), 0.0);
        assert_eq!(shake_amplitude(10.0), 0.0);
        assert!(shake_amplitude(FOV_SPEED) <= SHAKE_MAX);
        for i in 0..1000 {
            let (y, p) = tremble(i as f32 * 0.013);
            assert!(y.abs() <= 1.0 && p.abs() <= 1.0);
        }
    }
}
