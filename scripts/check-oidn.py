#!/usr/bin/env python3
"""Render a molecular image through the packaged native OIDN runtime."""
import argparse
import json
import os
from pathlib import Path
import runpy
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    binary = parser.parse_args().binary.resolve()
    with tempfile.TemporaryDirectory(prefix='cuemol-oidn-check-') as directory:
        work = Path(directory)
        molecule = work / 'molecule.pdb'
        molecule.write_text(
            'HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\n'
            'HETATM    2  O1  LIG A   1       1.400   0.000   0.000  1.00 10.00           O\nEND\n')
        output = work / 'denoised.png'
        requests = [{'jsonrpc': '2.0', 'id': 0, 'method': 'initialize', 'params': {}}]
        calls = [
            ('load_file', {'path': str(molecule)}),
            ('set_render_settings', {'settings':
             'umbreon.useGI=true; umbreon.denoise=OIDN; '
             'umbreon.giSamples=2; umbreon.supersample=1'}),
            ('render_image', {'fileName': str(output), 'width': 96, 'height': 96}),
        ]
        requests.extend({'jsonrpc': '2.0', 'id': index, 'method': 'tools/call',
                         'params': {'name': name, 'arguments': arguments}}
                        for index, (name, arguments) in enumerate(calls, 1))
        env = dict(os.environ, CUEMOL_CONFIG_DIR=str(work / 'config'), OMP_NUM_THREADS='1')
        result = subprocess.run([str(binary), 'mcp', 'serve'], cwd=work, env=env,
                                input=''.join(json.dumps(request) + '\n' for request in requests),
                                capture_output=True, encoding='utf-8', timeout=90)
        if result.returncode:
            raise RuntimeError(f'OIDN render failed ({result.returncode}):\n{result.stderr}')
        responses = {response['id']: response for response in
                     (json.loads(line) for line in result.stdout.splitlines())}
        for request in requests:
            response = responses.get(request['id'])
            if response is None or response.get('error') or response.get('result', {}).get('isError'):
                raise RuntimeError(f'OIDN render request failed: {response}\n{result.stderr}')
        png_colors = runpy.run_path(str(Path(__file__).with_name('check-povray.py')))['png_colors']
        count = png_colors(output)
        if count < 32:
            raise RuntimeError(f'OIDN output lacks molecular shading: {count} colors')
        print(f'Bundled OIDN molecular render passed: 96x96, {count} colors')


if __name__ == '__main__':
    main()
