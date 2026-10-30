# Reference comparison

This audit compares the published CueMol 2.4.1.547 application and its
`v2.4.1.547` source tag (`ef06137084421a1d0296d55a46b8b1463f6723f2`) with
the cuemol-rs baseline revision `f4ed01c` (v0.1.8). The later `d21afd1` revision
only adds validation documentation. Repairs and their subsequent checks are
recorded below. The upstream checkout is read-only.

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
| Shell | Activities, menus, toolbar, tab order, docks, compact windows, themes and keyboard access | Application shortcuts, browser tab closing and menu access, shared history dropdowns, Tauri palette folding and tree/text clipboard routing repaired and checked; browser recent files and remaining pane controls are still under comparison |
| Explorer and Inspector | Object/renderer/group hierarchy, selection, visibility, locking, drag/drop, context menus and property editors | Previous focused checks exist; dialog comparison pending |
| Color and styles | Coloring modes, paint entries, palettes, property inheritance, style creation/apply/edit/save | Previous focused checks exist; complete editor comparison pending |
| Console | CueMol/PyMOL dialects, command arguments, completion, history, multiline input, IME and transaction behavior | Shared CueMol/PyMOL interpreters, completion, native QAX logging/replay and cancellable execution implemented; focused shared, browser and OS-input checks passed; affected regression checks, CI and Pages passed at `99b7c16`; public review repairs and release delivery are in progress |
| Actions | History rows, operation inspection, QAX export, bundled/user macros and reusable actions | History selection, operation inspection, QAX export and bundled/user macro libraries are implemented in native, WASM and Tauri; CI, Pages and the macOS window check passed at `488ce9c`; review repairs passed affected checks and were published at `53100c5` |
| Scripts and services | JavaScript/Python helpers, retained state, callbacks, errors, file arguments and asynchronous readers | Previous compatibility checks exist; shared workload comparison pending |
| File and scene I/O | Reader options, saved cameras, source metadata, selections, linked/embedded data and cancellation | Embedded QDF/PDB secondary-structure defaults and topology transfers repaired; paired scene loading measured below; broader I/O comparison pending |
| Navigation and cameras | Rotation, pan, zoom, fitting, clipping, bookmarks, visibility and screen coordinates | Previous numeric and interaction oracles exist; integrated comparison pending |
| Selection and annotation | Expressions, residue ranges, lasso, named selections, labels and measurement modes | Sequence toggle/range/context actions now share a virtualized native/browser panel; insertion-code selection and pivot centering repaired; broader annotation comparison pending |
| Molecular editing | Merge/delete, chain/residue changes, secondary structure, transformations and undo/redo | Command coverage exists; dialog and edge-case comparison pending |
| Analysis and surfaces | Alignment, interactions, surface creation/regeneration/cutting and electrostatics | Numerical/tool checks exist; dialog and performance comparison pending |
| Viewport rendering | Molecular/map/surface representations, transparency, lighting, edges, fog, AA and AO | Ribbon widths, topology links, applied pivots and secondary-structure junctions repaired; remaining viewport effects and scene comparisons are pending |
| Render and export | Umbreon settings, preview, cancel/save, physical units, alpha and still/movie output | Six opaque native export conditions compared below; physical lighting and geometry repaired, traversal optimized; transparent scenes, denoising and broader export controls remain under comparison |
| Animation and MD | Morphing, camera/property motions, seek/pause/stop, indexed trajectories and bounded memory | Previous state/interaction checks exist; complete playback comparison pending |
| Native integration | Windows, dialogs, settings, recent files, installer/update and OS credential stores | Five-platform published checks exist; local OS-input prerequisites remain documented |
| External prerequisites | Real AI providers and hardware quad-buffer presentation | Credentials/target hardware remain required; quad-buffer backend is unimplemented |

POV-Ray changes remain outside this request's established scope; rendering work
uses Umbreon. Any newly confirmed mismatch must have a reproducer, a repair or
explicit unresolved status, and evidence after the repair. Publication requires
the affected regression checks, public review, terminal CI, distribution and
Pages verification. Unresolved required rows prevent a claim of full parity.

## Saved-scene and backbone repairs

