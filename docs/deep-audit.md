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
| Large molecules | Load, topology, secondary structure, base pairs, edits, serialization and unchanged snapshots | Indexed topology, residue-property lookup, unchanged-edit snapshots and large DNA pairing pass the comparisons below. The large browser probe displays 5XNL and restores all eight rename edits through Undo/Redo on both backends. |
| Long trajectories | Indexed reads, random seek, averaging, selection, append, playback, Undo/Redo and bounded retention | Averaging and selection pass exact-coordinate, cell and failure checks across storage/block types. The 2,000-frame indexed run below visits every frame. Both browser backends also index the 480 MB file, check eleven nonsequential seeks and pass the repeated-scene checks below. |
| Maps and surfaces | Large grids, repeated replacement, cancellation, failed inputs, geometry and memory retention | Large OpenDX grids, XPLOR/BRIX reader boundaries and shared surface/LW payload retention pass the checks below. Both backends display a 128-cubed grid and a 147,456-vertex surface, then restore eight edits through Undo/Redo. |
| Input boundaries | Truncated, malformed and non-finite inputs; failed loads preserve the existing scene | Extreme-coordinate overflow, invalid hydrogen-bond geometry and five malformed-map failures are reproduced and fixed below. Shared application loads preserve existing scenes on these map errors. The browser failure regression also preserves redo, releases its parser worker and accepts a later valid input. |
| Combined scene edits | Multiple scenes, selection, cameras, animation, Undo/Redo and save/reload | Both browser backends pass twelve combined cycles, embedded saves, exact averaged coordinates and the broader animation/history checks. |
| Rendering jobs | Repeated still/movie output, concurrent edits, cancellation/failure and worker/output cleanup | Browser still/movie, cancellation and concurrent-edit checks pass. Real native GPU, FFmpeg and POV-Ray output checks also pass; see below. |
| Application lifecycle | Repeated opens/closes, settings/restart, CLI/MCP/Python and frontend resource retention | Forty MCP cycles, Python state, configuration restarts, twelve small-scene cycles per browser backend and six large-molecule cycles per backend pass. |
| Delivery | Affected local checks, complete CI, five native packages and deployed WASM at the final product revision | Product revision `f6c4154` passes complete CI, all five native build/package/OS-input jobs and public WASM checks. See the delivery evidence below. |

The shared run passes 1,867 checks, with four opt-in cases then passing
against real FFmpeg, POV-Ray and public metadata services. Native egui/UI tests
pass 388 checks. Tauri model checks pass 214 cases, and all six opt-in GPU/movie
cases pass separately. Clippy completes for the affected molecule, trajectory
and shared-core targets with the existing style warnings. The browser integration and final distribution checks below
are additional requirements, rather than being inferred from these counts.

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

## Trajectory averaging and selected coordinates

The previous averaging path decoded the requested center frame twice and
materialized every contributing frame as a full double-precision vector array.
Load selections also converted all source atoms before keeping the selected
ones. The shared reader now borrows memory/owned frames, decodes each file frame
once and adds coordinates directly to one output array. Selection maps are
composed before conversion. Summation order, floating-point bits, center-frame
cell parameters and transactional application are preserved. Normal playback
without averaging keeps its existing direct path.

Three fresh processes per version alternate order. An averaging radius of four
uses up to nine frames, with shorter windows at the ends. The 2,000-frame DCD
contains 20,000 atoms per frame and occupies about 458 MiB; a coprime seek step
visits every frame. Hashing checks all coordinates after each application but is
excluded from the reported application time, as is rendering.

| Source | Atoms / frames / updates | Application time, before/after (ms) | Peak RSS, before/after (MiB) |
| --- | --- | --- | --- |
| Memory | 20,000 / 64 / 64 | 17.0 / 10.9 | 63.84 / 63.39 |
| Indexed DCD | 20,000 / 64 / 64 | 47.2 / 39.1 | 24.61 / 24.38 |
| Memory | 200,000 / 32 / 32 | 117.9 / 77.8 | 325.73 / 319.70 |
| Indexed DCD | 200,000 / 32 / 32 | 274.2 / 222.4 | 135.33 / 129.30 |
| Indexed DCD | 20,000 / 2,000 / 2,000 | 1,546 / 1,305 | 24.66 / 24.39 |

