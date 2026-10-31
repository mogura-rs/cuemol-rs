"""Validate native build metadata before packaging or publishing."""
import hashlib
import json
import re
import subprocess


def validate(info, target, revision, web_sha256=None):
    if not re.fullmatch(r'[0-9a-f]{40}', revision):
        raise ValueError('Expected a full source commit')
    if info.get('commit') != revision or info.get('target') != target:
        raise ValueError('Native binary was built from another source or target')
    if info.get('dirty') is not False:
        raise ValueError('Release binaries require clean native sources')
    if not {'gui', 'bundled-web', 'scripting'}.issubset(info.get('features', [])):
        raise ValueError('Release binary lacks application features')
    if info.get('web_dirty') is not False:
        raise ValueError('Release binaries require clean browser sources')
    if info.get('web_commit') != revision:
        raise ValueError('Embedded browser was built from another source')
    digest = info.get('web_manifest_sha256', '')
    if not isinstance(digest, str) or not re.fullmatch(r'[0-9a-f]{64}', digest):
        raise ValueError('Embedded browser manifest checksum is missing')
    if web_sha256 is not None and digest != web_sha256:
        raise ValueError('Embedded browser differs from the release browser artifact')


def inspect_binary(binary, target, revision):
    result = subprocess.run([str(binary), '--build-info'], check=True,
                            capture_output=True, text=True, timeout=30)
    info = json.loads(result.stdout)
    validate(info, target, revision)
    with binary.open('rb') as stream:
        info['binary_sha256'] = hashlib.file_digest(stream, 'sha256').hexdigest()
    return info
