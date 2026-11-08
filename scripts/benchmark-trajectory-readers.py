import argparse
from array import array
import hashlib
import json
import os
from pathlib import Path
import statistics
import struct
import subprocess
import sys
import tempfile
import time


def floats(values, double=False, big=False):
    data = array('d' if double else 'f', values)
    if (sys.byteorder == 'big') != big:
        data.byteswap()
    return data.tobytes()


def record(stream, data):
    marker = struct.pack('<I', len(data))
    stream.write(marker + data + marker)


def generate_dcd(path, atoms, frames, malformed=False):
    with path.open('wb') as stream:
        header = bytearray(84)
        header[:4] = b'CORD'
        struct.pack_into('<I', header, 4, frames)
        struct.pack_into('<I', header, 44, 1)
        record(stream, header)
        record(stream, bytes(4))
        record(stream, struct.pack('<I', atoms))
        for frame in range(frames):
            record(stream, floats([30 + frame, 0, 40, 0, 0, 50], double=True))
            if malformed:
                stream.write(struct.pack('<I', atoms * 4) + bytes(16))
                return
            for axis in range(3):
                record(stream, floats(((atom * 3 + axis + frame) % 2048 / 4 for atom in range(atoms))))


def generate_xtc(path, atoms, frames, long=False, malformed=False):
    with path.open('wb') as stream:
        for frame in range(frames):
            stream.write(struct.pack('>iiif', 2023 if long else 1995, atoms, frame, frame / 2))
            stream.write(floats([3 + frame / 10, 0, 0, 0, 4, 0, 0, 0, 5], big=True))
            stream.write(struct.pack('>if', atoms, 0 if malformed else 1000))
            if malformed:
                return
            stream.write(struct.pack('>7i', 0, 0, 0, 127, 0, 0, 9))
            packed = bytearray()
            bits = count = 0
            for atom in range(atoms):
                bits = (bits << 9) | (((atom + frame) % 128) << 1)
                count += 9
                while count >= 8:
                    count -= 8
                    packed.append((bits >> count) & 255)
                bits &= (1 << count) - 1
            if count:
                packed.append(bits << (8 - count))
            stream.write(struct.pack('>Q' if long else '>I', len(packed)))
            stream.write(packed)
            stream.write(bytes((-len(packed)) % 4))


def generate_trr(path, atoms, frames, double=False, malformed=False):
    with path.open('wb') as stream:
        version = b'GMX_trn_file'
        scalar = 8 if double else 4
        for frame in range(frames):
            stream.write(struct.pack('>iii', 1993, len(version) + 1, len(version)))
            stream.write(version + bytes((-len(version)) % 4))
            stream.write(struct.pack('>13i', 0, 0, 9 * scalar, 0, 0, 0, 0,
                                     atoms * 3 * scalar, 0, 0, atoms, frame, 0))
            stream.write(floats([frame / 2, 0], double=double, big=True))
            stream.write(floats([3 + frame / 10, 0, 0, 0, 4, 0, 0, 0, 5], double=double, big=True))
            if malformed:
                return
            stream.write(floats(((index + frame) % 2048 / 32 for index in range(atoms * 3)),
                                double=double, big=True))


def generate_netcdf(path, atoms, frames, malformed=False):
    header = bytearray(b'CDF\x01' + struct.pack('>I', frames))

    def name(value):
        encoded = value.encode('ascii')
        header.extend(struct.pack('>I', len(encoded)) + encoded + bytes((-len(encoded)) % 4))

    header.extend(struct.pack('>II', 10, 3))
    for label, count in [('frame', 0), ('atom', atoms), ('spatial', 3)]:
        name(label)
        header.extend(struct.pack('>I', count))
    header.extend(struct.pack('>II', 12, 1))
    name('Conventions')
    header.extend(struct.pack('>II', 2, 5) + b'AMBER\0\0\0')
    header.extend(struct.pack('>II', 11, 3))
    positions = []
    offset = 0
    for label, dimensions, size in [('coordinates', [0, 1, 2], atoms * 12),
                                     ('cell_lengths', [0, 2], 12), ('cell_angles', [0, 2], 12)]:
        name(label)
        header.extend(struct.pack('>I', len(dimensions)))
        header.extend(struct.pack('>' + 'I' * len(dimensions), *dimensions))
        header.extend(struct.pack('>IIII', 0, 0, 5, size))
        positions.append((len(header), offset))
        header.extend(bytes(4))
        offset += size
    for position, offset in positions:
        struct.pack_into('>I', header, position, len(header) + offset)
    with path.open('wb') as stream:
        stream.write(header)
        if not malformed:
            for frame in range(frames):
                stream.write(floats(((index + frame) % 2048 / 4 for index in range(atoms * 3)), big=True))
                stream.write(floats([30 + frame, 40, 50, 90, 90, 90], big=True))


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1048576), b''):
            result.update(chunk)
    return result.hexdigest()