All coordinate hashes match, including memory-versus-file comparisons. An
additional five-process comparison of 1,024 unaveraged memory updates measures
35.45/35.85 ms, with overlapping ranges of 34.77-36.03/34.80-36.20 ms. It does
not establish a speedup for the unchanged path. The first averaging candidate
reduced allocation but regressed small-frame timing; moving format dispatch
outside the atom loop produced the final results above. Its diagnostic results
remain separate from the final measurements.

Focused tests cover nested selections with repeated/reordered atoms, owned,
memory and indexed data, windows crossing block boundaries, signed zero, very
large windows, end frames and differing cell metadata. Truncating an already
indexed file causes an error while preserving coordinates, cell, frame index
and movement generation. Existing script checks cover selection, averaging,
append/reorder/remove and Undo. Raw measurements are retained in
`.cache/deep-audit/trajectory/paired.json` and `direct-paired.json`. The expanded
`benchmark-trajectory-playback.py` accepts frame count, averaging radius and
storage modes; averaging requires building the example with `scripting`.
The full desktop-core suite with scripting and the MD-tools suite pass 440
checks across 31 groups, with zero failures or ignored cases.

## Map dimensions, data validation and OpenDX memory

Five regressions first failed with panics: mismatched OpenDX array/grid counts,
overflowing OpenDX dimensions, overflowing XPLOR extents, an oversized XPLOR
allocation requested by a short input, and overflowing BRIX extents. Readers
now use checked extents/products, reject inconsistent data counts and return
allocation errors. XPLOR rejects a declared voxel count greater than the entire
text's byte count before allocating. OpenDX counts actual data tokens before
allocating its output. Zero dimensions and malformed explicit item counts are
also rejected. Repeated failed loads through both `PreparedFile` and
`DesktopAppState` leave the existing scene, active scene and Undo state intact.

OpenDX previously retained its input-order double array while creating another
double array in the application's X-fastest order. It now identifies contiguous
data spans and decodes directly into the final array. This preserves comments,
metadata order, data terminators, ASCII/Unicode whitespace, signed zero and
floating-point bits. Asymmetric grids test every output position independently.

Three fresh processes per version alternate order, with two parses per process.
Time measures parsing an already loaded string and excludes coordinate hashing;
peak RSS includes the input string and the complete process. All values, origin,
spacing and dimension hashes match the previous implementation.

| Input | Values | Repeated parse, before/after (ms) | Peak RSS, before/after (MiB) |
| --- | --- | --- | --- |
| OpenDX 64 cubed | 262,144 | 6.74 / 8.85 | 8.05 / 5.94 |
| OpenDX 128 cubed | 2,097,152 | 49.73 / 46.01 | 70.11 / 35.95 |
| OpenDX 256 cubed | 16,777,216 | 410.52 / 357.97 | 534.17 / 276.02 |
| XPLOR 128 cubed | 2,097,152 | 84.13 / 82.19 | 54.95 / 54.95 |

The small OpenDX timing varied across runs. A separate five-process comparison,
with five parses per process, measures 5.97/5.73 ms and 8.08/5.94 MiB. It does not
establish a consistent small-input speedup. The first implementation reduced
memory but slowed the largest input; grouping consecutive data lines removed
that regression before the final comparison. Diagnostic runs remain distinct
from `maps/paired.json`, `small-paired.json` and `xplor-paired.json`.

The installed C++ 3.0.0.554 exports identical single-precision values for all
250,047 voxels of the 63-cubed input, including origin and spacing. Its 64-cubed
export differs at exactly the final voxel: the input and Rust contain -155.9375,
while C++ returns the preceding value, -158.25. The reference reader fills a
three-value line buffer but indexes its last partial line from the wrong slot
(`OpenDXPotReader::readDensity`). The Rust reader preserves the correct input;
the reference checkout is unchanged. This is a reference limitation, not an
assertion of complete native-reader equivalence.

The molecular, surface, crystallography and desktop-core suites with scripting
pass 792 checks in 57 groups. A subsequently added partial-block BRIX case also
passes with its existing companion case. The first broad run accidentally used
an incomplete sibling fixture directory and failed two unrelated file lookups;
the passing run explicitly sets `CUEMOL2_REF_DIR` to the copied complete fixture
tree. Optional external-oracle wrappers retain the earlier stated limits.