The scene reader had used the PDB reader's secondary-structure reconstruction
default for embedded `qdfpdb` data. The published reference preserves deposited
secondary structure for this format. Dispatch now retains that default while
honoring an explicit reader override; ordinary PDB imports retain their existing
reconstruction default. Tests pass a scene through parsing, the worker payload
and application to verify all four format/override combinations.

Loaded backbone renderers now use the directed polymer links and pivot names
assigned by topology. Distance-based connectivity remains a fallback for raw
coordinates that have not had topology applied. Previously, nearby pivots could
join a chain break or miss a custom pivot. Links survive coordinate updates,
copying, residue edits, undo and worker transfers. Unknown residue topologies are
generated from the first occurrence of a residue name in each molecule and
reused, matching published-reference probes. A new molecule copy rebuilds its
topology. This corrected the 4HHB bond count from 4,627 to the reference's 4,638.

An unstyled ribbon now uses the factory section width. A plain `secondary2=H`
also remains a helix body at a chain break; it no longer acquires an inferred
head junction that extends beyond the last residue. Tests that require connected
protein or nucleotide backbones now include the actual linking atoms. The
picking fixture reapplies topology after changing residue identities. Their
selection, transform, dimension and color assertions remain in place.

## Physical lighting and export measurements

The reference's default GI path length is one bounce. The previous Rust default
of four changed both the image and the workload. The repaired renderer also
uses GGX highlights and dielectric glossy reflection, the reference's Sobol
sample sequence, and geometric-face orientation for grazing primary normals.
Previously, dielectric reflections were missing and grazing ribbon ends could
become too dark. The reduced GI grid uses normal/depth-guided interpolation and
recomputes uncovered or low-confidence samples. Its storage covers the current
render tile plus a one-row halo; tests check thin geometry, cancellation and
identical output across tile boundaries.

The CPU acceleration structure now uses binned surface-area splits, adjacent
child nodes and direction-aware bounds tests. It retains the previous tie rules
for coincident surfaces. Opaque primary rays avoid collecting and sorting every
intersection, cylinder axes are prepared once, and workers take short row blocks
to distribute uneven work. These optimizations preserve the tested image hashes.
An isolated 4HHB ribbon check alternated three fresh processes of each Rust
build, retaining the same repaired geometry and lighting. Median first/repeated
export times fell from 1,339/1,353 ms to 1,114/1,120 ms, about 17%, with identical
RGBA hashes in every export. This compares the traversal optimization with its
immediate predecessor; it is not a comparison with the original appearance.

The following measurements used the same ARM64 macOS host, CueMol 2.4.1.547,
embedded PDB data, saved camera, factory renderer, 640 x 480 output, 3 x 3
supersampling and no denoising. Each condition ran in three fresh processes with
two exports per process. Times are medians of the first and repeated exports,
including scene preparation, CPU rendering and PNG writing. RSS is the range of
whole-process peaks, including loading and each application's runtime. The two
engines use their default worker allocation; the generic `numThreads` setting
does not control the reference Umbreon exporter.

| Scene | GI | Reference first / repeat (ms) | Rust first / repeat (ms) | Reference / Rust peak RSS (MiB) | Whole-image RGBA mean / maximum error |
| --- | --- | ---: | ---: | --- | ---: |
| 1CRN ribbon | Off | 41.4 / 41.0 | 43.1 / 40.9 | 137.9 / 16.0-16.2 | 0.000056 / 20 |
| 1CRN ribbon | On | 214.2 / 212.2 | 271.1 / 266.7 | 486.1 / 22.3-22.4 | 0.00436 / 24 |
| 4HHB ribbon | Off | 77.5 / 75.6 | 106.0 / 103.1 | 178.2-179.8 / 55.7-55.8 | 0.000039 / 1 |
| 4HHB ribbon | On | 627.0 / 623.2 | 1104.5 / 1108.2 | 531.1 / 55.8-55.9 | 0.03594 / 7 |
| 1CRN CPK | On | 563.3 / 568.0 | 774.8 / 765.5 | 481.5 / 20.1-20.4 | 0.00628 / 17 |
| 4HHB ball-and-stick | On | 3321.3 / 3319.3 | 5746.3 / 5835.4 | 489.5 / 28.9-29.0 | 0.21596 / 31 |

