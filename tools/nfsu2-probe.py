#!/usr/bin/env python3
"""Check SPEED2-ATLAS.md's static readings against the running game's memory.

Starts the game as this script's child (`kernel.yama.ptrace_scope = 1` only lets an ancestor read
another process's memory — see `nfsu2-memlog.py`, whose launcher and reader this reuses), samples a
set of fixed addresses every two seconds while you play, and at the end prints, per claim, what was
seen and whether it agrees with the atlas.

    tools/nfsu2-probe.py <log.csv> [seconds]     # launch, probe, summarise (default 900 s)

Environment: as `nfsu2-memlog.py launch` — NFSU2_PREFIX, PROTONPATH, UMU_RUN, NFSU2_PRELAUNCH.

What is checked (the image is not relocated, so every address below is fixed):

* game-flow state `0x8654A4` (§2, §23.1) and the race-parameter block `0x89E7A0` (§21.1);
* the clock: the simulation tick `0x86518C` and the real tick `0x8651AC` run at **4000 per second**
  (§2, §18.1);
* no day/night on PC: the gate `0x870D14` and the blend `0x8A1C88`/`0x8A1C90` stay 0 (§15.4);
* the fog the renderer gets, `0x89E850` (§15.2): start (`+0x0C`) and density (`+0x10`) in range;
* the career: cash `0x861E74`, stage `0x862838` (§10);
* the atlas's "never written" globals (`NEVER_WRITTEN`) stay 0 — a static search for a literal address
  misses fields written through an object pointer, which is how §15.4's day/night gate was got wrong;
* the streamer `0x88EB00` (§13.1): state 0..6, never more than two reads in flight;
* **the engine curve each car drives on** (§22.2): every Engine object (vtable `0x7A235C`) points at
  its physics copy's torque table (`+0x34` → copy `+0x200`) and boost table (`+0x5C`); the copy
  sits `0x30` into the physics info object (`+0x00` car index, `+0x2E0` upgrade levels), so the
  torque the car drives on can be compared, point by point, with the car's record:
  `stock + gain[0]·w_engine + boost + gain[3]·w_ecu`, the boost moved toward its target by
  `w_turbo`, `w = level / 3`. Only cars driven by the player have an Engine object (rivals do not use
  the physics mover), so drive a turbocharged car (SKYLINE, RX7, LANCEREVO8, IMPREZAWRX, ECLIPSE,
  3000GT, GOLF) to test the boost term.
"""
import importlib.util
import math
import os
import struct
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("memlog", os.path.join(HERE, "nfsu2-memlog.py"))
memlog = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(memlog)

ENGINE_VT = 0x7A235C
FLOW = ["NONE", "LOADING_FRONTEND", "UNLOADING_FRONTEND", "IN_FRONTEND", "LOADING_REGION",
        "LOADING_TRACK", "RACING", "UNLOADING_TRACK", "UNLOADING_REGION", "EXIT_DEMO_DISC"]
CTI_SIZE = 0x890
# Addresses the atlas calls "never written" from a static search for their literal address. That search
# missed the day/night gate, which an object's constructor writes as `this+0x64` (§24), so each of these
# is watched: anything but 0 means the claim is wrong. (name, address, size, atlas section)
NEVER_WRITTEN = [
    ("joylog bayrağı", 0x8650C8, 4, "§18.2.6"),
    ("milyon nakit bayrağı", 0x838440, 1, "§10.12.7"),
    ("her şey açık bayrağı", 0x838441, 1, "§10.7"),
    ("sis geliştirici sabitleri", 0x8A1D70, 4, "§15.2"),
    ("yağmur zorlaması", 0x8A1D48, 4, "§15.7"),
    ("HUD'suz kip", 0x836474, 4, "§20.5"),
    ("0x890100", 0x890100, 4, "§21.4"),
    ("drift ikinci bayrağı", 0x890104, 4, "§21.4"),
    ("drag ikinci bayrağı", 0x890108, 4, "§21.4"),
    ("Street X ikinci bayrağı", 0x89010C, 4, "§21.4"),
    ("0x890110", 0x890110, 4, "§21.4"),
]
CATS = ["WT", "TR", "NO", "EN", "TU", "SU", "BR", "EC", "TI", "AE"]


def cti_records(prefix):
    """The CarTypeInfo records of GLOBALB, by index (chunk 0x34600, 16-aligned, 0x890 each)."""
    path = os.path.join(prefix, "drive_c", "Need for Speed Underground 2", "GLOBAL", "GLOBALB.BUN")
    b = open(path, "rb").read()
    o = 0
    while o + 8 <= len(b):
        cid, sz = struct.unpack_from("<II", b, o)
        if cid == 0x34600:
            base = (o + 0x17) & ~0xF
            n = (o + 8 + sz - base) // CTI_SIZE
            return [b[base + i * CTI_SIZE: base + (i + 1) * CTI_SIZE] for i in range(n)]
        o += 8 + sz
    sys.exit("no 0x34600 in GLOBALB.BUN")


