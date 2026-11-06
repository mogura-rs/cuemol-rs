#!/usr/bin/env python3
"""Compare animation property metadata and default flags with the read-only reference."""
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
    work = ROOT / '.cache/animation-property-reference'
    work.mkdir(parents=True, exist_ok=True)
    binary, output = work / 'oracle', work / 'properties.json'
    subprocess.run(shlex.split(os.environ.get('CXX', 'c++')) + [
        '-O2', '-std=c++20', '-DHAVE_CONFIG_H', '-DNO_SCRIPT',
        '-I' + str(reference / 'src'), '-I' + str(reference / 'src/modules'),
        '-I' + str(build / 'src'), '-I' + str(build / '_deps/spdlog-src/include'),
        str(ROOT / 'crates/cuemol-command/tests/reference_animation_properties.cpp'),
        '-L' + str(build / 'src'), '-lcuemol2', '-Wl,-rpath,' + str(build / 'src'),
        '-o', str(binary)], check=True, cwd=ROOT)
    subprocess.run([str(binary), str(args.config.resolve()), str(output)], check=True, cwd=ROOT)
    subprocess.run(['cargo', 'test', '--locked', '-p', 'cuemol-command', '--test',
                    'animation_properties_compat', '--', '--nocapture'], check=True, cwd=ROOT,
                   env=dict(os.environ, CUEMOL_ANIM_PROPERTY_ORACLE=str(output)))


if __name__ == '__main__':
    main()