Portable helpers are `map_reader_bench`, `potential_check`,
`scripts/benchmark-map-readers.py` and `scripts/export-reference-potential.mjs`.
The benchmark script can generate deterministic OpenDX and XPLOR inputs. Raw
measurements, before/after failure logs and native QDF exports are retained under
`.cache/deep-audit/maps`.

## Surface, lightweight and opaque payload snapshots

Surface text/raw bytes, lightweight QDF bytes and unknown QSC payloads previously
copied their entire buffers when a scene was cloned. Application Undo retains
before/after snapshots, so even renaming a scene repeatedly retained duplicate
geometry. These immutable payloads now use shared `Arc` storage. Geometry edits
replace the payload; they do not alter older snapshots. Existing export methods
still return owned bytes where their API requires them.

A regression first demonstrated detached surface buffers after a metadata edit.
The final tests check shared storage through metadata changes and Undo/Redo,
replacement isolation, lightweight save/reload bytes, and preservation of opaque
plugin data. The molecular, shared application and command suites pass 971 checks
across 72 groups. Four explicitly optional checks require live metadata services,
FFmpeg or POV-Ray and remain for the final integration run. This count does not
include the deferred native/browser frontend runs. Both native frontends and
their test targets also pass a combined `egui-gui,tauri-gui` compilation check.

The probe loads each input through `DesktopAppState`, makes eight scene-name
edits through `ChangeSet`, keeps all eight changes and performs two complete
Undo/Redo cycles. Every step hashes the full retained payload and verifies the
restored scene name. Three fresh processes per version alternate order; all
hashes match the preceding implementation.

| Input | Geometry or payload | Eight edits, before/after (ms) | Peak RSS, before/after (MiB) |
| --- | --- | --- | --- |
| OBJ surface | 65,536 vertices / 130,050 triangles | 7.99 / 0.121 | 147.39 / 26.52 |
| OBJ surface | 147,456 vertices / 293,378 triangles | 18.71 / 0.127 | 346.67 / 61.73 |
| QDF surface | Same 147,456-vertex mesh | 8.43 / 0.129 | 165.83 / 41.56 |
| Lightweight QDF | Same indexed geometry | 8.37 / 0.120 | 160.19 / 35.86 |
| Unknown QSC object | Approximately 8 MiB of retained opaque data | 8.25 / 0.105 | 165.17 / 52.92 |

Edit time includes snapshot capture and change recording, and excludes rendering
and verification hashes. RSS includes loading and both verification cycles. The
separately recorded Undo/Redo times include hashing and should not be interpreted
as isolated UI latency. Creating shared allocations adds about 0.5 ms to the
initial QDF surface/LW loads in this comparison; it is not a loading speedup.
OBJ geometry is a deterministic synthetic height field, not a molecular SES
accuracy benchmark. Opaque data is retained without requiring its original plugin.

The `scene_snapshot_bench` example and `benchmark-scene-snapshots.py` reproduce
the checks. The script generates OBJ fixtures; the example can convert them to
surface or lightweight QDF for paired runs. Raw evidence is retained in
`.cache/deep-audit/scene-payloads/paired.json`. The old `LoadedSurface` comment
described textual QDF1/PLY only, although readers also accept binary QDF/PLY;
the comment now matches the implementation.

## Tauri measurement overlays

Running the Tauri suite with the complete reference fixture tree exposed a
duplicate measurement-line path. Simple AtomIntr lines were present in the
depth-tested GPU snapshot, but viewport metadata also sent them to the SVG
overlay. SVG then displayed a second line without the scene's depth test.
The metadata no longer includes these duplicate lines. The reproducer retains
both GPU lines and returns zero SVG lines after the fix.

The existing regression silently skipped without an optional reference QSC.
It now creates two positional measurements locally, so it runs in an ordinary
checkout. The self-contained version first fails before the runtime change.
After the change, 214 Tauri model checks pass. Five opt-in cases also pass with
real GPU rendering and FFmpeg: animated PNG frames, movie re-encoding, hover
geometry reuse, empty-scene center marks/HiDPI and volume-only rendering. A
separate GPU pixel test confirms that a line behind opaque geometry remains
occluded. These checks exercise the Tauri model and rendering paths without
claiming a physical Mac window/input check. Raw logs are retained as
`tauri-lines-before.log`, `tauri-model-final.log`, `tauri-gpu-movie-tests.log`
and `measurement-depth-gpu.log` under the audit cache.

