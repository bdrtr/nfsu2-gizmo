#!/usr/bin/env python3
"""SPEED2.EXE'nin `.rsrc` bölümünü gezer; RT_RCDATA (10) kaynaklarını — 23 derlenmiş D3DX efekti
(`IDI_WORLD_FX` … `IDI_RUNWAYLIGHT_FX`, fx_2_0) — listeler ve istenirse bir dizine çıkarır.
Çıkan dosyalar `fx2.py` ile okunur (SPEED2-ATLAS.md §12). Oyun verisi: çıktıyı repoya koyma.

    tools/fx/rsrc.py <SPEED2.EXE> [ÇIKTI_DİZİNİ]
    tools/fx/fx2.py <ÇIKTI_DİZİNİ>/IDI_CAR_FX
"""
import os
import struct
import sys

b = open(sys.argv[1], 'rb').read()
pe = struct.unpack_from('<I', b, 0x3C)[0]
n = struct.unpack_from('<H', b, pe + 6)[0]
opt = struct.unpack_from('<H', b, pe + 20)[0]
RVA = RO = None
for i in range(n):
    o = pe + 24 + opt + 40 * i
    if b[o:o + 8].rstrip(b'\0') == b'.rsrc':
        _, RVA, _, RO = struct.unpack_from('<IIII', b, o + 8)
assert RO is not None, '.rsrc yok'


def name_at(off):
    ln = struct.unpack_from('<H', b, RO + off)[0]
    return b[RO + off + 2:RO + off + 2 + 2 * ln].decode('utf-16le')


out = []


def walk(off, path):
    nn, ni = struct.unpack_from('<HH', b, RO + off + 12)
    for k in range(nn + ni):
        idv, ov = struct.unpack_from('<II', b, RO + off + 16 + 8 * k)
        nm = name_at(idv & 0x7FFFFFFF) if idv & 0x80000000 else idv
        if ov & 0x80000000:
            walk(ov & 0x7FFFFFFF, path + [nm])
        else:
            drva, sz = struct.unpack_from('<II', b, RO + ov)
            out.append((path + [nm], drva, sz))


walk(0, [])
for p, drva, sz in out:
    print(p, f'rva {drva:#x}', f'dosya {RO + drva - RVA:#x}', sz)
if len(sys.argv) > 2:
    d = sys.argv[2]
    os.makedirs(d, exist_ok=True)
    for p, drva, sz in out:
        if p[0] == 10:
            open(os.path.join(d, str(p[1]).replace('\\', '_')), 'wb').write(b[RO + drva - RVA:RO + drva - RVA + sz])
