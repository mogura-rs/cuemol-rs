# /// script
# requires-python = ">=3.10"
# dependencies = ["pyautogui==0.9.54", "pillow>=11,<14"]
# ///
"""Check an actual native window using OS mouse events and captured pixels."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

import pyautogui
from PIL import Image, ImageChops, ImageGrab


class ApplicationExited(Exception):
    pass


def window_bounds(pid):
    if sys.platform == 'darwin':
        import AppKit
        import Quartz
        application = AppKit.NSRunningApplication.runningApplicationWithProcessIdentifier_(pid)
        if application is None:
            raise RuntimeError('The application has not registered with macOS yet')
        application.activateWithOptions_(AppKit.NSApplicationActivateIgnoringOtherApps)
        for window in Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly, 0):
            if window.get('kCGWindowOwnerPID') == pid and window.get('kCGWindowLayer') == 0:
                bounds = window['kCGWindowBounds']
                return [int(bounds[key]) for key in ('X', 'Y', 'Width', 'Height')]
    elif sys.platform == 'win32':
        from ctypes import wintypes
        user = ctypes.WinDLL('user32', use_last_error=True)
        user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
        user.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
        user.IsWindowVisible.argtypes = [wintypes.HWND]
        user.SetForegroundWindow.argtypes = [wintypes.HWND]
        result = []
        callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

        @callback_type
        def visit(handle, _):
            owner = wintypes.DWORD()
            user.GetWindowThreadProcessId(handle, ctypes.byref(owner))
            if owner.value == pid and user.IsWindowVisible(handle):
                rect = wintypes.RECT()
                if user.GetWindowRect(handle, ctypes.byref(rect)):
                    if rect.right - rect.left > 300 and rect.bottom - rect.top > 200:
                        user.SetForegroundWindow(handle)
                        result.append([rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top])
            return True

        user.EnumWindows(visit, 0)
        if result:
            return result[0]
    else:
        handles = subprocess.check_output(['xdotool', 'search', '--onlyvisible', '--pid', str(pid)], text=True).split()
        for handle in handles:
            geometry = subprocess.check_output(['xdotool', 'getwindowgeometry', '--shell', handle], text=True)
            values = dict(line.split('=', 1) for line in geometry.splitlines() if '=' in line)
            if int(values['WIDTH']) > 300 and int(values['HEIGHT']) > 200:
                subprocess.run(['xdotool', 'windowfocus', handle], check=True)
                return [int(values[key]) for key in ('X', 'Y', 'WIDTH', 'HEIGHT')]
    raise RuntimeError('The application has no visible window')


def wait_for(operation, timeout=30):
    deadline = time.monotonic() + timeout
    error = None
    while time.monotonic() < deadline:
        try:
            result = operation()
            if result:
                return result
        except (OSError, RuntimeError, subprocess.SubprocessError, KeyError, json.JSONDecodeError) as failure:
            error = failure
        time.sleep(0.2)
    raise RuntimeError(f'Timed out waiting for the native GUI: {error}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    binary, work = args.binary.resolve(), args.out.resolve()
    work.mkdir(parents=True, exist_ok=True)
    fixture = work / 'molecule with spaces.pdb'
    fixture.write_text(
        'HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00 10.00           C\n'
        'HETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00 10.00           O\n'
        'HETATM    3  N1  LIG A   1       1.000   1.500   0.000  1.00 10.00           N\n'
        'CONECT    1    2\nCONECT    2    3\nEND\n')
    config = work / 'config'
    config.mkdir(exist_ok=True)
    screen = pyautogui.size()
    window_width, window_height = min(1024, screen.width - 80), min(768, screen.height - 80)
    (config / 'config.tsv').write_text(
        f'window:width\t{window_width}\nwindow:height\t{window_height}\nwindow:x\t40\nwindow:y\t40\n')
    env = {key: value for key, value in os.environ.items() if not key.startswith('CUEMOL_')}
    env.update(CUEMOL_CONFIG_DIR=str(config), CUEMOL_CACHE_DIR=str(work / 'cache'))

    def call(name, arguments=None, timeout=10):
        raw = subprocess.check_output([str(binary), 'mcp', 'call', name, json.dumps(arguments or {})],
                                      cwd=work, env=env, text=True, stderr=subprocess.PIPE, timeout=timeout)
        outer = json.loads(raw)
        result = json.loads(outer['result']['content'][0]['text'])
        if not result['ok']:
            raise RuntimeError(result)
        return result['result']

    report = {'system': platform.system(), 'machine': platform.machine()}
    with (work / 'gui.log').open('w') as log:
        process = subprocess.Popen([str(binary), str(fixture)], cwd=work, env=env,
                                   stdout=log, stderr=subprocess.STDOUT)
        try:
            def loaded():
                if process.poll() is not None:
                    raise ApplicationExited(f'The application exited with status {process.returncode} before loading the fixture; see gui.log')
                state = call('get_scene_state')
                return state if state.get('objects') else None

            state = wait_for(loaded, timeout=90)
            bounds = wait_for(lambda: window_bounds(process.pid))
            x, y, width, height = bounds
            time.sleep(0.5)

            def capture(name=None):
                image = ImageGrab.grab(bbox=(x, y, x + width, y + height)).convert('RGB')
                if name:
                    image.save(work / name)
                return image

            before_image = capture('before.png')
            before = call('get_view')
            pyautogui.moveTo(x + width * 0.58, y + height * 0.43)
            pyautogui.dragRel(80, 35, duration=0.5, button='left')
            rotated = wait_for(lambda: (view if (view := call('get_view'))['rotation'] != before['rotation'] else None))
            after_image = capture('rotated.png')
            difference = ImageChops.difference(before_image, after_image)
            changed = sum(max(pixel) > 12 for pixel in difference.getdata())
            if changed < 50:
                raise RuntimeError(f'Mouse rotation changed state but only {changed} screen pixels')
            pyautogui.scroll(3)
            zoomed = wait_for(lambda: (view if (view := call('get_view'))['zoom'] != rotated['zoom'] else None))
            capture('zoomed.png')
            colored = sum(max(pixel) - min(pixel) > 70 and max(pixel) > 100 for pixel in after_image.getdata())
            if colored < 100:
                raise RuntimeError(f'The window contains too few colored pixels: {colored}')

            def script(source, timeout=10):
                return call('execute_script', {'source': source}, timeout=timeout)

            def label_pixels():
                image = capture()
                mask = Image.new('L', image.size)
                mask.putdata([255 if r > 180 and g < 60 and b > 180 else 0 for r, g, b in image.getdata()])
                return mask if mask.histogram()[255] > 100 else None

            call('set_view', {'center': [0, 0, 0], 'zoom': 8})
            script('var tree=JSON.parse(scene.getObjectTreeJSON());'
                   'tree[0].renderers.forEach(function(r){cuemol.getRenderer(r.ID).visible=false;});'
                   'var fontProbe=cuemol.getObject(tree[0].ID).createRenderer("*namelabel");'
                   'fontProbe.name="Font probe";fontProbe.font_size=32;fontProbe.font_name="Hack";'
                   'fontProbe.halign="center";fontProbe.valign="middle";'
                   'fontProbe.color=cuemol.makeColor("#ff00ff");fontProbe.label_on_top=true;'
                   'fontProbe.setLabel(2,"iiiiWW Ω");')
            mono = wait_for(label_pixels)
            capture('font-monospace.png')
            get_probe = 'var fontProbe=scene.getRendByName("Font probe");'
            script(get_probe + 'fontProbe.font_name="Ubuntu";')
            proportional = wait_for(lambda: mask if (mask := label_pixels()) is not None
                                    and ImageChops.difference(mono, mask).getbbox() else None)
            capture('font-proportional.png')
            if mono.getbbox()[2] - mono.getbbox()[0] <= proportional.getbbox()[2] - proportional.getbbox()[0]:
                raise RuntimeError('The named monospaced and proportional fonts have indistinguishable advances')
            script(get_probe + 'fontProbe.font_name="serif";fontProbe.font_style="italic";fontProbe.font_weight="bold";')
            styled = wait_for(lambda: mask if (mask := label_pixels()) is not None
                             and ImageChops.difference(proportional, mask).getbbox() else None)
            script(get_probe + 'fontProbe.setLabel(2,"Other");')
            wait_for(lambda: mask if (mask := label_pixels()) is not None
                     and ImageChops.difference(styled, mask).getbbox() else None)
            script(get_probe + 'fontProbe.font_style="ITALIC";fontProbe.font_weight="BOLD";'
                   'fontProbe.setLabel(2,"iiiiWW Ω");')
            wait_for(lambda: mask if (mask := label_pixels()) is not None
                     and ImageChops.difference(styled, mask).getbbox() is None else None)
            capture('font-styled.png')
            script(get_probe + 'fontProbe.setLabel(2,"Top\\nBottom");')
            multiline = wait_for(lambda: mask if (mask := label_pixels()) is not None
                                 and ImageChops.difference(styled, mask).getbbox() else None)
            for text in ['Top\rBottom', 'Top\r\nBottom', 'Top\n\rBottom']:
                script(get_probe + 'fontProbe.setLabel(2,"Other");')
                wait_for(lambda: mask if (mask := label_pixels()) is not None
                         and ImageChops.difference(multiline, mask).getbbox() else None)
                script(get_probe + f'fontProbe.setLabel(2,{json.dumps(text)});')
                wait_for(lambda: mask if (mask := label_pixels()) is not None
                         and ImageChops.difference(multiline, mask).getbbox() is None else None)
            capture('font-multiline.png')
            python_source = """
