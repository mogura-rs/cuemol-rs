#!/usr/bin/env -S uv run --script
"""Bundle the pinned browser video muxer with its license and provenance."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parent.parent
VERSION = "1.65.2"
REPOSITORY = "https://github.com/Vanilagy/mediabunny"
SOURCES = {
    "mediabunny.js": (
        f"{REPOSITORY}/releases/download/v{VERSION}/mediabunny.min.mjs",
        "08149edcc8cf980da65d94a0a54a8fc0fa85a65f89b66f55c906f33618363919",
    ),
    "LICENSE": (
        f"https://raw.githubusercontent.com/Vanilagy/mediabunny/v{VERSION}/LICENSE",
        "3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04",
    ),
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", type=Path, default=ROOT / "web/pkg/movie")
    args = parser.parse_args()
    cache = ROOT / ".cache/browser-movie"
    cache.mkdir(parents=True, exist_ok=True)
    for name, (url, expected) in SOURCES.items():
        path = cache / name
        if path.is_file() and digest(path) == expected:
            continue
        with urlopen(url, timeout=60) as response:
            data = response.read(8 * 1024 * 1024)
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"Browser movie dependency checksum mismatch: {name}")
        path.write_bytes(data)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    for name in SOURCES:
        shutil.copyfile(cache / name, args.out_dir / name)
    (args.out_dir / "SOURCE.json").write_text(json.dumps({
        "repository": REPOSITORY, "version": VERSION,
        "license": "MPL-2.0",
        "files": {name: {"url": url, "sha256": sha} for name, (url, sha) in SOURCES.items()},
    }, indent=2) + "\n")
    print(f"Bundled browser movie encoder support {VERSION}")


if __name__ == "__main__":
    main()
