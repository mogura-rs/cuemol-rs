#!/usr/bin/env python3
"""Build and package the upstream POV-Ray console renderer and its runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / '.cache/povray'
VERSION = '3.8.0-beta.2'
URL = f'https://codeload.github.com/POV-Ray/povray/tar.gz/refs/tags/v{VERSION}'
SHA256 = '5d275daf159e3cf5ad655d11b53956ea0087687169fc81cfc81fa12166f30344'
JOBS = str(min(os.cpu_count() or 2, 8))


def run(*args, **kwargs):
    subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def output(*args, **kwargs):
    return subprocess.check_output([str(arg) for arg in args], text=True, **kwargs)


def source():
    CACHE.mkdir(parents=True, exist_ok=True)
    archive = CACHE / f'povray-v{VERSION}.tar.gz'
    if not archive.is_file():
        temporary = archive.with_suffix('.download')
        urllib.request.urlretrieve(URL, temporary)
        temporary.replace(archive)
    with archive.open('rb') as data:
        if hashlib.file_digest(data, 'sha256').hexdigest() != SHA256:
            raise RuntimeError('POV-Ray source checksum mismatch')
    root = CACHE / f'povray-{VERSION}'
    if not root.is_dir():
        with tarfile.open(archive) as data:
            data.extractall(CACHE, filter='data')
    return root, archive


def unix_build(root):
    if not (root / 'configure').is_file():
        run('sh', './prebuild.sh', cwd=root / 'unix')
    build = CACHE / 'build'
    build.mkdir(exist_ok=True)
    env = os.environ.copy()
    env['CXXFLAGS'] = env.get('CXXFLAGS', '-O2 -std=c++14') + ' -fno-fast-math'
    if platform.system() == 'Darwin':
        truetype = root / 'source/core/shape/truetype.cpp'
        truetype.write_text(truetype.read_text().replace(
            '#if !defined(TARGET_OS_MAC)\ntypedef int Fixed;\n#endif',
            'typedef POV_INT32 Fixed;'))
        supplied = env.get('CUEMOL_POVRAY_PREFIXES')
        prefixes = supplied.split(os.pathsep) if supplied else [
            output('brew', '--prefix', name).strip()
            for name in ('boost', 'libpng', 'jpeg-turbo', 'libtiff')]
        env['CPPFLAGS'] = ' '.join(f'-I{p}/include' for p in prefixes) + ' ' + env.get('CPPFLAGS', '')
        env['LDFLAGS'] = ' '.join(f'-L{p}/lib' for p in prefixes) + ' ' + env.get('LDFLAGS', '')
    run(root / 'configure', '--disable-optimiz-arch', '--without-x', '--without-libsdl',
        '--without-openexr', '--with-boost-thread=boost_thread',
        'COMPILED_BY=CueMol contributors', '--prefix=/usr', cwd=build, env=env)
    run('make', '-j', JOBS, cwd=build, env=env)
    return build / 'unix/povray'


def windows_build(root):
    attribute = root / 'libraries/openexr/IlmImf/ImfAttribute.cpp'
    attribute.write_text(attribute.read_text().replace(
        'struct NameCompare: std::binary_function <const char *, const char *, bool>',
        'struct NameCompare'))
    config = root / 'windows/povconfig/syspovconfig.h'
    config.write_text(config.read_text().replace('// #define _CONSOLE', '#define _CONSOLE'))
    build = root / 'source/base/build.h'
    text = build.read_text().replace('YOUR NAME (YOUR EMAIL)', 'CueMol contributors')
    build.write_text(text.replace('#error "Please fill in BUILT_BY, then remove this line"', ''))
    for project in (root / 'windows/vs2015').glob('*.vcxproj'):
        project.write_text(project.read_text(encoding='utf-8-sig').replace(
            '<FloatingPointModel>Fast</FloatingPointModel>',
            '<FloatingPointModel>Precise</FloatingPointModel>'), encoding='utf-8')
    locator = Path(os.environ['ProgramFiles(x86)']) / 'Microsoft Visual Studio/Installer/vswhere.exe'
    msbuild = output(locator, '-latest', '-products', '*', '-requires',
                     'Microsoft.Component.MSBuild', '-find', 'MSBuild/**/Bin/MSBuild.exe').splitlines()[0]
    run(msbuild, root / 'windows/vs2015/console.vcxproj', f'/m:{JOBS}',
        '/p:Configuration=Release', '/p:Platform=x64', '/p:PlatformToolset=v143',
        '/p:WindowsTargetPlatformVersion=10.0', '/p:BuildProjectReferences=true')
    return root / 'windows/vs2015/bin64/povconsole64.exe'


def copy_linux_runtime(binary, destination):
    libraries = destination / 'lib'
    libraries.mkdir(exist_ok=True)
    baseline = re.compile(r'^(?:ld-linux|lib(?:c|m|dl|rt|pthread|resolv|stdc\+\+|gcc_s)\.so)')
    pending, copied = [binary], {}
    while pending:
        file = pending.pop()
        text = output('ldd', file)
        if 'not found' in text:
            raise RuntimeError(f'Unresolved POV-Ray runtime dependency: {text}')
        for line in text.splitlines():
            match = re.match(r'\s*(\S+) => (/\S+)', line)
            if not match or baseline.match(match[1]) or match[1] in copied:
                continue
            name, path = match.groups()
            original = Path(path).resolve()
            copied[name] = original
            target = libraries / name
            shutil.copy2(original, target)
            pending.append(original)
    run('patchelf', '--set-rpath', '$ORIGIN/../lib', binary)
    for name in copied:
        run('patchelf', '--set-rpath', '$ORIGIN', libraries / name)
    records = []
    licenses = destination / 'licenses/runtime'
    licenses.mkdir(parents=True, exist_ok=True)
    for name, path in copied.items():
        query = subprocess.run(['dpkg-query', '-S', str(path)], capture_output=True, text=True)
        package = query.stdout.split(': ')[0].splitlines()[0] if query.returncode == 0 else ''
        record = {'library': name}
        if package:
            record['package'] = package
            record['version'] = output('dpkg-query', '-W', '-f=${Version}', package)
            copyright_file = Path('/usr/share/doc') / package.split(':')[0] / 'copyright'
            if copyright_file.is_file():
                shutil.copy2(copyright_file, licenses / f'{package.replace(":", "-")}.txt')
        records.append(record)
    extra = os.environ.get('CUEMOL_POVRAY_DEPENDENCY_DOCS')
    if extra:
        for copyright_file in Path(extra).glob('*/copyright'):
            shutil.copy2(copyright_file, licenses / f'{copyright_file.parent.name}.txt')
    return records


def copy_macos_runtime(binary, destination):
    libraries = destination / 'lib'
    libraries.mkdir(exist_ok=True)
    pending, copied, records = [(binary, binary)], {}, []
    while pending:
        original, target = pending.pop()
        identities = output('otool', '-D', original).splitlines()[1:]
        load_commands = output('otool', '-l', original)
        rpaths = re.findall(r'cmd LC_RPATH\s+cmdsize \d+\s+path (.+?) \(offset', load_commands)
        for line in output('otool', '-L', original).splitlines()[1:]:
            dependency = line.strip().split(' (')[0]
            if dependency.startswith(('/usr/lib/', '/System/Library/')) or dependency in identities:
                continue
            expanded = dependency.replace('@loader_path', str(original.parent)).replace('@executable_path', str(binary.parent))
            path = Path(expanded)
            if expanded.startswith('@rpath/'):
                candidates = [Path(prefix.replace('@loader_path', str(original.parent))) / path.name for prefix in rpaths]
                candidates.extend(Path(prefix) / path.name for prefix in os.environ.get('DYLD_LIBRARY_PATH', '').split(os.pathsep) if prefix)
                path = next((candidate for candidate in candidates if candidate.is_file()), path)
            if not path.is_absolute() or not path.is_file():
                raise RuntimeError(f'Unresolved POV-Ray dylib: {dependency}')
            name = path.name
            if name not in copied:
                copied[name] = path.resolve()
                shutil.copy2(path, libraries / name)
                pending.append((path, libraries / name))
                prefix = next((parent for parent in path.resolve().parents if (parent / 'INSTALL_RECEIPT.json').is_file()), None)
                if prefix:
                    package, version = prefix.parent.name, prefix.name
                    records.append({'library': name, 'package': package, 'version': version})
                    for source in prefix.iterdir():
                        if source.is_file() and any(token in source.name.lower() for token in ('license', 'copying', 'copyright')):
                            license_dir = destination / 'licenses/runtime' / package
                            license_dir.mkdir(parents=True, exist_ok=True)
                            shutil.copy2(source, license_dir / source.name)
            relative = '@loader_path/../lib/' if target == binary else '@loader_path/'
            run('install_name_tool', '-change', dependency, relative + name, target)
        if target != binary:
            run('install_name_tool', '-id', '@rpath/' + target.name, target)
    for path in [*(libraries / name for name in copied), binary]:
        run('codesign', '--force', '--sign', '-', path)
    return records


def smoke(binary, include):
    with tempfile.TemporaryDirectory(prefix='cuemol-povray-') as directory:
        work = Path(directory)
        (work / 'sphere.pov').write_text(
            '#version 3.7;\n#include "colors.inc"\nglobal_settings { assumed_gamma 1 }\n'
            'camera { location <0,0,-4> look_at 0 }\n'
            'light_source { <-3,4,-4> color White }\n'
            'sphere { 0,1 pigment { color Red } }\n')
        run(binary, '+Isphere.pov', '+Osphere.png', '+W32', '+H32', '+FN8', '-D',
            '+WT1', f'+L"{include}"', cwd=work)
        if (work / 'sphere.png').read_bytes()[:24] != b'\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0 \0\0\0 ':
            raise RuntimeError('POV-Ray did not produce a 32 by 32 PNG')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / '.cache/bundled-tools/povray')
    args = parser.parse_args()
    destination = args.out.resolve()
    (destination / 'bin').mkdir(parents=True, exist_ok=True)
    (destination / 'licenses').mkdir(exist_ok=True)
    root, archive = source()
    system = platform.system()
    built = windows_build(root) if system == 'Windows' else unix_build(root)
    binary = destination / 'bin' / ('povray.exe' if system == 'Windows' else 'povray')
    shutil.copy2(built, binary)
    shutil.copytree(root / 'distribution/include', destination / 'include', dirs_exist_ok=True)
    shutil.copy2(root / 'LICENSE', destination / 'licenses/POV-Ray-AGPL-3.0.txt')
    for path in (root / 'libraries').rglob('*'):
        if path.is_file() and (path.name.lower().startswith(('license', 'copying')) or path.name == 'README'):
            target = destination / 'licenses/upstream' / path.relative_to(root / 'libraries')
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
    (destination / 'sources').mkdir(exist_ok=True)
    shutil.copy2(archive, destination / 'sources' / archive.name)
    shutil.copy2(Path(__file__), destination / 'sources/build.py')
    dependencies = copy_linux_runtime(binary, destination) if system == 'Linux' else (
        copy_macos_runtime(binary, destination) if system == 'Darwin' else [])
    (destination / 'cuemol.ini').write_text('; Library paths are supplied by cuemol.\n')
    (destination / 'SOURCES.json').write_text(json.dumps(
        {'version': VERSION, 'url': URL, 'sha256': SHA256, 'dependencies': dependencies,
         'build': 'sources/build.py', 'openexr': system == 'Windows'}, indent=2) + '\n')
    smoke(binary, destination / 'include')
    print(f'Bundled POV-Ray: {destination}')


if __name__ == '__main__':
    main()
