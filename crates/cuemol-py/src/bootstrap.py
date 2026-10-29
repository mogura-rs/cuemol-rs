import json
import os

_protocol_input = open(os.dup(0), "r", encoding="utf-8")
exec(compile(json.loads(_protocol_input.readline()), "<cuemol-binding>", "exec"))
