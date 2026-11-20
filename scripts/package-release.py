#!/usr/bin/env python3
"""Package the native binary, browser assets and bundled scientific tools."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import zipfile
from distribution_info import inspect_binary

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--target', required=True)
args = parser.parse_args()
windows = 'windows' in args.target
binary = root / 'target' / args.target / 'release' / ('cuemol.exe' if windows else 'cuemol')
tools = root / '.cache/bundled-tools'
if not binary.is_file() or not (tools / 'SOURCES.json').is_file():
    raise SystemExit('Build the native application and tools bundle first.')
dist = root / 'dist'
dist.mkdir(exist_ok=True)
revision = os.environ.get('GITHUB_SHA') or subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
if subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() != revision:
    raise SystemExit('Checkout does not match the requested source commit.')
if subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=root):
    raise SystemExit('Commit source changes and rebuild before release packaging.')
info = inspect_binary(binary, args.target, revision)
package = root / '.cache/package' / args.target
shutil.rmtree(package, ignore_errors=True)
package.mkdir(parents=True)
shutil.copy2(binary, package / binary.name)
(package / 'build-info.json').write_text(json.dumps(info, indent=2) + '\n')
shutil.copytree(tools, package / 'cuemol-tools')
licenses = package / 'cuemol-tools/licenses'
for notice in (root / 'licenses').iterdir():
    shutil.copy2(notice, licenses / notice.name)
for name in ('LICENSE-MIT', 'LICENSE-APACHE', 'SOURCE.json'):
    shutil.copy2(root / 'vendor/tiny-http' / name, licenses / ('tiny-http-' + name))
if windows:
    output = dist / f'cuemol-{args.target}.zip'
    temporary = output.with_suffix('.tmp')
    with zipfile.ZipFile(temporary, 'w', zipfile.ZIP_DEFLATED, compresslevel=3) as archive:
        for path in sorted(package.rglob('*')):
            if path.is_file():
                archive.write(path, path.relative_to(package))
    temporary.replace(output)
else:
    output = dist / f'cuemol-{args.target}.tar.gz'
    temporary = output.with_suffix('.tmp')
    with tarfile.open(temporary, 'w:gz', compresslevel=3) as archive:
        for path in package.iterdir():
            archive.add(path, path.name)
    temporary.replace(output)
if platform.system() == 'Darwin':
    app = package / 'cuemol-rs.app'
    subprocess.run(['sh', str(root / 'scripts/package-macos-app.sh'), str(binary), str(app), str(package / 'cuemol-tools')], check=True)
    shutil.copy2(package / 'build-info.json', app / 'Contents/Resources/build-info.json')
    subprocess.run(['plutil', '-lint', str(app / 'Contents/Info.plist')], check=True)
    subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(app), str(dist / f'cuemol-rs-{args.target}.zip')], check=True)
