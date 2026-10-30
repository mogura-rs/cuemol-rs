#!/usr/bin/env python3
"""Compare sequence refreshes in fresh native and optional reference processes."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time


def generate_fixture(path, atoms):
    if not 1 <= atoms <= 39996:
        raise ValueError("the generated PDB supports 1 to 39996 atoms")
    with path.open("w") as output:
        for index in range(atoms):
            x = (index % 100) * 1.5
            y = ((index // 100) % 100) * 1.5
            z = (index // 10000) * 1.5
            output.write(
                f"ATOM  {index + 1:5d}  C{index % 4}  GLY A{index // 4 + 1:4d}    "
                f"{x:8.3f}{y:8.3f}{z:8.3f}  1.00 10.00           C  \n"
            )
        output.write("END\n")


def measure(command, prefix, *, reference=False):
    stdout = prefix.with_suffix(".stdout")
    stderr = prefix.with_suffix(".stderr")
    env = os.environ.copy()
    if reference:
        env["ELECTRON_RUN_AS_NODE"] = "1"
    started = time.monotonic()
    with stdout.open("w") as out, stderr.open("w") as err:
        with subprocess.Popen(command, stdout=out, stderr=err, env=env) as process:
            try:
                _, status, usage = os.wait4(process.pid, 0)
            except BaseException:
                process.terminate()
                process.wait()
                raise
            process.returncode = os.waitstatus_to_exitcode(status)
            if process.returncode:
                raise RuntimeError(f"{prefix.name} exited with {process.returncode}; see {stderr.name}")
    elapsed = time.monotonic() - started
    lines = stdout.read_text().splitlines()
    if reference:
        result = next(line.removeprefix("SEQUENCE_RESULT ") for line in lines if line.startswith("SEQUENCE_RESULT "))
    else:
        result = next(line for line in reversed(lines) if line.startswith("{"))
    data = json.loads(result)
    data["process_elapsed_s"] = elapsed
    data["peak_rss_mib"] = usage.ru_maxrss / (1024 * 1024 if sys.platform == "darwin" else 1024)
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--fixture", type=Path)
    inputs.add_argument("--generate-atoms", type=int)
    parser.add_argument("--reference-binary", type=Path)
    parser.add_argument("--reference-app", type=Path)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--reference-version")
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--frames", type=int, default=60)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.samples < 1 or args.frames < 1:
        parser.error("samples and frames must be positive")
    if bool(args.reference_binary) != bool(args.reference_app):
        parser.error("reference-binary and reference-app are required together")
    if not hasattr(os, "wait4"):
        parser.error("this benchmark requires POSIX wait4 for per-process peak RSS")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.generate_atoms is not None:
        args.fixture = args.output.parent / f"sequence-{args.generate_atoms}.pdb"
        try:
            generate_fixture(args.fixture, args.generate_atoms)
        except ValueError as error:
            parser.error(str(error))
    raw = args.output.parent / f"{args.output.stem}-raw"
    raw.mkdir(exist_ok=True)
    report = {
        "platform": platform.platform(), "machine": platform.machine(),
        "revision": args.revision, "reference_version": args.reference_version,
        "fixture": args.fixture.name, "fixture_sha256": hashlib.sha256(args.fixture.read_bytes()).hexdigest(),
        "samples": args.samples, "frames": args.frames, "results": [],
        "stages": "first_ms and ms_per_frame measure sequence data construction or cache lookup; process RSS includes loading and runtime",
    }
    digest = None
    for sample in range(args.samples):
        conditions = ["reference"] if args.reference_binary else []
        conditions += ["uncached", "cached"]
        for condition in conditions:
            if condition == "reference":
                command = [str(args.reference_binary.resolve()), str(Path(__file__).with_name("benchmark-reference-sequence.mjs")), str(args.reference_app.resolve()), str(args.fixture.resolve()), str(args.frames)]
            else:
                command = [str(args.binary.resolve()), str(args.fixture.resolve()), str(args.frames), condition]
            result = measure(command, raw / f"{condition}-{sample + 1}", reference=condition == "reference")
            if digest is None:
                digest = result["digest"]
            if result["digest"] != digest:
                raise RuntimeError(f"{condition} sequence/selection differs from the first condition")
            report["results"].append({"condition": condition, "sample": sample + 1, **result})
            args.output.write_text(json.dumps(report, indent=2) + "\n")
            print(f"{condition} {sample + 1}/{args.samples}: {result['ms_per_frame']:.6f} ms, {result['peak_rss_mib']:.3f} MiB", flush=True)


if __name__ == "__main__":
    main()
