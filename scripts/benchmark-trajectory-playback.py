import argparse
import array
import json
import os
from pathlib import Path
import statistics
import struct
import subprocess
import sys
import tempfile
import time


def write_dcd(path, atoms, frames=4):
    def record(output, data):
        size = struct.pack('<i', len(data))
        output.write(size)
        output.write(data)
        output.write(size)

    header = bytearray(84)
    header[:4] = b'CORD'
    struct.pack_into('<i', header, 4, frames)
    struct.pack_into('<i', header, 12, 1)
    struct.pack_into('<f', header, 40, 1)
    struct.pack_into('<i', header, 80, 24)
    with path.open('wb') as output:
        record(output, header)
        record(output, struct.pack('<i', 0))
        record(output, struct.pack('<i', atoms))
        for frame in range(frames):
            for axis in range(3):
                values = array.array('f', (index * 0.125 if axis == 0 else frame if axis == 1 else -0.25
                                          for index in range(atoms)))
                if sys.byteorder != 'little':
                    values.byteswap()
                record(output, values.tobytes())


def measure(binary, source, atoms, iterations, directory, average=0, frames=4):
    env = dict(os.environ, CUEMOL_CONFIG_DIR=str(directory / 'config'))
    with tempfile.TemporaryFile() as output:
        process = subprocess.Popen([str(binary), str(source), str(atoms), str(iterations), str(average), str(frames)],
                                   env=env, stdout=output, stderr=subprocess.STDOUT)
        started = time.monotonic()
        peak_mib = None
        if hasattr(os, 'wait4'):
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    peak_mib = usage.ru_maxrss / (1024 ** 2 if sys.platform == 'darwin' else 1024)
                    break
                if time.monotonic() - started > 180:
                    process.kill()
                    process.wait()
                    raise TimeoutError('trajectory benchmark exceeded 180 seconds')
                time.sleep(0.01)
        else:
            try:
                process.wait(timeout=180)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise
        output.seek(0)
        text = output.read().decode('utf-8', errors='replace')
    if process.returncode:
        raise RuntimeError(text)
    return dict(json.loads(text.strip().splitlines()[-1]), peak_mib=peak_mib)


def main():
    parser = argparse.ArgumentParser(description='Measure scene coordinate updates, excluding rendering and hash verification.')
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--atoms', nargs='+', type=int, default=[20000, 200000])
    parser.add_argument('--iterations', type=int, default=32)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--frames', type=int, default=4)
    parser.add_argument('--average', type=int, default=0)
    parser.add_argument('--modes', nargs='+', choices=('memory', 'indexed_dcd'), default=['memory', 'indexed_dcd'])
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.samples < 1 or args.iterations < 1 or not 0 < args.frames <= 2**31 - 1 or not 0 <= args.average <= 2**31 - 1 or any(not 0 < atoms <= 2**31 - 1 for atoms in args.atoms):
        parser.error('counts must be positive and fit in a signed 32-bit integer')
    rows = []
    with tempfile.TemporaryDirectory(prefix='cuemol-playback-bench-') as directory:
        directory = Path(directory)
        for atoms in args.atoms:
            path = directory / 'trajectory.dcd'
            write_dcd(path, atoms, args.frames)
            expected_hash = None
            for mode, source in [('memory', 'memory'), ('indexed_dcd', path)]:
                if mode not in args.modes:
                    continue
                samples = [measure(args.binary.resolve(), source, atoms, args.iterations, directory, args.average, args.frames)
                           for _ in range(args.samples)]
                for sample in samples:
                    expected_hash = expected_hash or sample['hash']
                    assert sample['hash'] == expected_hash
                    assert sample['atoms'] == atoms and sample['frames'] == args.frames
                    assert sample['average'] == args.average
                row = dict(atoms=atoms, mode=mode, samples=samples,
                           median={key: statistics.median(sample[key] for sample in samples)
                                   for key in ['apply_ms', 'load_ms']},
                           median_peak_mib=statistics.median(sample['peak_mib'] for sample in samples)
                           if samples[0]['peak_mib'] is not None else None)
                rows.append(row)
                print(json.dumps(row), flush=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({'results': rows}, indent=2) + '\n')


if __name__ == '__main__':
    main()
