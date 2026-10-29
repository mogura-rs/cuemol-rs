import cuemol
from . import fileio, renderer


def bgcolor(aCol):
    scene = cuemol.scene()
    scene.bgcolor = cuemol.col(aCol, scene) if isinstance(aCol, str) else aCol


def get(aObj, aName):
    return cuemol._internal.getProp(cuemol.rend(aObj), aName)


def set(aObj, aName, aVal):
    cuemol._internal.setProp(cuemol.rend(aObj), aName, aVal)


def reset(aObj, aName):
    cuemol._internal.resetProp(cuemol.rend(aObj), aName)


def delete(aObj, aType=None):
    target = aObj if cuemol.iswrapper(aObj) else None
    if target is None:
        for lookup in (cuemol.rend, cuemol.obj):
            try:
                target = lookup(aObj)
                break
            except RuntimeError:
                continue
    if cuemol.isobj(target):
        target.getScene().destroyObject(target.uid)
        return True
    if cuemol.isrend(target):
        target.getClientObj().destroyRenderer(target.uid)
        return True
    return False


def load(aFileName, aName=None, aFmt=None, aScene=None, aOpts=None):
    handler, name, compression = fileio.guessFormatFromFname(aFileName, aFmt)
    options = dict(aOpts or {})
    if compression:
        options.setdefault("compress", {"gz": "gzip", "bz2": "bzip2"}.get(compression, compression))
    name = name if aName is None else aName
    if handler["category"] == 3:
        return fileio.loadScene(aFileName, name, aScene, handler["name"], options)
    obj = fileio.loadObject(aFileName, name, aScene, handler["name"], options)
    renderer.setupDefaultRenderer(obj)
    return obj
