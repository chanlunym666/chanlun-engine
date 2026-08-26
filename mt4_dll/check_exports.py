# -*- coding: utf-8 -*-
"""校验 slzs_chanlun_mt4.dll 导出表 (PE 解析, 验证中枢/markers 导出就位)
用法: python check_exports.py <dll路径>
"""
import os
import struct
import sys

if len(sys.argv) < 2 or not os.path.exists(sys.argv[1]):
    print('用法: python check_exports.py <slzs_chanlun_mt4.dll 路径>')
    sys.exit(1)
path = sys.argv[1]
d = open(path, 'rb').read()

pe = struct.unpack_from('<I', d, 0x3C)[0]
num_sections = struct.unpack_from('<H', d, pe + 6)[0]
opt_size = struct.unpack_from('<H', d, pe + 20)[0]
sec = pe + 24 + opt_size
exp_rva, exp_size = struct.unpack_from('<II', d, pe + 24 + 96)  # DataDirectory[0] = Export Table


def rva_to_off(rva):
    for i in range(num_sections):
        va, vs, raw, rs = struct.unpack_from('<IIII', d, sec + i * 40 + 12)
        if va <= rva < va + vs:
            return raw + (rva - va)
    return None


off = rva_to_off(exp_rva)
assert off is not None, f'export dir not found: rva={exp_rva:#x} size={exp_size}'
cnt = struct.unpack_from('<I', d, off + 24)[0]
names_rva = struct.unpack_from('<I', d, off + 32)[0]
noff = rva_to_off(names_rva)
names = []
for i in range(cnt):
    nrva = struct.unpack_from('<I', d, noff + i * 4)[0]
    so = rva_to_off(nrva)
    e = d.index(b'\x00', so)
    names.append(d[so:e].decode('ascii'))

print(f'total exports: {cnt}')
for n in sorted(names):
    print(' ', n)
