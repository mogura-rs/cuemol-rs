#!/usr/bin/env python3
"""Verify the bundled APBS runtime with a charged-solute potential calculation."""
import argparse
import math
import os
from pathlib import Path
import re
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    binary = parser.parse_args().binary.resolve()
    with tempfile.TemporaryDirectory(prefix='cuemol-apbs-check-') as directory:
        work = Path(directory)
        (work / 'ion.pqr').write_text(
            'ATOM      1  NA  ION     1       0.000   0.000   0.000  1.0000 1.5000\n')
        (work / 'ion.in').write_text('''read
  mol pqr ion.pqr
end
elec
  mg-auto
  dime 33 33 33
  cglen 40 40 40
  fglen 20 20 20
  cgcent mol 1
  fgcent mol 1
  mol 1
  lpbe
  bcfl sdh
  pdie 2.0
  sdie 78.54
  chgm spl2
  srfm smol
  srad 1.4
  sdens 10.0
  swin 0.3
  temp 298.15
  calcenergy no
  calcforce no
  write pot dx potential
end
quit
''')
        env = dict(os.environ, CUEMOL_CONFIG_DIR=str(work / 'config'), OMP_NUM_THREADS='1')
        result = subprocess.run([str(binary), 'tools', 'run', 'apbs', 'ion.in'],
                                cwd=work, env=env, capture_output=True, text=True, timeout=90)
        if result.returncode:
            raise RuntimeError(f'APBS calculation failed ({result.returncode}):\n'
                               f'{result.stdout}\n{result.stderr}')
        paths = [work / 'potential.dx', work / 'potential-PE0.dx']
        output = next((path for path in paths if path.is_file()), None)
        if output is None:
            raise RuntimeError(f'APBS did not write its potential grid:\n{result.stdout}')
        data = output.read_text()
        header = re.search(r'items\s+(\d+)\s+data follows\s*\n', data)
        if header is None or int(header[1]) != 33 ** 3:
            raise RuntimeError('APBS wrote an invalid grid header')
        values = []
        for line in data[header.end():].splitlines():
            if len(values) >= 33 ** 3:
                break
            values.extend(float(value) for value in line.split())
        if len(values) != 33 ** 3 or not all(math.isfinite(value) for value in values):
            raise RuntimeError('APBS wrote an incomplete or non-finite potential grid')
        if not values[len(values) // 2] > values[0] > 0:
            raise RuntimeError('APBS potential does not decay away from the positive charge')
        print(f'Bundled APBS calculation passed: {len(values)} finite potential samples')


if __name__ == '__main__':
    main()
