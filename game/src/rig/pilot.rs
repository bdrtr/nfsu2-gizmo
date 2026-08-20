//! The driver that is not a keyboard.
//!
//! [`Driver`](super::Driver) turns key presses into [`Controls`]; this turns a **course** into the
//! same thing, so a rival goes through the identical `CarRig::drive` the player does. Nothing here
//! touches physics: a pilot that could push its car would be a different car from the one being
//! raced against, and the first thing anyone would ask is whether it was cheating.
//!
//! ## What it follows, and why not a line
//!
//! There is no race line in the data — `ROADMAP.md` carries the two readings that were tried for
//! one and refuted. There is a **network** ([`Network`]) and there are **waypoints**: the event
//! outline, far too coarse to drive at a median step of 425 m, but with road under 14 of its 17
//! corners and in lap order.
//!
//! So a pilot walks the network toward the next waypoint. At every junction it takes the branch
//! that gets it nearer, and it refuses to turn round. That drives the course the event describes
//! over roads the file says exist, which is a weaker claim than "the race line" and the one the
//! data supports.
//!
//! ## Pure pursuit over that
//!
//! Steering is the oldest rule there is: aim at a point some way ahead and turn at it. The state
//! that matters is *which node* the car is at, advanced only when the car has passed it, so a
//! route that crosses itself — Bayview is a city, its routes cross constantly — cannot hand the
//! driver the wrong branch when two roads pass within a few metres.
//!
//! ## What is deliberately not modelled
//!
//! No racing line and no braking points: a rival drives the middle of the road at a speed the corner
//! allows. It does now keep a following distance — see `TRAFFIC_LOOK_PER_SPEED` — because having no
//! traffic model at all turned out to be measurable rather than merely untidy, but it will not go
//! round a slower car, so a queue behind one stays a queue.

use super::drive::Controls;
use crate::world::{Ground, Network, Walls};
use gizmo::prelude::*;

/// How high the question "is something standing in the way" is asked at, and how finely the walk
/// between the car and its aim follows the ground under it.
///
/// The same half metre and the same 3 m as everywhere else this city is walked: half a metre
/// clears a kerb and every crest that is road, and 3 m is the spacing at which a walk still sees
/// the ground it is walking on.
const WALL_LIFT: f32 = 0.5;
const WALL_STEP: f32 = 3.0;
/// How many ticks a way-round decision is kept before it is worked out again.
const WALL_EVERY: u8 = 10;
/// How many multiples of the shift the search will widen to before giving up.
///
/// It gives up rather than going wider because past a point the aim is no longer on the same road,
/// and a pilot that has run out of ways round is exactly the case the stall-and-reverse rule below
/// already handles.
const WALL_WIDEST: f32 = 4.0;
/// How long a geometric escape steers the car, in seconds.
///
/// **Swept twice, and the first sweep is why the second one works.** Steering at the most
/// open heading refuted: 799 waypoints with the rule off against 781 / 757 / 742 / 737 at
/// 1, 2, 3.5 and 6 s — monotonically worse, while `away` went 63/64 to **64/64**. The
/// mechanism was freeing every car and then driving it off the course. Weighting the
/// opening by the direction the car actually needs to go turns the same sweep round:
/// **802 / 827 / 833 / 838**, still 64/64 away, distinct nodes 1247 → 1329.
///
/// 3.5 is the middle of the 2-6 s plateau and the best of it on the tiebreaker: most
/// distinct nodes (1329), fewest cars stopped before t=30 (15 against 18), one car off the
/// world. Six seconds covers five more waypoints and gives back both of those.
const ESCAPE_FOR: f32 = 3.5;
/// Headings probed for a way out, how far each is probed, and the height window the ground
/// is allowed to move through along one — the same 8 m `drop_walled` settled on, for the
/// same reason: a straight probe over a crest is not a hole.
const ESCAPE_RAYS: usize = 12;
const ESCAPE_REACH: f32 = 20.0;
const ESCAPE_SLACK: f32 = 8.0;
/// How much clear ground a heading needs before it counts as a way out, and how near the
/// escape point counts as having arrived.
const ESCAPE_MIN: f32 = 8.0;
const ESCAPE_ARRIVED: f32 = 4.0;
/// How far sideways the aim moves per step of that search, in metres.
///
/// Swept over eight routes, and **seven of the eight settings beat having no rule at all** on
/// waypoints driven past — 615, 630, 653, 663, 584, 642, 636 at 2, 4, 5, 6, 7, 8 and 10 m against
/// **605** with the rule off. So the gain is the rule and not the number.
///
/// Five rather than the peak. Six covers most (663) but its neighbour at seven is a hole (584,
/// below the baseline), and the peak beside a hole is what a tuned number looks like. Five is
/// within 1.5 % of it, takes the **most junctions of the whole sweep (1,358) over the most
/// distinct nodes (980)** — which is the tiebreaker between real progress and a car going round in
/// circles — drops nobody off the world, and has good values on both sides of it (630 at four,
/// 663 at six). The hole at seven is two routes going badly at once rather than a trend:
/// `Paths4001` falls 141 → 117 and `Paths4002` 27 → 22, and both recover at eight.
const AIM_CLEAR: f32 = 5.0;

/// How far ahead to aim, as a multiple of speed in m/s, and the bounds that keeps it inside.
///
/// Speed-proportional because a fixed distance is wrong at both ends: short enough to be accurate
/// at 30 km/h is a violent oscillation at 150, and long enough to be smooth at 150 cuts every
/// corner at 30.
/// Seconds of travel the pilot aims ahead.
///
/// **Swept over eight routes and 0.9 holds.** Longer looked promising and was not: 1.5 gives 724
/// waypoints driven past and 1.8 gives 858, against **869** here, with distinct nodes agreeing
/// (1 122 and 1 276 against 1 364).
///
/// The way it looked promising is worth more than the result. A four-route screen — 4001, 4121,
/// 4041, 4102 — had 1.8 winning by 11 % (587 against 528), and the curve across 0.6…5.0 was
/// cleanly unimodal with its peak right there. It was not noise: 1.8 really does gain **+59** on
/// those four and really does lose **−70** on the four that were not screened. The subset was
/// unrepresentative, which a subset is free to be. Screening on half the routes to save time has
/// now produced a false positive twice in a row (see the steering rate above); it is not a cheap
/// version of the sweep, it is a different and unreliable instrument.
///
/// **Re-swept 2026-08-20 after the ring moved, and it reversed.** The 0.9 above was fitted against
/// the chord ring; with the ring pulled onto the corridor the eight-route field says:
///
/// | seconds | waypoints | time on corridor | never lost | fell | furthest |
/// |---|---|---|---|---|---|
/// | 0.6 | 1302 | 77.0 % | 33 | 3 | 5514 m |
/// | 0.9 (was) | 1317 | **78.9 %** | 32 | 3 | 5784 m |
/// | 1.3 | 1378 | 73.0 % | 16 | **0** | 5878 m |
/// | 1.5 | 1375 | 70.2 % | 12 | 5 | 5785 m |
/// | **1.8** | 1417 | 77.7 % | 19 | 1 | **6023 m** |
/// | 2.4 | **1433** | 72.0 % | 15 | 7 | 5377 m |
///
/// +100 waypoints over 0.9 — two and a half times the sweep's measured noise floor of ±37 — with
/// the biggest single route +86, so it is a field result rather than one route's. `furthest` adds
/// +239 m and the falls go 3 → 1.
///
/// **Time on the corridor looks like it disagrees and cannot.** The field mean falls 1.2 points,
/// and *all* of that is `Paths4081` collapsing 99.1 % → 53.6 %; five of the eight routes improve,
/// three of them hugely (`Paths4102` +19.0, `Paths4061` +10.3, `Paths4121` +7.0). A margin of 1.2
/// against a single route's 45.5 is carried by definition.
///
/// **The cost is real and is one corner.** `Paths4081` has a cliff between 0.9 and 1.3 — 99.1 %,
/// then 50.3, 49.6, 53.6, 48.1 — and its cars leave at `(−354, −180)` at 48 km/h with the aim
/// 33 m away at 52°, cutting a corner the shorter lookahead took. That corner is the next thing
/// to look at, not a reason to keep a constant the field has moved away from.
const LOOKAHEAD_PER_SPEED: f32 = 1.8;
const LOOKAHEAD_MIN: f32 = 12.0;
const LOOKAHEAD_MAX: f32 = 40.0;

/// Below this speed an escape drives on full throttle, because the lift is what keeps it stuck.
///
/// **The escape was re-arming itself, and the arithmetic was already written in [`CORNER_LIFT`].**
/// At full lock the pedal goes to `1 − 0.85·0.75 = 0.36`, about 0.29 m/s² from rest, which in
/// [`STALL_FOR`]'s 1.5 s reaches 0.44 m/s against a [`STALL_SPEED`] of 0.7 — the car cannot clear
/// its own stall threshold. An escape aims at the clearest ground it can find, which for a car
/// facing a wall is behind it, which is full lock, which is 0.36 of pedal, which is another stall.
/// Traced on `Paths4021`: **eighteen escapes and three metres moved**, still for 56 % of the race,
/// on flat ground with eleven of twelve directions clear.
///
/// The lift is right for a *corner*, where the car has speed to lose. An escape at rest has no
/// corner and nothing to lose, so below this speed it is not applied. Above it — an escape that
/// begins while the car is still rolling — the corner reading is the right one and the lift stays.
///
/// **Swept over eight routes**, and the narrowing is what makes it keepable:
///
/// | escape throttle | waypoints | never lost the course | fell off the world |
/// |---|---|---|---|
/// | lifted as always | 1 000 | **33 / 64** | **4** |
/// | full, at any speed | 1 038 | 27 / 64 | 7 |
/// | **full below 2 m/s** | **1 035** | 30 / 64 | 6 |
///
/// The gain lands exactly where the mechanism said it would — `Paths4021` +17 and `Paths4041` +19,
/// the two routes with cars standing still — and **the whole of the extra falling is 4041's two
/// cars**. That cost is named rather than waved away: this install ships **no barriers at all**
/// (`world::collide::Bounds`), so a car that starts moving again eventually finds an unfenced edge.
/// It is not the rear-lock cap's failure in disguise — that one was refuted because clamping the
/// wheel stopped cars turning *away* from an edge, a driving defect; this is cars driving further
/// on a map with nothing at its rim, which is the standing barrier item and not this rule's doing.
///
/// `NFS_ESCFULL=0` puts the lift back everywhere.
/// **Read route by route, the threshold is nothing and the rule is everything.** Not lifting at
/// all while escaping is solid: +35 waypoints over lifting, larger than any single route's share
/// of it. The 2 m/s *threshold* is not: full throttle always scores 1038 against this one's 1035,
/// keeps 3 fewer cars on course and falls once more — three measures, two for the threshold, all
/// three inside the routes' own spread. It stays because it is the incumbent, not because it won.
const ESCAPE_FULL: f32 = 2.0;

