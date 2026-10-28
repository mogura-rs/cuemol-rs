#!/usr/bin/env python3
"""Build cached browser kernels from the vendored scientific implementations."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SDK_VERSION = "4.0.20"
SDK_REVISION = "e4fe26ef59168ff44f4c23c466e497bf60b3411e"


def compiler():
    override = os.environ.get("EMXX") or shutil.which("em++")
    if override:
        return [override]
    sdk = ROOT / ".cache/emsdk"
    if not (sdk / "emsdk.py").is_file():
        subprocess.run(["git", "clone", "--no-checkout", "--filter=blob:none",
                        "https://github.com/emscripten-core/emsdk.git", str(sdk)], check=True)
        subprocess.run(["git", "-C", str(sdk), "checkout", SDK_REVISION], check=True)
    if not (sdk / ".emscripten").is_file():
        for operation in ("install", "activate"):
            subprocess.run([sys.executable, str(sdk / "emsdk.py"), operation, SDK_VERSION], check=True)
    os.environ["EM_CONFIG"] = str(sdk / ".emscripten")
    return [sys.executable, str(sdk / "upstream/emscripten/em++.py")]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out-dir", type=Path, default=ROOT / "web/pkg/kernels")
    args = parser.parse_args()
    sources = ROOT / "crates/cuemol-molanl/vendor"
    build = ROOT / ".cache/web-kernels"
    build.mkdir(parents=True, exist_ok=True)
    emxx = compiler()
    version = subprocess.check_output([*emxx, "--version"])
    digest = hashlib.sha256(version + Path(__file__).read_bytes())
    for source in sorted(sources.rglob("*")):
        if source.is_file():
            digest.update(source.relative_to(sources).as_posix().encode())
            digest.update(source.read_bytes())
    fingerprint = digest.hexdigest()
    stamp = build / "usalign.sha256"
    if not stamp.is_file() or stamp.read_text() != fingerprint or not (build / "usalign.wasm").is_file():
        command = [*emxx, "-std=c++17", "-O2", "-fexceptions",
                   str(sources / "USalignImpl.cpp"), str(sources / "usalign_bridge.cpp"),
                   "-sMODULARIZE=1", "-sEXPORT_ES6=1", "-sALLOW_MEMORY_GROWTH=1",
                   "-sENVIRONMENT=web,worker", "-sDISABLE_EXCEPTION_CATCHING=0",
                   "-sEXPORTED_FUNCTIONS=" + json.dumps(["_cuemol_usalign", "_malloc", "_free"]),
                   "-sEXPORTED_RUNTIME_METHODS=" + json.dumps(["HEAPU8", "HEAPF64", "UTF8ToString"]),
                   "-o", str(build / "usalign.js")]
        subprocess.run(command, check=True)
        stamp.write_text(fingerprint)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    for name in ("usalign.js", "usalign.wasm"):
        shutil.copyfile(build / name, args.out_dir / name)
    shutil.copyfile(sources / "usalign/LICENSE", args.out_dir / "USalign-LICENSE")


if __name__ == "__main__":
    main()
