#!/usr/bin/env python3
"""NFSU2 FEng ön yüz paketleri (`*.fng`) — listeler ve döker. Yalnız okur.

Biçim SPEED2.EXE'den okundu (SPEED2-ATLAS.md §14): paketler `0x30203` (açık) ve `0x30210`
(`u32 ad hash'i` + JDLZ/HUFF) chunk'larında; açılmış paket `FEn\\xE7` kökü altında 4 harfli etiketli
bloklar, yapraklarda 2 harf + `u16` boylu alanlar.

    fng.py <dosya>                       # dosyadaki paketler: ad, hash, sıkıştırma, boy
    fng.py <dosya> <paket.fng>           # paketin dökümü (kaynaklar, nesne ağacı, betikler, mesajlar)
    fng.py <dosya> <paket.fng> --names <dosya>...   # hash'leri bu dosyalardaki dizgelerle çöz
                                                    # (ör. SPEED2.EXE, LANGUAGES/English.bin)
                                                    # tools/feng-names.txt: çözülmüş mesaj adları (§14.9.6)

<dosya>: FRONTEND/FrontB.lzc, GLOBAL/GLOBALB.BUN, GLOBAL/InGame*.bun … (JDLZ'li .lzc de olur).
"""
import re
import struct
import sys

# --- sıkıştırma ---------------------------------------------------------------------------------


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


class _Bits:
    def __init__(self, d):
        self.d, self.pos, self.bits, self.left, self.bu = d, 0, 0, -16, 0
        self.get(0)

    def _get16(self):
        hi = self.d[self.pos] if self.pos < len(self.d) else 0
        lo = self.d[self.pos + 1] if self.pos + 1 < len(self.d) else 0
        self.bu = ((self.bu << 16) | (hi << 8) | lo) & 0xFFFFFFFF
        self.pos += 2

    def _refill(self):
        if self.left < 0:
            self._get16()
            self.bits = (self.bu << (-self.left)) & 0xFFFFFFFF
            self.left += 16

    def get(self, n):
        v = 0
        if n:
            v = self.bits >> (32 - n)
            self.bits = (self.bits << n) & 0xFFFFFFFF
            self.left -= n
        self._refill()
        return v

    def consume(self, n):
        self.bits = (self.bits << n) & 0xFFFFFFFF
        self.left -= n
        self._refill()


def _getnum(b):
    if b.bits & 0x80000000:
        return b.get(3) - 4
    n = 2
    if b.bits >> 16:
        while True:
            b.bits = (b.bits << 1) & 0xFFFFFFFF
            n += 1
            if b.bits & 0x80000000:
                break
        b.bits = (b.bits << 1) & 0xFFFFFFFF
        b.left -= n - 1
        b.get(0)
    else:
        b.get(1)
        while True:
            n += 1
            if b.get(1):
                break
    bias = (1 << n) - 4
    raw = ((b.get(n - 16) << 16) | b.get(16)) if n > 16 else b.get(n)
    return raw + bias


def huff(buf):
    """EA HUFF (PryHUB `compression::huff`'ın Python eşi)."""
    out_len = struct.unpack_from('<I', buf, 8)[0]
    b = _Bits(buf[16:])
    typ = b.get(16)
    if typ & 0x8000:
        if typ & 0x100:
            b.get(16), b.get(16)
        ulen = (b.get(16) << 16) | b.get(16)
    else:
        if typ & 0x100:
            b.get(8), b.get(16)
        ulen = (b.get(8) << 16) | b.get(16)
    typ &= ~0x100
    clue = b.get(8)
    delta, cmp_tbl = [0] * 33, [0] * 34
    numchars, numbits, basecmp = 0, 1, 0
    while True:
        basecmp <<= 1
        delta[numbits] = (basecmp - numchars) & 0xFFFFFFFF
        bn = _getnum(b)
        numchars += bn
        basecmp += bn
        cmp = ((basecmp << (16 - min(numbits, 16))) & 0xFFFF) if bn else 0
        cmp_tbl[numbits] = cmp
        numbits += 1
        if bn == 0 or cmp != 0:
            continue
        break
    mostbits = numbits - 1
    cmp_tbl[mostbits] = 0xFFFFFFFF
    codetbl, used, nextchar = [0] * 256, [False] * 256, 0xFF
    for k in range(numchars):
        leap = _getnum(b) + 1
        while True:
            nextchar = (nextchar + 1) & 0xFF
            if not used[nextchar]:
                leap -= 1
            if leap == 0:
                break
        used[nextchar] = True
        codetbl[k] = nextchar
    out = bytearray()
    ulen = min(ulen, out_len) if out_len else ulen
    while len(out) < ulen:
        cmp16 = b.bits >> 16
        ln = 1
        while ln < mostbits and cmp16 >= cmp_tbl[ln]:
            ln += 1
        code = codetbl[((b.bits >> (32 - ln)) - delta[ln]) & 0xFFFFFFFF]
        b.consume(ln)
        if code != clue:
            out.append(code)
            continue
        run = _getnum(b)
        if run:
            prev = out[-1]
            out.extend([prev] * min(run, ulen - len(out)))
        elif b.get(1):
            break
        else:
            out.append(b.get(8))
    if typ in (0x32FB, 0xB2FB):
        acc = 0
        for i, x in enumerate(out):
            acc = (acc + x) & 0xFF
            out[i] = acc
    elif typ in (0x34FB, 0xB4FB):
        acc = acc2 = 0
        for i, x in enumerate(out):
            acc = (acc + x) & 0xFFFFFFFF
            acc2 = (acc2 + acc) & 0xFFFFFFFF
            out[i] = acc2 & 0xFF
    return bytes(out[:ulen])


