import json
import importlib.abc
import importlib.util
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

    def __hash__(self):
        return _rpc("hash", target=self._handle)

    def __contains__(self, name):
        return sys.modules["cuemol"].hasProp(self, name)

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
        if name in ("UndoTxn", "txn"):
            return getattr(importlib.import_module("cuemol.undo_txn"), name)
        if "cuemol." + name in _helper_sources:
            return importlib.import_module("cuemol." + name)
        try:
            return _rpc("root", name=name)
        except RuntimeError as error:
            raise AttributeError(str(error)) from error

    def getWrpClass(self, clsnm):
        if not self.hasClass(clsnm):
            raise ImportError(f"unknown CueMol class: {clsnm}")
        if clsnm not in _wrapper_classes:
            _wrapper_classes[clsnm] = _WrapperType(clsnm, (_Proxy,), {"_interface_name": clsnm})
        return _wrapper_classes[clsnm]

    def createWrapper(self, obj):
        if isinstance(obj, dict):
            for key, value in obj.items():
                obj[key] = self.createWrapper(value)
        elif isinstance(obj, list):
            for index, value in enumerate(obj):
                obj[index] = self.createWrapper(value)
        return obj

    def conv_dict_arg(self, d):
        if not isinstance(d, dict):
            raise TypeError("expected a dictionary")
        return {key: item._wrapped if self.iswrapper(item) else item for key, item in d.items()}

    def createObj(self, name, *arguments):
        return _rpc("root", name="createObj")(name, *arguments)

    def getService(self, name):
        return _rpc("root", name="getService")(name)

    def println(self, astr):
        print(astr)

    def svc(self, name):
        return self.getService(name)

    def sceMgr(self):
        return self.getService("SceneManager")

    def strMgr(self):
        return self.getService("StreamManager")

    def iswrapper(self, aObj):
        return isinstance(aObj, _Proxy)

    def isimpl(self, aObj, aIfName):
        return self.iswrapper(aObj) and self.implIface2(aObj, aIfName)

    def isscene(self, aObj):
        return self.isimpl(aObj, "Scene")

    def isview(self, aObj):
        return self.isimpl(aObj, "View")

    def isobj(self, aObj):
        return self.isimpl(aObj, "Object")

    def isrend(self, aObj):
        return self.isimpl(aObj, "Renderer")

    def issel(self, aObj):
        return self.isimpl(aObj, "MolSelection")

    def iscol(self, aObj):
        return self.isimpl(aObj, "AbstractColor")

    def createScene(self):
        manager = self.sceMgr()
        value = manager.createScene()
        manager.setActiveSceneID(value.uid)
        return value

    def scene(self, aScene=None):
        value = aScene
        if self.isscene(value):
            return value
        manager = self.sceMgr()
        if value is None:
            if manager.getSceneUIDList() == "":
                return self.createScene()
            value = manager.activeSceneID
            if not value:
                raise RuntimeError("Active scene ID is not defined")
        if isinstance(value, float) and value.is_integer():
            value = int(value)
        if not isinstance(value, int):
            raise RuntimeError(f"scene {value} not found")
        return manager.getScene(value)

    def view(self, aScene=None, aView=None):
        scene, value = aScene, aView
        if self.isview(value):
            return value
        scene = self.scene(scene)
        if value is None:
            if scene.getViewCount() == 0:
                value = scene.createView()
                scene.setActiveViewID(value.uid)
                return value
            value = scene.activeViewID
        if isinstance(value, float) and value.is_integer():
            value = int(value)
        return self.sceMgr().getView(value) if isinstance(value, int) and value else None

    def obj(self, aName, aScene=None):
        value, scene = aName, aScene
        if self.isobj(value):
            return value
        scene = self.scene(scene)
        result = scene.getObjectByName(value) if isinstance(value, str) else scene.getObject(value)
        if result is None:
            raise RuntimeError(f"object {value} not found")
        return result

    def rend(self, aRend, aObj=None):
        value, obj = aRend, aObj
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

    def vec(self, aX, aY, aZ, *args):
        if len(args) > 1:
            raise RuntimeError("too many args for vec()")
        x, y, z = aX, aY, aZ
        w = args[0] if args else None
        value = self.createObj("Vector")
        value.x, value.y, value.z = x, y, z
        if w is not None:
            value.w = w
        return value

    def sel(self, aSelStr, aScene=None):
        if self.issel(aSelStr):
            return aSelStr
        selection = self.createObj("SelCommand")
        if not selection.compile(aSelStr, self.scene(aScene).uid):
            raise ValueError(selection.error_msg)
        return selection

    def col(self, aColStr, aScene=None):
        expression, scene = aColStr, aScene
        return expression if self.iscol(expression) else self.svc("StyleManager").compileColor(expression, self.scene(scene).uid)

    def timeval(self, aMilli):
        milliseconds = aMilli
        value = self.createObj("TimeValue")
        value.millisec = milliseconds
        return value

    def copy(self, aObj, aNewObjName):
        result = self._copyObject(self.obj(aObj).uid)
        result.name = aNewObjName
        return result


