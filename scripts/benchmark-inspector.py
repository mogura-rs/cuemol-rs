#!/usr/bin/env python3
"""Measure property queries in fresh processes and verify a saved baseline."""

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
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--iterations', type=int, default=300)
    parser.add_argument('--style-colors', type=int, nargs='+', default=[0, 1000])
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not hasattr(os, 'wait4'):
        parser.error('POSIX wait4 is required for per-process peak RSS')
    if min(args.samples, args.iterations) < 1 or min(args.style_colors) < 0:
        parser.error('samples and iterations must be positive; color counts must be nonnegative')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw = args.output.parent / f'{args.output.stem}-raw'
    raw.mkdir(exist_ok=True)
    report = {
        'platform': platform.platform(), 'machine': platform.machine(),
        'binary': args.binary.name,
        'revision': args.revision, 'fixture': args.fixture.name,
        'fixture_sha256': hashlib.sha256(args.fixture.read_bytes()).hexdigest(),
        'samples': args.samples, 'iterations': args.iterations, 'results': [],
        'stages': 'cold_ms and query_ms measure full list_node_props calls; process RSS includes molecule loading and runtime',
    }
    with args.binary.open('rb') as binary:
        report['binary_sha256'] = hashlib.file_digest(binary, 'sha256').hexdigest()
    digests = {}
    if args.baseline:
        baseline = json.loads(args.baseline.read_text())
        if baseline['fixture_sha256'] != report['fixture_sha256']:
            parser.error('baseline input differs')
        digests.update((row['style_colors'], row['digest']) for row in baseline['results'])
    for colors in args.style_colors:
        for sample in range(1, args.samples + 1):
            prefix = raw / f'colors-{colors}-{sample}'
            started = time.monotonic()
            with prefix.with_suffix('.stdout').open('w') as out, prefix.with_suffix('.stderr').open('w') as err:
                command = [str(args.binary.resolve()), str(args.fixture.resolve()), str(args.iterations), str(colors)]
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
            result = next(json.loads(line.removeprefix('INSPECTOR_RESULT '))
                          for line in prefix.with_suffix('.stdout').read_text().splitlines()
                          if line.startswith('INSPECTOR_RESULT '))
            result.update(sample=sample, process_elapsed_s=time.monotonic() - started,
                          peak_rss_mib=usage.ru_maxrss / (1024 * 1024 if sys.platform == 'darwin' else 1024))
            if colors in digests and result['digest'] != digests[colors]:
                raise RuntimeError(f'property data changed for {colors} style colors')
            digests[colors] = result['digest']
            report['results'].append(result)
            args.output.write_text(json.dumps(report, indent=2) + '\n')
            print(f"colors={colors}, sample={sample}: {result['query_ms']:.4f} ms/query, {result['peak_rss_mib']:.3f} MiB", flush=True)


if __name__ == '__main__':
    main()