def unpack(raw):
    if raw[:4] == b'JDLZ':
        return jdlz(raw), 'JDLZ'
    if raw[:4] == b'HUFF':
        return huff(raw), 'HUFF'
    return raw, '-'


# --- chunk'lar ve paketler ------------------------------------------------------------------------


def chunks(b, o=0, end=None):
    end = len(b) if end is None else end
    while o + 8 <= end:
        cid, sz = struct.unpack_from('<II', b, o)
        yield o, cid, sz
        if cid & 0x80000000 and o + 8 + sz <= end:
            yield from chunks(b, o + 8, o + 8 + sz)
        o += 8 + sz


def packages(path):
    """(ad hash'i, sıkıştırma, paket baytları) — `0x30210` ve `0x30203` chunk'larından."""
    b = open(path, 'rb').read()
    if b[:4] == b'JDLZ':
        b = jdlz(b)
    for o, cid, sz in chunks(b):
        if cid == 0x30210:
            h = struct.unpack_from('<I', b, o + 8)[0]
            d, how = unpack(b[o + 12:o + 8 + sz])
            yield h, how, d
        elif cid == 0x30203:
            d = b[o + 8:o + 8 + sz]
            yield fhash(pkg_name(d)), 'açık', d


def fhash(s):
    """`0x505450`: büyük harfe çevirerek h = h·33 + c, başlangıç 0xFFFFFFFF."""
    h = 0xFFFFFFFF
    for c in (s.encode('latin1') if isinstance(s, str) else s):
        if 0x61 <= c <= 0x7A:
            c -= 0x20
        h = (h * 33 + c) & 0xFFFFFFFF
    return h


def pkg_name(d):
    return d[0x28:d.index(b'\0', 0x28)].decode('latin1') if d[:4] == b'FEn\xe7' else '?'


def blocks(d, o, end):
    """4 harfli etiketler: (etiket, yük ofseti, boy)."""
    while o + 8 <= end:
        t = d[o:o + 4].decode('latin1')
        sz = struct.unpack_from('<I', d, o + 4)[0]
        yield t, o + 8, sz
        o += 8 + sz


def fields(d, o, sz):
    """Yapraklardaki 2 harf + u16 boylu alanlar."""
    e = o + sz
    while o + 4 <= e:
        t = d[o:o + 2].decode('latin1')
        n = struct.unpack_from('<H', d, o + 2)[0]
        yield t, d[o + 4:o + 4 + n]
        o += 4 + n


TYPES = {1: 'resim', 2: 'metin', 3: 'model?', 4: 'liste', 5: 'grup', 6: 'kod listesi', 7: 'film',
         8: 'efekt?', 9: 'renkli resim', 10: 'animasyonlu resim?', 11: 'basit resim?'}
RES_TYPES = {0: 'yok/düz', 1: 'doku', 2: 'yazı tipi'}
# `0x52C7C0`: yanıt komutları
COMMANDS = {0: 'betik', 1: 'mesaj→Rt', 2: 'mesaj→oyun', 3: 'mesaj→oyun(paket)', 0x100: 'odak→Rt', 0x101: 'giriş aç/kapa',
            0x102: 'odağı sakla', 0x103: 'odağı geri yükle', 0x105: 'denetleyici aç(maske)', 0x106: 'denetleyici aç(hepsi)',
            0x107: 'denetleyici kapa(maske)', 0x108: 'denetleyici kapa(hepsi)', 0x200: 'paket aç', 0x201: 'paket değiştir',
            0x202: 'paket değiştir(her denetleyici)', 0x203: 'paketi kapat', 0x204: 'paket değiştir(0)',
            0x2C0: '0x2C0', 0x2C1: 'geçmişten paket aç', 0x2C2: '0x2C2',
            0x300: 'EĞER betik==', 0x301: 'EĞER betik!=', 0x500: 'DEĞİLSE', 0x501: 'SON'}