class _WrapperType(type):
    def __instancecheck__(self, value):
        return sys.modules["cuemol"].isimpl(value, self._interface_name)

    def __call__(self, value):
        if not isinstance(value, self):
            raise TypeError(f"expected {self._interface_name}")
        return value


_wrapper_classes = {}


class _Internal:
    Wrapper = _Proxy

    def __getattr__(self, name):
        return getattr(sys.modules["cuemol"], name)

    def getProp(self, obj, name):
        for component in name.split("."):
            obj = getattr(obj, component)
        return obj

    def setProp(self, obj, name, value):
        components = name.split(".")
        for component in components[:-1]:
            obj = getattr(obj, component)
        setattr(obj, components[-1], value)

    def resetProp(self, obj, name):
        components = name.split(".")
        for component in components[:-1]:
            obj = getattr(obj, component)
        return sys.modules["cuemol"].resetProp(obj, components[-1])

    def invokeMethod(self, obj, name, *arguments):
        return getattr(obj, name)(*arguments)

    def createBAryFromBytes(self, value):
        import base64
        return _rpc("bytes", value=base64.b64encode(bytes(value)).decode("ascii"))

    def print(self, value):
        print(value, end="")


class _HelperLoader(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    def find_spec(self, fullname, path=None, target=None):
        if fullname in _helper_sources:
            return importlib.util.spec_from_loader(fullname, self, origin=f"<{fullname}>")
        if fullname == "cuemol.wrappers":
            return importlib.util.spec_from_loader(fullname, self, is_package=True)
        if fullname.startswith("cuemol.wrappers."):
            name = fullname.rsplit(".", 1)[1]
            if sys.modules["cuemol"].hasClass(name):
                return importlib.util.spec_from_loader(fullname, self)
        return None

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        source = _helper_sources.get(module.__name__)
        if source is not None:
            module.__file__ = f"<{module.__name__}>"
            exec(compile(source, module.__file__, "exec"), module.__dict__)
        elif module.__name__ != "cuemol.wrappers":
            name = module.__name__.rsplit(".", 1)[1]
            setattr(module, name, sys.modules["cuemol"].getWrpClass(name))


_cuemol = _CueMol("cuemol")
_cuemol.__path__ = []
_cuemol.__package__ = "cuemol"
_cuemol.__spec__ = importlib.util.spec_from_loader("cuemol", loader=None, is_package=True)
_cuemol.__all__ = ["getWrpClass", "createWrapper", "conv_dict_arg", "createObj", "getService",
                   "println", "iswrapper", "isimpl", "isscene", "isview", "isobj", "isrend",
                   "issel", "iscol", "scene", "view", "createScene", "svc", "obj", "rend",
                   "sceMgr", "strMgr", "vec", "sel", "col", "timeval", "copy", "UndoTxn", "txn"]
_cuemol._Proxy = _Proxy
_cuemol._internal = _Internal()
sys.modules["cuemol"] = _cuemol
sys.meta_path.insert(0, _HelperLoader())
