import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time


def pdb_input(atoms):
    lines = []
    for index in range(atoms):
        chain = chr(65 + index // 9999)
        xyz = (index % 100 * 4, index // 100 % 100 * 4, index // 10000 * 4)
        lines.append(f"HETATM{index + 1:5d}  C1  LIG {chain}{index % 9999 + 1:4d}    "
                     + "".join(f"{coordinate:8.3f}" for coordinate in xyz)
                     + "  1.00 10.00           C  \n")
    return "".join(lines) + "END\n"


def measure(binary, atoms, directory):
    pdb, qdf = directory / "input.pdb", directory / "output.qdf"
    pdb.write_text(pdb_input(atoms))
    source = """
var streams = cuemol.getService('StreamManager');
function read(path, format) {
    var reader = streams.createHandler(format, 0);
    var object = reader.createDefaultObj();
    reader.attach(object); reader.path = path; reader.read(); reader.detach();
    return object;
}
var start = Date.now();
var original = read(PDB, 'pdb');
var parsed = Date.now();
var writer = streams.createHandler('qdfmol', 1);
writer.attach(original); writer.path = QDF; writer.write(); writer.detach();
var saved = Date.now();
var restored = read(QDF, 'qdfmol');
var finished = Date.now();
print(JSON.stringify({read_ms: parsed-start, write_ms: saved-parsed, reload_ms: finished-saved,
    atoms: restored.getAtomSize(), first_x: restored.getAtomByID(1).pos.x,
    last_x: restored.getAtomByID(ATOMS).pos.x}));
""".replace("PDB", json.dumps(str(pdb))).replace("QDF", json.dumps(str(qdf))).replace("ATOMS", str(atoms))
    env = dict(os.environ, CUEMOL_CONFIG_DIR=str(directory / "config"))
    with tempfile.TemporaryFile() as output:
        started = time.monotonic()
        process = subprocess.Popen([str(binary), "-c", source], env=env, stdout=output, stderr=subprocess.STDOUT)
        peak_mib = None
        if hasattr(os, "wait4"):
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    peak_mib = usage.ru_maxrss / (1024 ** 2 if sys.platform == "darwin" else 1024)
                    break
                if time.monotonic() - started > 180:
                    process.kill()
                    process.wait()
                    raise TimeoutError("molecular I/O benchmark exceeded 180 seconds")
                time.sleep(0.01)
        else:
            try:
                process.wait(timeout=180)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise
        output.seek(0)
        text = output.read().decode("utf-8", errors="replace")
    if process.returncode:
        raise RuntimeError(text)
    result = json.loads(text.strip().splitlines()[-1])
    assert result["atoms"] == atoms and result["first_x"] == 0
    assert result["last_x"] == (atoms - 1) % 100 * 4
    return dict(result, peak_mib=peak_mib, qdf_sha256=hashlib.sha256(qdf.read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path)
    parser.add_argument("--atoms", type=int, nargs="+", default=[1000, 5000, 20000])
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.samples < 1 or any(not 1 <= count <= 99999 for count in args.atoms):
        parser.error("use positive samples and 1..99999 atoms per PDB")
    report = {"samples": args.samples, "results": []}
    hashes = {}
    with tempfile.TemporaryDirectory(prefix="cuemol-molecule-bench-") as directory:
        for label, binary in (("before", args.before), ("after", args.after)):
            if binary is None:
                continue
            for atoms in args.atoms:
                samples = [measure(binary.resolve(), atoms, Path(directory)) for _ in range(args.samples)]
                for sample in samples:
                    assert hashes.setdefault(atoms, sample["qdf_sha256"]) == sample["qdf_sha256"]
                row = {"version": label, "atoms": atoms, "samples": samples,
                       "median": {key: statistics.median(sample[key] for sample in samples)
                                  for key in ("read_ms", "write_ms", "reload_ms")},
                       "median_peak_mib": statistics.median(sample["peak_mib"] for sample in samples)
                       if samples[0]["peak_mib"] is not None else None}
                report["results"].append(row)
                print(json.dumps(row), flush=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
