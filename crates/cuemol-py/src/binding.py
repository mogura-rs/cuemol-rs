import json
import os
import sys
import types

_protocol_input = open(os.dup(0), "r", encoding="utf-8")
_protocol_output = open(os.dup(1), "w", encoding="utf-8", buffering=1)
_released = []
_callbacks = {}
_next_callback = 0


def _rpc(operation, **arguments):
    request = {"op": operation, **arguments, "release": list(_released)}
    _released.clear()
    _protocol_output.write(json.dumps({"rpc": request}) + "\n")
    _protocol_output.flush()
    while True:
        line = _protocol_input.readline()
        if not line:
            raise RuntimeError("CueMol application disconnected")
        result = json.loads(line)
        if "callback" in result or "release_callbacks" in result:
            _execute_request(result)
        else:
            break
    if "error" in result:
        raise RuntimeError(result["error"])
    return _decode(result.get("value"))


def _encode(value):
    global _next_callback
    if isinstance(value, _Proxy):
        return {"$cuemol_ref": value._handle}
    if isinstance(value, (list, tuple)):
        return [_encode(item) for item in value]
    if isinstance(value, dict):
        return {key: _encode(item) for key, item in value.items()}
    if callable(value):
        _next_callback += 1
        _callbacks[_next_callback] = value
        return {"$cuemol_callback": _next_callback}
    return value


def _decode(value):
    if isinstance(value, dict):
        if "$cuemol_ref" in value:
            return _Proxy(value["$cuemol_ref"])
        return {key: _decode(item) for key, item in value.items()}
    if isinstance(value, list):
        return [_decode(item) for item in value]
    return value


class _Proxy:
    def __init__(self, handle):
        object.__setattr__(self, "_handle", handle)

    @property
    def _wrapped(self):
        return self

    def __getattr__(self, name):
        try:
            return _rpc("get", target=self._handle, name=name)
        except RuntimeError as error:
            raise AttributeError(str(error)) from error

    def __setattr__(self, name, value):
        _rpc("set", target=self._handle, name=name, value=_encode(value))

    def __call__(self, *arguments):
        return _rpc("call", target=self._handle, arguments=_encode(arguments))

    def __eq__(self, other):
        return isinstance(other, _Proxy) and _rpc("equal", target=self._handle, other=other._handle)

    def __str__(self):
        return _rpc("string", target=self._handle)

    def __repr__(self):
        return str(self)

    def __del__(self):
        _released.append(object.__getattribute__(self, "_handle"))


class _CueMol(types.ModuleType):
    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        return _rpc("root", name=name)

    def svc(self, name):
        return self.getService(name)

    def sceMgr(self):
        return self.getService("SceneManager")

    def strMgr(self):
        return self.getService("StreamManager")

    def iswrapper(self, value):
        return isinstance(value, _Proxy)

    def isimpl(self, value, interface):
        return self.iswrapper(value) and self.implIface2(value, interface)

    def isscene(self, value):
        return self.isimpl(value, "Scene")

    def isview(self, value):
        return self.isimpl(value, "View")

    def isobj(self, value):
        return self.isimpl(value, "Object")

    def isrend(self, value):
        return self.isimpl(value, "Renderer")

    def issel(self, value):
        return self.isimpl(value, "MolSelection")

    def iscol(self, value):
        return self.isimpl(value, "AbstractColor")

    def createScene(self):
        manager = self.sceMgr()
        value = manager.createScene()
        manager.setActiveSceneID(value.uid)
        return value

    def scene(self, value=None):
        if self.isscene(value):
            return value
        manager = self.sceMgr()
        if value is None:
            if manager.getSceneUIDList() == "":
                return self.createScene()
            value = manager.activeSceneID
        return manager.getScene(value)

    def view(self, scene=None, value=None):
        if self.isview(value):
            return value
        scene = self.scene(scene)
        if value is None:
            if scene.getViewCount() == 0:
                value = scene.createView()
                scene.setActiveViewID(value.uid)
                return value
            value = scene.activeViewID
        return self.sceMgr().getView(value) if value else None

    def obj(self, value, scene=None):
        if self.isobj(value):
            return value
        scene = self.scene(scene)
        result = scene.getObjectByName(value) if isinstance(value, str) else scene.getObject(value)
        if result is None:
            raise RuntimeError(f"object {value} not found")
        return result

    def rend(self, value, obj=None):
        if self.isrend(value):
            return value
        owner = self.scene() if obj is None else self.obj(obj)
        if isinstance(value, str):
            result = owner.getRendByName(value) if obj is None else owner.getRendererByName(value)
        else:
            result = owner.getRenderer(value)
        if result is None:
            raise RuntimeError(f"renderer {value} not found")
        return result

    def vec(self, x, y, z, w=None):
        value = self.createObj("Vector")
        value.x, value.y, value.z = x, y, z
        if w is not None:
            value.w = w
        return value

    def sel(self, expression, scene=None):
        return expression if self.issel(expression) else self.makeSel(expression, self.scene(scene).uid)

    def col(self, expression, scene=None):
        return expression if self.iscol(expression) else self.svc("StyleManager").compileColor(expression, self.scene(scene).uid)

    def timeval(self, milliseconds):
        value = self.createObj("TimeValue")
        value.millisec = milliseconds
        return value

    def copy(self, value, name):
        value = self.obj(value)
        scene = value.getScene()
        manager = self.strMgr()
        result = manager.fromXML(manager.toXML(value), scene.uid)
        result.name = name
        scene.addObject(result)
        return result


sys.modules["cuemol"] = _CueMol("cuemol")
