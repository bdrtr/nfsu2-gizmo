//! # Drive an NFSU2 car through Bayview (M3)
//!
//! The city `nfs_fly` looks at, given to physics, with the car `nfs_drive` drives standing on it.
//! Both halves are the shared layers — [`nfsu2::world`] turns the region bundles into cell-sized
//! meshes and per-cell triangle soup, [`nfsu2::rig`] builds the car — so what is here is only the
//! join: one `Collider::trimesh` per cell, and a car placed on top.
//!
//! **The colliders and the visuals are built from the same objects.** `dedup` and `nearest` run
//! once, up front, and both halves are handed the result — otherwise the edge of an object budget
//! is a place where you can see a road you cannot drive on, or hit one that is not drawn.
//!
//! ```bash
//! cargo run --release -p nfsu2 --bin nfs_cruise -- "$NFSU2_ROOT/TRACKS" "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
//! ```
//!
//! Controls: **W/↑** accelerate · **S/↓** reverse · **A/D or ←/→** steer · **Space** brake ·
//! **R** back to the start · **T** auto-shift · hold **right mouse** to orbit · **F** print where
//! the car is.
//!
//! Env: `NFS_FREEROAM=1` **serbest dolaşım** — Bayview with no race in it: loads `STREAML4RA.BUN`
//! (the one region of the eight that is the city), starts on the free-roam grid, draws no line and
//! never says you are off course · `NFS_SPOT=<n>` with it, stand at the n'th of the 24 places the
//! free-roam markers name instead · `NFS_TIERS=all` draw the coarse detail tiers too (off by default — they are distance
//! imposters and from a car they are blurred boxes in open ground) · `NFS_ROUTE=<Paths*.bin>` load that race: its line is drawn on the road and the HUD says
//! where you are on it and whether you are still on it · `NFS_AT="x,y,z"` where to start — downtown sits near `y ≈ 27` and the airport near
//! `y ≈ -11`, so the height matters as much as the place · `NFS_BUDGET=<n>` caps objects,
//! nearest-first · `NFS_DIAG=1` prints the physics' own view once a second · plus everything
//! [`nfsu2::rig`] reads (`NFS_PAINT`, `NFS_KIT`, `NFS_ENGINE`, `NFS_SHOTCAM`, …).

use gizmo::egui;
use gizmo::physics::world::PhysicsWorld;
use gizmo::prelude::*;
use gizmo_nfs::types::AssetHash;
use nfsu2::geom::add_transform;
use nfsu2::rig::{spawn_car, CarRig, ChaseCamera, Driver, Pilot, Placement, Rescue};
use nfsu2::scene::{self, Textures};
// Aliased: `world` is the ECS `World` in every function here, and a module by the same name three
// characters from a variable of another type is a re-read waiting to happen.
use nfsu2::world as city;
use std::collections::HashMap;

/// How far apart the driven waypoints are after the outline is subdivided.
///
/// Short enough that a grid is never far from one and long enough that a pilot is not chasing a
/// point under its own bumper.
const WAYPOINT_STEP: f32 = 40.0;

const DEFAULT_CAR: &str =
    "/home/bedir/Games/need-for-speed-underground-2/drive_c/Need for Speed Underground 2/CARS/240SX/GEOMETRY.BIN";

/// Where to start when nothing says otherwise: the street outside Bayview's tuning shops.
///
/// The old default, `(1354, −11, −2457)`, was chosen for having ground rather than for being
/// anywhere: it is **inside the airport**. The objects covering it are `TRN_RDP_RUNWAY_KT_CHOP_*`,
/// and a bundle-wide render of `STREAML4RB` is a runway, an apron and taxiways. Every building in
/// the city (`XB_*`) sits between `x ≈ −400` and `x ≈ 1000`, so that spawn was east of all of them,
/// on open tarmac with the skyline a kilometre off — which is why driving from it felt like being
/// outside the world rather than in it.
///
/// This one is measured the same way, by dropping the car and reading `NFS_DIAG`: it settles at
/// `y = 27.06` with four wheels down, under the elevated highway, with `XB_PERFORMANCE_SHOP`,
/// `XB_PAINTSHOPA`, `XB_AUDIOSHOPA` and `XB_BODYSHOPA` within 200 m. Note the height — the city is
/// not flat, and `y ≈ −11` is true of the airport, not of downtown.
const DEFAULT_AT: Vec3 = Vec3::new(710.0, 27.0, 888.0);

/// How many places a starting grid has. The file's own number: every full grid in the install is
/// eight markers numbered `0..7`, and `gizmo_nfs::world::routes::grids` returns nothing else.
const GRID_SLOTS: usize = 8;

/// How far above the named point the car is dropped.
///
/// The city's surface height at a given XZ is not known without querying it — the point above is a
/// road *near* `y = -11`, not a contact patch — so the car is dropped from a little way up and the
/// suspension settles it. Too small and it spawns inside the tarmac; too large and it lands hard.
const DROP: f32 = 1.5;

/// How far above the named point the ground is looked for.
///
/// `Ground::height_at` answers with the highest surface *at or below* the point it is given, so a
/// spawn named a few centimetres inside the tarmac would otherwise find the road under the road.
/// Two metres is enough to clear that and short enough not to reach the deck of an overpass the
/// car is meant to be driving beneath.
const SPAWN_PROBE: f32 = 2.0;

/// How long the car may be outside the mapped city before it is put back.
///
/// Long enough to be a warning rather than a punishment: at 100 km/h it is 140 m of road, which is
/// far more than any corner cuts. It is deliberately much longer than the fall guard's 2.5 s,
/// because the two answer different questions — falling is unrecoverable and being outside is not.
const OUT_OF_BOUNDS_GRACE: f32 = 5.0;

/// How far ahead the boundary is asked about, in seconds of travel at the current speed.
///
/// Seconds rather than metres because the warning has to arrive in time to act on, and at 30 km/h
/// 200 m is a leisurely warning while at 200 km/h it is 3.6 seconds of panic.
const OUT_OF_BOUNDS_LOOKAHEAD: f32 = 2.0;

