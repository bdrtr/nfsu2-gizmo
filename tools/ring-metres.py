#!/usr/bin/env python3
"""Progress in **metres of the ring the car actually drove**, which is the only column two
different courses can be compared on.

`sweep-table.py` counts waypoints driven past, and that count is a ruler made from the ring itself:
change the ring and the ruler changes with it. The house rule has been to refuse the column outright
when an arm moves the ring's length — but "refuse" leaves the deciding question unanswered, because
every course-building change moves it. Multiplying each car's waypoints by the ring's own mean
spacing (`length / waypoints`, both printed by `NFS_CURVE=1`) turns the count into metres, and metres
mean the same thing on both rings.

    tools/ring-metres.py <log-dir> <curve-dir> <arm>[:ring] ...

`<log-dir>` holds the sweep's `<arm>-<route>.log` files; `<curve-dir>` holds a
`NFS_CURVE=1 NFS_SECONDS=1` run per ring, named the same way. Arms that share a ring name it after
the colon — the ring depends only on `NFS_WALKLINE`/`NFS_WALKFIT`/`NFS_WPSTEP`, so a brake or
lookahead sweep drives one ring under many arm names:

    tools/ring-metres.py logs curves taban dort fren40:dort fren35:dort

The first arm is the baseline every other is compared against. Two columns are printed for each:
everything, and only the cars that were **still gaining waypoints in the last third of the race** —
because time on the corridor and cars-that-never-leave both score a stopped car as a success, and a
stopped car earns zero metres here by construction. Each comparison carries the field margin, the
biggest single-route difference and the same TAŞINIYOR verdict the other tools print.
"""
import os
import re
import sys

ROUTES = "4001 4002 4021 4041 4061 4081 4102 4121".split()
LENGTH = re.compile(r"halkanın şekli: (\d+) m uzunluk")
COUNT = re.compile(r"(\d+) waypoint'in \d+ tanesi en yakın")
CAR = re.compile(r"(\d+) waypoints driven past, last gained at\s*([\d.]+)s")
# A car that gained nothing in the last third of the race has stopped — see `sweep-columns.py`.
MOVING_AFTER = 60.0


def spacing(curve_dir, ring, route):
    """The ring's own mean waypoint spacing in metres, from its `NFS_CURVE=1` run."""
    path = os.path.join(curve_dir, f"{ring}-{route}.log")
    if not os.path.exists(path):
        sys.exit(f"halka ölçüsü yok: {path}\n(NFS_CURVE=1 NFS_SECONDS=1 ile bir koşu gerekiyor)")
    text = open(path, encoding="utf-8", errors="replace").read()
    length, count = LENGTH.search(text), COUNT.search(text)
    if not length or not count:
        sys.exit(f"halka satırı okunamadı: {path}")
    return int(length.group(1)) / max(int(count.group(1)), 1)


def read(log_dir, curve_dir, arm, ring):
    """Metres driven per route, and the same restricted to cars still gaining at the end."""
    every, moving, cars = {}, {}, {}
    for route in ROUTES:
        path = os.path.join(log_dir, f"{arm}-{route}.log")
        if not os.path.exists(path):
            every[route] = moving[route] = cars[route] = 0
            continue
        step = spacing(curve_dir, ring, route)
        text = open(path, encoding="utf-8", errors="replace").read()
        every[route] = moving[route] = 0.0
        cars[route] = 0
        for m in CAR.finditer(text):
            driven, last = int(m.group(1)), float(m.group(2))
            every[route] += driven * step
            if last >= MOVING_AFTER:
                moving[route] += driven * step
                cars[route] += 1
    return every, moving, cars


def main(argv):
    if len(argv) < 4:
        sys.exit(__doc__)
    log_dir, curve_dir, names = argv[0], argv[1], argv[2:]
    arms = [(n.split(":")[0], n.split(":")[-1]) for n in names]
    data = {arm: read(log_dir, curve_dir, arm, ring) for arm, ring in arms}
    for title, index in (("metre (bütün arabalar)", 0), ("metre (hâlâ ilerleyenler)", 1)):
        print(f"\n== {title} ==")
        print("  rota" + "".join(f"{a:>14}" for a, _ in arms))
        for route in ROUTES:
            print(f"  {route}" + "".join(f"{data[a][index][route]:14.0f}" for a, _ in arms))
        print("  ALAN" + "".join(f"{sum(data[a][index].values()):14.0f}" for a, _ in arms))
        base = data[arms[0][0]][index]
        for arm, _ in arms[1:]:
            diff = {r: data[arm][index][r] - base[r] for r in ROUTES}
            margin, biggest = sum(diff.values()), max(diff.values(), key=abs)
            verdict = "TAŞINIYOR" if abs(biggest) > abs(margin) else "sağlam"
            won = sum(1 for v in diff.values() if v > 0)
            print(f"    {arm} − {arms[0][0]}: {margin:+.0f} m · en büyük tek rota {biggest:+.0f}"
                  f" → {verdict} · {won}/8 rotada ileride")
    print("\n== hâlâ ilerleyen araba ==")
    print("  rota" + "".join(f"{a:>14}" for a, _ in arms))
    for route in ROUTES:
        print(f"  {route}" + "".join(f"{data[a][2][route]:14}" for a, _ in arms))
    print("  ALAN" + "".join(f"{sum(data[a][2].values()):14}" for a, _ in arms))


if __name__ == "__main__":
    main(sys.argv[1:])
