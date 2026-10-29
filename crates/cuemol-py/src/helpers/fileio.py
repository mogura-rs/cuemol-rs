import json
import os

import cuemol as cm

__all__ = ["loadScene", "loadObject", "load"]


def guessFormatFromFname(aPathName, aFmt=None):
    filename = os.fspath(aPathName)
    compression = ""
    for suffix, mode in ((".gz", "gz"), (".xz", "xz"), (".bz2", "bz2")):
        if filename.lower().endswith(suffix):
            filename, compression = filename[:-len(suffix)], mode
            break
    for handler in json.loads(cm.strMgr().getInfoJSON2()):
        if handler["category"] not in (0, 3):
            continue
        if aFmt is not None:
            matches = aFmt == handler["name"]
        else:
            extensions = [os.path.splitext(pattern.strip())[1]
                          for pattern in handler["fext"].split(";")]
            matches = any(ext and filename.lower().endswith(ext.lower()) for ext in extensions)
        if matches:
            return handler, os.path.basename(filename), compression
    raise RuntimeError(f"cannot guess file format from pathname: {aPathName}")


def _handler(name, category, path, options):
    handler = cm.strMgr().createHandler(name, category)
    for key, value in (options or {}).items():
        cm._internal.setProp(handler, key, value)
    handler.setPath(os.fspath(path))
    return handler


def loadScene(aFileName, aName, aScene, aFmtName, aOpts=None):
    scene = cm.scene(aScene)
    reader = _handler(aFmtName, 3, aFileName, aOpts)
    reader.attach(scene)
    try:
        reader.read()
    finally:
        reader.detach()
    if aName is not None:
        scene.name = aName
    return scene


def loadObject(aFileName, aName, aScene, aFmtName, aOpts=None):
    scene = cm.scene(aScene)
    reader = _handler(aFmtName, 0, aFileName, aOpts)
    obj = reader.createDefaultObj()
    reader.attach(obj)
    try:
        reader.read()
    finally:
        reader.detach()
    if aName is not None:
        obj.name = aName
    scene.addObject(obj)
    return obj


def saveObject(aObj, aFileName, aFmtName, aOpts=None):
    writer = _handler(aFmtName, 1, aFileName, aOpts)
    writer.attach(cm.obj(aObj))
    try:
        writer.write()
    finally:
        writer.detach()


def getReaderCategoryID(format):
    return 3 if format == "qsc_xml" else 0


def load(filename, name=None, format=None, scene=None):
    from . import renderer
    handler, default_name, compression = guessFormatFromFname(filename, format)
    name = default_name if name is None else name
    options = {"compress": {"gz": "gzip", "bz2": "bzip2"}.get(compression, compression)} if compression else None
    if handler["category"] == 3:
        return loadScene(filename, name, scene, handler["name"], options)
    obj = loadObject(filename, name, scene, handler["name"], options)
    renderer.setupDefaultRenderer(obj)
    return obj
