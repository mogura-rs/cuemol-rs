#!/usr/bin/env python3
"""Compare animation command conversions with the read-only TypeScript and C++ reference."""
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
    work = ROOT / '.cache/animation-value-reference'
    work.mkdir(parents=True, exist_ok=True)
    binary, cases, output = work / 'oracle', work / 'cases', work / 'results.json'
    subprocess.run(['node', str(ROOT / 'scripts/reference-animation-values.mjs'), str(reference), str(cases)], check=True)
    subprocess.run(shlex.split(os.environ.get('CXX', 'c++')) + [
        '-O2', '-std=c++20', '-DHAVE_CONFIG_H', '-DNO_SCRIPT',
        '-I' + str(reference / 'src'), '-I' + str(reference / 'src/modules'),
        '-I' + str(build / 'src'), '-I' + str(build / '_deps/spdlog-src/include'),
        str(ROOT / 'crates/cuemol-command/tests/reference_animation_values.cpp'),
        '-L' + str(build / 'src'), '-lcuemol2', '-Wl,-rpath,' + str(build / 'src'),
        '-o', str(binary)], check=True, cwd=ROOT)
    subprocess.run([str(binary), str(args.config.resolve()), str(cases.with_suffix('.txt')), str(output)], check=True)
    subprocess.run(['cargo', 'test', '--locked', '-p', 'cuemol-command', '--test',
                    'animation_coercion', '--', '--nocapture'], check=True, cwd=ROOT,
                   env=dict(os.environ, CUEMOL_ANIM_VALUE_CASES=str(cases.with_suffix('.json')),
                            CUEMOL_ANIM_VALUE_ORACLE=str(output)))


if __name__ == '__main__':
    main()
