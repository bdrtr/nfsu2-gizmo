"""Minimal D3D9 shader bytecode (vs/ps 1.x-2.0) disassembler, CTAB and preshader (PRES/CLIT/FXLC) reader."""
import struct

OPS = {  # opcode: (name, nparams for SM1 where length field is absent)
    0: ('nop', 0), 1: ('mov', 2), 2: ('add', 3), 3: ('sub', 3), 4: ('mad', 4), 5: ('mul', 3),
    6: ('rcp', 2), 7: ('rsq', 2), 8: ('dp3', 3), 9: ('dp4', 3), 10: ('min', 3), 11: ('max', 3),
    12: ('slt', 3), 13: ('sge', 3), 14: ('exp', 2), 15: ('log', 2), 16: ('lit', 2), 17: ('dst', 3),
    18: ('lrp', 4), 19: ('frc', 2), 20: ('m4x4', 3), 21: ('m4x3', 3), 22: ('m3x4', 3), 23: ('m3x3', 3),
    24: ('m3x2', 3), 31: ('dcl', 2), 32: ('pow', 3), 33: ('crs', 3), 34: ('sgn', 4), 35: ('abs', 2),
    36: ('nrm', 2), 37: ('sincos', 4), 46: ('mova', 2), 64: ('texcoord', 1), 65: ('texkill', 1),
    66: ('tex', 1), 67: ('texbem', 2), 68: ('texbeml', 2), 69: ('texreg2ar', 2), 70: ('texreg2gb', 2),
    71: ('texm3x2pad', 2), 72: ('texm3x2tex', 2), 73: ('texm3x3pad', 2), 74: ('texm3x3tex', 2),
    76: ('texm3x3spec', 3), 77: ('texm3x3vspec', 2), 78: ('expp', 2), 79: ('logp', 2), 80: ('cnd', 4),
    81: ('def', 5), 82: ('texreg2rgb', 2), 83: ('texdp3tex', 2), 84: ('texm3x2depth', 2), 85: ('texdp3', 2),
    86: ('texm3x3', 2), 87: ('texdepth', 1), 88: ('cmp', 4), 89: ('bem', 3), 90: ('dp2add', 4),
    0xFFFD: ('phase', 0),
}
SRCMOD = {0: '{}', 1: '-{}', 2: '{}_bias', 3: '-{}_bias', 4: '{}_bx2', 5: '-{}_bx2', 6: '1-{}', 7: '{}_x2',
          8: '-{}_x2', 9: '{}_dz', 10: '{}_dw', 11: 'abs({})', 12: '-abs({})', 13: '!{}'}
SHIFT = {0: '', 1: '_x2', 2: '_x4', 3: '_x8', 15: '_d2', 14: '_d4', 13: '_d8'}
USAGE = {0: 'position', 1: 'blendweight', 2: 'blendindices', 3: 'normal', 4: 'psize', 5: 'texcoord',
         6: 'tangent', 7: 'binormal', 8: 'tessfactor', 9: 'positiont', 10: 'color', 11: 'fog', 12: 'depth', 13: 'sample'}


def regname(tok, ps, ver):
    t = ((tok >> 28) & 7) | ((tok >> 8) & 0x18)
    n = tok & 0x7FF
    if t == 0: return f'r{n}'
    if t == 1: return f'v{n}'
    if t == 2: return f'c{n}'
    if t == 3: return f't{n}' if ps else f'a{n}'
    if t == 4: return ['oPos', 'oFog', 'oPts'][n]
    if t == 5: return f'oD{n}'
    if t == 6: return f'oT{n}'
    if t == 7: return f'i{n}'
    if t == 8: return f'oC{n}'
    if t == 9: return 'oDepth'
    if t == 10: return f's{n}'
    if t == 14: return f'b{n}'
    return f'?{t}_{n}'


def dst(tok, ps, ver):
    r = regname(tok, ps, ver)
    m = (tok >> 16) & 0xF
    mod = (tok >> 20) & 0xF
    sh = (tok >> 24) & 0xF
    s = r + ('' if m == 0xF else '.' + ''.join('xyzw'[i] for i in range(4) if m >> i & 1))
    pre = ''
    if mod & 1: pre += '_sat'
    if mod & 2: pre += '_pp'
    return s, SHIFT.get(sh, f'_sh{sh}') + pre