TARGETS = {0: 'herkes', 0xFFFFFFFF: 'oyun', 0xFFFFFFFE: 'tüm paketler', 0xFFFFFFFD: 'bu paket(yanıt)',
           0xFFFFFFFC: 'bu paket', 0xFFFFFFFB: 'oyun(paket)', 0xFFFFFFFA: 'FEng'}
PARAM_TYPES = {1: 'int', 2: 'float', 3: 'vec2', 4: 'vec3', 5: 'quat', 6: 'renk'}
# nesne verisi (SA) dword ofsetleri
DATA_FIELDS = {0: 'renk', 4: 'pivot', 7: 'konum', 10: 'dönüş', 14: 'boy', 17: 'uv', 21: 'köşe renkleri'}
INTERP_ACTION = {0: 'dur', 1: 'döngü', 2: 'git-gel'}


class Names:
    def __init__(self, paths=()):
        self.db = {}
        for w in ('Init', 'FadeIn', 'FadeOut', 'Highlight', 'UnHighlight', 'DIM', 'UnDim', 'Blink', 'hide', 'show'):
            self.db[fhash(w)] = w
        for p in paths:
            self.add(open(p, 'rb').read())

    def add(self, b):
        for m in re.finditer(rb'[\x20-\x7e]{3,64}', b):
            self.db.setdefault(fhash(m.group(0)), m.group(0).decode('latin1'))

    def __call__(self, h):
        return self.db.get(h, f'{h:#010x}')


def color(b4):
    """FEColor dosyada B, G, R, A (`0x5054D0` → D3DCOLOR); RGBA olarak döner."""
    bb, g, r, a = b4
    return f'rgba({r},{g},{bb},{a})'