/// Past this angle the aim point has no usable side, so the side already chosen is kept.
///
/// **A point dead behind has no side, and pure pursuit tosses a coin about it every tick.** Traced
/// on `Paths4021` with `NFS_LOST=1`: a car stands on flat open ground — 169 of 169 cells of a 13×13
/// ground scan have surface, eleven of twelve directions clear past 8 m, four wheels down carrying
/// its full weight — inside an escape whose target is **20 m at 180°**, and the wheel reads
/// `−0.85, +0.85, +0.85, −0.85, …` while the pedal alternates `0.36, −0.70`. `atan2` returns +179°
/// or −179° depending on which side of the nose a millimetre of drift puts the target, so `want`
/// slams from one lock to the other and the car rocks in place. Its own summary: **seventeen
/// escapes, zero metres moved**, still for 52 % of the race, and 0.03 of throttle asked for while
/// standing.
///
/// The escape is not at fault — reversing out is exactly its job, and where it points is the
/// clearest ground it could find. What has no answer is the *steering law*: pure pursuit cannot
/// express "turn to 180°", so it thrashes. Past this angle the side is held instead of recomputed.
///
/// **Swept over eight routes**, and the width matters more than the idea:
///
/// | held past | waypoints | furthest | never lost the course |
/// |---|---|---|---|
/// | off | 986 | 5 413 m | 32 / 64 |
/// | 150° | 980 | **5 522 m** | **34 / 64** |
/// | **170°** | **1 000** | 5 381 m | 33 / 64 |
///
/// 170° is the one that wins the measure a lost car cannot inflate, and it wins it *quietly*: four
/// routes better, two worse by one and two, two unchanged. 150° reaches further and keeps two more
/// cars but pays 33 waypoints on `Paths4102` alone — a wide guard commits the wheel in ordinary
/// three-quarter turns as well, and those are turns pure pursuit can do perfectly well.
///
/// `NFS_BEHIND=0` restores the coin toss.
/// **Read route by route, holding a side is solid and this angle is not.** Against no rule at all
/// the field gains +14 and no single route moves it by more than 8 — a real win. Against 150° it
/// gains +20, but `Paths4102` alone is −31, more than the whole margin. Between 150° and 170°
/// the eight-route field cannot choose; see `ROADMAP.md`, 2026-08-20.
const BEHIND: f32 = 2.97;

/// The lateral acceleration the course's own corners are taken at, in m/s². **0: refuted.**
///
/// Measured, not chosen: the 90th percentile of the field's own cornering above 28 km/h is
/// 5.2 m/s² (`ROADMAP.md`, 2026-08-20). `v = sqrt(a·r)` turns the ring's radius into a speed.
///
/// | | waypoints | on the corridor | on their side | furthest | lost the line |
/// |---|---|---|---|---|---|
/// | **off** (kept) | — | **73.8 %** | **0.8 %** | **5951 m** | 40 |
/// | 5.2 | **−153** | 69.7 % | 1.6 % | 5667 m | **44** |
/// | 8 | −28 | 72.0 % | 1.0 % | 5790 m | 39 |
///
/// **It loses at both settings, and it fails at the place that asked for it.** `Paths4121` is
/// where eight cars meet a 13 m radius at 68 km/h; braking for it takes that route **−55**. So
/// the corner the ring describes is not the corner the road has: the pull moves waypoints one at
/// a time and leaves kinks, and a 13 m radius in a city street is a kink, not a hairpin. Smoothing
/// those kinks away was measured separately and is also neutral, which closes the loop — **the
/// ring's radii are not trustworthy enough to brake on, and making them smoother does not make
/// them true.**
///
/// Fourth refusal of a "do less" lever this day, and the sharpest: it was aimed at a place where
/// the arithmetic said the corner was impossible, and slowing for it still lost.
const RING_HELD: f32 = 0.0;

/// Steering lock the pilot will ask for, as a fraction of the controller's own.
const STEER_LIMIT: f32 = 0.85;

/// How much of the steering lock is left at [`CAP_SPEED`], as a fraction. **1.0: refuted.**
///
/// The flat cap was refuted because it takes the lock away from a *standing* car too, and a car
/// that cannot turn cannot turn away from an edge — falls tripled. Scaling with speed fixes
/// exactly that, and the fix works: over the eight routes falls stay at 6, 6, 6 and 8 against the
/// flat cap's 5 and 6 from a baseline of 2. The idea still loses, at every setting swept:
///
/// | at 60 km/h | waypoints | time on course | never lost | fell |
/// |---|---|---|---|---|
/// | **no cap** (kept) | **1046** | **82.0 %** | **30** | **6** |
/// | 70 % | 872 | 74.2 % | 26 | 6 |
/// | 50 % | 861 | 76.0 % | 27 | 6 |
/// | 35 % | 682 | 70.7 % | 26 | 8 |
///
/// Solid by the route-spread rule: −174, −185 and −364 with the biggest single route −107, −82 and
/// −120, and the baseline ahead on seven, five and seven of the eight.
///
/// **Two independent experiments now say the same thing.** Pure pursuit's own geometry, which asks
/// for less lock when the aim swings wide, lost by 207; this, which asks for less lock at speed,
/// loses by 174 to 364. Every way of making this pilot steer *less* is worse, and the reason is
/// the one written at the steering law: it does not track a path, it chases a point, and the
/// over-steering is how it gets back. The lever that is left is not the steering response.
const CAP_FAST: f32 = 1.0;

/// The speed, in m/s, at which [`CAP_FAST`] is reached. 16.7 m/s is 60 km/h.
const CAP_SPEED: f32 = 16.7;

/// The driven car's wheelbase, in metres — the length pure-pursuit geometry turns a curvature into
/// a steering angle with.
///
/// A constant here and a measurement everywhere else: the sim's own `car ready` line reports the
/// 240SX at 4.39 m long, and `WheelFit` puts its axles 2.6 m apart. Every car in the eight-route
/// sweep is that car, so nothing in what is measured depends on the difference — but a pilot
/// driving a bus would want the bus's number, and the honest place for it is the car rather than
/// this file. Wire it through when a second car is driven.
const WHEELBASE: f32 = 2.6;

/// Below this, in m/s, a car asking for throttle is not driving.
const STALL_SPEED: f32 = 0.7;
/// How long it has to be true before the pilot tries something else.
const STALL_FOR: f32 = 1.5;
/// How long a car is left alone at the start before it can be called stuck.
const SETTLE: f32 = 3.0;
/// Backwards speed, in m/s, above which a car is held to be moving rather than stalled. 0 turns
/// the exemption off, which is the shape the stall rule was measured in before it existed.
///
/// **Swept over eight routes and every setting beats having no exemption at all** on waypoints
/// driven past: 840, 869, 860 and 851 at 0.2, 0.5, 1.0 and 2.0 m/s against **833** with it off,
/// and distinct nodes agree (1345 / 1364 / 1342 / 1335 against 1329). The curve rises to 0.5 and
/// falls away either side, which is what a threshold should look like when it is finding a real
/// boundary rather than being tuned: too small and a car still crawling backwards is condemned,
/// too large and one genuinely stuck against something gets to hide behind a trickle of reverse.
const STALL_BACK: f32 = 0.5;
/// How near the waypoint being held counts as having reached it, in metres.
///
/// Swept over eight routes and **all eight settings beat having no second arm at all** on waypoints
/// driven past: 707, 703, 780, 799, 788, 678, 697 at 8, 12, 15, 18, 20, 25 and 40 m against **653**
/// with the arm off. 15-20 is a plateau (780 / 799 / 788) bounded by 703 below and 678 above, and
/// eighteen is its middle as well as the best of it — most course covered, furthest driven, most
/// distinct nodes, and the cars that stop before t=30 s down from 30 of 64 to 18.
///
/// **The ceiling has a reason, which is why the plateau ends where it does.** Waypoints are 40 m
/// apart — the step the course is densified to, see [`crate::world::densify`] — so at *half* of that the two arms
/// meet: on an evenly spaced course, being within 20 m of the one you hold already means the next
/// one is further, which is the first arm's own test. Past half the rule stops saying "reached" and
/// starts saying "skip", and the numbers follow — 678 at 25 m, and 40 m drops four cars off the
/// world. Eighteen is just inside that. **It is tied to the spacing, not absolute**: change
/// `NFS_WPSTEP` and this wants moving with it.
const REACHED: f32 = 18.0;
/// How near the held waypoint has to be before "I have driven past it" is allowed to release it.
///
/// **The third way to be done with a waypoint, and the one the other two cannot see.** Measured on
/// `Paths4102` with `NFS_LOST=1`: the whole field drives *on the line* at 88 km/h past waypoint 11,
/// forty metres to the side of it, and the counter sticks — the held one bottoms out at 40 m, well
/// outside [`REACHED`]'s eighteen, and the next one is 81 m away and stays further for the whole
/// pass, so "is the next nearer" never fires either. The waypoint ring and the carriageway the car
/// is on are different paths of the same route; the corridor merges them and says "on course", the
/// counter does not. The goal then sweeps from −61° to −140°, the aim walk follows it backwards,
/// and the pilot asks for full lock at 88 km/h — which the trace says is the field's single largest
/// loss of course, 28 of the 50 cars that lose it.
///
/// **Past it along the road, not merely behind the nose**, and that distinction is the rule. The
/// first cut asked whether the waypoint had fallen behind the *car*, which cannot tell "I drove by
/// on the next carriageway" from "I swung wide at a corner and it went behind my shoulder": it
/// gains 127 waypoints over five routes and loses **120 on `Paths4121` alone**, where the cars are
/// handed a goal across the corner they were already failing at and stop dead against it, six
/// waypoints each against twenty-six.
///
/// **And near it**, which is this constant: a car far from the waypoint it holds is a lost car, and
/// the one thing this arm must never do is walk the ring for one — the failure that refuted
/// "advance while the held one is behind" (92 laps and 5,999 waypoints for a single car). Swept
/// over eight routes:
///
/// | bound | waypoints | never lost the course | fell off the world |
/// |---|---|---|---|
/// | off | 927 | **22 / 64** | 2 |
/// | 30 m | 883 | 14 / 64 † | 4 |
/// | **60 m** | **986** | **32 / 64** | 4 |
/// | 80 m | 973 | 23 / 64 † | 3 |
/// | 100 m | 956 | 24 / 64 † | 4 |
///
/// † counted before `nfs_sim` stopped calling a car "lost the course" when it had never been on it
/// — two grids sit outside the corridor and eight cars were being written off at t = 3 s, so those
/// three rows understate by up to eight. The `off` and `60 m` rows are re-measured; the waypoint
/// column, which is what decides, was never affected.
///
/// Unimodal, with thirty *below* having no rule at all — it sits under the forty metres the failure
/// actually needs, so it never fires where it would help and only ever fires close in, where
/// [`REACHED`] already had it.
///
/// **Widening past sixty was tried for a reason and it does not pay.** With the rule in, the cars
/// that still lose the course to a backwards aim hold a goal a median 66 m away — just outside this
/// bound — and seven of the sixteen are on `Paths4102` at one waypoint. Releasing at 80 m makes
/// **that route worse**, 147 waypoints to 126, and 100 m is no better. Whatever is holding those
/// cars, it is not the bound: the waypoint they are stuck on is one it costs something to let go
/// of. The weak column is the last one: two more cars off the world at
/// every setting that fires, plausibly because the field now carries speed on the course for
/// longer, but that is a guess and `fallen` is on watch.
///
/// Set `NFS_PASSED=0` to turn the arm off. **Known edge:** `w[i+1]` wraps to the first waypoint at
/// the end of a sprint, so the last waypoint's forward direction points back down the course. No
/// car has reached the end of a route in ninety seconds — the best is 25 of 130 — so it is recorded
/// rather than special-cased.
/// **Read route by route, only the wide gap is solid.** 60 beats 30 by +103 with no route
/// swinging it by more than 60 — real. But 60 beats 80 by only +13 (one route is −21) and 100 by
/// +30 (one route is −47), and it beats switching the release *off* by +75 of which `Paths4121`
/// alone is +104 — with off ahead on three of the eight. The release is measured; 60 rather than
/// 80 is not. See `ROADMAP.md`, 2026-08-20.
const PASSED_NEAR: f32 = 60.0;