def src(tok, ps, ver, rel=''):
    r = regname(tok, ps, ver)
    if tok & 0x2000:
        r = r[0] + f'[a0.x+{r[1:]}]'
    sw = (tok >> 16) & 0xFF
    comps = ''.join('xyzw'[(sw >> (2 * i)) & 3] for i in range(4))
    if comps == 'xyzw': swz = ''
    elif len(set(comps)) == 1: swz = '.' + comps[0]
    else: swz = '.' + comps
    return SRCMOD.get((tok >> 24) & 0xF, '{}?').format(r + swz)


def ctab(words, base):
    """words: dword list of the CTAB comment payload after the fourcc."""
    raw = struct.pack(f'<{len(words)}I', *words)
    def cs(o):
        e = raw.find(b'\0', o)
        return raw[o:e].decode('latin1')
    size, creator, ver, n, cinfo, flags, target = struct.unpack_from('<7I', raw, 0)
    out = []
    for i in range(n):
        name, rs, ri, rc, _, ti, dv = struct.unpack_from('<IHHHHII', raw, cinfo + 20 * i)
        cls, typ, rows, cols, elems, members, moff = struct.unpack_from('<7H', raw, ti)
        default = None
        if dv:
            default = struct.unpack_from(f'<{rc*4}f', raw, dv)
        out.append(dict(name=cs(name), set=['bool', 'int4', 'float4', 'sampler'][rs], reg=ri, count=rc,
                        cls=cls, type=typ, rows=rows, cols=cols, default=default))
    return out, cs(target)


PRES_OPS = {0x000: 'nop', 0x100: 'mov', 0x101: 'neg', 0x103: 'rcp', 0x104: 'frc', 0x105: 'exp', 0x106: 'log',
            0x107: 'rsq', 0x108: 'sin', 0x109: 'cos', 0x10a: 'asin', 0x10b: 'acos', 0x10c: 'atan',
            0x200: 'min', 0x201: 'max', 0x202: 'lt', 0x203: 'ge', 0x204: 'add', 0x205: 'mul',
            0x206: 'atan2', 0x208: 'div', 0x300: 'cmp', 0x301: 'movc', 0x500: 'dot', 0x502: 'noise'}


def fxlc(words, lits, ins_ctab, nameof_out):
    """Decode an FXLC block. lits: CLIT doubles; ins_ctab: constants table of the preshader inputs."""
    n = words[0]
    p = 1
    lines = []
    def argname(table, off):
        if table == 1:
            return repr(lits[off]) if off < len(lits) else f'lit[{off}]'
        if table == 2:
            reg, comp = divmod(off, 4)
            for c in ins_ctab:
                if c['reg'] <= reg < c['reg'] + c['count']:
                    sub = reg - c['reg']
                    return f"{c['name']}{'' if c['count'] == 1 else f'[{sub}]'}.{'xyzw'[comp]}"
            return f'in{reg}.{"xyzw"[comp]}'
        if table == 4:
            reg, comp = divmod(off, 4)
            return nameof_out(reg, comp)
        if table == 7:
            reg, comp = divmod(off, 4)
            return f'tmp{reg}.{"xyzw"[comp]}'
        return f'T{table}[{off}]'
    for _ in range(n):
        tok = words[p]; p += 1
        op = (tok & 0x7FF00000) >> 20
        scalar = tok >> 31
        ncomp = tok & 0xFFFF
        nargs = words[p]; p += 1
        args = []
        for _a in range(nargs + 1):
            if words[p] != 0:
                idx_t, idx_o = words[p + 1], words[p + 2]
                p += 3
                t, o = words[p], words[p + 1]; p += 2
                args.append(argname(t, o) + f'[idx {argname(idx_t, idx_o)}]')
                continue
            p += 1
            t, o = words[p], words[p + 1]; p += 2
            args.append((t, o))
        names = []
        for a in args:
            names.append(a if isinstance(a, str) else a)
        out = names[-1]
        ins = names[:-1]
        def nm(a, k):
            if isinstance(a, str): return a
            t, o = a
            if t in (1,) and scalar and a is ins[0]:
                return argname(t, o)
            return argname(t, o + k)
        for k in range(ncomp):
            iv = []
            for j, a in enumerate(ins):
                if scalar and j == 0 and op not in (0x500,):
                    iv.append(nm(a, 0) if isinstance(a, str) else argname(a[0], a[1]))
                else:
                    iv.append(nm(a, k) if isinstance(a, str) else argname(a[0], a[1] + k))
            lines.append(f'{argname(out[0], out[1] + k) if not isinstance(out, str) else out} = '
                         f'{PRES_OPS.get(op, hex(op))}({", ".join(iv)})')
            if op == 0x500:
                break
    return lines


