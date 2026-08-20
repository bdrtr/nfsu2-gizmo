#!/usr/bin/env python3
"""Count the departures from the course by what the car was doing when it crossed the edge.

Thirty-four of sixty-four cars leave the course in the eight-route sweep, and until now the reasons
were read from traces a few cars at a time. `nfs_sim` prints one `çıkarken:` line per departure —
the step the car went over the corridor's edge, not the moment three seconds later when the
departure is admitted — and this buckets them.

    tools/leaving.py <log-dir> <arm>...
"""
import os
import re
import sys

ROUTES = "4001 4002 4021 4041 4061 4081 4102 4121".split()
LINE = re.compile(
    r"çıkarken: t=\s*([\d.]+)s \(\s*(-?\d+),\s*(-?\d+)\)"
    r" · \s*(-?\d+) km/h · direksiyon\s*(-?[\d.]+) · fren\s*([\d.]+)"
    r" · nişan (?:\s*(\d+) m\s*(-?\d+)°|\s+—) · hedef (?:\s*(\d+) m\s*(-?\d+)°|\s+—)"
    r" · (KAÇIŞ · )?vazgeçti (\d+)"
)


def main(argv):
    if len(argv) < 2:
        sys.exit(__doc__)
    directory, names = argv[0], argv[1:]
    for name in names:
        rows = []
        for route in ROUTES:
            path = os.path.join(directory, f"{name}-{route}.log")
            if os.path.exists(path):
                rows += [(route, m) for m in LINE.finditer(open(path).read())]
        if not rows:
            print(f"  {name}: hiç ayrılma satırı yok")
            continue
        n = len(rows)
        buckets = {
            "nişan arkada (>90°)": 0,
            "hedef arkada (>90°)": 0,
            "tam kilitte (|d| ≥ 0.84)": 0,
            "yavaş (<20 km/h)": 0,
            "hızlı (>60 km/h)": 0,
            "kaçışta": 0,
            "vazgeçtiği düğüm var": 0,
            "frende (>0.5)": 0,
        }
        for _, m in rows:
            aim = int(m.group(8)) if m.group(8) else None
            goal = int(m.group(10)) if m.group(10) else None
            speed, steer, brake = int(m.group(4)), abs(float(m.group(5))), float(m.group(6))
            if aim is not None and abs(aim) > 90:
                buckets["nişan arkada (>90°)"] += 1
            if goal is not None and abs(goal) > 90:
                buckets["hedef arkada (>90°)"] += 1
            buckets["tam kilitte (|d| ≥ 0.84)"] += steer >= 0.84
            buckets["yavaş (<20 km/h)"] += abs(speed) < 20
            buckets["hızlı (>60 km/h)"] += abs(speed) > 60
            buckets["kaçışta"] += bool(m.group(11))
            buckets["vazgeçtiği düğüm var"] += int(m.group(12)) > 0
            buckets["frende (>0.5)"] += brake > 0.5
        print(f"  {name}: {n} ayrılma")
        for k, v in sorted(buckets.items(), key=lambda x: -x[1]):
            print(f"      {k:<26} {v:>3}  (%{100 * v / n:>4.1f})")


if __name__ == "__main__":
    main(sys.argv[1:])
