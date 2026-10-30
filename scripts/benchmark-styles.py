#!/usr/bin/env python3
"""Measure retained style snapshots in fresh processes and verify their contents."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--colors', type=int, default=20000)
    parser.add_argument('--snapshots', type=int, default=64)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if min(args.colors, args.snapshots, args.samples) < 1:
        parser.error('all counts must be positive')
    if not hasattr(os, 'wait4'):
        parser.error('POSIX wait4 is required for per-process peak RSS')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw = args.output.parent / f'{args.output.stem}-raw'
    raw.mkdir(exist_ok=True)
    report = {
        'platform': platform.platform(), 'machine': platform.machine(),
        'binary': args.binary.name, 'revision': args.revision,
        'colors': args.colors, 'snapshots': args.snapshots,
        'samples': args.samples, 'results': [],
        'stages': 'first_ms measures the first style-manager clone; snapshot_ms includes one selection edit and a retained clone; RSS includes initialization and content checks',
    }
    with args.binary.open('rb') as binary:
        report['binary_sha256'] = hashlib.file_digest(binary, 'sha256').hexdigest()
    digest = None
    if args.baseline:
        baseline = json.loads(args.baseline.read_text())
        for key in ('colors', 'snapshots', 'machine', 'platform'):
            if baseline[key] != report[key]:
                parser.error(f'baseline {key} differs')
        digest = baseline['results'][0]['digest']
    for sample in range(1, args.samples + 1):
        prefix = raw / f'sample-{sample}'
        started = time.monotonic()
        command = [str(args.binary.resolve()), str(args.colors), str(args.snapshots)]
        with prefix.with_suffix('.stdout').open('w') as out, prefix.with_suffix('.stderr').open('w') as err:
            with subprocess.Popen(command, stdout=out, stderr=err) as process:
                try:
                    _, status, usage = os.wait4(process.pid, 0)
                except BaseException:
                    process.terminate()
                    process.wait()
                    raise
                process.returncode = os.waitstatus_to_exitcode(status)
                if process.returncode:
                    raise RuntimeError(f'{prefix.name} exited with {process.returncode}; see its stderr')
        result = json.loads(prefix.with_suffix('.stdout').read_text().splitlines()[-1])
        if digest is not None and result['digest'] != digest:
            raise RuntimeError('snapshot contents differ')
        digest = result['digest']
        result.update(sample=sample, process_elapsed_s=time.monotonic() - started,
                      peak_rss_mib=usage.ru_maxrss / (1024 * 1024 if sys.platform == 'darwin' else 1024))
        report['results'].append(result)
        args.output.write_text(json.dumps(report, indent=2) + '\n')
        print(f"sample={sample}: {result['snapshot_ms']:.6f} ms/snapshot, {result['peak_rss_mib']:.3f} MiB", flush=True)


if __name__ == '__main__':
    main()
