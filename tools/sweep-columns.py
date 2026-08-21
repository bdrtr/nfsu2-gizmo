#!/usr/bin/env python3
"""Read an eight-route sweep arm by arm, on every column the ritual compares.

`sweep-table.py` answers one question — waypoints driven past — and answers it well. This reads the
rest of the same logs: the driving columns (`away`, `fallen`, `furthest`, `junctions`, distinct
nodes, time on course, cars that never left it) and the **height** columns that arrived with the
graph solve (how far a car gets above the node its own pilot is holding, how often, how many links
no road could climb, how many trees the height solve fell into, how many nodes took a height from a
neighbour).

    tools/sweep-columns.py <log-dir> <baseline-arm> <arm>...

where <log-dir> holds `<arm>-<route>.log` files as written by the sweep loop:

    for r in 4001 4002 4021 4041 4061 4081 4102 4121; do
      NFS_ROUTE=".../Paths$r.bin" NFS_SECONDS=90 nfs_sim ... > $DIR/$ARM-$r.log
    done

Two readings that are easy to get wrong, both of them house rules:

- **The waypoint number is the per-car sum**, never `SUMMARY waypoint=`, which is a per-route figure
  about half the scale. The noise floor on the field sum is ±37 (2.8 %).
- **A field total is a sum over eight routes and a sum can be carried by one of them.** Every
  comparison line below prints the biggest single-route difference beside the margin and says
  TAŞINIYOR when the one exceeds the other. That verdict is the point of the tool.

And two that are specific to the deck and held-node columns:

- `güverte max` is the single worst step of the run, so it moves on one sample; `güverte %adım` is
  the share of steps and is what an arm is judged on. The `· araba düğümünde` variant restricts it
  to the steps where the car is actually within `DECK_NEAR` of the node it holds, and that is the
  honest one — the unrestricted figure also counts a car standing on a different road.
- **The held-node columns are not a score.** `plan mesafesi`, `tutulan = en yakın` and
  `daha yakını vardı` are defined against the held node, so any change to how the pilot advances
  moves them by construction. `junctions` and distinct nodes inflate the same way. Judge such an
  arm on `waypoint`, `kursta süre`, `fallen` and `away`, and read the rest as a side channel.
"""
import os
import re
import sys

ROUTES = "4001 4002 4021 4041 4061 4081 4102 4121".split()

DRIVEN = re.compile(r"(\d+) waypoints driven past")
NODES = re.compile(r"over\s+(\d+) distinct nodes")
NEVER = re.compile(r"kursu hiç bırakmadı")
TIME = re.compile(r"kursta geçen süre: %([\d.]+)")
DECK = re.compile(r"güverte: araba tuttuğu düğümün (-?[\d.]+) m üstüne kadar çıkıyor · "
                  r"adımların %([\d.]+)")
NEAR = re.compile(r"m\): \d+ / \d+ · \d+ tanesinde, yani %([\d.]+)")
PLAN = re.compile(r"plan mesafesi: ortalama ([\d.]+) m · %([\d.]+)'i 15 m'den, %([\d.]+)'i 30")
HELD = re.compile(r"tutulan düğüm en yakını mı: adımların %([\d.]+)'inde evet · %([\d.]+)'inde")
AIM = re.compile(r"nişan mesafesi: ortalama ([\d.]+) m · %([\d.]+)'i 40 m'den")
AIMB = re.compile(r"%([\d.]+)'inde arabanın arkasında")
# One per car: its own totals, and whether the line under it says it never left the course.
CAR = re.compile(r"car \d+: .*?(\d+) waypoints driven past")
# The progress side of the same per-car line. A car that gained nothing in the last third of the
# race has stopped, and time-on-corridor and never-lost-the-line both score a stopped car as a
# success — the lesson of 2026-08-21, learnt by accepting a change on those two columns alone.
GAIN = re.compile(r"(\d+) waypoints driven past, last gained at\s*([\d.]+)s")
SECONDS = 90.0
KEPT = "kursu hiç bırakmadı"
HAD = re.compile(r"o adımların %([\d.]+)'inde düğümün kendi XZ")
SUMM = re.compile(r"SUMMARY held=(\d+) away=(\d+) junctions=(\d+) waypoint=(\d+) "
                  r"furthest=(\d+)\s+fallen=(\d+)")