## Browser combined-operation checks

The malformed-map worker test reads invalid OpenDX, XPLOR and BRIX files in
sequence. Each error leaves the original scene and Redo entry intact, releases
the worker, and allows a subsequent valid molecular load. All 15 loading checks
pass, including queued input, cancellation, concurrent rendering and delayed
network replies.

The repeated-scene test performs twelve cycles on each of WebGL and WebGPU.
Each cycle loads 512 atoms, a 128-frame DCD, a 32-cubed potential and a surface
with 2,304 vertices. It seeks beginning/middle/end frames, edits selections and
names, changes the camera and repeats Undo/Redo three times. Two cycles also
save and reload an embedded QSC and check exact averaged coordinates. Closing
the temporary scenes restores the empty base scene and releases the parser
worker. The test records WASM allocation capacity and JS heap observations;
these are distinct from process RSS and live allocation counts.
In the final full-Chromium run, WASM capacity spans 41.875-49.5 MiB on WebGL
and 39.5-47.125 MiB on WebGPU. Each backend's final capacity is 3.8125 MiB
above its fourth-cycle sample. Observed JS heap maxima are 40.56/42.13 MiB,
with final samples of 20.11/22.65 MiB. These twelve-cycle observations bound
the tested workload; they do not establish a leak-free lifetime for every scene.

Browser file trajectories use asynchronous indexed `File.slice` reads.
`set_frame` is tested on that path; synchronous script averaging is tested on
the core trajectory restored from embedded QSC. The test does not claim script
averaging support for the browser's separate indexed-file representation.

On this macOS host, Chromium 156 Headless Shell processed commands but sometimes
left the captured canvas blank or on its startup image. The same six focused
WebGPU checks pass with Chromium's full binary in headless mode. Playwright now
selects that binary on macOS; other platforms retain their existing selection.
The failed diagnostic run is kept separate from the final full-browser run.

## CLI, MCP and Python lifecycle

Two fresh MCP processes each perform twenty cycles with 4HHB (4,779 atoms).
Every cycle creates a scene, loads the molecule, edits its name, undoes the
edit, rejects a malformed OpenDX input without changing the scene, redoes the
name edit and closes the scene. Each close returns to the sole empty base
scene. A Python counter persists through all twenty cycles within each
process. All 646 JSON-RPC requests pass, and both processes exit successfully.
The helper/Python selected-atom export check also passes on the rebuilt CLI.

Separate CLI invocations save `view_input:gpu-pick` as false and then true.
For each value, both the next CLI read and a newly started MCP process's
`ViewInputConfig.gpu_pick` report the saved value. The probe uses an isolated
configuration directory. Evidence is retained in `mcp-lifecycle.json`,
`config-restart.json` and `python-mcp.log` under the audit cache.


## Large browser workloads and measurement limits

Both WebGL and WebGPU display 5XNL (98,986 atoms), a smooth 128-cubed OpenDX
potential and an OBJ surface with 147,456 vertices and 293,378 triangles.
Each object receives eight name edits, eight Undo operations and eight Redo
operations before its temporary scene is closed. Every restored name matches.
The potential uses a full-region cyan isosurface at absolute level 0.25 and an
explicit centered view; the initial default-level image was empty and is kept
as a diagnostic. Final captures are inspected and count colored geometry
outside the tool palette, so UI controls cannot satisfy the pixel check.

The 20,000-atom trajectory contains 2,000 frames and occupies 480,048,116 bytes.
Both backends retain the file and index it in 62/65 ms, then seek eleven times
across the start, middle, end and reverse order. The first and last atom
coordinates match exactly at every checked frame. Loading the source topology
and rendering are separate from indexing. The observed seek times of 210-218 ms
include browser command polling and redraw, so they are not decoder benchmarks.
The separate core benchmark above visits every frame.

A second probe repeats loading 5XNL, three edits, Undo/Redo and closing the
scene six times on each backend. WASM capacity reaches 1,059.5 MiB on WebGL
and 865.06 MiB on WebGPU. Capacity stops increasing after the third and first
cycles respectively; growth from cycle three through six is zero. The large
geometry remains expensive, and WASM capacity does not shrink when allocations
are freed. These observations establish reuse for these cycles, not process
RSS, live retained bytes, or a universally small memory footprint. The twelve
smaller combined cycles above exercise a different workload.