Errors use channel values from 0 to 255 and include the background. Every
condition produced one repeated RGBA hash across all six exports. Both readers
reported 327 atoms / 337 bonds for 1CRN and 4,779 atoms / 4,638 bonds for 4HHB.
Residual image differences remain, especially in GI ball-and-stick rendering and
isolated edge pixels. A separate ball-and-stick check with GI disabled reduced
the mean error to 0.00024, so the dominant remaining difference is in lighting.
These results do not establish parity for transparent, denoised or animated
scenes.

The Rust process uses less memory in these cases, but GI exports remain slower
than the reference. Loading is also workload-dependent: median 1CRN load times
were approximately 2.3-2.4 ms in Rust and 7.2-7.6 ms in the reference, while 4HHB
took 63-71 ms and 18 ms respectively. Export timings exclude those load times.
No whole-application speedup is inferred from these measurements.

To reproduce an export comparison with an installed reference application:

```sh
uv run --no-project python scripts/make-render-benchmark-scene.py \
  input.pdb .cache/render-audit/scene.qsc --renderer ribbon
cargo build --locked --release -p cuemol-command --example render_bench
uv run scripts/benchmark-render.py \
  --binary target/release/examples/render_bench \
  --fixture .cache/render-audit/scene.qsc \
  --reference-binary "$REFERENCE_BINARY" --reference-app "$REFERENCE_APP" \
  --reference-version 2.4.1.547 --revision "$(git rev-parse HEAD)" \
  --output .cache/render-audit/comparison.json
```

Use `--no-gi` when creating the direct-lighting fixture. The benchmark records
the platform, binary and fixture hashes, per-process RSS, stage timings, atom
and bond counts, and decoded image hashes. It rejects mismatched molecule
identities/counts and image dimensions. Generated fixtures and results belong
in the ignored cache.

The complete shared/native run passed 1,729 tests with one ignored. After
cylinder-axis reuse was added, all 93 render unit/integration tests passed and
both WASM modules rebuilt. The browser run passed 139 cases and exposed one
invalid spline fixture: its CA-only PDB has no polymer links, and both the
published reference and Rust export an empty image for that input. The fixture
now includes the linking N/C atoms; its full-chain and partial-selection pixel
assertions passed unchanged. Public review and delivery checks are in progress.

## Shell control inventory

The initial inventory uses the published reference's `shared/menuTemplate.ts`,
`renderer/shell/ActivityBar.tsx`, `Toolbar.tsx` and keybinding dispatcher.
The Rust entry points are the native `menu_bar` and `keyboard_shortcuts`, the
browser `shell_menu` and toolbar, and Tauri's `index.html` and keydown handler.
This is a source comparison; screen checks are recorded separately.

| Reference control | Observed application coverage | Remaining difference |
| --- | --- | --- |
| Explorer, View, Selection, Crystal, Actions | All five built-in activities are present | Individual pane controls still require comparison |
| New Tab, Open File, Save As, Open Scene, Reload Scene, Save Scene, Render, Undo, Redo | Toolbar buttons and history dropdowns are present in all three frontends | Native/WASM dropdowns now support jumping several steps; broader history limits still need comparison |
| Save current view, Close Tab, scene save variants | Menu access is present in all three frontends; WASM tabs now have close buttons and a save/discard/cancel dialog | Browser downloads cannot confirm that a user saved the file to disk; unmodified-scene tracking still differs |
| Cut, Copy, Paste, Select All | Edit menus route tree, camera and text operations; native clipboard payloads are shared across processes, and browser clipboard access is asynchronous | Paint and Actions routing still needs complete native/WASM comparison; browser tree multi-selection remains under comparison |
| Molecular editing and analysis dialogs | All frontends have named menu entries; WASM opens the corresponding shared command form | Specialized form layout still needs comparison |
| Scene background/proofing/properties and view projection/center marker/properties | Shared commands, property controls and browser menu entries exist | Integrated screen comparison remains |
| Command/Ctrl shortcuts | Main file operations, Close Tab, alternate Redo, Save Scene As and palette folding are wired | Browsers reserve New/Close Tab keys, so Ctrl/Cmd+Alt+T/W is also supported |
| Plain viewport letter shortcuts | Viewport commands exclude command modifiers and text editing | Remaining navigation modes still need comparison, especially the Tauri palette |

