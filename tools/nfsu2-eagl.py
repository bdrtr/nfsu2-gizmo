#!/usr/bin/env python3
"""NFSU2 EAGL animasyon kanalları — çözücü (SPEED2-ATLAS.md §17.9). Yalnız okur.

`tools/nfsu2-anim.py`'nin `Elf`/`bank`/`walk` yardımcılarını kullanır (yanında olmalı ya da --anim ile verilir).

    tools/nfsu2-eagl.py <dosya> [ad-parçası] [--frames a,b,c] [--anim tools/nfsu2-anim.py]

Bir bankadaki her anim (tür 0xF birleşik) için çocuk kanallarını çözer:
  tür 0x00  ham kare dizisi (program: 0 dördey, 1 Euler°, 2 öteleme)          — 0x6E7790
  tür 0x0A  DeltaF: her kare için kanal başına 4/8/16 bitlik delta              — 0x6E4950, akış 0x6EFA20
  tür 0x0B  DeltaQ: DeltaF ile aynı akış, kanallar dördey (4 float)             — 0x6E4C50
  tür 0x14  anahtarlı vec3: blok başı u16 anahtar + u8 deltalar                — 0x6E9780, tablo 0x6E89C0
  tür 0x15  anahtarlı skaler: aynı blok düzeni, kanal başına 1 değer            — 0x6E82D0, tablo 0x6E78F0
  tür 0x11  anahtarlı dördey: u16×3 ilk anahtar (w işareti 0. bitte) + 3 baytlık delta — 0x6ED680, 0x6ECB40, 0x6ECA40
Çıktı değerleri poz tamponunun float yuvalarına gider: yuva/12 = kemik (kemik başına 12 float = 0x30 bayt).
"""
import importlib.util
import math
import os
import struct
import sys