def f9(rec, off):
    return list(struct.unpack_from("<9f", rec, off))


def expected_torque(rec, levels):
    """The drive's torque in kN·m for a record and the ten category levels (SPEED2-ATLAS §22.2)."""
    w_en, w_tu, w_ec = (min(max(levels[i] / 3.0, 0.0), 1.0) for i in (3, 4, 7))
    stock, boost, target = f9(rec, 0x310), f9(rec, 0x334), f9(rec, 0x620)
    g0, g3 = f9(rec, 0x530), f9(rec, 0x5F0)
    out = []
    for i in range(9):
        b = boost[i] + (target[i] - boost[i]) * w_tu
        out.append(stock[i] + g0[i] * w_en + b + g3[i] * w_ec)
    return out


def expected_rpm(rec, levels):
    """Idle and limiter after the ECU's rpm block: level L = ⌊levels[7] + ½⌋, block `+0x520 + 0x40·L`."""
    idle, _, lim = struct.unpack_from("<3f", rec, 0x300)
    if levels[7] != 0.0:
        L = int(levels[7] + 0.5)
        if 1 <= L <= 3:
            d = struct.unpack_from("<3f", rec, 0x520 + 0x40 * L)
            idle, lim = idle + d[0], lim + d[2]
    return idle, lim