struct CruiseState {
    rig: CarRig,
    driver: Driver,
    camera: ChaseCamera,
    /// The last whole second `NFS_DIAG` printed a line for.
    diag_tick: i32,
    t: f32,
    stats: CityStats,
    /// Which cells the city covers — the map's edge, since the files carry no barriers.
    bounds: city::Bounds,
    /// The drivable surface, kept past setup because the barrier is derived from it every step.
    ground: city::Ground,
    /// The other half of the same triangles: what a car cannot drive *through*. Kept for the same
    /// reason — the pilots ask it every step whether something stands between them and where they
    /// are steering.
    walls: city::Walls,
    /// Seconds spent outside those cells, unbroken. Reset the moment the car is back in.
    out_for: f32,
    /// Whether where the car is *pointed* leaves the map — the warning that arrives in time.
    heading_out: bool,
    /// The race this file describes, if `NFS_ROUTE` named one.
    course: Option<Course>,
    /// Free roam: Bayview with no race in it. Mutually exclusive with `course` by construction.
    free_roam: bool,
    /// The lone free-roam markers, so the HUD can say how many places there are to jump to.
    spots: usize,
    /// The rivals, and the driver each one has.
    field: Vec<(CarRig, Pilot)>,
    /// How many pilots produced controls last frame — zero means they are not being driven at all,
    /// which looks exactly like being driven badly.
    driving: usize,
    /// The road network the rivals drive. Empty in free roam and when no route is loaded.
    net: city::Network,
    /// The event outline, in lap order: the waypoints the rivals steer between junctions by.
    waypoints: Vec<Vec3>,
    /// Recent frame times, in milliseconds, newest last.
    ///
    /// **The number the project has never had.** Every decision about culling, detail tiers and
    /// streaming has been argued from object counts, and an object count is not a frame time: the
    /// engine's own spatial index ships with a benchmark saying a BVH *loses* to a linear scan
    /// below roughly eight thousand renderables and telling the caller to measure their own scene
    /// instead of quoting the band. This is that measurement.
    frames: Vec<f32>,
}

/// How many frames the timing window keeps. One second at 60 Hz, so a stutter shows as a p95 that
/// moves rather than as a mean that hides it.
const FRAME_WINDOW: usize = 60;

/// A route file's network, and where the car is on it.
///
/// The install ships no barriers (`ROADMAP.md` §M4), so "off the course" cannot be read — it is
/// derived from the race's own paths by [`city::Corridor`]. Being outside this is a *different*
/// question from being outside [`city::Bounds`]: the map's edge is where the world stops, and this
/// is where the race does.
struct Course {
    corridor: city::Corridor,
    /// The last fix taken, for the HUD. `None` means the network does not reach the car at all.
    fix: Option<city::Fix>,
    /// Seconds spent off the course, unbroken.
    off_for: f32,
    /// What the file called itself, so the HUD can say which race is loaded.
    name: String,
}

use nfsu2::world::COURSE_HALF_WIDTH;

/// How long the car may be off the course before the HUD stops being polite about it.
const OFF_COURSE_GRACE: f32 = 4.0;

/// What loading the city produced, kept for the HUD — the numbers that say whether the world under
/// the car is the world in front of it.
struct CityStats {
    objects: usize,
    meshes: usize,
    cells: usize,
    triangles: usize,
    drivable: usize,
    /// Sky shells and panorama panels — drawn, but as a backdrop rather than as world.
    backdrop: usize,
}

fn main() {
    gizmo::app::setup_panic_hook();
    App::<CruiseState>::new("Gizmo — NFSU2 Bayview", 1600, 900)
        .add_plugin(gizmo::plugins::TransformPlugin)
        .set_setup(setup)
        .set_update(update)
        .set_ui(ui)
        .set_render(|world, _s, encoder, view, renderer, _t| {
            // The city is baked-lit and opaque; the screen-space passes cost frame time and have
            // nothing to work with here.
            renderer.gpu_fluid = None;
            renderer.gpu_particles = None;
            // The glare threshold the renderer ships with extracts nothing from a baked-lit
            // night map — see `scene::city_glare` for the measurement.
            let (bt, bi) = scene::city_glare();
            renderer.bloom_threshold = bt;
            renderer.bloom_intensity = bi;
            renderer.ssr = None;
            renderer.ssgi = None;
            renderer.volumetric = None;
            // TAA stays on, unlike the small-world binaries: `nfs_fly` keeps it and this is the
            // same city at the same distances.
            gizmo::systems::default_render_pass(world, encoder, view, renderer);
        })
        .run()
        .expect("failed to run app");
}

