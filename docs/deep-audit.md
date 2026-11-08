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
| Large molecules | Load, topology, secondary structure, base pairs, edits, serialization and unchanged snapshots | Indexed topology, residue-property lookup, unchanged-edit snapshots and large DNA pairing pass the comparisons below. Frontend integration remains part of the combined-operation checks. |
| Long trajectories | Indexed reads, random seek, averaging, selection, append, playback, Undo/Redo and bounded retention | Open. |
| Maps and surfaces | Large grids, repeated replacement, cancellation, failed inputs, geometry and memory retention | Open. |
| Input boundaries | Truncated, malformed and non-finite inputs; failed loads preserve the existing scene | Extreme-coordinate overflow and invalid hydrogen-bond geometry are reproduced and fixed below. Broader reader and transactional-load checks remain open. |
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

## Unchanged edits and snapshot retention

Recalculating base pairs on a protein with no base-pair annotations previously
detached the entire shared atom array. Removing an absent residue property or
setting its existing value also detached the residue-property map. Keeping Undo
snapshots alive therefore retained unnecessary copies even though the molecule
was unchanged. A regression first reproduced the atom-storage detachment before
the fix. These operations now preserve shared storage unless a value actually
changes; stale base-pair annotations are still removed without modifying older
snapshots.

The edit probe loads 6VXX (23,694 atoms), retains ten successive molecular
snapshots, recalculates base pairs and attempts an absent-property removal at
each step. Three fresh processes per version alternate order. Median peak RSS
falls from 109.9 to 24.0 MiB and the ten-operation time from 47.4 to 31.4 ms.
Every retained snapshot and the final molecule serialize identically to the
initial state. RSS includes input loading and the equality checks, while the
time covers only the snapshot/edit loop. These measurements compare against
the already optimized topology build, isolating this additional change.

All 269 molecular checks pass after the change, with the same explicit optional
oracle limits described above. The raw comparison is
`.cache/deep-audit/noop-edits/paired.json`; the portable probe is built with
`cargo build --release --locked -p cuemol-molstr --example molecular_edit_bench`
and accepts an input PDB and a positive snapshot count.

## Nucleic-acid neighbors and coordinate boundaries

Base-pair search now builds a temporary sparse index of eligible edge atoms and
checks nearby cells before applying the unchanged distance and plane rules.
Candidate ordering preserves the previous nearest-atom and partner tie rules.
Atom-name comparisons no longer allocate uppercase strings. Infinite cutoffs
retain the exhaustive fallback. The index omits non-finite positions and uses
checked cell arithmetic at extreme finite coordinates.

The fixture generator tiles the public 1BNA model into independent copies while
preserving atom fields and unique residue indices. Three fresh processes per
version, with two reads/recalculations per process, give these medians:

| Input | Atoms | Repeated read, before/after (ms) | Base pairs, before/after (ms) | Peak RSS, before/after (MiB) |
| --- | --- | --- | --- | --- |
| 1BNA | 566 | 1.12 / 0.699 | 0.497 / 0.155 | 4.48 / 4.31 |
| 32 copies | 18,112 | 426 / 21.4 | 447 / 5.61 | 31.09 / 30.83 |
| 64 copies | 36,224 | 1,675 / 47.5 | 1,650 / 13.2 | 58.95 / 59.11 |

All QDF bytes and final hashes match the preceding exhaustive implementation.
The installed C++ CueMol 3.0.0.554 also exports identical residue-to-partner
assignments: 12 pairs for the original model and 768 for the 64-copy model.
The C++ export is read back through the QDF reader before comparison. The
reference test covers those unambiguous structures; seeded differential checks
separately cover equal-distance partners, insertion codes, case/whitespace,
zero/infinite cutoffs, tilt boundaries and non-finite edge coordinates.

Two additional regressions failed before repair. A valid PDB coordinate of
positive or negative `1e30` overflowed a signed grid-cell addition during DSSP
in debug builds. A non-finite donor position was also converted into the
strongest possible hydrogen bond by the energy clamp. DSSP now uses the checked
neighbor index and rejects non-finite bond distances. All 273 molecular checks
pass, and final decoded/serialized results still match all five public molecular
fixtures. This does not claim that every reader rejects non-finite input.

Evidence is under `.cache/deep-audit/basepair-index`. The portable helpers are
`scripts/make-molecular-benchmark-fixture.py`,
`scripts/export-reference-base-pairs.mjs` and the `base_pair_check` example.
The reference export runs with the reference application's Node runtime and
does not modify its installation or source checkout.
