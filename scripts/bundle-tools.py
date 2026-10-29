#!/usr/bin/env python3
"""Build relocatable native tools with pinned inputs and preserved licenses."""
import argparse
import hashlib
import importlib.metadata
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / '.cache' / 'tools'
APBS_VERSION = '3.4.1'
FETK_REV = '57195e55351e04ce6ee0ef56a143c996a9aee7e2'
APBS_HASHES = {
    'Linux': '750f6a2df7b5a82b69be5c4cb192115c02fa261d0ff52cf1d9632ed1eda8b4f0',
    'Windows': '77dbccc31307b82f2009a179dd1890f7a05061596771df726a4d95d2958887f8',
}
SOURCES = []


def run(*args, **kwargs):
    subprocess.run([str(a) for a in args], check=True, **kwargs)


def fetch(url, name, digest):
    path = CACHE / name
    if not path.is_file():
        temporary = path.with_suffix(path.suffix + '.download')
        with urllib.request.urlopen(url) as response, temporary.open('wb') as output:
            shutil.copyfileobj(response, output)
        temporary.replace(path)
    actual = hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()
    if actual != digest:
        raise RuntimeError(f'Checksum mismatch: {name}: {actual}')
    SOURCES.append({'url': url, 'sha256': digest})
    return path


def unpack_tar(archive, destination):
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as contents:
        contents.extractall(destination, filter='data')


