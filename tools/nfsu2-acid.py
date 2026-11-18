#!/usr/bin/env python3
"""NFSU2 Acid parçacık efektleri (`0x80035000`: `0x35021` tanımlar, `0x35020` yayıcılar) — yalnız okur.

Alanlar SPEED2.EXE'den okundu (SPEED2-ATLAS.md §19): yayım `0x491E50`, parçacık güncellemesi `0x492E10`,
çizim `0x48BDF0`, eğri `0x4881D0`.

    nfsu2-acid.py <dosya>                  # tanımların özeti (ad, doku, grup, hız, ömür, …)
    nfsu2-acid.py <dosya> <ad-parçası> -v   # eşleşen tanımların tüm alanları + anahtarlar
    nfsu2-acid.py <dosya> --emitters        # yerleşik/şablon yayıcılar (konum, durum)
    nfsu2-acid.py <dosya> --check           # renk/boy/dönüş kübiklerinin 4 anahtardan geçtiğini sına

<dosya>: GLOBAL/GLOBALB.BUN, TRACKS/L4R?.BUN, TRACKS/STREAML4R?.BUN, FRONTEND/PLATFORMS/Showroom.BIN …
"""
import struct
import sys


def walk(b, o=0, end=None):
    end = len(b) if end is None else end
    while o + 8 <= end:
        cid, sz = struct.unpack_from('<II', b, o)
        yield cid, o, sz
        if cid & 0x80000000 and sz:
            yield from walk(b, o + 8, o + 8 + sz)
        o += 8 + sz


def records(b, cid_want, size):
    out = []
    for cid, o, sz in walk(b):
        if cid == cid_want:
            a = (o + 0x17) & ~0xF          # veri 16'ya hizalı
            n = struct.unpack_from('<I', b, a + 0xC)[0]   # başlık 0x10: {?, ?, sürüm, sayı}
            out += [b[a + 0x10 + size * i:a + 0x10 + size * (i + 1)] for i in range(n)]
    return out


def cstr(r, o, n):
    return r[o:o + n].split(b'\0')[0].decode('latin1', 'replace')


F = lambda r, o: struct.unpack_from('<f', r, o)[0]
I8 = lambda r, o: struct.unpack_from('<b', r, o)[0]

# 0x35021 tanımı (0x220) — ofsetler kaydın başından
FIELDS = [
    (0x18, 'b', 'yön kipi (0 üç eksen rastgele, ≠0 koni)'), (0x19, 'B', 'doku ızgarası log2 sütun'),
    (0x1A, 'B', 'doku ızgarası log2 satır'), (0x1B, 'b', 'kare animasyonu fps (0 yok)'),
    (0x1C, 'B', 'rastgele başlangıç karesi'), (0x1D, 'B', 'rastgele aynalama'),
    (0x1E, 'B', 'sahibin hızını her karede taşı'), (0x1F, 'b', 'en/boy kipi 0..3'),
    (0x20, 'B', 'çizim: 0 kamera billboard (toplu), ≠0 dörtgen; alt 4 bit düzlem'),
    (0x21, 'B', 'parçacıklar kapalıyken de yay'),
    (0x24, 'f', 'kapalı süre s'), (0x28, 'f', 'kapalı süre sapması'), (0x2C, 'f', 'açık süre s'),
    (0x30, 'f', 'açık süre sapması'), (0x34, 'f', 'yayım hızı'), (0x38, 'f', 'yayım hızı sapması'),
    (0x3C, 'f', 'ömür s'), (0x40, 'f', 'ömür sapması'), (0x44, 'f', 'hız m/s'), (0x48, 'f', 'hız sapması'),
    (0x4C, 'f', 'dönüş yayılımı °'), (0x50, 'f', 'yön yayılımı °'), (0x54, 'f', 'sahip hızı payı'),
    (0x58, 'f', 'pay sapması'), (0x5C, 'i', 'araba bağlama noktası no'),
    (0x60, '3f', 'konum ofseti'), (0x70, '3f', 'yayım kutusu'), (0x80, 'f', 'görüş uzaklığı m'),
    (0x84, 'f', 'yerçekimi m/s²'), (0x88, 'f', 'karesel sürüklenme'), (0x8C, 'f', 'en büyük ekran boyu (0 → 512)'),
]


