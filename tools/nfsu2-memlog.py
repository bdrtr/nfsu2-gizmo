#!/usr/bin/env python3
"""Log the running game's own physics, for setting Gizmo's car against NFSU2's.

`SPEED2.EXE` (v1.2) keeps every car's suspension, wheels and body as heap objects whose first
dword is their class's vtable. This finds the Suspension objects (`0x7A210C`) in the process's
memory, follows each to its Wheel (`+0xA0`) and RigidBody (`+0xA4`), and samples every four-wheeled
body about 30 times a second: speed, yaw rate, lateral and longitudinal acceleration, the four
wheel loads, suspension compression and contact. How the offsets were read is in `ROADMAP.md`,
"Oyunun kendi süspansiyonu ve lastiği", and in PryHUB's `gizmo_nfs::Chassis`.

    tools/nfsu2-memlog.py log <out.csv> [seconds]      # sample a game that is already running
    tools/nfsu2-memlog.py launch <out.csv> [seconds]   # start the game first, then sample it
    tools/nfsu2-memlog.py loads <out.csv>              # the table that found the downforce
    tools/nfsu2-memlog.py movers [seconds] [launch]    # which class moves which car, printed

**Why `launch` exists.** With `kernel.yama.ptrace_scope = 1` a process's memory can only be read
by one of its ancestors, so a game started from Lutris cannot be read from here. `launch` starts
it under umu as this script's own child. It needs:

    NFSU2_PREFIX   the Wine/Proton prefix the game is installed in
    PROTONPATH     the Proton build umu should run (Lutris keeps them under runners/wine/)
    UMU_RUN        umu-run, if it is not on PATH
    NFSU2_PRELAUNCH  optional, a script to start beside the game (an affinity pin, say)

Columns: `t` seconds · `rb` the body's address, one per car · `mass_t` · `speed_kmh` · `yaw_dps` ·
`alat_vw_g` lateral acceleration as speed × yaw rate (overreads while the car slides) ·
`alat_dv_g` / `along_dv_g` from the change of velocity in the car's frame (noisy: the sample clock
is not the game's) · `fz0..3` wheel loads in N, front-left, front-right, rear-left, rear-right ·
`x0..3` suspension compression in m · `c0..3` contact · `pos_*` world position.

The first measurement (2026-10-25, a 240SX, `loads`): the four loads summed read 1.00 / 1.43 /
1.65 / 1.90 × the car's weight at 20 / 60 / 100 / 140 km/h, whatever the cornering — the game's
downforce, linear in speed.
"""
import csv
import math
import os
import re
import statistics
import struct
import subprocess
import sys
import time

SUSP_VT = 0x7A210C
WHEEL_VT = 0x7A20E8
RB_VT = 0x7A20C4
G = 9.8


# What `0x5F46A0` builds, by the number it is asked for. A car holds its mover at `+0x34`.
MOVERS = {
    0x7A5BB0: "tür0",
    0x7A25E0: "PhysMov",
    0x78456C: "tür2",
    0x7A2538: "MeloMvr",
    0x7A2144: "tür4",
    0x7A2590: "tür5",
}
# In a car: its `DriverInfo` and its speed. In a `DriverInfo`: the kind (`0x6373E0`'s second
# argument: 2 a rival, 3 traffic; 1 taken to be a person, from `0x5F46A0`), skill, name.
CAR_DRIVER, CAR_MOVER, CAR_SPEED = 0x14, 0x34, 0x430
DRIVER_KIND, DRIVER_SKILL, DRIVER_NAME = 0x4, 0x901, 0x90C
# Fixed addresses, the image is not relocated.
STATE = {
    "bakış_ayarı": 0x83AA74,
    "zorluk": 0x89E7F4,
}
FLAGS = {"drag?": 0x89E7D8, "drift": 0x89E7D9, "tür2": 0x89E7E3}


def pid_of():
    out = subprocess.run(["pgrep", "-x", "SPEED2.EXE"], capture_output=True, text=True).stdout.split()
    return int(out[0]) if out else None


class Mem:
    def __init__(self, pid):
        self.pid = pid
        self.f = open(f"/proc/{pid}/mem", "rb", buffering=0)

    def read(self, addr, n):
        self.f.seek(addr)
        return self.f.read(n)

    def u32(self, a):
        return struct.unpack("<I", self.read(a, 4))[0]

    def f32(self, a, n=1):
        v = struct.unpack(f"<{n}f", self.read(a, 4 * n))
        return v if n > 1 else v[0]

    def regions(self):
        for line in open(f"/proc/{self.pid}/maps"):
            m = re.match(r"([0-9a-f]+)-([0-9a-f]+) (\S+)", line)
            lo, hi, perm = int(m[1], 16), int(m[2], 16), m[3]
            # The 32-bit game's heap lives below 4 GiB, readable and writable.
            if "rw" in perm[:2] and hi <= 0x1_0000_0000 and hi - lo <= 512 << 20:
                yield lo, hi


