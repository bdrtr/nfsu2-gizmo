#!/usr/bin/env python3
"""NFSU2 ses dosyaları — ABK (AEMS bankası, içinde BNKl v5 + EA-XA v2) ve GIN (`Gnsu20` taneli motor, EA-XAS v0).

Biçimler SPEED2-ATLAS.md §11.1'de. Yalnız okur; WAV çıktısı istenirse verilen dizine/dosyaya yazar
(oyun verisi — repoya koyma).

    tools/nfsu2-sound.py abk <dosya.abk> [--wav DİZİN]   # başlık, modüller, sesler (örnek, döngü, hız)
    tools/nfsu2-sound.py gin <dosya.gin> [--wav ÇIKTI]   # devir aralığı, tane tablosu, çevrim kontrolü
"""
import os
import struct
import sys
import wave

XA_COEF = [(0, 0), (240, 0), (460, -208), (392, -220)]


def clamp16(s):
    return max(-32768, min(32767, s))


# --- ABK / BNK -------------------------------------------------------------------------------

def pt_header(b, o):
    """EA `PT` etiketli başlık: `etiket, uzunluk, değer(BE)`; `0xFD` alt başlığa geçer, `0xFF` biter."""
    assert b[o:o + 2] == b'PT', b[o:o + 4]
    o += 4
    d, sub = {}, False
    while True:
        t = b[o]
        o += 1
        if t == 0xFF:
            return d
        if t in (0xFC, 0xFD, 0xFE):
            sub = True
            continue
        n = b[o]
        o += 1
        d[(sub, t)] = int.from_bytes(b[o:o + n], 'big') if n else 0
        o += n


