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
from PIL import ImageChops, ImageGrab


class ApplicationExited(Exception):
    pass


def window_bounds(pid):
    if sys.platform == 'darwin':
        import AppKit
        import Quartz
        application = AppKit.NSRunningApplication.runningApplicationWithProcessIdentifier_(pid)
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
    (config / 'config.tsv').write_text('window:width\t1024\nwindow:height\t768\nwindow:x\t40\nwindow:y\t40\n')
    env = {key: value for key, value in os.environ.items() if not key.startswith('CUEMOL_')}
    env.update(CUEMOL_CONFIG_DIR=str(config), CUEMOL_CACHE_DIR=str(work / 'cache'))

    def call(name, arguments=None):
        raw = subprocess.check_output([str(binary), 'mcp', 'call', name, json.dumps(arguments or {})],
                                      cwd=work, env=env, text=True, stderr=subprocess.PIPE, timeout=10)
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

            def capture(name):
                image = ImageGrab.grab(bbox=(x, y, x + width, y + height)).convert('RGB')
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
            report.update(ok=True, window=bounds, atoms=state['objects'][0]['atoms'],
                          rotation_before=before['rotation'], rotation_after=rotated['rotation'],
                          zoom_before=rotated['zoom'], zoom_after=zoomed['zoom'],
                          changed_pixels=changed, colored_pixels=colored)
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
