import cuemol


class UndoTxn:
    def __init__(self, aMsg=None, aScene=None, enable=True):
        self.msg = aMsg or ""
        self._scene = cuemol.scene(aScene)
        self._enable = enable

    def __enter__(self):
        if self._enable:
            self._scene.startUndoTxn(self.msg)
        return self

    def __exit__(self, exception_type, exception_value, traceback):
        if self._enable:
            if exception_type is None:
                self._scene.commitUndoTxn()
            else:
                self._scene.rollbackUndoTxn()
        return False


def txn(aScene, aMsg, aFunc):
    try:
        with UndoTxn(aMsg, aScene):
            aFunc()
    except Exception:
        return False
    return True
