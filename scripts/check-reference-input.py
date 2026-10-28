#!/usr/bin/env python3
"""Build the read-only C++ input oracle and compare camera event sequences."""
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
    reference, build, config = args.reference.resolve(), args.build_dir.resolve(), args.config.resolve()
    if not config.is_file():
        parser.error('The C++ runtime configuration must exist')
    output = ROOT / '.cache/input-reference'
    output.mkdir(parents=True, exist_ok=True)
    oracle = output / 'oracle'
    compiler = shlex.split(os.environ.get('CXX', 'c++'))
    subprocess.run(compiler + ['-std=c++17', '-DHAVE_CONFIG_H',
        '-I' + str(reference / 'src'), '-I' + str(build / 'src'),
        '-I' + str(build / '_deps/spdlog-src/include'),
        str(ROOT / 'crates/cuemol-qsys/tests/reference_input.cpp'),
        '-L' + str(build / 'src'), '-lcuemol2', '-Wl,-rpath,' + str(build / 'src'),
        '-o', str(oracle)], check=True, cwd=ROOT)
    environment = dict(os.environ, CUEMOL2_INPUT_ORACLE=str(oracle), CUEMOL2_INPUT_CONFIG=str(config))
    subprocess.run(['cargo', 'test', '--locked', '-p', 'cuemol-qsys', '--test',
        'view_input_compat', '--', '--nocapture'], check=True, cwd=ROOT, env=environment)


if __name__ == '__main__':
    main()
