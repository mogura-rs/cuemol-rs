"""Check an installed Python package's native command outside the source tree."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--environment", type=Path, required=True)
    args = parser.parse_args()
    directory = args.environment.resolve()
    binary = directory / ("Scripts/cuemol.exe" if os.name == "nt" else "bin/cuemol")
    with tempfile.TemporaryDirectory(prefix="cuemol-package-") as temporary:
        work = Path(temporary)
        config = work / "config"
        config.mkdir()
        (config / "startup.js").write_text("scene.setName('startup-scene');", encoding="utf-8")
        env = {key: value for key, value in os.environ.items() if not key.startswith("CUEMOL_")}
        env.update(CUEMOL_CONFIG_DIR=str(config), CUEMOL_CACHE_DIR=str(work / "cache"))

        def run(*arguments):
            return subprocess.run(
                [str(binary), *arguments], cwd=work, env=env, check=True,
                text=True, encoding="utf-8", capture_output=True, timeout=60,
            ).stdout

        version = run("--version").strip()
        skill = run("skill")
        if not skill.startswith("---\nname: cuemol\n") or "cuemol mcp tools" not in skill:
            raise RuntimeError("The installed command did not return its standalone agent skill")
        if run("-c", "print(scene.name); print(21 * 2);") != "startup-scene\n42\n":
            raise RuntimeError("Startup or command-line scripting failed in the installed package")
        tools = json.loads(run("mcp", "tools"))["tools"]
        if "get_scene_state" not in {tool["name"] for tool in tools}:
            raise RuntimeError("The installed MCP catalog lacks scene access")
        if Path(run("config", "path").strip()) != config:
            raise RuntimeError("The installed command did not use the configured directory")
        print(json.dumps({"ok": True, "version": version, "tools": len(tools),
                          "skill_bytes": len(skill.encode()), "startup": True}))


if __name__ == "__main__":
    main()