The browser checks exercise actual WASM: undo/redo shortcuts, modifier isolation,
centering, palette folding, IME, tab close cancellation, saved QSC downloads,
undo after closing, the molecular edit form and two-step history navigation.
The four shell cases and four text-input cases passed. All sixteen Tauri shell
cases passed with mocked IPC. The five shared UI tests and 177 native GUI tests
passed, with one native test ignored. A Linux window driven through OS input
also passed the existing rotation, zoom, slab, centering, font, Python helper and
console checks, plus Shift-Ctrl-Z redo and persisted palette folding. A captured
history menu exposed a missing caret glyph; using the existing icon font fixed
it, and the affected browser case passed again.

The Crystal pane also referenced a nonexistent command for symmetry mates.
Its controls now open the actual symmetry settings and display commands.
Tauri's C/F shortcuts now center or fit the current molecule selection without
changing that selection; Home fits the scene. The affected model and shell
cases check selection preservation and modifier isolation.

The native menus also retain older UXP-specific entries that the published
Tritium menu removed; their presence does not demonstrate that the current
Tritium controls match.

## Clipboard comparison and repairs

The reference uses typed XML/QDF payloads for objects, renderers, cameras and
styles. Native frontends now read the OS clipboard instead of retaining separate
application-only copies. They also exchange the reference's plain-text envelope
and platform-specific clipboard formats. Browser clipboard operations use the
same envelope. Pasting creates a new object identity and retains embedded data,
renderer properties, group targets and collision naming. The reference's current
multi-selection behavior expands renderer groups and removes repeated members.
Shared tests cover these cases, including an empty molecule and deletion of the
source before pasting. An installed reference application read the Rust object,
renderer and camera payloads after a reference-to-Rust-to-reference round trip.

Edit-menu actions preserve text focus while the menu is open. An overlapping
popup previously selected the tree behind it; popup hit testing now takes
precedence. Native egui's disabled clipboard backend also swallowed paste keys
and kept copied text in an internal buffer. Native key events and text output
now use the same OS clipboard adapter as the tree. An actual Linux window
passed molecule cut/paste, two-step undo and text copy read by a separate process,
followed by text cut/paste and command execution. The OS check script exercises
these operations on its supported platforms; Linux requires `xclip`.

Browser writes must succeed before cutting, and a changed scene invalidates a
pending cut or paste. Text, tree and modal focus have browser and Tauri shell
checks. Tauri context menus re-read the OS clipboard kind instead of relying on
stale per-window flags. Tauri paint routing currently handles one selected row;
complete paint multi-selection and native/WASM Paint/Actions routing remain
part of the editor comparison. Cross-platform OS checks, release delivery and
the broader shell comparison must still be completed for the final revision.

Public review exposed stale text focus after keyboard navigation, repeated OS
clipboard queries while a menu remained open, and a paint wire-format mismatch.
Text focus now clears on leaving an editor and stays attached to a pending
asynchronous paste. Native menus cache the clipboard kind until the menu closes
or the window regains focus; Tauri shares one kind query across each context
menu. Paint envelopes now use the reference's `sel`/`col` keys and accept BOM,
CRLF and wrapped base64. The worker's `selStr`/`colorValue` names are internal
DTO fields and are not the clipboard wire keys.

Cutting a selected group together with its member now deletes each once and
records one undo step. Browser renderer copying resolves the renderer's actual
owner. Failed pastes restore the scene, and incompatible renderers do not mark
the destination modified. Affected checks passed: four shared clipboard cases,
eleven core cases, 178 native GUI cases with one ignored, 154 Tauri model cases
with six ignored, and the shared focus case. Thirty existing browser and Tauri
shell cases passed; the expanded text/Undo case also passed after adding its
missing menu-coordinate probe.