fn setup(world: &mut World, renderer: &gizmo::renderer::Renderer) -> CruiseState {
    let tracks = city::tracks_path(std::env::args().nth(1));
    let car_path = std::env::args()
        .nth(2)
        .or_else(|| std::env::var("NFSU2_CAR").ok())
        .unwrap_or_else(|| DEFAULT_CAR.to_string());
    let at = city::start_at(DEFAULT_AT);
    let budget = std::env::var("NFS_BUDGET").ok().and_then(|s| s.parse::<usize>().ok());

    // A race is driven in a region, so load that region rather than every bundle in the folder.
    // The eight are not versions of one city — see `world::bundle_for_route` for what stacking them
    // costs — and until something streams by position, the route names the region for us.
    //
    // Free roam is the same question with a fixed answer. It is not a race and has no route file,
    // so `NFS_FREEROAM=1` names the city directly: `STREAML4RA.BUN`, the one region of the eight
    // that *is* Bayview. See `world::REGIONS` for what the other seven are.
    let free_roam = std::env::var("NFS_FREEROAM").is_ok_and(|v| v != "0");
    let region = if free_roam {
        city::free_roam_bundle(std::path::Path::new(&tracks))
    } else {
        std::env::var("NFS_ROUTE").ok().and_then(|r| city::bundle_for_route(std::path::Path::new(&r)))
    };
    if let Some(b) = &region {
        let what = if free_roam { "free roam is driven in" } else { "this race is driven in" };
        println!("region: {} — the bundle {what}", b.display());
    }
    let source = region.map_or_else(|| tracks.clone(), |b| b.display().to_string());
    let city::Bundles { files, meshes, packs, shared } = city::load(&source);
    println!(
        "{} bundle(s), {} objects, {} packs, {} shared textures",
        files.len(),
        meshes.len(),
        packs.len(),
        shared.len()
    );

    // One preparation, feeding both halves — and neither of them gets the backdrop, because a sky
    // shell handed to physics is a dome the car would drive into.
    //
    // The backdrop comes out first and is kept, not dropped. `is_drawn` excludes two families for
    // two different reasons: the LOD proxy is a coarse stand-in that sits across the real streets
    // and has nothing to replace it with, while the sky shell and panorama panels were only ever a
    // problem because *ordinary* geometry that large is a wall across the frame. Drawn as a skybox
    // they cannot be — the panorama encloses the city, so it sits behind it — so the reason to drop
    // does not apply and the world stops ending at a flat grey horizon.
    let (backdrop, rest): (Vec<_>, Vec<_>) =
        meshes.into_iter().partition(|m| city::is_backdrop(&m.header.name));
    let mut drawn = rest;
    drawn.retain(|m| city::is_drawn(&m.header.name));
    let declared = drawn.len();
    let mut objects = city::dedup(drawn);
    println!("{declared} drawn objects, {} after dedup", objects.len());
    city::nearest(&mut objects, at, budget);

    // The city's own detail tiers — `_1A`/`_1B`/`_1Z`, one design in three levels. Only the
    // richest of each family is drawn: the coarse ones are distance imposters, and from a car they
    // are blurred boxes standing in open ground. `NFS_TIERS=all` puts them back.
    println!("{}", city::lod::report(&objects));
    if std::env::var("NFS_TIERS").as_deref() == Ok("all") {
        println!("NFS_TIERS=all: the coarse detail tiers are drawn too");
    } else {
        let before = objects.len();
        objects = city::lod::keep_finest(objects);
        println!("detail tiers: dropped {} coarser members, {} objects left", before - objects.len(), objects.len());
    }

    let mut phys = PhysicsWorld::new();
    phys.integrator.gravity = Vec3::new(0.0, -9.81, 0.0);

    // ── The city as something to hit ──
    //
    // One static trimesh body per cell. Not one for the whole city (its bounding box would pair
    // with every dynamic body every step) and not one per solid (14,000 bodies). The cell is the
    // unit that is neither, and it is the same 256 m cell the visuals merge into, so the collider
    // under the car and the mesh in front of it come from the same objects.
    let colliders = city::collision_cells(&objects);
    let bounds = city::Bounds::of(&colliders);

    // Where the ground actually is, instead of where the spawn constant guesses it is. Every
    // `NFS_AT` in this file's history was found by flying there, pressing F and writing the number
    // down — including the height, which is why the airport's `y = -11` was carried downtown where
    // the ground is at `y = 27`. Asked properly, the number comes from the city.
    let ground = city::Ground::of(&colliders);
    // What a car cannot drive through, for the pilots' "not through that" rule — the same index the
    // sim measures with, so the two cannot disagree about where the concrete is.
    let walls = city::Walls::of(&colliders);
    let named_at = std::env::var("NFS_AT").is_ok();
    //
    // Free roam takes the `Err` arm on purpose even if `NFS_ROUTE` is set: there is nothing to
    // follow, no line to draw and no corridor to be outside of. That is what free roam is.
    // The raw table and the event, kept beside the built line so the network and the waypoints can
    // be made from them without reading the file twice.
    let mut route_nodes: Vec<gizmo_nfs::world::routes::RouteNode> = Vec::new();
    let mut course_event: Option<gizmo_nfs::world::routes::RaceEvent> = None;
    let (course_paths, mut course) = match std::env::var("NFS_ROUTE") {
        _ if free_roam => (Vec::new(), None),
        Err(_) => (Vec::new(), None),
        Ok(file) => {
            let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("read {file}: {e}"));
            let nodes =
                gizmo_nfs::world::routes::nodes(&bytes).expect("read the route file's nodes");
            route_nodes = nodes.clone();
            let paths = city::build_route(&nodes, &city::road_ground(&objects));
            let corridor = city::Corridor::of(&paths, COURSE_HALF_WIDTH);
            let name = std::path::Path::new(&file)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            println!(
                "route {name}: {} paths · {} segments · {:.0} m of line · corridor half-width {COURSE_HALF_WIDTH} m",
                paths.len(),
                corridor.segments(),
                paths.iter().map(city::RoutePath::length).sum::<f32>()
            );
            (paths, Some(Course { corridor, fix: None, off_for: 0.0, name }))
        }
    };

    // Where the file puts the cars, if it says. `TrackPosMarkers*.bin` sits beside the route file
    // and holds the event's starting grids; `start_of`'s least-progress node is the fallback for
    // when it does not, and it is a guess where this is a record.
    let mut spot_count = 0usize;
    // The other seven places on the grid, if this is a race and the file names them. Kept beside
    // the pole rather than derived twice: `city::start_slots` and `city::start_grid_facing` share
    // the choice of grid, so these are the same eight the pole came from.
    let mut grid_slots: Vec<Vec3> = Vec::new();
    let course_start = if free_roam {
        // The city's own free-roam grid, and the places it names around it. Only `ROUTESL4RA`
        // carries any: the other seven `TrackPosMarkersFreeRoam.bin` are 16-byte shells.
        let file = std::path::Path::new(&tracks).join("ROUTESL4RA/TrackPosMarkersFreeRoam.bin");
        std::fs::read(&file).ok().and_then(|bytes| {
            let m = gizmo_nfs::world::routes::markers(&bytes).ok()?;
            let spots = city::free_roam_spots(&m);
            spot_count = spots.len();
            println!(
                "free roam: {} markers · 1 grid · {} named places in the city",
                m.len(),
                spots.len()
            );
            // NFS_SPOT=<n>: stand at one of the lone markers instead of the grid. They are spread
            // over the whole map and the file considers each of them a place, so they are the
            // cheapest way to look at a part of Bayview that is not the start.
            match std::env::var("NFS_SPOT").ok().and_then(|v| v.parse::<usize>().ok()) {
                Some(i) if !spots.is_empty() => {
                    let p = spots[i % spots.len()];
                    // A lone marker carries no direction — it is one point, and a grid's front/back
                    // trick has nothing to work with. Facing the grid gives the car *a* heading that
                    // is at least about the city rather than about the axes, and any of them is a
                    // guess: the name behind the group hash is what would say which way a place
                    // faces, and it is not recovered yet.
                    let toward =
                        city::free_roam_start(&m).map_or(Vec3::Z, |(g, _)| (g - p) * Vec3::new(1.0, 0.0, 1.0));
                    println!("  spot {}/{}: {p:?}", i % spots.len(), spots.len());
                    Some((p, toward.normalize_or(Vec3::Z)))
                }
                _ => city::free_roam_start(&m),
            }
        })
    } else {
        std::env::var("NFS_ROUTE")
        .ok()
        .and_then(|r| {
            let route = std::path::Path::new(&r);
            let event: u16 = route
                .file_stem()?
                .to_str()?
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .parse()
                .ok()?;
            let dir = route.parent()?;
            let bytes = std::fs::read(dir.join("TrackPosMarkersAll.bin")).ok()?;
            let m = gizmo_nfs::world::routes::markers(&bytes).ok()?;
            // A track has two grids — its two race directions — and the event's own outline says
            // which one this race uses. Without it the choice was "the first", which is a coin
            // flip; see `city::start_grid_facing` for what the outline is worth in numbers.
            let route_bytes = std::fs::read(route).ok()?;
            let catalogue = gizmo_nfs::world::routes::events(&route_bytes).unwrap_or_default();
            course_event = catalogue.iter().find(|e| e.id == event).cloned();
            let placed = match catalogue.iter().find(|e| e.id == event) {
                Some(e) => {
                    if let Some((slots, _)) = city::start_slots(&m, e) {
                        grid_slots = slots;
                    }
                    city::start_grid_facing(&m, e)
                }
                None => city::start_grid(&m, event),
            };
            if placed.is_some() {
                let how = if catalogue.iter().any(|e| e.id == event) {
                    "grid picked by the event outline"
                } else {
                    "first grid — this event has no outline"
                };
                println!("start grid: event {event}, {how}");
            }
            placed
        })
        .or_else(|| city::start_of(&course_paths))
    };

    // A race has a start line, and it is the only thing in the data that names one: the point of
    // least `progress`. It takes precedence over the downtown default, and gives the car a heading
    // as well — a grid position pointing at a wall is worse than no grid position.
    let (at, start_heading) = match (named_at, course_start) {
        (false, Some((p, dir))) => {
            println!("starting at {p:?}");
            (p, Some(dir))
        }
        _ => (at, None),
    };

    let at = match ground.height_at(at + Vec3::Y * SPAWN_PROBE) {
        Some(y) => {
            println!("ground at ({:.0},{:.0}) is y={y:.2} — asked, not guessed", at.x, at.z);
            Vec3::new(at.x, y, at.z)
        }
        None => {
            println!("no drivable ground under ({:.0},{:.0}) — falling back to the named height", at.x, at.z);
            at
        }
    };
    println!("ground grid: {} cells of {} m, {} triangle refs", ground.cells(), city::GROUND_CELL, ground.refs());

    // MEASUREMENT ONLY for now: how much of what we draw stands over a cell with no ground in it.
    let (off, off_v): (usize, usize) = objects
        .iter()
        .filter(|m| {
            let c = city::world_point(&m.header, m.header.bbox_min)
                .midpoint(city::world_point(&m.header, m.header.bbox_max));
            !bounds.contains(c)
        })
        .fold((0, 0), |(n, v), m| (n + 1, v + m.positions.len()));
    let total_v: usize = objects.iter().map(|m| m.positions.len()).sum();
    println!(
        "off-map: {off} of {} objects ({:.1}%), {off_v} of {total_v} vertices ({:.1}%)",
        objects.len(),
        100.0 * off as f32 / objects.len() as f32,
        100.0 * off_v as f32 / total_v as f32
    );

    // NFS_ROUTE=<Paths*.bin>: the race being driven. Its paths are stood on the city and kept as
    // a corridor, because "off the course" cannot be read from the install — there are no barriers
    // in it — and the race's own network is what it has to be derived from.
    let mut stats = CityStats {
        objects: objects.len(),
        meshes: 0, // the visuals are built below
        cells: colliders.len(),
        triangles: colliders.iter().map(city::CityCollider::triangles).sum(),
        drivable: colliders.iter().map(city::CityCollider::drivable).sum(),
        backdrop: 0, // the backdrop is built below
    };
    for cell in colliders {
        // Vertices are already relative to the cell's own origin, so the body goes there and the
        // collider's bounds are the cell's rather than the city's.
        let at_cell = Transform::new(cell.origin);
        let entity = world.spawn();
        add_transform(world, entity, at_cell);
        let collider = Collider::trimesh(cell.vertices, cell.indices);
        world.add_component(entity, RigidBody::new_static());
        world.add_component(entity, Velocity::default());
        world.add_component(entity, collider.clone());
        world.add_component(entity, gizmo::physics::components::PhysicsMaterial::ASPHALT);
        phys.add_body(
            gizmo::physics::BodyHandle::from_id(entity.id()),
            RigidBody::new_static(),
            at_cell,
            Velocity::default(),
            collider,
        );
    }
    println!(
        "collision: {} cells, {} triangles ({} drivable)",
        stats.cells, stats.triangles, stats.drivable
    );

    // ── The city as something to look at ──
    // `None` for the budget: `nearest` has already run, and running it again on an already-trimmed
    // list would be a second sort for nothing.
    let visuals = city::build_region(&renderer.device, objects, &packs, Some(&shared), at, None);
    stats.meshes = visuals.meshes.len();

    // The sky and the painted panorama, through the same builder — they are ordinary geometry, and
    // only their *material* is unusual. `at` for the budget centre is irrelevant here: `None` keeps
    // all of them, and there are 40 sky shells and a handful of panels across the eight bundles.
    let sky = city::build_region(&renderer.device, backdrop, &packs, Some(&shared), at, None);
    stats.backdrop = sky.meshes.len();
    // Whether the sky is the game's or a white stand-in is not a thing to judge by eye — an
    // untextured shell and an overcast sky texture are the same pale grey on screen.
    println!(
        "backdrop: {} meshes, {} textured, {} unresolved runs",
        sky.meshes.len(),
        sky.meshes.iter().filter(|m| m.texture.is_some()).count(),
        sky.unresolved_runs
    );

    let mut assets = AssetManager::new();
    let white = assets.create_white_texture(
        &renderer.device,
        &renderer.queue,
        &renderer.scene.texture_bind_group_layout,
    );
    {
        let mut tex = Textures {
            assets: &mut assets,
            device: &renderer.device,
            queue: &renderer.queue,
            layout: &renderer.scene.texture_bind_group_layout,
        };
        let mut bound: HashMap<AssetHash, _> = HashMap::new();
        for key in visuals.meshes.iter().chain(&sky.meshes).filter_map(|m| m.texture) {
            if bound.contains_key(&key) {
                continue;
            }
            // The region's own packs first, then the shared tiers — the same order the mesh's key
            // was resolved in, so a key that resolved shared uploads from shared.
            let own = packs.iter().find_map(|p| p.get(key).and_then(|r| p.decode(r).ok()));
            let Some(img) = own.or_else(|| shared.get(key).cloned()) else { continue };
            if let Some(bg) = tex.upload(&format!("city_{:08X}", key.0), &img.rgba, img.width, img.height) {
                bound.insert(key, bg);
            }
        }
        if !course_paths.is_empty() {
            // Half a metre, not the three the headless overview needs: a chase camera is metres
            // from the road, and three would float.
            let verts = city::ribbon(&course_paths, 4.0, 0.5);
            if !verts.is_empty() {
                let mesh = Mesh::from_vertices(&renderer.device, &verts, String::from("course"));
                let material =
                    Material::new(white.clone()).with_unlit(Vec4::new(0.95, 0.15, 0.15, 1.0));
                scene::spawn_mesh(world, mesh, material, Transform::new(Vec3::ZERO));
            }
        }

        let lift = scene::city_lift();
        for m in &visuals.meshes {
            // The city's lighting is baked into its vertex colours; `BakedLit` multiplies them in
            // rather than relighting a static world that was never drawn to be relit.
            let material = match m.texture.and_then(|k| bound.get(&k)) {
                Some(bg) => Material::new(bg.clone()).with_baked_lit(Vec4::new(1.0, 1.0, 1.0, 1.0)),
                None => Material::new(white.clone()).with_baked_lit(Vec4::new(0.35, 0.35, 0.38, 1.0)),
            };
            // The two arms the engine gained and nothing was pulling — `scene::city_lift`.
            let material = material.with_ambient(lift.0).with_emissive(lift.1);
            scene::spawn_mesh(world, m.mesh.clone(), material, Transform::new(m.origin));
        }

        // `MaterialType::Backdrop`: drawn first, camera-locked, depth writes off — the three
        // things a painted backdrop needs, which the note on `is_backdrop` had been asking for.
        //
        // This replaced a choice between two wrongs, and the reason it is worth a paragraph is that
        // the *pixels* of the old one are still what the numbers in `MOTOR-NOTLARI.md` item 7 are
        // measured against:
        //
        //   - `MaterialType::Skybox` got the *depth* right — `sky.wgsl` pins NDC z to the far
        //     plane, so it can never occlude the city — but contained no `textureSample` at all,
        //     discarding the mesh's texture and vertex colour for a procedural gradient computed
        //     from `scene.sun_color`. NFSU2's own painted sky never reached the screen; what you
        //     got was the engine's, at a pale (225,231,234). Frame median 30/255.
        //   - `MaterialType::Unlit` got the *pixels* right (`vertex colour × albedo × texture`) and
        //     the depth wrong: the panorama panels drew as ordinary geometry in front of the city,
        //     which is the "wall across the whole frame" that had them excluded in the first place.
        //     Frame median 14/255, with two pale panels between the camera and the world.
        //
        // **`with_backdrop_placed`, and the distinction is the whole point.** The locked variant
        // is right for a unit-cube skybox and wrong for this: NFSU2's backdrop is *world-placed
        // geometry* 12–18 km across, and locking it to the camera dragged a three-kilometre panel
        // onto the lens — move the camera 1,000 m and the panel stayed at the same screen position
        // and the same size, covering the city rather than sitting behind it. The engine gained
        // the placed variant for exactly this (`MOTOR-NOTLARI.md` 7 and 9); the same measurement
        // now reads 0.0 %.
        //
        // On by default: this is the game's own sky, and the frame is wrong without it in a way
        // that is easy to mistake for a different bug. The city's reflective surfaces mirror
        // whatever the sky is, so with no backdrop they show the engine's pale grey and read as
        // flat white patches on the ground — one cause, two symptoms. `NFS_BACKDROP=0` turns it
        // off.
        //
        // Double-sided, because a sky shell is seen from the inside and its triangles face out —
        // single-sided it culls to nothing, which looks exactly like not drawing it at all.
        if std::env::var("NFS_BACKDROP").as_deref() != Ok("0") {
            for m in &sky.meshes {
                let material = match m.texture.and_then(|k| bound.get(&k)) {
                    Some(bg) => Material::new(bg.clone()),
                    None => Material::new(white.clone()),
                };
                let material = material.with_backdrop_placed(Vec4::ONE).with_double_sided(true);
                scene::spawn_mesh(world, m.mesh.clone(), material, Transform::new(m.origin));
            }
        }
    }

    // The sun is for the *car*: its materials are PBR and the city's are baked-lit, so this lights
    // one object in the frame — and casts the shadow that puts it on the road rather than above it.
    scene::add_lights(
        world,
        Transform::new(at + Vec3::new(0.0, 400.0, 0.0))
            .with_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0).normalize(), -0.9)),
        2.2,
        at + Vec3::new(-200.0, 200.0, -100.0),
    );

    let rig = spawn_car(
        world,
        renderer,
        &mut assets,
        &mut phys,
        &car_path,
        match start_heading {
            Some(h) => Placement::facing(at, h, DROP),
            None => Placement { ground: at, yaw: 0.0, clearance: DROP },
        },
    );
    // The graph the rivals drive, and the waypoints they steer between junctions by. Built here
    // because both want `ground`, which is the same surface the drawn line stands on — a driver and
    // a ribbon that disagreed about where the road is would be very hard to read.
    let net = city::Network::of(&route_nodes, &ground);
    if !net.is_empty() {
        let (edges, dead, steep, walled) = net.shape();
        println!(
            "network: {} nodes · {edges} links · {dead} with no way out · {steep} steeper than 1:1 \
             · {walled} dropped because the road does not continue along them",
            net.len()
        );
    }
    // Subdivided, because the outline's own corners are up to 425 m apart — see `route::densify`.
    let coarse: Vec<Vec3> = course_event
        .as_ref()
        .map(|e| e.outline.iter().map(|p| city::remap([p[0], p[1], 0.0])).collect())
        .unwrap_or_default();
    let course_line = city::densify(&coarse, WAYPOINT_STEP);
    if !course_line.is_empty() {
        println!("course: {} waypoints from the event outline", course_line.len());
    }

    // NFS_RIVALS=<n>: fill the rest of the grid. A starting grid has eight places and the file
    // names all eight; until now seven of them stood empty, which is the one thing that makes a
    // race look like a drive. They have no driver yet — they stand on their marks — so this is the
    // formation, not the field.
    //
    // Each is its own `spawn_car`, which re-reads and re-parses the model per car. Wasteful and
    // left that way on purpose: the cost is measured below and it is not where the frame goes, and
    // a shared-geometry path is a change to `rig` that should be made when something needs it.
    let rivals: usize = std::env::var("NFS_RIVALS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if grid_slots.is_empty() { 0 } else { GRID_SLOTS - 1 })
        .min(grid_slots.len().saturating_sub(1));
    let mut field = Vec::new();
    for slot in grid_slots.iter().skip(1).take(rivals) {
        // The marker's own height is within a metre of the ground almost everywhere, but "almost"
        // is what drops a car through the road — ask the city, the same as for the player.
        let stand = match ground.height_at(*slot + Vec3::Y * SPAWN_PROBE) {
            Some(y) => Vec3::new(slot.x, y, slot.z),
            None => *slot,
        };
        let rig = spawn_car(
            world,
            renderer,
            &mut assets,
            &mut phys,
            &car_path,
            match start_heading {
                Some(h) => Placement::facing(stand, h, DROP),
                None => Placement { ground: stand, yaw: 0.0, clearance: DROP },
            },
        );
        let mut pilot = Pilot::new();
        pilot.place(stand, start_heading.unwrap_or(Vec3::NEG_Z), &net, &course_line);
        field.push((rig, pilot));
    }
    if !field.is_empty() {
        // The formation, in its own terms: how far apart the cars are across a row and between the
        // rows. A grid that came out as eight cars in a heap, or in one line, says the slot order
        // or the heading was read wrong, and both are cheap to state and expensive to eyeball.
        let h = start_heading.unwrap_or(Vec3::Z);
        let side = Vec3::new(-h.z, 0.0, h.x).normalize_or_zero();
        let along = |p: &Vec3| (*p - grid_slots[0]).dot(h);
        let across = |p: &Vec3| (*p - grid_slots[0]).dot(side);
        let rows: Vec<f32> = grid_slots.iter().map(along).collect();
        let cols: Vec<f32> = grid_slots.iter().map(across).collect();
        let gap = |v: &[f32]| {
            let mut u = v.to_vec();
            u.sort_by(f32::total_cmp);
            u.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 0.5).fold(f32::MAX, f32::min)
        };
        println!(
            "grid: {} rivals on their marks, {} places · rows {:.1}..{:.1} m, columns {:.1}..{:.1} m \
             · closest row gap {:.1} m, closest column gap {:.1} m",
            field.len(),
            grid_slots.len(),
            rows.iter().copied().fold(f32::MAX, f32::min),
            rows.iter().copied().fold(f32::MIN, f32::max),
            cols.iter().copied().fold(f32::MAX, f32::min),
            cols.iter().copied().fold(f32::MIN, f32::max),
            gap(&rows),
            gap(&cols),
        );
    }

    world.insert_resource(assets);
    world.insert_resource(phys);

    // near 0.5 / far 20 000 — the pair `nfs_fly` measured for this city. The ratio is what costs
    // depth precision, and a city needs the far plane, so the near plane is what has to give.
    let camera =
        ChaseCamera::spawn(world, rig.start.position + Vec3::new(0.0, 4.0, 10.0), 0.5, 20_000.0);

    println!("cruising at {:?} — {} meshes drawn", rig.start.position, stats.meshes);
    println!("bounds: {} cells with ground", bounds.cells());
    CruiseState {
        ground,
        walls,
        rig,
        driver: Driver::new(),
        camera,
        diag_tick: -1,
        t: 0.0,
        stats,
        bounds,
        out_for: 0.0,
        heading_out: false,
        course: course.take(),
        free_roam,
        spots: spot_count,
        field,
        driving: 0,
        net,
        waypoints: course_line,
        frames: Vec::with_capacity(FRAME_WINDOW),
    }
}

