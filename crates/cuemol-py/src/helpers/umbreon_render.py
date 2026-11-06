"""Render scenes with the shared Rust Umbreon exporter."""

import argparse

import cuemol as cm

__all__ = ["apply_scene_settings", "render", "render_file"]

EXPORTER_CATEGORY = 2
DEFAULT_CAMERA = "__current"
RENDER_APP_DATA_ID = "render"
RENDER_APP_DATA_CLASS = "RenderSettings"


def _scene(scene):
    result = cm.scene(scene)
    if result is None:
        raise RuntimeError(f"scene ({scene}) does not exist")
    return result


def apply_scene_settings(scene, exporter, camera=DEFAULT_CAMERA):
    """Apply stored settings without creating scene app data."""
    scene = _scene(scene)
    settings = scene.getAppData(RENDER_APP_DATA_ID)
    stored = settings is not None
    if not stored:
        settings = cm.createObj(RENDER_APP_DATA_CLASS)
    backend = exporter.applyRenderSettings(settings, "")
    if not stored:
        exporter.perspective = scene.getCamera(camera).perspec
    return backend


def render(scene, out_png, width=None, height=None, camera=DEFAULT_CAMERA):
    """Write a PNG using saved settings and optional size overrides."""
    scene = _scene(scene)
    if not scene.hasCamera(camera):
        raise RuntimeError(f"scene has no camera named '{camera}'; pass an existing camera name")
    exporter = cm.strMgr().createHandler("umbreon", EXPORTER_CATEGORY)
    if exporter is None:
        raise RuntimeError("umbreon exporter is not available")
    apply_scene_settings(scene, exporter, camera)
    if width is not None:
        exporter.width = width
    if height is not None:
        exporter.height = height
    exporter.camera = camera
    exporter.attach(scene)
    try:
        exporter.setPath(str(out_png))
        exporter.write()
    finally:
        exporter.detach()


def render_file(qsc_path, out_png, width=None, height=None, camera=DEFAULT_CAMERA):
    """Load a scene, render it, and return it for further use."""
    result = cm.svc("CmdMgr").runCmdArgs("load_scene", {"file_path": str(qsc_path)})
    scene = result["result_scene"]
    if scene is None:
        raise RuntimeError(f"cannot load scene file: {qsc_path}")
    render(scene, out_png, width, height, camera)
    return scene


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("qsc", help="input scene file")
    parser.add_argument("png", help="output PNG file")
    parser.add_argument("-W", "--width", type=int)
    parser.add_argument("-H", "--height", type=int)
    parser.add_argument("-c", "--camera", default=DEFAULT_CAMERA)
    args = parser.parse_args(argv)
    if ((args.width is not None and args.width <= 0)
            or (args.height is not None and args.height <= 0)):
        parser.error("width and height must be positive")
    render_file(args.qsc, args.png, args.width, args.height, args.camera)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
