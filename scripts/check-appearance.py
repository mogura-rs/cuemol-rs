#!/usr/bin/env python3
"""Compare wgpu viewport fixtures with the read-only reference GLSL shaders.

Run with uv run --no-project --with moderngl --with numpy --with pillow python
scripts/check-appearance.py. The reference tree and an EGL adapter are required.
"""
import argparse
import json
import os
from pathlib import Path
import re
import struct
import subprocess

import moderngl
import numpy as np
from PIL import Image


def shader_source(path, directories, seen=None):
    seen = set() if seen is None else seen
    if path in seen:
        return ""
    seen.add(path)
    lines = []
    for line in path.read_text().splitlines():
        match = re.match(r'\s*#include [<"]([^>"]+)[>"]', line)
        if match:
            included = next((directory / match[1] for directory in directories if (directory / match[1]).is_file()), None)
            if included is None:
                raise RuntimeError(f"Unresolved shader include: {match[1]}")
            lines.append(shader_source(included, directories, seen))
        elif not line.startswith("#pragma once"):
            lines.append(line)
    return "\n".join(lines) + "\n"


def reference_image(ctx, programs, fixture, postprocess=None, offset=(0, 0)):
    width, height = fixture["size"]
    view = np.array(fixture["view"], dtype="f4")
    distance, slab, zoom = (fixture[key] for key in ("distance", "slab", "zoom"))
    near, far = max(0.1, distance - slab / 2), distance + slab
    projection = np.zeros((4, 4), dtype="f4")
    if fixture["perspective"]:
        projection[0, 0] = 2 * distance / zoom / (width / height)
        projection[1, 1] = 2 * distance / zoom
        projection[2, 2] = -(far + near) / (far - near)
        projection[2, 3] = -2 * near * far / (far - near)
        projection[3, 2] = -1
    else:
        projection[0, 0] = 2 / zoom / (width / height)
        projection[1, 1] = 2 / zoom
        projection[2, 2] = -2 / (far - near)
        projection[2, 3] = -(far + near) / (far - near)
        projection[3, 3] = 1
    projection[0] += np.float32(2 * offset[0] / width) * projection[3]
    projection[1] += np.float32(2 * offset[1] / height) * projection[3]
    normal = np.identity(4, dtype="f4")
    normal[:3, :3] = view[:3, :3]
    matrices = ctx.buffer(view.T.tobytes() + projection.T.tobytes() + normal.T.tobytes())
    fog_end = distance + slab / 2
    fog = ctx.buffer(struct.pack("8f", fog_end, 1 / (fog_end - max(distance, 1)), 0, 0, *fixture["background"][:3], 0))
    matrices.bind_to_uniform_block(0)
    fog.bind_to_uniform_block(1)
    color = ctx.texture((width, height), 4)
    normals = ctx.texture((width, height), 4, dtype="f2")
    depth = ctx.depth_texture((width, height))
    depth.compare_func = ""
    for texture in (depth, normals, color):
        texture.filter = (moderngl.NEAREST, moderngl.NEAREST)
        texture.repeat_x = texture.repeat_y = False
    framebuffer = ctx.framebuffer([color, normals], depth)
    framebuffer.use()
    framebuffer.clear(*fixture["background"], depth=1)
    ctx.enable(moderngl.DEPTH_TEST | moderngl.BLEND)
    ctx.depth_func = "<"
    ctx.blend_func = (moderngl.SRC_ALPHA, moderngl.ONE_MINUS_SRC_ALPHA, moderngl.ONE, moderngl.ONE_MINUS_SRC_ALPHA)
    quad = [(-1, -1), (1, -1), (-1, 1), (-1, 1), (1, -1), (1, 1)]

    def draw(kind, vertices, layout, attributes, params, mode=moderngl.TRIANGLES):
        program = programs[kind]
        draw_params = ctx.buffer(params)
        draw_params.bind_to_uniform_block(2)
        buffer = ctx.buffer(np.array(vertices, dtype="f4").tobytes())
        vao = ctx.vertex_array(program, [(buffer, layout, *attributes)])
        vao.render(mode)
        vao.release()
        buffer.release()
        draw_params.release()

    for bond in fixture["bonds"]:
        start = np.array(bond["from_radius"][:3])
        end = np.array(bond["to_edge"][:3])
        if bond["edge_mode"][2] > 0:
            vertices = [[*start, 1, *end, 1, *bond["color"], *bond["end_color"]]] * 4
            params = struct.pack("fffiffif4f", 1, bond["edge_mode"][2], 16 if bond["edge_mode"][1] else 0, 0, width, height, 0, 0, 0, 0, 0, 0)
            draw("line", vertices, "4f 4f 4f 4f", ["a_vertex1", "a_vertex2", "a_color1", "a_color2"], params, moderngl.TRIANGLE_STRIP)
            continue
        radius = bond["from_radius"][3]
        vertices = []
        for x, y in quad:
            position, direction, color_value = (start, end - start, bond["color"]) if y < 0 else (end, start - end, bond["end_color"])
            vertices.append([*position, *direction, x, y, radius, *color_value])
        draw("cylinder", vertices, "3f 3f 2f 1f 4f", ["a_vertex", "a_dir", "a_impos", "a_radius", "a_color"],
             struct.pack("ffi5f", 1, bond["to_edge"][3], int(bond["edge_mode"][0] > 1.5), 0, *bond["edge_color"]))
    for atom in fixture["atoms"]:
        position, radius = atom["center_radius"][:3], atom["center_radius"][3]
        vertices = [[*position, 1, x, y, radius, *atom["color"]] for x, y in quad]
        draw("sphere", vertices, "4f 2f 1f 4f", ["a_vertex", "a_impos", "a_radius", "a_color"],
             struct.pack("ffi5f", 1, atom["edge"][0], int(atom["edge"][1] > 1.5), 0, *atom["edge_color"]))
    for mesh in fixture["meshes"]:
        vertices = []
        for index in mesh["indices"]:
            value = mesh["vertices"][index]
            vertices.append([*value[:3], 1, *value[3:6], 0, *value[6:10]])
        draw("mesh", vertices, "4f 4f 4f", ["aVertex", "aNormal", "aColor"], struct.pack("fiii", 1, 1, 0, 0))
    if postprocess is None:
        pixels = np.frombuffer(framebuffer.read(components=4, alignment=1), dtype="u1").reshape(height, width, 4)[::-1].copy()
    else:
        pixels = postprocess(ctx, fixture, color, normals, depth)
    for resource in (framebuffer, depth, normals, color, matrices, fog):
        resource.release()
    return pixels


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, default=Path(os.environ.get("CUEMOL2_REF_DIR", "../cuemol2")))
    parser.add_argument("--artifacts", type=Path, default=Path(".cache/appearance-reference"))
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    if not args.skip_build:
        env = dict(os.environ, CUEMOL_APPEARANCE_DIR=str(args.artifacts.resolve()))
        subprocess.run(["cargo", "test", "--locked", "-p", "cuemol-desktop-core", "--features", "offscreen", "--test", "viewport_appearance", "--", "--nocapture"], env=env, check=True)
    directories = [args.reference / "src/sysdep/ogl_core", args.reference / "src/modules/molvis"]
    ctx = moderngl.create_standalone_context(backend="egl", require=330)
    programs = {}
    for name, vertex, fragment in [("mesh", "trig_vert.glsl", "trig_frag.glsl"), ("sphere", "sphere2_vertex.glsl", "sphere_frag.glsl"), ("cylinder", "cylinder_vertex.glsl", "cylinder_frag.glsl"), ("line", "linew2_vert.glsl", "linew_frag.glsl")]:
        sources = [shader_source(next(directory / filename for directory in directories if (directory / filename).is_file()), directories) for filename in (vertex, fragment)]
        program = ctx.program(vertex_shader="#version 330 core\n" + sources[0], fragment_shader="#version 330 core\n" + sources[1])
        for binding, block in enumerate(("MatricesBlock", "FogBlock", "DrawParamsBlock")):
            program[block].binding = binding
        programs[name] = program
    results = []
    for path in sorted(args.artifacts.glob("*.json")):
        if path.name == "comparison.json":
            continue
        fixture = json.loads(path.read_text())
        reference = reference_image(ctx, programs, fixture)
        Image.fromarray(reference).save(path.with_name(path.stem + "-reference.png"))
        actual = np.asarray(Image.open(path.with_name(path.stem + "-rust.png")).convert("RGBA"))
        background = np.rint(np.asarray(fixture["background"]) * 255).astype("i2")
        foreground = int((np.abs(reference.astype("i2") - background).max(axis=2) > 2).sum())
        difference = np.abs(reference.astype("i2") - actual.astype("i2"))
        result = {"case": path.stem, "mean_error": float(difference.mean()), "max_error": int(difference.max()),
                  "pixels_over_2": int((difference.max(axis=2) > 2).sum()), "foreground_pixels": foreground}
        result["passed"] = foreground >= 4 and result["mean_error"] <= 0.25 and result["pixels_over_2"] <= 8
        results.append(result)
        print(json.dumps(result))
    report = {"renderer": ctx.info["GL_RENDERER"], "cases": results}
    (args.artifacts / "comparison.json").write_text(json.dumps(report, indent=2) + "\n")
    if not results or not all(row["passed"] for row in results):
        raise SystemExit("Reference shader comparison failed")


if __name__ == "__main__":
    main()
