#!/usr/bin/env python3
"""NFSU2 kayıt dosyası (`20CM` kabı) — başlığı ve üç CRC'yi doğrular, profil bölgelerini haritalar.

Biçim `SPEED2.EXE`'den okundu (SPEED2-ATLAS.md §10.9): EA memcard kitaplığı `0x6FC000..0x6FE200`,
CRC `0x6FE173` (tablo `0x8145E0`), profil serileştiricisi `0x52A280`.

    tools/nfsu2-save.py <kayıt>            # başlık, CRC'ler, profilse kariyer özeti
    tools/nfsu2-save.py <kayıt> --map      # profil verisinin bölge haritası
    tools/nfsu2-save.py <kayıt> --fix OUT  # CRC'leri yeniden hesaplayıp OUT'a yazar (girdi değişmez)

Kayıtlar wine önekinde: `drive_c/users/<kullanıcı>/AppData/Local/NFS Underground 2/<profil>/<profil>`.
"""
import struct
import sys

MAGIC = b'20CM'
HDR = 0x1C


def _table():
    t = []
    for i in range(256):
        r = i << 24
        for _ in range(8):
            r = ((r << 1) ^ 0x04C11DB7 if r & 0x80000000 else r << 1) & 0xFFFFFFFF
        t.append(r)
    return t


T = _table()


def crc(data):
    """`0x6FE173`: ilk dört bayt büyük-endian okunup tersleniyor, kalanı tabloyla içeri kaydırılıyor
    (artırılmış/"augmented" MSB-first CRC-32), sonuç tersleniyor. Dört bayttan kısa girdi → 0."""
    if len(data) < 4:
        return 0
    r = ~struct.unpack_from('>I', data)[0] & 0xFFFFFFFF
    for b in data[4:]:
        r = (((r << 8) | b) & 0xFFFFFFFF) ^ T[r >> 24]
    return ~r & 0xFFFFFFFF


def header(d):
    if d[:4] != MAGIC:
        raise SystemExit(f'sihir {d[:4]!r}, beklenen {MAGIC!r}')
    size, n1, n2, c1, c2, ch = struct.unpack_from('<6I', d, 4)
    return dict(size=size, n1=n1, n2=n2, crc1=c1, crc2=c2, crch=ch)


def blocks(d, h):
    b1 = d[HDR:HDR + h['n1']]
    b2 = d[HDR + h['n1']:HDR + h['n1'] + h['n2']]
    return b1, b2


def check(d):
    h = header(d)
    b1, b2 = blocks(d, h)
    got = dict(crc1=crc(b1), crc2=crc(b2), crch=crc(d[:0x18]))
    return h, got


# Profil verisi (blok 2, 0xD692 bayt) — `0x52A280` sırası. Dosya ofseti = blok 2 başı (0x24) + birikim.
PROFILE_REGIONS = [
    ('A1 rekor/en iyi süre 48×0x18 (0x83AD90)', 0x480), ('A2 u32 +0x540', 4),
    ('A3 20 araba ×0x430 (+0x548)', 0x53C0), ('A4 5 kariyer arabası ×0x7F2', 5 * 0x7F2),
    ('A5 u32 +0x812C', 4), ('A6 6 ×0x434', 0x1938), ('A7 u32 +0x9A80', 4),
    ('B1 8 aşama yuvası ×0x101 (0x85AD40+0x72D8)', 0x808),
    ('B2 nakit i32 +0x7134', 4), ('B3 u8 +0x8868', 1), ('B4 başlatılan yarış u32 +0x872C', 4),
    ('B5 u8 +0x873E', 1), ('B6 u8 +0x873F', 1), ('B7 geçerli aşama u32 +0x7AF8', 4),
    ('B8 olay durumu sayısı u32 +0x7AFC', 4), ('B9 olay durumu 384×{hash,u8[4]}', 0xC00),
    ('B10 geçerli araba, son yarış', 0x1D), ('B11 nakit noktası bayrakları +0x8768', 0x100),
    ('B12 SMS kutusu 256×0x14 +9 (+0x5D20)', 0x1409), ('B13 4 u32 +0x8869', 0x10),
    ('B14 bildirim kuyruğu +0x887C (0x8635BC)', 0x190), ('B15 güncel olay hash', 4),
    ('C1 +0x2D8 ses/oyun seçenekleri', 0xD8), ('C2 +0x48 ses düzeyleri', 0x3C), ('C3 +0x38 hızlı yarış/birim', 0x10),
    ('C4 +0x84 denetleyici', 0x50), ('C5 +0x28 çevrimiçi/HUD', 0x10),
    ('C6 +0xD4 hızlı yarış yapılandırması', 0x17C), ('C7 +0x250', 4), ('C8 +0x254', 4), ('C9 +0xA0F9', 1), ('C10 +0xA0F8', 1),
    ('C11 +0xA0F4', 4), ('C12 +0xA0FC pist başına hızlı yarış', 0xA50), ('C13 +0x258', 0x80), ('C14 +0x00 profil adı', 0x20),
    ('D oyun ayarları 0x850080 (0x515BF0)', 0x33), ('E giriş aygıtları 6×{GUID, 9 u32, u8, 0x80 B} (0x5B9420)', 0x43E),
]
PROFILE_LEN = 0xD692