The review's concern about trimming legacy NUL padding was checked against the
published reference codec. It also trims trailing UTF-16 NUL units and an odd
padding byte; object data chunks are base64 text after an XML sentinel, so the
native wire payload does not end in raw QDF NUL bytes. The plain-text envelope
retains arbitrary binary bytes. Clipboard source loading uses the same scene
reader as other embedded scene inputs.

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
P, a missing pivot, an unknown ligand and a transformed object. A further
published-application probe overrides the GLY pivot with N and confirms that it
survives a residue-name change. The molecule now retains the pivot assigned by
its applied topology, instead of consulting the bundled dictionary at query
time. Copying, chain/residue edits, undo and worker transfers retain it. If the
pivot atom is deleted, the remaining atoms define the centroid. Pivot names and
undo snapshots share storage; the worker format version was advanced because
its molecule payload changed.

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

A subsequent public review exposed a Tauri cache key that ignored individual
molecule selections. Changing one molecule to the same expression already
selected elsewhere could therefore leave the sequence highlight stale. The
key now includes each molecule's selection, with a reproducer that fails before
the repair and passes afterward.

The reference also clips negative residue numbers, overlaps insertion-code
cells and caps the sequence canvas at 30,000 pixels. These inherited limits are
intentionally removed. All three frontends now use one layout that gives signed
residue numbers and insertion codes distinct columns, while retaining ordinary
positive numbering. The Tauri shell creates only visible chain rows and residue
buttons, and its ruler bitmap is limited to the viewport width. Range selection
uses the full sequence model, so offscreen residues remain included.

The new tests click negative, inserted and distant residues, preserve markers
while scrolling, and select across 5,002 residues. The Tauri DOM test supplies
40 chains of 5,005 residues and checks that fewer than 1,000 residue buttons and
20 chain rows exist, then verifies the last row is fully displayed after vertical
scrolling. This is a shell test with mocked IPC; separate Rust tests verify the
actual sequence DTO and shared layout. All four real-WASM sequence tests and
four Tauri model tests passed. The Tauri shell suite passed 11 tests; the affected
virtualization case passed again after correcting the splitter's scroll extent.

Public review found an initial Shift-click difference and inconsistent ruler
marks between frontends. The reference falls back to toggle-and-center when no
marker exists; native, WASM and Tauri now do the same. The ruler draws a tick for
each insertion column and omits the zero label in every frontend. The affected
pointer and browser checks passed after these corrections. An obsolete static
source assertion still required the removed DOM-based range implementation;
that assertion was removed because the browser test checks the actual range.

The review also raised very large numbering spans as a potential coordinate
limit. For spans above 100,000, missing runs longer than eight residue numbers
now occupy one column marked with an ellipsis. Existing residues and insertion
codes retain separate columns, including both signed 32-bit limits. The layout
and pointer tests cover these limits; Tauri's DTO and virtualized shell also
exercise them. Ordinary numbering, including the 5,000-residue case, is
unchanged. Row ordering is guaranteed by the shared sequence builder, including
its sorted fallback; atom-order permutation regressions cover that requirement.
The rebuilt WASM passed all thirteen affected Sequence, Actions and Console
cases, including counting the atoms selected at both integer limits and across
the compressed gaps. Tauri passed five sequence model cases, the Actions model
case and all fifteen shell cases. The six worker-transfer cases passed as well.
Captured images then exposed overlapping ruler labels beyond the signed-number
limit. Labels now stay within that limit and avoid other labels and gap marks;
all three affected browser cases passed after the correction.

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

After retaining custom topology pivots, the same macOS PDB workload was rerun
in three fresh processes per condition. Uncached reads were 0.99–1.02 ms with
24.45–26.47 MiB peak RSS; the published reference measured 2.27–2.34 ms with
92.83–93.05 MiB. All samples retained the same sequence digest. This checks the
additional topology metadata on parsed input; it does not measure GUI painting.

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


## Console comparison and repairs

The shared command layer now reads the published native command catalog and
implements the 63 commands advertised by the reference PyMOL console. Both
dialects are available in native egui, WASM and Tauri alongside the existing
JavaScript console. The catalog and color-name tables were checked against the
published `v2.4.1.547` console sources under
`tritium/react-gui/src/plugins/console/worker/`. This matches the reference
console's supported subset; it does not imply compatibility with arbitrary
PyMOL Python programs.