def fmt(r, o, t):
    if t == 'f':
        return f'{F(r, o):g}'
    if t == '3f':
        return '(' + ', '.join(f'{F(r, o + 4 * k):g}' for k in range(3)) + ')'
    if t == 'i':
        return str(struct.unpack_from('<i', r, o)[0])
    return str(struct.unpack_from('<' + t, r, o)[0])


def keys(r):
    T = [F(r, 0x90 + 4 * k) for k in range(4)]
    rgba = [tuple(F(r, 0xA0 + 0x10 * k + 4 * c) for c in range(4)) for k in range(4)]
    size = [F(r, 0xE0 + 4 * k) for k in range(4)]
    rot = [F(r, 0xF0 + 4 * k) for k in range(4)]
    return T, rgba, size, rot


def cubic(r, base, ch, t):
    a3, a2, a1, a0 = (F(r, base + 0x10 * k + 4 * ch) for k in range(4))
    return ((a3 * t + a2) * t + a1) * t + a0


def check(r):
    T, rgba, size, rot = keys(r)
    if T[3] <= 0:
        return None
    e = 0.0
    for k in range(4):
        t = T[k] / T[3]
        e = max(e, *(abs(cubic(r, 0x100, c, t) - rgba[k][c]) for c in range(4)))
        e = max(e, abs(cubic(r, 0x140, 0, t) - size[k]) * 100)
        e = max(e, abs(cubic(r, 0x140, 1, t) / (65536 / 360) - rot[k]))
    return e


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    b = open(sys.argv[1], 'rb').read()
    D = records(b, 0x35021, 0x220)
    if '--emitters' in sys.argv:
        names = {struct.unpack_from('<I', r, 8)[0]: cstr(r, 0x180, 0x40) for r in D}
        for r in records(b, 0x35020, 0xD0):
            h = struct.unpack_from('<I', r, 8)[0]
            st = struct.unpack_from('<I', r, 0x10)[0]
            pos = tuple(round(F(r, 0x70 + 4 * k), 2) for k in range(3))
            print(f'{cstr(r, 0x90, 0x40):34s} tanım {h:#010x} {names.get(h, "?")[:0]}durum {st} konum {pos}')
        return
    if '--check' in sys.argv:
        res = [check(r) for r in D]
        ok = sum(1 for e in res if e is not None and e < 0.05)
        print(f'tanım {len(D)}, 4 anahtardan geçen kübik {ok}, en kötü {max(e for e in res if e is not None):.4f}')
        return
    pat = sys.argv[2] if len(sys.argv) > 2 and not sys.argv[2].startswith('-') else None
    for r in D:
        name = cstr(r, 0x180, 0x40)
        if pat and pat.lower() not in name.lower():
            continue
        print(f'{name:30s} doku {cstr(r, 0x1C0, 0x20):18s} grup {cstr(r, 0x1F0, 0x30):26s} '
              f'yayım {F(r, 0x34):g}/çağrı, ömür {F(r, 0x3C):g} s, hız {F(r, 0x44):g} m/s, g {F(r, 0x84):g}')
        if '-v' in sys.argv:
            for o, t, d in FIELDS:
                print(f'    +{o:#05x} {d:48s} {fmt(r, o, t)}')
            T, rgba, size, rot = keys(r)
            print(f'    anahtar zamanları {T}')
            for k in range(4):
                print(f'      {k}: RGBA {tuple(round(x) for x in rgba[k])}, boy {size[k]:g} m, dönüş {rot[k]:g}°')


if __name__ == '__main__':
    main()
