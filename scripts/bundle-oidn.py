#!/usr/bin/env python3
"""Bundle verified native OIDN runtimes or browser kernels and model weights."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / '.cache/oidn-packages'
VERSION = '2.5.0'
WEB_REV = 'aee3d609e1d45eb14da7c3010bb9aa4bd9988d6c'
HASHES = {
    'x86_64.linux': '254a8355856f8cd433edb7dd832b5fb02e7157d073f7d22e331ce06533fb491f',
    'arm64.macos': '586142ec125de0bf5b01d3cc4c76985d4fafb0fc91e9f6562e32f3b669f86be5',
    'x86_64.macos': 'afa810e4a184df145659a0ab140c1fd126897a529819f14e083e9c2de191ac31',
    'x64.windows': '6ae0474ef7606d68647c1e2c2842832d6af01128ee84a523d30368a91013b707',
    'src': '96c3a46122759803d5f6701ffba4bef6eac0981dced5279e66f2815e3ed3c2cc',
}
SOURCES = []


def fetch(url, name, digest):
    CACHE.mkdir(parents=True, exist_ok=True)
    path = CACHE / name
    if not path.is_file():
        temporary = path.with_suffix(path.suffix + '.download')
        with urllib.request.urlopen(url) as response, temporary.open('wb') as output:
            shutil.copyfileobj(response, output)
        temporary.replace(path)
    with path.open('rb') as data:
        if hashlib.file_digest(data, 'sha256').hexdigest() != digest:
            raise RuntimeError(f'Checksum mismatch: {name}')
    SOURCES.append({'url': url, 'sha256': digest})
    return path


def unpack(archive, destination):
    destination.mkdir(parents=True, exist_ok=True)
    if archive.suffix == '.zip':
        with zipfile.ZipFile(archive) as contents:
            contents.extractall(destination)
    else:
        with tarfile.open(archive) as contents:
            contents.extractall(destination, filter='data')


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def arm_linux(output):
    archive = fetch(f'https://github.com/RenderKit/oidn/releases/download/v{VERSION}/oidn-{VERSION}.src.tar.gz',
                    f'oidn-{VERSION}.src.tar.gz', HASHES['src'])
    source = CACHE / 'source'
    unpack(archive, source)
    root = source / f'oidn-{VERSION}'
    compiler = fetch('https://github.com/ispc/ispc/releases/download/v1.28.2/ispc-v1.28.2-linux.aarch64.tar.gz',
                     'ispc-1.28.2-arm64.tar.gz', 'c42267566b8c17a2a00668e168f56087b41e55cf1cea047dd0631cf512d011f7')
    unpack(compiler, CACHE / 'ispc')
    ispc = next((CACHE / 'ispc').glob('*/bin/ispc'))
    tbb = fetch('https://codeload.github.com/uxlfoundation/oneTBB/tar.gz/refs/tags/v2022.3.0',
                'onetbb-2022.3.0.tar.gz', '01598a46c1162c27253a0de0236f520fd8ee8166e9ebb84a4243574f88e6e50a')
    unpack(tbb, source)
    tbb_source = source / 'oneTBB-2022.3.0'
    tbb_install = CACHE / 'tbb-install'
    jobs = str(min(os.cpu_count() or 2, 8))
    run('cmake', '-S', tbb_source, '-B', CACHE / 'tbb-build', '-DCMAKE_BUILD_TYPE=Release', '-DCMAKE_INSTALL_LIBDIR=lib',
        '-DTBB_TEST=OFF', '-DTBB_STRICT=OFF', '-DTBBMALLOC_BUILD=OFF', f'-DCMAKE_INSTALL_PREFIX={tbb_install}')
    run('cmake', '--build', CACHE / 'tbb-build', '--parallel', jobs)
    run('cmake', '--install', CACHE / 'tbb-build')
    run('cmake', '-S', root, '-B', CACHE / 'native-build', '-DCMAKE_BUILD_TYPE=Release', '-DCMAKE_INSTALL_LIBDIR=lib',
        '-DOIDN_DEVICE_CPU=ON', '-DOIDN_DEVICE_CUDA=OFF', '-DOIDN_DEVICE_HIP=OFF', '-DOIDN_DEVICE_SYCL=OFF',
        '-DOIDN_FILTER_RTLIGHTMAP=OFF', '-DOIDN_APPS=OFF', '-DOIDN_INSTALL_DEPENDENCIES=ON',
        f'-DISPC_EXECUTABLE={ispc}', f'-DTBB_ROOT={tbb_install}', f'-DCMAKE_INSTALL_PREFIX={output}')
    run('cmake', '--build', CACHE / 'native-build', '--parallel', jobs)
    run('cmake', '--install', CACHE / 'native-build')
    (output / 'licenses').mkdir(exist_ok=True)
    for file in root.glob('*.txt'):
        if any(word in file.name.lower() for word in ('license', 'notice', 'third-party')):
            shutil.copy2(file, output / 'licenses' / file.name)
    shutil.copy2(tbb_source / 'LICENSE.txt', output / 'licenses/oneTBB-LICENSE.txt')


def native(output):
    system = platform.system()
    arm = platform.machine().lower() in ('aarch64', 'arm64')
    if system == 'Linux' and arm:
        arm_linux(output)
        return
    target = {'Linux': 'x86_64.linux', 'Darwin': 'arm64.macos' if arm else 'x86_64.macos',
              'Windows': 'x64.windows'}[system]
    suffix = 'zip' if system == 'Windows' else 'tar.gz'
    name = f'oidn-{VERSION}.{target}'
    archive = fetch(f'https://github.com/RenderKit/oidn/releases/download/v{VERSION}/{name}.{suffix}',
                    f'{name}.{suffix}', HASHES[target])
    extracted = CACHE / 'native'
    unpack(archive, extracted)
    root = extracted / name
    directory = 'bin' if system == 'Windows' else 'lib'
    destination = output / directory
    destination.mkdir(parents=True, exist_ok=True)
    for file in (root / directory).iterdir():
        if not file.is_file():
            continue
        lower = file.name.lower()
        if ('openimagedenoise' in lower and not any(kind in lower for kind in ('device_cuda', 'device_hip', 'device_sycl', 'device_metal'))
                or lower.startswith(('libtbb', 'tbb12', 'tbbbind'))):
            if file.suffix not in ('.a', '.lib', '.exe', '.pdb'):
                shutil.copy2(file, destination / file.name)
    shutil.copytree(root / 'doc', output / 'licenses', dirs_exist_ok=True)


def web(output):
    package = fetch('https://registry.npmjs.org/oidn-webgpu/-/oidn-webgpu-0.6.0.tgz',
                    'oidn-webgpu-0.6.0.tgz', 'ce262579ab7e604fce9420d9f165b6d1d18c548c4149c588c73eb724bf4daf0a')
    with tarfile.open(package) as contents:
        for source, target in [('package/dist/oidn.js', 'oidn.js'), ('package/LICENSE', 'OIDN-WebGPU-LICENSE')]:
            (output / target).write_bytes(contents.extractfile(source).read())
    base = f'https://raw.githubusercontent.com/pissang/oidn-webgpu/{WEB_REV}/weights/'
    for name, digest, target in [
        ('rt_hdr_calb_cnrm.tza', 'd0057057bd76c64e3e67b449728ef8f4fd2f11ccfb74c1e59b9d50dafd50ee0e', 'rt_hdr_calb_cnrm.tza'),
        ('LICENSE.txt', 'cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30', 'OIDN-Weights-LICENSE'),
    ]:
        shutil.copy2(fetch(base + name, name, digest), output / target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--web', action='store_true')
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    output = (args.out or ROOT / ( 'web/pkg/oidn' if args.web else '.cache/bundled-tools/oidn')).resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.web:
        web(output)
    else:
        native(output)
    (output / 'SOURCES.json').write_text(json.dumps(SOURCES, indent=2) + '\n')
    print(f'Bundled OIDN: {output}')


if __name__ == '__main__':
    main()
