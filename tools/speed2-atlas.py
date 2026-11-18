#!/usr/bin/env python3
"""A map of `SPEED2.EXE` (v1.2) drawn from the binary itself: which code reads which string,
which function loads which chunk, which vtable belongs to which named class.

The executable has no symbols and no RTTI, but it is full of names: SlotPool names, screen
names, camera movers, FEng package names, file paths, attribute names. Every one of them is
referenced from code by an absolute address. Following those references gives each function a
handful of words, and a run of functions that share words is a subsystem. This prints that map;
it does not decompile anything (REA does that), it says where to look.

    tools/speed2-atlas.py <SPEED2.EXE> chunks              # chunk id -> loader / unloader / users
    tools/speed2-atlas.py <SPEED2.EXE> funcs  [regex]      # function -> the strings it touches
    tools/speed2-atlas.py <SPEED2.EXE> string <regex>      # string -> the functions touching it
    tools/speed2-atlas.py <SPEED2.EXE> regions [kib]       # address ranges and their vocabulary
    tools/speed2-atlas.py <SPEED2.EXE> vtables             # vtables named by their constructor's strings

Needs `objdump` (binutils). The disassembly is cached next to nothing: it is rebuilt in memory
on each run (about 20 s). No game data is written anywhere.
"""
import bisect
import collections
import re
import struct
import subprocess
import sys

CHUNK_LOADER_CTOR = 0x43C0B0  # bChunkLoader(id, load, unload): this=ecx, hashed into 0x828050[64]
# A second, plain loader list: 36 load functions at 0x7FA858 and their 36 unloaders right after it,
# tried in order by 0x570D70 (unload 0x577B10). The chunk id is compared inside each load function.
STATIC_LOADERS, STATIC_UNLOADERS, STATIC_COUNT = 0x7FA858, 0x7FA8E8, 36


def pe_sections(b):
    pe = struct.unpack_from('<I', b, 0x3C)[0]
    n = struct.unpack_from('<H', b, pe + 6)[0]
    opt = struct.unpack_from('<H', b, pe + 20)[0]
    base = struct.unpack_from('<I', b, pe + 24 + 28)[0]
    out = []
    for i in range(n):
        o = pe + 24 + opt + 40 * i
        name = b[o:o + 8].rstrip(b'\0').decode('latin1')
        vsize, va, rsize, roff = struct.unpack_from('<IIII', b, o + 8)
        out.append((name, base + va, vsize, roff, rsize))
    return out


class Exe:
    def __init__(self, path):
        self.path = path
        self.b = open(path, 'rb').read()
        self.secs = pe_sections(self.b)
        text = next(s for s in self.secs if s[0] == '.text')
        self.text = (text[1], text[1] + text[2])

    def off(self, va):
        for _, sva, vs, ro, rs in self.secs:
            if sva <= va < sva + min(vs, rs):
                return ro + va - sva
        return None

    def dword(self, va):
        o = self.off(va)
        return None if o is None else struct.unpack_from('<I', self.b, o)[0]

    def cstr(self, va, lim=200):
        o = self.off(va)
        if o is None:
            return None
        e = self.b.find(b'\0', o, o + lim)
        if e <= o:
            return None
        s = self.b[o:e]
        if not all(32 <= c < 127 for c in s):
            return None
        return s.decode('ascii')

    def in_text(self, va):
        return self.text[0] <= va < self.text[1]


def disasm(path):
    out = subprocess.run(['objdump', '-d', '-M', 'intel', '--no-show-raw-insn', path],
                         capture_output=True, text=True, check=True).stdout
    ins = []
    for l in out.split('\n'):
        m = re.match(r'\s+([0-9a-f]+):\s+(.*)', l)
        if m:
            ins.append((int(m.group(1), 16), m.group(2).strip()))
    return ins


IMM = re.compile(r'0x([0-9a-f]{5,8})\b')  # 5 digits: chunk ids such as `cmp eax,0x34102`


