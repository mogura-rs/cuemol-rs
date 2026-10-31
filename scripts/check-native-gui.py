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
from PIL import Image, ImageChops, ImageGrab, ImageStat


class ApplicationExited(Exception):
    pass


def desktop_work_area():
    if sys.platform == 'darwin':
        import AppKit
        screen = AppKit.NSScreen.screens()[0]
        frame, visible = screen.frame(), screen.visibleFrame()
        return [round(visible.origin.x),
                round(frame.origin.y + frame.size.height - visible.origin.y - visible.size.height),
                round(visible.size.width), round(visible.size.height)]
    if sys.platform == 'win32':
        from ctypes import wintypes
        user = ctypes.WinDLL('user32', use_last_error=True)
        user.SystemParametersInfoW.argtypes = [wintypes.UINT, wintypes.UINT,
                                              ctypes.POINTER(wintypes.RECT), wintypes.UINT]
        user.SystemParametersInfoW.restype = wintypes.BOOL
        area = wintypes.RECT()
        if not user.SystemParametersInfoW(0x0030, 0, ctypes.byref(area), 0):
            raise ctypes.WinError(ctypes.get_last_error())
        return [area.left, area.top, area.right - area.left, area.bottom - area.top]
    screen = pyautogui.size()
    return [0, 0, screen.width, screen.height]


def window_bounds(pid, content=False):
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
                x, y, width, height = [int(bounds[key]) for key in ('X', 'Y', 'Width', 'Height')]
                if content:
                    style = (AppKit.NSWindowStyleMaskTitled | AppKit.NSWindowStyleMaskClosable
                             | AppKit.NSWindowStyleMaskMiniaturizable | AppKit.NSWindowStyleMaskResizable)
                    rect = AppKit.NSWindow.contentRectForFrameRect_styleMask_(
                        AppKit.NSMakeRect(0, 0, width, height), style)
                    return [round(x + rect.origin.x),
                            round(y + height - rect.origin.y - rect.size.height),
                            round(rect.size.width), round(rect.size.height)]
                return [x, y, width, height]
    elif sys.platform == 'win32':
        from ctypes import wintypes
        user = ctypes.WinDLL('user32', use_last_error=True)
        user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
        user.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
        user.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
        user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
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
                        if content:
                            origin = wintypes.POINT()
                            if not (user.GetClientRect(handle, ctypes.byref(rect))
                                    and user.ClientToScreen(handle, ctypes.byref(origin))):
                                return True
                            result.append([origin.x, origin.y, rect.right, rect.bottom])
                        else:
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


