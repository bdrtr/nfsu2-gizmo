#!/usr/bin/env python3
"""fx_2_0 (D3DX9 compiled effect) reader: parameters, techniques, passes, states, shaders, FXLC.
Layout follows Wine's d3dx9 effect.c parser. Usage: fx2.py <file.fx> [...]"""
import struct, sys, os
sys.path.insert(0, os.path.dirname(__file__))
import d3dbc

TYPES = ['void', 'bool', 'int', 'float', 'string', 'texture', 'texture1d', 'texture2d', 'texture3d',
         'texturecube', 'sampler', 'sampler1d', 'sampler2d', 'sampler3d', 'samplercube', 'pixelshader',
         'vertexshader', 'pixelfragment', 'vertexfragment']
# Wine state_table (index -> name), the parts that matter here
STATE = {}
_rs = ['ZENABLE', 'FILLMODE', 'SHADEMODE', 'ZWRITEENABLE', 'ALPHATESTENABLE', 'LASTPIXEL', 'SRCBLEND',
       'DESTBLEND', 'CULLMODE', 'ZFUNC', 'ALPHAREF', 'ALPHAFUNC', 'DITHERENABLE', 'ALPHABLENDENABLE',
       'FOGENABLE', 'SPECULARENABLE', 'FOGCOLOR', 'FOGTABLEMODE', 'FOGSTART', 'FOGEND', 'FOGDENSITY',
       'RANGEFOGENABLE', 'STENCILENABLE', 'STENCILFAIL', 'STENCILZFAIL', 'STENCILPASS', 'STENCILFUNC',
       'STENCILREF', 'STENCILMASK', 'STENCILWRITEMASK', 'TEXTUREFACTOR', 'WRAP0', 'WRAP1', 'WRAP2', 'WRAP3',
       'WRAP4', 'WRAP5', 'WRAP6', 'WRAP7', 'WRAP8', 'WRAP9', 'WRAP10', 'WRAP11', 'WRAP12', 'WRAP13', 'WRAP14',
       'WRAP15', 'CLIPPING', 'LIGHTING', 'AMBIENT', 'FOGVERTEXMODE', 'COLORVERTEX', 'LOCALVIEWER',
       'NORMALIZENORMALS', 'DIFFUSEMATERIALSOURCE', 'SPECULARMATERIALSOURCE', 'AMBIENTMATERIALSOURCE',
       'EMISSIVEMATERIALSOURCE', 'VERTEXBLEND', 'CLIPPLANEENABLE', 'POINTSIZE', 'POINTSIZE_MIN',
       'POINTSPRITEENABLE', 'POINTSCALEENABLE', 'POINTSCALE_A', 'POINTSCALE_B', 'POINTSCALE_C',
       'MULTISAMPLEANTIALIAS', 'MULTISAMPLEMASK', 'PATCHEDGESTYLE', 'DEBUGMONITORTOKEN', 'POINTSIZE_MAX',
       'INDEXEDVERTEXBLENDENABLE', 'COLORWRITEENABLE', 'TWEENFACTOR', 'BLENDOP', 'POSITIONDEGREE',
       'NORMALDEGREE', 'SCISSORTESTENABLE', 'SLOPESCALEDEPTHBIAS', 'ANTIALIASEDLINEENABLE',
       'MINTESSELLATIONLEVEL', 'MAXTESSELLATIONLEVEL', 'ADAPTIVETESS_X', 'ADAPTIVETESS_Y', 'ADAPTIVETESS_Z',
       'ADAPTIVETESS_W', 'ENABLEADAPTIVETESSELLATION', 'TWOSIDEDSTENCILMODE', 'CCW_STENCILFAIL',
       'CCW_STENCILZFAIL', 'CCW_STENCILPASS', 'CCW_STENCILFUNC', 'COLORWRITEENABLE1', 'COLORWRITEENABLE2',
       'COLORWRITEENABLE3', 'BLENDFACTOR', 'SRGBWRITEENABLE', 'DEPTHBIAS', 'SEPARATEALPHABLENDENABLE',
       'SRCBLENDALPHA', 'DESTBLENDALPHA', 'BLENDOPALPHA']
for i, n in enumerate(_rs):
    STATE[i] = n
_tss = ['COLOROP', 'COLORARG0', 'COLORARG1', 'COLORARG2', 'ALPHAOP', 'ALPHAARG0', 'ALPHAARG1', 'ALPHAARG2',
        'RESULTARG', 'BUMPENVMAT00', 'BUMPENVMAT01', 'BUMPENVMAT10', 'BUMPENVMAT11', 'TEXCOORDINDEX',
        'BUMPENVLSCALE', 'BUMPENVLOFFSET', 'TEXTURETRANSFORMFLAGS', 'CONSTANT']
for i, n in enumerate(_tss):
    STATE[0x67 + i] = 'TSS.' + n
STATE.update({0x79: 'NPATCHMODE', 0x7a: 'FVF'})
STATE.update({0x7b + i: n for i, n in enumerate(['TRANSFORM.PROJECTION', 'TRANSFORM.VIEW', 'TRANSFORM.WORLD',
                                                 'TRANSFORM.TEXTURE'])})
