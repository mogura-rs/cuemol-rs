import contextlib
import json
import os
import sys
import tempfile
import traceback

_scope = {"__name__": "__main__", "__builtins__": __builtins__}


@contextlib.contextmanager
def _redirect_output(output, errors):
    sys.stdout.flush()
    sys.stderr.flush()
    saved = os.dup(0), os.dup(1), os.dup(2)
    previous_input = sys.stdin
    empty_input = open(os.devnull, "r", encoding="utf-8")
    try:
        os.dup2(empty_input.fileno(), 0)
        sys.stdin = empty_input
        os.dup2(output.fileno(), 1)
        os.dup2(errors.fileno(), 2)
        with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
            yield
    finally:
        output.flush()
        errors.flush()
        for descriptor, original in zip((0, 1, 2), saved):
            os.dup2(original, descriptor)
            os.close(original)
        sys.stdin = previous_input
        empty_input.close()


def _execute_request(request):
    with tempfile.TemporaryFile(mode="w+", encoding="utf-8", errors="replace", buffering=1) as output, \
         tempfile.TemporaryFile(mode="w+", encoding="utf-8", errors="replace", buffering=1) as errors:
        status, value = 0, None
        with _redirect_output(output, errors):
            try:
                if "source" in request:
                    exec(compile(request["source"], "<cuemol>", "exec"), _scope, _scope)
                elif "callback" in request:
                    value = _encode(_callbacks[request["callback"]](*_decode(request["arguments"])))
                else:
                    for callback in request["release_callbacks"]:
                        _callbacks.pop(callback, None)
            except BaseException:
                status = 1
                traceback.print_exc()
            _rpc("release")
        output.seek(0)
        errors.seek(0)
        result = {"status": status, "stdout": output.read(), "stderr": errors.read(), "value": value}
    _protocol_output.write(json.dumps(result) + "\n")
    _protocol_output.flush()


for _line in _protocol_input:
    _execute_request(json.loads(_line))