def clipboard_text():
    if sys.platform == 'darwin':
        command = ['pbpaste']
    elif sys.platform == 'win32':
        command = ['powershell', '-NoProfile', '-Command', 'Get-Clipboard -Raw']
    else:
        command = ['xclip', '-selection', 'clipboard', '-o']
    return subprocess.check_output(command, text=True, timeout=5).rstrip('\r\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--selection-pane-only', action='store_true')
    parser.add_argument('--cross-selection-only', action='store_true')
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
    available_x, available_y, available_width, available_height = desktop_work_area()
    window_width = min(1024, available_width - 80)
    window_height = min(768, available_height - 120)
    window_x, window_y = available_x + 40, available_y + 40
    (config / 'config.tsv').write_text(
        f'window:width\t{window_width}\nwindow:height\t{window_height}\n'
        f'window:x\t{window_x}\nwindow:y\t{window_y}\n')
    env = {key: value for key, value in os.environ.items() if not key.startswith('CUEMOL_')}
    env.update(CUEMOL_CONFIG_DIR=str(config), CUEMOL_CACHE_DIR=str(work / 'cache'))

    def call(name, arguments=None, timeout=10):
        response = subprocess.run([str(binary), 'mcp', 'call', name, json.dumps(arguments or {})],
                                  cwd=work, env=env, encoding='utf-8', capture_output=True, timeout=timeout)
        if response.returncode:
            raise RuntimeError(f'{name} failed: {response.stdout}\n{response.stderr}')
        outer = json.loads(response.stdout)
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
            outer_x, outer_y, outer_width, outer_height = bounds
            if (outer_x < available_x or outer_y < available_y
                    or outer_x + outer_width > available_x + available_width
                    or outer_y + outer_height > available_y + available_height):
                raise RuntimeError(f'The application window {bounds} exceeds the desktop work area')
            client = wait_for(lambda: window_bounds(process.pid, content=True))
            x, y, width, height = client
            if [width, height] != [window_width, window_height]:
                raise RuntimeError(f'The client area {client} differs from the requested size')
            report.update(window=bounds, client=client)
            time.sleep(0.5)

            def capture(name=None):
                image = ImageGrab.grab(bbox=(x, y, x + width, y + height)).convert('RGB')
                if name:
                    image.save(work / name)
                return image

            if args.cross_selection_only:
                probe = state['objects'][0]
                target = call('load_file', {'path': str(fixture)})['objectId']
                call('rename_node', {'nodeType': 'object', 'nodeId': target, 'name': 'Target'})
                call('set_visible', {'nodeType': 'object', 'nodeId': target, 'visible': False})
                for renderer in probe['renderers']:
                    call('delete_node', {'nodeType': 'renderer', 'nodeId': renderer['id']})
                expression = 'name C1 around 2.1 [Target]'
                call('create_renderer', {'objId': probe['id'], 'rendererType': 'cpk',
                                         'name': 'cross', 'selection': expression})
                call('set_projection', {'perspective': False})
                call('set_view', {'center': [-1, 0, 0], 'zoom': 12, 'distance': 200, 'slab': 50})
                pyautogui.moveTo(x + 5, y + 5)
                crop = (int(width * .32), int(height * .18), int(width * .95), int(height * .70))

                def count():
                    return call('count_selection', {'objId': probe['id'], 'selection': expression})['count']

                def shift(value):
                    call('execute_script', {'source':
                        f"var offset = cuemol.createObj('Vector'); offset.x = {value}; "
                        "var matrix = cuemol.createObj('Matrix'); matrix.setTranslate(offset); "
                        f"cuemol.getObject({target}).xformMat = matrix;"})

                if count() != 2:
                    raise RuntimeError('The cross-molecule selection did not select two atoms')
                time.sleep(.3)
                before = capture('cross-selected.png').crop(crop)
                shift(20)
                if count() != 0:
                    raise RuntimeError('The selection retained atoms after its target moved')
                wait_for(lambda: ImageChops.difference(before, capture().crop(crop)).getbbox())
                empty = capture('cross-target-moved.png').crop(crop)
                changed = sum(max(pixel) > 12 for pixel in ImageChops.difference(before, empty).getdata())
                if changed < 100:
                    raise RuntimeError(f'Target movement changed only {changed} viewport pixels')
                shift(0)
                if count() != 2:
                    raise RuntimeError('The selection did not update after restoring its target')
                wait_for(lambda: ImageChops.difference(before, capture().crop(crop)).getbbox() is None)
                capture('cross-restored.png')
                report.update(ok=True, window=bounds, cross_selection_live=True,
                              cross_selection_changed_pixels=changed, cross_selection_restored_pixels=True)
                print(json.dumps(report), flush=True)
                return

            def check_selection_pane():
                primary = 'command' if sys.platform == 'darwin' else 'ctrl'
                object_id = call('get_scene_state')['objects'][0]['id']
                dy = (height - 768) * 0.48

                def selection():
                    return call('get_prop', {'path': f'#{object_id}.sel'})['value']

                def input_text(text):
                    pyautogui.click(x + 120, y + 512 + dy)
                    pyautogui.hotkey(primary, 'a')
                    pyautogui.write(text, interval=0.02)

                call('set_mol_selection', {'objId': object_id, 'selection': 'name O1'})
                call('center_view', {'objId': object_id, 'selection': 'name O1', 'zoom': False})
                expected_center = call('get_view')['center']
                call('set_view', {'center': [40, 50, 60]})
                pyautogui.click(x + 22, y + 176)
                time.sleep(0.3)
                input_text('name C1')
                pyautogui.click(x + 22, y + 88)
                pyautogui.click(x + 22, y + 176)
                pyautogui.click(x + 112, y + 540 + dy)
                wait_for(lambda: call('get_view')['center'] == expected_center)
                if selection() != 'name O1':
                    raise RuntimeError('Center applied an uncommitted selection draft')
                pyautogui.click(x + 80, y + 540 + dy)
                wait_for(lambda: selection() == 'name C1')
                pyautogui.hotkey(primary, 'z')
                wait_for(lambda: selection() == 'name O1')
                pyautogui.hotkey(primary, 'shift', 'z')
                wait_for(lambda: selection() == 'name C1')
                capture('selection-pane-applied.png')
                pyautogui.click(x + 144, y + 540 + dy)
                wait_for(lambda: selection() == '')
                input_text('name C1')
                pyautogui.click(x + 176, y + 540 + dy)
                time.sleep(0.2)
                pyautogui.click(x + 250, y + 569 + dy)
                pyautogui.write('os_carbon', interval=0.03)
                pyautogui.press('enter')
                wait_for(lambda: 'os_carbon' in call('get_scene_state')['namedSelections'])
                if call('count_selection', {'objId': object_id, 'selection': 'os_carbon'})['count'] != 1:
                    raise RuntimeError('The named selection does not match its definition')
                if selection() != '':
                    raise RuntimeError('Defining a name changed the applied selection')
                input_text('os_carbon')
                pyautogui.press('enter')
                wait_for(lambda: selection() == 'os_carbon')
                capture('selection-pane-named.png')
                time.sleep(0.3)
                pyautogui.click(x + 195, y + 596 + dy)
                time.sleep(0.2)
                capture('selection-pane-term.png')
                scroll_offset = 0
                if 687 + dy >= height - 24:
                    pyautogui.moveTo(x + 275, y + height - 60)
                    time.sleep(0.2)
                    tab_top = round(596 + dy) - 8
                    marker = capture().crop((179, tab_top, 209, tab_top + 16))
                    pyautogui.scroll(-2)
                    time.sleep(0.5)
                    scrolled = capture('selection-pane-scrolled.png')
                    scores = []
                    for offset in range(-min(160, tab_top), 0):
                        candidate = scrolled.crop((179, tab_top + offset, 209, tab_top + offset + 16))
                        difference = ImageStat.Stat(ImageChops.difference(marker, candidate))
                        scores.append((sum(difference.mean) / 3, offset))
                    score, scroll_offset = min(scores)
                    if score > 2:
                        raise RuntimeError('The Selection tab could not be located after scrolling')
                pyautogui.click(x + 100, y + 659 + dy + scroll_offset)
                pyautogui.write('A')
                pyautogui.click(x + 120, y + 687 + dy + scroll_offset)
                wait_for(lambda: selection() == "'A'.*.*")
                if call('count_selection', {'objId': object_id, 'selection': selection()})['count'] != 3:
                    raise RuntimeError('The live selection builder did not select the fixture chain')
                capture('selection-pane-builder.png')
                return {'selection_pane_drafts': True, 'selection_pane_center': True,
                        'selection_pane_undo': True, 'selection_pane_named': True,
                        'selection_pane_live_builder': True}

            if args.selection_pane_only:
                report.update(check_selection_pane(), ok=True, window=bounds)
                print(json.dumps(report), flush=True)
                return

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

            pyautogui.moveTo(x + width * 0.58, y + height * 0.43)
            pyautogui.keyDown('shift')
            try:
                pyautogui.dragRel(80, 0, duration=0.5, button='left')
            finally:
                pyautogui.keyUp('shift')
            clipped = wait_for(lambda: view if (view := call('get_view'))['slab'] != zoomed['slab'] else None)
            if abs(clipped['zoom'] - zoomed['zoom']) > 1e-6:
                raise RuntimeError('Horizontal Shift-drag changed zoom instead of only the slab')
            capture('slab.png')
            call('set_mol_selection', {'objId': state['objects'][0]['id'], 'selection': 'name O1'})
            call('set_view', {'center': [4, 5, 6]})
            pyautogui.press('c')
            centered = wait_for(lambda: view if (view := call('get_view'))['center'] == [0, 0, 0] else None)
            capture('centered.png')

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
            mono_width = mono.getbbox()[2] - mono.getbbox()[0]
            proportional = wait_for(lambda: mask if (mask := label_pixels()) is not None
                                    and mask.getbbox()[2] - mask.getbbox()[0] < mono_width
                                    and ImageChops.difference(mono, mask).getbbox() else None)
            capture('font-proportional.png')
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
            pyautogui.click(x + width * 0.52, y + height - 36)
            qax = work / 'console-actions.qax'
            qax.unlink(missing_ok=True)
            pyautogui.write('open_log console-actions.qax', interval=0.015)
            pyautogui.press('enter')
            wait_for(qax.is_file)
            pyautogui.write('rename "molecule with spaces", console-probe', interval=0.015)
            pyautogui.press('enter')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'console-probe')
            pyautogui.write('select console-p', interval=0.02)
            pyautogui.press('tab')
            pyautogui.write(', name N1', interval=0.02)
            pyautogui.press('enter')
            wait_for(lambda: call('get_prop', {'path': f"#{state['objects'][0]['id']}.sel"})['value'] == 'name N1')
            pyautogui.write('rename console-probe, first', interval=0.015)
            pyautogui.hotkey('shift', 'enter')
            if call('get_scene_state')['objects'][0]['name'] != 'console-probe':
                raise RuntimeError('Shift-Enter submitted a partial console command')
            pyautogui.write('rename first, final', interval=0.015)
            pyautogui.press('enter')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'final')
            pyautogui.write('close_log', interval=0.02)
            pyautogui.press('enter')
            wait_for(lambda: 'final' in qax.read_text())
            pyautogui.write('undo', interval=0.02)
            pyautogui.press('enter')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'console-probe')
            capture('console-multiline.png')
            pyautogui.click(x + width * 0.7, y + height * 0.4)
            primary = 'command' if sys.platform == 'darwin' else 'ctrl'
            pyautogui.hotkey(primary, 'shift', 'z')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'final')
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'console-probe')

            original = call('get_scene_state')['objects'][0]
            pyautogui.click(x + 145, y + 164)
            pyautogui.hotkey(primary, 'c')
            pyautogui.hotkey(primary, 'x')
            wait_for(lambda: not call('get_scene_state')['objects'])
            pyautogui.hotkey(primary, 'v')
            pasted = wait_for(lambda: call('get_scene_state')['objects'])
            if len(pasted) != 1 or pasted[0]['id'] == original['id'] or pasted[0]['atoms'] != original['atoms']:
                raise RuntimeError('The OS clipboard did not restore the copied molecule')
            capture('clipboard-pasted.png')
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: not call('get_scene_state')['objects'])
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: call('get_scene_state')['objects'][0]['id'] == original['id'])

            pyautogui.click(x + width * 0.52, y + height - 36)
            command = 'rename console-probe, clipboard-probe'
            pyautogui.write(command, interval=0.015)
            pyautogui.hotkey(primary, 'a')
            pyautogui.hotkey(primary, 'c')
            wait_for(lambda: clipboard_text() == command)
            pyautogui.hotkey(primary, 'x')
            pyautogui.hotkey(primary, 'v')
            pyautogui.press('enter')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'clipboard-probe')
            pyautogui.write('undo', interval=0.015)
            pyautogui.press('enter')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'console-probe')
            pyautogui.click(x + width * 0.7, y + height * 0.4)

            def palette_collapsed():
                settings = dict(line.split('\t', 1) for line in (config / 'config.tsv').read_text().splitlines() if '\t' in line)
                return settings.get('layout:tool-palette-collapsed') == 'true'

            initial_palette = palette_collapsed()
            pyautogui.hotkey(primary, 'shift', 'p')
            wait_for(lambda: palette_collapsed() != initial_palette)
            capture('palette-shortcut.png')
            pyautogui.hotkey(primary, 'shift', 'p')
            wait_for(lambda: palette_collapsed() == initial_palette)

            pyautogui.click(x + 145, y + 164)
            time.sleep(0.2)
            pyautogui.click(x + width - 194, y + 149)

            def inspector_filter(text):
                def focused():
                    image = capture().crop((width - 290, 164, width - 10, 184))
                    return sum(b - r > 30 and g - r > 10 for r, g, b in image.getdata()) > 200

                pyautogui.click(x + width - 150, y + 174)
                wait_for(focused)
                pyautogui.hotkey(primary, 'a')
                pyautogui.write(text, interval=0.02)

                def copied_filter():
                    pyautogui.hotkey(primary, 'a')
                    pyautogui.hotkey(primary, 'c')
                    copied = clipboard_text()
                    if copied != text:
                        raise RuntimeError(f'The property filter copied {copied[:80]!r}, expected {text!r}')
                    return True

                wait_for(copied_filter)
                capture(f'inspector-filter-{text}.png')
                pyautogui.press('tab')
                wait_for(lambda: not focused())

            inspector_filter('name')
            capture('inspector-name-before.png')
            pyautogui.click(x + width - 100, y + 225)
            pyautogui.hotkey(primary, 'a')
            pyautogui.write('inspector-probe', interval=0.02)
            pyautogui.press('enter')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'inspector-probe')
            capture('inspector-name.png')
            pyautogui.click(x + width * 0.6, y + height * 0.4)
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'console-probe')
            pyautogui.hotkey(primary, 'shift', 'z')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'inspector-probe')
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: call('get_scene_state')['objects'][0]['name'] == 'console-probe')

            inspector_filter('locked')
            pyautogui.click(x + width - 168, y + 225)
            locked_path = f"#{original['id']}.locked"
            wait_for(lambda: call('get_prop', {'path': locked_path})['value'] == 'true')
            capture('inspector-locked.png')
            pyautogui.click(x + width - 20, y + 225)
            wait_for(lambda: call('get_prop', {'path': locked_path})['value'] == 'false')
            pyautogui.click(x + width * 0.6, y + height * 0.4)
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: call('get_prop', {'path': locked_path})['value'] == 'true')
            pyautogui.hotkey(primary, 'shift', 'z')
            wait_for(lambda: call('get_prop', {'path': locked_path})['value'] == 'false')
            capture('inspector-reset.png')

            pyautogui.click(x + 145, y + 140)
            time.sleep(0.2)
            inspector_filter('name')
            pyautogui.click(x + width - 100, y + 225)
            pyautogui.hotkey(primary, 'a')
            pyautogui.write('Inspector scene', interval=0.02)
            pyautogui.press('enter')
            wait_for(lambda: call('get_prop', {'path': 'name'})['value'] == 'Inspector scene')
            if call('get_scene_state')['objects'][0]['name'] != 'console-probe':
                raise RuntimeError('The Scene inspector edited the selected object')
            capture('inspector-scene.png')

            pyautogui.click(x + 180, y + 13)
            time.sleep(0.2)
            capture('inspector-view-menu.png')
            pyautogui.click(x + 240, y + 228)
            time.sleep(0.2)
            inspector_filter('zoom')
            before_inspector_zoom = call('get_view')['zoom']
            pyautogui.moveTo(x + width * 0.6, y + height * 0.4)
            pyautogui.scroll(2)
            inspector_zoom = wait_for(lambda: zoom if (zoom := call('get_view')['zoom']) != before_inspector_zoom else None)
            pyautogui.doubleClick(x + width - 100, y + 225, interval=0.1)
            pyautogui.hotkey(primary, 'a')
            pyautogui.hotkey(primary, 'c')
            def copied_zoom():
                text = clipboard_text()
                try:
                    return abs(float(text) - inspector_zoom) <= 0.01
                except ValueError:
                    return False
            wait_for(copied_zoom)
            pyautogui.write('37.25', interval=0.1)
            pyautogui.moveTo(x + width * 0.6, y + height * 0.4)
            pyautogui.scroll(-2)
            wait_for(lambda: call('get_view')['zoom'] != inspector_zoom)
            pyautogui.hotkey(primary, 'a')
            pyautogui.hotkey(primary, 'c')
            wait_for(lambda: clipboard_text() == '37.25')
            pyautogui.press('enter')
            wait_for(lambda: abs(call('get_view')['zoom'] - 37.25) < 0.001)
            inspector_filter('name')
            pyautogui.click(x + width - 100, y + 225)
            pyautogui.hotkey(primary, 'a')
            pyautogui.write('Orbit view', interval=0.02)
            pyautogui.moveTo(x + width * 0.6, y + height * 0.4)
            pyautogui.scroll(2)
            wait_for(lambda: abs(call('get_view')['zoom'] - 37.25) > 0.01)
            pyautogui.hotkey(primary, 'a')
            pyautogui.hotkey(primary, 'c')
            wait_for(lambda: clipboard_text() == 'Orbit view')
            pyautogui.press('enter')
            wait_for(lambda: call('get_prop', {'path': 'view.name'})['value'] == 'Orbit view')
            capture('inspector-view.png')
            script('var tree=JSON.parse(scene.getObjectTreeJSON());'
                   'var obj=cuemol.getObject(tree[0].ID);'
                   'tree[0].renderers.forEach(function(r){obj.destroyRenderer(r.ID);});'
                   'var axes=obj.createRenderer("tube");axes.name="Tube axes";'
                   'axes.section.width=1;axes.section.tuber=0.5;')
            axes_id = next(renderer['id'] for renderer in call('get_scene_state')['objects'][0]['renderers']
                           if renderer['name'] == 'Tube axes')
            pyautogui.click(x + 175, y + 188)
            time.sleep(0.2)
            pyautogui.click(x + width - 255, y + 149)
            inspector_filter('section.width')
            capture('inspector-axes-before.png')
            call('clear_undo')
            pyautogui.doubleClick(x + width - 100, y + 225, interval=0.1)
            pyautogui.hotkey(primary, 'a')
            pyautogui.write('2')
            pyautogui.press('enter')
            def axis(key):
                return call('get_prop', {'path': f'#{axes_id}.section.{key}'})['value']
            wait_for(lambda: axis('width') == '2' and axis('tuber') == '0.25')
            inspector_filter('section.tuber')
            pyautogui.doubleClick(x + width - 100, y + 225, interval=0.1)
            pyautogui.hotkey(primary, 'a')
            pyautogui.hotkey(primary, 'c')
            def copied_axis():
                try:
                    return float(clipboard_text()) == 0.5
                except ValueError:
                    return False
            wait_for(copied_axis)
            pyautogui.click(x + width * 0.6, y + height * 0.4)
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: axis('width') == '1' and axis('tuber') == '0.5')
            pyautogui.hotkey(primary, 'shift', 'z')
            wait_for(lambda: axis('width') == '2' and axis('tuber') == '0.25')
            capture('inspector-axes.png')
            script(f'var obj=cuemol.getObject(JSON.parse(scene.getObjectTreeJSON())[0].ID);'
                   f'obj.destroyRenderer({axes_id});'
                   'var loops=obj.createRenderer("disorder");loops.name="Optional loops";')
            loops_id = next(renderer['id'] for renderer in call('get_scene_state')['objects'][0]['renderers']
                            if renderer['name'] == 'Optional loops')
            pyautogui.click(x + 175, y + 188)
            time.sleep(0.2)
            inspector_filter('loopsize2')
            def loop_size():
                return call('get_prop', {'path': f'#{loops_id}.loopsize2'})['value']
            def optional_editor():
                return capture().crop((width - 160, 214, width - 36, 237))
            def copied_number(expected):
                pyautogui.hotkey(primary, 'a')
                pyautogui.hotkey(primary, 'c')
                text = clipboard_text()
                try:
                    matches = float(text) == expected
                except ValueError:
                    matches = False
                if not matches:
                    raise RuntimeError(f'The numeric editor copied {text[:80]!r}, expected {expected}')
                return True
            disabled_editor = optional_editor()
            pyautogui.click(x + width - 174, y + 225)
            wait_for(lambda: loop_size() == '2')
            wait_for(lambda: ImageChops.difference(disabled_editor, optional_editor()).getbbox())
            capture('inspector-optional-enabled.png')
            pyautogui.doubleClick(x + width - 100, y + 225, interval=0.1)
            wait_for(lambda: copied_number(2.0))
            pyautogui.write('3.4', interval=0.1)
            wait_for(lambda: copied_number(3.4))
            capture('inspector-optional-draft.png')
            if loop_size() != '2':
                raise RuntimeError('The optional numeric draft changed the property before commit')
            pyautogui.press('enter')
            wait_for(lambda: loop_size() == '3.4')
            pyautogui.click(x + width - 174, y + 225)
            wait_for(lambda: loop_size() == '-1')
            pyautogui.click(x + width - 174, y + 225)
            wait_for(lambda: loop_size() == '3.4')
            pyautogui.click(x + width * 0.6, y + height * 0.4)
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: loop_size() == '-1')
            pyautogui.hotkey(primary, 'shift', 'z')
            wait_for(lambda: loop_size() == '3.4')
            capture('inspector-optional.png')
            script(f'var obj=cuemol.getObject(JSON.parse(scene.getObjectTreeJSON())[0].ID);'
                   f'obj.destroyRenderer({loops_id});'
                   'var labels=obj.createRenderer("*namelabel");labels.name="Font choices";'
                   'labels.font_name="Ubuntu";labels.font_size=32;'
                   'labels.setLabel(2,"iiiiWW");labels.label_on_top=true;')
            labels_id = next(renderer['id'] for renderer in call('get_scene_state')['objects'][0]['renderers']
                             if renderer['name'] == 'Font choices')
            pyautogui.click(x + 175, y + 188)
            time.sleep(0.2)
            inspector_filter('font_name')
            pyautogui.click(x + width - 100, y + 225)
            time.sleep(0.2)
            capture('inspector-font-options.png')
            pyautogui.press('escape')
            def label_font():
                return call('get_prop', {'path': f'#{labels_id}.font_name'})['value']
            if label_font() != 'Ubuntu':
                raise RuntimeError('Closing the font picker changed its value')
            pyautogui.click(x + width - 100, y + 225)
            time.sleep(0.2)
            pyautogui.click(x + width - 100, y + 304)
            wait_for(lambda: label_font() == 'monospace')
            capture('inspector-font-selected.png')
            pyautogui.click(x + width * 0.6, y + height * 0.4)
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: label_font() == 'Ubuntu')
            pyautogui.hotkey(primary, 'shift', 'z')
            wait_for(lambda: label_font() == 'monospace')
            properties = call('list_node_props', {'nodeType': 'renderer', 'nodeId': labels_id})['properties']
            if not next(row for row in properties if row['key'] == 'sel')['readonly']:
                raise RuntimeError('The label renderer exposes an editable unsupported selection')
            script(f'var obj=cuemol.getObject(JSON.parse(scene.getObjectTreeJSON())[0].ID);'
                   f'obj.destroyRenderer({labels_id});'
                   'var builder=obj.createRenderer("tube");builder.name="Selection builder";')
            builder_id = next(renderer['id'] for renderer in call('get_scene_state')['objects'][0]['renderers']
                              if renderer['name'] == 'Selection builder')
            pyautogui.click(x + 175, y + 188)
            time.sleep(0.2)
            inspector_filter('sel')
            def renderer_selection():
                return call('get_prop', {'path': f'#{builder_id}.sel'})['value']
            initial_selection = renderer_selection()
            call('clear_undo')
            wait_for(lambda: window_bounds(process.pid))
            pyautogui.click(x + width - 47, y + 225)

            def selection_popup_visible():
                image = capture().crop((width - 340, 245, width - 315, 290))
                return sum(min(pixel) > 20 for pixel in image.getdata()) > image.width * image.height * 0.8

            wait_for(selection_popup_visible)
            capture('inspector-selection-open.png')
            pyautogui.click(x + width - 215, y + 272)
            time.sleep(0.2)
            pyautogui.click(x + width - 280, y + 309)
            time.sleep(0.2)
            capture('inspector-selection-keywords.png')
            pyautogui.click(x + width - 280, y + 415)
            time.sleep(0.2)
            pyautogui.click(x + width - 250, y + 335)
            pyautogui.write('A')
            pyautogui.click(x + width - 275, y + 362)
            time.sleep(0.2)
            capture('inspector-selection-draft.png')
            if renderer_selection() != initial_selection:
                raise RuntimeError('A selection-builder draft changed the renderer before commit')
            pyautogui.click(x + width * 0.5, y + height * 0.5)
            wait_for(lambda: renderer_selection() == "chain 'A'")
            capture('inspector-selection-applied.png')
            pyautogui.hotkey(primary, 'z')
            wait_for(lambda: renderer_selection() == initial_selection)
            pyautogui.hotkey(primary, 'shift', 'z')
            wait_for(lambda: renderer_selection() == "chain 'A'")
            report.update(check_selection_pane())
            report.update(ok=True, window=bounds,
                          work_area=[available_x, available_y, available_width, available_height],
                          window_contained=True, atoms=state['objects'][0]['atoms'],
                          rotation_before=before['rotation'], rotation_after=rotated['rotation'],
                          zoom_before=rotated['zoom'], zoom_after=zoomed['zoom'],
                          slab_before=zoomed['slab'], slab_after=clipped['slab'],
                          centered_selection=centered['center'],
                          changed_pixels=changed, colored_pixels=colored,
                          font_family_pixels_changed=True, font_style_case_equal=True,
                          font_line_endings_equal=True,
                          python_helpers=True, console_input=True, console_completion=True,
                          console_multiline_undo=True, console_qax=True,
                          clipboard_cut_paste_undo=True,
                          clipboard_text_external=True,
                          shell_shortcuts=True,
                          inspector_text_undo=True, inspector_boolean_reset=True,
                          inspector_scene_target=True, inspector_view_navigation=True,
                          inspector_numeric_draft=True,
                          inspector_text_draft=True,
                          inspector_derived_axes=True,
                          inspector_optional_value=True,
                          inspector_optional_input_acknowledged=True,
                          inspector_filter_input_acknowledged=True,
                          inspector_font_choices=True,
                          inspector_selection_popup_acknowledged=True,
                          inspector_selection_builder=True,
                          font_pixels=styled.histogram()[255])
            print(json.dumps(report), flush=True)
        except BaseException as error:
            report.update(ok=False, error=str(error))
            try:
                capture('failure.png')
            except Exception:
                pass
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