import json
import cuemol as cm
from cuemol import cmd, fileio, logging, mol_util, net_fetch, renderer, scene_util
from cuemol.undo_txn import UndoTxn
objects = json.loads(cm.scene().getObjectTreeJSON())
molecule = cm.obj(objects[0]['ID'])
assert isinstance(molecule, cm.getWrpClass('MolCoord'))
assert len(list(mol_util.AtomIter(molecule))) == 3
assert len(list(mol_util.ResidIter(molecule))) == 1
fileio.saveObject(molecule, 'python helper output.pdb', 'pdb',
                  {'sel': cm.sel('name O1', molecule.scene_uid)})
"""
            (work / 'python helper output.pdb').unlink(missing_ok=True)
            script(f"getService('PythonBridge').runString({json.dumps(python_source)});", timeout=60)
            exported = (work / 'python helper output.pdb').read_text().splitlines()
            atoms = [line for line in exported if line.startswith(('ATOM  ', 'HETATM'))]
            if len(atoms) != 1 or atoms[0][12:16].strip() != 'O1':
                raise RuntimeError('Python helper selection did not export exactly the oxygen atom')
            report.update(ok=True, window=bounds, atoms=state['objects'][0]['atoms'],
                          rotation_before=before['rotation'], rotation_after=rotated['rotation'],
                          zoom_before=rotated['zoom'], zoom_after=zoomed['zoom'],
                          changed_pixels=changed, colored_pixels=colored,
                          font_family_pixels_changed=True, font_style_case_equal=True,
                          font_line_endings_equal=True,
                          python_helpers=True,
                          font_pixels=styled.histogram()[255])
            print(json.dumps(report), flush=True)
        except BaseException as error:
            report.update(ok=False, error=str(error))
            raise
        finally:
            (work / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == '__main__':
    main()
