#!/usr/bin/env python3
"""Compare Rust label formatting with the read-only CueMol C++ implementation."""
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
    output = ROOT / '.cache/label-reference'
    output.mkdir(parents=True, exist_ok=True)
    oracle = output / 'oracle'
    subprocess.run(shlex.split(os.environ.get('CXX', 'c++')) + [
        '-O2', '-std=c++20', '-DHAVE_CONFIG_H', '-DNO_SCRIPT',
        '-I' + str(reference / 'src'), '-I' + str(build / 'src'),
        '-I' + str(build / '_deps/spdlog-src/include'),
        str(ROOT / 'crates/cuemol-molstr/tests/reference_labels.cpp'),
        str(reference / 'src/modules/molstr/LabelFormat.cpp'),
        '-L' + str(build / 'src'), '-lcuemol2', '-Wl,-rpath,' + str(build / 'src'),
        '-o', str(oracle)], check=True, cwd=ROOT)
    environment = dict(os.environ, CUEMOL_LABEL_ORACLE=str(oracle),
                       CUEMOL_LABEL_CONFIG=str(config), CUEMOL2_REF_DIR=str(reference))
    subprocess.run(['cargo', 'test', '--locked', '-p', 'cuemol-molstr', '--test',
                    'label_format_compat', '--', '--nocapture'], check=True, cwd=ROOT, env=environment)


if __name__ == '__main__':
    main()