def regions():
    o = HDR + 8
    out = {}
    for name, n in PROFILE_REGIONS:
        out[name.split()[0]] = (o, n, name)
        o += n
    assert o - HDR - 8 == PROFILE_LEN, hex(o)
    return out


def career(d):
    r = regions()
    u32 = lambda key: struct.unpack_from('<I', d, r[key][0])[0]
    name = d[r['C14'][0]:r['C14'][0] + 0x20].split(b'\0')[0].decode('latin1')
    print(f'profil {name!r}, nakit ${struct.unpack_from("<i", d, r["B2"][0])[0]}, geçerli aşama {u32("B7")} '
          f'(oyuncunun "Stage 1"i = 2), başlatılan yarış {u32("B4")}, olay durumu {u32("B8")} giriş, '
          f'alınan nakit noktası {sum(1 for b in d[r["B11"][0]:r["B11"][0] + 0x100] if b)}')
    o = r['B1'][0]
    for i in range(8):
        s = d[o + i * 0x101:o + (i + 1) * 0x101]
        outrun, rep, races = struct.unpack_from('<ifi', s, 0xDC)
        sponsor = struct.unpack_from('<I', s, 0xE8)[0]
        done = s[0xFC]  # aşama tamamlandı (0x532C60)
        if outrun or rep or races or sponsor:
            print(f'  yuva {i}: outrun {outrun}, itibar {rep:g}, itibarlı yarış {races}, '
                  f'sponsor hash {sponsor:#010x}' + (', tamamlandı' if done else ''))


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    d = open(sys.argv[1], 'rb').read()
    h, got = check(d)
    print(f'boy {len(d)} (başlık {h["size"]}), blok1 {h["n1"]:#x}, blok2 {h["n2"]:#x}')
    for k in ('crc1', 'crc2', 'crch'):
        print(f'  {k}: dosya {h[k]:#010x}, hesap {got[k]:#010x} {"TAMAM" if h[k] == got[k] else "BOZUK"}')
    b1, _ = blocks(d, h)
    if len(b1) >= 8:
        ver, n2 = struct.unpack_from('<II', b1)
        print(f'  sürüm {ver:#x}' + ('' if ver == 0x10D else ' (exe 0x10D yazar)'))
    if h['n2'] == PROFILE_LEN:
        career(d)
        if '--map' in sys.argv:
            for o, n, name in regions().values():
                nz = sum(1 for b in d[o:o + n] if b)
                print(f'{o:#07x} {n:#7x} sıfırdan farklı {nz:5d}  {name}')
    elif h['n2'] == 0x100000:
        print('  dergi/DVD kapağı: blok 2 = 1 MiB (512×512 BGRA olduğu çıkarım)')
    if '--fix' in sys.argv:
        out = sys.argv[sys.argv.index('--fix') + 1]
        b = bytearray(d)
        b1, b2 = blocks(b, h)
        struct.pack_into('<II', b, 0x10, crc(bytes(b1)), crc(bytes(b2)))
        struct.pack_into('<I', b, 0x18, crc(bytes(b[:0x18])))
        open(out, 'wb').write(b)
        print(f'CRC\'ler yazıldı: {out}')


if __name__ == '__main__':
    main()
