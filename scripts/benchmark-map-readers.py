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


def generate_dx(path, size):
    with path.open('w') as stream:
        stream.write(f'object 1 class gridpositions counts {size} {size} {size}\n'
                     'origin -1 2 -3\ndelta 0.5 0 0\ndelta 0 0.75 0\ndelta 0 0 1.25\n'
                     f'object 2 class gridconnections counts {size} {size} {size}\n'
                     f'object 3 class array type double rank 0 items {size ** 3} data follows\n')
        for start in range(0, size ** 3, 3):
            values = [f'{((index * 37) % 10007 - 5003) / 16:.4f}'
                      for index in range(start, min(start + 3, size ** 3))]
            stream.write(' '.join(values) + '\n')
        stream.write('attribute "dep" string "positions"\n')


def generate_xplor(path, size):
    with path.open('w') as stream:
        stream.write('0\n' + ''.join(f'{value:8}' for value in [size, 0, size - 1] * 3) + '\n')
        stream.write(''.join(f'{value:12.5E}' for value in [size, size, size, 90, 90, 90]) + '\nZYX\n')
        for section in range(size):
            stream.write(f'{section}\n')
            end = (section + 1) * size ** 2
            for start in range(section * size ** 2, end, 6):
                values = [f'{((index * 37) % 10007 - 5003) / 16:12.5E}'
                          for index in range(start, min(start + 6, end))]
                stream.write(''.join(values) + '\n')


def measure(binary, source, iterations, timeout):
    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen([str(binary), str(source), str(iterations)],
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
    rows = [json.loads(line) for line in output.splitlines()]
    assert len(rows) == iterations
    assert len({row['hash'] for row in rows}) == 1
    return dict(iterations=rows, peak_mib=peak_mib)


def main():
    parser = argparse.ArgumentParser(description='Compare map reader time, peak RSS and exact grid hashes.')
    parser.add_argument('--before', required=True, type=Path)
    parser.add_argument('--after', required=True, type=Path)
    parser.add_argument('--input', nargs='*', type=Path, default=[])
    parser.add_argument('--generate-dx', nargs='*', type=int, default=[])
    parser.add_argument('--generate-xplor', nargs='*', type=int, default=[])
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--iterations', type=int, default=2)
    parser.add_argument('--timeout', type=float, default=180)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if (min(args.samples, args.iterations, args.timeout) <= 0
            or any(size <= 0 for size in args.generate_dx + args.generate_xplor)
            or not (args.input or args.generate_dx or args.generate_xplor)):
        parser.error('positive counts and at least one input or generated grid are required')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    sources = list(args.input)
    for size in args.generate_dx:
        source = args.output.parent / f'grid-{size}.dx'
        generate_dx(source, size)
        sources.append(source)
    for size in args.generate_xplor:
        source = args.output.parent / f'grid-{size}.xplor'
        generate_xplor(source, size)
        sources.append(source)
    variants = [('before', args.before.resolve()), ('after', args.after.resolve())]
    results = []
    for source in sources:
        samples = {label: [] for label, _ in variants}
        identity = None
        for sample in range(args.samples):
            for label, binary in variants[::1 if sample % 2 == 0 else -1]:
                result = measure(binary, source.resolve(), args.iterations, args.timeout)
                row = result['iterations'][0]
                current = (row['hash'], row['counts'], row['values'])
                if identity is None:
                    identity = current
                assert current == identity, (source.name, label, current, identity)
                samples[label].append(result)
                print(json.dumps(dict(input=source.name, version=label, sample=sample, **result)), flush=True)
        for label, _ in variants:
            rows = samples[label]
            results.append(dict(input=source.name, input_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                                version=label, samples=rows,
                                cold_median_ms=statistics.median(row['iterations'][0]['read_ms'] for row in rows),
                                warm_median_ms=statistics.median(row['iterations'][-1]['read_ms'] for row in rows),
                                median_peak_mib=statistics.median(row['peak_mib'] for row in rows) if rows[0]['peak_mib'] is not None else None))
    args.output.write_text(json.dumps(dict(
        binaries={label: hashlib.sha256(binary.read_bytes()).hexdigest() for label, binary in variants},
        results=results), indent=2) + '\n')


if __name__ == '__main__':
    main()