def eaxa_v2(b, o, n):
    """EA-XA v2: 15 baytlık çerçeve = 28 örnek; `0xEE` ile başlayan 61 baytlık çerçeve ham PCM16-BE."""
    out, h1, h2 = [], 0, 0
    while len(out) < n:
        fb = b[o]
        if fb == 0xEE:
            out += struct.unpack_from('>28h', b, o + 5)
            h1, h2 = out[-1], out[-2]
            o += 61
            continue
        c1, c2 = XA_COEF[fb >> 4] if fb >> 4 < 4 else (0, 0)
        sh = 20 - (fb & 0xF)
        for i in range(28):
            by = b[o + 1 + i // 2]
            nib = by >> 4 if i % 2 == 0 else by & 0xF
            nib -= 16 if nib >= 8 else 0
            s = clamp16(((nib << sh) + h1 * c1 + h2 * c2 + 128) >> 8)
            out.append(s)
            h2, h1 = h1, s
        o += 15
    return out[:n]


def abk(path):
    b = open(path, 'rb').read()
    assert b[:4] == b'ABKC', b[:4]
    nmod = struct.unpack_from('<H', b, 0x0A)[0]
    size, code_end, modtab, bnk, bnksz = struct.unpack_from('<5I', b, 0x14)
    # +0x30 çağrı düzeltmeleri (sayı + ofsetler; işlenen = 0x817FE0 ilkel indeksi), +0x34 mutlak düzeltmeler,
    # +0x38 adlı nesneler: sayı + 12 baytlık {nesne ofseti, ad kaydı ofseti, u8 tür} (yükleyici 0x7212E5; ad kaydı =
    # {u16 proje, u16 kimlik, ad}). Modül = başlığı nesne−4'te olan adlı nesne; başlık +0x28 giriş noktası, +0x2C şablon
    # başı (= kod sonu; çok modüllü bankada modüller kodu paylaşıyor), +0x30 örnek/şablon boyu, +0x34 = boy − 0x10
    # (örnek içi üstbilgi). İlk başlık +0x1C'de,
    # +0x0A modül sayısı. Başlık tutarsızsa nesne modül değil (port vb.).
    reloc, absfix, named = struct.unpack_from('<3I', b, 0x30)
    mods, ports = [], []
    if named:
        for k in range(struct.unpack_from('<I', b, named)[0]):
            obj, rec, kind = struct.unpack_from('<IIB', b, named + 4 + 12 * k)
            proj, mid = struct.unpack_from('<HH', b, rec)
            name = b[rec + 4:b.index(b'\0', rec + 4)].decode('latin1')
            c0, c1, inst, ihdr = struct.unpack_from('<4I', b, obj - 4 + 0x28)
            if c0 < c1 <= code_end and inst and ihdr == inst - 0x10:
                mods.append(dict(hdr=obj - 4, name=name, proj=proj, mid=mid, code=(c0, c1), inst=inst))
            else:
                ports.append((obj, (name, proj, mid, kind)))
    sounds = []  # banka yuvası sırasıyla; boş yuva None
    if bnk and b[bnk:bnk + 4] == b'BNKl':
        ver, n = struct.unpack_from('<HH', b, bnk + 4)
        for i, rel in enumerate(struct.unpack_from(f'<{n}I', b, bnk + 0x14)):
            sounds.append(pt_header(b, bnk + 0x14 + 4 * i + rel) if rel else None)
    return dict(b=b, size=size, nmod=nmod, code_end=code_end, bnk=bnk, bnksz=bnksz, mods=mods, ports=ports,
                sounds=sounds)


def cmd_abk(path, wav_dir):
    r = abk(path)
    print(f'{os.path.basename(path)}: boy {len(r["b"])} (başlık {r["size"]}), modül {r["nmod"]}, '
          f'kod sonu {r["code_end"]:#x}, BNK {r["bnk"]:#x}+{r["bnksz"]:#x}, '
          f'yuva {len(r["sounds"])} ({sum(1 for s in r["sounds"] if s)} dolu)')
    for m in r['mods']:
        print(f'  modül {m["name"]!r}: başlık {m["hdr"]:#x}, giriş {m["code"][0]:#x}, şablon {m["code"][1]:#x}+{m["inst"]:#x}, '
              f'proje {m["proj"]:#06x}, kimlik {m["mid"]:#06x}')
    for o, (name, proj, mid, kind) in r['ports']:
        print(f'  adlı nesne {name!r} @ {o:#x} (modül değil), proje {proj:#06x}, kimlik {mid:#06x}')
    for i, p in enumerate(r['sounds']):
        if p is None:
            print(f'  [{i}] boş yuva')
            continue
        ns, data = p.get((True, 0x85), 0), p.get((True, 0x88), 0)
        ls, le = p.get((True, 0x86)), p.get((True, 0x87))
        sr = p.get((True, 0x84), 22050)
        loop = f'döngü {ls}..{le}' if ls is not None else 'döngüsüz'
        print(f'  [{i}] {ns} örnek, {sr} Hz, {loop}, veri {data:#x}')
        if wav_dir:
            x = eaxa_v2(r['b'], r['bnk'] + data, ns)
            write_wav(os.path.join(wav_dir, f'{os.path.splitext(os.path.basename(path))[0]}_{i:02d}.wav'), x, sr)


# --- GIN ---------------------------------------------------------------------------------------

def gin(path):
    b = open(path, 'rb').read()
    assert b[:6] == b'Gnsu20', b[:8]
    lo, hi, nr, ng, ns, sr = struct.unpack_from('<ff4I', b, 8)
    o = 0x20
    # iki tablo da n+1 girişli (ayrıştırıcı 0x466770); veri hemen arkasından, öneksiz
    rpm_tab = struct.unpack_from(f'<{nr + 1}I', b, o)
    o += 4 * (nr + 1)
    grains = struct.unpack_from(f'<{ng + 1}I', b, o)  # [0] = 0, [ng] = ns − 1
    o += 4 * (ng + 1)
    return dict(b=b, lo=lo, hi=hi, nr=nr, ng=ng, ns=ns, sr=sr, rpm_tab=rpm_tab, grains=grains, data=o)


def eaxs_v0(b, o, n):
    """EA-XAS v0: 0x13 baytlık çerçeve = 2 başlık örneği (alt dört bitte katsayı/kaydırma) + 30 nibble = 32 örnek."""
    out = []
    while len(out) < n and o + 0x13 <= len(b):
        fr = b[o:o + 0x13]
        o += 0x13
        h2 = struct.unpack_from('<h', fr, 0)[0] & ~0xF
        h1 = struct.unpack_from('<h', fr, 2)[0] & ~0xF
        c1, c2 = XA_COEF[fr[0] & 0xF] if fr[0] & 0xF < 4 else (0, 0)
        sh = 20 - (fr[2] & 0xF)
        out += [h2, h1]
        for i in range(30):
            by = fr[4 + i // 2]
            nib = by >> 4 if i % 2 == 0 else by & 0xF
            nib -= 16 if nib >= 8 else 0
            s = clamp16(((nib << sh) + h1 * c1 + h2 * c2 + 128) >> 8)
            out.append(s)
            h2, h1 = h1, s
    return out[:n]


def cmd_gin(path, wav_out):
    g = gin(path)
    expect = -(-g['ns'] // 32) * 0x13
    have = len(g['b']) - g['data']
    print(f'{os.path.basename(path)}: devir {g["lo"]:.1f}..{g["hi"]:.1f}, devir tablosu {g["nr"]}, tane {g["ng"]}, '
          f'{g["ns"]} örnek @ {g["sr"]} Hz; veri {have} B (beklenen {expect})')
    gr = g['grains']
    if len(gr) > 2:
        sizes = [gr[k + 1] - gr[k] for k in range(len(gr) - 1)]
        print(f'  ilk tane {sizes[0]} örnek = {sizes[0] / (120.0 / g["lo"] * g["sr"]):.3f} × (720° @ en düşük devir), '
              f'en kısa {min(sizes)} = {min(sizes) / (120.0 / g["hi"] * g["sr"]):.3f} × (720° @ en yüksek devir)')
    print(f'  devir → konum {g["rpm_tab"][:3]} … {g["rpm_tab"][-2:]}, taneler {gr[:3]} … {gr[-1:]}')
    if wav_out:
        write_wav(wav_out, eaxs_v0(g['b'], g['data'], g['ns']), g['sr'])


def write_wav(path, x, sr):
    with wave.open(path, 'wb') as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(struct.pack(f'<{len(x)}h', *x))
    print(f'  → {path}')


def main():
    if len(sys.argv) < 3 or sys.argv[1] not in ('abk', 'gin'):
        raise SystemExit(__doc__)
    out = sys.argv[sys.argv.index('--wav') + 1] if '--wav' in sys.argv else None
    (cmd_abk if sys.argv[1] == 'abk' else cmd_gin)(sys.argv[2], out)


if __name__ == '__main__':
    main()
