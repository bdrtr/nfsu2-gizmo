#!/usr/bin/env python3
"""NFSU2 animasyon, NIS ve kamera verisi — yalnız okur (SPEED2-ATLAS.md §17).

    nfsu2-anim.py banks <dosya>            # EAGL bankaları (0xE34010) ve iskeletler (0xE34009): adlar, kare, süre
    nfsu2-anim.py cameras <dosya> [-v]     # kamera izleri (0x8003B200 rota / 0x8003B201 genel / 0x8003B202 sahne)
    nfsu2-anim.py scenes <InGameCommon.bun> # NIS sahne dizini (0x37060)
    nfsu2-anim.py scene <Scene_*_BundleB.bun>  # sahne başlığı (0x37030) ve nesneler (0x37040)
    nfsu2-anim.py evt <Scene_*_EvtTrk.txt> # olay izi (zaman 30 fps kare)
    nfsu2-anim.py parts <CARS/x/PARTS_ANIMATIONS.bin>  # parça animasyonları + menteşe matrisleri (0x37100)

<dosya>: herhangi bir .BUN/.bun/.bin; JDLZ'li .lzc açılır.
"""
import math
import struct
import sys

FPS = 30.0  # EAGL anim bloklarında 0x1E; olay izi 0x786460 = 1/30


# --- kap ---------------------------------------------------------------------------------------

def jdlz(buf):
    n = struct.unpack_from('<I', buf, 8)[0]
    out = bytearray()
    i, f1, f2 = 16, 1, 1
    while i < len(buf) and len(out) < n:
        if f1 == 1:
            f1 = buf[i] | 0x100
            i += 1
        if f2 == 1:
            f2 = buf[i] | 0x100
            i += 1
        if f1 & 1:
            b0, b1 = buf[i], buf[i + 1]
            i += 2
            if f2 & 1:
                ln, dist = (((b0 & 0xF0) << 4) | b1) + 3, (b0 & 0x0F) + 1
            else:
                ln, dist = (b0 & 0x1F) + 3, (b1 | ((b0 & 0xE0) << 3)) + 17
            st = len(out) - dist
            for k in range(min(ln, n - len(out))):
                out.append(out[st + k])
            f2 >>= 1
        else:
            out.append(buf[i])
            i += 1
        f1 >>= 1
    return bytes(out)


def load(path):
    b = open(path, 'rb').read()
    return jdlz(b) if b[:4] == b'JDLZ' else b


def walk(b, o=0, end=None, d=0, out=None):
    out = [] if out is None else out
    end = len(b) if end is None else end
    while o + 8 <= end:
        cid, sz = struct.unpack_from('<II', b, o)
        if o + 8 + sz > end:
            break
        out.append((d, o, cid, sz))
        if cid & 0x80000000 and sz:
            walk(b, o + 8, o + 8 + sz, d + 1, out)
        o += 8 + sz
    return out


def aligned(o):
    """Chunk verisi 16'ya hizalı başlar (8 baytlık 0x11 dolgusu)."""
    return (o + 0x17) & ~0xF


# --- EAGL ELF ------------------------------------------------------------------------------------

