#!/usr/bin/env python3
"""Stage or verify a portable browser distribution and its source manifest."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = (
    'index.html', 'kernels.js', 'oidn.js', 'pkg/cuemol_web.js',
    'pkg/cuemol_web_bg.wasm', 'pkg/kernels/usalign.js', 'pkg/kernels/usalign.wasm',
    'pkg/kernels/lcms.js', 'pkg/kernels/lcms.wasm',
    'pkg/kernels/surface.js', 'pkg/kernels/surface.wasm',
    'pkg/oidn/oidn.js', 'pkg/oidn/rt_hdr_calb_cnrm.tza',
    'pkg/oidn/OIDN-WebGPU-LICENSE', 'pkg/oidn/OIDN-Weights-LICENSE',
    'pkg/oidn/SOURCES.json', 'pkg/kernels/BALL-LICENSE',
    'pkg/kernels/Boost-LICENSE', 'pkg/kernels/MeshMS-SOURCE.json',
    'pkg/kernels/BALL-SOURCE.json', 'pkg/kernels/LittleCMS-LICENSE',
    'pkg/kernels/USalign-LICENSE',
)


def digest(path):
    with path.open('rb') as data:
        return hashlib.file_digest(data, 'sha256').hexdigest()


def verify(directory, revision=None):
    manifest = json.loads((directory / 'build-info.json').read_text())
    if revision and manifest['commit'] != revision:
        raise ValueError('Browser artifact does not match the requested commit')
    for name in REQUIRED:
        if name not in manifest['files']:
            raise ValueError(f'Missing browser asset: {name}')
    actual = {p.relative_to(directory).as_posix() for p in directory.rglob('*')
              if p.is_file() and p.name != 'build-info.json'}
    if actual != set(manifest['files']):
        raise ValueError('Browser artifact contains missing or unexpected files')
    for name, expected in manifest['files'].items():
        relative = Path(name)
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError('Invalid browser artifact path')
        path = directory / relative
        if path.is_symlink() or digest(path) != expected:
            raise ValueError(f'Browser asset checksum mismatch: {name}')
        if path.suffix == '.wasm':
            with path.open('rb') as module:
                if module.read(4) != b'\x00asm':
                    raise ValueError(f'Invalid WebAssembly module: {name}')
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=ROOT / 'web')
    parser.add_argument('--out', type=Path, default=ROOT / 'dist/web')
    parser.add_argument('--revision')
    parser.add_argument('--verify', type=Path)
    args = parser.parse_args()
    if args.verify:
        result = verify(args.verify, args.revision)
        print(f"Verified browser artifact from {result['commit']}")
        return
    revision = args.revision or subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if not re.fullmatch(r'[0-9a-f]{40}', revision):
        raise ValueError('Expected a full source commit')
    source, output = args.source.resolve(), args.out.resolve()
    if source == output or output in source.parents or output == ROOT:
        raise ValueError('The output must be a separate staging directory')
    for name in REQUIRED:
        if not (source / name).is_file():
            raise ValueError(f'Build browser assets first: missing {name}')
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    for name in ('index.html', 'kernels.js', 'oidn.js'):
        shutil.copy2(source / name, output / name)
    shutil.copytree(source / 'pkg', output / 'pkg')
    manifest = {
        'commit': revision,
        'repository': os.environ.get('GITHUB_REPOSITORY', ''),
        'files': {p.relative_to(output).as_posix(): digest(p)
                  for p in sorted(output.rglob('*')) if p.is_file()},
    }
    (output / 'build-info.json').write_text(json.dumps(manifest, indent=2) + '\n')
    verify(output, revision)
    print(f"Staged {len(manifest['files'])} browser assets from {revision}")


if __name__ == '__main__':
    main()
