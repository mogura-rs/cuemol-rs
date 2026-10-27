#!/usr/bin/env python3
"""Install the Linux CI wasm-bindgen binary matching Cargo.lock."""
import hashlib
from pathlib import Path
import tarfile
import tomllib
import urllib.request

root = Path(__file__).resolve().parent.parent
lock = tomllib.loads((root / 'Cargo.lock').read_text())
version = next(p['version'] for p in lock['package'] if p['name'] == 'wasm-bindgen')
checksums = {'0.2.114': 'ce21be002bdbc22ede2a5cf250cb801d2106956fc3c529c75d9452729db3490c'}
if version not in checksums:
    raise SystemExit('Update the verified wasm-bindgen checksum for the new Cargo.lock version.')
destination = root / '.cache/web-tools/bin'
destination.mkdir(parents=True, exist_ok=True)
archive = destination.parent / f'wasm-bindgen-{version}.tar.gz'
if not archive.exists():
    urllib.request.urlretrieve(f'https://github.com/wasm-bindgen/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-unknown-linux-musl.tar.gz', archive)
with archive.open('rb') as data:
    if hashlib.file_digest(data, 'sha256').hexdigest() != checksums[version]:
        raise SystemExit('wasm-bindgen checksum mismatch')
with tarfile.open(archive) as contents:
    member = next(m for m in contents if m.name.endswith('/wasm-bindgen'))
    (destination / 'wasm-bindgen').write_bytes(contents.extractfile(member).read())
(destination / 'wasm-bindgen').chmod(0o755)
