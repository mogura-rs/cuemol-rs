#!/usr/bin/env python3
"""Compare molecular surfaces with the read-only CueMol C++ implementation."""
import argparse
import os
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, default=ROOT.parent / 'cuemol2')
    parser.add_argument('--build-dir', type=Path, required=True)
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--meshms-reference', type=Path, default=ROOT / '.cache/meshms')
    args = parser.parse_args()
    reference, build, config = args.reference.resolve(), args.build_dir.resolve(), args.config.resolve()
    if not config.is_file():
        parser.error('The C++ runtime configuration must exist')
    output = ROOT / '.cache/surface-reference'
    output.mkdir(parents=True, exist_ok=True)
    oracle = output / 'oracle'
    meshms = args.meshms_reference.resolve()
    if not (meshms / 'include/meshms/meshms.hpp').is_file():
        parser.error('Provide an unmodified MeshMS source checkout with --meshms-reference')
    compiler = shlex.split(os.environ.get('CXX', 'c++'))
    exports = ['-rdynamic', '-ldl'] if sys.platform.startswith('linux') else ['-Wl,-export_dynamic']
    subprocess.run(compiler + ['-O2', '-std=c++20', '-ffp-contract=off',
        '-DHAVE_CONFIG_H', '-DNO_SCRIPT', '-DHAVE_MESHMS',
        '-I' + str(reference / 'src'), '-I' + str(build / 'src'),
        '-I' + str(build / '_deps/spdlog-src/include'), '-I' + str(meshms / 'include'),
        str(ROOT / 'crates/cuemol-surface/tests/reference_surface.cpp'),
        str(reference / 'src/modules/surface/DirectSurfRenderer.cpp'),
        *map(str, sorted((meshms / 'src').glob('*.cpp'))),
        '-L' + str(build / 'src'), '-lcuemol2', '-Wl,-rpath,' + str(build / 'src'),
        *exports, '-o', str(oracle)], check=True, cwd=ROOT)
    environment = dict(os.environ, CUEMOL_SURFACE_ORACLE=str(oracle),
                       CUEMOL_SURFACE_CONFIG=str(config), CUEMOL2_REF_DIR=str(reference))
    subprocess.run(['cargo', 'test', '--locked', '-p', 'cuemol-surface', '--test',
        'direct_surface_compat', '--', '--nocapture'], check=True, cwd=ROOT, env=environment)


if __name__ == '__main__':
    main()