def disasm(b, off=0):
    """Disassemble shader bytecode starting at b[off]. Returns (lines, ctab, end_offset)."""
    ver = struct.unpack_from('<I', b, off)[0]
    ps = (ver >> 16) == 0xFFFF
    major, minor = (ver >> 8) & 0xFF, ver & 0xFF
    lines = [f"{'ps' if ps else 'vs'}_{major}_{minor}"]
    p = off + 4
    consts = []
    pres_lines = []
    while True:
        tok = struct.unpack_from('<I', b, p)[0]
        p += 4
        if tok == 0x0000FFFF:
            break
        op = tok & 0xFFFF
        if op == 0xFFFE:
            n = (tok >> 16) & 0x7FFF
            words = list(struct.unpack_from(f'<{n}I', b, p))
            p += 4 * n
            fourcc = struct.pack('<I', words[0])
            if fourcc == b'CTAB':
                consts, target = ctab(words[1:], 0)
                for c in consts:
                    d = '' if c['default'] is None else f'  = {tuple(round(x, 4) for x in c["default"])}'
                    lines.append(f"// {c['set'][0] if c['set']!='sampler' else 's'}{c['reg']}"
                                 f"{'' if c['count']==1 else '..'+str(c['reg']+c['count']-1)}  {c['name']}{d}")
            elif fourcc == b'PRES':
                pres_lines = preshader(words[1:], b, consts)
                lines += ['// preshader:'] + ['//   ' + l for l in pres_lines]
            else:
                lines.append(f'// comment {fourcc!r} {n} dwords')
            continue
        name, npar = OPS.get(op, (f'op{op}', 0))
        if major >= 2:
            npar = (tok >> 24) & 0xF
        elif op == 66 and ps and minor == 4:
            npar = 2; name = 'texld'
        elif op == 64 and ps and minor == 4:
            npar = 2; name = 'texcrd'
        elif op == 66 and ps and major >= 2:
            name = 'texld'
        params = list(struct.unpack_from(f'<{npar}I', b, p))
        p += 4 * npar
        co = '+' if tok & 0x40000000 else ''
        if op == 31:  # dcl
            u = params[0]
            if ps and major >= 2 and ((params[1] >> 28) & 7) | ((params[1] >> 8) & 0x18) == 10:
                lines.append(f'dcl_{["?","?","2d","cube","volume"][(u>>27)&0xF]} {dst(params[1], ps, major)[0]}')
            else:
                lines.append(f'dcl_{USAGE.get(u & 0x1F, u & 0x1F)}{(u >> 16) & 0xF or ""} {dst(params[1], ps, major)[0]}')
            continue
        if op == 81:
            fl = struct.unpack('<4f', struct.pack('<4I', *params[1:5]))
            lines.append(f'def {dst(params[0], ps, major)[0]}, {", ".join(f"{x:g}" for x in fl)}')
            continue
        if npar == 0:
            lines.append(co + name)
            continue
        d, mod = dst(params[0], ps, major)
        ss = [src(x, ps, major) for x in params[1:]]
        lines.append(f'{co}{name}{mod} {d}' + (', ' + ', '.join(ss) if ss else ''))
    return lines, consts, p


def preshader(words, b, vsconsts):
    raw = struct.pack(f'<{len(words)}I', *words)
    # PRES payload: version token (0x46584xxx 'FX' tag), then comments CTAB/CLIT/FXLC, then end
    p = 1
    ins_ctab = []
    lits = []
    out = []
    while p < len(words):
        tok = words[p]; p += 1
        if tok & 0xFFFF == 0xFFFE:
            n = (tok >> 16) & 0x7FFF
            w = words[p:p + n]; p += n
            cc = struct.pack('<I', w[0])
            if cc == b'CTAB':
                ins_ctab, _ = ctab(list(w[1:]), 0)
            elif cc == b'CLIT':
                cnt = w[1]
                lits = list(struct.unpack(f'<{cnt}d', struct.pack(f'<{2*cnt}I', *w[2:2 + 2 * cnt])))
            elif cc == b'FXLC':
                def nameof_out(reg, comp):
                    for c in vsconsts:
                        if c['set'] == 'float4' and c['reg'] <= reg < c['reg'] + c['count']:
                            pass
                    return f'c{reg}.{"xyzw"[comp]}'
                try:
                    out += fxlc(list(w[1:]), lits, ins_ctab, nameof_out)
                except Exception as e:
                    out.append(f'fxlc decode error {e}')
            elif cc == b'PRSI':
                out.append(f'PRSI {list(w[1:])}')
        elif tok == 0xFFFF:
            break
    return out
