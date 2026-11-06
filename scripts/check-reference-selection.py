#!/usr/bin/env python3
"""Compare selection rewrites with the current read-only C++ reference sources."""
import argparse
import os
from pathlib import Path
import shlex
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, default=ROOT.parent / 'cuemol2')
    parser.add_argument('--build-dir', type=Path, required=True)
    parser.add_argument('--config', type=Path, required=True)
    args = parser.parse_args()
    reference, build = args.reference.resolve(), args.build_dir.resolve()
    output = ROOT / '.cache/selection-reference'
    output.mkdir(parents=True, exist_ok=True)
    oracle = output / 'oracle'
    subprocess.run(shlex.split(os.environ.get('CXX', 'c++')) + [
        '-O2', '-std=c++20', '-DHAVE_CONFIG_H', '-DNO_SCRIPT',
        '-I' + str(reference / 'src'), '-I' + str(build / 'src'),
        '-I' + str(build / '_deps/spdlog-src/include'),
        str(ROOT / 'crates/cuemol-molstr/tests/reference_selection.cpp'),
        *(str(reference / 'src/modules/molstr' / source)
          for source in ('SelCommand.cpp', 'SelNodes.cpp', 'SelSimplifier.cpp')),
        '-L' + str(build / 'src'), '-lcuemol2', '-Wl,-rpath,' + str(build / 'src'),
        '-o', str(oracle)], check=True, cwd=ROOT)
    environment = dict(os.environ, CUEMOL_SELECTION_ORACLE=str(oracle),
                       CUEMOL_SELECTION_CONFIG=str(args.config.resolve()))
    subprocess.run(['cargo', 'test', '--locked', '-p', 'cuemol-molstr', '--test',
                    'selection_simplify', '--', '--nocapture'], check=True, cwd=ROOT, env=environment)


if __name__ == '__main__':
    main()
