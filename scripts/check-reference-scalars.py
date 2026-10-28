#!/usr/bin/env python3
"""Compare continuous scalar-map samples with the read-only C++ implementation."""
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
    output = ROOT / '.cache/scalar-reference'
    output.mkdir(parents=True, exist_ok=True)
    for name, source, test, variable in [
        ('oracle', 'reference_scalar.cpp', 'scalar_map_compat', 'CUEMOL_SCALAR_ORACLE'),
        ('levels', 'reference_map_levels.cpp', 'map_levels_compat', 'CUEMOL_MAP_LEVEL_ORACLE'),
    ]:
        oracle = output / name
        subprocess.run(shlex.split(os.environ.get('CXX', 'c++')) + [
            '-O2', '-std=c++20', '-DHAVE_CONFIG_H', '-DNO_SCRIPT',
            '-I' + str(reference / 'src'), '-I' + str(build / 'src'),
            '-I' + str(build / '_deps/spdlog-src/include'),
            str(ROOT / 'crates/cuemol-xtal/tests' / source),
            '-L' + str(build / 'src'), '-lcuemol2', '-Wl,-rpath,' + str(build / 'src'),
            '-o', str(oracle)], check=True, cwd=ROOT)
        environment = dict(os.environ, **{variable: str(oracle),
                           "CUEMOL_SCALAR_CONFIG": str(args.config.resolve())})
        subprocess.run(['cargo', 'test', '--locked', '-p', 'cuemol-xtal', '--test',
                        test, '--', '--nocapture'], check=True, cwd=ROOT, env=environment)



if __name__ == '__main__':
    main()