class Probe:
    def __init__(self, mem, records):
        self.mem, self.records = mem, records
        self.seen = {}          # claim -> list of (ok, text)
        self.prev_ticks = None
        self.engines, self.last_scan = [], 0.0

    def note(self, claim, ok, text):
        self.seen.setdefault(claim, []).append((ok, text))

    def sample(self, now):
        m = self.mem
        flow = m.u32(0x8654A4)
        racing = flow == 6
        self.note("oyun akışı 0x8654A4 (§2, §23.1)", flow < len(FLOW), FLOW[flow] if flow < len(FLOW) else hex(flow))

        sim, real = m.u32(0x86518C), m.u32(0x8651AC)
        if self.prev_ticks:
            t0, s0, r0 = self.prev_ticks
            dt = now - t0
            if dt > 0.5:
                rs = (real - r0) / dt
                self.note("gerçek tik 4000/s 0x8651AC (§18.1)", abs(rs - 4000) < 400, f"{rs:.0f}/s")
                if racing:
                    ss = (sim - s0) / dt
                    self.note("sim tiki 4000/s yarışta 0x86518C (§2)", abs(ss - 4000) < 400, f"{ss:.0f}/s")
        self.prev_ticks = (now, sim, real)

        gate, blend, flag = m.u32(0x870D14), m.f32(0x8A1C88), m.read(0x8A1C90, 1)[0]
        self.note("gece/gündüz kapısı 0x870D14 = 0 (§15.4)", gate == 0, str(gate))
        self.note("gece ağırlığı 0x8A1C88/0x8A1C90 = 0 (§15.4)", blend == 0.0 and flag == 0, f"{blend:g}/{flag}")

        if racing:
            start, density, colour = m.f32(0x89E85C), m.f32(0x89E860), m.u32(0x89E864)
            ok = 0.0 <= start < 250.0 and 0.0 <= density <= 1.0
            self.note("sis çıktısı 0x89E850 (§15.2)", ok, f"başlangıç {start:g} m, yoğunluk {density:g}, renk {colour:#08x}")

        cash, stage = struct.unpack("<i", m.read(0x861E74, 4))[0], m.u32(0x862838)
        self.note("kariyer nakit/aşama (§10)", 0 <= stage <= 7, f"${cash}, aşama {stage}")

        ev, mode, traffic = m.u32(0x89E7A0), m.u32(0x89E7B0), m.u32(0x89E7A8)
        ai, laps, diff = m.u32(0x89E7C4), m.u32(0x89E7BC), m.u32(0x89E7F4)
        self.note("yarış parametreleri 0x89E7A0 (§21.1)", mode in (0, 1, 3, 5, 7, 8),
                  f"olay {ev}, kip {mode}, trafik {traffic}, AI {ai}, tur {laps}, zorluk {diff}")

        for name, addr, size, ref in NEVER_WRITTEN:
            v = m.u32(addr) if size == 4 else m.read(addr, 1)[0]
            self.note(f"'hiç yazılmıyor': {name} {addr:#x} = 0 ({ref})", v == 0, str(v))

        ts = 0x88EB00
        state, inflight, active = m.u32(ts + 0x168), m.u32(ts + 0x20), m.u32(ts + 0x24)
        section = struct.unpack("<h", m.read(ts + 0xB0 + 0x16, 2))[0]
        self.note("akış durumu 0..6, uçuşta ≤ 2 (§13.1)", state <= 6 and inflight <= 2,
                  f"durum {state}, uçuşta {inflight}, etkin {active}, bölge {section}")

        if now - self.last_scan > 10 or not self.engines:
            try:
                self.engines = memlog.find_objects(m, ENGINE_VT)
            except OSError:
                self.engines = []
            self.last_scan = now
        for e in self.engines:
            try:
                if m.u32(e) != ENGINE_VT:
                    continue
                tq_p, boost_p = m.u32(e + 0x34), m.u32(e + 0x5C)
                if boost_p - tq_p != 0x24:
                    continue
                info = tq_p - 0x200 - 0x30
                idx = m.u32(info)
                if idx >= len(self.records):
                    continue
                levels = m.f32(info + 0x2E0, 10)
                got = m.f32(tq_p, 9)
                idle_lim = (m.f32(e + 0x28), m.f32(e + 0x2C))
            except (OSError, struct.error):
                continue
            rec = self.records[idx]
            name = rec[:0x10].split(b"\0")[0].decode("latin1")
            want = expected_torque(rec, levels)
            diff = max(abs(a - b) for a, b in zip(got, want)) * 1000.0
            stock_only = max(abs(a - b) for a, b in zip(got, f9(rec, 0x310))) * 1000.0
            turbo = any(v > 0 for v in f9(rec, 0x334))
            lv = " ".join(f"{c}{levels[i]:.1f}" for i, c in enumerate(CATS) if levels[i])
            want_rpm = expected_rpm(rec, levels)
            rpm_ok = abs(idle_lim[0] - want_rpm[0]) < 1 and abs(idle_lim[1] - want_rpm[1]) < 1
            self.note(f"motor devir sınırları, ECU bloğu (§22.2): {name}", rpm_ok,
                      f"rölanti/kesici {idle_lim[0]:.0f}/{idle_lim[1]:.0f}, beklenen {want_rpm[0]:.0f}/{want_rpm[1]:.0f}")
            self.note(f"motor eğrisi = stok + turbo + kazançlar (§22.2): {name}{' (turbo)' if turbo else ''}",
                      diff < 0.5, f"fark {diff:.2f} N·m (yalnız stokla {stock_only:.1f}); seviyeler: {lv or 'stok'}")

    def summary(self):
        lines = []
        for claim, obs in self.seen.items():
            oks = [o for o, _ in obs]
            verdict = "TUTTU" if all(oks) else ("TUTMADI" if not any(oks) else "KARIŞIK")
            texts = []
            for _, t in obs:
                if t not in texts:
                    texts.append(t)
            sample = "; ".join(texts[:4]) + (f"; … (+{len(texts) - 4})" if len(texts) > 4 else "")
            lines.append(f"{verdict:8s} {claim}  [{len(obs)} örnek] {sample}")
        return "\n".join(lines)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    seconds = float(sys.argv[2]) if len(sys.argv) > 2 else 900.0
    prefix = os.environ.get("NFSU2_PREFIX") or sys.exit("NFSU2_PREFIX gerekli — başlığa bak")
    records = cti_records(prefix)
    memlog.launch(out)
    pid = None
    while pid is None:
        pid = memlog.pid_of()
        if pid is None:
            time.sleep(1)
    mem = memlog.Mem(pid)
    print(f"pid {pid}; {len(records)} araba kaydı; {seconds:.0f} s", flush=True)
    probe = Probe(mem, records)
    t0 = time.time()
    with open(out, "w", encoding="utf-8") as f:
        f.write("t,iddia,tuttu,değer\n")
        while time.time() - t0 < seconds and os.path.exists(f"/proc/{pid}"):
            now = time.time()
            before = {k: len(v) for k, v in probe.seen.items()}
            try:
                probe.sample(now)
            except (OSError, struct.error) as e:
                print(f"{now - t0:6.1f}s okunamadı: {e}", flush=True)
            for claim, obs in probe.seen.items():
                for ok, text in obs[before.get(claim, 0):]:
                    f.write(f'{now - t0:.1f},"{claim}",{int(bool(ok))},"{text}"\n')
            f.flush()
            time.sleep(2.0)
    report = probe.summary()
    print("\n" + report, flush=True)
    open(out + ".özet.txt", "w", encoding="utf-8").write(report + "\n")


if __name__ == "__main__":
    main()
