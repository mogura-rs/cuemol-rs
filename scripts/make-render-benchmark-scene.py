"""Create a portable QSC render fixture from the first model of a PDB file."""

import argparse
import math
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--renderer", choices=("ribbon", "ballstick", "cpk"), default="ribbon")
    parser.add_argument("--width", type=int, default=640)
    parser.add_argument("--height", type=int, default=480)
    parser.add_argument("--supersample", type=int, default=3)
    parser.add_argument("--gi", action=argparse.BooleanOptionalAction, default=True)
    args = parser.parse_args()
    if args.width < 1 or args.height < 1 or not 1 <= args.supersample <= 8:
        parser.error("dimensions must be positive and supersample must be between 1 and 8")
    if args.input.resolve() == args.output.resolve():
        parser.error("input and output must be different files")
    source = args.input.read_text()
    coordinates = []
    for line in source.splitlines():
        if line.startswith("ENDMDL"):
            break
        if line[:6] in ("ATOM  ", "HETATM"):
            coordinates.append(tuple(float(line[start:start + 8]) for start in (30, 38, 46)))
    if not coordinates:
        parser.error("the first PDB model contains no atom coordinates")
    if any(not math.isfinite(value) for point in coordinates for value in point):
        parser.error("PDB coordinates must be finite")
    minimum = [min(point[axis] for point in coordinates) for axis in range(3)]
    maximum = [max(point[axis] for point in coordinates) for axis in range(3)]
    center = ",".join(str((low + high) * 0.5) for low, high in zip(minimum, maximum))
    span = [high - low for low, high in zip(minimum, maximum)]
    zoom = 1.2 * max(span[1], span[0] * args.height / args.width, 1.)
    scene = f'''<?xml version="1.0" encoding="utf-8"?>
<scene>
  <qsc_opts version="QDF0"/>
  <object type="MolCoord" name="benchmark" src="datachunk:00000" srctype="qdfpdb">
    <renderer type="{args.renderer}" group="" name="{args.renderer}"/>
  </object>
  <camera center="({center})" distance="200" name="__current" rotation="(0,0,0,1)" slab="500" zoom="{zoom}"/>
  <appdata id="render" type="RenderSettings" backend="umbreon" clipPlane="false" height="{args.height}" projection="orthographic" width="{args.width}">
    <umbreon denoise="None" supersample="{args.supersample}" useGI="{str(args.gi).lower()}"/>
  </appdata>
</scene>

========== End of XML ==========

==========START datachunk:00000==========
{source.rstrip()}
==========END datachunk:00000==========
'''
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(scene)


if __name__ == "__main__":
    main()