def strings_ok(s):
    return s is not None and len(s) >= 4 and re.search(r'[A-Za-z]{3}', s)


class Atlas:
    def __init__(self, path):
        self.exe = Exe(path)
        self.ins = disasm(path)
        self.addrs = [a for a, _ in self.ins]
        starts = set()
        for a, t in self.ins:
            m = re.match(r'call\s+0x([0-9a-f]+)$', t)
            if m:
                starts.add(int(m.group(1), 16))
        # Every .rdata/.data dword that points at an instruction start in .text is a function
        # pointer candidate (vtables, callback tables, chunk loaders).
        self.vt_slots = {}
        istart = set(self.addrs)
        for name, va, vs, ro, rs in self.exe.secs:
            if name not in ('.rdata', '.data'):
                continue
            for o in range(0, min(vs, rs) - 3, 4):
                v = struct.unpack_from('<I', self.exe.b, ro + o)[0]
                if self.exe.in_text(v) and v in istart:
                    self.vt_slots[va + o] = v
                    starts.add(v)
        # Callbacks handed over as immediates (`push 0x5df8a0` into bChunkLoader, file-queue
        # callbacks) are functions too, when the code before them ends a function or pads.
        prev = {a: t for (a, _), (_, t) in zip(self.ins[1:], self.ins)}
        for a, t in self.ins:
            m = re.match(r'(?:push|mov\s+\S+,)\s+0x([0-9a-f]+)$', t)
            if m:
                v = int(m.group(1), 16)
                if v in istart and prev.get(v, '').startswith(('ret', 'nop', 'int3', 'jmp')):
                    starts.add(v)
        self.starts = sorted(s for s in starts if self.exe.in_text(s))
        self.fstr = collections.defaultdict(list)  # func -> [(insn addr, string)]
        self.sfun = collections.defaultdict(set)   # string -> funcs
        self.imm_refs = collections.defaultdict(set)  # imm value -> funcs
        for a, t in self.ins:
            if t.startswith(('call', 'jmp', 'j')):
                continue
            for m in IMM.finditer(t):
                v = int(m.group(1), 16)
                f = self.func(a)
                self.imm_refs[v].add(f)
                s = self.exe.cstr(v)
                if strings_ok(s):
                    self.fstr[f].append((a, s))
                    self.sfun[s].add(f)

    def func(self, a):
        i = bisect.bisect_right(self.starts, a) - 1
        return self.starts[i] if i >= 0 else 0

    # -- reports ---------------------------------------------------------------------------

    def chunks(self):
        ins = self.ins
        rows = []
        for i, (a, t) in enumerate(ins):
            if t == f'call   {CHUNK_LOADER_CTOR:#x}':
                win = [x[1] for x in ins[max(0, i - 8):i]]
                pushes = [int(w.split()[1], 16) for w in win if re.match(r'push\s+0x', w)]
                if len(pushes) >= 3:
                    cid, load, unload = pushes[-1], pushes[-2], pushes[-3]
                    rows.append((cid, load, unload))
        print('# bChunkLoader registrations (static init): id  load  unload')
        reg = {c for c, _, _ in rows}
        for cid, load, unload in sorted(rows, key=lambda r: r[0] & 0x7fffffff):
            print(f'{cid:#010x}\tload {load:#x}\tunload {unload:#x}')
        def chunky(v):
            lo = v & 0x7fffffff
            return (0x30000 <= lo <= 0x3ffff and lo & 0xff00 != 0xff00) or 0x134000 <= lo <= 0x13ffff \
                or (v >> 24) in (0xb0, 0xb3) or (v >> 12) == 0xE34  # EAGL anim bankası/iskelet
        by_func = collections.defaultdict(set)
        for v, fs in self.imm_refs.items():
            if chunky(v) and v != 0x30000:
                for f in fs:
                    by_func[f].add(v)
        print(f'\n# static loader list {STATIC_LOADERS:#x} (tried in order): #  load  unload  chunk ids the loader compares')
        for i in range(STATIC_COUNT):
            load = self.exe.dword(STATIC_LOADERS + 4 * i)
            unload = self.exe.dword(STATIC_UNLOADERS + 4 * i)
            ids = ' '.join(f'{v:#x}' for v in sorted(by_func.get(load, ()), key=lambda v: v & 0x7fffffff))
            print(f'{i}\tload {load:#x}\tunload {unload:#x}\t{ids or "(id not an immediate)"}')
        print('\n# every chunk-id-shaped immediate, and the functions using it')
        for v in sorted(self.imm_refs, key=lambda v: v & 0x7fffffff):
            if chunky(v) and v not in (0x30000,):
                fs = ' '.join(f'{f:#x}' for f in sorted(self.imm_refs[v]))
                print(f'{v:#010x}\t{"R " if v in reg else "  "}{fs}')

    def funcs(self, rx):
        r = re.compile(rx) if rx else None
        for f in sorted(self.fstr):
            ss = []
            for _, s in self.fstr[f]:
                if s not in ss:
                    ss.append(s)
            if r and not any(r.search(s) for s in ss):
                continue
            print(f'{f:#x}\t' + ' | '.join(s[:60] for s in ss[:12]) + (' …' if len(ss) > 12 else ''))

    def string(self, rx):
        r = re.compile(rx)
        for s in sorted(self.sfun):
            if r.search(s):
                print(f'{s[:70]!r}\t' + ' '.join(f'{f:#x}' for f in sorted(self.sfun[s])))

    def regions(self, kib):
        step = kib * 1024
        lo, hi = self.exe.text
        words = re.compile(r'[A-Z][a-z]+|[a-z]{4,}|[A-Z]{3,}')
        for base in range(lo & ~(step - 1), hi, step):
            c = collections.Counter()
            nf = 0
            for f in self.fstr:
                if base <= f < base + step:
                    nf += 1
                    for _, s in self.fstr[f]:
                        for w in set(words.findall(s)):
                            if len(w) > 3:
                                c[w.lower()] += 1
            top = ', '.join(w for w, _ in c.most_common(14))
            print(f'{base:#x}-{base + step:#x}\t{nf:4d} fn\t{top}')

    def vtables(self):
        # A vtable is a run of >=2 consecutive function-pointer slots. Name it by the strings
        # of the functions that store its address (constructors write `mov [reg], vtable`).
        slots = sorted(self.vt_slots)
        runs = []
        i = 0
        while i < len(slots):
            j = i
            while j + 1 < len(slots) and slots[j + 1] == slots[j] + 4:
                j += 1
            if j > i:
                runs.append((slots[i], j - i + 1))
            i = j + 1
        stores = collections.defaultdict(set)
        rx = re.compile(r'mov\s+DWORD PTR \[e[a-z]{2}(?:\+0x[0-9a-f]+)?\],0x([0-9a-f]{6})$')
        for a, t in self.ins:
            m = rx.match(t)
            if m:
                stores[int(m.group(1), 16)].add(self.func(a))
        for va, n in runs:
            ctors = stores.get(va, set())
            names = []
            for f in sorted(ctors):
                for _, s in self.fstr.get(f, []):
                    if s not in names:
                        names.append(s)
            print(f'{va:#x}\t{n:3d} slots\tctor {" ".join(f"{f:#x}" for f in sorted(ctors)) or "-"}'
                  f'\t{" | ".join(s[:40] for s in names[:5])}')


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(2)
    at = Atlas(sys.argv[1])
    cmd, rest = sys.argv[2], sys.argv[3:]
    if cmd == 'chunks':
        at.chunks()
    elif cmd == 'funcs':
        at.funcs(rest[0] if rest else None)
    elif cmd == 'string':
        at.string(rest[0])
    elif cmd == 'regions':
        at.regions(int(rest[0]) if rest else 64)
    elif cmd == 'vtables':
        at.vtables()
    else:
        print(__doc__)
        sys.exit(2)


if __name__ == '__main__':
    main()
