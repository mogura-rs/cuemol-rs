import logging
import sys
from logging import CRITICAL, DEBUG, ERROR, FATAL, INFO, WARN, WARNING

_default_handler = None
_root_names = None


class _CurrentErrorStream(logging.Handler):
    def emit(self, record):
        sys.stderr.write(self.format(record) + "\n")
        sys.stderr.flush()


def _get_library_root_loggers(names):
    return [logging.getLogger(name) for name in names]


def get_logger(name):
    return logging.getLogger(name)


def setup(root_names=None):
    global _default_handler, _root_names
    if _default_handler is not None:
        raise RuntimeError("logger is already initialized")
    _root_names = ["cuemol", "cuemol_gui"] if root_names is None else root_names
    if isinstance(_root_names, str):
        _root_names = [_root_names]
    _default_handler = _CurrentErrorStream()
    _default_handler.setFormatter(logging.Formatter("%(name)s: %(message)s"))
    for logger in _get_library_root_loggers(_root_names):
        logger.addHandler(_default_handler)
        logger.setLevel(logging.INFO)
        logger.propagate = False


def set_verbosity(verbosity):
    if _root_names is None:
        raise RuntimeError("logger is not initialized")
    for logger in _get_library_root_loggers(_root_names):
        logger.setLevel(verbosity)
