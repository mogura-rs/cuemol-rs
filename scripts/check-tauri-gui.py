# /// script
# requires-python = ">=3.10"
# dependencies = ["pyautogui==0.9.54", "pillow>=11,<14", "pyobjc-framework-ApplicationServices; sys_platform == 'darwin'"]
# ///
"""Exercise a macOS Tauri window using OS input and captured label pixels."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import runpy
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET

import pyautogui
from PIL import Image, ImageChops, ImageGrab


class MacControls:
    def __init__(self, pid):
        import ApplicationServices as AX
        if not AX.AXIsProcessTrusted():
            raise RuntimeError("The GUI check needs macOS accessibility permission")
        self.ax = AX
        self.application = AX.AXUIElementCreateApplication(pid)
        AX.AXUIElementSetAttributeValue(self.application, "AXManualAccessibility", True)
        AX.AXUIElementSetAttributeValue(self.application, "AXEnhancedUserInterface", True)

    def attribute(self, element, name):
        error, value = self.ax.AXUIElementCopyAttributeValue(element, name, None)
        return None if error else value

    def elements(self):
        pending = [self.application]
        for _ in range(4000):
            if not pending:
                return
            element = pending.pop()
            yield element
            pending.extend(reversed(self.attribute(element, "AXChildren") or []))
        raise RuntimeError("The accessibility tree exceeded the check's traversal bound")

    def labels(self, element):
        return [str(value) for name in ("AXTitle", "AXDescription", "AXValue", "AXPlaceholderValue")
                if (value := self.attribute(element, name)) is not None]

    def find(self, role, label, prefix=False):
        for element in self.elements():
            if self.attribute(element, "AXRole") != role:
                continue
            if self.attribute(element, "AXEnabled") is False:
                continue
            if any(value.startswith(label) if prefix else value == label
                   for value in self.labels(element)):
                return element
        return None

    def click(self, element, double=False):
        self.ax.AXUIElementPerformAction(element, "AXScrollToVisible")
        position = self.attribute(element, "AXPosition")
        size = self.attribute(element, "AXSize")
        valid_point, point = self.ax.AXValueGetValue(position, self.ax.kAXValueCGPointType, None)
        valid_size, size = self.ax.AXValueGetValue(size, self.ax.kAXValueCGSizeType, None)
        if not valid_point or not valid_size or size.width <= 0 or size.height <= 0:
            raise RuntimeError("The accessibility control has no clickable bounds")
        pyautogui.click(point.x + size.width / 2, point.y + size.height / 2,
                        clicks=2 if double else 1, interval=0.12)

    def fill(self, element, value):
        self.click(element)
        pyautogui.hotkey("command", "a")
        pyautogui.write(value, interval=0.02)

    def describe(self):
        return [{"role": self.attribute(element, "AXRole"), "labels": self.labels(element)}
                for element in self.elements() if self.labels(element)]


def prepare_scene(cli, work, env):
    molecule = work / "molecule with spaces.pdb"
    molecule.write_text(
        "HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00 10.00           C\n"
        "HETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00 10.00           O\n"
        "HETATM    3  N1  LIG A   1       1.000   1.500   1.000  1.00 10.00           N\n"
        "CONECT    1    2\nCONECT    2    3\nEND\n")
    scene = work / "scene with spaces.qsc"
    style_path = work / "linked palette.xml"
    style_path.write_text('<styles id="Probe palette"><color id="accent" value="#123456"/>'
                          '<sel id="focus" value="name C1"/></styles>')
    source = f"""