class Elf:
    """0xE34010/0xE34009 yükü: ELF32 REL (e_machine 8), .data + .symtab + .rel.data; EAGL_TOOLLIB_VERSION-4."""

    def __init__(self, b):
        assert b[:4] == b'\x7fELF', b[:4]
        self.b = b
        shoff, = struct.unpack_from('<I', b, 0x20)
        shnum, shstrndx = struct.unpack_from('<HH', b, 0x30)
        self.sh = [dict(zip(('name', 'type', 'flags', 'addr', 'off', 'size', 'link', 'info', 'align', 'entsize'),
                            struct.unpack_from('<10I', b, shoff + 40 * i))) for i in range(shnum)]
        st = self.sh[shstrndx]
        for s in self.sh:
            s['nm'] = self.cstr(st['off'] + s['name'])
        self.sec = {s['nm']: s for s in self.sh}
        self.syms = []
        sy = self.sec.get('.symtab')
        if sy:
            strt = self.sh[sy['link']]
            for k in range(sy['size'] // 16):
                n, v, sz, info, other, shndx = struct.unpack_from('<IIIBBH', b, sy['off'] + 16 * k)
                self.syms.append((self.cstr(strt['off'] + n), v))

    def cstr(self, o):
        return self.b[o:self.b.index(b'\0', o)].decode('latin1')

    def data(self):
        d = self.sec['.data']
        return self.b[d['off']:d['off'] + d['size']]

    def sym(self, name):
        return next((v for n, v in self.syms if n == name), None)


def bank(b, o, s):
    """__AnimationBank:::bank: +4 sayı, +0x10 anim göstericileri, +0x14 ad göstericileri. Anim başlığı:
    {u8 0x0F, u8, u16 tür (0x1414/0x1400/0xEBD7/0xC6C5…), u32 veri, u16, u16 kare}; veri bloğu +8'de fps baytı."""
    e = Elf(b[aligned(o):o + 8 + s])
    d = e.data()
    bk = e.sym('__AnimationBank:::bank')
    n, = struct.unpack_from('<I', d, bk + 4)
    anims, tab = struct.unpack_from('<II', d, bk + 0x10)
    out = []
    for k in range(n):
        ao, = struct.unpack_from('<I', d, anims + 4 * k)
        nm = e.cstr(e.sec['.data']['off'] + struct.unpack_from('<I', d, tab + 4 * k)[0])
        kind, = struct.unpack_from('<H', d, ao + 2)
        ptr, = struct.unpack_from('<I', d, ao + 4)
        nf, = struct.unpack_from('<H', d, ao + 10)
        fps = d[ptr + 8] if ptr + 8 < len(d) else None
        out.append(dict(name=nm, kind=kind, frames=nf, fps=fps, data=ptr, hdr=ao, d=d))
    return out


def cmd_banks(path):
    b = load(path)
    total = 0
    for d, o, c, s in walk(b):
        if c == 0xE34010:
            an = bank(b, o, s)
            total += len(an)
            print(f'banka @{o:#x} ({s:#x} B): {len(an)} anim')
            for a in an:
                print(f'   {a["name"]:58s} tür {a["kind"]:#06x} {a["frames"]:4d} kare '
                      f'({a["frames"] / (a["fps"] or FPS):.2f} s @ {a["fps"]})')
        elif c == 0xE34009:
            e = Elf(b[aligned(o):o + 8 + s])
            bones = sum(1 for n, _ in e.syms if n.startswith('__Bone'))
            sk = [n for n, _ in e.syms if n.startswith('__Skeleton')]
            print(f'iskelet @{o:#x}: {sk} {bones} kemik')
    print(f'toplam {total} anim')


# --- kamera izleri ---------------------------------------------------------------------------------

CAM_SET = {0x8003B200: 'rota', 0x8003B201: 'genel', 0x8003B202: 'sahne'}
SPACE = ['Car', 'World', 'Hybrid', 'Scene']  # 0x7EB514


def cam_key(b, o):
    """0x88'lik anahtar (alan adları .moo yazıcısından, 0x442340)."""
    u8 = lambda k: b[o + k]
    f = lambda k: round(struct.unpack_from('<f', b, o + k)[0], 4)
    v = lambda k: tuple(round(x, 3) for x in struct.unpack_from('<3f', b, o + k))
    return dict(type=u8(0), smooth=u8(1), eye_interp=('linear', 'cubic')[u8(2)], look_interp=('linear', 'cubic')[u8(3)],
                eye_space=SPACE[u8(4)] if u8(4) < 4 else u8(4), look_space=SPACE[u8(5)] if u8(5) < 4 else u8(5),
                to_cars=u8(6), to_world=u8(7), ignore_orient=u8(8), car_lag=u8(9), param=f(0xC),
                tangent=(f(0x10), f(0x14)), eye=(v(0x18), v(0x24)), look=(v(0x30), v(0x3C)), lens=(f(0x48), f(0x4C)),
                near=(f(0x50), f(0x54)), noise_mag=(f(0x58), f(0x5C)), noise_freq=(f(0x60), f(0x64)),
                focal=(f(0x68), f(0x6C)), aperture=(u8(0x70), u8(0x71)), letterbox=(u8(0x72), u8(0x73)),
                fx_intensity=(u8(0x74), u8(0x75)), tint=(tuple(b[o + 0x76:o + 0x79]), tuple(b[o + 0x79:o + 0x7C])),
                gain=(tuple(b[o + 0x7C:o + 0x7E]), tuple(b[o + 0x7E:o + 0x80])), bloom=(u8(0x80), u8(0x81)),
                audio=struct.unpack_from('<I', b, o + 0x84)[0])


def cam_sets(b):
    for d, o, c, s in walk(b):
        if c not in CAM_SET:
            continue
        p = o + 8
        while p + 8 <= o + 8 + s:
            cid, sz = struct.unpack_from('<II', b, p)
            if cid == 0x3B210:
                grp, n = struct.unpack_from('<II', b, p + 8)
                q = p + 0x10
                tracks = []
                for _ in range(n):
                    length, = struct.unpack_from('<f', b, q + 0xC)
                    nk, = struct.unpack_from('<b', b, q + 0x10)
                    name = b[q + 0x12:q + 0x20].split(b'\0')[0].decode('latin1')
                    tracks.append((name, length, [cam_key(b, q + 0x20 + 0x88 * k) for k in range(nk)]))
                    q += 0x20 + 0x88 * nk
                yield CAM_SET[c], grp, tracks
            p += 8 + sz


def fov(lens):
    """0x443D80: yarım açı = atan2(1,33·12, lens) (0x7870A8, 0x7870AC)."""
    return 2 * math.degrees(math.atan2(1.33 * 12.0, lens))


def cmd_cameras(path, verbose):
    b = load(path)
    for kind, grp, tracks in cam_sets(b):
        print(f'{kind} grup {grp:#010x}: {len(tracks)} iz, anahtar {[len(t[2]) for t in tracks]}')
        if verbose:
            for name, length, keys in tracks:
                print(f'   iz {name!r} uzunluk {length}')
                for k in keys:
                    print(f'      t={k["param"]} göz {k["eye"]} bakış {k["look"]} lens {k["lens"]} '
                          f'(≈{fov(k["lens"][0]):.0f}°) uzay {k["eye_space"]}/{k["look_space"]} tür {k["type"]}')


# --- NIS -------------------------------------------------------------------------------------------

def cmd_scenes(path):
    b = load(path)
    o, s = next((o, s) for d, o, c, s in walk(b) if c == 0x37060)
    x = b[o + 8:o + 8 + s]
    n, = struct.unpack_from('<I', x, 0)
    for i in range(n):
        h, v = struct.unpack_from('<II', x, 4 + 8 * i)
        name = x[0x808 + 0x40 * (v >> 24):0x808 + 0x40 * ((v >> 24) + 1)].split(b'\0')[0].decode('latin1')
        print(f'{i:2d} {h:#010x} {name}')


def cmd_scene(path):
    b = load(path)
    for d, o, c, s in walk(b):
        if c == 0x37030:
            x = b[o + 8:o + 8 + s]
            h, = struct.unpack_from('<I', x, 0)
            name = x[4:0x1C].split(b'\0')[0].decode('latin1')
            crowd, one, cars, total, go, count = struct.unpack_from('<6I', x, 0x1C)
            desc = x[0x34:].split(b'\0')[0].decode('latin1')
            print(f'sahne {name} ({h:#010x}): araba {cars}, kalabalık {crowd}, {total} kare ({total / FPS:.1f} s), '
                  f'GO karesi {go}, geri sayım başı {count} — "{desc}"')
        elif c == 0x37040:
            p = aligned(o)
            kind, h, _, flags = struct.unpack_from('<IIII', b, p)
            m = struct.unpack_from('<16f', b, p + 0x10)
            h2, h3 = struct.unpack_from('<II', b, p + 0x6C)
            print(f'   nesne tür {kind} hash {h:#010x} bayrak {flags:#x} konum ({m[12]:.2f}, {m[13]:.2f}, {m[14]:.2f}) '
                  f'+0x6C {h2:#010x} +0x70 {h3:#010x}')
    for d, o, c, s in walk(b):
        if c == 0xE34010:
            for a in bank(b, o, s):
                if a['name'].startswith('ZPM_Car'):
                    print(f'   {a["name"]}: {a["frames"]} kare')


EVT = ['', 'CAR_TRIGGER_EFFECT', 'CAR_HYDRAULIC_BOUNCE', 'CAR_HYDRAULIC_EXTEND', 'CAR_BRAKE_LOCK',
       'CAR_BURNOUT_ADDITIVE', 'CAR_ENGINE_TORQUE', 'SEQ_CAR_NITRO', 'SEQ_CAR_BURNOUT', 'SEQ_CAR_TORQUE',
       'SCREEN_FLASH', 'WORLD_ANIM_EVENT', 'WORLD_MESSAGE', 'CAR_AUDIO_PATTERN', 'CAR_AUDIO_REV']


def cmd_evt(path):
    for line in open(path, encoding='latin1'):
        t = line.split()
        if t and t[0] == 'EVENT:':
            frame = float(t[1].rstrip('f'))
            kind = EVT.index(t[2]) if t[2] in EVT else '?'
            print(f'{frame:7.1f} kare = {frame / FPS:6.2f} s  [{kind}] {t[2]} {" ".join(t[3:])}')


def cmd_parts(path):
    b = load(path)
    for d, o, c, s in walk(b):
        if c == 0xE34010:
            for a in bank(b, o, s):
                if a['name'].endswith('_q') and a['frames']:
                    d_ = a['d']
                    # ham kanal (sınıf 0, tek kemik, op 0): kareler bilgi bloğunun +0x2C'sinden, dördey x,y,z,w (§17.9.2)
                    qs = [struct.unpack_from('<4f', d_, a['data'] + 0x2C + 16 * k) for k in range(a['frames'])]
                    last = qs[-1]
                    norm = math.sqrt(sum(x * x for x in last))
                    angle = lambda q: 2 * math.degrees(math.acos(max(-1.0, min(1.0, abs(q[3]) / (math.sqrt(sum(x * x for x in q)) or 1)))))
                    ang, peak = angle(last), max(angle(q) for q in qs)
                    tag = f' (tepe {peak:.1f}°)' if peak > ang + 0.5 else ''
                    tag += '' if abs(norm - 1) < 0.01 else ' (birim değil: sınıf 0 değil — tools/nfsu2-eagl.py)'
                    print(f'{a["name"]:40s} {a["frames"]:3d} kare ({a["frames"] / FPS:.2f} s) son açı {ang:5.1f}°{tag}')
        elif c == 0x37100:
            p = aligned(o)
            n, h = struct.unpack_from('<II', b, p)
            k = (o + 8 + s - (p + 0x10)) // 0x40
            print(f'menteşe tablosu: araba hash {h:#010x}, +0 {n}, {k} matris (exe ad sırası 0x7EA568):')
            for i in range(k):
                m = struct.unpack_from('<16f', b, p + 0x10 + 0x40 * i)
                print(f'   [{i:2d}] konum ({m[12]:.3f}, {m[13]:.3f}, {m[14]:.3f})')


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    cmd, path = sys.argv[1], sys.argv[2]
    {'banks': lambda: cmd_banks(path), 'cameras': lambda: cmd_cameras(path, '-v' in sys.argv),
     'scenes': lambda: cmd_scenes(path), 'scene': lambda: cmd_scene(path), 'evt': lambda: cmd_evt(path),
     'parts': lambda: cmd_parts(path)}.get(cmd, lambda: (_ for _ in ()).throw(SystemExit(__doc__)))()


if __name__ == '__main__':
    main()