/// The window's median and 95th percentile in milliseconds, or `None` before it has filled.
///
/// Both, because a median alone says the frame is fine while every twentieth one is not, and a
/// mean would let one 200 ms hitch pass as a rounding error across a second.
fn frame_ms(frames: &[f32]) -> Option<(f32, f32)> {
    if frames.len() < FRAME_WINDOW / 2 {
        return None;
    }
    let mut v = frames.to_vec();
    v.sort_by(f32::total_cmp);
    Some((v[v.len() / 2], v[v.len() * 19 / 20]))
}

fn update(world: &mut World, state: &mut CruiseState, dt: f32, input: &Input) {
    state.t += dt;
    if state.frames.len() == FRAME_WINDOW {
        state.frames.remove(0);
    }
    state.frames.push(dt * 1000.0);

    let controls = state.driver.read(input, dt);
    state.rig.drive(world, &controls);

    // The rivals. Same `drive` the player's controls go through — a pilot that reached past it
    // into physics would be racing a different car from the one on screen.
    state.driving = 0;
    // The rivals see each other, and they see the player: a field that only avoids its own kind
    // would drive straight through whoever is being raced.
    let traffic: Vec<Vec3> = state
        .field
        .iter()
        .filter_map(|(r, _)| r.pose(world).map(|p| p.position))
        .chain(state.rig.pose(world).map(|p| p.position))
        .collect();
    // Borrowed field by field rather than through `state`, which is what lets a pilot read the
    // city while the field it is in is held mutably. It also retires the `mem::take` dance that
    // used to stand in for exactly this.
    let CruiseState { field, net, waypoints, walls, ground, driving, .. } = state;
    for (rig, pilot) in field.iter_mut() {
        let Some(pose) = rig.pose(world) else { continue };
        let c = pilot.drive(
            pose.position,
            pose.rotation,
            pose.speed,
            net,
            waypoints,
            &traffic,
            Some((&*walls, &*ground)),
        );
        if let Some(c) = c {
            rig.drive(world, &c);
            *driving += 1;
        }
    }

    if input.is_key_just_pressed(KeyCode::KeyR as u32) {
        state.rig.reset(world);
        state.driver.reset();
    }

    // The barrier the files do not carry, derived from the ground the city does have. Applied to
    // the player and to every rival through the same call: a fence one of them can drive through is
    // not a fence, it is a handicap.
    let CruiseState { rig, driver, field, ground, .. } = state;
    driver.step_physics_with(world, dt, |w| {
        if let Some(p) = rig.pose(w) {
            rig.hold_at_edge(w, p, ground);
        }
        for (r, _) in field.iter_mut() {
            if let Some(p) = r.pose(w) {
                r.hold_at_edge(w, p, ground);
            }
        }
    });

    let Some(pose) = state.rig.pose(world) else { return };

    // The city has holes the shipped geometry never covered; a car that finds one must come back,
    // not fall for ever. One frame is skipped rather than driving the camera to a pose that no
    // longer exists.
    match state.rig.keep_in_world(world, pose, dt) {
        Rescue::None => {}
        Rescue::ToLastGround => {
            state.driver.reset();
            println!("out of the world at {:?} — back on the last ground", pose.position);
            return;
        }
        // The rig has already said why, once. Repeating it every 2.5 s would bury the diagnostics.
        Rescue::NowhereSafe => {
            state.driver.reset();
            return;
        }
    }

    // The boundary is asked **ahead of** the car, not under it, and that is the whole design.
    //
    // Asking where the car *is* was tried first and is worthless here: outside the mapped cells
    // this city has no ground at all, so leaving the map is always a fall, the fall guard fires at
    // 2.5 s and a 5 s boundary timer never gets a turn. Measured — spawned at (4000, 30, 3000), the
    // only thing that ever printed was the rig's "no ground under the spawn point".
    //
    // Asked two seconds ahead it answers a question nothing else does, while the answer is still
    // useful: *you are driving off the edge of the world*. That is the warning NFSU2's barriers
    // would have made unnecessary, and until they can be derived it is what there is.
    let forward = pose.rotation * Vec3::NEG_Z;
    let look = (pose.speed.abs() * OUT_OF_BOUNDS_LOOKAHEAD).clamp(40.0, 250.0);
    state.heading_out = !state.bounds.contains(pose.position + forward * look);

    // Where the car is on the race, if one is loaded. Asked **under** the car rather than ahead of
    // it, unlike the map boundary above: leaving the course is recoverable and instantaneous, so
    // the honest thing to report is where you are, not where you are going.
    if let Some(course) = state.course.as_mut() {
        course.fix = course.corridor.locate(pose.position);
        let on = course.fix.is_some_and(|f| f.distance <= COURSE_HALF_WIDTH);
        course.off_for = if on { 0.0 } else { course.off_for + dt };
    }

    // The car being outside *itself* is the rare case — 17 of the city's 489 cells have collision
    // but nothing drivable, a rooftop or a wall face — and there the wheels are down and no other
    // guard applies, so it still gets a countdown.
    if state.bounds.contains(pose.position) {
        state.out_for = 0.0;
    } else {
        state.out_for += dt;
        if state.out_for >= OUT_OF_BOUNDS_GRACE {
            state.out_for = 0.0;
            state.rig.recover(world);
            state.driver.reset();
            println!("out of bounds at {:?} — back on the last ground", pose.position);
            return;
        }
    }

    if input.is_key_just_pressed(KeyCode::KeyF as u32) {
        println!("NFS_AT=\"{:.0},{:.0},{:.0}\"", pose.position.x, pose.position.y, pose.position.z);
    }
    diagnose(world, state, pose);

    state.rig.sync_visuals(world, pose, dt);
    state.camera.update(world, input, pose, dt);
}

