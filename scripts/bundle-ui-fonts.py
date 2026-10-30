#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["fonttools==4.61.1"]
# ///
"""Bundle a pinned Japanese UI font and its license in the browser assets."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
from urllib.parse import quote
from urllib.request import urlopen

from fontTools import version as fonttools_version
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parent.parent
REVISION = "66a36c8c94b1a5d992ee4e7f392fccfe4945767c"
BASE = f"https://raw.githubusercontent.com/google/fonts/{REVISION}/ofl/notosansjp"
SOURCES = {
    "NotoSansJP[wght].ttf": "c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f",
    "OFL.txt": "1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", type=Path, default=ROOT / "web/pkg/fonts")
    args = parser.parse_args()
    cache = ROOT / ".cache/ui-fonts"
    cache.mkdir(parents=True, exist_ok=True)
    for name, expected in SOURCES.items():
        path = cache / name
        if path.is_file() and digest(path) == expected:
            continue
        with urlopen(f"{BASE}/{quote(name)}", timeout=60) as response:
            data = response.read(32 * 1024 * 1024)
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"UI font checksum mismatch: {name}")
        path.write_bytes(data)
    provenance = {"repository": "https://github.com/google/fonts", "revision": REVISION,
                  "sources": SOURCES, "fonttools": fonttools_version, "weight": 400}
    output = cache / "NotoSansJP-Regular.ttf"
    stamp = cache / "SOURCE.json"
    recorded = json.loads(stamp.read_text()) if stamp.is_file() else {}
    if (not output.is_file() or recorded.get("build") != provenance
            or recorded.get("sha256") != digest(output)):
        with TTFont(cache / "NotoSansJP[wght].ttf", recalcTimestamp=False) as font:
            regular = instantiateVariableFont(font, {"wght": 400}, inplace=True)
            regular.save(output)
        stamp.write_text(json.dumps({"build": provenance, "sha256": digest(output)}, indent=2) + "\n")
    args.out_dir.mkdir(parents=True, exist_ok=True)
    for source in (output, stamp, cache / "OFL.txt"):
        shutil.copyfile(source, args.out_dir / source.name)
    print(f"Bundled Japanese UI font ({output.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