STATE.update({0x7f + i: 'MATERIAL.' + n for i, n in enumerate(['DIFFUSE', 'AMBIENT', 'SPECULAR', 'EMISSIVE', 'POWER'])})
STATE.update({0x84 + i: 'LIGHT.' + n for i, n in enumerate(['TYPE', 'DIFFUSE', 'SPECULAR', 'AMBIENT', 'POSITION',
                                                           'DIRECTION', 'RANGE', 'FALLOFF', 'ATTENUATION0',
                                                           'ATTENUATION1', 'ATTENUATION2', 'THETA', 'PHI'])})
STATE.update({0x91: 'LIGHTENABLE', 0x92: 'VERTEXSHADER', 0x93: 'PIXELSHADER'})
STATE.update({0x94 + i: n for i, n in enumerate(['VSFLOAT', 'VSBOOL', 'VSINT', 'VSCONST', 'VSCONST1', 'VSCONST2',
                                                 'VSCONST3', 'VSCONST4', 'PSFLOAT', 'PSBOOL', 'PSINT', 'PSCONST',
                                                 'PSCONST1', 'PSCONST2', 'PSCONST3', 'PSCONST4'])})
STATE[0xa4] = 'TEXTURE'
_ss = ['ADDRESSU', 'ADDRESSV', 'ADDRESSW', 'BORDERCOLOR', 'MAGFILTER', 'MINFILTER', 'MIPFILTER',
       'MIPMAPLODBIAS', 'MAXMIPLEVEL', 'MAXANISOTROPY', 'SRGBTEXTURE', 'ELEMENTINDEX', 'DMAPOFFSET']
for i, n in enumerate(_ss):
    STATE[0xa5 + i] = 'SAMP.' + n
STATE[0xb2] = 'SAMPLER'


class Fx:
    def __init__(self, raw):
        self.raw = raw
        tag, off = struct.unpack_from('<II', raw, 0)
        assert tag == 0xFEFF0901, hex(tag)
        self.d = raw[8:]
        self.p = off
        self.objects = {}
        self.params = []
        self.techs = []
        self.parse()

    def u(self):
        v = struct.unpack_from('<I', self.d, self.p)[0]
        self.p += 4
        return v

    def name(self, off):
        if off == 0:
            return ''
        n = struct.unpack_from('<I', self.d, off)[0]
        return self.d[off + 4:off + 4 + n].rstrip(b'\0').decode('latin1')

    def typedef(self, off):
        save = self.p
        self.p = off
        t = dict(type=self.u(), cls=self.u())
        t['name'] = self.name(self.u())
        t['sem'] = self.name(self.u())
        t['elems'] = self.u()
        if t['cls'] == 1:
            t['cols'] = self.u(); t['rows'] = 1
        elif t['cls'] in (0, 2, 3):
            t['cols'] = self.u(); t['rows'] = self.u()
        elif t['cls'] == 5:
            nm = self.u()
            t['members'] = []
            for _ in range(nm):
                t['members'].append(self.typedef(self.p))
        self.p = save if off != save else self.p
        return t

    def value(self, t, off):
        save = self.p
        self.p = off
        n = max(1, t['elems'])
        tn = TYPES[t['type']] if t['type'] < len(TYPES) else t['type']
        if t['cls'] == 4:  # object
            if tn.startswith('sampler'):
                vals = []
                for _ in range(n):
                    cnt = self.u()
                    vals.append([self.state() for _ in range(cnt)])
                v = ('sampler', vals)
            else:
                v = ('obj', [self.u() for _ in range(n)])
        elif t['cls'] == 5:
            v = ('struct', None)
        else:
            cnt = t.get('cols', 1) * t.get('rows', 1) * n
            ws = [self.u() for _ in range(cnt)]
            if tn == 'float':
                v = ('f', struct.unpack(f'<{cnt}f', struct.pack(f'<{cnt}I', *ws)))
            else:
                v = ('i', ws)
        self.p = save
        return v

    def state(self):
        op, idx, toff, voff = self.u(), self.u(), self.u(), self.u()
        t = self.typedef(toff)
        return dict(op=op, name=STATE.get(op, f'state{op:#x}'), idx=idx, t=t, v=self.value(t, voff))

    def parse(self):
        npar, ntech, _unk, nobj = self.u(), self.u(), self.u(), self.u()
        for _ in range(npar):
            toff, voff, flags, nann = self.u(), self.u(), self.u(), self.u()
            anns = [(self.u(), self.u()) for _ in range(nann)]
            t = self.typedef(toff)
            self.params.append(dict(t=t, v=self.value(t, voff), flags=flags,
                                    anns=[(self.typedef(a)['name'], self.value(self.typedef(a), b)) for a, b in anns]))
        for _ in range(ntech):
            nm = self.name(self.u())
            nann, npass = self.u(), self.u()
            for _a in range(nann):
                self.u(); self.u()
            passes = []
            for _p in range(npass):
                pn = self.name(self.u())
                pann, nst = self.u(), self.u()
                for _a in range(pann):
                    self.u(); self.u()
                passes.append(dict(name=pn, states=[self.state() for _ in range(nst)]))
            self.techs.append(dict(name=nm, passes=passes))
        nstr, nres = self.u(), self.u()
        for _ in range(nstr):
            oid = self.u()
            sz = self.u()
            self.objects[oid] = self.d[self.p:self.p + sz]
            self.p += (sz + 3) & ~3
        self.res = []
        for _ in range(nres):
            ti, idx, ei, si, usage = self.u(), self.u(), self.u(), self.u(), self.u()
            r = dict(tech=ti, idx=idx, elem=ei, state=si, usage=usage)
            if usage == 0:
                sz = self.u(); r['data'] = self.d[self.p:self.p + sz]; self.p += (sz + 3) & ~3
            elif usage == 1:
                sz = self.u(); r['data'] = self.d[self.p:self.p + sz]; self.p += (sz + 3) & ~3
            elif usage == 2:
                ln = self.u()
                r['param'] = self.d[self.p:self.p + ln].rstrip(b'\0').decode('latin1'); self.p += (ln + 3) & ~3
                sz = self.u(); r['data'] = self.d[self.p:self.p + sz]; self.p += (sz + 3) & ~3
            self.res.append(r)


