# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow>=11,<14"]
# ///
"""Measure native image exports in fresh processes against an optional reference."""

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image, ImageChops


def measure(command, directory, reference):
    directory.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    if reference:
        env['ELECTRON_RUN_AS_NODE'] = '1'
    started = time.monotonic()
    with (
        (directory / 'stdout.log').open('w') as out,
        (directory / 'stderr.log').open('w') as err,
        subprocess.Popen(command, stdout=out, stderr=err, env=env) as process,
    ):
        try:
            _, status, usage = os.wait4(process.pid, 0)
        except BaseException:
            process.terminate()
            process.wait()
            raise
        process.returncode = os.waitstatus_to_exitcode(status)
        if process.returncode:
            raise RuntimeError(f'{directory.name} exited with {process.returncode}; see its logs')
    elapsed = time.monotonic() - started
    line = next(line for line in (directory / 'stdout.log').read_text().splitlines() if line.startswith('RENDER_RESULT '))
    result = json.loads(line.removeprefix('RENDER_RESULT '))
    result.update(process_elapsed_s=elapsed,
                  peak_rss_mib=usage.ru_maxrss / (1024 * 1024 if sys.platform == 'darwin' else 1024))
    for sample in result['samples']:
        file = Path(sample['result']['path'])
        with Image.open(file) as image:
            rgba = image.convert('RGBA')
            if image.size != (sample['result']['width'], sample['result']['height']):
                raise RuntimeError('The image dimensions differ from the render result')
            sample['rgba_sha256'] = hashlib.sha256(rgba.tobytes()).hexdigest()
            sample['png_sha256'] = hashlib.sha256(file.read_bytes()).hexdigest()
            sample['result']['path'] = file.name
    return result


def compare_images(reference, current):
    with Image.open(reference) as image:
        size = image.size
        first = image.convert('RGBA')
    with Image.open(current) as image:
        if image.size != size:
            raise RuntimeError('The reference and current image dimensions differ')
        second = image.convert('RGBA')
    histogram = ImageChops.difference(first, second).histogram()
    counts = [sum(histogram[channel * 256 + error] for channel in range(4))
              for error in range(256)]
    channels = sum(counts)
    return {'rgba_mean_absolute_error': sum(error * count for error, count in enumerate(counts)) / channels,
            'rgba_max_absolute_error': max(error for error, count in enumerate(counts) if count),
            'differing_channels': channels - counts[0],
            'channels': channels}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--fixture', required=True, type=Path)
    parser.add_argument('--reference-binary', type=Path)
    parser.add_argument('--reference-app', type=Path)
    parser.add_argument('--reference-version')
    parser.add_argument('--revision', required=True)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--repetitions', type=int, default=2)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.samples < 1 or args.repetitions < 1:
        parser.error('samples and repetitions must be positive')
    if bool(args.reference_binary) != bool(args.reference_app):
        parser.error('reference-binary and reference-app are required together')
    if not hasattr(os, 'wait4'):
        parser.error('POSIX wait4 is required for per-process peak RSS')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw = args.output.parent / f'{args.output.stem}-raw'
    report = {'platform': platform.platform(), 'machine': platform.machine(),
              'revision': args.revision, 'reference_version': args.reference_version,
              'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              'fixture': args.fixture.name,
              'fixture_sha256': hashlib.sha256(args.fixture.read_bytes()).hexdigest(),
              'samples': args.samples, 'repetitions': args.repetitions, 'results': [],
              'stages': 'prepare_ms + render_ms covers scene preparation, ray tracing, denoising and PNG writing; process RSS includes loading and runtime'}
    conditions = ['reference', 'current'] if args.reference_binary else ['current']
    for sample in range(args.samples):
        paths = {}
        objects = {}
        for condition in conditions:
            directory = raw / f'{condition}-{sample + 1}'
            paths[condition] = directory
            if condition == 'reference':
                command = [str(args.reference_binary.resolve()), str(Path(__file__).with_name('benchmark-reference-render.mjs')),
                           str(args.reference_app.resolve()), str(args.fixture.resolve()), str(directory.resolve()), str(args.repetitions)]
            else:
                command = [str(args.binary.resolve()), str(args.fixture.resolve()), str(directory.resolve()), str(args.repetitions)]
            result = measure(command, directory, condition == 'reference')
            objects[condition] = sorted((item['name'], item['atoms'], item['bonds']) for item in result['objects'])
            if any(count == 0 for _, count, _ in objects[condition]):
                raise RuntimeError('A benchmark molecule has no atoms')
            report['results'].append({'condition': condition, 'sample': sample + 1, **result})
            args.output.write_text(json.dumps(report, indent=2) + '\n')
            times = [round(item['prepare_ms'] + item['render_ms'], 3) for item in result['samples']]
            print(f"{condition} {sample + 1}/{args.samples}: {times} ms, {result['peak_rss_mib']:.3f} MiB", flush=True)
        if 'reference' in paths:
            if objects['reference'] != objects['current']:
                raise RuntimeError('Reference and current molecule identities, atom counts or bond counts differ')
            report.setdefault('image_comparisons', []).append(compare_images(paths['reference'] / 'render-0.png', paths['current'] / 'render-0.png'))
            args.output.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