The probe code, timings and captures are retained in
`.cache/deep-audit/browser-workloads`. Local WASM was rebuilt from `b283c68`;
`f6c4154` changes native Tauri metadata and browser tests without changing WASM
runtime sources. Release and public-web checks use the clean `f6c4154` artifact.

## Broad browser and rendering checks

The full local Chromium run initially passes 393 of 398 cases, skips the
optional CMYK reference and fails four timing/input cases. The failures are
reproduced individually. The inspector test now fixes the pointing device to
mouse, since macOS's automatic trackpad detection correctly changes subsequent
wheel events to panning. Style double-clicks allow a rendered frame between
synthetic button transitions and verify the editor opened before the next
command. The sequence test polls an atomic DOM measurement while selection
refreshes replace its virtualized rows. No application behavior is weakened.
All four then pass, including three consecutive repetitions (12 checks).
The CMYK check now accepts `CUEMOL2_REF_DIR`; with the supplied reference profile
it passes as well. Thus all 398 distinct local cases are exercised successfully
across the full run and focused reruns, rather than claiming the initial full
run was green.

The broad tests cover multi-scene history, camera drafts, animation timing and
references, embedded scene saves, live selection, malformed/cancelled loads,
still-image history, movie encoding, Stop during concurrent edits, stale async
results, output locks and cleanup of completed versus active sessions. Native
Tauri adds real GPU volume/hover/center-mark checks, PNG animation frames and
FFmpeg re-encoding. The separate command cases exercise seven FFmpeg formats,
POV-Ray paths containing spaces and actual output files. The measured-depth
GPU regression also verifies that occluded measurement pixels remain hidden.

The opt-in Tauri profile rotates 5XNL through fifteen 1,280-by-720 frames and
round-trips every encoded PNG back to its exact RGBA bytes. All frames pass.
This unoptimized test build measures median render/PNG times of 432/46.6 ms;
it is a correctness and workload observation, not a release-speed claim.


## Product delivery evidence

The final runtime changes are in `f6c4154`. Its
[CI run](https://github.com/mogura-rs/cuemol-rs/actions/runs/42468589990)
passes all jobs. The four browser shards report 100, 100, 99 and 98 passing
cases; their one optional CMYK skip is covered by the successful local and
public checks with the reference profile. Later changes only adjust tests
and this audit document.

The [distribution validation run](https://github.com/mogura-rs/cuemol-rs/actions/runs/42468589993)
passes native x86-64/ARM64 Linux, Intel/Apple-silicon macOS and x86-64 Windows.
Each job checks CLI/Python entry points, APBS/PDB2PQR, FFmpeg, POV-Ray, OIDN,
installer/self-upgrade behavior and the packaged executable. Its actual window
probe verifies pixels, OS input, camera navigation, clipboard/history,
multiline Python/console use, inspector drafts and derived properties.
All five result files report success, and their captured windows are inspected.
The artifacts retain native build identity, embedded-web manifest identity,
executable checksums and bundled-tool provenance. The workflow used
`publish=false`; it does not create a new tagged release.

The [Pages deployment](https://github.com/mogura-rs/cuemol-rs/actions/runs/42470180722)
serves the clean `f6c4154` build. All 45 public assets match their manifest
hashes, including the WASM content type and source identity. Eight public
browser checks pass: malformed-map recovery, both repeated-scene backends,
both read-only-script backends, lighting, OIDN and reference CMYK proof.
Raw CI jobs/logs, package GUI results, downloaded manifests and public-browser
results are retained under `.cache/deep-audit`. Test/documentation follow-ups
are checked by their own final CI and public artifact identity rather than
relabeling these native packages as a later source build.

The retained limits are the explicitly deferred physical Mac screen/input
permissions and hardware quad-buffer presentation. Native Tauri is covered by
its model tests and real GPU/output checks; browser shell interactions also run
against the shell test bridge. Those checks do not replace the deferred macOS
WebKit/OS-input run. Neither these finite workload checks nor the source-oracle
comparisons establish correctness for every possible input.