def fmtv(v):
    k, x = v
    if k == 'f':
        return '(' + ', '.join(f'{y:g}' for y in x) + ')'
    if k == 'i':
        return '(' + ', '.join(str(y) for y in x) + ')'
    if k == 'obj':
        return 'obj' + str(x)
    if k == 'sampler':
        return 'sampler{' + '; '.join(', '.join(f"{s['name']}={fmtv(s['v'])}" for s in st) for st in x) + '}'
    return str(v)


def dump(path, out=sys.stdout):
    fx = Fx(open(path, 'rb').read())
    pr = lambda *a: print(*a, file=out)
    pr(f'#### {os.path.basename(path)}')
    pr('## parameters')
    for i, p in enumerate(fx.params):
        t = p['t']
        tn = TYPES[t['type']] if t['type'] < len(TYPES) else t['type']
        dims = f"{t.get('rows','')}x{t.get('cols','')}" if 'cols' in t else ''
        pr(f"  [{i}] {tn}{dims}{'['+str(t['elems'])+']' if t['elems'] else ''} {t['name']} : {t['sem']} = {fmtv(p['v'])}"
           + (f"  ann={p['anns']}" if p['anns'] else ''))
    # map resources to states
    resmap = {}
    for r in fx.res:
        resmap[(r['tech'], r['idx'], r['state'])] = r
    shaders_done = set()
    pr('## techniques')
    for ti, t in enumerate(fx.techs):
        pr(f"technique {t['name']}")
        for pi, ps in enumerate(t['passes']):
            pr(f"  pass {ps['name'] or pi}")
            for si, s in enumerate(ps['states']):
                r = resmap.get((ti, pi, si))
                extra = ''
                if r is not None:
                    if r['usage'] == 1:
                        extra = f"  <- param {r['data'].rstrip(bytes(1)).decode('latin1')}"
                    elif r['usage'] == 2:
                        extra = f"  <- selector[{r['param']}] (FXLC)"
                    elif s['name'] not in ('VERTEXSHADER', 'PIXELSHADER'):
                        import struct as _s
                        w = list(_s.unpack_from(f"<{len(r['data'])//4}I", r['data']))
                        extra = '  <- expr: ' + ' ; '.join(d3dbc.preshader(w, None, []))
                    else:
                        extra = ''
                pr(f"    {s['name']}[{s['idx']}] = {fmtv(s['v'])}{extra}")
                if s['name'] in ('VERTEXSHADER', 'PIXELSHADER'):
                    data = None
                    if r is not None and r['usage'] == 0:
                        data = r['data']
                    elif s['v'][0] == 'obj':
                        data = fx.objects.get(s['v'][1][0])
                    key = (s['v'][1][0] if s['v'][0] == 'obj' else None)
                    if data and len(data) >= 4:
                        if key in shaders_done:
                            pr(f'      (shader obj {key} listed above)')
                            continue
                        shaders_done.add(key)
                        try:
                            lines, _, _ = d3dbc.disasm(data, 0)
                            for l in lines:
                                pr('      ' + l)
                        except Exception as e:
                            pr(f'      disasm error {e}')
    # parameter-level resources (sampler states etc.)
    for r in fx.res:
        if r['tech'] == 0xFFFFFFFF:
            pp = fx.params[r['idx']]
            st = pp['v'][1][r['elem']][r['state']] if pp['v'][0] == 'sampler' else None
            if r['usage'] == 1:
                e = 'param ' + r['data'].rstrip(bytes(1)).decode('latin1')
            else:
                import struct as _s
                w = list(_s.unpack_from(f"<{len(r['data'])//4}I", r['data']))
                e = 'expr: ' + ' ; '.join(d3dbc.preshader(w, None, []))
            pr(f"  {pp['t']['name']}.{st['name'] if st else r['state']} <- {e}")


if __name__ == '__main__':
    for f in sys.argv[1:]:
        dump(f)
