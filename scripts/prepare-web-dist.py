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
    'pkg/source-build.json',
    'pkg/logo.png', 'index.html', 'kernels.js', 'oidn.js', 'input.js', 'loader.js', 'loader-worker.js', 'pkg/cuemol_web.js',
    'pkg/cuemol_web_bg.wasm', 'pkg/cuemol_web_loader.js', 'pkg/cuemol_web_loader_bg.wasm',
    'pkg/kernels/usalign.js', 'pkg/kernels/usalign.wasm',
    'pkg/kernels/lcms.js', 'pkg/kernels/lcms.wasm',
    'pkg/kernels/surface.js', 'pkg/kernels/surface.wasm',
    'pkg/oidn/oidn.js', 'pkg/oidn/rt_hdr_calb_cnrm.tza',
    'pkg/oidn/OIDN-WebGPU-LICENSE', 'pkg/oidn/OIDN-Weights-LICENSE',
    'pkg/oidn/SOURCES.json', 'pkg/kernels/BALL-LICENSE',
    'pkg/kernels/Boost-LICENSE', 'pkg/kernels/MeshMS-SOURCE.json',
    'pkg/kernels/BALL-SOURCE.json', 'pkg/kernels/LittleCMS-LICENSE',
    'pkg/kernels/USalign-LICENSE',
    'pkg/kernels/EDTSurf-LICENSE', 'pkg/kernels/EDTSurf-SOURCE.json',
    'pkg/licenses/molstar.txt', 'pkg/licenses/smaa.txt', 'pkg/licenses/xegtao.txt',
    'pkg/licenses/phosphor.txt',
    'pkg/fonts/NotoSansJP-Regular.ttf', 'pkg/fonts/OFL.txt', 'pkg/fonts/SOURCE.json',
)


def digest(path):
    with path.open('rb') as data:
        return hashlib.file_digest(data, 'sha256').hexdigest()


def verify(directory, revision=None):
    manifest = json.loads((directory / 'build-info.json').read_text())
    if revision and manifest['commit'] != revision:
        raise ValueError('Browser artifact does not match the requested commit')
    source = json.loads((directory / 'pkg/source-build.json').read_text())
    if source.get('commit') != manifest['commit'] or source.get('dirty') != manifest.get('dirty'):
        raise ValueError('Browser source metadata differs from the artifact manifest')
    if revision and source.get('dirty') is not False:
        raise ValueError('Release browser assets require clean sources')
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
    build = json.loads((source / 'pkg/source-build.json').read_text())
    if build.get('commit') != revision:
        raise ValueError('Browser build does not match the requested commit; rebuild first')
    if args.revision and build.get('dirty') is not False:
        raise ValueError('Release browser assets require clean sources')
    current_commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    current_dirty = bool(subprocess.check_output(
        ['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT))
    if current_commit != build['commit'] or current_dirty != build['dirty']:
        raise ValueError('Source checkout changed since the browser build; rebuild first')
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    for name in ('index.html', 'kernels.js', 'oidn.js', 'input.js', 'loader.js', 'loader-worker.js'):
        shutil.copy2(source / name, output / name)
    shutil.copytree(source / 'pkg', output / 'pkg')
    manifest = {
        'commit': revision,
        'dirty': build['dirty'],
        'repository': os.environ.get('GITHUB_REPOSITORY', ''),
        'files': {p.relative_to(output).as_posix(): digest(p)
                  for p in sorted(output.rglob('*')) if p.is_file()},
    }
    (output / 'build-info.json').write_text(json.dumps(manifest, indent=2) + '\n')
    verify(output, args.revision)
    print(f"Staged {len(manifest['files'])} browser assets from {revision}")


if __name__ == '__main__':
    main()
