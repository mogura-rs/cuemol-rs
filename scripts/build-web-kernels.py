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


def build_kernel(name, source_root, inputs, options, exports, runtime, output, emxx):
    build = ROOT / ".cache/web-kernels"
    build.mkdir(parents=True, exist_ok=True)
    version = subprocess.check_output([*emxx, "--version"])
    digest = hashlib.sha256(version + json.dumps([options, exports, runtime]).encode())
    for source in sorted(source_root.rglob("*")):
        if source.is_file():
            digest.update(source.relative_to(source_root).as_posix().encode())
            digest.update(source.read_bytes())
    fingerprint = digest.hexdigest()
    stamp = build / f"{name}.sha256"
    products = [build / f"{name}.{extension}" for extension in ("js", "wasm")]
    if not stamp.is_file() or stamp.read_text() != fingerprint or not all(p.is_file() for p in products):
        command = [*emxx, "-O2", *options, *map(str, inputs),
                   "-sMODULARIZE=1", "-sEXPORT_ES6=1", "-sALLOW_MEMORY_GROWTH=1",
                   "-sENVIRONMENT=web,worker", "-sSTACK_SIZE=2097152", "-sEXPORTED_FUNCTIONS=" + json.dumps(exports + ["_malloc", "_free"]),
                   "-sEXPORTED_RUNTIME_METHODS=" + json.dumps(runtime), "-o", str(products[0])]
        subprocess.run(command, check=True)
        stamp.write_text(fingerprint)
    for product in products:
        shutil.copyfile(product, output / product.name)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out-dir", type=Path, default=ROOT / "web/pkg/kernels")
    args = parser.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)
    emxx = compiler()
    sources = ROOT / "crates/cuemol-molanl/vendor"
    build_kernel("usalign", sources, [sources / "USalignImpl.cpp", sources / "usalign_bridge.cpp"],
                 ["-std=c++17", "-fexceptions", "-sDISABLE_EXCEPTION_CATCHING=0"],
                 ["_cuemol_usalign"], ["HEAPU8", "HEAPF64", "UTF8ToString"], args.out_dir, emxx)
    shutil.copyfile(sources / "usalign/LICENSE", args.out_dir / "USalign-LICENSE")
    sources = ROOT / "crates/cuemol-gfx/vendor"
    build_kernel("lcms", sources, [*sorted((sources / "lcms2/src").glob("*.c")), sources / "cms_bridge.c"],
                 ["-x", "c", "-std=c99", "-I" + str(sources / "lcms2/include")],
                 ["_cuemol_cms_create", "_cuemol_cms_destroy", "_cuemol_cms_color", "_cuemol_cms_info",
                  "_cmsCreate_sRGBProfile", "_cmsSaveProfileToMem", "_cmsCloseProfile"],
                 ["HEAPU8", "HEAPU32", "UTF8ToString"], args.out_dir, emxx)
    shutil.copyfile(sources / "lcms2/LICENSE", args.out_dir / "LittleCMS-LICENSE")
    sources = ROOT / "crates/cuemol-surface/vendor"
    inputs = sorted((sources / "meshms/src").glob("*.cpp"))
    for directory in ("COMMON", "DATATYPE", "STRUCTURE"):
        inputs.extend(sorted((sources / "BALL" / directory).glob("*.cpp")))
    inputs.append(sources / "surface_bridge.cpp")
    build_kernel("surface", sources, inputs,
                 ["-std=c++20", "-fexceptions", "-ffp-contract=off", "-sDISABLE_EXCEPTION_CATCHING=0",
                  "-I" + str(sources), "-I" + str(sources / "meshms/include")],
                 ["_cuemol_surface_build", "_cuemol_surface_destroy", "_cuemol_surface_backend",
                  "_cuemol_surface_vertex_count", "_cuemol_surface_triangle_count",
                  "_cuemol_surface_vertices", "_cuemol_surface_triangles"],
                 ["HEAPU8", "HEAPU32", "HEAPF64", "UTF8ToString"], args.out_dir, emxx)
    for source, name in [("CueMol-LICENSE", "BALL-LICENSE"), ("Boost-LICENSE", "Boost-LICENSE"),
                         ("meshms/SOURCE.json", "MeshMS-SOURCE.json"), ("BALL-SOURCE.json", "BALL-SOURCE.json")]:
        shutil.copyfile(sources / source, args.out_dir / name)



if __name__ == "__main__":
    main()