/// How near a waypoint counts as having driven past it, for [`Pilot::covered`].
///
/// The waypoints are 40 m apart after `route::densify`, so this is one step: near enough that a car
/// on the road claims every one it passes, far enough that it does not have to clip the exact point.
const COVERED_WITHIN: f32 = 40.0;
/// How long a reversing manoeuvre lasts.
const BACK_FOR: f32 = 1.2;
/// The step the pilot falls back on when a caller does not say.
///
/// **It used to be assumed rather than passed, and the assumption was wrong in both callers.** The
/// doc here read "it is called once per frame and the physics runs at sixty, so this is right where
/// it matters" — but `nfs_sim` calls `drive` inside its 240 Hz physics loop, ungated, so every
/// timer in this file ran at **four times** wall clock: `STALL_FOR`'s 1.5 s fired after 0.375 s,
/// `BACK_FOR`'s 1.2 s reversed for 0.3 s, `ESCAPE_FOR`'s 3.5 s lasted 0.875 s. And `nfs_cruise`
/// calls it once per rendered frame at 100-126 fps while advancing 1/60 per call, so the game ran
/// its own different multiple, ~1.7-2.1x. The sim was not measuring the game.
///
/// The evidence was in this file's own recorded measurements the whole time and went unread: 4002
/// was logged as "35 s of a 90 s race spent in reverse and 115 escapes", and 35/115 = **0.304 s**
/// per reverse against a `BACK_FOR` of 1.2 — exactly a quarter. At the assumed rate one
/// stall→reverse cycle costs at least `STALL_FOR + BACK_FOR` = 2.7 s, so 90 seconds holds 33 of
/// them; 115 was arithmetically impossible. Likewise "126 escapes" at `ESCAPE_FOR` 3.5 s is 441
/// seconds of escaping inside a 90-second race.
///
/// `drive` now takes `dt` and the callers pass their own step, so both are right and they agree
/// with each other. This constant remains only as the fallback for a caller with nothing better.
const TICK: f32 = 1.0 / 60.0;

/// The speed, in m/s, at which full lock is as much as the car will hold. Above it, braking.
///
/// Swept over eight routes on all three measures at once — cars that got away, cars that fell off
/// the world, and distance covered by the ones still on it. No brake gives 41/64 away, 20 fallen
/// and 2,404 m; 20 m/s gives 41, 18 and 2,418; 14 gives 41, 17 and 2,354; **9 gives 44, 13 and
/// 2,594**; 6 gives 41, 14 and 2,242. Nine wins on every column, which is rarer than it sounds and
/// is why it is the default.
/// **Re-swept 2026-08-20 on the pulled ring, and 9 stays — but the reason is the roll, not the
/// waypoints.** The departure census had pointed straight here: of the 49 cars that leave the
/// course, **49 % are braking hard** and 41 % are above 60 km/h when they cross the edge, which is
/// this constant's own subject.
///
/// | at 60 km/h the limit is | waypoints | on the corridor | on their side | furthest |
/// |---|---|---|---|---|
/// | 6 (brake early) | −83 | 69.4 % | 1.3 % | 5727 m |
/// | 7.5 | −14 | 73.6 % | **0.7 %** | 5790 m |
/// | **9** (kept) | — | 73.8 % | 0.8 % | 5951 m |
/// | 10.5 | +44 | 74.6 % | 1.8 % | 6251 m |
/// | 12 (brake late) | +70 | **74.5 %** | 1.6 % | **6217 m** |
///
/// Braking later really does gain: +44 and +70 waypoints, and 300 m of `furthest`. Two things
/// stop it. The gains are **carried** — 10.5's biggest single route is −55 against a +44 margin
/// and 12's is +76 against +70 — and the cost is uniform and large in the one column today's
/// rollover work created: **time spent lying on their side doubles**, 0.8 % → 1.8 %. A car that
/// brakes later carries more speed into the corner it was going to run wide of, and goes over.
///
/// 6 is solidly worse on every column at once, so the useful range is narrow and 9 sits in it.
/// This is the first lever measured today that buys progress by doing *less*, and it is refused
/// on a measure that did not exist this morning.
const BRAKE_SPEED: f32 = 9.0;
/// How hard the brake comes on past that.
const BRAKE_GAIN: f32 = 1.5;

/// What a corner may ask of the tyres before the brake comes on, in m/s².
///
/// **This exists because [`BRAKE_SPEED`]'s rule cannot see a corner until it is in one.** That rule
/// is `(speed / BRAKE_SPEED) * |steer|`, and the wheel only turns once the corner has arrived.
/// Traced with `NFS_LOST=1` at the place six of eight cars leave the course on `Paths4121`, it
/// peaks at **0.92** against a threshold of 1.0 — the brake never comes on at all — while the car
/// enters a 48° bend at 66 km/h. Lowering the threshold instead is separately refuted: it brakes
/// everywhere, and what is missing is not harder but earlier.
///
/// Pure pursuit follows an arc through the aim point of radius `L / (2 sin θ)`, so holding that
/// line at that speed costs `2 v² sin θ / L` of lateral acceleration. That is a number in m/s²
/// which can be compared against what a tyre has, rather than a tuned ratio — and on the same trace
/// it passes 9.6 m/s² **three and a half seconds and sixty metres** before the car goes wide.
///
/// **Swept over eight routes.** Waypoints driven past is the measure that decides it, for the same
/// reason it decides the traffic look: it cannot be inflated by a lost car.
///
/// (The "never lost the course" column here predates the fix to that counter — see
/// [`PASSED_NEAR`]; it understates by up to eight and the waypoint column does not.)
///
/// | grip | waypoints | distinct nodes | never lost the course | fallen |
/// |---|---|---|---|---|
/// | off | 883 | 1 352 | 9 / 64 | 3 |
/// | 6 | 602 | 953 | **22 / 64** | 1 |
/// | **8** | **927** | 1 280 | 14 / 64 | 2 |
/// | 12 | 815 | 1 192 | 13 / 64 | 1 |
///
/// Eight is the only value that beats the field with the rule off, and the curve is unimodal around
/// it. **Six is the trap**: it keeps 22 of 64 cars on the course and is far the worst arm, because
/// those cars stay on it by crawling — 10.2 waypoints each against 20.1 at eight and 18.8 with the
/// rule off. Keeping the car on the road is not the goal; getting it round is.
///
/// **The honest part of the record.** Per route, eight wins four (4001 +95, 4041 +26, 4102 +18,
/// 4121 +11) and loses three (4061 −48, 4021 −34, 4081 −24), so the +44 net is carried by one
/// route and a future reader is entitled to distrust it. Junctions (−6.6 %) and distinct nodes
/// (−5.3 %) also fall, and that is worth the paragraph it takes to say why it is not lost progress:
/// splitting both by whether the car ever lost the course, the cars that **stayed** on it go from 9
/// banking 187 nodes and 169 waypoints to 14 banking 279 and 282, while the lost population shrinks
/// from 55 cars to 50. The whole of the fall is fewer lost cars wandering the graph — the same
/// inflation `nfs_sim` records when it notes that a flipped car goes on taking junctions. The ratio
/// of junctions to distinct nodes is unmoved, 1.069 → 1.054.
///
/// It does **not** fix 4121: seven of eight still leave at waypoint 6, now 19 m further round the
/// bend and at 50 km/h instead of 66.
///
/// **What was written here as the next candidate is withdrawn.** It said the remainder was the
/// steering, because the pilot asks for only 0.50 of its lock at −54° and the angle-to-lock map is
/// a straight ramp rather than pure pursuit's own geometry. The arithmetic says the opposite: that
/// corner is a 21 m radius, which the correct geometry would take at about a *quarter* lock, so
/// the honest law asks for **less** steering there, not more. The corner is a speed problem and
/// this constant is the lever for it; what is left is that 8 is above anything the car can
/// actually do — measured at 5.2 m/s² at the 90th percentile of the field's own cornering
/// (`ROADMAP.md`, 2026-08-20) — which is why the brake only arrives once the car is already
/// running wide.
///
/// **Re-swept on the honest ring, and the physical value still loses.** That last sentence carried
/// an implication worth testing: if 8 only wins because the chord's angle overstates the road's
/// curvature, then on a ring walked along the network — where there is no overstatement — the
/// measured 5.2 should be the one that works. It is not. Over the eight routes under
/// `NFS_WALKLINE=1`, waypoints driven past run **1662 at 8, 1610 at 6.5, 1570 at 5**, and the
/// falls double at 5 (3 → 6) — the cars that brake harder do stay on the ring longer (15 → 18
/// never losing it) and the ring is what runs off the road.
///
/// The field total hides the shape, though. Route by route, 6.5 **beats** 8 on four of the eight
/// and ties a fifth; it loses the total on one, `Paths4061`, by −66 — more than the whole field
/// margin of −52, and spread over six of that route's eight cars rather than banked by one. So
/// this is not one value being right. It is one value being a compromise between routes that want
/// different ones, and 8 stays because it wins the deciding measure and is already in, not
/// because the sweep found it correct.
/// **And read route by route, even the chord-ring win splits in two.** 8 over 6 (+325) and over
/// 12 (+112) are solid — larger than any single route's share. 8 over *no curvature brake at all*
/// is +44 with `Paths4001` alone worth +95, and the brakeless pilot ahead on three of the eight:
/// the question "should there be this brake" is answered by one route, the question "how much"
/// by the field.
const GRIP: f32 = 8.0;

/// How much steering costs throttle.
///
/// **Ramping this in with speed is refuted, and the reasoning that led there is worth keeping
/// because it was half right.** At full lock the term takes the pedal to 1 − 0.85·0.75 = 0.36,
/// which from rest is about 0.29 m/s²; in the 1.5 s of [`STALL_FOR`] that reaches 0.44 m/s
/// against a [`STALL_SPEED`] of 0.7, so a car that has to turn hard to get going cannot clear its
/// own stall threshold. That arithmetic is correct and it is not the reason the cars were stuck —
/// [`STALL_BACK`] was. Charging the lift only above 4 or 8 m/s, measured on top of that fix,
/// gives **821** and **690** waypoints driven past against **869** with it charged flat. Full
/// pedal at full lock from rest does not rescue a car, it launches one; the two mechanisms looked
/// alike from the trace and only the sweep told them apart.
///
/// **And a third threshold, on a field twice as good, says the same.** After the escape fix
/// ([`ESCAPE_FULL`]) the trace shows the identical trap outside an escape — 0.84 of lock, 0.37 of
/// pedal, aiming 69-96 m away at −88°, stalling before an escape is even called for — so charging
/// the lift only above **2 m/s** was swept on a field scoring 1,035 rather than 869. It gains 16
/// waypoints on `Paths4021`, the route with the stuck car, and loses **56 on 4121, 35 on 4102 and
/// 34 on 4061**: 919 against 1,035. Three thresholds now, 8, 4 and 2 m/s, all refuted. The lift at
/// low speed is not the general defect it keeps looking like; it is only wrong *inside an escape*,
/// where there is no corner at all.
const CORNER_LIFT: f32 = 0.75;

