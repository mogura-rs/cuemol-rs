#!/usr/bin/env python3
"""Validate the release matrix and create the web archive and checksums."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import tarfile
import zipfile

TARGETS = ('x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu',
           'aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc')
spec = importlib.util.spec_from_file_location('web_dist', Path(__file__).with_name('prepare-web-dist.py'))
web_dist = importlib.util.module_from_spec(spec)
spec.loader.exec_module(web_dist)


def prepare(directory, web, revision):
    web_dist.verify(web, revision)
    expected = []
    for target in TARGETS:
        windows = 'windows' in target
        name = f'cuemol-{target}.' + ('zip' if windows else 'tar.gz')
        path = directory / name
        if windows:
            with zipfile.ZipFile(path) as archive:
                names = set(archive.namelist())
                info = json.loads(archive.read('build-info.json'))
        else:
            with tarfile.open(path) as archive:
                names = set(archive.getnames())
                info = json.load(archive.extractfile('build-info.json'))
        for required in ('cuemol.exe' if windows else 'cuemol', 'cuemol-tools/SOURCES.json', 'build-info.json'):
            if required not in names:
                raise ValueError(f'{name} lacks {required}')
        if info['commit'] != revision or info['target'] != target:
            raise ValueError(f'{name} was built from another source or target')
        expected.append(name)
        if 'apple' in target:
            name = f'cuemol-rs-{target}.zip'
            with zipfile.ZipFile(directory / name) as archive:
                if 'cuemol-rs.app/Contents/MacOS/cuemol-rs' not in archive.namelist():
                    raise ValueError(f'{name} lacks the application executable')
                app_info = json.loads(archive.read('cuemol-rs.app/Contents/Resources/build-info.json'))
                if app_info != info:
                    raise ValueError(f'{name} does not match its command-line archive')
            expected.append(name)
    name = 'cuemol-web.zip'
    temporary = directory / (name + '.tmp')
    with zipfile.ZipFile(temporary, 'w', zipfile.ZIP_DEFLATED, compresslevel=3) as archive:
        for path in sorted(web.rglob('*')):
            if path.is_file():
                archive.write(path, path.relative_to(web))
    temporary.replace(directory / name)
    expected.append(name)
    actual = {p.name for p in directory.iterdir() if p.is_file() and p.name != 'SHA256SUMS'}
    if actual != set(expected):
        raise ValueError(f'Unexpected release assets: {sorted(actual ^ set(expected))}')
    lines = []
    for name in sorted(expected):
        with (directory / name).open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        lines.append(f'{digest}  {name}\n')
    (directory / 'SHA256SUMS').write_text(''.join(lines))
    print(f'Prepared {len(expected)} release assets from {revision}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dist', type=Path, default=Path('dist'))
    parser.add_argument('--web', type=Path, default=Path('web'))
    parser.add_argument('--revision', required=True)
    args = parser.parse_args()
    prepare(args.dist, args.web, args.revision)


if __name__ == '__main__':
    main()