/// `NFS_DIAG=1`: once a second, whether the car is standing on the city or falling through it.
///
/// The one question this binary exists to answer. A car that never grounds is a collider that was
/// not built where the road is; a car that grounds and then sinks is a narrowphase problem. Both
/// look the same from the outside and differ in these numbers.
fn diagnose(world: &World, state: &mut CruiseState, pose: nfsu2::rig::Pose) {
    if std::env::var("NFS_DIAG").is_err() {
        return;
    }
    let tick = state.t as i32;
    if tick == state.diag_tick {
        return;
    }
    state.diag_tick = tick;
    let vehicles = world.borrow::<gizmo::physics::vehicle::VehicleController>();
    let Some(v) = vehicles.get(state.rig.chassis) else { return };
    let grounded: Vec<bool> = v.wheels.iter().map(|w| w.is_grounded).collect();
    let vel = world.borrow::<Velocity>().get(state.rig.chassis).map_or(Vec3::ZERO, |v| v.linear);
    // The two things a pilot is judged on, and both are numbers rather than impressions: does it
    // stay inside the corridor the race defines, and does its progress only go up. A rival that
    // drifts out is lost; one whose progress falls has been pulled onto a crossing branch, which
    // is the failure `Pilot`'s monotone index exists to prevent.
    if let Some(c) = &state.course {
        if !state.field.is_empty() {
            let mut inside = 0;
            let mut worst = 0.0f32;
            for (rig, _) in &state.field {
                let Some(p) = rig.pose(world) else { continue };
                match c.corridor.locate(p.position) {
                    Some(f) => {
                        inside += usize::from(f.distance <= COURSE_HALF_WIDTH);
                        worst = worst.max(f.distance);
                    }
                    None => worst = f32::INFINITY,
                }
            }
            let moved = state.field.iter().filter(|(_, p)| p.passed() > 0).count();
            let junctions: usize = state.field.iter().map(|(_, p)| p.passed()).sum();
            // Where each of them actually is, once a second. A field that stops in one place is an
            // obstacle; a field that stops in seven is the driver.
            if std::env::var("NFS_FIELD").is_ok() {
                for (k, (rig, pl)) in state.field.iter().enumerate() {
                    if let Some(p) = rig.pose(world) {
                        println!(
                            "    rival {k}: ({:>7.0},{:>7.0}) {:>4.0} km/h · node {:?} · \
                             {} junctions · waypoint {}",
                            p.position.x,
                            p.position.z,
                            p.speed * 3.6,
                            pl.node(),
                            pl.passed(),
                            pl.goal()
                        );
                        if let Some(j) = pl.node().and_then(|i| state.net.node(i)) {
                            println!(
                                "        node at ({:>7.0},{:>6.1},{:>7.0}) · {:.0} m away, {:.1} m \
                                 above · links {:?}",
                                j.at.x,
                                j.at.y,
                                j.at.z,
                                (Vec3::new(j.at.x, 0.0, j.at.z) - Vec3::new(p.position.x, 0.0, p.position.z)).length(),
                                j.at.y - p.position.y,
                                j.links
                            );
                        }
                    }
                }
            }
            let lead = state.field.first().and_then(|(r, _)| r.pose(world));
            let (lp, ls) = lead.map_or((Vec3::ZERO, 0.0), |p| (p.position, p.speed));
            println!(
                "field  {} rivals · {inside} inside the corridor · furthest {worst:.0} m · \
                 {moved} moving · {} driving · {junctions} junctions taken · lead at \
                 ({:.0},{:.0}) {:.0} km/h, waypoint {}",
                state.field.len(),
                state.driving,
                lp.x,
                lp.z,
                ls * 3.6,
                state.field.first().map_or(0, |(_, p)| p.goal()),
            );
        }
    }
    let (med, p95) = frame_ms(&state.frames).unwrap_or((f32::NAN, f32::NAN));
    println!(
        "diag  pos ({:+.1},{:+.2},{:+.1})  vel ({:+.2},{:+.2},{:+.2})  speed {:+.1} km/h  cell {:?}           grounded {grounded:?}  frame {med:.1}/{p95:.1} ms ({:.0} fps)",
        pose.position.x, pose.position.y, pose.position.z,
        vel.x, vel.y, vel.z,
        v.current_speed_kmh,
        city::cell_of(pose.position),
        1000.0 / med,
    );
}