def find_objects(mem, vt):
    pat = struct.pack("<I", vt)
    found = []
    for lo, hi in mem.regions():
        try:
            data = mem.read(lo, hi - lo)
        except OSError:
            continue
        i = data.find(pat)
        while i >= 0:
            if (lo + i) % 4 == 0:
                found.append(lo + i)
            i = data.find(pat, i + 1)
    return found


def cars(mem):
    """Body address -> its four (suspension, wheel) pairs, front-left first."""
    by_rb = {}
    for s in find_objects(mem, SUSP_VT):
        try:
            wheel, rb = mem.u32(s + 0xA0), mem.u32(s + 0xA4)
            if mem.u32(wheel) != WHEEL_VT or mem.u32(rb) != RB_VT:
                continue
        except (OSError, struct.error):
            continue
        by_rb.setdefault(rb, []).append((s, wheel))
    out = {}
    for rb, v in by_rb.items():
        if len(v) != 4:
            continue
        # Each corner's position relative to the centre of gravity, x forward and y left.
        pos = {s: mem.f32(s + 0x90, 2) for s, _ in v}
        out[rb] = sorted(v, key=lambda sw: (-pos[sw[0]][0] > 0, -pos[sw[0]][1]))
    return out


def launch(out):
    prefix = os.environ.get("NFSU2_PREFIX")
    proton = os.environ.get("PROTONPATH")
    if not prefix or not proton:
        sys.exit("launch needs NFSU2_PREFIX and PROTONPATH — see the header")
    env = dict(os.environ, GAMEID="umu-default", WINEPREFIX=prefix, WINEDLLOVERRIDES="dinput8=n,b")
    cwd = os.path.join(prefix, "drive_c", "Need for Speed Underground 2")
    log = open(out + ".game.log", "w")
    subprocess.Popen([os.environ.get("UMU_RUN", "umu-run"), "SPEED2.EXE"],
                     cwd=cwd, env=env, stdout=log, stderr=log)
    if os.environ.get("NFSU2_PRELAUNCH"):
        subprocess.Popen([os.environ["NFSU2_PRELAUNCH"]], stdout=log, stderr=log)


def log(out, seconds):
    pid = None
    while pid is None:
        pid = pid_of()
        if pid is None:
            time.sleep(1)
    mem = Mem(pid)
    print(f"pid {pid}", flush=True)
    t0 = time.time()
    found, last_scan, prev = {}, 0.0, {}
    with open(out, "w") as f:
        f.write("t,rb,mass_t,speed_kmh,yaw_dps,alat_vw_g,alat_dv_g,along_dv_g,"
                "fz0,fz1,fz2,fz3,x0,x1,x2,x3,c0,c1,c2,c3,pos_x,pos_y,pos_z\n")
        while time.time() - t0 < seconds and os.path.exists(f"/proc/{pid}"):
            now = time.time()
            # Cars come and go with races; look for them again every few seconds.
            if now - last_scan > 5 or not found:
                try:
                    found = cars(mem)
                except OSError:
                    found = {}
                last_scan = now
                print(f"{now - t0:6.1f}s cars: {len(found)}", flush=True)
            for rb, sw in list(found.items()):
                try:
                    if mem.u32(rb) != RB_VT:
                        continue
                    mass = mem.f32(rb + 0x164)
                    pos = mem.f32(rb + 0x20, 3)
                    rot = mem.f32(rb + 0x30, 12)  # three rows of four: forward, left, up
                    v = mem.f32(rb + 0xA0, 3)
                    w = mem.f32(rb + 0xC0, 3)
                    fz, xs, cs = [], [], []
                    for s, wh in sw:
                        fz.append(mem.f32(wh + 0x164) * 1000.0)  # kN in the game
                        xs.append(mem.f32(s + 0x1C))
                        cs.append(mem.u32(s + 0x80))
                except (OSError, struct.error):
                    continue
                fwd, left, up = rot[0:3], rot[4:7], rot[8:11]
                speed = math.sqrt(sum(c * c for c in v))
                yaw = sum(a * b for a, b in zip(w, up))
                alat_dv = along_dv = float("nan")
                p = prev.get(rb)
                if p and now - p[0] > 1e-3:
                    dv = [(a - b) / (now - p[0]) for a, b in zip(v, p[1])]
                    alat_dv = sum(a * b for a, b in zip(dv, left)) / G
                    along_dv = sum(a * b for a, b in zip(dv, fwd)) / G
                prev[rb] = (now, v)
                f.write(f"{now - t0:.3f},{rb:#x},{mass:.3f},{speed * 3.6:.1f},{math.degrees(yaw):.1f},"
                        f"{speed * yaw / G:.3f},{alat_dv:.3f},{along_dv:.3f},"
                        + ",".join(f"{x:.0f}" for x in fz) + ","
                        + ",".join(f"{x:.4f}" for x in xs) + ","
                        + ",".join(str(c) for c in cs) + ","
                        + ",".join(f"{x:.1f}" for x in pos) + "\n")
            f.flush()
            time.sleep(1 / 30)