def measure(binary, source, atoms, iterations, mode, skip, timeout):
    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen([str(binary), str(source), str(atoms), str(iterations), mode, str(skip)],
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
    assert len({(row['hash'], row['atoms'], row['frames'], row['error']) for row in rows}) == 1
    return dict(iterations=rows, peak_mib=peak_mib)


def main():
    parser = argparse.ArgumentParser(description='Compare trajectory decoding, peak RSS and coordinate/cell hashes.')
    parser.add_argument('--before', required=True, type=Path)
    parser.add_argument('--after', type=Path)
    parser.add_argument('--atoms', type=int, default=200000)
    parser.add_argument('--frames', type=int, default=16)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--iterations', type=int, default=2)
    parser.add_argument('--formats', nargs='+', choices=['dcd', 'xtc', 'xtc64', 'trr', 'trr64', 'netcdf'],
                        default=['dcd', 'xtc', 'xtc64', 'trr', 'trr64'])
    parser.add_argument('--timeout', type=float, default=180)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.atoms < 10 or min(args.frames, args.samples, args.iterations, args.timeout) <= 0:
        parser.error('at least ten atoms and positive frame/sample/iteration counts are required')
    fixtures = args.output.parent / 'fixtures'
    fixtures.mkdir(parents=True, exist_ok=True)
    cases = []
    for label, generator, options in [
        ('dcd', generate_dcd, {}), ('xtc', generate_xtc, {}),
        ('xtc64', generate_xtc, dict(long=True)),
        ('trr', generate_trr, {}), ('trr64', generate_trr, dict(double=True)),
        ('netcdf', generate_netcdf, {}),
    ]:
        if label not in args.formats:
            continue
        suffix = 'nc' if label == 'netcdf' else label.removesuffix('64')
        source = fixtures / f'{label}-{args.atoms}-{args.frames}.{suffix}'
        generator(source, args.atoms, args.frames, **options)
        cases.extend([(source, args.atoms, 'all', 1), (source, args.atoms, 'selected', 2)])
    for suffix, generator in [('dcd', generate_dcd), ('xtc', generate_xtc), ('trr', generate_trr), ('nc', generate_netcdf)]:
        if ('netcdf' if suffix == 'nc' else suffix) not in args.formats:
            continue
        source = fixtures / f'malformed.{suffix}'
        generator(source, 2000000, 1, malformed=True)
        cases.append((source, 2000000, 'error', 1))
    variants = [('before', args.before.resolve())]
    if args.after:
        variants.append(('after', args.after.resolve()))
    results = []
    for source, atoms, mode, skip in cases:
        identity = None
        samples = {label: [] for label, _ in variants}
        for sample in range(args.samples):
            for label, binary in variants[::1 if sample % 2 == 0 else -1]:
                result = measure(binary, source.resolve(), atoms, args.iterations, mode, skip, args.timeout)
                row = result['iterations'][0]
                current = (row['hash'], row['atoms'], row['frames'], row['error'])
                if identity is None:
                    identity = current
                assert current == identity, (source.name, mode, label, current, identity)
                samples[label].append(result)
                print(json.dumps(dict(input=source.name, mode=mode, version=label, sample=sample, **result)), flush=True)
        for label, _ in variants:
            rows = samples[label]
            results.append(dict(input=source.name, input_sha256=digest(source), mode=mode, skip=skip, version=label,
                                samples=rows,
                                warm_median_ms=statistics.median(row['iterations'][-1]['read_ms'] for row in rows),
                                median_peak_mib=statistics.median(row['peak_mib'] for row in rows) if rows[0]['peak_mib'] is not None else None))
    args.output.write_text(json.dumps(dict(binaries={label: digest(binary) for label, binary in variants},
                                          results=results), indent=2) + '\n')


if __name__ == '__main__':
    main()
