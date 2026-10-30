#!/usr/bin/env python3
"""Compare selection queries in fresh processes with output and RSS checks."""

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
    parser.add_argument('--target-fixture', type=Path)
    parser.add_argument('--expression', action='append', required=True)
    parser.add_argument('--reference-binary', type=Path)
    parser.add_argument('--reference-app', type=Path)
    parser.add_argument('--reference-version')
    parser.add_argument('--revision', required=True)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--iterations', type=int, default=5)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not hasattr(os, 'wait4'):
        parser.error('POSIX wait4 is required for per-process peak RSS')
    if min(args.samples, args.iterations) < 1:
        parser.error('samples and iterations must be positive')
    if bool(args.reference_binary) != bool(args.reference_app):
        parser.error('reference-binary and reference-app are required together')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw = args.output.parent / f'{args.output.stem}-raw'
    raw.mkdir(exist_ok=True)
    report = {
        'platform': platform.platform(), 'machine': platform.machine(),
        'revision': args.revision, 'reference_version': args.reference_version,
        'fixture': args.fixture.name, 'fixture_sha256': hashlib.sha256(args.fixture.read_bytes()).hexdigest(),
        'samples': args.samples, 'iterations': args.iterations, 'results': [],
        'stages': 'cold_ms and query_ms include expression compilation and atom counting; digest computation is outside the timing; process RSS includes loading and runtime',
    }
    with args.binary.open('rb') as binary:
        report['binary_sha256'] = hashlib.file_digest(binary, 'sha256').hexdigest()
    expected = {}
    if args.target_fixture:
        report.update(target_fixture=args.target_fixture.name,
                      target_sha256=hashlib.sha256(args.target_fixture.read_bytes()).hexdigest())
    if args.baseline:
        baseline = json.loads(args.baseline.read_text())
        if baseline['fixture_sha256'] != report['fixture_sha256']:
            parser.error('baseline fixture differs')
        if baseline.get('target_sha256') != report.get('target_sha256'):
            parser.error('baseline target fixture differs')
        expected.update((row['expression'], (row['atoms'], row['count'], row['digest'])) for row in baseline['results'])
    for number, expression in enumerate(args.expression):
        for sample in range(1, args.samples + 1):
            for condition in (['reference', 'rust'] if args.reference_binary else ['rust']):
                prefix = raw / f'{number}-{condition}-{sample}'
                if condition == 'reference':
                    command = [str(args.reference_binary.resolve()), str(Path(__file__).with_name('benchmark-reference-selection.mjs')), str(args.reference_app.resolve()), str(args.fixture.resolve()), str(args.iterations), expression]
                else:
                    command = [str(args.binary.resolve()), str(args.fixture.resolve()), str(args.iterations), expression]
                if args.target_fixture:
                    command.append(str(args.target_fixture.resolve()))
                env = os.environ.copy()
                if condition == 'reference':
                    env['ELECTRON_RUN_AS_NODE'] = '1'
                started = time.monotonic()
                with prefix.with_suffix('.stdout').open('w') as out, prefix.with_suffix('.stderr').open('w') as err:
                    with subprocess.Popen(command, stdout=out, stderr=err, env=env) as process:
                        try:
                            _, status, usage = os.wait4(process.pid, 0)
                        except BaseException:
                            process.terminate()
                            process.wait()
                            raise
                        process.returncode = os.waitstatus_to_exitcode(status)
                        if process.returncode:
                            raise RuntimeError(f'{prefix.name} exited with {process.returncode}; see its stderr')
                result = next(json.loads(line.removeprefix('SELECTION_RESULT ')) for line in prefix.with_suffix('.stdout').read_text().splitlines() if line.startswith('SELECTION_RESULT '))
                output = (result['atoms'], result['count'], result['digest'])
                if expression in expected and output != expected[expression]:
                    raise RuntimeError(f'{condition} selection differs for {expression}: {output} != {expected[expression]}')
                expected[expression] = output
                result.update(condition=condition, sample=sample, process_elapsed_s=time.monotonic() - started,
                              peak_rss_mib=usage.ru_maxrss / (1024 * 1024 if sys.platform == 'darwin' else 1024))
                report['results'].append(result)
                args.output.write_text(json.dumps(report, indent=2) + '\n')
                print(f"{condition} {sample}: {expression}: {result['query_ms']:.4f} ms, {result['peak_rss_mib']:.3f} MiB", flush=True)


if __name__ == '__main__':
    main()
