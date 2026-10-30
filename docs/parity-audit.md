# Reference comparison

This audit compares the published CueMol 2.4.1.547 application and its
`v2.4.1.547` source tag (`ef06137084421a1d0296d55a46b8b1463f6723f2`) with
cuemol-rs application revision `f4ed01c` (v0.1.8). The later `d21afd1` revision
only adds validation documentation. The upstream checkout is read-only.

The checklist below distinguishes a confirmed difference from an unmeasured
area. Earlier evidence in [validation.md](validation.md) remains useful for its
listed cases, but does not establish equivalence for an entire feature family.
Generated inputs, profiles, screenshots and raw measurements remain in the
ignored checkout cache.

## Comparison requirements

Use the same inputs, object order, camera, renderer properties, image size and
quality settings. Record the reference build, tested revision and runtime with
each result. Compare semantic scene state and exported data as well as displayed
pixels. A successful command alone does not establish the intended behavior.

Performance measurements use at least three sequential fresh-process samples
per condition. Record the measured stage, elapsed time, peak resident memory
and output checks. Report cold and repeated operations separately. Browser
linear memory is recorded separately from process RSS. Do not infer an overall
speedup from a single optimized stage or compare timings from different hosts.

## Feature checklist

| Feature family | Required comparison | Current audit status |
| --- | --- | --- |
| Shell | Activities, menus, toolbar, tab order, docks, compact windows, themes and keyboard access | Previous focused checks exist; full control inventory pending |
| Explorer and Inspector | Object/renderer/group hierarchy, selection, visibility, locking, drag/drop, context menus and property editors | Previous focused checks exist; dialog comparison pending |
| Color and styles | Coloring modes, paint entries, palettes, property inheritance, style creation/apply/edit/save | Previous focused checks exist; complete editor comparison pending |
| Console | CueMol/PyMOL dialects, command arguments, completion, history, multiline input, IME and transaction behavior | Confirmed difference: native and browser consoles only execute JavaScript |
| Scripts and services | JavaScript/Python helpers, retained state, callbacks, errors, file arguments and asynchronous readers | Previous compatibility checks exist; shared workload comparison pending |
| File and scene I/O | Reader options, saved cameras, source metadata, selections, linked/embedded data and cancellation | Previous compatibility checks exist; paired performance comparison pending |
| Navigation and cameras | Rotation, pan, zoom, fitting, clipping, bookmarks, visibility and screen coordinates | Previous numeric and interaction oracles exist; integrated comparison pending |
| Selection and annotation | Expressions, residue ranges, lasso, named selections, labels and measurement modes | Sequence toggle/range/context actions now share a virtualized native/browser panel; insertion-code selection and pivot centering repaired; broader annotation comparison pending |
| Molecular editing | Merge/delete, chain/residue changes, secondary structure, transformations and undo/redo | Command coverage exists; dialog and edge-case comparison pending |
| Analysis and surfaces | Alignment, interactions, surface creation/regeneration/cutting and electrostatics | Numerical/tool checks exist; dialog and performance comparison pending |
| Viewport rendering | Molecular/map/surface representations, transparency, lighting, edges, fog, AA and AO | Previous primitive/oracle checks exist; paired scene comparison pending |
| Render and export | Umbreon settings, preview, cancel/save, physical units, alpha and still/movie output | Previous focused checks exist; reference timing and memory comparison pending |
| Animation and MD | Morphing, camera/property motions, seek/pause/stop, indexed trajectories and bounded memory | Previous state/interaction checks exist; complete playback comparison pending |
| Native integration | Windows, dialogs, settings, recent files, installer/update and OS credential stores | Five-platform published checks exist; local OS-input prerequisites remain documented |
| External prerequisites | Real AI providers and hardware quad-buffer presentation | Credentials/target hardware remain required; quad-buffer backend is unimplemented |

POV-Ray changes remain outside this request's established scope; rendering work
uses Umbreon. Any newly confirmed mismatch must have a reproducer, a repair or
explicit unresolved status, and evidence after the repair. Publication requires
the affected regression checks, public review, terminal CI, distribution and
Pages verification. Unresolved required rows prevent a claim of full parity.

## Sequence comparison and repairs

The reference panel obtains each molecule's own selection. Both frontends now
follow that rule, so an unselected molecule no longer inherits another
molecule's residue selection. The shared panel supports click/toggle, Shift
ranges, drag ranges, context centering, nearby selections, inversion and copying.
It uses the reference row/ruler heights, measures the monospaced cell width and
draws only visible rows and columns. Its sequence data is retained until a scene,
edit generation or fallback selection changes.

Published-reference probes also confirmed that `resid 1` excludes `1A`, and
that `resid 1:2` includes `1A` and `1B` but excludes `2A`. Selection comparisons
now use the residue number and insertion code together. Reversed ranges retain
the reference's inclusive behavior. The regression cases include negative
residues, insertion codes and molecule atom-order permutations.