NET = re.compile(r"network: (\d+) nodes · (\d+) links · (\d+) with no way out · (\d+) steeper")
SOLVE = re.compile(r"kot çözümü: (\d+) ağaç · komşudan yükseklik alan (\d+) düğüm")

# (key, heading, how the eight routes combine into a field figure)
COLUMNS = [
    ("driven", "waypoint (araba toplamı)", sum),
    ("on_course", "kursta süre %", lambda v: sum(v) / len(v)),
    ("never", "kursu hiç bırakmayan araba", sum),
    ("stalled", "ilerlemesi duran araba", sum),
    ("stall_share", "duruş payı %", lambda v: sum(v) / len(v)),
    ("away", "away", sum),
    ("fallen", "fallen", sum),
    ("junctions", "junctions", sum),
    ("furthest", "furthest m", sum),
    ("nodes", "distinct nodes", sum),
    ("deck_pct", "güverte %adım (>3 m)", lambda v: sum(v) / len(v)),
    ("near_pct", "güverte %adım · araba düğümünde", lambda v: sum(v) / len(v)),
    ("plan_mean", "tutulan düğüme plan mesafesi m", lambda v: sum(v) / len(v)),
    ("plan_30", "düğüme 30 m'den uzak %adım", lambda v: sum(v) / len(v)),
    ("held_is", "tutulan = en yakın %", lambda v: sum(v) / len(v)),
    ("held_nearer", "daha yakını vardı %", lambda v: sum(v) / len(v)),
    ("aim_mean", "nişan mesafesi m", lambda v: sum(v) / len(v)),
    ("aim_far", "nişan 40 m'den uzak %adım", lambda v: sum(v) / len(v)),
    ("aim_behind", "nişan arabanın arkasında %adım", lambda v: sum(v) / len(v)),
    ("deck_max", "güverte max m (tek adım)", max),
    ("had", "…o adımlarda yüzey VARDI %", lambda v: sum(v) / len(v)),
    ("steep", "1:1'den dik bağlantı", sum),
    ("links", "links", sum),
    ("dead", "çıkışsız düğüm", sum),
    ("trees", "kot çözümü: ağaç", sum),
    ("filled", "komşudan kot alan düğüm", sum),
]


def one(text):
    """Every column of a single route's log, zero where the line is absent."""
    s, d, n, k = (SUMM.search(text), DECK.search(text), NET.search(text), SOLVE.search(text))
    t, h = TIME.search(text), HAD.search(text)
    near, plan, held = NEAR.search(text), PLAN.search(text), HELD.search(text)
    am, ab = AIM.search(text), AIMB.search(text)
    # The population split: which cars never left the course, and what they alone covered.
    lines = text.splitlines()
    kept_cars = kept_wp = lost_cars = lost_wp = 0
    for i, line in enumerate(lines):
        m = CAR.search(line)
        if not m:
            continue
        if i + 1 < len(lines) and KEPT in lines[i + 1]:
            kept_cars, kept_wp = kept_cars + 1, kept_wp + int(m.group(1))
        else:
            lost_cars, lost_wp = lost_cars + 1, lost_wp + int(m.group(1))
    stalled = 0
    share = 0.0
    gains = GAIN.findall(text)
    for _, last in gains:
        idle = max(0.0, (SECONDS - float(last)) / SECONDS)
        share += idle
        stalled += int(idle > 1.0 / 3.0)
    # `deck_max` starts at f32::MIN, so a car that never held a node prints -3.4e38. Anything that
    # far down is "no sample", not a car under the road.
    worst = float(d.group(1)) if d else 0.0
    return dict(
        driven=sum(int(v) for v in DRIVEN.findall(text)),
        nodes=sum(int(v) for v in NODES.findall(text)),
        never=len(NEVER.findall(text)),
        on_course=float(t.group(1)) if t else 0.0,
        deck_max=worst if worst > -1e30 else 0.0,
        deck_pct=float(d.group(2)) if d else 0.0,
        had=float(h.group(1)) if h else 0.0,
        away=int(s.group(2)) if s else 0,
        junctions=int(s.group(3)) if s else 0,
        furthest=int(s.group(5)) if s else 0,
        fallen=int(s.group(6)) if s else 0,
        links=int(n.group(2)) if n else 0,
        dead=int(n.group(3)) if n else 0,
        steep=int(n.group(4)) if n else 0,
        trees=int(k.group(1)) if k else 0,
        filled=int(k.group(2)) if k else 0,
        near_pct=float(near.group(1)) if near else 0.0,
        plan_mean=float(plan.group(1)) if plan else 0.0,
        plan_30=float(plan.group(3)) if plan else 0.0,
        held_is=float(held.group(1)) if held else 0.0,
        held_nearer=float(held.group(2)) if held else 0.0,
        aim_mean=float(am.group(1)) if am else 0.0,
        aim_far=float(am.group(2)) if am else 0.0,
        aim_behind=float(ab.group(1)) if ab else 0.0,
        stalled=stalled,
        stall_share=100.0 * share / max(len(gains), 1),
        kept_cars=kept_cars,
        kept_wp=kept_wp,
        lost_cars=lost_cars,
        lost_wp=lost_wp,
    )


