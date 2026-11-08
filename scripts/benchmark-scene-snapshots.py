import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time


def generate_obj(path, size):
    with path.open('w') as stream:
        for y in range(size):
            for x in range(size):
                stream.write(f'v {x / 10:.1f} {y / 10:.1f} {(x * y % 17) / 100:.2f}\n')
        for y in range(size - 1):
            for x in range(size - 1):
                vertex = 1 + x + y * size
                stream.write(f'f {vertex} {vertex + 1} {vertex + size}\n'
                             f'f {vertex + 1} {vertex + size + 1} {vertex + size}\n')


def measure(binary, source, edits, timeout):
    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen([str(binary), str(source), str(edits)],
                                   stdout=log, stderr=subprocess.STDOUT)
        start = time.monotonic()
        peak_mib = None
        if hasattr(os, 'wait4'):
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    peak_mib = usage.ru_maxrss / (1024 ** 2 if sys.platform == 'darwin' else 1024)
                    break
                if time.monotonic() - start > timeout:
                    process.kill()
                    process.wait()
                    raise TimeoutError(f'{source.name} exceeded {timeout} seconds')
                time.sleep(0.01)
        else:
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise
        log.seek(0)
        output = log.read().decode('utf-8', errors='replace')
    if process.returncode:
        raise RuntimeError(output)
    return dict(**json.loads(output), peak_mib=peak_mib)


def main():
    parser = argparse.ArgumentParser(description='Measure scene snapshots and retained Undo/Redo changes.')
    parser.add_argument('--before', required=True, type=Path)
    parser.add_argument('--after', type=Path)
    parser.add_argument('--input', nargs='*', type=Path, default=[])
    parser.add_argument('--generate-obj', nargs='*', type=int, default=[])
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--edits', type=int, default=8)
    parser.add_argument('--timeout', type=float, default=180)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if (min(args.samples, args.edits, args.timeout) <= 0
            or any(size < 2 for size in args.generate_obj)
            or not (args.input or args.generate_obj)):
        parser.error('positive counts and at least one input or generated grid are required')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    sources = list(args.input)
    for size in args.generate_obj:
        source = args.output.parent / f'surface-{size}.obj'
        generate_obj(source, size)
        sources.append(source)
    variants = [('before', args.before.resolve())]
    if args.after:
        variants.append(('after', args.after.resolve()))
    results = []
    for source in sources:
        samples = {label: [] for label, _ in variants}
        identity = None
        for sample in range(args.samples):
            for label, binary in variants[::1 if sample % 2 == 0 else -1]:
                result = measure(binary, source.resolve(), args.edits, args.timeout)
                if identity is None:
                    identity = result['hash']
                assert result['hash'] == identity, (source.name, label)
                samples[label].append(result)
                print(json.dumps(dict(input=source.name, version=label, sample=sample, **result)), flush=True)
        for label, _ in variants:
            rows = samples[label]
            results.append(dict(input=source.name, input_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                                version=label, samples=rows,
                                medians={key: statistics.median(row[key] for row in rows) if rows[0][key] is not None else None
                                         for key in ['load_ms', 'edits_ms', 'undo_redo_with_hash_ms', 'peak_mib']}))
    args.output.write_text(json.dumps(dict(
        binaries={label: hashlib.sha256(binary.read_bytes()).hexdigest() for label, binary in variants},
        results=results), indent=2) + '\n')


if __name__ == '__main__':
    main()