Native arguments support quoted names, explicit node types, UID hints,
object/renderer paths, lists and wildcard expansion. Property paths now match
the reference's shortest object-name prefix, including names containing dots.
An object named `view` takes precedence over the view shorthand. Completion
reads the current scene, argument role, property schema and native directory.
Published-application probes confirmed property, renderer, parameter-name,
boolean and filename replacements, including their separators.

PyMOL commands cover selection expressions, representations, colors, settings,
cameras, alignment, maps, measurements, labels, spectrum coloring and selected
exports. Nondefault arguments that the reference ignores produce a message.
The live reference probe confirmed distance, angle, selected atom count and
saved scene output. Its torsion operation rejected the synthetic noncollinear
case, so that case is recorded as a reference limitation rather than a passing
cross-application measurement comparison.

A native `open_log` writes completed operations as QAX actions. A standalone
QAX replay keeps one undo entry per action; a replay nested in a command
submission shares that submission's undo. The published application replayed
an actual Rust GUI log containing rename, selection and a two-command rename,
then restored all three actions through successive Undo operations. This also
verified stable name paths when recorded UIDs differ. Macro definitions are
validated and are not executed by file replay; the Actions sidebar described
below supplies the separate macro library and history browser. Scene saves, QSC loads and history
resets must be submitted alone and cannot run inside a QAX action. The published
application also clears Undo/Redo after a scene save. Console saves now follow
that rule only after a successful write; failed saves preserve the history.

Downloads and image generation yield to the GUI. Stopping a submission keeps
completed edits in its undo entry and rejects late results. Browser tests cover
ordered downloads, cancellation, selected PDB and PNG output, live label and
measurement state, multiline commands, completion, history, IME and Undo/Redo.
The Tauri model suite passed 151 tests with six ignored. Its shell test also
checks dialect persistence, completion, multiline input, IME suppression,
restoring an unfinished history draft and Stop; the shell uses mocked IPC.

The Linux OS-input check uses the actual native executable. It typed commands,
completed a molecule name with Tab, entered a second line with Shift-Enter,
undid both edits together and saved QAX. The same run passed rotation, zoom,
slab, centering, font-pixel and Python-helper checks. During this check, the new
console exposed an input-height error that shrank the viewport across frames.
Reserving the button's minimum height repaired the drift. A subsequent
multiline regression exposed expansion beyond six lines; the input now scrolls
within that limit. The viewport stability regression and native screen test
cover these cases.

The shared regression run passed 1,464 tests. Native validation passed 175
unit tests with one ignored, plus 68 Tauri shell structure checks; obsolete
JavaScript-only source assertions were removed in favor of the executable
console checks. All 116 browser cases outside the console passed. The final
isolated WASM build passed all six console cases, including a twelve-line
submission, scene downloads, history reset, cancellation and IME. The final
transaction changes also passed the affected command, native and Tauri model
tests. Public review, terminal CI and release delivery remain separate gates.

One pre-existing documentation mismatch was also repaired: `add_paint` promised
to insert the newest paint above earlier entries, while the implementation
appended it. It now inserts first and updates the molecule selection as the
reference does. `add_paint_at` retains explicit placement and selection.

## Actions comparison and review repairs

The Actions sidebar now displays history steps and their recorded operations,
selects multiple steps, moves to a chosen state, copies commands and exports
QAX. A saved macro can bind its target to a different selected molecule.
The library includes the twelve presets from the published reference, alongside
user files. Native files live under the configuration directory's `actions`
subdirectory; browser files persist in local storage and support import/export.
The three frontends use the same QAX and macro engine. History records retain
operations, while existing shared undo snapshots retain scene data.

A macro resolves renderer names after earlier operations create them. Failed
targets do not replay completed targets, and partial changes retain an undo
entry. Recording now resolves property targets before the edit, allowing a
saved macro to change a new target instead of the molecule used to record it.
The shared tests cover replay, target rebinding, failures, file collisions and
unsupported library commands. Native and Tauri model tests cover history
movement; real-WASM tests cover preset execution, exported QAX replay and
cross-molecule reuse. The Tauri shell tests exercise the corresponding IPC
controls. These checks do not establish every preset's scientific output.

