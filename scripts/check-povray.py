#!/usr/bin/env python3
"""Render a CueMol-exported molecular scene with the bundled POV-Ray executable."""
import argparse
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib


def png_colors(path):
    data = path.read_bytes()
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('POV-Ray output is not a PNG')
    width, height, depth, color = struct.unpack('>IIBB', data[16:26])
    if (width, height, depth) != (96, 96, 8) or color not in (2, 6):
        raise ValueError('POV-Ray output has unexpected dimensions or format')
    compressed, offset = bytearray(), 8
    while offset < len(data):
        size = struct.unpack('>I', data[offset:offset + 4])[0]
        if data[offset + 4:offset + 8] == b'IDAT':
            compressed.extend(data[offset + 8:offset + 8 + size])
        offset += size + 12
    raw = zlib.decompress(compressed)
    channels, colors = (3 if color == 2 else 4), set()
    stride = width * channels
    previous = bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind, row = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        for i in range(stride):
            left = row[i - channels] if i >= channels else 0
            above = previous[i]
            diagonal = previous[i - channels] if i >= channels else 0
            predicted = left + above - diagonal
            paeth = min((left, above, diagonal), key=lambda value: abs(predicted - value))
            row[i] = (row[i] + (0, left, above, (left + above) // 2, paeth)[kind]) & 255
        colors.update(bytes(row[i:i + channels]) for i in range(0, stride, channels))
        previous = row
    return len(colors)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    with tempfile.TemporaryDirectory(prefix='cuemol-povray-check-') as directory:
        work = Path(directory)
        env = dict(os.environ, CUEMOL_CONFIG_DIR=str(work / 'config'))
        (work / 'molecule.pdb').write_text(
            'HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\n'
            'HETATM    2  O1  LIG A   1       1.400   0.000   0.000  1.00 10.00           O\nEND\n')
        subprocess.run([str(binary), 'export-scene', 'molecule.pdb', 'molecule.pov'],
                       cwd=work, env=env, check=True)
        scene = work / 'molecule.pov'
        scene.write_text('#include "colors.inc"\n' + scene.read_text())
        subprocess.run([str(binary), 'tools', 'run', 'povray', '+Imolecule.pov',
                        '+Omolecule.png', '+W96', '+H96', '+FN8', '-D', '+WT1'],
                       cwd=work, env=env, check=True)
        count = png_colors(work / 'molecule.png')
        if count < 32:
            raise RuntimeError(f'POV-Ray output lacks molecular shading: {count} colors')
        print(f'Bundled POV-Ray exported-scene check passed: 96x96, {count} colors')


if __name__ == '__main__':
    main()