def load_anim_module(path):
    spec = importlib.util.spec_from_file_location('nfsu2_anim', path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


u16 = lambda d, o: struct.unpack_from('<H', d, o)[0]
u32 = lambda d, o: struct.unpack_from('<I', d, o)[0]
f32 = lambda d, o: struct.unpack_from('<f', d, o)[0]


# --- tür 0: ham ---------------------------------------------------------------------------------

def raw(d, o):
    """{u16 0, u16 imza, u32 n, u32 kare başına float, u32 kare, u32 örnek_prog[n], u32 ara_prog[n], f32 veri}.
    Program: kemik başına {sayı, op…}; op 0 dördey (4 float, xyzw), 1 Euler derece (3 float, 0x6E7130), 2 öteleme (3)."""
    n, stride, frames = u32(d, o + 4), u32(d, o + 8), u32(d, o + 0xC)
    prog = [u32(d, o + 0x10 + 4 * i) for i in range(n)]
    groups, i = [], 0
    while i < n:
        c = prog[i]
        groups.append(prog[i + 1:i + 1 + c])
        i += 1 + c
    base = o + 4 * (2 * n + 4)
    out = []
    for f in range(frames):
        p = base + 4 * stride * f
        fr = []
        for ops in groups:
            bone = {}
            for op in ops:
                k = 4 if op == 0 else 3
                vals = struct.unpack_from(f'<{k}f', d, p)
                p += 4 * k
                bone[('q', 'euler', 't')[op]] = vals
            fr.append(bone)
        out.append(fr)
    return dict(kind='ham', frames=frames, groups=groups, values=out)


# --- tür 0xA / 0xB: delta akışı ---------------------------------------------------------------------

def delta_stream(d, s, frames):
    """Akış: {u16 n, u8 bit (4/8/16), u8, n × {f32 sapma, f32 ölçek, f32 taban}, deltalar}; kare k→k+1 için n delta,
    değer += sapma + ölçek·u (u işaretsiz; 4 bitte alt yarım bayt önce)."""
    n, bits = u16(d, s), d[s + 2]
    recs = [struct.unpack_from('<3f', d, s + 4 + 12 * i) for i in range(n)]
    p = s + 4 + 12 * n
    cur = [r[2] for r in recs]
    out = [list(cur)]
    per = {16: 2 * n, 8: n, 4: (n + 1) // 2}[bits]
    for k in range(frames - 1):
        q = p + per * k
        if bits == 16:
            us = struct.unpack_from(f'<{n}H', d, q)
        elif bits == 8:
            us = d[q:q + n]
        else:
            us = []
            for i in range(n):
                by = d[q + i // 2]
                us.append(by & 0xF if i % 2 == 0 else by >> 4)
        cur = [c + r[0] + r[1] * u for c, r, u in zip(cur, recs, us)]
        out.append(list(cur))
    return n, bits, out


def delta(d, o, quat):
    s, frames = u32(d, o + 4), u16(d, o + 8)
    n, bits, vals = delta_stream(d, s, frames)
    ns = n // 4 if quat else n
    slots = [u16(d, o + 0xA + 2 * i) for i in range(ns)]
    return dict(kind='DeltaQ' if quat else 'DeltaF', frames=frames, bits=bits, n=n, slots=slots, values=vals)


# --- tür 0x14 / 0x15: anahtarlı bloklar --------------------------------------------------------------

def keyed(d, o, comp):
    """{u16 tür, u16 imza, u32 yuvalar*, u32 anahtar zamanları* (0 = her kare), u16 anahtar, u16 kanal, u8 blok biti,
    kanal kayıtları (vec3: 0x24 = min[3], aralık[3], a[3], b[3]; skaler: 0xC = min, aralık, a, b), bloklar}.
    Blok: 2^bit anahtar; ilk anahtar u16 (min + u·aralık/65535), sonrakiler u8 delta ((2a/65535 − 1)·aralık + u·2b·aralık/65535/255)."""
    slots_p, keys_p, nkeys, nch, bits = u32(d, o + 4), u32(d, o + 8), u16(d, o + 0xC), u16(d, o + 0xE), d[o + 0x10]
    rs = 0x24 if comp == 3 else 0xC
    tab = []
    for c in range(nch):
        r = o + 0x14 + rs * c
        mn = struct.unpack_from(f'<{comp}f', d, r)
        rg = struct.unpack_from(f'<{comp}f', d, r + 4 * comp)
        a = struct.unpack_from(f'<{comp}H', d, r + 8 * comp)
        b = struct.unpack_from(f'<{comp}H', d, r + 8 * comp + 2 * comp)
        tab.append([(mn[i], rg[i] / 65535.0, 2 * a[i] * rg[i] / 65535.0 - rg[i], 2 * b[i] * rg[i] / 65535.0 / 255.0)
                    for i in range(comp)])
    blk = (((1 << bits) + 1) * comp * nch + 1) & ~1
    data = o + 0x14 + rs * nch
    vals = []
    for k in range(nkeys):
        b0, w = k >> bits, k & ((1 << bits) - 1)
        p = data + blk * b0
        cur = []
        for c in range(nch):
            us = struct.unpack_from(f'<{comp}H', d, p + 2 * comp * c)
            cur.append([t[0] + t[1] * u for t, u in zip(tab[c], us)])
        q = p + 2 * comp * nch
        for j in range(w):
            for c in range(nch):
                for i in range(comp):
                    cur[c][i] += tab[c][i][2] + tab[c][i][3] * d[q + comp * nch * j + comp * c + i]
        vals.append([x for ch in cur for x in ch])
    times = list(struct.unpack_from(f'<{nkeys}H', d, keys_p)) if keys_p else list(range(nkeys))
    slots = [u16(d, slots_p + 2 * i) for i in range(nch)]
    return dict(kind='anahtarlı vec3' if comp == 3 else 'anahtarlı skaler', frames=nkeys, bits=bits, n=nch,
                slots=slots, times=times, values=vals)


# --- tür 0x11: anahtarlı, paketli dördey (kemik başına) ------------------------------------------------

def quat11(d, o):
    """{u16 0x11, u16 imza, u16 anahtar, u8 kanal, _, u32 anahtar zamanları* (0 = her kare), u32 kemik no* (u8[kanal]),
    u8 blok biti}; +0x12'den kanal başına 6 × u16 {min[3], aralık[3]} (u·2/65535 − 1 / u·2/65535; 0x6ECA40), sonra bloklar
    ((2^b + 1)·kanal·3 + 1) & ~1 bayt (0x6ECD30). Blok başı kanal başına u16×3 ilk anahtar: x = (u0>>1)·2/32767 − 1,
    y/z = u·2/65535 − 1, w işareti u0 & 1 (0x6ECB40); sonraki anahtarlar kanal başına 3 bayt delta: x += min + (b0>>1)·aralık/127,
    y/z += min + b·aralık/255, w işareti b0 & 1 (0x6ED680). w = ±√(1 − x² − y² − z²); toplam > 1 ise normalize, w = 0 (0x6ECAD0).
    Çıkış: poz + kemik·0x30 + 0x10 (dördey yuvası); kareler arası nlerp (0x6ECC80)."""
    nkeys, nch, keys_p, bones_p, bits = u16(d, o + 4), d[o + 6], u32(d, o + 8), u32(d, o + 0xC), d[o + 0x10]
    rec = []
    for c in range(nch):
        u = struct.unpack_from('<6H', d, o + 0x12 + 12 * c)
        rec.append(([x * 2 / 65535.0 - 1 for x in u[:3]], [x * 2 / 65535.0 for x in u[3:]]))
    blk = (((1 << bits) + 1) * nch * 3 + 1) & ~1
    data = o + 0x12 + 12 * nch
    vals = []
    for k in range(nkeys):
        b0, w_in = k >> bits, k & ((1 << bits) - 1)
        p = data + blk * b0
        cur, sign = [], []
        for c in range(nch):
            u0, u1, u2 = struct.unpack_from('<3H', d, p + 6 * c)
            cur.append([(u0 >> 1) * 2 / 32767.0 - 1, u1 * 2 / 65535.0 - 1, u2 * 2 / 65535.0 - 1])
            sign.append(u0 & 1)
        q = p + 6 * nch
        for j in range(w_in):
            for c in range(nch):
                by = d[q + 3 * nch * j + 3 * c:q + 3 * nch * j + 3 * c + 3]
                mn, rg = rec[c]
                cur[c][0] += mn[0] + (by[0] >> 1) * rg[0] / 127.0
                cur[c][1] += mn[1] + by[1] * rg[1] / 255.0
                cur[c][2] += mn[2] + by[2] * rg[2] / 255.0
                sign[c] = by[0] & 1
        fr = []
        for c in range(nch):
            x, y, z = cur[c]
            s = x * x + y * y + z * z
            if s > 1.0:
                n = s ** 0.5
                fr += [x / n, y / n, z / n, 0.0]
            else:
                w = (1.0 - s) ** 0.5
                fr += [x, y, z, -w if sign[c] else w]
        vals.append(fr)
    times = list(struct.unpack_from(f'<{nkeys}H', d, keys_p)) if keys_p else list(range(nkeys))
    bones = [d[bones_p + i] for i in range(nch)] if bones_p else list(range(nch))
    return dict(kind='paketli dördey', frames=nkeys, bits=bits, n=nch, slots=[12 * b_ + 4 for b_ in bones],
                bones=bones, times=times, values=vals)


def child(d, o):
    t = u16(d, o)
    if t == 0x0:
        return raw(d, o)
    if t in (0xA, 0xB):
        return delta(d, o, t == 0xB)
    if t == 0x14:
        return keyed(d, o, 3)
    if t == 0x15:
        return keyed(d, o, 1)
    if t == 0x11:
        return quat11(d, o)
    return dict(kind=f'tür {t:#x} (çözülmedi)', frames=None, values=None)


def composite(d, h):
    """Tür 0xF: {u16 0xF, u16 iskelet imzası, u32 bilgi*, u16 çocuk sayısı, u16 kare, u32 çocuk*[n]}."""
    sig, info, nch, frames = u16(d, h + 2), u32(d, h + 4), u16(d, h + 8), u16(d, h + 0xA)
    kids = [u32(d, h + 0xC + 4 * i) for i in range(nch)]
    return sig, frames, d[info + 8], [(u16(d, k), child(d, k)) for k in kids]


def qangle(q):
    w = max(-1.0, min(1.0, q[3]))
    return math.degrees(2 * math.acos(abs(w)))


def main():
    args = [a for a in sys.argv[1:]]
    mod = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'nfsu2-anim.py')
    if '--anim' in args:
        i = args.index('--anim'); mod = args[i + 1]; del args[i:i + 2]
    frames_sel = None
    if '--frames' in args:
        i = args.index('--frames'); frames_sel = [int(x) for x in args[i + 1].split(',')]; del args[i:i + 2]
    if not args:
        raise SystemExit(__doc__)
    na = load_anim_module(mod)
    path, pat = args[0], (args[1] if len(args) > 1 else '')
    b = na.load(path)
    for dd_, o, c, s in na.walk(b):
        if c != 0xE34010:
            continue
        for a in na.bank(b, o, s):
            if pat not in a['name']:
                continue
            d = a['d']
            sig, frames, fps, kids = composite(d, a['hdr'])
            print(f"{a['name']}: imza {sig:#06x}, {frames} kare @ {fps} fps")
            for t, ch in kids:
                v = ch['values']
                extra = ''
                if ch['kind'] == 'DeltaQ':
                    norms = [math.sqrt(sum(x * x for x in fr[4 * i:4 * i + 4])) for fr in v for i in range(len(ch['slots']))]
                    extra = f", {ch['bits']} bit, {len(ch['slots'])} dördey, |q| {min(norms):.4f}..{max(norms):.4f}"
                elif ch['kind'] == 'paketli dördey':
                    extra = f", blok 2^{ch['bits']}, {ch['n']} kanal, kemikler {ch['bones'][:8]}"
                elif ch['kind'] == 'DeltaF':
                    extra = f", {ch['bits']} bit, {ch['n']} float"
                elif ch['kind'].startswith('anahtarlı'):
                    extra = f", blok 2^{ch['bits']}, {ch['n']} kanal, anahtar zamanları {ch['times'][:6]}{'…' if len(ch['times']) > 6 else ''}"
                elif ch['kind'] == 'ham':
                    extra = f", program {ch['groups']}"
                slots = ch.get('slots')
                print(f"  çocuk tür {t:#04x} {ch['kind']}{extra}" + (f", yuvalar {slots[:8]}{'…' if len(slots) > 8 else ''}" if slots else ''))
                if v and frames_sel is not None:
                    for f in frames_sel:
                        if f < len(v):
                            fr = v[f]
                            if ch['kind'] == 'ham':
                                print(f'    kare {f}: ' + '; '.join(
                                    ', '.join(f"{k} {tuple(round(x, 4) for x in val)}" + (f' ({qangle(val):.1f}°)' if k == 'q' else '')
                                              for k, val in bone.items()) for bone in fr))
                            elif ch['kind'] == 'paketli dördey':
                                qs = [fr[4 * i:4 * i + 4] for i in range(ch['n'])]
                                print(f'    anahtar {f}: ' + ' '.join(f'k{ch["bones"][i]}:{qangle(q):.1f}° ' + str(tuple(round(x, 3) for x in q)) for i, q in enumerate(qs[:6])))
                            elif ch['kind'] == 'DeltaQ':
                                qs = [fr[4 * i:4 * i + 4] for i in range(len(ch['slots']))]
                                print(f'    kare {f}: ' + ' '.join(f'k{ch["slots"][i] // 12}:{qangle(q):.1f}°' for i, q in enumerate(qs[:12])))
                            else:
                                print(f'    anahtar {f}: ' + ' '.join(f'{x:.3f}' for x in fr[:12]))


if __name__ == '__main__':
    main()
