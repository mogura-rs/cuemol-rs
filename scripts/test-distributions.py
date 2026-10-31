#!/usr/bin/env python3
"""Exercise portable artifact staging, integrity failures and the release matrix."""
import importlib.util
import hashlib
import io
import json
import os
import shutil
import subprocess
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile
from distribution_info import validate


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


web_dist = load('prepare-web-dist')
release = load('prepare-release')
COMMIT = '1' * 40


class Distributions(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.web = self.root / 'web'
        self.web.mkdir()
        files = {}
        for name in web_dist.REQUIRED:
            path = self.web / name
            path.parent.mkdir(parents=True, exist_ok=True)
            if name == 'pkg/source-build.json':
                path.write_text(json.dumps({'commit': COMMIT, 'dirty': False}))
            else:
                path.write_bytes(b'\0asm\1\0\0\0' if path.suffix == '.wasm' else b'fixture')
            files[name] = web_dist.digest(path)
        (self.web / 'build-info.json').write_text(json.dumps({'commit': COMMIT, 'dirty': False, 'files': files}))

    def native_info(self, target):
        return {'commit': COMMIT, 'target': target, 'dirty': False,
                'features': ['gui', 'bundled-web', 'scripting'],
                'web_commit': COMMIT, 'web_dirty': False,
                'web_manifest_sha256': web_dist.digest(self.web / 'build-info.json'),
                'binary_sha256': hashlib.sha256(b'binary').hexdigest()}

    def matrix(self):
        directory = self.root / 'dist'
        directory.mkdir()
        for target in release.TARGETS:
            windows = 'windows' in target
            entries = {
                'cuemol.exe' if windows else 'cuemol': b'binary',
                'cuemol-tools/SOURCES.json': b'[]',
                'build-info.json': json.dumps(self.native_info(target)).encode(),
            }
            if windows:
                with zipfile.ZipFile(directory / f'cuemol-{target}.zip', 'w') as archive:
                    for name, value in entries.items():
                        archive.writestr(name, value)
            else:
                with tarfile.open(directory / f'cuemol-{target}.tar.gz', 'w:gz') as archive:
                    for name, value in entries.items():
                        member = tarfile.TarInfo(name)
                        member.size = len(value)
                        archive.addfile(member, io.BytesIO(value))
            if 'apple' in target:
                with zipfile.ZipFile(directory / f'cuemol-rs-{target}.zip', 'w') as archive:
                    archive.writestr('cuemol-rs.app/Contents/MacOS/cuemol-rs', b'binary')
                    archive.writestr('cuemol-rs.app/Contents/Resources/build-info.json', entries['build-info.json'])
        return directory

    def test_artifact_hashes_and_source_are_enforced(self):
        self.assertEqual(web_dist.verify(self.web, COMMIT)['commit'], COMMIT)
        with self.assertRaises(ValueError):
            web_dist.verify(self.web, '2' * 40)
        (self.web / 'index.html').write_bytes(b'corrupted')
        with self.assertRaises(ValueError):
            web_dist.verify(self.web, COMMIT)

    def test_unexpected_files_are_rejected(self):
        (self.web / 'local-settings.json').write_text('{}')
        with self.assertRaises(ValueError):
            web_dist.verify(self.web, COMMIT)

    def test_native_build_metadata_rejects_incomplete_or_stale_binaries(self):
        target = release.TARGETS[0]
        for change in [
            {'commit': '2' * 40}, {'dirty': True}, {'dirty': None},
            {'features': ['scripting']}, {'web_dirty': True},
            {'web_commit': '2' * 40}, {'web_manifest_sha256': None},
            {'web_manifest_sha256': '2' * 64},
        ]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate(self.native_info(target) | change, target, COMMIT,
                         web_dist.digest(self.web / 'build-info.json'))

    def test_staging_cannot_relabel_an_old_browser_build(self):
        output = self.root / 'staged'
        result = subprocess.run(['uv', 'run', '--no-project', 'python',
                                 str(Path(web_dist.__file__)), '--source', str(self.web),
                                 '--out', str(output), '--revision', '2' * 40],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('rebuild first', result.stderr)
        self.assertFalse(output.exists())

    def test_python_package_binary_cannot_replace_the_release_build(self):
        target = release.TARGETS[0]
        info = self.native_info(target) | {
            'features': ['gui', 'scripting'], 'web_commit': None,
            'web_dirty': None, 'web_manifest_sha256': None,
        }
        with self.assertRaisesRegex(ValueError, 'lacks application features'):
            validate(info, target, COMMIT)

    def test_dirty_browser_cannot_be_released(self):
        source = self.web / 'pkg/source-build.json'
        source.write_text(json.dumps({'commit': COMMIT, 'dirty': True}))
        manifest = json.loads((self.web / 'build-info.json').read_text())
        manifest['dirty'] = True
        manifest['files']['pkg/source-build.json'] = web_dist.digest(source)
        (self.web / 'build-info.json').write_text(json.dumps(manifest))
        with self.assertRaises(ValueError):
            web_dist.verify(self.web, COMMIT)
        self.assertTrue(web_dist.verify(self.web)['dirty'])

    def test_corrupted_native_binary_blocks_release(self):
        directory = self.matrix()
        target = 'x86_64-pc-windows-msvc'
        with zipfile.ZipFile(directory / f'cuemol-{target}.zip', 'w') as archive:
            archive.writestr('cuemol.exe', b'different-binary')
            archive.writestr('cuemol-tools/SOURCES.json', b'[]')
            archive.writestr('build-info.json', json.dumps(self.native_info(target)))
        with self.assertRaisesRegex(ValueError, 'different native binary'):
            release.prepare(directory, self.web, COMMIT)
        self.assertFalse((directory / 'SHA256SUMS').exists())

    def test_changed_app_binary_with_matching_metadata_blocks_release(self):
        directory = self.matrix()
        target = 'aarch64-apple-darwin'
        with zipfile.ZipFile(directory / f'cuemol-rs-{target}.zip', 'w') as archive:
            archive.writestr('cuemol-rs.app/Contents/MacOS/cuemol-rs', b'different-binary')
            archive.writestr('cuemol-rs.app/Contents/Resources/build-info.json',
                            json.dumps(self.native_info(target)))
        with self.assertRaisesRegex(ValueError, 'different application binary'):
            release.prepare(directory, self.web, COMMIT)
        self.assertFalse((directory / 'SHA256SUMS').exists())

    def test_complete_matrix_has_eight_checksummed_archives(self):
        directory = self.matrix()
        release.prepare(directory, self.web, COMMIT)
        lines = (directory / 'SHA256SUMS').read_text().splitlines()
        self.assertEqual(len(lines), 8)
        for line in lines:
            digest, name = line.split('  ')
            self.assertEqual(web_dist.digest(directory / name), digest)
        with zipfile.ZipFile(directory / 'cuemol-web.zip') as archive:
            self.assertEqual(json.loads(archive.read('build-info.json'))['commit'], COMMIT)

    def test_mismatched_application_blocks_release(self):
        directory = self.matrix()
        path = directory / 'cuemol-rs-aarch64-apple-darwin.zip'
        with zipfile.ZipFile(path, 'w') as archive:
            archive.writestr('cuemol-rs.app/Contents/MacOS/cuemol-rs', b'binary')
            archive.writestr('cuemol-rs.app/Contents/Resources/build-info.json',
                            json.dumps({'commit': '2' * 40, 'target': 'aarch64-apple-darwin'}))
        with self.assertRaises(ValueError):
            release.prepare(directory, self.web, COMMIT)
        self.assertFalse((directory / 'SHA256SUMS').exists())

    def test_missing_platform_blocks_release(self):
        directory = self.matrix()
        (directory / 'cuemol-x86_64-pc-windows-msvc.zip').unlink()
        with self.assertRaises(FileNotFoundError):
            release.prepare(directory, self.web, COMMIT)
        self.assertFalse((directory / 'SHA256SUMS').exists())

    @unittest.skipUnless(shutil.which('sha256sum'), 'Linux publishing helper')
    def test_draft_publish_and_published_version_guard(self):
        directory = self.matrix()
        release.prepare(directory, self.web, COMMIT)
        mock = self.root / 'commands'
        mock.mkdir()
        gh = mock / 'gh'
        gh.write_text("""#!/bin/sh
printf '%s\n' "$*" >> "$MOCK_LOG"
case "$*" in
  *'--json isDraft'*) printf '%s\n' "$MOCK_DRAFT" ;;
  *'--json assets'*) for file in ./*; do basename "$file"; done ;;
  'release download '*) while [ "$1" != '--dir' ]; do shift; done; cp SHA256SUMS "$2/SHA256SUMS" ;;
esac
""")
        gh.chmod(0o755)
        log = self.root / 'calls'
        env = {'PATH': str(mock) + os.pathsep + os.environ['PATH'],
               'RELEASE_TAG': 'v1.2.3', 'GH_REPO': 'fixture/repository',
               'MOCK_DRAFT': 'true', 'MOCK_LOG': str(log)}
        script = Path(__file__).with_name('publish-release.sh').resolve()
        result = subprocess.run(['bash', str(script), str(directory)], env=env, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertIn('release edit v1.2.3 --draft=false', log.read_text())
        log.write_text('')
        env['MOCK_DRAFT'] = 'false'
        result = subprocess.run(['bash', str(script), str(directory)], env=env, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('release upload', log.read_text())
        self.assertNotIn('release edit', log.read_text())


if __name__ == '__main__':
    unittest.main()