def dump(d, names):
    names.add(d)
    tree = {t: (o, sz) for t, o, sz in blocks(d, 8, len(d))}
    o, _ = tree['PkHd']
    ver = struct.unpack_from('<I', d, o)[0]
    hdr = struct.unpack_from('<5I', d, o + 4)
    print(f'{pkg_name(d)}  sürüm {ver:#x}  başlık {hdr}  hash {fhash(pkg_name(d)):#010x}')
    if 'TypS' in tree:
        o, sz = tree['TypS']
        print('  TypS (tür → veri boyu):', [(TYPES.get(t, t), s) for t, s in struct.iter_unpack('<II', d[o:o + sz])])
    res = []
    if 'Res\xcc' in tree:
        ro, rsz = tree['Res\xcc']
        sub = {t: (o, sz) for t, o, sz in blocks(d, ro, ro + rsz)}
        no, _ = sub['RsNm']
        qo, _ = sub['RsRq']
        for i in range(struct.unpack_from('<I', d, qo)[0]):
            h, off, typ, a, bb, c = struct.unpack_from('<6I', d, qo + 4 + 24 * i)
            res.append((d[no + off:d.index(b'\0', no + off)].decode('latin1'), typ))
        print(f'  kaynaklar ({len(res)}):', ', '.join(f'{n}[{RES_TYPES.get(t, t)}]' for n, t in res))
    guid_name = {}
    objs = []
    if 'Obj\xcc' in tree:
        oo, osz = tree['Obj\xcc']
        for t, po, psz in blocks(d, oo, oo + osz):
            if t == 'Butn':
                print('  düğme sayısı:', struct.unpack_from('<I', d, po)[0])
            if t != 'FOb\xea':
                continue
            ob = {'scripts': [], 'msgs': []}
            for t2, so, ssz in blocks(d, po, po + psz):
                if t2 == 'ObjD':
                    ob['f'] = dict(fields(d, so, ssz))
                elif t2 == 'Scrp':
                    ob['scripts'].append(list(fields(d, so, ssz)))
                elif t2 == 'MsgR':
                    ob['msgs'] = list(fields(d, so, ssz))
            f = ob['f']
            guid, oh, flags, ri = struct.unpack_from('<IIIi', f['OP'])
            ob.update(guid=guid, oh=oh, flags=flags, res=ri, type=struct.unpack_from('<I', f['Ot'])[0],
                      parent=struct.unpack_from('<I', f['PA'])[0] if 'PA' in f else None)
            guid_name[guid] = names(oh)
            objs.append(ob)
    kids = {}
    for ob in objs:
        kids.setdefault(ob['parent'], []).append(ob)

    def show(ob, depth):
        f, pad = ob['f'], '  ' * (depth + 2)
        sa = f['SA']
        pos = struct.unpack_from('<3f', sa, 28)
        size = struct.unpack_from('<3f', sa, 56)
        rn = res[ob['res']][0] if 0 <= ob['res'] < len(res) else ''
        extra = ''
        if 'St' in f:
            extra += ' "' + f['St'].decode('utf-16le', 'replace').rstrip('\0') + '"'
        if 'SH' in f:
            extra += f' etiket {struct.unpack_from("<I", f["SH"])[0]:#010x}'
        if 'Sj' in f:
            extra += f' hiza {struct.unpack_from("<I", f["Sj"])[0]}'
        if len(sa) >= 0x54:
            extra += ' uv(%.2f,%.2f,%.2f,%.2f)' % struct.unpack_from('<4f', sa, 0x44)
        print(f'{pad}[{TYPES.get(ob["type"], ob["type"])}] {names(ob["oh"])} guid {ob["guid"]:#x} bayrak {ob["flags"]:#x}'
              f' {rn} konum({pos[0]:.1f},{pos[1]:.1f},{pos[2]:.1f}) boy({size[0]:.2f},{size[1]:.2f})'
              f' {color(struct.unpack_from("<4I", sa, 0))}{extra}')
        for sc in ob['scripts']:
            hd = dict(sc).get('Sh')
            if not hd:
                continue
            h, ln, sfl, ntr = struct.unpack_from('<4I', hd)
            tracks = []
            cur = None
            for t, v in sc:
                if t == 'FI':
                    cur = {'pt': v[0], 'size': v[1], 'interp': v[2], 'act': v[3], 'len': struct.unpack_from('<I', v, 4)[0] & 0xFFFFFF}
                    tracks.append(cur)
                elif t == 'To' and cur is not None:
                    cur['off'] = struct.unpack_from('<I', v)[0]
                elif t == 'Kd' and cur is not None:
                    step = cur['size'] + 4
                    cur['keys'] = len(v) // step
            ev = [struct.unpack_from('<3I', v, 12 * k) for t, v in sc if t == 'EV' for k in range(len(v) // 12)]
            chain = [names(struct.unpack_from('<I', v)[0]) for t, v in sc if t == 'Sc']
            tr = ', '.join(f'{DATA_FIELDS.get(t.get("off"), t.get("off"))}:{PARAM_TYPES.get(t["pt"], t["pt"])}×{t.get("keys", 0)}'
                           f'{"/" + INTERP_ACTION.get(t["act"] & 0x7F, t["act"]) if t["act"] & 0x7F else ""}' for t in tracks)
            evs = ', '.join(f'{names(e)}→{TARGETS.get(tg, guid_name.get(tg, hex(tg)))}@{tm}' for e, tg, tm in ev)
            print(f'{pad}  betik {names(h)} {ln} birim ({ln / 1200:.3f} s) [{tr}]' + (f' olay: {evs}' if evs else '')
                  + (f' → {chain[0]}' if chain else ''))
        msgs(ob['msgs'], pad + '  ', guid_name, names)
        for k in kids.get(ob['guid'], []):
            show(k, depth + 1)

    for ob in kids.get(None, []):
        show(ob, 0)
    if 'PkgR' in tree:
        o, sz = tree['PkgR']
        print('  paket yanıtları:')
        msgs(list(fields(d, o, sz)), '    ', guid_name, names)
    if 'Targ' in tree:
        o, sz = tree['Targ']
        for t, v in fields(d, o, sz):
            if t == 'Mt':
                m, *gs = struct.unpack_from(f'<{len(v) // 4}I', v)
                print(f'  hedef {names(m)} → {", ".join(guid_name.get(g, hex(g)) for g in gs)}')


def msgs(fl, pad, guid_name, names):
    cur = None
    for t, v in fl:
        x = struct.unpack_from('<I', v)[0] if len(v) >= 4 else None
        if t == 'Mi':
            cur = names(x)
        elif t == 'Ri':
            cmd = x
        elif t == 'Ru':
            par = x
        elif t == 'Rt':
            tgt = TARGETS.get(x, guid_name.get(x, hex(x)))
            print(f'{pad}mesaj {cur}: {COMMANDS.get(cmd, hex(cmd))} {names(par) if par else ""} {("→ " + tgt) if x else ""}')


def main():
    a = [x for x in sys.argv[1:]]
    if not a:
        raise SystemExit(__doc__)
    extra = []
    if '--names' in a:
        i = a.index('--names')
        extra, a = a[i + 1:], a[:i]
    names = Names(extra)
    if len(a) == 1:
        for h, how, d in packages(a[0]):
            n = pkg_name(d)
            ok = 'TAMAM' if fhash(n) == h else 'HASH?'
            print(f'{n:40} {h:#010x} {ok} {how:5} {len(d)}')
        return
    for h, how, d in packages(a[0]):
        if pkg_name(d).lower() == a[1].lower():
            dump(d, names)
            return
    raise SystemExit(f'{a[1]} bulunamadı')


if __name__ == '__main__':
    main()
