from pathlib import Path

import cuemol as cm


def bg_color(colstr, aScene=None):
    scene = cm.scene(aScene)
    with cm.UndoTxn("Change background color", scene):
        scene.bgcolor = cm.col(colstr, scene)


def get_scene_ids():
    return [int(value) for value in cm.sceMgr().getSceneUIDList().split(",") if value]


def get_scenes():
    manager = cm.sceMgr()
    return [manager.getScene(uid) for uid in get_scene_ids()]


def del_scene(scene):
    if scene is None:
        raise RuntimeError("scene is None")
    cm.sceMgr().destroyScene(cm.scene(scene).uid)


def del_all_scenes():
    cm.sceMgr().destroyAllScenes()


def undo(aScene=None):
    cm.scene(aScene).undo(0)


def redo(aScene=None):
    cm.scene(aScene).redo(0)


def current(rend_name=None, aScene=None):
    scene = cm.scene(aScene)
    if rend_name is not None:
        renderer = scene.getRendByName(rend_name)
        if renderer is None:
            cm.println(f"Error, renderer name={rend_name} is not found.")
            return
        scene.activeRendID = renderer.uid
        cm.println(f"Current renderer is changed to {rend_name} ({renderer.type_name})")
        return
    if not scene.activeRendID:
        cm.println("No current renderer")
        return
    renderer = cm.sceMgr().getRenderer(scene.activeRendID)
    if renderer is None:
        cm.println(f"Invalid current renderer (ID={scene.activeRendID})")
        return
    obj = renderer.getClientObj()
    cm.println(f"Current renderer:\n  id={renderer.uid}\n  name={renderer.name} ({renderer.type_name})\n  Object={obj.name} ({cm.getClassName(obj)})")


def load_camera_file(scene, file_name, name=None, view=None):
    scene = cm.scene(scene)
    name = Path(file_name).name if name is None else name
    camera = scene.loadCamera(str(file_name))
    scene.setCamera(name, camera)
    scene.loadViewFromCam(scene.activeViewID if view is None else view.uid, name)
    return camera
