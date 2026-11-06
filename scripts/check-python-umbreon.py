#!/usr/bin/env python3
"""Check Python Umbreon helpers against actual exported pixels and scene state."""

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
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    pixels = runpy.run_path(str(Path(__file__).with_name("check-povray.py")))["png_pixels"]
    with tempfile.TemporaryDirectory(prefix="cuemol-python-umbreon-") as directory:
        work = Path(directory)
        fixture = work / "molecule.pdb"
        fixture.write_text(
            "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\n"
            "HETATM    2  O1  LIG A   1       1.400   0.000   0.000  1.00 10.00           O\nEND\n",
            encoding="utf-8")
        source = '''
from pathlib import Path
import cuemol as cm
from cuemol import umbreon_render as render
scene = cm.scene()
scene.saveViewToCam(scene.activeViewID, '__current')
assert scene.getAppData('render') is None
exporter = cm.strMgr().createHandler('umbreon', 2)
assert render.apply_scene_settings(scene, exporter) == 'umbreon'
assert exporter.width == 1200 and exporter.height == 1200
assert exporter.perspective == scene.getCamera('__current').perspec
assert scene.getAppData('render') is None
settings = scene.getCreateAppData('render', 'RenderSettings')
settings.width, settings.height, settings.unit = 96, 64, 'px'
settings.backend = 'umbreon'
settings.projection = 'orthographic'
settings.umbreon.useGI = False
settings.umbreon.supersample = 1
assert render.apply_scene_settings(scene, exporter) == 'umbreon'
exporter.camera = '__current'
exporter.attach(scene)
exporter.setPath('direct.png')
exporter.write()
exporter.detach()
render.render(scene, Path('helper λ.png'))
render.render(scene, 'override.png', width=80, height=48)
assert settings.width == 96 and settings.height == 64
assert scene.getAppData('render').backend == 'umbreon'
Path('preserved.png').write_bytes(b'existing output')
try:
    render.render(scene, 'preserved.png', camera='missing')
except RuntimeError as error:
    assert 'camera' in str(error)
else:
    raise AssertionError('missing camera accepted')
assert Path('preserved.png').read_bytes() == b'existing output'
settings.backend = 'umbreon_npr'
settings.umbreon_npr.supersample = 1
assert render.apply_scene_settings(scene, exporter) == 'umbreon_npr'
render.render(scene, 'npr.png')
settings.backend = 'povray'
assert render.apply_scene_settings(scene, exporter) == 'umbreon'
settings.backend = 'umbreon'
writer = cm.createObj('SceneXMLWriter')
writer.attach(scene)
writer.setPath('saved scene λ.qsc')
writer.write()
writer.detach()
loaded = render.render_file(Path('saved scene λ.qsc'), Path('reloaded.png'))
assert cm.isscene(loaded) and loaded.hasCamera('__current')
assert render.main(['saved scene λ.qsc', 'main.png', '-W', '80', '-H', '48']) == 0
print('Python Umbreon helpers completed')
'''
        calls = [("load_file", {"path": str(fixture)}),
                 ("execute_script", {"source": "getService('PythonBridge').runString(" + json.dumps(source) + ");"})]
        requests = [{"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}}]
        requests.extend({"jsonrpc": "2.0", "id": index, "method": "tools/call",
                         "params": {"name": name, "arguments": arguments}}
                        for index, (name, arguments) in enumerate(calls, 1))
        env = dict(os.environ, CUEMOL_CONFIG_DIR=str(work / "config"))
        result = subprocess.run([str(binary), "mcp", "serve"], cwd=work, env=env,
                                input="".join(json.dumps(item) + "\n" for item in requests),
                                capture_output=True, encoding="utf-8", timeout=180)
        if result.returncode:
            raise RuntimeError(result.stderr)
        responses = {item["id"]: item for item in map(json.loads, result.stdout.splitlines())}
        for request in requests:
            response = responses.get(request["id"])
            if response is None or response.get("error") or response.get("result", {}).get("isError"):
                raise RuntimeError(f"Python Umbreon request failed: {response}\n{result.stderr}")
        direct = pixels(work / "direct.png", (96, 64))
        channels, raw = direct[2:]
        if len({raw[i:i + channels] for i in range(0, len(raw), channels)}) < 32:
            raise RuntimeError("Umbreon image lacks molecular shading")
        for name in ("helper λ.png", "reloaded.png"):
            if pixels(work / name, (96, 64)) != direct:
                raise RuntimeError(f"{name} does not match the direct exporter")
        if pixels(work / "npr.png", (96, 64)) == direct:
            raise RuntimeError("NPR settings did not change the image")
        if pixels(work / "override.png", (80, 48)) != pixels(work / "main.png", (80, 48)):
            raise RuntimeError("Size override differs after scene reload")
        if args.out:
            args.out.mkdir(parents=True, exist_ok=True)
            for path in work.glob("*.png"):
                if path.name != "preserved.png":
                    shutil.copy2(path, args.out / path.name)
        print("Python Umbreon defaults, stored settings, NPR, overrides, scene reload, pixels and errors passed")


if __name__ == "__main__":
    main()