fn ui(world: &mut World, state: &mut CruiseState, ctx: &egui::Context) {
    let speed = world
        .borrow::<gizmo::physics::vehicle::VehicleController>()
        .get(state.rig.chassis)
        .map(|v| v.current_speed_kmh.abs())
        .unwrap_or(0.0);
    let s = &state.stats;
    egui::Area::new(egui::Id::new("city"))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(24.0, 24.0))
        .show(ctx, |ui| {
            ui.label(format!("{} obje · {} mesh · {} backdrop", s.objects, s.meshes, s.backdrop));
            ui.label(format!("{} hücre · {} üçgen ({} sürülebilir)", s.cells, s.triangles, s.drivable));
            if state.free_roam {
                ui.label(format!(
                    "SERBEST DOLAŞIM · free roam · {} nokta (NFS_SPOT=0..{})",
                    state.spots,
                    state.spots.saturating_sub(1)
                ));
            }
            if let Some((med, p95)) = frame_ms(&state.frames) {
                ui.label(format!("{med:.1} ms · p95 {p95:.1} ms · {:.0} fps", 1000.0 / med));
            }
            ui.label("W/S · A/D · Space · R · F konumu yazdırır");
        });
    // The warning is the point of the grace period — a countdown nobody sees is just a delay.
    if state.out_for > 0.0 || state.heading_out {
        egui::Area::new(egui::Id::new("oob"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
            .show(ctx, |ui| {
                if state.out_for > 0.0 {
                    let left = (OUT_OF_BOUNDS_GRACE - state.out_for).max(0.0);
                    ui.heading(format!("Haritanın dışındasın · {left:.0} sn"));
                    ui.label("Out of bounds — turn back");
                } else {
                    ui.heading("Haritanın kenarına gidiyorsun");
                    ui.label("Heading off the map");
                }
            });
    }
    // Where the race is, when there is one. The distance is shown rather than a bare on/off,
    // because a corridor half-width is a judgement and a number lets it be argued with.
    if let Some(c) = &state.course {
        egui::Area::new(egui::Id::new("course"))
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(24.0, -24.0))
            .show(ctx, |ui| {
                ui.label(format!("rota {} · koridor ±{COURSE_HALF_WIDTH:.0} m", c.name));
                match c.fix {
                    None => ui.label("Bu yarışın ağı buraya ulaşmıyor · off the race network"),
                    Some(f) => ui.label(format!(
                        "hat {} · {:.0} m · mesafe {:.0}",
                        f.path, f.distance, f.progress
                    )),
                };
            });
        if c.off_for > 0.0 {
            egui::Area::new(egui::Id::new("offcourse"))
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 140.0))
                .show(ctx, |ui| {
                    if c.off_for >= OFF_COURSE_GRACE {
                        ui.heading("Parkur dışındasın");
                        ui.label("Off the course — get back on the road");
                    } else {
                        ui.label(format!("parkur dışı · {:.1} sn", c.off_for));
                    }
                });
        }
    }

    egui::Area::new(egui::Id::new("spd"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-30.0, -30.0))
        .show(ctx, |ui| {
            ui.heading(format!("{speed:.0} km/h"));
        });
}
