#!/usr/bin/env python3
"""Compare live AO/AA output against CueMol's original GLSL passes on EGL.

Use uv run --no-project --with moderngl --with numpy --with pillow python.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess

import moderngl
import numpy as np
from PIL import Image

spec = importlib.util.spec_from_file_location("appearance", Path(__file__).with_name("check-appearance.py"))
appearance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(appearance)

VERTEX = """#version 330 core
out vec2 v_uv;
void main() {
    vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"""


class Effects:
    def __init__(self, ctx, reference):
        self.ctx = ctx
        self.programs = {}
        directory = reference / "src/sysdep/ogl_core"
        for name in ("gtao", "ao_denoise", "ao_composite", "fxaa", "smaa_edges", "smaa_weights", "smaa_blend"):
            self.programs[name] = ctx.program(vertex_shader=VERTEX, fragment_shader="#version 330 core\n" + (directory / f"{name}_frag.glsl").read_text())
        assets = Path(__file__).resolve().parents[1] / "crates/cuemol-desktop-core/assets/smaa"
        self.area = ctx.texture((160, 560), 2, (assets / "area.dat").read_bytes())
        self.search = ctx.texture((66, 33), 1, (assets / "search.dat").read_bytes(), alignment=1)
        self.area.filter = (moderngl.LINEAR, moderngl.LINEAR)
        self.search.filter = (moderngl.NEAREST, moderngl.NEAREST)
        for texture in (self.area, self.search):
            texture.repeat_x = texture.repeat_y = False
        self.noise = 0.0

    def __call__(self, ctx, fixture, color, normals, depth):
        width, height = fixture["size"]
        options = fixture["effects"]
        ao_size = ((width + 1) // 2, (height + 1) // 2) if options["half"] else (width, height)
        distance, slab, zoom = (fixture[key] for key in ("distance", "slab", "zoom"))
        near, far = max(0.1, distance - slab / 2), distance + slab
        perspective = fixture["perspective"]
        half_view = zoom / 2 / (distance if perspective else 1)
        unpack = (far * near / (far - near), far / (far - near)) if perspective else (near, far - near)
        uniforms = {
            "u_depthUnpack": unpack, "u_ndcToViewMul": (2 * half_view * width / height, 2 * half_view),
            "u_ndcToViewAdd": (-half_view * width / height, -half_view), "u_viewportPixelSize": (1 / ao_size[0], 1 / ao_size[1]),
            "u_effectRadius": options["radius"], "u_finalValuePower": options["power"], "u_sliceCount": options["slices"],
            "u_stepCount": options["steps"], "u_hasNormal": 1, "u_debugMode": 0, "u_aoNoiseOffset": self.noise,
            "u_maxScreenspaceRadius": 256 * ao_size[0] / width, "u_isOrtho": int(not perspective),
            "u_fogEnd": distance + slab / 2, "u_fogScale": 1 / (distance + slab / 2 - max(distance, 1)),
            "u_aoTexelSize": (1 / ao_size[0], 1 / ao_size[1]), "u_hasAO": int(options["ao"]),
            "u_upsample": int(options["half"]), "u_rcpFrame": (1 / width, 1 / height), "u_threshold": options["threshold"],
        }
        allocated = []
        ctx.disable(moderngl.DEPTH_TEST | moderngl.BLEND)
        color.filter = (moderngl.LINEAR, moderngl.LINEAR)

        def draw(name, size, textures, linear=False):
            program = self.programs[name]
            output = ctx.texture(size, 4)
            output.repeat_x = output.repeat_y = False
            output.filter = (moderngl.LINEAR, moderngl.LINEAR) if linear else (moderngl.NEAREST, moderngl.NEAREST)
            framebuffer = ctx.framebuffer([output])
            framebuffer.use()
            framebuffer.clear(0, 0, 0, 0)
            for key, value in uniforms.items():
                if key in program:
                    program[key].value = value
            for unit, (key, texture) in enumerate(textures.items()):
                texture.use(unit)
                program[key].value = unit
            vao = ctx.vertex_array(program, [])
            vao.render(vertices=3)
            vao.release()
            framebuffer.release()
            allocated.append(output)
            return output

        if options["ao"]:
            ao = draw("gtao", ao_size, {"u_depthTex": depth, "u_normalTex": normals})
            ao = draw("ao_denoise", ao_size, {"u_aoTex": ao})
            color = draw("ao_composite", (width, height), {"u_colorTex": color, "u_aoTex": ao, "u_depthTex": depth}, True)
        if options["aa"] == "fxaa":
            color = draw("fxaa", (width, height), {"u_colorTex": color})
        elif options["aa"] == "smaa":
            edges = draw("smaa_edges", (width, height), {"u_colorTex": color}, True)
            weights = draw("smaa_weights", (width, height), {"u_edgesTex": edges, "u_areaTex": self.area, "u_searchTex": self.search}, True)
            color = draw("smaa_blend", (width, height), {"u_colorTex": color, "u_weightsTex": weights})
        image = np.frombuffer(color.read(alignment=1), dtype="u1").reshape(height, width, 4)[::-1].copy()
        for resource in allocated:
            resource.release()
        return image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, default=Path(os.environ.get("CUEMOL2_REF_DIR", "../cuemol2")))
    parser.add_argument("--artifacts", type=Path, default=Path(".cache/effects-reference"))
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    if not args.skip_build:
        env = dict(os.environ, CUEMOL_EFFECTS_DIR=str(args.artifacts.resolve()))
        subprocess.run(["cargo", "test", "--locked", "-p", "cuemol-desktop-core", "--features", "offscreen", "--test", "viewport_effects", "--", "--nocapture"], env=env, check=True)
    ctx = moderngl.create_standalone_context(backend="egl", require=330)
    directories = [args.reference / "src/sysdep/ogl_core", args.reference / "src/modules/molvis"]
    programs = {}
    for name, vertex, fragment in [("mesh", "trig_vert.glsl", "trig_frag.glsl"), ("sphere", "sphere2_vertex.glsl", "sphere_frag.glsl"), ("cylinder", "cylinder_vertex.glsl", "cylinder_frag.glsl"), ("line", "linew2_vert.glsl", "linew_frag.glsl")]:
        sources = [appearance.shader_source(next(directory / filename for directory in directories if (directory / filename).is_file()), directories) for filename in (vertex, fragment)]
        program = ctx.program(vertex_shader="#version 330 core\n" + sources[0], fragment_shader="#version 330 core\n" + sources[1])
        for binding, block in enumerate(("MatricesBlock", "FogBlock", "DrawParamsBlock")):
            program[block].binding = binding
        programs[name] = program
    effects = Effects(ctx, args.reference)
    results = []
    for path in sorted(args.artifacts.glob("*.json")):
        if path.name == "comparison.json":
            continue
        fixture = json.loads(path.read_text())
        offsets = fixture["effects"]["offsets"]
        if offsets:
            accumulation = np.zeros((fixture["size"][1], fixture["size"][0], 4), dtype="f2")
            for index, offset in enumerate(offsets):
                effects.noise = float(np.float32(index) * np.float32(0.6180339887) % np.float32(1))
                sample = appearance.reference_image(ctx, programs, fixture, effects, offset)
                accumulation = (accumulation.astype("f4") + sample.astype("f4") / (255 * len(offsets))).astype("f2")
            reference = np.rint(np.clip(accumulation.astype("f4"), 0, 1) * 255).astype("u1")
        else:
            effects.noise = 0.0
            reference = appearance.reference_image(ctx, programs, fixture, effects)
        Image.fromarray(reference).save(path.with_name(path.stem + "-reference.png"))
        actual = np.asarray(Image.open(path.with_name(path.stem + "-rust.png")).convert("RGBA"))
        delta = np.abs(reference.astype("i2") - actual.astype("i2"))
        background = np.rint(np.asarray(fixture["background"]) * 255).astype("i2")
        foreground = int((np.abs(reference.astype("i2") - background).max(axis=2) > 2).sum())
        result = {"case": path.stem, "mean_error": float(delta.mean()), "max_error": int(delta.max()),
                  "pixels_over_2": int((delta.max(axis=2) > 2).sum()), "foreground_pixels": foreground}
        result["passed"] = foreground >= 4 and result["mean_error"] <= 0.25 and result["pixels_over_2"] <= 16
        print(json.dumps(result))
        results.append(result)
    (args.artifacts / "comparison.json").write_text(json.dumps({"renderer": ctx.info["GL_RENDERER"], "cases": results}, indent=2) + "\n")
    if not results or not all(row["passed"] for row in results):
        raise SystemExit("Reference postprocess comparison failed")


if __name__ == "__main__":
    main()
