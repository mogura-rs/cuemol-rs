# /// script
# requires-python = ">=3.10"
# dependencies = ["pyautogui==0.9.54", "pillow>=11,<14"]
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

import pyautogui
from PIL import Image, ImageChops, ImageGrab


def prepare_scene(cli, work, env):
    molecule = work / "molecule with spaces.pdb"
    molecule.write_text(
        "HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00 10.00           C\n"
        "HETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00 10.00           O\n"
        "HETATM    3  N1  LIG A   1       1.000   1.500   1.000  1.00 10.00           N\n"
        "CONECT    1    2\nCONECT    2    3\nEND\n")
    scene = work / "scene with spaces.qsc"
    source = f"""
var reader = cuemol.getService('StreamManager').createHandler('pdb', 0);
var object = reader.createDefaultObj();
reader.attach(object); reader.path = {json.dumps(str(molecule))}; reader.read(); reader.detach();
scene.addObject(object); object.name = 'Tauri probe';
object.createRenderer('ballstick');
var label = object.createRenderer('*namelabel');
label.font_name = 'serif'; label.font_size = 26;
label.font_weight = 'BOLD'; label.font_style = 'ITALIC';
label.color = cuemol.makeColor('#ff00ff'); label.label_on_top = true;
label.setLabel(3, 'Top\\rBottom');
var camera = cuemol.Camera(); camera.zoom = 9; camera.distance = 40; camera.slab = 20;
scene.setCamera('__current', camera);
var writer = cuemol.SceneXMLWriter(); writer.embedAll = true;
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
            bounds = wait_for(lambda: window_bounds(process.pid), timeout=90)
            x, y, width, height = bounds

            def capture(name=None):
                if process.poll() is not None:
                    raise helpers["ApplicationExited"]("The Tauri process exited; see gui.log")
                image = ImageGrab.grab(bbox=(x, y, x + width, y + height)).convert("RGB")
                if name:
                    image.save(work / name)
                mask = Image.new("L", image.size)
                mask.putdata([255 if r > 180 and g < 60 and b > 180 else 0
                              for r, g, b in image.getdata()])
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
            report.update(ok=True, window=bounds, label_pixels=before.histogram()[255],
                          updates_before_release=changes, drag_seconds=duration,
                          wheel_changed_label_pixels=True)
        except BaseException as error:
            report["error"] = str(error)
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
