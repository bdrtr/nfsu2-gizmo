#!/usr/bin/env python3
"""Read an eight-route sweep route by route, and say whether the field margin survives it.

A sweep arm's field total is a sum over eight routes, and a sum can be carried by one of them.
When the biggest single-route difference is larger than the whole margin, the arm did not beat
the baseline — one route did, and the other seven are noise around it. Measured 2026-08-20, that
is the case for four of the eleven comparisons behind today's pilot constants (`ROADMAP.md`).

    tools/sweep-table.py <log-dir> <baseline-arm> <arm>...

where <log-dir> holds `<arm>-<route>.log` files as written by the sweep loop:

    for r in 4001 4002 4021 4041 4061 4081 4102 4121; do
      NFS_ROUTE=".../Paths$r.bin" NFS_SECONDS=90 nfs_sim ... > $DIR/$ARM-$r.log
    done

The deciding measure is the per-car sum of "N waypoints driven past", never `SUMMARY waypoint=`.
"""
import os
import re
import sys

ROUTES = "4001 4002 4021 4041 4061 4081 4102 4121".split()
DRIVEN = re.compile(r"(\d+) waypoints driven past")


def arm(directory, name):
    """Waypoints driven past on each route, zero where the log is missing."""
    out = []
    for route in ROUTES:
        path = os.path.join(directory, f"{name}-{route}.log")
        text = open(path).read() if os.path.exists(path) else ""
        out.append(sum(int(v) for v in DRIVEN.findall(text)))
    return out


def main(argv):
    if len(argv) < 3:
        sys.exit(__doc__)
    directory, names = argv[0], argv[1:]
    cols = {n: arm(directory, n) for n in names}
    w = max(len(n) for n in names) + 2
    print("  rota  " + "".join(f"{n:>{w}}" for n in names))
    for i, route in enumerate(ROUTES):
        print(f"  {route}  " + "".join(f"{cols[n][i]:>{w}}" for n in names))
    print("  TOPLAM" + "".join(f"{sum(cols[n]):>{w}}" for n in names))
    base = names[0]
    print()
    for n in names[1:]:
        d = [cols[n][i] - cols[base][i] for i in range(len(ROUTES))]
        margin, worst = sum(d), max(d, key=abs)
        # Count ties. Two arms often differ on only three or four routes — reporting a win
        # count without them reads as a sweep of the field when it was a draw on half of it.
        win = sum(1 for v in d if v < 0)
        tie = sum(1 for v in d if v == 0)
        print(
            f"  {base} − {n}: alan {-margin:+} · rota rota {d} · "
            f"{base} {win}/8 rotada önde, {tie} berabere · en büyük tek rota {worst:+}"
            f" → {'TAŞINIYOR' if abs(worst) > abs(margin) else 'sağlam'}"
        )


if __name__ == "__main__":
    main(sys.argv[1:])