var reader = cuemol.getService('StreamManager').createHandler('pdb', 0);
var object = reader.createDefaultObj();
reader.attach(object); reader.path = {json.dumps(str(molecule))}; reader.read(); reader.detach();
scene.addObject(object); object.name = 'Tauri probe';
object.createRenderer('ballstick');
var label = object.createRenderer('*namelabel');
label.name = 'probe-label';
label.font_name = 'serif'; label.font_size = 26;
label.font_weight = 'BOLD'; label.font_style = 'ITALIC';
label.color = cuemol.makeColor('#ff00ff'); label.label_on_top = true;
label.setLabel(3, 'Top\\rBottom');
var camera = cuemol.Camera(); camera.zoom = 9; camera.distance = 40; camera.slab = 20;
scene.setCamera('__current', camera);
cuemol.getService('StyleManager').loadStyleSetFromFile(scene.uid, {json.dumps(str(style_path))}, false);
var writer = cuemol.SceneXMLWriter(); writer.embedAll = false;
writer.attach(scene); writer.path = {json.dumps(str(scene))}; writer.write(); writer.detach();
print(object.getAtomSize());
"""
    output = subprocess.check_output([str(cli), "-c", source], cwd=work, env=env, text=True)
    if output.strip().splitlines()[-1] != "3" or not scene.is_file():
        raise RuntimeError(f"Cannot prepare the three-atom scene: {output}")
    return scene


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    binary, cli, work = args.binary.resolve(), args.cli.resolve(), args.out.resolve()
    work.mkdir(parents=True, exist_ok=True)
    helpers = runpy.run_path(str(Path(__file__).with_name("check-native-gui.py")))
    window_bounds, wait_for = helpers["window_bounds"], helpers["wait_for"]
    config = work / "config"
    config.mkdir(exist_ok=True)
    env = {key: value for key, value in os.environ.items() if not key.startswith("CUEMOL_")}
    env.update(CUEMOL_CONFIG_DIR=str(config), CUEMOL_CACHE_DIR=str(work / "cache"))
    scene = prepare_scene(cli, work, env)
    contents = work / "Tauri Check.app" / "Contents"
    (contents / "MacOS").mkdir(parents=True, exist_ok=True)
    executable = contents / "MacOS" / "cuemol-rs"
    shutil.copy2(binary, executable)
    (contents / "Info.plist").write_bytes(plistlib.dumps({
        "CFBundleName": "cuemol-rs", "CFBundleExecutable": "cuemol-rs",
        "CFBundleIdentifier": "org.cuemol.validation", "CFBundlePackageType": "APPL",
        "NSHighResolutionCapable": True,
    }))
    report = {"ok": False}
    with (work / "gui.log").open("w") as log:
        process = subprocess.Popen([str(executable), str(scene)], cwd=work, env=env,
                                   stdout=log, stderr=subprocess.STDOUT)
        try:
            def visible_window():
                if process.poll() is not None:
                    raise helpers["ApplicationExited"]("The Tauri process exited before opening a window; see gui.log")
                return window_bounds(process.pid)

            bounds = wait_for(visible_window, timeout=90)
            x, y, width, height = bounds

            def capture(name=None, color="magenta"):
                if process.poll() is not None:
                    raise helpers["ApplicationExited"]("The Tauri process exited; see gui.log")
                image = ImageGrab.grab(bbox=(x, y, x + width, y + height)).convert("RGB")
                if name:
                    image.save(work / name)
                mask = Image.new("L", image.size)
                mask.putdata([255 if b > 180 and
                              ((r > 180 and g < 60) if color == "magenta" else (g > 180 and r < 60))
                              else 0 for r, g, b in image.getdata()])
                return mask

            def settled():
                previous, unchanged = None, 0

                def sample():
                    nonlocal previous, unchanged
                    current = capture()
                    if previous is not None and not ImageChops.difference(previous, current).getbbox():
                        unchanged += 1
                    else:
                        unchanged = 0
                    previous = current
                    return current if current.getbbox() and unchanged >= 2 else None

                return wait_for(sample)

            wait_for(lambda: mask if (mask := capture()).histogram()[255] > 100 else None,
                     timeout=90)
            before = settled()
            capture("before.png")
            left, top, right, bottom = before.getbbox()
            scale_x, scale_y = width / before.width, height / before.height
            point_x = x + (left + right) * 0.5 * scale_x
            point_y = y + (top + bottom) * 0.5 * scale_y
            pyautogui.moveTo(point_x, point_y)
            pyautogui.mouseDown()
            previous = before
            changes = 0
            started = time.monotonic()
            try:
                for step in range(1, 21):
                    pyautogui.moveTo(point_x + step * 4, point_y + step)
                    time.sleep(0.03)
                    current = capture()
                    if ImageChops.difference(previous, current).getbbox():
                        changes += 1
                        previous = current
            finally:
                pyautogui.mouseUp()
            duration = time.monotonic() - started
            if changes < 3:
                raise RuntimeError(f"Only {changes} label updates appeared before mouse release")
            rotated = settled()
            if not ImageChops.difference(before, rotated).getbbox():
                raise RuntimeError("Rotation did not persist after mouse release")
            capture("rotated.png")
            pyautogui.scroll(3)
            wait_for(lambda: mask if (mask := capture()).getbbox()
                     and ImageChops.difference(rotated, mask).getbbox() else None)
            if not ImageChops.difference(rotated, settled()).getbbox():
                raise RuntimeError("Wheel input did not persist after rendering settled")
            capture("zoomed.png")
            zoomed = settled()
            pyautogui.click(x + width * 0.6, y + height - 43)
            pyautogui.write('set "Tauri probe/probe-label.color", cyan', interval=0.01)
            pyautogui.hotkey('shift', 'enter')
            pyautogui.write('set "Tauri probe/probe-label.font_size", 40', interval=0.01)
            pyautogui.press('enter')
            wait_for(lambda: mask if (mask := capture(color="cyan")).histogram()[255] > 100 else None)
            if capture().histogram()[255] != 0:
                raise RuntimeError("The console did not replace the label color")
            capture("console-multiline.png")
            pyautogui.write('undo', interval=0.03)
            pyautogui.press('enter')
            wait_for(lambda: mask if not ImageChops.difference(zoomed, mask := capture()).getbbox() else None)
            capture("console-undo.png")
            controls = MacControls(process.pid)

            def control(role, label, prefix=False):
                return wait_for(lambda: controls.find(role, label, prefix=prefix))

            def field_value(label, expected):
                def matches():
                    field = controls.find("AXTextField", label)
                    if field is None:
                        return False
                    value = controls.attribute(field, "AXValue")
                    return expected(value) if callable(expected) else value == expected
                return wait_for(matches)

            def rgba_matches(value):
                if not isinstance(value, str) or not value.startswith("rgba(") or not value.endswith(")"):
                    return False
                try:
                    values = [float(number) for number in value[5:-1].split(",")]
                except ValueError:
                    return False
                return len(values) == 4 and all(abs(actual - expected) <= 1 / 255
                    for actual, expected in zip(values, [0.2, 0.4, 0.6, 0.5]))

            controls.click(control("AXButton", "Probe palette", prefix=True), double=True)
            field_value("Style set name", "Probe palette")
            controls.fill(control("AXTextField", "Color accent"), "rgba(0.2,0.4,0.6,0.5)")
            controls.click(control("AXButton", "Apply color accent"))
            field_value("Color accent", lambda value: value != "rgba(0.2,0.4,0.6,0.5)" and rgba_matches(value))
            controls.click(control("AXButton", "Selection"))
            controls.fill(control("AXTextField", "Selection focus"), "name O1")
            controls.click(control("AXButton", "Apply selection focus"))
            field_value("Selection focus", "name O1")
            controls.fill(control("AXTextField", "Style set name"), " Renamed palette ")
            controls.click(control("AXButton", "Rename"))
            field_value("Style set name", "Renamed palette")
            controls.click(control("AXButton", "Save style..."))

            def saved_style():
                root = ET.parse(work / "linked palette.xml").getroot()
                color, selection = root.find("color[@id='accent']"), root.find("sel[@id='focus']")
                return (root.get("id") == "Renamed palette" and color is not None
                        and rgba_matches(color.get("value"))
                        and selection is not None and selection.get("value") == "name O1")

            wait_for(saved_style)
            capture("style-scene-saved.png")
            controls.click(control("AXButton", "Close"))
            controls.click(control("AXButton", "user (global)"), double=True)
            field_value("Style set name", "user")
            controls.fill(control("AXTextField", "New color name"), "native-accent")
            controls.click(control("AXButton", "Add"))
            field_value("Color native-accent", "#FFFFFF")
            controls.fill(control("AXTextField", "Color native-accent"), "#22cc44")
            controls.click(control("AXButton", "Apply color native-accent"))
            field_value("Color native-accent", "#22CC44")
            controls.fill(control("AXTextField", "Style set name"), " Global palette ")
            controls.click(control("AXButton", "Rename"))
            field_value("Style set name", "Global palette")
            capture("style-global-renamed.png")
            controls.click(control("AXButton", "Close"))
            control("AXButton", "Global palette", prefix=True)
            report.update(ok=True, window=bounds, label_pixels=before.histogram()[255],
                          updates_before_release=changes, drag_seconds=duration,
                          wheel_changed_label_pixels=True, console_multiline_undo=True,
                          style_scene_saved=True, style_global_renamed=True)
        except BaseException as error:
            report["error"] = str(error)
            if "controls" in locals():
                try:
                    report["controls"] = controls.describe()
                except Exception as control_error:
                    report["controls_error"] = str(control_error)
            if "capture" in locals():
                try:
                    capture("failure.png")
                except Exception as capture_error:
                    report["capture_error"] = str(capture_error)
            raise
        finally:
            (work / "result.json").write_text(json.dumps(report, indent=2) + "\n")
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
