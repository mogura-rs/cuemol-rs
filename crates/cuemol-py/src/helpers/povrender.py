"""POV-Ray scene and animation helpers backed by the shared Rust renderer."""
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import cuemol as cm

POVRAY_BIN = "povray"
POVRAY_INC = None


def _positive_integer(value, name):
    number = int(value)
    if number != value or number <= 0:
        raise ValueError(f"{name} must be a positive integer")
    return number


class PovRender:
    def __init__(self, width, height, camera="__current", nthr=1, fps=25.0,
                 clip_slab=True, post_blend=True, edge_lines=True,
                 use_pix_imgs=False, perspective=False, shadow=False,
                 use_fog=True, rad_mode=-1, povray_bin=None, povray_inc=None,
                 blendpng_bin=None):
        self.width = _positive_integer(width, "width")
        self.height = _positive_integer(height, "height")
        self.camera = camera
        self.nthreads = _positive_integer(nthr, "nthr")
        self.povray_bin = povray_bin if povray_bin is not None else POVRAY_BIN
        self.povray_inc = povray_inc if povray_inc is not None else POVRAY_INC
        self.mbClip = clip_slab
        self.mbPostBlend = post_blend
        self.mbShowEdgeLines = edge_lines
        self.mbUsePixImgs = use_pix_imgs
        self.perspective = perspective
        self.shadow = shadow
        self.use_fog = use_fog
        self.mFPS = float(fps)
        if not math.isfinite(self.mFPS) or self.mFPS <= 0:
            raise ValueError("fps must be finite and positive")
        self.blendpng_path = blendpng_bin
        self.mnLightSpread = 1
        self.mnRadMode = rad_mode
        self.mbRadiosity = rad_mode != -1
        self.mdLightInten = 1.6 if self.mbRadiosity else 1.3
        self.mdFlashFrac = 0.0 if self.mbRadiosity else 0.8 / 1.3
        self.mdAmbFrac = 0.7 if self.mbRadiosity else 0.0

    def _writePovFiles(self, pov_base, scene, animMgr=None):
        scene = cm.scene(scene)
        if scene is None:
            raise ValueError("scene does not exist")
        exporter = cm.svc("StreamManager").createHandler("pov", 2)
        if exporter is None:
            raise RuntimeError("cannot create POV-Ray exporter")
        exporter.useClipZ = self.mbClip
        exporter.usePostBlend = self.mbPostBlend
        exporter.showEdgeLines = self.mbShowEdgeLines
        exporter.usePixImgs = self.mbUsePixImgs
        exporter.perspective = self.perspective
        exporter.makeRelIncPath = False
        exporter.camera = self.camera
        exporter.width = _positive_integer(self.width, "width")
        exporter.height = _positive_integer(self.height, "height")
        exporter.setPath(str(pov_base) + ".pov")
        exporter.setSubPath("inc", str(pov_base) + ".inc")
        if animMgr is not None:
            animMgr.writeFrame(exporter)
        else:
            if self.camera == "__current":
                scene.saveViewToCam(scene.activeViewID, self.camera)
            exporter.attach(scene)
            try:
                exporter.write()
            finally:
                exporter.detach()
        return json.loads(exporter.blendTable) if self.mbPostBlend and exporter.blendTable else {}

    def _render_blend_pov(self, pov_base, blend_table):
        binary = os.environ.get("CUEMOL_EXECUTABLE") or shutil.which("cuemol")
        if not binary:
            raise RuntimeError("cuemol executable is required for POV-Ray rendering")
        request = {
            "executable": os.fspath(self.povray_bin),
            "include_dir": os.fspath(self.povray_inc) if self.povray_inc else "",
            "input_path": str(pov_base) + ".pov",
            "output_path": str(pov_base) + ".png",
            "options": {
                "width": _positive_integer(self.width, "width"),
                "height": _positive_integer(self.height, "height"),
                "threads": _positive_integer(self.nthreads, "nthr"),
                "perspective": bool(self.perspective),
                "shadow": bool(self.shadow),
                "use_fog": bool(self.use_fog),
                "light_spread": int(self.mnLightSpread),
                "light_intensity": float(self.mdLightInten),
                "flash_fraction": float(self.mdFlashFrac),
                "ambient_fraction": float(self.mdAmbFrac),
                "radiosity": int(self.mnRadMode) if self.mbRadiosity else -1,
                "post_blend": bool(self.mbPostBlend),
                "blend_executable": os.fspath(self.blendpng_path) if self.blendpng_path else None,
                "verbose": False,
            },
        }
        completed = subprocess.run([binary, "tools", "render-pov", "-"],
                                   input=json.dumps(request, allow_nan=False),
                                   capture_output=True, text=True, check=False)
        if completed.returncode:
            raise RuntimeError("POV-Ray rendering failed: " + completed.stderr.strip())
        output = Path(str(pov_base) + ".png")
        if not output.is_file():
            raise RuntimeError("POV-Ray did not produce the requested PNG")
        return output

    def _render_to(self, scene, output, animation=None):
        output = Path(output).resolve()
        with tempfile.TemporaryDirectory(prefix=".cuemol-pov-", dir=output.parent) as work:
            base = Path(work) / "scene"
            table = self._writePovFiles(base, scene, animation)
            rendered = self._render_blend_pov(base, table)
            os.replace(rendered, output)

    def render_simple(self, scene, basename):
        self._render_to(scene, os.fspath(basename) + ".png")

    def render_anim(self, scene, basename, startfrm, nfrms):
        if int(startfrm) != startfrm or startfrm < 0:
            raise ValueError("startfrm must be a non-negative integer")
        if int(nfrms) != nfrms:
            raise ValueError("nfrms must be an integer")
        scene = cm.scene(scene)
        if scene is None:
            raise ValueError("scene does not exist")
        animation = scene.getAnimMgr()
        if animation.size <= 0:
            return
        animation.startcam = self.camera
        start = cm.timeval(math.floor(1000.0 * startfrm / self.mFPS))
        end = animation.length
        if start.millisec > end.millisec:
            return
        count = animation.setupRender(start, end, self.mFPS)
        if nfrms < 0:
            nfrms = count
        for frame in range(int(startfrm), int(startfrm + nfrms)):
            self._render_to(scene, f"{os.fspath(basename)}-{frame:04d}.png", animation)


def render(scene, out_png_file, width=640, height=480, camera="__current", nthr=1):
    scene = cm.scene(scene)
    if scene is None:
        raise ValueError("scene does not exist")
    if camera != "__current":
        scene.loadViewFromCam(scene.activeViewID, camera)
        scene.saveViewToCam(scene.activeViewID, "__current")
    PovRender(width, height, camera=camera, nthr=nthr, use_pix_imgs=True)._render_to(scene, out_png_file)
