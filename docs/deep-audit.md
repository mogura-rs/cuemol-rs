# Repeated-operation and performance audit

This audit starts from `b492599` after the preceding five-platform and public
browser validation. It tests combined operations and larger workloads beyond
the earlier feature checks. Each completed row must identify its input,
observable result, measurement scope and remaining limits. Generated inputs,
logs and captures belong in `.cache/deep-audit`.

The existing exclusions remain physical screen/input permission checks on the
separately authorized Mac and hardware quad-buffer presentation. Paid provider
requests remain outside tests under the repository instructions.

| Area | Required checks | Current evidence |
| --- | --- | --- |
| Large molecules | Load, topology, secondary structure, base pairs, edits, serialization and unchanged snapshots | Indexed topology and residue-property lookup pass the comparisons below; base-pair and repeated-edit work remains open. |
| Long trajectories | Indexed reads, random seek, averaging, selection, append, playback, Undo/Redo and bounded retention | Open. |
| Maps and surfaces | Large grids, repeated replacement, cancellation, failed inputs, geometry and memory retention | Open. |
| Input boundaries | Truncated, malformed and non-finite inputs; failed loads preserve the existing scene | Open. |
| Combined scene edits | Multiple scenes, selection, cameras, animation, Undo/Redo and save/reload | Open. |
| Rendering jobs | Repeated still/movie output, concurrent edits, cancellation/failure and worker/output cleanup | Open. |
| Application lifecycle | Repeated opens/closes, settings/restart, CLI/MCP/Python and frontend resource retention | Open. |
| Delivery | Affected local checks, complete CI, five native packages and deployed WASM at the final product revision | Open for this audit's changes. |

## Molecular lookup and topology

Previously each residue-property assignment scanned all atoms and constructed
temporary chain names, while each generated bond scanned all prior bonds.
The molecular model now builds a shared immutable residue index on demand and
invalidates it after identity edits or snapshot restoration. Topology generation
uses a temporary bond-pair set, reuses the existing bond buffer, and borrows
residue name maps instead of cloning them for inter-residue links. Deposited
persistent bonds retain their order, orientation, bond order and stereo value.

Five public PDB inputs cover 1CRN, 4HHB, 1BNA, 6VXX and 5XNL. The last contains
98,986 atoms, 10,930 residues and 101,764 inferred/deposited bonds. Three fresh
processes per version run each input twice, alternating version order. The
comparison checks exact QDF bytes and final molecular hashes, including topology,
secondary structure, base pairs and residue properties. Every comparison passes.

| Input | Repeated read, before/after (ms) | Repeated topology, before/after (ms) | Residue properties, before/after (ms) |
| --- | --- | --- | --- |
| 1CRN | 0.740 / 0.922 | 0.404 / 0.399 | 0.128 / 0.015 |
| 4HHB | 37.6 / 5.52 | 34.4 / 2.41 | 29.1 / 0.108 |
| 1BNA | 1.76 / 0.858 | 1.01 / 0.244 | 0.681 / 0.014 |
| 6VXX | 592 / 46.6 | 559 / 13.6 | 440 / 0.465 |
| 5XNL | 8,252 / 381 | 7,998 / 51.2 | 6,233 / 2.06 |

Values are medians on one Apple-silicon macOS host. The benchmark's peak RSS
includes retained QDF comparison bytes and repeated serialization, not just
normal application loading. Its 5XNL median rises from 96.0 to 124.4 MiB;
an isolated `cuemol pdb-info` comparison instead measures 92.1 to 97.5 MiB
and 8.09 to 0.387 seconds across three fresh processes per version. The output
is byte-identical. This optimization trades about 5.4 MiB of reader peak memory
for speed on that input; it is not a memory reduction. Repeated serialization
amplifies the peak difference in the broader probe.
Small-input timing is mixed, so these results do not establish a universal
speedup. Raw samples and input provenance are retained in
`.cache/deep-audit/topology-index/paired.json` and
`.cache/deep-audit/fixtures/sources.json`.

The molecular crate run passes 268 checks across 21 groups, with zero failures
or ignored cases. The copied reference fixtures were present for fixture tests;
three optional native-oracle wrappers still report their missing external
oracle outputs and are not counted as executed C++ comparisons. New regressions
exercise identity changes, clone isolation, copy/delete/restore, transfer and
Undo, plus repeated topology reconstruction with persistent stereo bonds.

Reproduce the paired measurements with separately built baseline and candidate
executables:

```sh
cargo build --release --locked -p cuemol-molstr --example molecular_bench
uv run --no-project python scripts/benchmark-molecular-structure.py \
  --before .cache/deep-audit/baseline/molecular_bench \
  --after target/release/examples/molecular_bench \
  --input .cache/deep-audit/fixtures/*.pdb \
  --samples 3 --iterations 2 \
  --output .cache/deep-audit/paired.json
```