def apbs(output, force_source):
    source = fetch(f'https://codeload.github.com/Electrostatics/apbs/tar.gz/refs/tags/v{APBS_VERSION}',
                   'apbs-source.tar.gz', '644e6246fd37c9dd4172fc7be1763337082ab8584dbcd53e738c831b62c89bb2')
    root = CACHE / 'source'
    src = root / f'apbs-{APBS_VERSION}'
    if not src.exists():
        unpack_tar(source, root)
    fetk_archive = fetch(f'https://codeload.github.com/Electrostatics/FETK/tar.gz/{FETK_REV}',
                         'fetk-source.tar.gz', 'd9affc573b1f5410a7d2dd550a6d9a05a349d3c52e2c5615084e850ed684e6f6')
    with tarfile.open(fetk_archive) as contents:
        for module in ('maloc', 'punc', 'gamer', 'mc'):
            for name in ('LGPL.txt', 'license.h'):
                source_name = f'FETK-{FETK_REV}/{module}/doc/{name}'
                (output / 'licenses' / f'FETK-{module}-{name}').write_bytes(contents.extractfile(source_name).read())
    destination = output / 'apbs'
    system = platform.system()
    if not force_source and system != 'Darwin' and platform.machine().lower() in ('amd64', 'x86_64'):
        archive = fetch(f'https://github.com/Electrostatics/apbs/releases/download/v{APBS_VERSION}/APBS-{APBS_VERSION}.{system}.zip',
                        f'apbs-{system.lower()}.zip', APBS_HASHES[system])
        extracted = CACHE / f'apbs-{system.lower()}'
        with zipfile.ZipFile(archive) as contents:
            contents.extractall(extracted)
        package = extracted / f'APBS-{APBS_VERSION}.{system}'
        shutil.copytree(package, destination, dirs_exist_ok=True)
        if system == 'Windows':
            # The upstream executable and its DLLs are 32-bit and link Python 3.9.
            runtime = fetch('https://www.python.org/ftp/python/3.9.13/python-3.9.13-embed-win32.zip',
                            'python-3.9.13-embed-win32.zip',
                            'f8ed5e019d7bc6dba1d7dfa5d59052b5241c37e8eaa5293133c898ac7acedb98')
            with zipfile.ZipFile(runtime) as contents:
                contents.extractall(destination / 'bin')
                (output / 'licenses' / 'APBS-Python.txt').write_bytes(contents.read('LICENSE.txt'))
    else:
        archive = fetk_archive
        fetk = root / f'FETK-{FETK_REV}'
        if not fetk.exists():
            unpack_tar(archive, root)
        build = CACHE / 'apbs-build'
        extra = []
        if system == 'Linux':
            toolchain = CACHE / 'apbs-toolchain.cmake'
            toolchain.write_text(
                'set(CMAKE_C_STANDARD_LIBRARIES "-Wl,-Bstatic -lgomp -Wl,-Bdynamic -lm -lpthread")\n'
                'set(CMAKE_CXX_STANDARD_LIBRARIES "-Wl,-Bstatic -lgomp -Wl,-Bdynamic -lm -lpthread")\n')
            extra += ['-DBLA_VENDOR=OpenBLAS', f'-DCMAKE_TOOLCHAIN_FILE={toolchain}']
        elif system == 'Darwin':
            prefixes = [subprocess.check_output(['brew', '--prefix', name], text=True).strip()
                        for name in ('suite-sparse', 'openblas', 'gettext', 'libiconv', 'libomp')]
            extra.append('-DCMAKE_PREFIX_PATH=' + ';'.join(prefixes))
            omp = prefixes[-1]
            extra += [f'-DCMAKE_C_STANDARD_LIBRARIES=-L{omp}/lib -lomp', '-DCMAKE_C_FLAGS=-Wno-error=implicit-int -Wno-error=implicit-function-declaration -Wno-error=incompatible-function-pointer-types']
        run('cmake', '-S', src, '-B', build, *extra,
            '-DCMAKE_POLICY_VERSION_MINIMUM=3.5', '-DCMAKE_BUILD_TYPE=Release',
            '-DAPBS_STATIC_BUILD=ON', '-Denable_internal_blaslib=OFF', '-DBUILD_DOC=OFF', '-DBUILD_TOOLS=OFF',
            '-DENABLE_TESTS=OFF', '-DENABLE_PYTHON=OFF', '-DENABLE_PYGBE=OFF',
            '-DENABLE_BEM=OFF', '-DGET_NanoShaper=OFF', '-DENABLE_OPENMP=OFF',
            f'-DFETCHCONTENT_SOURCE_DIR_FETK={fetk}', f'-DCMAKE_INSTALL_PREFIX={destination}')
        run('cmake', '--build', build, '--parallel', str(min(os.cpu_count() or 2, 8)))
        run('cmake', '--install', build)
        if system == 'Darwin':
            spec = importlib.util.spec_from_file_location('bundle_povray', ROOT / 'scripts/bundle-povray.py')
            runtime = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(runtime)
            dependencies = runtime.copy_macos_runtime(destination / 'bin/apbs', destination)
            SOURCES.append({'tool': 'APBS', 'runtime': dependencies})

        for module in ('maloc', 'punc', 'gamer', 'mc'):
            for name in ('LGPL.txt', 'license.h'):
                shutil.copy2(fetk / module / 'doc' / name, output / 'licenses' / f'FETK-{module}-{name}')
        relink = destination / 'relink'
        relink.mkdir(exist_ok=True)
        shutil.copy2(build / 'src/CMakeFiles/apbs.dir/main.c.o', relink / 'main.c.o')
        shutil.copy2(build / 'src/CMakeFiles/apbs.dir/link.txt', relink / 'link.txt')

    for directory in ('include', 'share', 'lib/cmake'):
        shutil.rmtree(destination / directory, ignore_errors=True)
    executable = destination / 'bin' / ('apbs.exe' if os.name == 'nt' else 'apbs')
    executable.chmod(executable.stat().st_mode | 0o111)
    shutil.copy2(src / 'LICENSE.md', output / 'licenses' / 'APBS.txt')
    result = subprocess.run([str(executable), '--version'], capture_output=True, text=True, cwd=CACHE)
    if APBS_VERSION not in result.stdout + result.stderr:
        raise RuntimeError(f'APBS version check failed ({result.returncode}): {result.stdout}\n{result.stderr}')


