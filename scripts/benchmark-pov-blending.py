#!/usr/bin/env python3
"""Measure native PNG composition without running a ray tracer."""
import argparse
import json
import os
from pathlib import Path
import statistics
import struct
import subprocess
import sys
import tempfile
import time
import zlib


def write_png(path, size, shift):
    def chunk(output, kind, data):
        output.write(struct.pack('>I', len(data)) + kind + data
                     + struct.pack('>I', zlib.crc32(kind + data)))
    with path.open('wb') as output:
        output.write(b'\x89PNG\r\n\x1a\n')
        chunk(output, b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 2, 0, 0, 0))
        compressor = zlib.compressobj(1)
        rows = [bytes([0]) + bytes(value % 256 for x in range(size)
                 for value in [x + shift, y + 2 * shift, x + y + 3 * shift])
                for y in range(min(size, 256))]
        for y in range(size):
            compressed = compressor.compress(rows[y % len(rows)])
            if compressed:
                chunk(output, b'IDAT', compressed)
        chunk(output, b'IDAT', compressor.flush())
        chunk(output, b'IEND', b'')


def measure(binary, request, work, size, helper):
    env = dict(os.environ, CUEMOL_CONFIG_DIR=str(work / 'config'))
    with tempfile.TemporaryFile() as output:
        started = time.monotonic()
        process = subprocess.Popen([str(binary), 'tools', 'render-pov', str(request)],
                                   stdout=output, stderr=subprocess.STDOUT, env=env)
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
                    raise TimeoutError('PNG composition exceeded 180 seconds')
                time.sleep(0.01)
        else:
            process.wait(timeout=180)
        elapsed = time.monotonic() - started
        output.seek(0)
        if process.returncode:
            raise RuntimeError(output.read().decode(errors='replace'))
    digest = subprocess.check_output([
        sys.executable, '-c',
        'import hashlib,runpy,sys;from pathlib import Path;'
        'decode=runpy.run_path(sys.argv[1])["png_pixels"];'
        'pixels=decode(Path(sys.argv[2]),(int(sys.argv[3]),)*2)[3];'
        'print(hashlib.sha256(pixels).hexdigest())',
        str(helper), str(work / 'result.png'), str(size),
    ], text=True, timeout=120).strip()
    return dict(elapsed_ms=1000 * elapsed, peak_mib=peak_mib, pixel_sha256=digest)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--size', type=int, default=2048)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if os.name != 'posix':
        parser.error('the copy-only renderer fixture requires a POSIX shell')
    if args.size <= 0 or args.samples <= 0:
        parser.error('size and samples must be positive')
    helper = Path(__file__).with_name('check-povray.py').resolve()
    with tempfile.TemporaryDirectory(prefix='cuemol-pov-bench-') as directory:
        work = Path(directory)
        groups = [2, 5, 7, 9]
        for group in [10] + groups:
            write_png(work / f'{group}.png', args.size, group * 17)
        (work / 'scene.pov').write_text('// CueMol post-blend groups: 2 5 7 9\n')
        renderer = work / 'copy-renderer'
        renderer.write_text("""#!/bin/sh
group=10
for arg in "$@"; do
    case "$arg" in
        +O*) output=${arg#'+O'} ;;
        Declare=_cuemol_blend_group=*) group=${arg##*=} ;;
    esac
done
cp "$group.png" "$output"
""")
        renderer.chmod(0o755)
        request = work / 'request.json'
        request.write_text(json.dumps(dict(executable=str(renderer),
            input_path=str(work / 'scene.pov'), output_path=str(work / 'result.png'),
            options=dict(width=args.size, height=args.size, post_blend=True))))
        samples = [measure(args.binary.resolve(), request, work, args.size, helper)
                   for _ in range(args.samples)]
        assert len({sample['pixel_sha256'] for sample in samples}) == 1
        result = dict(size=args.size, layers=len(groups), samples=samples,
                      median_elapsed_ms=statistics.median(sample['elapsed_ms'] for sample in samples),
                      median_peak_mib=statistics.median(sample['peak_mib'] for sample in samples)
                      if samples[0]['peak_mib'] is not None else None)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