def arm(directory, name):
    out = {}
    for route in ROUTES:
        path = os.path.join(directory, f"{name}-{route}.log")
        out[route] = one(open(path, encoding="utf-8").read() if os.path.exists(path) else "")
    return out


def main(argv):
    if len(argv) < 2:
        sys.exit(__doc__)
    directory, names = argv[0], argv[1:]
    data = {n: arm(directory, n) for n in names}
    base = names[0]
    w = max(len(n) for n in names) + 4
    for key, label, field in COLUMNS:
        print(f"\n== {label} ==")
        print("  rota  " + "".join(f"{n:>{w}}" for n in names))
        for route in ROUTES:
            print(f"  {route}  " + "".join(f"{data[n][route][key]:>{w}.1f}" for n in names))
        total = {n: field([data[n][r][key] for r in ROUTES]) for n in names}
        print("  ALAN  " + "".join(f"{total[n]:>{w}.1f}" for n in names))
        for n in names[1:]:
            step = [data[n][r][key] - data[base][r][key] for r in ROUTES]
            margin, worst = total[n] - total[base], max(step, key=abs)
            carried = margin != 0 and abs(worst) > abs(margin)
            print(
                f"    {n} − {base}: {margin:+.1f} · en büyük tek rota {worst:+.1f}"
                f" → {'TAŞINIYOR' if carried else 'sağlam'}"
            )
    split(data, names)


def split(data, names):
    """The one reading that keeps a good change from looking like a loss.

    A car that leaves the course keeps walking the graph and keeps banking waypoints, so a change
    that *keeps cars on the road* shrinks the population that was inflating the field total. Judge
    it on the cars that stayed: how many there are, and what each of them covered.
    """
    print("\n== kursu bırakan / bırakmayan nüfus ==")
    for n in names:
        k = sum(data[n][r]["kept_cars"] for r in ROUTES)
        kw = sum(data[n][r]["kept_wp"] for r in ROUTES)
        l = sum(data[n][r]["lost_cars"] for r in ROUTES)
        lw = sum(data[n][r]["lost_wp"] for r in ROUTES)
        print(
            f"  {n:>10}  hiç bırakmayan {k:>3} araba · {kw:>5} waypoint "
            f"(araba başına {kw / max(k, 1):5.1f})"
        )
        print(
            f"  {'':>10}  bırakan       {l:>3} araba · {lw:>5} waypoint "
            f"(araba başına {lw / max(l, 1):5.1f})"
        )


if __name__ == "__main__":
    main(sys.argv[1:])