def python_tools(output):
    environment = CACHE / 'python'
    python = environment / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')
    if not python.exists():
        run('uv', 'venv', '--python', '3.12', environment)
    run('uv', 'pip', 'sync', '--python', python, '--require-hashes', ROOT / 'scripts/tool-requirements.txt')
    entry = CACHE / 'pdb2pqr-entry.py'
    entry.write_text('from pdb2pqr.main import main\nif __name__ == "__main__":\n    main()\n')
    run(python, '-m', 'PyInstaller', '--noconfirm', '--clean', '--onedir', '--name', 'pdb2pqr',
        '--distpath', output, '--workpath', CACHE / 'pyinstaller', '--specpath', CACHE,
        '--collect-all', 'pdb2pqr', '--collect-all', 'propka',
        '--copy-metadata', 'pdb2pqr', '--copy-metadata', 'propka', entry)
    helper = r'''
import importlib.metadata,json,shutil,sys
from pathlib import Path
import imageio_ffmpeg
out=Path(sys.argv[1]); binary=Path(imageio_ffmpeg.get_ffmpeg_exe())
dest=out/'ffmpeg';dest.mkdir(exist_ok=True)
shutil.copy2(binary,dest/('ffmpeg.exe' if sys.platform=='win32' else 'ffmpeg'))
versions={}
for dist in importlib.metadata.distributions():
    name=dist.metadata['Name']; versions[name]=dist.version
    for file in dist.files or []:
        if any(token in file.name.lower() for token in ('license','copying','notice')):
            src=dist.locate_file(file)
            if src.is_file():
                target=out/'licenses'/name/file.name;target.parent.mkdir(parents=True,exist_ok=True)
                shutil.copy2(src,target)
(out/'python-packages.json').write_text(json.dumps(versions,indent=2)+'\n')
'''
    run(python, '-c', helper, output)
    pdb2pqr = output / 'pdb2pqr' / ('pdb2pqr.exe' if os.name == 'nt' else 'pdb2pqr')
    run(pdb2pqr, '--version')
    ffmpeg = output / 'ffmpeg' / ('ffmpeg.exe' if os.name == 'nt' else 'ffmpeg')
    ffmpeg.chmod(ffmpeg.stat().st_mode | 0o111)
    with (output / 'ffmpeg-build.txt').open('w') as log:
        run(ffmpeg, '-version', stdout=log, stderr=subprocess.STDOUT)
    SOURCES.append({'url':'https://github.com/imageio/imageio-ffmpeg/tree/v0.6.0/binaries',
                    'requirements':'scripts/tool-requirements.txt'})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / '.cache/bundled-tools')
    parser.add_argument('--apbs-source', action='store_true')
    parser.add_argument('--oidn-only', action='store_true')
    args = parser.parse_args()
    output = args.out.resolve()
    CACHE.mkdir(parents=True, exist_ok=True)
    (output / 'licenses').mkdir(parents=True, exist_ok=True)
    if args.oidn_only:
        run(sys.executable, ROOT / 'scripts/bundle-oidn.py', '--out', output / 'oidn')
        return
    apbs(output, args.apbs_source)
    python_tools(output)
    run(sys.executable, ROOT / "scripts/bundle-povray.py", "--out", output / "povray")
    run(sys.executable, ROOT / "scripts/bundle-oidn.py", "--out", output / "oidn")
    license_file = fetch('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n7.0.2/COPYING.GPLv3',
                         'ffmpeg-COPYING.GPLv3', '8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903')
    shutil.copy2(license_file, output / 'licenses/FFmpeg-GPL-3.0.txt')

    (output / 'SOURCES.json').write_text(json.dumps(SOURCES, indent=2) + '\n')
    (output / 'README.txt').write_text(
        'APBS, PDB2PQR, FFmpeg and POV-Ray are separate executables distributed with CueMol.\n'
        'The OIDN runtime and its source manifest are under oidn/.\n'
        'Their original licenses are in licenses/. Pinned sources and package versions\n'
        'are recorded in SOURCES.json and python-packages.json. FFmpeg is built and\n'
        'distributed by imageio-ffmpeg; its build configuration is in ffmpeg-build.txt.\n'
        'Umbreon and USalign are compiled into cuemol. POV-Ray sources and licenses are under povray/.\n')
    print(f'Bundled tools: {output}')


if __name__ == '__main__':
    main()
