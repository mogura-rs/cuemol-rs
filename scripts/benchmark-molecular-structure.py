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


def measure(binary, source, iterations, output, timeout):
    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen([str(binary), str(source), str(iterations), str(output)],
                                   stdout=log, stderr=subprocess.STDOUT)
        started = time.monotonic()
        peak_mib = None
        if hasattr(os, 'wait4'):
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    peak_mib = usage.ru_maxrss / (1024 ** 2 if sys.platform == 'darwin' else 1024)
                    break
                if time.monotonic() - started > timeout:
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
        text = log.read().decode('utf-8', errors='replace')
    if process.returncode:
        raise RuntimeError(text)
    rows = [json.loads(line) for line in text.splitlines()]
    assert len(rows) == iterations
    assert len({row['read_hash'] for row in rows}) == 1
    assert len({row['final_hash'] for row in rows}) == 1
    return dict(iterations=rows, peak_mib=peak_mib,
                qdf_sha256=hashlib.sha256(output.read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser(description='Measure molecular reads and edit phases with lossless output checks.')
    parser.add_argument('--before', required=True, type=Path)
    parser.add_argument('--after', type=Path)
    parser.add_argument('--input', required=True, nargs='+', type=Path)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--iterations', type=int, default=3)
    parser.add_argument('--timeout', type=float, default=180)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.samples < 1 or args.iterations < 1 or args.timeout <= 0:
        parser.error('counts and timeout must be positive')
    variants = [('before', args.before.resolve())]
    if args.after is not None:
        variants.append(('after', args.after.resolve()))
    results = []
    with tempfile.TemporaryDirectory(prefix='cuemol-structure-bench-') as directory:
        for source in args.input:
            source = source.resolve()
            samples = {label: [] for label, _ in variants}
            expected = None
            for sample in range(args.samples):
                for label, binary in variants[::1 if sample % 2 == 0 else -1]:
                    result = measure(binary, source, args.iterations,
                                     Path(directory) / 'output.qdf', args.timeout)
                    identity = (result['qdf_sha256'], result['iterations'][0]['final_hash'])
                    if expected is None:
                        expected = identity
                    assert identity == expected, (source.name, label, identity, expected)
                    samples[label].append(result)
                    print(json.dumps(dict(input=source.name, version=label, sample=sample, **result)), flush=True)
            for label, _ in variants:
                rows = samples[label]
                warm = [iteration for row in rows for iteration in row['iterations'][1:]]
                metrics = ('read_ms', 'topology_ms', 'secondary_ms', 'basepair_ms', 'properties_ms')
                results.append(dict(input=source.name, input_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                                    version=label, samples=rows,
                                    cold_median_ms={key: statistics.median(row['iterations'][0][key] for row in rows) for key in metrics},
                                    warm_median_ms={key: statistics.median(row[key] for row in warm) for key in metrics} if warm else None,
                                    median_peak_mib=statistics.median(row['peak_mib'] for row in rows) if rows[0]['peak_mib'] is not None else None))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(dict(results=results), indent=2) + '\n')


if __name__ == '__main__':
    main()
