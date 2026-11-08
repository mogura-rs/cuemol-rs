"""Tile the first PDB model into independent copies for molecular benchmarks."""

import argparse
import hashlib
import json
import math
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--copies', type=int, default=32)
    parser.add_argument('--spacing', type=float, default=100.)
    args = parser.parse_args()
    if args.input.resolve() == args.output.resolve():
        parser.error('input and output must differ')
    if args.copies < 1 or not math.isfinite(args.spacing) or args.spacing <= 0:
        parser.error('copies and spacing must be positive and finite')
    atoms = []
    for line in args.input.read_text().splitlines():
        if line.startswith('ENDMDL'):
            break
        if line[:6] in ('ATOM  ', 'HETATM'):
            atoms.append(line.ljust(80))
    if not atoms:
        parser.error('the first model contains no atoms')
    if len(atoms) * args.copies > 99999:
        parser.error('the requested copies exceed the PDB atom-number range')
    residues = [int(line[22:26]) for line in atoms]
    stride = max(residues) - min(residues) + 1
    lines = []
    width = math.ceil(args.copies ** (1 / 3))
    for copy in range(args.copies):
        offsets = [copy % width, copy // width % width, copy // (width * width)]
        for line in atoms:
            serial = len(lines) + 1
            residue = int(line[22:26]) + copy * stride
            coordinates = [float(line[start:start + 8]) + offset * args.spacing
                           for start, offset in zip((30, 38, 46), offsets)]
            formatted = [f'{coordinate:8.3f}' for coordinate in coordinates]
            if not -999 <= residue <= 9999 or any(len(value) != 8 for value in formatted):
                parser.error('the requested copies exceed PDB residue or coordinate ranges')
            lines.append(f'{line[:6]}{serial:5d}{line[11:22]}{residue:4d}'
                         f'{line[26:30]}{"".join(formatted)}{line[54:]}')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text('\n'.join([*lines, 'END', '']))
    print(json.dumps(dict(source=args.input.name,
                          source_sha256=hashlib.sha256(args.input.read_bytes()).hexdigest(),
                          output=args.output.name,
                          output_sha256=hashlib.sha256(args.output.read_bytes()).hexdigest(),
                          copies=args.copies, spacing=args.spacing, atoms=len(lines))))


if __name__ == '__main__':
    main()
