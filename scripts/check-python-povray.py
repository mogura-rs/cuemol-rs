#!/usr/bin/env python3
"""Check live Python POV-Ray helpers using the packaged native tool."""
import argparse
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--renderer-host', type=Path)
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    png_helpers = runpy.run_path(str(Path(__file__).with_name('check-povray.py')))
    png_colors, png_pixels = png_helpers['png_colors'], png_helpers['png_pixels']
    with tempfile.TemporaryDirectory(prefix='cuemol-python-pov-check-') as directory:
        work = Path(directory)
        fixture = work / 'molecule.pdb'
        fixture.write_text(
            'HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\n'
            'HETATM    2  O1  LIG A   1       1.400   0.000   0.000  1.00 10.00           O\nEND\n')
        source = '''
import json
from pathlib import Path
import cuemol as cm
from cuemol import povrender
scene = cm.scene()
molecule = cm.obj(json.loads(scene.getObjectTreeJSON())[0]['ID'])
render = povrender.PovRender(96, 96, fps=2.0, use_fog=False)
render.render_simple(scene, 'class output λ')
povrender.render(scene, 'function output λ.png', width=96, height=96)
labels = molecule.createRenderer('*namelabel')
labels.setLabel(1, 'CueMol')
labels.color = cm.col('#00ff00')
labels.font_size = 12
labels.dispy = 20
labels.label_on_top = True
povrender.PovRender(96, 96, use_pix_imgs=False).render_simple(scene, 'no labels')
povrender.PovRender(96, 96, use_pix_imgs=True).render_simple(scene, 'with labels')
labels.visible = False
distance = molecule.createRenderer('atomintr')
distance.appendById(1, molecule.uid, 2, False)
distance.showlabel = True
distance.label_on_top = True
povrender.PovRender(96, 96, use_pix_imgs=False).render_simple(scene, 'no distance label')
povrender.PovRender(96, 96, use_pix_imgs=True).render_simple(scene, 'distance label')
distance.visible = False

layer = molecule.createRenderer('cpk')
layer.name = 'transparent'
layer.defaultcolor = cm.col('#ff0000')
layer.alpha = 0.5
render.render_simple(scene, 'blended output λ')
Path('failure output.png').write_bytes(b'existing file')
failed = povrender.PovRender(96, 96, povray_bin=str(Path('missing-renderer').resolve()))
try:
    failed.render_simple(scene, 'failure output')
except RuntimeError:
    pass
else:
    raise AssertionError('missing renderer unexpectedly succeeded')
assert Path('failure output.png').read_bytes() == b'existing file'
end = cm.timeval(1000)
manager = scene.getAnimMgr()
manager.length = end
spin = cm.createObj('SimpleSpin')
spin.angle = 180
spin.end = end
manager.append(spin)
render.render_anim(scene, 'animation output λ', 0, 2)
assert not list(Path('.').glob('.cuemol-pov-*'))
print('Python POV-Ray helpers completed')
'''
        requests = [
            {'jsonrpc': '2.0', 'id': 0, 'method': 'initialize', 'params': {}},
            {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': 'load_file', 'arguments': {'path': str(fixture)}}},
            {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/call', 'params': {'name': 'execute_script', 'arguments': {
                'source': 'getService("PythonBridge").runString(' + json.dumps(source) + ');'}}},
        ]
        env = dict(os.environ, CUEMOL_CONFIG_DIR=str(work / 'config'))
        if args.renderer_host:
            env['CUEMOL_EXECUTABLE'] = str(args.renderer_host.resolve())
        result = subprocess.run([str(binary), 'mcp', 'serve'], cwd=work, env=env,
                                input=''.join(json.dumps(item) + '\n' for item in requests),
                                capture_output=True, text=True, timeout=180)
        if result.returncode:
            raise RuntimeError(result.stderr)
        responses = {item['id']: item for item in map(json.loads, result.stdout.splitlines())}
        for request in requests:
            response = responses.get(request['id'])
            if response is None or response.get('error') or response.get('result', {}).get('isError'):
                raise RuntimeError(f'Python POV-Ray request failed: {response}\n{result.stderr}')
        names = ['class output λ.png', 'function output λ.png', 'blended output λ.png',
                 'animation output λ-0000.png', 'animation output λ-0001.png', 'with labels.png']
        for name in names:
            count = png_colors(work / name)
            if count < 32:
                raise RuntimeError(f'{name} has no molecular shading: {count} colors')
        if png_pixels(work / 'no labels.png') == png_pixels(work / 'with labels.png'):
            raise RuntimeError('POV pixel labels are missing')
        if png_pixels(work / 'no distance label.png') == png_pixels(work / 'distance label.png'):
            raise RuntimeError('POV distance label is missing')
        if png_pixels(work / names[0]) == png_pixels(work / names[2]):
            raise RuntimeError('post-blended renderer did not change the image')
        if png_pixels(work / names[3]) == png_pixels(work / names[4]):
            raise RuntimeError('animation frames did not change')
        if args.out:
            args.out.mkdir(parents=True, exist_ok=True)
            for path in work.glob('*.png'):
                shutil.copy2(path, args.out / path.name)
        print('Python POV-Ray class/function, name/measurement labels, post-blending, animation, paths and failure preservation passed')


if __name__ == '__main__':
    main()