/// How far ahead to look for another car, as a multiple of speed in m/s, and the floor under it.
///
/// Speed-proportional for the same reason the steering lookahead is: a following distance that is
/// right at 30 km/h is tailgating at 120. The floor is what makes a **standing** grid file out
/// instead of shunting — with no floor, a car doing nothing looks nothing up.
///
/// Swept over eight routes. The measure that decides it is **waypoints actually driven past**,
/// because that is what having no traffic model was costing and because it cannot be inflated:
///
/// | multiple | away | off the world | distance | waypoints driven past | stopped before t=30 |
/// |---|---:|---:|---:|---:|---:|
/// | none | 62/64 | 0 | 4,129 m | 462 | 39 |
/// | 0.6 | 63/64 | 2 | 3,965 | 584 | 33 |
/// | 1.2 | 63/64 | 3 | 3,401 | 526 | 36 |
/// | **2.0** | **63/64** | **0** | **3,611** | **589** | **31** |
/// | 3.0 | 63/64 | 3 | 3,612 | 587 | 34 |
/// | 4.5 | 63/64 | 3 | 3,787 | 572 | 28 |
///
/// Coverage jumps as soon as there is any following distance at all and then sits on a plateau;
/// two is the best of it. **The distance column goes the other way and that is not a surprise** —
/// `furthest` is the leading car of each route, and a leader stuck behind someone is what a
/// following distance costs. The column that says the *field* is driving the course went up 27 %.
///
/// The falls column is chaos rather than signal at this scale (0, 2, 3, 0, 3, 3); two landing on
/// zero is luck, not a property of the number.
const TRAFFIC_LOOK_PER_SPEED: f32 = 2.0;
const TRAFFIC_LOOK_MIN: f32 = 8.0;

/// How far to either side of its own line a car counts as being in the way, in metres.
///
/// The body is 1.64 m wide, so this is a car and a half: wide enough that a rival drifting in the
/// next lane registers before it is a collision, narrow enough that the car on the other
/// carriageway does not.
const TRAFFIC_HALF_WIDTH: f32 = 2.2;

/// How far sideways the aim point is pushed to get round a car in the way, in metres.
///
/// Swept over eight routes on top of the following distance:
///
/// | offset | away | off the world | distance | waypoints driven past | stopped before t=30 |
/// |---|---:|---:|---:|---:|---:|
/// | none | 63/64 | 0 | 3,611 m | 589 | 31 |
/// | **1.5** | **63/64** | **0** | **4,187** | **605** | 32 |
/// | 2.0 | 63/64 | 3 | 4,260 | 605 | 28 |
/// | 2.5 | 63/64 | 4 | 3,559 | 577 | 27 |
/// | 3.0 | 63/64 | 2 | 3,866 | 617 | 27 |
/// | 5.0 | 62/64 | 3 | 4,029 | 609 | 30 |
///
/// Coverage sits on a plateau of 605-617 for anything from 1.5 m up, so the wider offsets buy at
/// most a dozen waypoints — and cost two to four cars off the world for them. One and a half metres
/// is the only setting that moves anything without that, and it is the one with a reading behind it
/// as well as a measurement: a little over half a car's width, which is the smallest shift that
/// actually clears a car in the same lane.
///
/// It also gives back what the following distance cost. Distance covered goes 3,611 → 4,187 m,
/// above even the 4,129 the field managed with no traffic model at all — the queue was the price,
/// and going round is what stops paying it.
const PASS_OFFSET: f32 = 1.5;

/// One rival's driver.
#[derive(Clone, Debug, Default)]
pub struct Pilot {
    /// Where it is on the network.
    at: Option<u32>,
    /// Where it came from, so it does not turn round at a junction.
    from: Option<u32>,
    /// Which waypoint of the course it is heading for.
    goal: usize,
    /// Completed laps. Counted where the waypoint index wraps, which is the only place a closed
    /// course says a lap ended.
    laps: u32,
    /// Where the pilot started, so a lap is measured from the grid rather than from waypoint zero
    /// — those are different points, and using the wrong one credits a lap before the first is
    /// driven.
    line: usize,
    /// Smoothed steering, the same field [`Driver`](super::Driver) keeps and for the same reason.
    steer: f32,
    /// How many junctions it has taken. The one number that says a pilot is getting somewhere.
    passed: usize,
    /// How many *distinct* nodes it has been on. Against [`Self::passed`] this is the difference
    /// between a car making its way round a course and one going round in circles.
    seen: std::collections::BTreeSet<u32>,
    /// How long this pilot has been driving. Only used to give a car a moment to settle before
    /// it is judged stuck.
    age: f32,
    /// How long the car has been asking for throttle and not moving.
    ///
    /// A pilot with nothing else to try holds full lock and a fifteen-percent throttle against
    /// whatever it is touching, for ever. Three of eight rivals do exactly that.
    ///
    /// **It is close to neutral and the numbers say so.** Held against eight routes with the
    /// manoeuvre disabled it wins on two and loses on one: 4041 goes 5/8 to 2/8 with it, 4061 1/8
    /// to 0/8, and 4102 7/8 to 8/8. It is kept because a driver that tries to back off what it has
    /// hit is a driver and one that grinds for ever is not, not because it moved a number.
    ///
    /// Three tries that came before it are worth having written down: spreading the grid apart
    /// (5/8, 6/8, 5/8 at one, two and four times the spacing), staggering the start (5/8, 4/8, 3/8,
    /// 6/8 at nought to two seconds), and gating recovery on progress rather than time, which
    /// locked it away from the only cars that needed it.
    stalled: f32,
    /// How long is left of a reversing manoeuvre.
    backing: f32,
    /// Nodes this pilot has tried to reach and could not.
    ///
    /// **Observation rather than prediction, and that is the whole idea.** Four rules that tried to
    /// tell in advance which links a car could drive were swept and thrown out — a gradient limit,
    /// a placement cone, a speed-scaled steering lock and a kerb-step test — and each cut real roads
    /// faster than it cut obstacles, because any test sharp enough to catch a half-metre kerb also
    /// catches every crest that is road. A car that has actually failed to get somewhere has
    /// evidence no geometric test has.
    ///
    /// Small and unbounded on purpose: a route has a few dozen junctions and a pilot that gave up
    /// on a dozen of them has bigger problems than the memory.
    ///
    /// Measured over eight routes it is the largest single gain the drivers have had: cars that got
    /// away 32/64 → **41/64**, distance covered 6.5 km → **7.9 km**, and it lands hardest exactly
    /// where the geometric rules could not — the route where nothing moved at all goes from 2 cars
    /// and 40 m to 5 and 170, and another from 0 and 29 m to 2 and 264.
    blocked: Vec<u32>,
    /// Which waypoints of the course the car has actually been near.
    ///
    /// **A progress measure a resync cannot inflate.** `along` is the waypoint *index*, so a pilot
    /// that re-finds the course by jumping its index forward is credited with everything in
    /// between; measured, that turns 5.5 waypoints of real progress into a reported 122.8. This can
    /// only be earned by having been there. Proximity is refuted as a rule for *advancing* — a car
    /// that strays never enters the radius, so the counter stops while the car drives on — and is
    /// exactly right for *counting*, for the same reason read the other way round.
    covered: std::collections::BTreeSet<usize>,
    /// Seconds still to wait before pulling away.
    ///
    /// A grid is four abreast and two deep with 3.5 m across and 5.7 m between the rows, against a
    /// car 1.64 m wide and 4.39 m long — so 1.3 m of clear air front to back. Eight cars given full
    /// throttle at once drive into each other, and a pilot with no traffic model cannot get out of
    /// that: measured, three of eight never leave the grid, and which three changes with the
    /// spacing rather than staying with a slot.
    hold: f32,
    /// The sideways shift the aim is carrying to get round something standing in the way, in
    /// metres, positive to the car's left.
    ///
    /// Held between evaluations rather than worked out every step. The search costs a handful of
    /// segment tests, but that is not the reason: a decision that flips side from one frame to the
    /// next is a steering wobble rather than a way round, and the thing being gone round is a
    /// hundred metres of concrete that will still be there in a tenth of a second.
    wall_shift: f32,
    /// Ticks left before the shift is worked out again.
    wall_due: u8,
    /// How many times giving up on a node has re-picked another, and how many of those pointed
    /// within 45° of the one abandoned — that is, sent the car the same way again.
    swaps: usize,
    swaps_same: usize,
    /// Where to drive while escaping, and for how long — a point in the world, not a node.
    ///
    /// **The graph has no answer here and that is measured, not assumed.** Three ways of
    /// choosing a different *node* are refuted in `ROADMAP.md` (blacklist the node, shun its
    /// heading, expire the list), and the reason they all fail is the same: they pick from
    /// branches that all lead back to the obstruction, and 92 % of the alternatives are
    /// already crossed off. Meanwhile the stuck car is standing in open ground — probed at
    /// the moment it gives up, **six to ten of twelve directions are clear for the full
    /// twenty metres of the probe**. So the escape stops asking the graph and asks the city.
    escape: Option<(Vec3, f32)>,
    /// How many escapes were started, and how far the car actually moved during them.
    ///
    /// The rule cut the early-stopper count 18 → 15 and the field's coverage 799 → 833, but
    /// the "nothing else explains it" bucket stayed at eight cars — so for those it is either
    /// not firing or firing and failing, and those want opposite fixes.
    escapes: usize,
    escape_moved: f32,
    /// Where the current escape started, so its displacement can be measured when it ends.
    escape_from: Option<Vec3>,
    /// Links the node being re-picked from had, summed over give-ups, and how many of them pointed
    /// away from the abandoned one. The difference between "no way out" and "no way that is
    /// different".
    swap_links: usize,
    swap_options: usize,
    swap_free: usize,
    /// The point the steering is actually aimed at, kept only so a harness can see it.
    ///
    /// Where a car is going and which node it holds are different facts, and reading one for the
    /// other cost a session: a trace showing the held node stuck and the car curving away from it
    /// looks like a car ignoring its pilot, and is not — the aim walks *past* the held node, so the
    /// two are supposed to disagree.
    aim: Option<Vec3>,
}

