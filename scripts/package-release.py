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
package = root / '.cache/package' / args.target
shutil.rmtree(package, ignore_errors=True)
package.mkdir(parents=True)
shutil.copy2(binary, package / binary.name)
revision = os.environ.get('GITHUB_SHA') or subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
version = subprocess.check_output([str(binary), '--version'], text=True).strip().removeprefix('cuemol ')
(package / 'build-info.json').write_text(json.dumps({'commit': revision, 'target': args.target, 'version': version}, indent=2) + '\n')
shutil.copytree(tools, package / 'cuemol-tools')
licenses = package / 'cuemol-tools/licenses'
shutil.copy2(root / 'crates/cuemol-molanl/vendor/usalign/LICENSE', licenses / 'USalign.txt')
shutil.copy2(root / 'crates/cuemol-gfx/vendor/lcms2/LICENSE', licenses / 'LittleCMS.txt')
for name in ('winit.txt', 'smaa.txt', 'molstar.txt', 'xegtao.txt'):
    shutil.copy2(root / 'licenses' / name, licenses / name)
for name in ('CueMol-LICENSE', 'Boost-LICENSE', 'BALL-SOURCE.json', 'meshms/SOURCE.json'):
    shutil.copy2(root / 'crates/cuemol-surface/vendor' / name, licenses / ('MeshMS-SOURCE.json' if name.startswith('meshms/') else name))
shutil.copy2(root / 'crates/cuemol-surface/vendor/edtsurf/LICENSE', licenses / 'EDTSurf.txt')
shutil.copy2(root / 'crates/cuemol-surface/vendor/edtsurf/SOURCE.json', licenses / 'EDTSurf-SOURCE.json')
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
    app = package / 'CueMol.app'
    subprocess.run(['sh', str(root / 'scripts/package-macos-app.sh'), str(binary), str(app), str(package / 'cuemol-tools')], check=True)
    shutil.copy2(package / 'build-info.json', app / 'Contents/Resources/build-info.json')
    subprocess.run(['plutil', '-lint', str(app / 'Contents/Info.plist')], check=True)
    subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(app), str(dist / f'CueMol-{args.target}.zip')], check=True)