def loads(path):
    """Summed wheel load over weight, by speed and by how hard the car is turning, per car."""
    rows = list(csv.DictReader(open(path)))
    by_car = {}
    for r in rows:
        by_car.setdefault(r["rb"], []).append(r)
    for rb, rs in by_car.items():
        moving = [r for r in rs if float(r["speed_kmh"]) > 20]
        if not moving:
            continue
        weight = float(rs[0]["mass_t"]) * G * 1000.0
        print(f"{rb} · {rs[0]['mass_t']} t · {len(moving)} moving samples")
        print("  speed      |  ΣFz/weight, by |speed × yaw rate|:  <0.3 g   0.3-1   1-2   2-3   3-4")
        bands = [(0, 0.3), (0.3, 1), (1, 2), (2, 3), (3, 4)]
        for lo in (0, 40, 80, 120, 160):
            cells = []
            for a0, a1 in bands:
                sel = [sum(float(r[f"fz{k}"]) for k in range(4)) / weight for r in rs
                       if lo <= float(r["speed_kmh"]) < lo + 40
                       and a0 <= abs(float(r["alat_vw_g"])) < a1
                       and all(r[f"c{k}"] == "1" for k in range(4))]
                cells.append(f"{statistics.median(sel):.2f}({len(sel)})" if sel else "  -  ")
            print(f"  {lo:3d}-{lo + 40:3d} km/h | " + "  ".join(cells))


def mover_cars(mem):
    """Every car found through its mover: car address -> the mover's address."""
    movers = [m for vt in MOVERS for m in find_objects(mem, vt)]
    out = {}
    for lo, hi in mem.regions():
        try:
            data = mem.read(lo, hi - lo)
        except OSError:
            continue
        for m in movers:
            pat = struct.pack("<I", m)
            i = data.find(pat)
            while i >= 0:
                car = lo + i - CAR_MOVER
                if (lo + i) % 4 == 0 and describe(mem, car) is not None:
                    out[car] = m
                i = data.find(pat, i + 1)
    return out


def describe(mem, car):
    """What a car says about itself, or None if this address is not one."""
    try:
        driver = mem.u32(car + CAR_DRIVER)
        kind = mem.u32(driver + DRIVER_KIND)
        if kind not in (1, 2, 3):
            return None
        mover = MOVERS.get(mem.u32(mem.u32(car + CAR_MOVER)))
        if mover is None:
            return None
        skill = struct.unpack("<b", mem.read(driver + DRIVER_SKILL, 1))[0]
        name = mem.read(driver + DRIVER_NAME, 16).split(b"\0")[0].decode("ascii", "replace")
        return kind, mover, skill, name, mem.f32(car + CAR_SPEED) * 3.6
    except (OSError, struct.error, TypeError):
        return None


def movers(seconds, start_game):
    """Print which class moves which car, each time that answer changes."""
    if start_game:
        launch("/tmp/nfsu2-movers")
    pid = pid_of()
    for _ in range(120 if start_game else 0):
        if pid is not None:
            break
        time.sleep(1.0)
        pid = pid_of()
    if pid is None:
        sys.exit("SPEED2.EXE is not running")
    mem = Mem(pid)
    kinds = {1: "oyuncu", 2: "rakip", 3: "trafik"}
    start, cars_at, seen = time.time(), {}, None
    while time.time() - start < seconds:
        try:
            cars_at = mover_cars(mem) or cars_at
        except OSError as e:
            sys.exit(f"{e}: with ptrace_scope 1 only a game this script started can be read; add `launch`")
        rows = sorted(filter(None, (describe(mem, c) for c in cars_at)), key=lambda r: (r[0], r[3]))
        now = (
            tuple((r[0], r[1], r[2], r[3]) for r in rows),
            tuple(mem.u32(a) for a in STATE.values()),
            tuple(mem.read(a, 1)[0] for a in FLAGS.values()),
        )
        if now != seen:
            seen = now
            print(f"--- t={time.time() - start:.0f} s")
            print("   ", " ".join(f"{k}={mem.u32(a)}" for k, a in STATE.items()),
                  " ".join(f"{k}={mem.read(a, 1)[0]}" for k, a in FLAGS.items()))
            for kind, mover, skill, name, kmh in rows:
                print(f"    {kinds[kind]:<7} {mover:<8} beceri={skill:<3} {kmh:6.1f} km/h  {name}")
            sys.stdout.flush()
        time.sleep(1.0)


def main():
    if len(sys.argv) >= 2 and sys.argv[1] == "movers":
        movers(float(sys.argv[2]) if len(sys.argv) > 2 else 600, "launch" in sys.argv)
        return
    if len(sys.argv) < 3 or sys.argv[1] not in ("log", "launch", "loads"):
        sys.exit(__doc__)
    cmd, out = sys.argv[1], sys.argv[2]
    if cmd == "loads":
        loads(out)
        return
    seconds = float(sys.argv[3]) if len(sys.argv) > 3 else 600
    if cmd == "launch":
        launch(out)
    log(out, seconds)


if __name__ == "__main__":
    main()
