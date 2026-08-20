#!/usr/bin/env python3
"""Compare sweep arms on measures that do not depend on which ring was driven.

Waypoints driven past is the deciding measure *within* one ring and meaningless across two: the
walked ring has roughly twice as many waypoints as the chord ring over the same roads, so its
counts are larger for no reason a driver would recognise. What survives the change of ring is the
network the cars actually covered, how far they got from the start line, and how many kept the
course — and of those, distinct nodes must be read **only over the cars that never lost the
course**, because a lost car goes on walking the graph (see the sweep memory).

    tools/ring-neutral.py <log-dir> <arm>...
"""
import os
import re
import sys

ROUTES = "4001 4002 4021 4041 4061 4081 4102 4121".split()
CAR = re.compile(
    r"^  car (\d+): .*?(\d+) junctions over\s+(\d+) distinct nodes · (\d+) waypoints driven past",
    re.M,
)


def read(directory, name):
    kept = lost = kept_nodes = lost_nodes = furthest = fallen = 0
    for route in ROUTES:
        path = os.path.join(directory, f"{name}-{route}.log")
        if not os.path.exists(path):
            continue
        text = open(path).read()
        lines = text.split("\n")
        for i, line in enumerate(lines):
            m = CAR.match(line)
            if not m:
                continue
            nodes = int(m.group(3))
            # The verdict is on the line under the car's own, where `nfs_sim` prints it.
            if "kursu hiç bırakmadı" in "\n".join(lines[i + 1 : i + 3]):
                kept += 1
                kept_nodes += nodes
            else:
                lost += 1
                lost_nodes += nodes
        s = re.search(r"furthest=(\d+)", text)
        f = re.search(r"fallen=(\d+)", text)
        furthest += int(s.group(1)) if s else 0
        fallen += int(f.group(1)) if f else 0
    return kept, lost, kept_nodes, lost_nodes, furthest, fallen


def main(argv):
    if len(argv) < 2:
        sys.exit(__doc__)
    directory, names = argv[0], argv[1:]
    w = max(len(n) for n in names) + 1
    print(
        f"  {'kol':<{w}} {'kursta':>7} {'düğüm/araba':>12} {'kaybeden':>9} "
        f"{'düğüm/araba':>12} {'furthest':>9} {'düşen':>6}"
    )
    for n in names:
        kept, lost, kn, ln, far, fell = read(directory, n)
        print(
            f"  {n:<{w}} {kept:>7} {kn / max(kept, 1):>12.1f} {lost:>9} "
            f"{ln / max(lost, 1):>12.1f} {far:>8} m {fell:>6}"
        )


if __name__ == "__main__":
    main(sys.argv[1:])