Sequence centering and the JavaScript residue `getPivotPos()` now use the
topology pivot atom, or the residue centroid when that atom is absent, and apply
the object's transform. Published-reference probes cover protein CA, nucleotide
P, a missing pivot, an unknown ligand and a transformed object. Custom topology
pivots require a separate persistence check; the current query uses the bundled
topology dictionary.

Three real-browser tests cover selection state, cross-molecule isolation,
centering, copying, context menus and undo/redo. The dark and light cases also
compare captured selection pixels with the reference CSS colors. Shared egui
event tests cover Shift capture and drag behavior. These checks complement the
core semantic tests; they do not establish parity for every sequence-panel
interaction. The full shared/native run passed 1,438 tests with one ignored,
and the browser suite passed all 112 cases before the final palette correction;
the three affected browser cases passed again afterward.

The Tauri adapter now uses the same per-molecule selection and transformed
pivot query. Its dedicated regression exercises two molecules with different
selections. The complete native suite passed 243 tests with one ignored; the
Tauri model suite passed 148 with six ignored. The Tauri workflow now runs that
model suite as well as the displayed-window check. Obsolete source-location
assertions were removed from the Tauri shell test, and its nucleotide-default
expectation was updated to the already verified 0.5-width elliptical section.

### Measurements

The following measurements were taken on 2026-10-30 after the selection and
sequence changes. Each condition ran in three fresh processes with 60 repeated
sequence reads. No timings from different hosts are compared.

| Same-host workload | Before or reference | Current | Semantic check |
| --- | --- | --- | --- |
| Linux x86_64, 100,000 generated atoms, 25,000 residues, uncached sequence read | Previous Rust: 68.58–69.54 ms/read; 52.06 MiB peak RSS | 17.11–17.19 ms/read; 45.81–46.15 MiB peak RSS | All samples match digest `185a642d1850fbd8`, including ten selected residues |
| macOS ARM64, 20,000 PDB atoms, 5,000 residues, uncached sequence read | CueMol 2.4.1.547: 2.20–2.28 ms/read; 92.84–92.88 MiB peak RSS | 0.99–1.02 ms/read; 24.36–26.33 MiB peak RSS | All samples match digest `ea5cad748c969bfe`, including ten selected residues |

These are sequence-data measurements, not GUI frame rates or whole-application
speedups. RSS includes each process's runtime and molecule loading; the two
applications use different runtimes. A repeated unchanged Rust cache lookup
avoids construction, but its nanosecond timing does not measure painting. The
reference UI also retains sequence data. Initial refreshes, repeated refreshes
and process lifetime are recorded separately by the benchmark driver.

The paired PDB can be regenerated by the driver. Its SHA-256 is
`73d0fed018d213da6bd4ed6970babf02b29e62b1bba3aff50e8616d0065b4198`.
Use a release build and keep generated reports in the ignored cache:

```sh
cargo build --locked --release -p cuemol-desktop-core --example sequence_bench
uv run --no-project python scripts/benchmark-sequence.py \
  --binary target/release/examples/sequence_bench \
  --generate-atoms 20000 --revision "$(git rev-parse HEAD)" \
  --reference-binary "$CUEMOL_REFERENCE_BINARY" \
  --reference-app "$CUEMOL_REFERENCE_APP" \
  --reference-version 2.4.1.547 \
  --output .cache/benchmarks/sequence.json
```

Set the two reference variables to the published application executable and
application root. The adapter uses that application's native module without
modifying it. Omit both reference arguments to measure Rust alone, or use
`--fixture structure.pdb` instead of generating a fixture. The RSS collector
requires POSIX `wait4`.

## Command and installation requests

Default protein ribbons, nucleotide cartoons and other-atom ball-and-stick
representations were already implemented. Focused tests use real protein,
nucleotide and small-molecule fixtures. The native OS-input check now also
verifies horizontal Shift-left-drag slab adjustment and `C` centering alongside
rotation and zoom.

The unified command already provides file launch, MCP, release updates,
configuration and `startup.js`. The added `cuemol skill` prints an embedded
Markdown skill without loading startup scripts. The same source document is on
GitHub, and a Colab notebook installs a release and displays it.

The root Python package now builds the native GUI command through Maturin.
Actual pip source installations on Linux x86_64 and macOS ARM64 passed execution
checks outside the checkout: version, offline skill, startup scripting, config
directory and the 140-tool MCP catalog. A separately built source-distribution
archive passed the same installed-command checks. The Linux installed binary also passed
the native OS-input and captured-pixel check. The release matrix repeats the
Python installation check on all five supported targets. A public Git-URL
installation pinned to `d34bd69` also passed the installed-command checks.
The new release's distribution checks remain delivery gates.