impl Pilot {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Put the pilot on the network at the node nearest the car.
    ///
    /// `facing` is taken and not used, and that is deliberate. Every car on a starting grid is
    /// within a few metres of every other, so nearest puts all eight pilots on the **same** node —
    /// and that node is off to one side of a grid four cars wide. The five that got away were 4.5
    /// to 11.0 m from it and the three that did not were 13.5, 14.3 and 16.4: the failure sorts
    /// perfectly by distance, which looks like a diagnosis and is not one.
    ///
    /// Restricting the choice to a cone in front of the car was the obvious cure and is **refuted**
    /// — swept over half-angles from 90° to 25°, it took the field from five cars away to three,
    /// three, two and two. The correlation is real and the intervention it suggests is wrong, so
    /// the parameter is gone rather than left at a value that reads as tuned.
    pub fn place(&mut self, at: Vec3, facing: Vec3, net: &Network, course: &[Vec3]) {
        let _ = facing;
        self.at = net.nearest(at);
        self.from = None;
        self.passed = 0;
        self.age = 0.0;

        // **Start at the waypoint nearest the grid, not at waypoint zero.** An event outline is a
        // closed ring drawn from wherever its author began, and nothing puts that beginning near
        // the start line. Measured over eight races, how well the field drives sorts *perfectly* by
        // how far waypoint 0 happens to be from the grid: 13 m and 10 m on the two that worked
        // (8/8 and 5/8 cars away), 33 m and 75 m on the two that half worked, and 160, 493, 785 and
        // 844 m on the four where nothing moved at all. A pilot aiming at a point half a kilometre
        // off the course drives off the course, from the first frame.
        self.goal = course
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let d = |p: &Vec3| (p.x - at.x).powi(2) + (p.z - at.z).powi(2);
                d(a.1).total_cmp(&d(b.1))
            })
            .map_or(0, |(i, _)| i);
        // And then the *next* one, because the nearest is the one the grid sits on: aiming at where
        // you already are is a car that turns in place.
        if !course.is_empty() {
            self.goal = (self.goal + 1) % course.len();
        }
        self.line = self.goal;
        self.laps = 0;
        self.blocked.clear();
        self.seen.clear();
    }

    /// Hold this pilot on the line for `seconds` before it pulls away.
    pub fn hold_for(&mut self, seconds: f32) {
        self.hold = seconds;
    }

    /// How many junctions it has driven through.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.passed
    }

    /// How many distinct nodes it has visited.
    #[must_use]
    pub fn seen(&self) -> usize {
        self.seen.len()
    }

    /// Which node of the network it is on.
    #[must_use]
    pub fn node(&self) -> Option<u32> {
        self.at
    }

    /// The point the steering is aimed at, once it has driven a frame.
    #[must_use]
    pub fn aim(&self) -> Option<Vec3> {
        self.aim
    }

    /// Where an escape manoeuvre is driving it, if one is running.
    ///
    /// An escape **overrides the graph's aim** for its duration, so a diagnostic that reads
    /// [`Self::aim`] alone cannot tell a pilot following the road from one reversing out of a
    /// corner — and the two look identical in a trace right up to the point where one of them is
    /// pointing backwards. This is the bit that separates them.
    #[must_use]
    pub fn escaping(&self) -> Option<Vec3> {
        self.escape.map(|(to, _)| to)
    }

    /// Which waypoint it is heading for.
    #[must_use]
    pub fn goal(&self) -> usize {
        self.goal
    }

    /// The nodes this pilot has given up on — see the [`Self::blocked`] field.
    ///
    /// Reported because the harvest turned out to be worth counting and worth **not** trusting: 369
    /// nodes over eight routes, 63 of 64 cars leaving at least one, and sharing the ones several
    /// cars agree on is refuted (`ROADMAP.md`).
    ///
    /// **Making it forget is refuted too, and that is the surprise.** At a give-up the node being
    /// re-picked from has 4.22 ways out, 1.56 of them point somewhere other than the one abandoned,
    /// and only **0.12** of those are not already on this list — so 92 % of a car's alternatives are
    /// nodes it crossed off earlier, and the list looked like the thing strangling the escape. It is
    /// not: expiring entries after 3, 6, 12 and 25 s loses on every column at every setting —
    /// waypoints 799 → 785 / 758 / 787 / 773, cars away 63/64 → 60-61, distance 5,394 → 5,074-5,218
    /// — while junctions leap to 2,038-2,551 against 1,566 with distinct nodes unmoved. That pairing
    /// is this project's signature for oscillation rather than progress: a car returns to a node it
    /// gave up on, fails there again, and gives up again. The permanence is load-bearing.
    #[must_use]
    pub fn given_up(&self) -> &[u32] {
        &self.blocked
    }

    /// How many times it gave up on a node and took another, and how many of those took it the
    /// **same way again** — the cost of the escape and its yield, side by side.
    ///
    /// Measured because the second number turned out to be almost the first: 94-100 % of the
    /// re-picks point within 45° of the node just abandoned, so a car backs off and drives at the
    /// same thing again. It is structural rather than bad luck — `Network::step_avoiding` ranks by
    /// nearness to the same waypoint, and a blacklist takes away one *node* while the direction
    /// stays exactly as attractive as it was.
    #[must_use]
    pub fn swaps(&self) -> (usize, usize) {
        (self.swaps, self.swaps_same)
    }

    /// How many geometric escapes were started, and the total ground covered during them.
    ///
    /// The pair is the point. An escape that never fires and one that fires constantly and gets
    /// nowhere are the same zero in every summary, and they want opposite fixes — the first a
    /// looser trigger, the second an entirely different mechanism. Measured on 4002 the answer was
    /// 44-126 escapes a car for 0.0-5.5 m, which is how the search moved off the pilot's steering
    /// and onto why the cars could not move at all.
    #[must_use]
    pub fn escapes(&self) -> (usize, f32) {
        (self.escapes, self.escape_moved)
    }

    /// Ways out of the node re-picked from, summed over give-ups, and how many of them pointed
    /// away from the one abandoned.
    #[must_use]
    pub fn swap_choice(&self) -> (usize, usize, usize) {
        (self.swap_links, self.swap_options, self.swap_free)
    }

    /// How many of the course's waypoints the car has actually driven past.
    #[must_use]
    pub fn covered(&self) -> usize {
        self.covered.len()
    }

    /// Completed laps.
    #[must_use]
    pub fn laps(&self) -> u32 {
        self.laps
    }

    /// How far round the course it is, as waypoints driven since the line — the number a running
    /// order is sorted on.
    ///
    /// Counted from the start rather than from waypoint zero, and added to the lap count, so it
    /// rises monotonically through a race instead of falling back to zero at every lap.
    #[must_use]
    pub fn along(&self, course: usize) -> usize {
        if course == 0 {
            return 0;
        }
        let round = (self.goal + course - self.line) % course;
        self.laps as usize * course + round
    }

    /// This frame's controls. `None` when it has not been placed or the network is empty.
    pub fn drive(
        &mut self,
        dt: f32,
        at: Vec3,
        facing: Quat,
        speed: f32,
        net: &Network,
        course: &[Vec3],
        traffic: &[Vec3],
        sight: Option<(&Walls, &Ground)>,
    ) -> Option<Controls> {
        let here = self.at?;
        // The caller's real step, not an assumed one — see `TICK`.
        let tick = if dt > 0.0 { dt } else { TICK };
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
        // Which way the car is pointing. Wanted by the aim walk and by the steering itself, so it
        // is worked out once here rather than at each.
        let f = flat(facing * Vec3::NEG_Z).normalize_or_zero();

        // Still on the line: brakes on, wheels straight, and no advance along the network — a
        // pilot that walked the graph while its car stood still would arrive already lost.
        if self.hold > 0.0 {
            self.hold -= tick;
            return Some(Controls {
                throttle: 0.0,
                brake: 1.0,
                steer: 0.0,
                toggle_auto_shift: false,
            });
        }

        // Advance while the **next** waypoint is nearer than the one held — the same self-limiting
        // rule the network walk uses, and for the same reason.
        //
        // Proximity was the first rule and it is wrong in a way that hides: a car that strays from
        // the course never comes within the radius, so its counter stops while the car keeps
        // driving. Measured, one field covered 1,843 m with the counter reading five waypoints of
        // forty metres — the standings said it had gone 200 m. A running order built on that is
        // fiction.
        if !course.is_empty() {
            let d = |i: usize| flat(course[i % course.len()] - at).length();
            // **"Advance while the one it holds is behind" is refuted here too**, and it was worth
            // trying because the lock is real: over eight routes 39 of 64 cars stop gaining course
            // progress before t=30 s and the field averages 5.2 waypoints of 126. A car that strays
            // past its waypoint sideways never gets nearer the next one than the one it holds, the
            // counter stops, and since the counter is what the pilot steers the network by it then
            // drives at a point behind it for the rest of the race.
            //
            // The reason it fails is the reason it failed on the node walk, and now it is clear:
            // **"behind" cannot terminate on a cycle.** A course is a closed ring, so a car pointed
            // away from it has a whole arc of it behind — the counter walks that arc, wraps, counts
            // a lap, and the early waypoints are behind too. Measured: 92 laps and 5,999 waypoints
            // driven for one car in ninety seconds, and two cars "FINISHED". Whatever the answer to
            // a car that has lost the course is, advancing is not it. See [`Self::lost`].
            // **The second way to be done with a waypoint: standing on it.**
            //
            // `Paths4081` needs this and nothing here provides it. Its grid is 8 m from waypoint
            // #124, so every pilot starts holding #125 with the car already on top of it, and the
            // first arm can never fire: the held one is at arm's length and the next — #0, the ring
            // wrapping — is 33 m off, so "is the next nearer" is permanently no. Measured, all
            // eight cars gain **nothing** in ninety seconds while taking seven junctions each,
            // steering at a point that ends up 128 m behind them, and finally reversing. Where the
            // grid is far from a waypoint this never comes up: `Paths4061` starts 42 m out and
            // walks 46 → 47 → 50 → 51 → 53 → 55 → 56 in the same time.
            //
            // **This is not the refuted resync above**, and the difference is the whole of it: that
            // one re-picks the nearest waypoint and can put the goal anywhere, including behind.
            // This only ever steps forward by one, only when the goal being left has been reached,
            // and at most three times a tick — so it cannot walk an arc of the ring the way
            // "advance while the held one is behind" did.
            let reached: f32 =
                std::env::var("NFS_REACHED").ok().and_then(|v| v.parse().ok()).unwrap_or(REACHED);
            // **Passed to the side, which neither arm above can see.** Measured on `Paths4102`
            // with `NFS_LOST=1`: the whole field drives *on the line* at 88 km/h past waypoint 11,
            // forty metres to the side of it, and the counter sticks — the held one bottoms out at
            // 40 m, well outside [`REACHED`]'s eighteen, and the next one is 81 m away and stays
            // further for the rest of the pass, so "is the next nearer" never fires either. The
            // waypoint ring and the carriageway the car is on are different paths of the same
            // route; the corridor merges them, the counter does not. The goal then swings from
            // −61° to −140°, the aim walk follows it backwards, and the pilot asks for full lock at
            // 88 km/h. That is the field's largest single loss of course.
            //
            // **This is not the refuted "advance while the one it holds is behind".** That one
            // could not terminate on a cycle: a car pointed away from a closed ring has an arc of
            // it behind, and the counter walked the arc and wrapped — 92 laps and 5,999 waypoints
            // for one car. The guard here is that the step must land on a waypoint that is
            // **ahead**, so the arm disarms itself the moment it fires: next tick the held one is
            // in front, `behind` is false, and nothing more happens until the car passes it too. A
            // run of waypoints behind the car can never be walked, whatever it is pointing at.
            let passed: f32 = std::env::var("NFS_PASSED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(PASSED_NEAR);
            // Past it in the course's own direction — see [`PASSED_NEAR`] for why that and not
            // "behind the nose", and for the sweep.
            let past = |i: usize| {
                let (a, b) = (course[i % course.len()], course[(i + 1) % course.len()]);
                flat(at - a).dot(flat(b - a).normalize_or_zero())
            };
            for _ in 0..3 {
                let next = (self.goal + 1) % course.len();
                // **Bounding the overshoot instead of the distance is refuted, and it is the more
                // obvious rule of the two.** Decomposing the goals that are *still* stuck with this
                // rule in place shows two shapes the distance bound cannot tell apart: on
                // `Paths4102` the car is 17 m past its waypoint and **64 m to the side of it** — the
                // ring is on the neighbouring carriageway — while on `Paths4081` a stuck goal is 6 m
                // to the side and **119 m behind**, a car that has turned round. "How far past along
                // the road" separates those cleanly and gains 70 waypoints on 4021 and 4041 doing
                // it. It also loses 104 on 4121 and 21 on 4102, for 927 against **986** — because
                // the virtue of the distance bound turns out to be the thing that looked like its
                // flaw: it **rejects** a waypoint that is far away sideways, and at 4121's corner a
                // car running wide is a metre or two past its waypoint and tens of metres beside it.
                // Releasing there is the skip that empties the route.
                let overtaken = passed > 0.0 && d(self.goal) < passed && past(self.goal) > 0.0;
                if !overtaken && d(next) >= d(self.goal) && d(self.goal) > reached {
                    break;
                }
                if next == self.line {
                    self.laps += 1;
                }
                self.goal = next;
                if overtaken {
                    break;
                }
            }
        }
        // **Re-finding the course when the counter stops is refuted too.** The lock is real and the
        // idea is the obvious one left after advancing failed: a pilot that has gained no waypoint
        // for a while asks the grid's own question again — nearest waypoint, nearest node — which is
        // bounded by construction and cannot run away. It loses anyway, at every interval swept, on
        // the one measure it cannot inflate. Waypoints actually driven past, over eight routes: 462
        // with it off, **369 at 4 s, 412 at 8 s, 435 at 16 s**; distance 4,129 m → 2,955 / 3,878 /
        // 3,640; cars away 62/64 → 53 / 59 / 62. The only column it wins is junctions, which is
        // exactly the column that re-picking a node inflates.
        //
        // It was worth doing for what it exposed: `along` is the waypoint *index*, so a pilot that
        // jumps its index forward is credited with everything in between, and the resync looked like
        // a twenty-two-fold gain (5.5 → 122.8) until it was measured with [`Self::covered`].
        // What it has actually driven past, as opposed to what its counter says.
        for (i, w) in course.iter().enumerate() {
            if flat(*w - at).length() <= COVERED_WITHIN {
                self.covered.insert(i);
            }
        }

        let toward = course.get(self.goal).copied().unwrap_or(at + facing * Vec3::NEG_Z * 1000.0);

        // Advance while the **next** node is nearer the car than the one held. Self-limiting by
        // construction: it stops the moment the held node is the nearest, so a stopped car stops
        // advancing and a fast one keeps up.
        //
        // Both simpler rules were tried and both fail, in opposite directions. "Advance when near
        // enough" is a runaway — the node stepped to is usually near enough too, so the pilot walks
        // the graph at frame rate while the car crawls: 6,600 junctions per rival in a hundred
        // seconds, against the one a second a car at 96 km/h over 29 m nodes can drive. "Advance
        // only when the node is behind" is the opposite: a node the car cannot reach is never
        // behind it, so the pilot stops at the first one it cannot get to and the car stops at 65 m.
        // **"Or the car has driven past it" is refuted, and by a factor of a thousand.** The
        // nearer-than rule visibly cannot let go of a node the car is standing on — no neighbour of
        // a node 1.2 m away is nearer to the car than that — so adding "advance also when the held
        // node is behind" looks like the missing half, and it is not: over eight routes junctions go
        // from 848 to **854,536** and distance covered *falls*, 3,652 → 2,699 m. It is the same
        // runaway the paragraph above records, arrived at from the other side. A node behind the car
        // is usually replaced by another node behind the car, so the loop advances to its cap every
        // frame for the rest of the race. Whatever fixes the sticky node, it is not this.
        let dist = |i: u32| net.node(i).map_or(f32::MAX, |j| flat(j.at - at).length());
        for _ in 0..3 {
            let Some(next) = net.step_avoiding(self.at?, self.from, toward, &self.blocked) else { break };
            if dist(next) >= dist(self.at?) {
                break;
            }
            self.from = self.at;
            self.at = Some(next);
            self.passed += 1;
            self.seen.insert(next);
        }
        let _ = here;

        // Aim: walk the network forward from the held node until far enough away, so the aim point
        // follows the road round a corner instead of cutting across it.
        //
        // **An aim point behind the car is not an aim point.** `walked` is seeded with the distance
        // from the car to its held node, so once a car has drifted further than one lookahead from
        // that node the loop breaks on its first test and the aim is left *on the node* — which by
        // then is behind. Pure pursuit at a point behind the car asks for full lock, and full lock
        // held is a circle: traced on `Paths4041`, car 3 arrives within 1.2 m of node 163, cannot
        // advance off it (no neighbour is nearer to the car than a node it is standing on), watches
        // the aim collapse back onto it, and drives a smooth arc off the edge of the world at 39
        // km/h. Nothing recovers it — the stall manoeuvre wants speed under 0.7 m/s and the car is
        // doing eleven.
        //
        // So the walk keeps going while the aim is behind, not only while it is near.
        //
        // **It is a trade and the trade is worth it.** Over eight routes: cars that got away
        // **52/64 → 62/64**, distance **2,594 → 3,652 m**, junctions **718 → 848**, cars off the
        // world unchanged at 10. Three routes gain heavily (4081 2/8 away and 21 junctions → 8/8 and
        // 159, 4061 4/8 and 237 m → 8/8 and 735, 4121 142 m → 724) and three lose (4002 172 → 51 m,
        // 4102 452 → 333, 4021 388 → 348). Kept because it wins every headline measure at once and
        // because the ratio of junctions to *distinct* nodes does not move (1.29 → 1.30), so the
        // extra junctions are new road rather than a car going round in circles — on 4102, where the
        // distance fell, the ratio goes 1.28 → 1.00 and every junction it takes is somewhere new.
        // `NFS_LOOK` re-opens the sweep that fixed this at 0.9. It was fixed against the chord
        // ring, and the ring has since moved: with the ring pulled onto the corridor the field's
        // departures changed character completely — "goal stranded behind, full lock" fell from
        // 44-53 % of them to 19-22 %, and what is left is **66 % braking hard and 59 % above
        // 60 km/h**, which is this constant's own subject.
        let per: f32 = std::env::var("NFS_LOOK")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(LOOKAHEAD_PER_SPEED);
        let look = (speed * per).clamp(LOOKAHEAD_MIN, LOOKAHEAD_MAX);
        let aim_line = std::env::var("NFS_AIMLINE").is_ok_and(|v| !v.is_empty() && v != "0");
        let aim_lerp = std::env::var("NFS_AIMLERP").is_ok_and(|v| !v.is_empty() && v != "0");
        let (mut cur, mut prev) = (self.at?, self.from);
        let mut aim = net.node(cur)?.at;
        let mut walked = flat(aim - at).length();
        for _ in 0..8 {
            if walked >= look && flat(aim - at).dot(f) > 0.0 {
                break;
            }
            let Some(next) = net.step_avoiding(cur, prev, toward, &self.blocked) else { break };
            // **Do not walk off the course to find something to aim at.** Measured 2026-08-20:
            // the aim is a *network node*, not a ring waypoint, so pulling the ring onto the
            // corridor did not put the aim there — and on `Paths4081` the aim is outside the
            // corridor on **36.7 %** of steps against 0.7 % on `Paths4061` and 0.0 % on
            // `Paths4001`. 4081 is the one route that collapsed when the lookahead went from 0.9 s
            // to 1.8 s (99.1 % → 53.6 % of race time on the corridor), and a longer walk is
            // exactly what reaches the parallel carriageway its junction 206 links to.
            //
            // This does **not** choose between arms — that rule was swept and refuted twice. It
            // stops the walk, leaving the aim on the last node that was on the line, and only once
            // there is already a usable aim ahead of the car: an aim behind is worse than an aim
            // off the course, which the block above measures in detail.
            //
            // **And it is refuted as a field rule, while being exactly right on the route that
            // motivated it.** On `Paths4081` it does what it was built to do: race time on the
            // corridor **53.6 % → 96.5 %** at a 20 m mark (89.9 % at 12 m) and the aim outside the
            // corridor 36.7 % → 2.1 %. Over the eight routes it loses anyway — −49 waypoints at
            // 20 m and −94 at 12 m — because the four routes it does not fix, it breaks:
            // `Paths4021` 94 → 77 %, `Paths4041` 91 → 82, `Paths4061` 65 → 51, `Paths4102` 68 → 57.
            // The width matters and neither is right for everything, which is the same shape as
            // every other rule that tried to keep this pilot near the line.
            //
            // The mark's width is the reason the first attempt did nothing at all: at the default
            // 40 m the parallel carriageway 30-40 m away is *on the line*, so nothing was ever
            // stopped and the run came out byte-identical.
            if aim_line
                && !net.on_line_at(next)
                && flat(aim - at).dot(f) > 0.0
                && walked >= LOOKAHEAD_MIN
            {
                break;
            }
            let p = net.node(next)?.at;
            let leg = flat(p - aim).length();
            // **`NFS_AIMLERP=1`: put the aim *on* the lookahead, not on the node past it.**
            //
            // The walk stops at whichever node first carries `walked` past `look`, so the aim is
            // a network node and nodes are ~30 m apart: when the walk steps, the aim **teleports**.
            // Traced on `Paths4081`, where eight cars lose the line at one place: at t=35.2 the
            // car is doing 80 km/h, 0.5 m from the corridor's centre with the wheel straight, and
            // the aim jumps from **−1° to 38° in one step** as the walk moves from node 239 to
            // 205. Full brake and −0.36 of lock follow, and forty metres later the car is out.
            //
            // Interpolating along the last leg makes the same target continuous: the aim slides
            // towards the corner instead of arriving at it. Nothing about *which* way it goes
            // changes — that is `step_avoiding`, and every attempt to influence it is refuted.
            //
            // **And it is refuted, harder than anything else measured on this pilot: −664
            // waypoints, behind on all eight routes, `furthest` 5951 → 3981 m.** The diagnosis was
            // right and the conclusion was wrong. The jump is real; it is not the defect.
            //
            // What the numbers say is that the **discreteness is load-bearing**. A node is a place
            // the road actually goes, and a point interpolated between two of them is a place the
            // road only goes if the road is straight there. Worse, an interpolated aim sits at
            // exactly `look` metres for ever: it recedes as the car approaches, so the car never
            // arrives anywhere. This pilot does not track a path, it **chases a point** — the
            // steering law's own doc says so — and a point that cannot be caught is not a target.
            // The teleport at a corner is what chasing discrete points costs, and it costs less
            // than the alternative by a factor of ten.
            if aim_lerp && walked + leg > look && leg > 0.01 {
                let t = ((look - walked) / leg).clamp(0.0, 1.0);
                aim = aim + (p - aim) * t;
                break;
            }
            walked += leg;
            aim = p;
            prev = Some(cur);
            cur = next;
        }

        // Which way is sideways. Wanted by the two rules below that move the aim off the line, so
        // it is worked out once.
        let side = Vec3::new(-f.z, 0.0, f.x);

        // **Not through that.** Both ways of dealing with a barrier through the *graph* were swept
        // and refuted — cutting the link and pricing it, `Network::drop_walled` carries the numbers
        // — and the measurement that closed that door opened this one. Asked of the unchanged
        // field: on `Paths4002` something stands between a car and the point it is steering at on
        // **72 %** of the first twenty seconds' samples, against **zero** on the other seven
        // routes. (Over ninety seconds 4081 reads 54 % and 4001 13 %, and both of those are zero
        // early — that is cars that are already lost aiming across buildings, an effect and not a
        // cause. Only 4002 does it from the grid.)
        //
        // So the rule is the same shape as going round a car in front, with one thing it cannot
        // assume: which side. A central reservation has good road on both sides — that is why the
        // file links across it in the first place — so the side is *searched* rather than guessed,
        // widening until the way is clear. Nothing else changes, so the car returns to the line by
        // itself once the concrete has run out.
        let clear: f32 =
            std::env::var("NFS_AIMCLEAR").ok().and_then(|v| v.parse().ok()).unwrap_or(AIM_CLEAR);
        if clear > 0.0 {
            if let Some((walls, ground)) = sight {
                if self.wall_due == 0 {
                    self.wall_due = WALL_EVERY;
                    self.wall_shift = 0.0;
                    if walls.across(ground, at, aim, WALL_LIFT, WALL_STEP) {
                        let mut off = clear;
                        'search: while off <= clear * WALL_WIDEST {
                            for s in [1.0f32, -1.0] {
                                let try_at = aim + side * s * off;
                                if !walls.across(ground, at, try_at, WALL_LIFT, WALL_STEP) {
                                    self.wall_shift = s * off;
                                    break 'search;
                                }
                            }
                            off += clear;
                        }
                    }
                } else {
                    self.wall_due -= 1;
                }
                aim += side * self.wall_shift;
            }
        }

        // Stuck, and doing something about it. Reverse for a moment with the lock reversed too,
        // which is what backs a car off the thing it has driven into rather than along it.
        //
        // The threshold is speed and not position because a car grinding along a wall is moving:
        // it is the *forward* speed that is gone.
        //
        // Two things this has to get right, and the first version got both wrong. A car that has
        // just been dropped on the grid is not stalled, it is settling — so the count only runs
        // once the pilot has driven at all. And a car that is *reversing* is not stalled either;
        // measuring `|speed|` made every car reverse for ever, the whole field ending sixty metres
        // behind the line facing the wrong way.
        if self.backing > 0.0 {
            self.backing -= tick;
            self.stalled = 0.0;
            return Some(Controls {
                throttle: -0.7,
                brake: 0.0,
                steer: -self.steer,
                toggle_auto_shift: false,
            });
        }
        // Gated on *time* and not on having taken a junction: the cars that need this most are
        // exactly the ones that never took one, and gating on progress meant the pilots that were
        // stuck from the start were the only ones that could not try to get out.
        self.age += tick;
        // **A car rolling backwards is not stalled — it is obeying.** `speed` is signed, so a car
        // that has just finished its 1.2 s of reverse is carrying negative speed, and `speed <
        // STALL_SPEED` counts that as a stall from the first tick. Getting from about −1.5 m/s back
        // up through +0.7 takes something over two seconds even at full pedal, and [`STALL_FOR`] is
        // 1.5: the manoeuvre therefore *guarantees* the next stall that starts it again. Measured
        // on 4002, that is what the cars were doing — 35 s of a 90 s race spent in reverse and 115
        // escapes, none of which could ever have completed. Rolling backwards is not standing
        // still, and only standing still is a stall.
        let stall_back: f32 = std::env::var("NFS_STALLBACK")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(STALL_BACK);
        let moving_back = stall_back > 0.0 && speed <= -stall_back;
        if self.age > SETTLE && speed < STALL_SPEED && !moving_back && self.hold <= 0.0 {
            self.stalled += tick;
        } else {
            self.stalled = 0.0;
        }
        if self.stalled >= STALL_FOR {
            self.stalled = 0.0;
            self.backing = BACK_FOR;
            // **Pick a way out of the city, not out of the node list.** Twelve headings, the
            // ground asked how far it holds along each and the walls asked whether anything
            // stands across it; the longest clear one wins and the car drives at it for a
            // bounded time, graph ignored. When it expires the pilot resumes exactly where it
            // was — nothing about the node walk is disturbed, which is why this can be tried
            // without unpicking any of the three refuted rules.
            let far: f32 =
                std::env::var("NFS_ESCAPE").ok().and_then(|v| v.parse().ok()).unwrap_or(ESCAPE_FOR);
            if far > 0.0 {
                if let Some((walls, ground)) = sight {
                    let mut best = (0.0f32, Vec3::ZERO);
                    for i in 0..ESCAPE_RAYS {
                        let a = i as f32 * std::f32::consts::TAU / ESCAPE_RAYS as f32;
                        let dir = Vec3::new(a.cos(), 0.0, a.sin());
                        let reach = ground
                            .gap_along(at, at + dir * ESCAPE_REACH, ESCAPE_SLACK, 2.0)
                            .unwrap_or(ESCAPE_REACH);
                        // **Hedefe doğru en açık, en açık değil.** İlk sürüm yalnız en uzun
                        // temiz yönü seçiyordu ve ölçüm onu çürüttü: araba kurtuluyor (2 sn'den
                        // itibaren 64/64 kavşak alıyor) ama parkurdan uzaklaşıyor, kapsama
                        // 799'dan 737'ye iniyor. Açıklığı, gidilmesi gereken yönle ağırlıklamak
                        // ikisini birden ister: yarısı mesafe, yarısı yön.
                        let want = flat(toward - at).normalize_or_zero();
                        let score = reach * (0.5 + 0.5 * dir.dot(want));
                        if score > best.0
                            && !walls.across(ground, at, at + dir * reach, WALL_LIFT, WALL_STEP)
                        {
                            best = (score, dir * reach);
                        }
                    }
                    // `best.1` artık yönün kendisi değil, o yöndeki tam vektör.
                    if best.1.length() > ESCAPE_MIN {
                        self.escape = Some((at + best.1, far));
                        self.escapes += 1;
                        self.escape_from = Some(at);
                    }
                }
            }
            // Give up on where it was going, and take the next best way out of where it came from.
            // Reversing alone only buys another run at the same obstacle.
            if let Some(bad) = self.at {
                if !self.blocked.contains(&bad) {
                    self.blocked.push(bad);
                }
                let from = self.from.unwrap_or(bad);
                if let Some(other) = net.step_avoiding(from, None, toward, &self.blocked) {
                    // **Whether giving up actually points anywhere else.** Blacklisting the node
                    // and re-picking is only worth the reverse it costs if the replacement lies in
                    // a different direction from the car; a node the same way on is the same
                    // obstacle with another name, and the field spends a quarter to a half of a
                    // race backing off and returning. Counted rather than assumed, and 45° is the
                    // line: past that the car has genuinely been sent somewhere else.
                    let bearing = |i: u32| {
                        net.node(i).map_or(Vec3::ZERO, |j| flat(j.at - at).normalize_or_zero())
                    };
                    self.swaps += 1;
                    if bearing(bad).dot(bearing(other)) > 0.7 {
                        self.swaps_same += 1;
                    }
                    // **How much choice there was**, as opposed to what was taken. Shunning the
                    // abandoned heading was refuted with a fallback rate that said the escape
                    // almost never has anywhere else to go (`ROADMAP.md`), but that was inferred
                    // rather than measured, and two very different worlds produce it: a node with
                    // one way out, or a node with four ways out that all point the same way. The
                    // next mechanism depends on which, so it is counted here.
                    if let Some(j) = net.node(from) {
                        let away = bearing(bad);
                        self.swap_links += j.links.len();
                        self.swap_options +=
                            j.links.iter().filter(|l| bearing(**l).dot(away) <= 0.7).count();
                        // And of those, the ones not already given up on. The difference between
                        // these two is the whole question: a graph that offers no alternative and
                        // a graph whose alternatives this car has already crossed off are the same
                        // to the escape and want opposite fixes.
                        self.swap_free += j
                            .links
                            .iter()
                            .filter(|l| bearing(**l).dot(away) <= 0.7 && !self.blocked.contains(l))
                            .count();
                    }
                    self.at = Some(other);
                    self.from = Some(from);
                }
            }
        }

        // **The car in front.** Until now a rival would drive into the back of another one, which
        // the module said out loud and which turned out to cost more than it looked: part of the
        // field never leaves a grid that is four abreast with 1.3 m of clear air, and when the
        // pilots' blacklists were harvested for graph truth the nodes several cars agreed on sat a
        // median 7.6-47.7 m from the start line — they were giving up on each other, not on the
        // road.
        //
        // The crudest rule that is about the right thing: anything in a corridor of its own width,
        // ahead, inside a following distance. `traffic` may contain this car's own position — its
        // own `along` is zero, which is not ahead — so the caller does not have to exclude it.
        let look_mul: f32 = std::env::var("NFS_TRAFFIC")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(TRAFFIC_LOOK_PER_SPEED);
        let mut blocking: Option<(f32, f32)> = None;
        let mut blocked_side = 0.0f32;
        if look_mul > 0.0 {
            let look = (speed * look_mul).max(TRAFFIC_LOOK_MIN);
            let near = traffic
                .iter()
                .filter_map(|o| {
                    let d = flat(*o - at);
                    let (fwd, lat) = (d.dot(f), d.dot(side));
                    (fwd > 0.0 && fwd <= look && lat.abs() <= TRAFFIC_HALF_WIDTH).then_some((fwd, lat))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((g, lat)) = near {
                blocking = Some((g, look));
                blocked_side = lat;
            }
        }

        // **Going round it.** Queueing is honest but it is also the whole field stopping behind one
        // stuck car, and the aim point is the cheap place to say "not through that". Shift it away
        // from whichever side the obstacle is on, by more the closer it is; nothing else changes, so
        // the car comes back to the line by itself as soon as the way is clear.
        let pass: f32 =
            std::env::var("NFS_PASS").ok().and_then(|v| v.parse().ok()).unwrap_or(PASS_OFFSET);
        if pass > 0.0 {
            if let Some((g, look)) = blocking {
                let urgency = (1.0 - g / look).clamp(0.0, 1.0);
                aim -= side * blocked_side.signum() * pass * urgency;
            }
        }

        // An escape overrides the graph's aim for its duration, and nothing else: the held
        // node, the blacklist and the waypoint are all left exactly as they were, so the
        // pilot resumes mid-stride when it expires.
        if let Some((to, left)) = self.escape {
            let left = left - tick;
            if left <= 0.0 || flat(to - at).length() < ESCAPE_ARRIVED {
                if let Some(from) = self.escape_from.take() {
                    self.escape_moved += flat(at - from).length();
                }
                self.escape = None;
            } else {
                self.escape = Some((to, left));
                aim = to;
            }
        }
        self.aim = Some(aim);
        let reach = flat(aim - at).length();
        let to = flat(aim - at).normalize_or_zero();
        let angle = f.cross(to).y.atan2(f.dot(to));
        // Past [`BEHIND`] the angle carries no usable side, so the side already chosen is kept and
        // the manoeuvre can finish. The measurement and the sweep are on the constant.
        let hold: f32 =
            std::env::var("NFS_BEHIND").ok().and_then(|v| v.parse().ok()).unwrap_or(BEHIND);
        let angle = if hold > 0.0 && angle.abs() > hold {
            hold * if self.steer < 0.0 { -1.0 } else { 1.0 }
        } else {
            angle
        };
        // A lock that grows with speed was tried here and is **refuted across routes**. The
        // reasoning was good — a raycast vehicle turns by generating lateral force and there is
        // none at rest, and the trace showed stuck cars sitting on full lock — and on the route it
        // was found on it worked: five cars away became six, 165 junctions became 198. On four
        // routes it is not an improvement but a trade: 4041 went 2/8 to 6/8 and 4102 went **8/8 to
        // 5/8**, 275 junctions down to 111. So it is gone rather than kept at a value that reads
        // as tuned.
        // **Steering at a point behind the car is the field's largest single loss of course, and
        // capping the lock for it is refuted.** Traced over eight routes with `NFS_LOST=1`, **28 of
        // the 50 cars that lost the course** spent half a second or more aiming at something behind
        // them — almost all starting *on* the line, 0.5 m from it at 45-88 km/h, and ending 20-50 m
        // outside it with the wheel pinned near full lock. On `Paths4102` it is the whole field,
        // six cars within a second of each other.
        //
        // So the obvious fix was to stop asking for the impossible: a car at 88 km/h cannot turn
        // round, and clamping the lock while the target is behind should let it hold the road while
        // it sheds speed. Swept, and it fails on both counts that matter:
        //
        // | cap | waypoints | never lost the course | fell off the world |
        // |---|---|---|---|
        // | none | **927** | 14 | **2** |
        // | 0.35 | 728 | 19 | 5 |
        // | 0.15 | 739 | 16 | 6 |
        //
        // It does keep more cars on the course, and it costs a fifth of the field's progress to do
        // it — but the number that closes it is the last column: **a car that cannot turn cannot
        // turn away from an edge either**, and three times as many drove off the world. The
        // manoeuvre being clamped is the same one that saves a car at the boundary.
        //
        // Not tried, and the better shape of the same idea: a cap that varies with speed — full
        // lock at rest, where turning round is exactly right and the escape machinery depends on
        // it, and little at 60 km/h. What is measured here is the flat cap only.
        //
        // **Tried on 2026-08-20**, because the departure census pointed straight at it: of the 34
        // cars that leave the course, **18 are at full lock** the step they cross the edge and 8
        // are above 60 km/h. `NFS_CAPFAST` is the lock left at [`CAP_SPEED`] and `NFS_CAPSPEED`
        // that speed; the cap runs linearly from 1.0 at rest, so nothing about standing still,
        // escaping or turning round changes.
        //
        // **And the law itself, measured on 2026-08-20.** The ramp above is linear in the angle:
        // full lock at 90°, half at 45°. Pure pursuit's own geometry is not — the arc from the car
        // to an aim point `L` away at angle `α` has curvature `2·sin α / L`, so the steer angle is
        // `atan(wheelbase · 2 sin α / L)` and the lock fraction is that over the car's own lock.
        // The two agree where the corner is real: at a 21 m radius taken with the aim 20 m ahead
        // at 45°, the ramp asks 0.42 and the geometry 0.41. They part company exactly where the
        // pilot was measured to fail — aim **49 m away at 66°**, where the ramp asks 0.62 and the
        // geometry **0.19**, because a point that far off the nose and that far away is not a
        // corner, it is the lookahead reaching around one.
        //
        // Past 90° the geometry has to be abandoned rather than trusted: `sin α` *falls* again, so
        // a target directly behind would ask for no lock at all, and the escape machinery depends
        // on full lock at rest. Saturated there instead, which is also where [`BEHIND`] takes over.
        //
        // **Swept, and refuted — the ramp is right and the honest geometry is wrong here.**
        // `NFS_PURSUIT=1` turns it on; over the eight routes it loses on every measure at once:
        //
        // | | waypoints | on course | nodes each | furthest | fell |
        // |---|---|---|---|---|---|
        // | **ramp** (kept) | **1046** | **30 / 64** | **20.0** | **5299 m** | **6** |
        // | pure pursuit | 839 | 27 / 64 | 15.1 | 4772 m | 7 |
        //
        // −207 waypoints, behind on **seven of the eight routes** with no ties, and the biggest
        // single route is −87 — a field result, not one route's.
        //
        // Why the wrong law wins is worth keeping, because it says what this pilot is: it does not
        // *track a path*, it **chases a point**, and chasing hard is what carries it back when the
        // lookahead swings wide. Pure pursuit's geometry answers "what arc reaches that point", and
        // an arc that reaches a point 49 m away at 66° is a gentle one — correct for a car that
        // means to arrive there, useless for a car that needs to be pointing at the road again in
        // the next second. The ramp's over-steering is the recovery.
        let cap_fast: f32 =
            std::env::var("NFS_CAPFAST").ok().and_then(|v| v.parse().ok()).unwrap_or(CAP_FAST);
        let cap_speed: f32 =
            std::env::var("NFS_CAPSPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(CAP_SPEED);
        let cap = 1.0 - (1.0 - cap_fast) * (speed.abs() / cap_speed.max(0.1)).clamp(0.0, 1.0);
        let geometry = std::env::var("NFS_PURSUIT").is_ok_and(|v| !v.is_empty() && v != "0");
        let want = if geometry {
            if angle.abs() >= std::f32::consts::FRAC_PI_2 || reach < 1.0 {
                angle.signum() * STEER_LIMIT
            } else {
                let curve = 2.0 * angle.sin() / reach;
                let lock = (WHEELBASE * curve).atan() / crate::car::tune::steering_lock();
                lock.clamp(-1.0, 1.0) * STEER_LIMIT
            }
        } else {
            (angle * 2.0 / std::f32::consts::PI).clamp(-1.0, 1.0) * STEER_LIMIT
        };
        let want = want * cap;

        // How fast the wheel catches up with what the pilot wants.
        //
        // **Swept and refuted, and the way it failed is the point.** Six of eight cars on 4121
        // leave the course at one corner, and a lagging wheel at 65 km/h through 50° is exactly
        // what running wide looks like — so raising this was the obvious fix. On that route it
        // looked like one: 0.5 took the cars losing the course there from **6 of 8 to 3 of 8**,
        // and the number never leaving it from 1 to 3.
        //
        // It did not survive the field. Over eight routes, 0.5 gives **773** waypoints driven past
        // against **869**, and 1 149 distinct nodes against 1 364 — while the corner metric it was
        // supposed to fix barely moves (53 of 64 cars leave the course either way, 11 never leave
        // against 12). The one-route result was the route, not the rule. This is what the sweep is
        // for, and it is worth remembering that a 2× improvement on a single route can mean
        // nothing at all.
        self.steer += (want - self.steer) * 0.35;
        // An escape at rest is not a corner — see [`ESCAPE_FULL`] for why the lift is what was
        // keeping the car there, and for the sweep.
        let esc_full: f32 =
            std::env::var("NFS_ESCFULL").ok().and_then(|v| v.parse().ok()).unwrap_or(ESCAPE_FULL);
        let lift =
            if self.escape.is_some() && speed.abs() < esc_full { 0.0 } else { CORNER_LIFT };
        let mut throttle = (1.0 - self.steer.abs() * lift).max(0.15);
        // `NFS_FLATOUT=1`: pedal to the floor, no brake, steering untouched. **An instrument, not
        // a driving mode.** "The car is not fast enough" is a claim about the drivetrain, and the
        // field's top speed on a city course cannot answer it — the pilot lifts for the wheel and
        // brakes for curvature, so what is measured is the course. With both taken away, what is
        // left is what the gearbox and the torque curve can do.
        let flat_out = std::env::var("NFS_FLATOUT").is_ok_and(|v| !v.is_empty() && v != "0");

        // **Brake.** Lifting the throttle was the whole speed policy and it is not enough: with no
        // brake a car carries 90 km/h into a corner, runs wide and leaves the road — measured, 20
        // of 64 cars across eight routes ended off the world, falling.
        //
        // The rule is the crudest one that is about the corner rather than about the moment: how
        // hard the car is turning, times how fast it is going, against what it could hold.
        let bs: f32 =
            std::env::var("NFS_BRAKE").ok().and_then(|v| v.parse().ok()).unwrap_or(BRAKE_SPEED);
        let over = (speed / bs) * self.steer.abs();
        let mut brake = ((over - 1.0) * BRAKE_GAIN).clamp(0.0, 1.0);
        if flat_out {
            throttle = 1.0;
            brake = 0.0;
        }
        // `NFS_FLATOUT=2` also nails the wheel straight, which is the only way to ask the
        // drivetrain a question with no course in it: the car leaves the road within seconds, and
        // what it reached before it did is the acceleration the gearbox and the curve give.
        if std::env::var("NFS_FLATOUT").as_deref() == Ok("2") {
            self.steer = 0.0;
        }

        // **And the corner the car has not reached yet**, which the rule above cannot see: it
        // watches how hard the wheel *is* turned, and the wheel only turns once the corner is here.
        // What the aim arc costs in lateral acceleration is visible seconds earlier — see [`GRIP`],
        // which carries the measurement and the sweep.
        //
        // An aim point behind the car has a small `sin θ` and asks for nothing here; that is
        // deliberate. It is a separate failure with a separate fix, and this term must not paper
        // over it by braking for it.
        let grip: f32 = std::env::var("NFS_GRIP").ok().and_then(|v| v.parse().ok()).unwrap_or(GRIP);
        // **Braking on the road's own bend instead of on the chord is refuted.** `θ` here carries
        // two things at once — the road's bend *and* this car's heading error — and the purer
        // quantity was tried: the tightest curvature of the walk's own node polyline, with nothing
        // about the car in it. Over eight routes it loses at both settings, 909 waypoints at
        // 5 m/s² and 848 at 6.5 against **927** for the chord, so the code is gone rather than kept
        // at a value that reads as tuned. The reading is worth keeping: slowing a car that has
        // *already* drifted off line is not a bug in the measurement, it is half of what the brake
        // is for, and "do not brake for your own mistake" is a refinement the field rejects.
        if grip > 0.0 {
            let reach = flat(aim - at).length().max(1.0);
            let need = 2.0 * speed * speed * angle.abs().sin() / reach;
            brake = brake.max(((need / grip - 1.0) * BRAKE_GAIN).clamp(0.0, 1.0));
        }

        // **And the corner the *course* has, which neither of the two above can see.**
        //
        // The refutation just above braked on the **network's** node polyline; this asks the
        // **ring** the pilot is actually steering at, and the difference is that the ring is now
        // honest — every waypoint outside the race corridor is pulled back into it, which it was
        // not when that measurement was taken.
        //
        // The place that asked for it: on `Paths4121` eight cars lose the line at one node, and
        // at the moment they do they are **0 m from that node and 14 cm from the corridor's
        // centre** — driving perfectly. The ring ahead of them turns at a **13 m radius**, which
        // `v = sqrt(a·r)` at the field's own measured 5.2 m/s² makes a **30 km/h** corner, and
        // they arrive at 68. Nothing in the pilot was looking that far ahead: `BRAKE_SPEED`
        // watches the wheel, which only turns once the corner is here, and `GRIP` watches the aim
        // chord, which is a network node a second away.
        //
        // Look ahead by the distance it takes to stop *to* that speed rather than a fixed reach,
        // because a corner you cannot brake for in time is not information.
        let ring_held: f32 =
            std::env::var("NFS_RINGBRAKE").ok().and_then(|v| v.parse().ok()).unwrap_or(RING_HELD);
        if ring_held > 0.0 && course.len() >= 3 && speed > 1.0 {
            let n = course.len();
            let look = (speed * speed / (2.0 * ring_held)).clamp(20.0, 200.0);
            let mut walked = 0.0;
            let mut i = self.goal % n;
            let mut limit = f32::INFINITY;
            while walked < look {
                let (a, b, c) = (course[(i + n - 1) % n], course[i], course[(i + 1) % n]);
                let (u, v) = (flat(b - a), flat(c - b));
                let (lu, lv) = (u.length(), v.length());
                if lu > 0.01 && lv > 0.01 {
                    let turn = (u / lu).cross(v / lv).y.asin().abs();
                    if turn > 1e-3 {
                        let r = 0.5 * (lu + lv) / turn;
                        limit = limit.min((ring_held * r).sqrt());
                    }
                }
                walked += lv;
                i = (i + 1) % n;
            }
            if speed > limit {
                brake = brake.max(((speed / limit - 1.0) * BRAKE_GAIN).clamp(0.0, 1.0));
            }
        }

        // **The car in front.** Until now a rival would drive into the back of another one, which
        // the module said out loud and which turned out to cost more than it looked: three of eight
        // never leave a grid that is four abreast with 1.3 m of clear air, and when the pilots'
        // blacklists were harvested for graph truth the nodes several cars agreed on sat a median
        // 7.6-47.7 m from the start line — they were giving up on each other, not on the road.
        //
        // The crudest rule that is about the right thing: anything in a corridor of its own width,
        // ahead, inside a following distance. `traffic` may contain this car's own position — its
        // own `along` is zero, which is not ahead — so the caller does not have to exclude it.
        if let Some((g, look)) = blocking {
            // Nothing at the far end of the look, everything at nose to tail.
            let close = (1.0 - g / look).clamp(0.0, 1.0);
            throttle *= 1.0 - close;
            brake = brake.max((close - 0.5).max(0.0) * 2.0);
        }

        Some(Controls { throttle, brake, steer: self.steer, toggle_auto_shift: false })
    }
}
