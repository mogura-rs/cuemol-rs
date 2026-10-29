import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time


def measure(binary, size, operations, directory):
    source = f"""
var start = Date.now();
var bytes = cuemol.ByteArray(); bytes.init(bytes.UINT8, {size});
var sum = 0;
for (var i = 0; i < {operations}; i++) {{
    var index = (i * 17) % {size};
    bytes.setValue(index, i % 256);
    sum += bytes.getValue(index);
}}
print(JSON.stringify({{milliseconds: Date.now() - start, checksum: sum}}));
"""
    env = dict(os.environ, CUEMOL_CONFIG_DIR=str(directory / "config"))
    with tempfile.TemporaryFile() as output:
        started = time.monotonic()
        process = subprocess.Popen([str(binary), "-c", source], env=env,
                                   stdout=output, stderr=subprocess.STDOUT)
        peak_mib = None
        if hasattr(os, "wait4"):
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    peak_mib = usage.ru_maxrss / (1024 ** 2 if sys.platform == "darwin" else 1024)
                    break
                if time.monotonic() - started > 120:
                    process.kill()
                    process.wait()
                    raise TimeoutError("byte-array benchmark exceeded 120 seconds")
                time.sleep(0.01)
        else:
            try:
                process.wait(timeout=120)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise
        elapsed = time.monotonic() - started
        output.seek(0)
        text = output.read().decode("utf-8", errors="replace")
    if process.returncode:
        raise RuntimeError(text)
    result = json.loads(text.strip().splitlines()[-1])
    if result["checksum"] != sum(i % 256 for i in range(operations)):
        raise AssertionError(result)
    return dict(result, process_seconds=elapsed, peak_mib=peak_mib)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path)
    parser.add_argument("--sizes", type=int, nargs="+", default=[1024, 8192, 65536])
    parser.add_argument("--operations", type=int, default=256)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.operations < 1 or args.samples < 1 or any(size < 1 for size in args.sizes):
        parser.error("sizes, operations, and samples must be positive")
    report = {"operations": args.operations, "samples": args.samples, "results": []}
    with tempfile.TemporaryDirectory(prefix="cuemol-byte-bench-") as directory:
        for label, binary in (("before", args.before), ("after", args.after)):
            if binary is None:
                continue
            for size in args.sizes:
                samples = [measure(binary.resolve(), size, args.operations, Path(directory))
                           for _ in range(args.samples)]
                row = {"version": label, "bytes": size, "samples": samples,
                       "median_ms": statistics.median(s["milliseconds"] for s in samples),
                       "median_peak_mib": statistics.median(s["peak_mib"] for s in samples)
                       if samples[0]["peak_mib"] is not None else None}
                report["results"].append(row)
                print(json.dumps(row), flush=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