Public Console review identified missing log finalization on cancellation and
no-op operations recorded after the first mutation. The frontends now finalize
interrupted submissions, compare each completed operation with its preceding
state and omit failed operations from the replay log. Empty atom lists and
XML attribute whitespace retain their meaning when QAX is read again.

A separate public review found completion errors after semicolons, at the end
of a target list and for filenames containing parser delimiters. Completion now
preserves the current segment and trailing spaces, applies each dialect's
argument rules and quotes filenames that would otherwise be parsed as syntax.
File patterns keep metacharacters in the working directory literal, explicit
null arguments stay null, and a continued comment at EOF remains a comment.
The final affected regression run passed 35 command tests, four shared UI tests,
177 native tests with one ignored, 68 shell structure checks, and 152 Tauri
model tests with six ignored. The Linux screen check ran the cartoon preset
and verified its scene state and displayed image before and after Undo/Redo.
The rebuilt WASM passed eight Actions/Console cases, and the Tauri shell passed
thirteen cases with mocked IPC. Public review and delivery remain separate gates.

The published Actions batch subsequently passed complete CI, Pages and the
macOS window test. That window test now enters a two-line CueMol command through
OS input, verifies the changed label color and size in captured pixels, and
checks that one Undo restores the original label.

A further public review found a stale Tauri running flag and a dropped refresh
request. The pane now refreshes on completion and replays a pending request.
An executable shell regression covers cancellation and a refresh delayed across
a library change. Native libraries now use the exact discovered filename for
rename/delete, including uppercase extensions and spaces; ambiguous legacy
names are refused, and a failed removal rolls back a newly created copy.
Native asynchronous macros also receive the active GUI context for wakeups.
Preset targets accept derived molecular types, and exported macros bind their
shared molecular base type. The affected shared, native and shell cases cover
these repairs; they do not replace the final release checks.

Several review concerns match the published reference's explicit behavior:
plain numeric references are UIDs, `#number` is a UID hint, and `/` separates
objects and renderers. Environment-variable names appear in reference path
completion, but its path resolver only expands a leading tilde; environment
expansion therefore remains an inherited limitation. A suggestion that quoted
space-containing names contradict the syntax help was not a discrepancy: the
help already requires those names to be quoted.

## Repeated object metadata

`MolCoord::residue_count` previously allocated chain strings and a tree for
every summary request, including unchanged GUI frames. It now retains the
count, invalidates it when atom identity can change, and leaves it intact for
coordinate-only trajectory and morph updates. The initial count uses a linear
walk for ordered atoms and a borrowed-key set for unordered atoms. The cache is
excluded from molecule equality and transfer data. Regression cases cover
clones, insertions, reversed atom order, deletion, copying, chain/residue edits,
coordinate-count errors and undo.

Three sequential fresh processes per condition measured one molecule with
100,000 generated atoms and 25,000 residues on the same Linux x86_64 host. Each
process requested the complete object-summary list 5,000 times after the first
request; the first and final summaries were checked against the expected counts.

| Measured stage | Previous Rust | Current |
| --- | --- | --- |
| First summary | 11.05–12.24 ms | 1.82–2.00 ms |
| Repeated unchanged summary | 6.54–7.46 ms/call | 0.000053–0.000054 ms/call |
| Whole-process peak RSS | 44.34 MiB | 37.41 MiB |

The repeated figure measures metadata enumeration for one object with a warm
count. It excludes rendering, event handling and painting, so it is not a GUI
frame-rate measurement. The 5,000-call interval is about 0.27 ms after caching;
small differences within that interval should not be interpreted as separate
performance effects. First-call timing and process RSS include their reported
cold costs separately.

To regenerate the workload on Linux:

```sh
cargo build --locked --release -p cuemol-desktop-core --example metadata_bench
for sample in 1 2 3; do
  env time -v target/release/examples/metadata_bench 100000 5000
done
```

The example also accepts a PDB path instead of the atom count. Keep captured
reports under the ignored cache. Retain the previous executable before a source
change when comparing two revisions.
