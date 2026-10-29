# Validation evidence

Entries record successive checkpoints, so earlier test counts and distribution
results are historical. Generated inputs and logs stay in the ignored checkout
cache. The current repairs and their limits appear in the completion audit;
passing tests do not establish equivalence for every parameter combination.

## Public release and installation

### Current release: v0.1.6

The [v0.1.6 release](https://github.com/mogura-rs/cuemol-rs/releases/tag/v0.1.6)
was published on 2026-10-29 UTC from application revision `8797617`.
Its [release workflow](https://github.com/mogura-rs/cuemol-rs/actions/runs/40958544520)
passed shared tests, browser tests and all five native builds. Each native
build executed Python helper imports, live-scene queries, selected PDB output
and persistent calls before packaging. The Windows build passed the previously
failing startup path described below. The default-branch cache preparation also
completed on all five targets; this release restored each native tool bundle
without rebuilding it.

The [application CI](https://github.com/mogura-rs/cuemol-rs/actions/runs/40958242903)
passed 1,651 Rust tests with one explicit POV-Ray exclusion, and 95 browser
cases with one optional ICC-fixture exclusion. Ten of those cases use a mock Tauri
bridge. Public Pages checks at `2de4332` separately passed all 86 application
cases in Chrome, including the reference ICC case, and 27 loading, font and
navigation cases each in Firefox and WebKit. All 33 deployed assets at
`8797617` matched their manifest and were byte-identical to that tested build.
Two fresh public checks also passed WebGPU display and compact-palette input.

All eight public archives were downloaded and verified against `SHA256SUMS`,
their source/target/version manifests and license/provenance files. Fresh
isolated installations on Linux x86-64 and macOS Apple Silicon passed public
release discovery, installation and self-update with the published binary hashes.
Both installations passed APBS calculations, PDB2PQR conversion of 1CRN to 642
atoms, FFmpeg encoding/decoding of four distinct frames, native OIDN rendering,
MCP checks and PNG export. All 33 embedded browser assets matched their manifest,
and the installed Linux server passed 17 application cases.

The published macOS app bundle also passed self-update from a path with spaces.
The test deliberately replaced version/provenance metadata and moved the bundled
tools away; updating restored them and retained the published executable hash.
The macOS application bundles remain unsigned.

The [published GUI matrix](https://github.com/mogura-rs/cuemol-rs/actions/runs/40962160335)
passed on Windows x86-64, Linux x86-64/ARM64 and macOS Intel/Apple Silicon.
The macOS checks launched the executable inside the actual published app bundle.
Each target passed OS drag and wheel input, camera-state and pixel checks,
font-family changes, uppercase styles, equivalent line endings and Python-selected
PDB output. Inspection of earlier captures exposed taskbar/Dock overlap at the
bottom of some small CI desktops. The driver now reserves their usable work area
and checks that the entire window fits before capturing it. All five targets
passed with this containment check. Inspection of all five current captures
confirmed that compact palettes remain above the console and the duplicate
viewport count overlay is gone.

The installed Linux window additionally passed residue/range selection, Undo,
distance/angle/torsion measurements, lasso, bond editing, affine transforms,
both-eye stereo picking and animation pause/stop through script and toolbar
controls. A separate
[hosted Tauri check](https://github.com/mogura-rs/cuemol-rs/actions/runs/40931192947)
at source revision `89fce01` observed 20 displayed updates while the OS mouse
button was held, followed by settled drag and wheel frames. This was a source
Tauri build, separately from the published native application matrix.

The performance tables below now compare the verified public v0.1.1 and v0.1.6
binaries. Hardware quad-buffer stereo and real cloud-provider turns remain
unresolved; the completion boundaries distinguish those from passing local and
loopback checks.

### Initial release: v0.1.1

The [v0.1.1 release](https://github.com/mogura-rs/cuemol-rs/releases/tag/v0.1.1)
was published on 2026-10-29 from application revision `c073b9a`.
Its [release workflow](https://github.com/mogura-rs/cuemol-rs/actions/runs/40867588832)
completed the shared and browser tests, five native builds, installed-tool
calculations, installer/self-update checks and publication. The native targets
were Linux x86-64 and ARM64, macOS Intel and Apple Silicon, and Windows x86-64.
The failed initial `v0.1.0` workflow did not publish a release; its tag was not
reassigned.

All eight public archives were downloaded and checked against the published
`SHA256SUMS`. Archive manifests, target identities, version, source revision,
browser payload and required license/provenance files were inspected. The
macOS bundles also had the expected executable name, version and supplied icon.
This checks the published bytes, separately from Actions artifacts.

Fresh isolated installations on Linux x86-64 and macOS Apple Silicon used the
public installer and latest-release URLs. Self-update replaced each executable
and retained the published binary hash; `self-upgrade --check` identified
`v0.1.1` in the renamed repository. The tests removed tool-path and dynamic-library
overrides and ran from an otherwise empty directory. Both installations passed:

- APBS produced 35,937 finite potential samples with the expected decay away
  from a positive ion.
- PDB2PQR converted 1CRN into 642 atoms with finite charges and nonnegative radii.
- FFmpeg encoded and decoded four distinct 64 x 64 video frames.
- POV-Ray rendered an exported molecular scene, and native OIDN denoised a
  molecular render; both produced nonuniform 96 x 96 images.
- MCP tool discovery, the missing-GUI connection error and 128 x 128 PNG output
  passed. All 29 embedded browser assets matched their manifest.

The Linux installation passed 17 browser cases through its embedded server,
covering WebGL/WebGPU pixels and input, read-only scripts and history, streamed
readers, trajectories, animation, affine transforms, visibility, PyMOL import
and stereo picking. Its native window was also operated under Xvfb. Visible
pixel and state checks passed labels, residue/range selection, Undo, distance,
angle and torsion measurements, lasso, bond editing, affine transforms,
both-eye stereo picking, and script/toolbar animation pause and stop.

The published Apple Silicon application bundle was updated from a path with
spaces after its version/provenance metadata was made stale and its bundled
tools were removed from their expected location. Self-update replaced the
executable with the published hash, corrected both metadata files and restored
the tools under `Contents/Resources`. Startup loaded 1CRN through the bundle's
GUI process. After the desktop was unlocked, a further isolated launch passed
twenty OS mouse-drag steps: the saved rotation changed while zoom and center
were preserved. The earlier macOS Tauri interaction checks are recorded below.

Generated release evidence is retained in the ignored validation cache,
including archive/install results, browser and native-window results, macOS
bundle-update results and tool-calculation logs. The macOS bundles remain unsigned.

### Published native GUI matrix

A separate [GUI workflow](https://github.com/mogura-rs/cuemol-rs/actions/runs/40890514343)
downloaded the published v0.1.1 application on all five native targets. Each
job verified its archive checksum, opened an isolated three-atom scene, captured
the OS window, sent drag and wheel input through the OS, and checked both saved
camera state and visible pixels. All five passed. Changed viewport pixels ranged
from 3,195 to 5,496, and every target changed rotation and zoom.

Initial Linux runs exposed missing X11 and graphics-driver runtime packages on
the minimal runners. The workflow now installs those packages before launch;
the Debian/Ubuntu runtime requirements are also documented in the README. This
matrix verifies the tested window/input paths, not every dialog or hardware GPU.

### Native credential storage

The public v0.1.6 binary passed credential save, load, replacement, deletion and
environment-variable fallback for OpenAI, Anthropic and Google on all five
native targets in the
[GUI matrix](https://github.com/mogura-rs/cuemol-rs/actions/runs/40962160335).
Each target made nine
authenticated requests to a loopback mock provider. These exercise the actual
OS stores, without calling a cloud AI service. Linux used an isolated D-Bus
session and GNOME Keyring; the macOS jobs used the actual published app bundle.

## Python helpers and byte-array storage

The native bridge now embeds the reference helper interfaces for file loading,
renderers, scene lookup, commands, molecular editing, URL input, logging,
transactions and wrapper conversion. Tests execute live object loading, selected
PDB output, object copies, atom/residue iteration, chain edits, transforms, RMSD,
compressed URL input and keyword-compatible calls. Python also writes a PNG
through the shared Umbreon exporter. `StreamManager.createHandler(..., 2)` now
constructs the functional scene exporter instead of an incomplete generic proxy.
The reference `povrender` module is outside this work.

JavaScript byte arrays previously represented every byte as a separate object
property and copied the complete array for each indexed access. Native typed
storage now retains aliases and bounds/type checks without those copies.
Python transfers byte blocks through one bulk RPC instead of individual calls.

`scripts/benchmark-byte-arrays.py` compared the public v0.1.1 and v0.1.6 binaries
on the same Linux host. Both executable hashes matched the verified public
archives. Each sample allocated an array and
performed 256 write/read pairs; all checksums were 32,640. Three independent
processes were measured per size. The byte-array and molecular-I/O measurements
began after this task's builds and GUI checks finished; other host workloads were
not controlled. The median internal times were:

| Array size | v0.1.1 | v0.1.6 |
| --- | ---: | ---: |
| 1 KiB | 270 ms | 1 ms |
| 8 KiB | 2,224 ms | 1 ms |
| 64 KiB | 19,112 ms | 1 ms |

The timer has millisecond resolution, so the updated values do not establish a
precise speedup ratio. At 64 KiB, median process peak RSS fell from 31.54 MiB to
21.66 MiB. These measurements cover byte-array access, not complete GUI frames.
The affected core, Python and command suites passed 324 tests; the additional
Python-to-Umbreon output check passed afterward.

Malformed Python RPC arguments now preserve pending object releases and discard
callbacks that were never submitted. NaN and infinity are rejected before a
request is written. Callback results use the same validation, and returned
object proxies stay alive until their reply is sent. Regressions reproduce the
old detached-object and callback leaks, then verify recovery in the same
interpreter. The core and Python suites pass with these cases, interrupted URL
downloads, `.mmcif` URL detection and transaction undo/redo/rollback coverage.

### Native Python process startup

The published v0.1.3 Windows GUI passed mouse, font-family, style and newline
checks, but starting Python returned Windows error 206, "The filename or extension
is too long." Its executable embeds CRLF source files;
the reconstructed worker command line occupied approximately 34,446 UTF-16 units,
exceeding the [Windows process limit](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw).
The worker now receives its embedded modules as one JSON message through standard
input. A short launcher retains the same buffered protocol stream for subsequent
requests, including bytes already read ahead during initialization.

A large Unicode bootstrap reproduced an argument-length failure on Linux before
the repair. Afterward, all eight Python crate tests passed on Linux and macOS,
and eleven live-scene Python regressions passed on Linux. Those regressions cover
callbacks, retained variables, selected output, compressed input and recovery
after exceptions. `scripts/check-python.py` adds a binary-level release gate on
every native target, including Unicode paths and separate persistent calls.
The v0.1.4 release gate passed on all five targets. The public Windows GUI also
passed helper imports, live object queries and selected-atom output, closing the
failure reproduced against v0.1.3.

## Molecular input and output costs

PDB duplicate-name checks and QDF chain/residue lookup previously scanned the
preceding atoms or residues. Temporary indexes now preserve insertion order and
the reference duplicate-name rules. The indexes are dropped before subsequent
processing. QDF input also avoids assigning bonds from rejected duplicate atoms
to an unrelated preceding atom.

`scripts/benchmark-molecular-io.py` compared the verified public v0.1.1 and
v0.1.6 native binaries on the same Linux x86_64 host. Each input contained
separate synthetic ligand residues, and each size used three fresh processes.
At 20,000 atoms, the median internal times were:

| Operation | v0.1.1 | v0.1.6 |
| --- | ---: | ---: |
| PDB input | 1,521 ms | 150 ms |
| QDF output | 186 ms | 6 ms |
| QDF input | 2,005 ms | 138 ms |

All 1,000-, 5,000- and 20,000-atom samples retained the expected atom count,
coordinates and identical QDF SHA-256 values across the two builds. Median peak
process RSS at 20,000 atoms was 59.54 MiB before and 59.21 MiB after, which was
effectively unchanged. This measures the CLI reader/writer
workload, not rendering or total browser memory. The full
molecular-reader suite passed with the reference fixtures present.

`web/benchmarks/load.mjs` measures file input, embedded scene save/reload and
main-thread timer delays in a fresh browser context per sample. In Chrome, the
20,000-atom median input time fell from 768 ms on the then-public WASM build to
159 ms on the optimized local build; the maximum timer gap fell from 692 ms to
77 ms. Embedded scene reload fell from 416 ms to 136 ms. Scene saving was
unchanged at approximately 53 ms. Parsing still occurs on the main thread in
these measurements, so they do not establish nonblocking parsing.

## Browser parsing workers

Browser file input now parses molecules, compressed data, maps and PyMOL pickle
data outside the UI runtime. A separate parser-only WASM module shares the
native readers. Binary transfers retain floating-point bits, atom IDs and order,
bond properties, unit cells, residue properties and map statistics. QSC loading
transfers parsed molecules and maps while the UI restores scene metadata with
current application IDs. Scene assembly and geometry construction still use
the UI runtime; they can produce frame delays for large inputs.

Input files remain ordered. Undo state is captured immediately before applying
a successful result, so parsing failures preserve intervening edits and redo.
Escape or the status-bar button terminates the worker and clears queued files.
Changing scenes cancels the active load and clears queued files, including
pending ICC input. A scene loaded by the queue becomes the target for its next
file. Nearby loads reuse a worker, which is
terminated after two idle seconds to release its linear memory.

The affected shared suites passed 808 tests. Chrome passed the 71 available
application cases, followed by the previously missing reference ICC case and
the added compressed-input case. The final parser-only module passed all seven
loading cases and the ICC comparison; Firefox and WebKit also exercise the
loading cases. These checks cover precision, compression, concurrent edits and
rotation, cancellation, queue order, error recovery, scene changes and worker
release/recreation. The ten Tauri shell tests use a separate mock bridge and are
not counted as deployed WASM checks.

After publication at `0144383`, all 33 public assets matched the source manifest.
Mac Chrome passed all 73 deployed application cases; Firefox and WebKit passed
seven loading cases each. A subsequent review prompted two additional cases:
manual scene changes incorrectly allowed a queued file or pending ICC profile
to alter the new scene. Both failures were reproduced against that public
revision. The shared load guard now cancels those operations and preserves the
new scene; generation checks also discard their late results. Another regression
reproduced a scene change after queueing but before parsing started. The queue
now retains its destination until it is drained or cancelled. All ten loading
cases pass, including a queued QSC followed by a molecule in its new scene.

A follow-up review exposed cancellation clearing the busy state of a concurrent
render. An actual browser regression reproduced this failure. Get PDB had two
related failures: a response could enter a different scene, and a failed request
could clear the render's busy state. Both were reproduced through the dialog.
PDB requests now share the input queue and cancel their fetch with an abort
signal. Their response is passed as a Blob-backed File to the parser worker.
All fourteen Chrome loading cases pass, including network cancellation, HTTP
error recovery, and serialized loading after render cancellation.

Three fresh Chrome contexts per size compared the already optimized public
revision `4b417fa` with this worker build. For 20,000 atoms, median input time was
157 ms before and 163 ms after; the maximum timer gap fell from 77 ms to 64 ms.
Embedded scene reload was 134 ms before and 149 ms after. This is a responsiveness
change, not a uniform reduction in elapsed loading time.

The parser module is approximately 1.8 MiB, compared with the 25 MiB application
module. Using it instead of a second complete application runtime reduced the
20,000-atom worker's linear memory from 22.5 to 15.5 MiB. Main-runtime linear
memory after scene reload was 54.44 MiB before worker loading and 52.50 MiB after.
The worker adds transient memory, reaching 20.13 MiB during the scene reload,
and is then released. These values describe WASM linear memory, not browser
process RSS or a claim of lower peak memory for every load.

## Compact viewport controls

The native/browser tool palette previously grew beyond a short viewport and
covered the console. A 960 x 600 browser regression reproduced this against the
public v0.1.4 runtime: the torsion button extended ten pixels below the viewport.
The complete Windows and macOS captures also showed the overlap.

The shared UI now bounds the palette to its viewport, keeps the collapse button
fixed and scrolls the tool list. The browser case verifies that the final tool
can be reached and selected without changing camera zoom, then opens its options
and collapses the palette. Captured option-menu contents remain visible. All 86
Chrome application cases pass, and the macOS native UI library passes 171 tests
with one explicit exclusion. Firefox and WebKit each pass 27 loading, font and
navigation cases, including the compact-palette regression.

A real Linux window at 944 x 648 passes drag, wheel, font and Python checks. An
additional OS wheel-input check scrolls the palette's visible tools while keeping
the camera center, rotation and zoom unchanged. The captured console is uncovered.
These checks exercise the vertical native/browser palette; Tauri uses its own
horizontal toolbar.

The public v0.1.5 captures exposed a second overlap: the native viewport repeated
the atom and bond counts at its lower edge, beneath the compact palette. Those
counts already appear in the bottom status bar. The native view now uses that
status bar alone, leaving the molecular viewport clear. The affected native UI
suite passes 171 tests with the optional POV-Ray process test excluded.
A real 944 x 588 Linux window also passes OS drag, wheel, font and Python checks.
Scrolling the tool list preserves the camera, and its captured lower edge no
longer overlaps an atom-count label.
The same checks pass with the public v0.1.6 binary. Its five-platform window
captures also confirm that counts appear only in the bottom status bar.

## Center marks and display scaling

Live center marks now invert the displayed RGB channels after molecular geometry
and labels, using the reference blend factors. Alpha remains unchanged. The
same GPU implementation draws the cross or rotated axes in native, browser and
Tauri viewports, including both stereo eyes. The previous implementation chose
a fixed gray from the background color and could lose contrast over objects.

Metal pixel tests cover colored backgrounds, transparent output, molecular
geometry, both eyes and rotated axes. Browser WebGL and WebGPU tests check
inversion, removal and a 20-logical-pixel cross at display scale two. Tauri also
renders marks in empty scenes. Display scaling now keeps its marks, labels and
selection outlines at their intended logical size, including reduced-resolution
navigation frames. GPU tests cover label scale one and two. The affected Tauri
suite passed 147 tests, its explicit GPU case passed, and all ten shell cases
passed, including startup and settled camera frames in an empty scene. The
native Linux window also passed OS drag and wheel input checks.

## Font families and text placement

Label font names now select installed native fonts, with embedded Hack and Ubuntu
fallbacks. The native path shapes Unicode text and retains glyph bitmaps between
labels; it clears retained bitmaps after the cache exceeds 8 MiB. A regular-only
font also supports requested bold through synthesis. Browser labels use the
reference OffscreenCanvas path. Both paths recognize uppercase style/weight
names, use integer CSS pixel sizes and round bitmap widths to four pixels.

Regressions first reproduced ignored family names and case-sensitive styles.
The repaired native tests cover family selection, monospaced advances, styles,
weight and an isolated regular-only font database. Metal tests use a fixed font
to check per-pixel occlusion, overlay ordering, alignment and HiDPI dimensions.
The shared-core suite passed 320 tests with the reference QIF files present.
The Tauri library passed 147 tests; six optional runtime cases were not run by
that ordinary library invocation. Chrome passed all 76 application cases before
the two additional queue cases; Firefox and WebKit each passed all eleven loading
and font checks after those additions. Browser tests compare glyph alpha against
the reference OffscreenCanvas algorithm and inspect displayed magenta labels on
both GPU backends.

Public-code review also identified a native crash for CR-only line endings. A
regression reproduced the out-of-bounds descent lookup. The label bitmap now
uses the shaper's actual line count; CR, CRLF and LFCR produce the same native
pixels as LF. All six native font cases pass. An isolated regular-only font
also passed italic synthesis without a production change.

A subsequent browser regression reproduced CR-only text being drawn on one
line. Browser line splitting now recognizes LF, CR, CRLF and LFCR without
allocating a normalized text copy, including consecutive line endings that
leave an empty line. The native regression covers those empty-line variants.
Chrome passed four font cases across WebGL and WebGPU; Firefox and WebKit each
passed the two WebGL font cases, with exact alpha-mask comparisons between
equivalent line-ending variants.

The Linux native window passed OS drag and wheel input, changes between embedded
monospaced and proportional fonts, and identical displayed pixels for lowercase
and uppercase bold/italic settings. It also displayed identical multiline labels
for LF, CR, CRLF and LFCR. The
[hosted Mac Tauri check](https://github.com/mogura-rs/cuemol-rs/actions/runs/40931192947)
built application revision `89fce01` and passed real OS mouse and wheel input.
Its CR-separated, bold italic label remained visible, with twenty changed
captures while the mouse button was held and a further change after scrolling.
The eight-second drag includes test capture delays and is not a frame-rate
benchmark. Exact glyph pixels remain dependent on the installed font and native
rasterizer.

## Browser click timing

CI exposed a residue-selection failure when a double-click took 1.7 seconds to
dispatch under software rendering. A controlled test with a 700 ms input-handler
stall reproduced the failure: the second release exceeded the shared 500 ms
timer and was treated as another single click. Browser input now retains the
native mouse click count as well as the event timestamp. A browser-recognized
double-click can therefore select a residue after a stall; dragging still uses
the shared movement classifier. The delayed-input regression passes with this
repair. The complete Chrome application suite passed all 83 cases after the
font, loading, network and input repairs.
Firefox and WebKit each passed all 25 loading, font and navigation cases on the
same build.

## Platform failures repaired before publication

Actual matrix failures led to these repairs: APBS static BLAS dependencies on
Linux ARM64, the required ISPC version for ARM64 OIDN, portable macOS APBS
libraries, Windows APBS's matching Python runtime, the obsolete POV-Ray
`std::binary_function` base, Windows surface-library math constants, and
USalign's platform-dependent `c_char` buffer type. Windows self-update now
removes inherited `PSModulePath` from its Windows PowerShell child. This avoids
loading PowerShell 7 module paths when the update starts from `pwsh`.
The final release executed the native calculations and platform installer checks
successfully on all five targets.

The Intel macOS cold build had also exceeded its job limit. Its allowance is
now 45 minutes, and completed tool caches are saved before application builds;
Rust caches are retained after downstream failure. The final run completed.
These operational observations are not a controlled build-speed benchmark.

## Public Pages checks

At application revision `14c182f`,
[CI](https://github.com/mogura-rs/cuemol-rs/actions/runs/40934636951)
passed 1,650 Rust tests with one explicit ignored case, and 94 browser cases
with one missing-reference-profile skip. The browser total includes ten Tauri
shell cases that use a mock bridge.
The [public deployment](https://github.com/mogura-rs/cuemol-rs/actions/runs/40936452224)
matched all 33 asset hashes in its manifest. Chrome then passed all 85 deployed
application cases with the reference profile available. Firefox and WebKit
each passed 26 deployed loading, font and navigation cases. These public counts
exclude the Tauri mock bridge.

The renamed [public browser application](https://mogura-rs.github.io/cuemol-rs/)
was checked at application revision `c073b9a`. Its
[CI run](https://github.com/mogura-rs/cuemol-rs/actions/runs/40867507458)
passed 1,632 Rust tests with one explicit ignored case, and 72 browser cases
with one missing-reference-profile skip. Compatibility tests that return early
when fixtures are absent are not counted as executed reference comparisons.
The separate reference-present checks remain recorded below.

The [Pages deployment](https://github.com/mogura-rs/cuemol-rs/actions/runs/40868785741)
published the expected commit and repository identity. All 29 public asset
hashes matched the manifest. Twenty-one browser cases had run against the public
URL at `2a3922b`, covering WebGL/WebGPU pixels and input, scripts and history,
trajectories, USalign, ICC handling, analytic surfaces, OIDN, affine geometry,
visibility, PyMOL import and stereo picking. The additional CMYK proof case
passed with the reference ICC profile available, for 22 public checks in total.
All 29 browser assets at `c073b9a` were byte-identical to that tested deployment;
the manifest recorded the newer source commit.

### Connection-pool repair

A separate release run exposed an HTTP startup stall before the application
module executed. The trace showed `input.js` waiting without response while
other modules completed. A direct server reproduction pinned the server to one
CPU, queued twelve keep-alive connections while it was stopped, then resumed
it. The upstream HTTP pool served four connections and left eight waiting.
The Rust regression reproduced the same failure on connection four.

The pool counted notified workers as idle even when queued jobs had already
reserved them. The local patch accounts for those jobs before accepting another
reservation. After the change, the Rust test passed, twenty repeated bursts
served all twelve clients each, and six repeated WebGPU/WebGL effect cases
passed. A separate slow-download test covers response-worker concurrency.
These tests establish the reproduced transport failure and its repair; they do
not imply that every possible browser startup failure has the same cause.

## Completion audit

The audit of application revision `092272a` found reproducible failures in
style extraction, Python state and command-line script arguments, together with
unnecessary molecular copies and Undo entries. The repair now covers these
entry points and related failures found while tracing their callers.

| Area | Repair and regression coverage |
| --- | --- |
| Style extraction | Preserve live and detached renderer properties, nested style values, inherited coloring and explicit overrides. |
| Python | Keep one interpreter per application, expose live CueMol objects and callbacks, preserve file globals and recover after exceptions. Release worker processes and detached payloads with their owners. |
| Script services | Keep log, event-listener, process-manager and molecular-client state across script calls. |
| Script arguments | Pass trailing arguments through `run` and `-c`, including Unicode, spaces and empty values. |
| Script history | Add Undo only when persistent scene state changes. Queries and failed scripts preserve Redo in native, Tauri, browser and MCP paths. |
| Molecular storage | Share atoms, bonds and residue properties across snapshots, with copy-on-write edits and selective Undo. Build render snapshots only when scripts need them. |
| Script edits | Flush queued object insertion before subsequent atom edits; preserve intentionally empty surface-renderer lists. |
| Tauri interaction | Update camera motion during a drag, coalesce pending requests, avoid applying deltas twice, preserve full resolution, settle rendering after cancellation and map HiDPI input to render pixels. Disable smart quote replacement in code inputs. |
| Application shell | Use Tritium activities, inspector and docks with persistent themes; recover from missing or damaged layout settings. Use the `cuemol-rs` name and supplied PNG logo. |
| Web asset serving | Serve independent requests with eight workers so a stalled large download does not stop every other asset. A slow-reader regression failed before the change and passed afterward; both formerly failing browser scenarios passed five repetitions each. |
| HTTP connection dispatch | Account for queued jobs when reserving idle workers. The upstream pool accepted twelve simultaneous keep-alive connections but served only four; the remaining connections could wait indefinitely. |
| MCP entry points | Bare `mcp` reports the same connection error as `mcp connect` instead of panicking. `render_image` declares file writes while preserving scene state and Redo. |
| Application updates | Replace the actual renamed executable, install missing resources into the application bundle, and update its version and provenance metadata with rollback on replacement failure. |

The original style probe used `linew` on `SimpleRenderer`. That property name was
incorrect: the simple renderer uses `width`. The `alpha` loss was a valid
failure. The corrected regression checks both `alpha=0.4` and `width=3.25`, plus
inherited nested properties and coloring. This corrects the earlier audit text.

```javascript
var object = cuemol.MolCoord();
object.name = 'probe';
scene.addObject(object);
var renderer = object.createRenderer('simple', 'probe-renderer');
renderer.alpha = 0.4;
renderer.width = 3.25;
var styles = getService('StyleManager');
var id = styles.createStyleSet('audit', scene.uid);
styles.createStyleFromObj(scene.uid, id, 'Probe', renderer);
print(styles.getStyleValue(scene.uid, 'audit', 'Probe.alpha'));
print(styles.getStyleValue(scene.uid, 'audit', 'Probe.width'));
```

```javascript
var bridge = getService('PythonBridge');
bridge.runString('completion_probe = 42');
bridge.runString('assert completion_probe == 42');
```

Regression tests cover Python reads and writes against the live scene, changes
made by a later JavaScript call, nested event callbacks, exception rollback,
subprocess output and native descriptor writes without corrupting the bridge
protocol. The core suite passed 264 tests with scripting enabled. The reference
checkout was present for the shared compatibility tests and seven QIF audits.
Those audits cover 166 class declarations, construction, inheritance, defaults
and enum round trips. Checking that every declared method is callable does not
execute every method or every possible argument combination.

### Script and history measurement

The baseline used a fresh Linux x86-64 release MCP process, optionally loaded
50,000 synthetic atoms, cleared Undo, issued 20 `get_scene_state` calls, then
issued 20 `execute_script` calls containing only `print(1);`. The scene JSON was
identical before and after. Loading and initialization were excluded from
request times. Three processes ran per atom count. Each request time below is
the median of each run's 20 calls, followed by the median across three runs.
Compilation from this task had finished; other shared-host workloads were not
controlled. Memory came from the server process's Linux `VmRSS`.

| Baseline atoms | Scene query | Script call | RSS before calls | RSS after queries | RSS after scripts |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 0.708 ms | 2.647 ms | 13.78 MiB | 13.78 MiB | 28.88 MiB |
| 50,000 | 3.798 ms | 74.628 ms | 55.70 MiB | 55.70 MiB | 428.32 MiB |

These baseline values include MCP and script execution, not GUI frame time.
The earlier implementation cloned molecule arrays and recorded up to 20 complete
Undo snapshots for unchanged scripts. The new storage and history paths remove
those copies and entries. Repeating the same protocol with the repaired release
produced the following results.

| Repaired atoms | Scene query | Script call | RSS before calls | RSS after queries | RSS after scripts |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 0.725 ms | 2.163 ms | 13.56 MiB | 13.56 MiB | 24.72 MiB |
| 50,000 | 4.283 ms | 2.253 ms | 55.66 MiB | 55.66 MiB | 62.22 MiB |

At 50,000 atoms the script median decreased by about 33 times and the final
RSS decreased by about 85%. Scene-query latency did not improve in this run.
All six processes preserved the scene JSON. These are measurements of the
listed workload on a shared host, not an overall application speedup.

### Desktop interaction checks

The native egui library passed 171 tests, and the Tauri library passed 147 tests
with five optional GPU/fixture tests excluded. The Tauri shell compatibility
suite passed 69 tests. Browser execution passed 64 viewer tests across WebGL2
and WebGPU plus seven shell tests; two later shell regressions brought the
separate shell run to nine passing tests. The shell tests mock IPC and verify
DOM behavior; they do not establish GPU correctness.

The optimized macOS `cuemol-rs.app` was also operated directly with 1CRN loaded.
A held mouse drag changed the actual viewport image 23 times within 617 ms,
with the first observed change at 19.7 ms. The preceding run observed 27 updates
within 613 ms and a first update at 48 ms. Observation includes input dispatch
and accessibility polling, so these are interaction samples rather than a
steady-state frame-rate benchmark. The viewport retained full resolution.
Literal double quotes were preserved and the console executed the test script.
A later check refreshed the accessibility tree for each observation and saw 26
viewport updates during a 914 ms drag, with the first change at 35.9 ms. The
final rotation matched all 30 submitted input steps, and the captured viewport
remained 972 x 486 pixels. These observations include accessibility overhead.
A separate optimized 1280 x 720 offscreen profile measured median rendering
at 3.978 ms and PNG encoding at 0.426 ms over 15 frames; it excludes WebView
transport and display latency.

The Linux release window passed visible-pixel checks for labels, residue/range
selection, Undo, three measurement modes, lasso, bond editing, affine transforms,
stereo picking in both eyes, and animation pause/stop through both script and
toolbar controls. Claude Code reviewed the interaction changes. Its script-file
rollback concern was disproved by a regression that changes the scene and camera
before throwing; the transaction commits only after successful evaluation.
Its cancellation concern led to the settled-frame regression and fix.

### Completion boundaries

The DCD, BVH, camera-animation and GPU-upload measurements below apply to their
recorded inputs and stages. They do not establish optimization of every reader,
renderer, map/surface calculation or complete GUI frame. Ordinary browser file
input uses the parser worker described above. Script asynchronous readers keep
their synchronous `waitLoadAsync` API and collect the input before parsing;
native execution uses a worker,
while the browser parses in `waitLoadAsync`. Indexed browser trajectories instead
read individual frames with `File.slice`.

Other completion boundaries apply to the native runtime, display and providers:

- Python execution requires native `uv`; supported helpers are listed above.
  The reference `povrender` module and third-party Python packages are not bundled.
- Hardware quad-buffer stereo is unimplemented. Implementing and validating its
  presentation backend requires a target OS, GPU and stereo display.
- Font rasterization can differ, and the Rust ray tracer does not promise the
  reference integrator's exact pixels.
- Actual OS credential-store checks are recorded above. Real AI provider turns
  remain unverified because CueMol credentials were not available for this
  validation; loopback-provider tests do not establish live-cloud behavior.

The public release and Pages checks distinguish local execution from actual
public deployment.

## Functional checks

- The headless workspace suite passed 1,091 tests at the packaging checkpoint,
  with optional compatibility tests subject to their documented skips. The final
  denoiser and render-setting regressions passed in the affected crate suites.
- The native egui library suite passed 157 tests, including wgpu shader and
  primitive-pipeline validation against an available adapter. The installed GUI
  also loaded 1CRN with its default ribbon and saved a 256 x 256 hatched PNG
  through the Rendering menu; both the dialog and output were inspected.
- Browser tests exercised visible molecular pixels, mouse zoom and slab control,
  commands, undo/redo, embedded QSC and PNG export, frame seeking, startup scripts,
  shared properties, and animation playback. Three tests passed under Chromium
  with the WebGL2 fallback, including the browser assets embedded in an installed
  release binary served from an otherwise empty directory. WebGPU was separately displayed and manipulated in
  Chrome on Apple hardware; this does not establish support for every GPU driver.
- APBS 3.4.1 produced a potential map for 1CRN through native MCP. Bundled
  PDB2PQR 3.8.0 converted the same structure, and bundled FFmpeg encoded a short
  generated video. The APBS source-build path also compiled on Linux x86_64.
  Native mouse rotation and slab changes remained responsive while an MCP APBS
  job ran through an intentionally delayed process fixture. An installed release
  with empty GUI preferences detected its bundled PDB2PQR/APBS and produced the
  same potential map without a tool-path environment override.
- MDTraj wrote DCD, XTC, TRR and NetCDF trajectories with known coordinates.
  Indexed reading with stride two and nonsequential seeks matched those
  coordinates, using the XTC format's quantization tolerance.
- Installer tests covered a local release archive, tool installation, checksum
  rejection without replacing the previous executable, rollback when the final
  binary replacement fails, and self-upgrade.

## Trajectory memory and load time

One synthetic little-endian DCD contained 20,000 atoms and 2,000 frames, occupying
480,048,116 bytes. The same native debug executable was measured with GNU time
on Linux x86_64. This is one run per command with no cold-cache guarantee.

| Reader | Wall time | Maximum RSS |
| --- | ---: | ---: |
| Legacy eager `dcd-info` | 7.99 s | 492,128 KiB |
| Indexed `trajectory-info` | 0.73 s | 22,052 KiB |

The indexed command decoded the first frame and retained a 32,768-byte frame
index. These numbers describe loading and inspection, not the GPU frame rate or
memory use of every topology, compression format, and rendering representation.

## Ray-intersection acceleration

A 128 x 96 render of 1CRN took 15.548398 seconds before the BVH and 0.109634
seconds after it in the debug build. RGB output matched exactly. Both runs used
the then-current direct-lighting path; the later hatching pass was not included.
A separate test compared BVH and brute-force intersections for 1,000 rays,
including parallel rays, outline bounds, and multiple shadow-distance cutoffs.
The single-scene timings should not be treated as a general speedup guarantee.

## Packaging

At the earlier packaging checkpoint, the Linux x86_64 archive was approximately 123 MiB and included the native
binary, embedded browser assets, and scientific tool bundle. Archive creation
with gzip level three took 10.74 seconds in one local run. This excludes the
native build and first-time tool downloads/builds.

## Historical verification scope

Release workflows were configured for Linux x86_64/ARM64, macOS Intel/Apple
Silicon, and Windows x86_64. At this checkpoint, local Linux and Apple Silicon
builds plus browser checks had run; other native targets and public installation
were waiting for authorization. Later public results are recorded separately.
The README lists implementation boundaries that also limit parity claims.

## Browser numerical kernels

USalign also runs in the browser through the same vendored C++ kernel used by
the native application. Five browser tests passed after adding this integration.
The alignment regression applies a known rotation and translation to twelve
noncoplanar C-alpha coordinates, checks the recovered transform through the
shared command API, and verifies unit TM scores and near-zero RMSD.

LittleCMS is compiled into both native and browser distributions. The native
profile tests and browser checks cover profile validation, alpha preservation,
profile persistence, proofed image export, and four Generic CMYK colors. The
CMYK values matched an independent call to the installed LittleCMS library.
The optional reference profile was present for these checks.

## Analytic molecular surfaces

The bundled MeshMS and BALL calculations run on native and WebAssembly targets.
A single-sphere regression checks radius, unit normals, and surface area against
analytic values. Shared command tests cover both engines, the Rust grid engine,
noninteger density, regeneration, and an embedded QSC save/reload. The browser
suite passed eight tests, including both analytic engines and regeneration.
All 157 native GUI tests passed after the backend selector was added. Source
revisions and local portability changes are recorded beside the bundled kernels.

## NPR styles and layer editing

The Rust line, dot, and stipple kernels matched 5,040 reference C++ samples
within the stated floating-point tolerance. The composition regression also
matched all nine styles for identical input tone, albedo, and shaded color.
These comparisons cover the procedural ink and composition stages, not every
ray-traced lighting result. Layer/tone serialization, deterministic marks, and
increasing coverage as tone darkens passed. The native GUI suite passed all
157 tests, and all nine browser tests passed, including custom hatch edits,
style differences, and transparent PNG output.

## Lighting and transparent sections

The diffuse path tracer is checked against an analytic uniform environment and
against red light reflected from a wall. Other regressions check direct-light
energy, AO radius and contact/shape scales, the nearest BVH hit, crease-angle
thresholds, and nonnegative per-pixel weights for overlapping transparent
sections. Equal-alpha sections share one veil pass, and the legacy affine blend
remains selectable. These tests establish the listed properties, not pixelwise
equivalence with Umbreon's Embree integrator or convergence for every scene.
The affected render suite passed 65 tests and the command integration suites
passed 12. All 157 native GUI tests and 10 browser tests passed. An older shell
compatibility assertion still named the pre-backend surface API; it was updated
to the current call, after which all 69 shell tests passed.

## OIDN denoising

Native OIDN 2.5.0 is loaded from the packaged CPU runtime. A 64 x 48 synthetic
HDR image with two constant regions and clean albedo/normal guides had less
than one quarter of its original mean squared noise error after filtering.
The test also checked the region values and unchanged alpha. A separate
recombination test preserved an unlit annotation while adding filtered indirect
irradiance to direct light. The render suite passed 67 tests, and the native
GUI and shell suites passed 157 and 69 tests respectively.

All 11 browser tests passed, including a real WebGPU OIDN execution using the
packaged model. The OIDN regression compared raw and filtered transparent PNGs,
confirmed changed RGB values and identical alpha, and verified the model request.
The interactive viewport used WebGL2 while OIDN used WebGPU in the same browser.
The native and WebGPU implementations use different model/runtime versions;
this check does not assert numerical identity between their filtered pixels.

## Responsive browser rendering

The shared renderer can evaluate bounded row ranges with identical camera
projection. Native tests compare their concatenated floating-point pixels to a
whole-frame render. Browser rendering reuses one BVH and adapts its row batches
to yield between calculations. All 12 browser tests passed, including camera
zoom during a large GI render, cancellation, and successful rendering afterwards.
OIDN cancellation also closes the pending browser render operation. The native
OIDN quality and alpha tests passed on Apple Silicon with its bundled CPU runtime.


## Optional AI Agent

Loopback mock providers exercised OpenAI Responses, Anthropic Messages, and
Google streaming generation with tool calls, images, opaque reasoning data,
usage accounting, model catalogs, malformed streams, and cancellation. No paid
provider requests or real credentials were used. The CLI regression supplied a
session key through standard input, edited a molecule, and saved the result as
an embedded scene without writing the key to configuration.

All 15 browser tests passed. Agent regressions cover streamed replies, downloaded
structures, rendered images, one undo entry per turn, cancellation after visible
edits, and indexed trajectory seeking with undo/redo. Native tests also cover
background APBS execution and rejection of late APBS output after a manual edit.
The shared render suite passed 68 tests, including cancellation before output
creation. These checks establish the integration contract; live model behavior
and platform credential stores require their respective services.


## Tritium interface

The native layout has interaction checks for activity switching, sidebar collapse,
property editing and undo. The browser suite exercises Inspector text editing,
toolbar undo, rectangle selection and right-drag zoom alongside the existing
trajectory, rendering, color-management and agent tests. All 17 browser checks
passed; the native suite passed 162 unit and 69 shell compatibility checks.
The native offscreen interface snapshot covers the panels and controls, while
browser screenshots include the live wgpu viewport. This does not make the
offscreen interface snapshot a native-window rendering test.


## Release and Pages workflows

Distribution helpers validate all five native targets, both macOS applications,
and the browser archive before creating checksums. Six local tests cover missing
platforms, source and checksum mismatches, stale macOS applications, draft retry,
and refusal to overwrite a published release. The Linux distribution passed
installation, self-upgrade, bundled PDB2PQR and image-rendering checks. A browser
smoke test passed with the staged assets served below a repository-style subpath.
The release build embeds the isolated, verified browser artifact through
`CUEMOL_WEB_DIR`; source-tree test files are not part of that artifact.
All five workflow files passed actionlint. At this earlier checkpoint, the
changes had not been pushed, so GitHub-hosted release and Pages runs had not
executed. Later public results are recorded separately above.

## Backbone geometry and secondary structure

The C++ reference library was built from the read-only sibling source and used
as an executable oracle for ribbon, cartoon and tube meshes. Forty-eight 1CRN
configurations covered the default, round and fancy sections, helix cylinders,
wavy/constant widths, ribbon helices, spherical caps, flat junctions, custom
arrows, putty, smoothing, anchor selections, partial and disjoint selections,
segment-end transparency, and smooth/stepped coloring. Every vertex position and normal
matched in both directions. The position tolerance was 0.000005 angstrom; the
largest observed position error in the standard styles was about 0.0000022
angstrom, consistent with the reference mesh storing float coordinates.
Nondegenerate triangle positions and winding also matched in both directions;
triangles with edges below 0.00001 angstrom were excluded from that comparison.
The Rust mesh shares rings instead of reproducing duplicate reference vertices.
All cases matched RGB and alpha within one 8-bit level, including solid colors,
named-color modifiers, molecular color inheritance, and smooth/stepped paint boundaries.
Partial ribbon/tube selections retain full-chain spline coefficients; cartoon
segments follow the reference's local fitting. Selection-end fading tests
include selections that contain no pivot atoms and unchanged physical termini.
Named paint selections are resolved once before coloring, including style aliases.

Twenty section-profile configurations matched C++ coordinates, normals and face
labels within 1e-12. Sixty weighted/unweighted smoothing fits, including short
segments and linear extrapolation, produced identical sampled coordinates,
first derivatives and second derivatives. These optional tests require
`CUEMOL_BACKBONE_ORACLE` and `CUEMOL_BACKBONE_CONFIG`; the reference program's
source is `crates/cuemol-molvis/tests/reference_backbone.cpp`.

The 1CRN secondary-structure assignment now agrees with the current C++ reader:
the first alpha helix ends at residue 17. Proline is excluded as a hydrogen-bond
donor regardless of case. Sparse hydrogen-bond and bridge searches match an
independent exhaustive enumeration in the shared tests. The secondary-structure
pass also avoids constructing atom-index lists for solvent residues and avoids
scanning the atom array for every property assignment.

The corrected source comments and style data include the previous proline
case-sensitivity claim, the obsolete DefaultRibbon dimensions, and the blanket
claim that tube/spline axial detail defaults to eight. The actual tube/spline
QIF default is six; the named ribbon styles and cartoon use eight.

The color cascade follows the renderer's scheme and resolves `$molcol` through
the object's scheme for every element. Named colors use the scene's definition
before the global one, including hue, saturation, brightness and alpha modifiers.
The default load palette follows Tritium's molecule order and persists its color
definitions in embedded scenes. Tests cover noncarbon pivots, paint fallbacks,
two distinct molecule colors and an embedded save/reload. Older synthetic coloring
fixtures omitted the explicit `$molcol` styles required by the C++ implementation;
those inputs now include the styles. The old load-palette expectation was also
updated from the legacy SteelBlue/khaki colors to the current Tritium palette.

A native Linux window in a private virtual display was inspected and controlled
with real input events. Wheel zoom, Shift-wheel slab thickness, trackball rotation,
Inspector editing and toolbar undo changed the live scene as expected. The
macOS GUI also loaded the molecule and accepted MCP representation changes;
its rendered screenshots were inspected, but screen capture permission was not
available for capturing the macOS window itself.

Saved molecular surfaces and both direct-surface renderers now resolve their
own coloring schemes, including solid, paint, atom-property colors and object
inheritance. Shared and legacy CLI exports check the resulting vertex colors.
The affected shared suites passed 206 tests, followed by 162 native GUI tests,
69 alternate-shell tests and 17 browser tests. The earlier full shared-workspace
run passed 1,140 tests; the final surface changes were then covered by the affected
suites. Native GUI builds with bundled browser assets passed on Linux and macOS.

The final native Linux window passed wheel zoom, Shift-wheel slab, trackball,
Inspector rename and toolbar undo checks. A real 1CRN molecule was also displayed
as ribbon and cartoon in WebGL and WebGPU. The Linux software-browser check
requires Vulkan compositing flags in addition to enabling the WebGPU API;
without them, even an independent WebGPU red-clear canvas was blank. With those
flags, both the minimal canvas and the application rendered correctly. The same
application was also checked with Chrome's WebGPU backend on a Mac GPU.
See Chromium's [SwiftShader configuration](https://github.com/chromium/chromium/blob/main/docs/gpu/swiftshader.md)
and [headless GPU guidance](https://chromium.googlesource.com/chromium/src.git/+/main/docs/gpu/using-gpu-hardware-in-headless-chrome.md).

Additional documentation corrections: the trajectory header previously described
the current C++ reader as eager, although it also indexes frames; the native
frontend comment referred to XUL rather than the current Tritium layout; and the
surface-preview comment said light grey although the helper applies atom colors.
Object-color documentation now states the solid default used for `$molcol`.
Synthetic tests that expected molecular inheritance without `$molcol`, or a
spline mesh for a physical chain with fewer than three residues, now express the
reference's actual prerequisites.

## Reference input comparison

The native and browser viewports now use the shared `ViewInputController`.
The read-only C++ oracle checks 3,862 camera states across Default, Coot, PyMOL,
iPhone and Trackpad presets, modifier combinations, rotated cameras, different
viewport sizes, custom and swapped bindings, axis locking and momentum. All
states match within the numeric tolerance in `view_input_compat.rs`.
The browser retains individual DOM events rather than summing them once per frame.
The native adapter converts physical positions to logical viewport coordinates.

The input implementation corrects the previous half-angle trackball error,
independent Euler browser rotation, distance-changing zoom, fixed pan scale,
ignored preset bindings, drag threshold, click timing and unconnected momentum.
Command-based rotation and translation now use the same reference conventions.
A real X11 wheel step produced a DOM delta of 120 in Chromium. The native
adapter now uses that same conversion. Cocoa wheel ticks use 40 pixels, while
Windows uses the configured system line/character counts and Chromium's
100/3 pixels-per-line conversion. Sources:
[Chromium Cocoa input](https://github.com/chromium/chromium/blob/main/components/input/web_input_event_builders_mac.mm),
[Cocoa tick scale](https://github.com/chromium/chromium/blob/main/ui/events/cocoa/cocoa_event_utils.h),
and [Windows input](https://github.com/chromium/chromium/blob/main/ui/events/blink/web_input_event_builders_win.cc).

The README and browser help previously documented the earlier custom mappings
(middle-drag pan, right-drag zoom, Shift-wheel slab and Ctrl-wheel depth). They
now describe the reference Default preset and explain how to select another
preset. Earlier mouse smoke checks in this report describe that previous mapping.
The optional snapshot-test comment also now identifies the Tritium layout rather
than the older XUL inventory.

The native event checks also found that replacing a camera discarded the last
pointer position. That position now belongs to the GUI, so an MCP camera update
no longer prevents the next drag from starting. Both frontends exclude floating
windows and menus from viewport input. A browser interaction test changes the
input device through Settings, confirms that the dialog captures wheel events,
and reloads the viewer to verify the saved trackpad behavior.

The published winit XInput2 path emits a wheel event on both button press and
release. The native input filter correlates the preceding raw button state and
drops only releases of wheel buttons 4 through 7 before either egui or the camera
receives them. Valuator scrolling is unchanged; pending state is cleared between
event batches and when focus leaves. Cargo resolves the unmodified published
crate, and release packages retain its license.

The final native X11 interaction check matched one wheel step from zoom 50 to
38, retained the camera distance, and checked the drag threshold, pan transform
and both Shift-drag axis locks. A floating Options window captured the wheel;
selecting Trackpad changed scrolling to translation and survived a full process
restart. The old multi-touch checkbox is represented by the input-device selector;
legacy enabled values migrate to Trackpad until an explicit device is selected.

At the input checkpoint, the shared workspace passed 1,149 tests and the alternate
shell passed 69 tests. Native GUI suites passed 162 tests each on Linux and macOS.
The browser suite's 20 existing cases passed, and its additional Settings case
passed after correcting the test's menu-transition wait. After the final device
migration change, all four input/render-cancellation browser cases passed again.
Distribution-helper tests passed six cases. The native and browser adapters were
also checked against the C++ input oracle and actual window events described above.

The workspace repository metadata also used a stale organization URL. It now
matches the configured Git remote and the installer/update repository.

## Final installed distribution check

The current Linux release build completed in 1 minute 57 seconds using the
existing dependency cache. Its archive was about 181 MiB and contained the
scientific runtimes, browser assets, and third-party notices. Installation into
an isolated directory, checksum-verified self-upgrade, bundled PDB2PQR/FFmpeg,
MCP tool discovery, and a 128 x 128 molecular PNG all succeeded. The installed
executable matched the archive byte for byte after self-upgrade.

From an empty working directory, the installed binary served its embedded viewer.
All 21 browser tests passed there, covering WebGPU presentation, molecular edits,
trajectory seeking, exports, numerical kernels, AI mock-provider interaction,
input settings, and cancellable rendering. A final macOS GUI build with the same
browser assets also passed. These were local checks; release publication and
GitHub Pages execution had not yet been authorized at this checkpoint.


## Viewport navigation and selection audit

The follow-up audit added the reference click-to-label behavior, residue toggles,
Shift residue ranges, lasso selection, distance/angle/torsion picking, bond edits,
context selection, hover feedback, tool shortcuts and a persisted tool palette.
Native and browser interfaces execute the same shared commands. Picking excludes
atoms outside visible renderer selections. Insertion codes and alternate-location
identifiers survive label and measurement references. Browser annotations now
include atom names, measurement values and simple-mode measurement lines.

The shared workspace passed 1,155 tests. Native GUI suites passed 164 tests on
Linux and Apple Silicon, and all 26 browser tests passed. The added browser tests
exercise click/drag distinctions, undo, cancellation, every tool shortcut,
hover state, cursors and palette persistence. A closed lasso path is treated as
a drag even when its endpoint returns to the starting point. A native layout regression checks
that new log lines cannot resize the viewport during successive atom clicks.
Selection undo/redo preserves the current view instead of automatically fitting
the molecule; camera-changing commands retain their explicit camera history.

The README's previous double-click-to-center mapping disagreed with the reference
navigation tool and was corrected: double-click toggles a residue, and centering
is available in the atom context menu. Generated fixtures and screenshots remain
in the ignored cache. These checks do not execute public release or Pages jobs.

An actual Linux window was exercised with pointer and keyboard events. Atom
labels, residue double-clicks, Shift range extension, all three measurement
modes, closed lasso selection, bond creation and undo passed. The camera remained
unchanged throughout these edits, and the final annotated viewport was inspected.

The measurement audit replaced cached cross-molecule endpoints with live atom
references. Translation, scaling, coordinate updates, object renaming, deletion,
and embedded QSC reload are covered. Scripted measurements and contact lines use
the same resolver, and transforms are applied once to both geometry and labels.
The native viewport now paints measurement values as well as atom names. Hover
picking is refreshed after camera changes even when the pointer stays still.

After these changes, the shared workspace passed 1,157 tests, the scripting-enabled
navigation suite passed nine tests, native suites passed 164 tests on Linux and
macOS, and all 27 browser tests passed. The added browser case saves and reloads
a cross-molecule measurement, then changes its transform again and checks the
visible value. Earlier documentation of atom-only native annotations did not
cover measurement values; the implementation and its comments now include both.

Picking now follows each visible renderer's selection and display transform.
Backbone renderers expose their residue pivots rather than hidden side-chain
atoms, and hover markers use the same projected positions. Both interfaces
exclude points outside the depth slab; overlapping projected points resolve to
the nearer depth. The new browser regression checks hidden side chains, renderer
translation, hover placement and depth clipping.

The reference's `MainChainRenderer::findPivotAtom` falls back to the residue's
normal pivot when a custom name is absent. Two older Rust tests and their comments
incorrectly required empty geometry in that case. They now check the fallback
coordinates and mesh. Running the C++ oracle on the reference protein produced
identical positions and normals for all 8,166 vertices with the default pivot
and with an absent custom pivot.

The resulting shared suite passed 1,158 tests, the scripting-enabled navigation
suite passed ten, native GUI suites passed 164 on Linux and macOS, and all 28
browser tests passed. The actual Linux window checks also passed again; distance,
angle and torsion values were visible alongside atom labels in the inspected
viewport. These are local checks and do not establish untested parameter
combinations or public distribution availability.

## Published winit dependency

The vendored winit source and workspace patch were removed. The lockfile now
resolves the unmodified crates.io 0.30.13 archive. The native suite passed 165
tests on Linux and Apple Silicon. An X11 window received real button events and
confirmed a single wheel step from zoom 50 to 38, unchanged camera distance,
rotation, translation, the drag threshold and both Shift-drag axis locks.
The input filter also checks repeated steps, both axes, raw valuator motion and
state reset without depending on an elapsed-time debounce.

## Shared viewport appearance

The native viewport now uses the browser's persistent molecular GPU pipelines.
Lighting follows the reference eye-space Blinn model: ambient 0.4, diffuse 0.8,
specular 0.16 and shininess 32. Mesh colors are lit per vertex; sphere and cylinder
impostors are lit per fragment. Fog blends into the scene background. Screen-space
lines, stipples, points and displaced lines use the depth buffer. Opaque renderer
sections precede transparent sections. Display and picking use their distinct
reference far planes, and camera exports preserve the scene background.

`scripts/check-appearance.py` runs the read-only reference GLSL shaders against
geometry exported by the Rust fixture test. All 18 Linux fixtures matched every
RGBA byte, including ribbon and cartoon protein meshes, camera rotations,
perspective, tilted cylinders, fog and stippled lines. The Apple Silicon Metal
fixtures passed the stated comparison tolerance. The comparison checks that
reference images contain foreground pixels; two empty images cannot pass.
This isolates shader equivalence for the same geometry. Backbone geometry has
separate C++ comparisons above; these images do not establish every renderer or
parameter combination.

The GPU-enabled shared core passed 138 unit tests and two pixel tests. The pixel
tests also cover transparent compositing, line occlusion, silhouette depth and
point displacement. Both native suites passed 165 tests. The scripting surface
fixture now explicitly tests SolidColoring fallback and a subsequent CPK color
change; its previous gray expectation contradicted the reference coloring
cascade. The lightweight-scene fixture now uses the actual `simple` renderer
name instead of the unregistered `line` name.

The full scripting audit also exposed differences in the reference class registry
that are outside these appearance changes. The appearance comparison is not a
full-feature completion claim; subsequent post-processing work is described below.

All 28 browser cases passed after the shared pipeline change. A real Linux window
also passed pointer/keyboard checks for labels, residue ranges, measurements,
closed lasso selection, bond editing and undo. The inspected viewport showed all
three measurement values. The 138 core unit tests and both pixel tests also ran
successfully on Metal after installing the required reference fixtures.

## Shared viewport post-processing

The native and browser viewports execute the reference GTAO, edge-aware denoise,
fog-aware AO composition, FXAA and three SMAA passes on the GPU. SMAA uses the
original area/search tables; the corresponding MIT notices accompany native and
web distributions. Temporal samples accumulate in a floating-point target and
reset on camera, geometry, size, background or setting changes. Cached frames
can be presented without restarting accumulation. Half-resolution AO is used
for a changed frame and replaced at full resolution on the next stationary frame.
PNG exporters use full-resolution AO without spatial post-AA, retaining alpha.

`scripts/check-effects.py` compiles the original read-only GLSL passes and
compares 13 contact-scene fixtures: orthographic/perspective AO, fog, half/full
resolution, FXAA, SMAA, its threshold, and temporal accumulation. Linux Vulkan
and Apple Silicon Metal outputs passed a mean channel error of at most 0.25
on the 0–255 scale and at most 16 pixels differing by more than two channel
levels. These bounds include rasterization and floating-point differences;
they do not claim that every pixel matches on every adapter. Test scenes avoid
an exact sphere-silhouette/pixel-center coincidence, whose single raw coverage
difference otherwise propagates through neighboring AO samples.

GPU regressions also verify visible contact darkening, distinct AA outputs,
half-resolution refinement, cached accumulation, camera-change resets, and
unchanged transparent-export alpha. The shared core passed 140 unit tests on
Linux and Metal, and both native unit suites passed 165 tests. Command and
scripting tests cover property discovery, invalid-value rejection, defaults,
reset, and QSC persistence, including `aaSmaaThreshold`.

All 30 browser tests passed, including separate WebGPU and WebGL2 checks for
AO darkening, both spatial AA methods, full-resolution refinement and temporal
history reset. A real Linux window passed the same setting changes through MCP;
6,796 foreground pixels darkened in the AO contact fixture. The native smoke
waits for the displayed GPU result after asynchronous commands. Both final
screenshots were inspected. WebGL2 uses packed 24-bit color depth for post-process
reads, avoiding unsupported depth-texture loads and shadow-sampler compilation.
The 18 raw appearance fixtures still matched every Linux RGBA byte.

## Tauri viewport integration

The optional Tauri shell now reads back the same molecular GPU pipeline, including
AO, spatial AA and a converged temporal average. The worker reads pixels only
once after convergence. It retains the scene primitives instead of tessellating
all atoms and cylinders into an extra CPU mesh. Screen lines stay in the scene
snapshot and depth-test with the molecule. Labels now use the shared GPU pass
described below.
Scene editing includes the SMAA threshold. Point and rectangle picking use the
reference picking slab rather than the farther display plane.

With reference fixtures present, all 143 Tauri tests passed on Apple Silicon,
including actual GPU AO/AA and the previously ignored animation image exporter.
The Linux shared pixel test confirms that live-view readback exactly matches
AO, SMAA and temporal-accumulation output; the same regression passed on Metal.
The native egui suite passed 165 tests and the shell contract suite passed 69.
The Linux Tauri build could not run because the host lacks Pango/GTK development
metadata; this is not evidence of a Linux Tauri runtime test.

Three old Tauri expectations were corrected after checking current behavior:
Tritium's DefaultNucl section width is 0.4 (the old XUL style says 0.5), the
default ribbon exposes its C-alpha pivots for picking, and contact annotations
retain live atom references instead of fixed positions. The class-registry audit
remains separate follow-up work.


## Scene application data and direct molecular surfaces

Scene application data now stores the reference RenderSettings classes with
live scripting properties, typed values, nested defaults, reset, undo and QSC
serialization. Existing command settings migrate into the same data. Unknown
application-data classes survive a read/write cycle. Focused shared tests cover
invalid input, default resets, legacy-setting precedence, removal and undo.

The tracestick representation follows the reference main-chain pivots and
visibility rules. Direct molecular surfaces now select distance fields,
EDTSurf or MeshMS, using reference detail-to-density conversion and atom ownership.
The EDTSurf sources and license are included in native and browser kernel builds.
Saved molecular surfaces retain the separate MeshMS/BALL/Rust backend interface.

The C++ surface oracle compares 27 combinations of engine, surface type and
detail on selected 1CRN residues. Vertex coordinates, normals and triangle
indices matched on Linux and Apple Silicon. The comparison initializes an
otherwise uninitialized EDTSurf smoothing flag; it does not reproduce allocator-
dependent behavior. Equidistant MeshMS ownership uses the smallest atom ID within
16 double-precision machine epsilons. Differences from the reference are accepted
only where float-coordinate quantization cannot distinguish the distances, while
Linux and Apple Silicon ownership arrays must match exactly.

Browser command calls return an ID, and status exposes lastCommand after that
command is dispatched. Callers can wait for that ID and busy=false before reading
results; a fixed delay did not reliably distinguish queued from completed work.

The registry covered all 166 reference classes at this checkpoint. The wider
QIF audit then reported missing members/defaults in maps, labels, trajectory
loading and exporters; later sections record their implementation and validation.
Registry coverage alone does not establish their behavior.

All 32 browser tests passed after adding command acknowledgments, including
all three direct-surface engines and application-data persistence through script
edits and QSC reloads. The class-set test passed with the reference present;
13 command integration tests, 117 shared core unit tests and 165 native GUI unit
tests also passed. The latter two counts describe the selected feature sets,
not the separate QIF member/default audit completed later.

## Shared outline groups

Renderer `egroup` values now survive scripts, undo and embedded QSC reloads and
reach the Umbreon renderer. Named groups use their first enabled edge style;
unnamed groups share equal edge styles. Group membership is independent of
object names, materials and transparency. Silhouettes preserve outer contours,
while joined groups suppress their internal contact lines. Edge colors follow
the group style and widths retain the reference minimum of one output pixel.

The `Renderer.qif` comment describes unnamed groups as object-specific, but
`UmbreonDisplayContext.cpp` groups them by style. This implementation follows
the executable code. The 71 render tests include pixel comparisons for merged
contact lines and shared styles. The focused scripting roundtrip test, 13 command
integration tests and 165 native GUI tests passed with the reference present.


## Atom label formatting and depth

NameLabelRenderer implements renderer and per-atom formats, alignment,
`label_on_top`, default resets and `setLabel`. The parser validates built-in
field types when a format is assigned. Runtime custom-property type errors
produce the reference placeholder, and missing properties produce empty text.
The default label includes alternate conformations and residue insertion codes.
QSC uses the reference `str` literal attribute and reads older `text` attributes.

The C++ oracle compared 651 field, Unicode, padding, integer, floating-point and
invalid-format cases with the read-only reference. Shared script tests cover
invalid assignments, per-atom overrides, reset and embedded scene roundtrips.
The shared GPU label pass uses cached glyph textures and compares each fragment
with packed scene depth. Native, browser, Tauri and camera image rendering share
that pass; explicit on-top labels bypass only the depth comparison. Pixel tests
verify partial occlusion, overlay display and alignment on Metal. At this earlier
checkpoint, the rasterizer used only bundled proportional and monospaced fonts
with synthetic bold and italic. Installed font selection and browser glyph
comparisons are recorded in the later font-family section above.

All 33 browser tests passed, including label depth, overlay and displacement.
The native GUI suite passed 165 tests. The Tauri suite passed 142 tests with one
optional fixture-dependent test ignored in this run. A real Linux window showed
no magenta label pixels behind the sphere, 171 when on-top was enabled and 220
after alignment and displacement changes. The final screenshots were inspected.


## Measurement selections and renderer identity

AtomIntr selection calls now validate nonempty selections before storing a
measurement and use their centroids for distances, angles and torsions.
`getValue` returns live world-space values and rejects missing or removed IDs.
Literal vector measurements retain their world coordinates when their owning
molecule is transformed. Shared tests cover these behaviors, coordinate edits,
invalid selections, detached vector measurements and QSC reloads.

Queued renderer edits now retain the renderer ID for properties, visibility,
reset and nested coloring. Previously, an unnamed CPK renderer could redirect
its selection to the unnamed selection renderer. Detached property changes no
longer edit a scene renderer with an empty name. QSC import also retains unnamed
renderers and assigns nested properties to each duplicate in document order.
The regression stores two unnamed CPK renderers with distinct selections,
coloring and visibility, then verifies that the displayed atom survives reload.
A real native window now shows only the selected CPK sphere; its saved renderer
contains `sel="aid 1"` while the selection overlay does not.

The shared core passed 196 tests on Linux and Apple Silicon, excluding the seven
separate registry audit tests. The native GUI passed 165 tests. The registry
member/default audit was still incomplete for maps, trajectory readers and
exporters at this checkpoint; later sections cover those gaps.
Nine QSC compatibility tests passed with the C++ reference fixtures present.
All 33 existing browser tests passed; after the final QSC fix, five save/load
and label tests passed, including the new unnamed-renderer and measurement API
regression. The unchanged tests were not repeated after that focused correction.

## Camera ordering, fitting and surface gradients

Cameras retain their UI order when replaced, append after the current maximum
when no order is supplied, and serialize in display order. Live camera
references now update subsequent queries within the same script. Replacement and
deletion detach old references so later edits cannot resurrect a camera.
Cloned camera values retain their source and visibility settings.

Molecule fitting uses transformed coordinates in the current view, adds 20%
padding on each side and keeps a minimum extent of four world units. Empty
selections leave the view unchanged. Principal-axis orientation follows the
reference Jacobi eigensolver and handedness convention. The C++ oracle compared
136 cases, including rotated clouds, empty selections and degenerate point sets;
centers, zoom and slab agree within 1e-7 and rotation dot products within 1e-8.
CLI centering now uses the selected centroid with the object transform, and
CLI fitting uses the same view calculation.

MultiGradient supports atomic JSON replacement, duplicate-value handling,
color-expression preservation and live references. Its default factory now
returns white, matching the reference implementation. Direct-surface gradients
sample the named scalar map after the object-to-map transform, and support the
reference normal offset. Setting a gradient before attaching a renderer, changing
it through a second reference, undoing a map change and reloading embedded QSC
all retain the displayed vertex colors. Pending and attached renderers now share
one property list; the separate lists previously omitted direct-surface gradients
from existing-renderer proxies.

The shared core passed 200 tests, Qsys passed 130, command integration passed
14 and the native GUI passed 165. These historical counts exclude the separate
registry audit for map controls, trajectory scripting, hover and exporters,
which was completed later.
All 36 browser tests passed, including camera order and orientation, live
surface-gradient references, invalid JSON rollback and scene reloads.

## Trajectory scripting and scene persistence

Trajectory script objects now use the same indexed sources as native playback.
The DCD, XTC and TRR reader defaults enable lazy loading, and explicit eager
loading remains available. Load selections retain file-order indices even when
atom IDs are sparse. Frame averaging, block reorder/removal and transactional
Undo update the same molecule shown by the viewport.

Linked QSC files retain relative trajectory paths and load successfully after
moving the scene and its source files together. Embedded QSC files store sampled,
selected frames as binary DCD and keep cell lengths and angles. The QSC writer
now preserves arbitrary binary extension chunks; its string method is only a
text inspection view. Topologies read through GRO, PSF and Amber readers use the
same scene loader callback as native file loading.

Browser scene downloads join the scene bytes with the original trajectory File
as Blob parts. Saving therefore does not decode all frames or copy the trajectory
through a JavaScript coordinate array. Reloaded embedded trajectories index their
encoded bytes and decode individual requested frames; loading an embedded QSC
still requires its encoded bytes in memory. Native file-backed sources continue
to seek the original file, and undo snapshots share their sources.

DCD cells are normalized to `(a,b,c,alpha,beta,gamma)` in angstroms and degrees.
The earlier reader exposed the raw DCD order while playback expected this
normalized order. Tests now cover both byte orders, angle cosines and degree
angles using the conventions in the
[VMD DCD reader](https://www.ks.uiuc.edu/Research/vmd/plugins/doxygen/dcdplugin_8c-source.html).
The reference CueMol reader copies raw DCD cell values; this fix makes the Rust
reader consistent with its XTC, TRR and NetCDF readers instead of copying that
inconsistency.

The shared application suite passed 202 tests, excluding the seven separate QIF
audits, and the native GUI suite passed 165 tests. The full molecule and MD reader
suites also passed with the reference checkout available. The exhaustive API
audit still required map controls, alignment methods, hover and exporter work
at this checkpoint; the later sections record their completion.
All 36 browser tests passed, including saving a File-backed DCD, reloading its
QSC after a page reload, seeking the final frame and reading its coordinates
through the script API. The command compatibility suite passed all 14 tests.

## Alignment service parity

`MolAnlManager.superposeUSalign1`, `superposeUSalign2` and
`superposeUSalignJSON` now use the shared native/browser US-align kernel.
Statistics include reference- and moving-normalized TM scores, RMSD, aligned
residue count, sequence identity and the nucleic-acid flag. Automatic pivot
selection chooses CA or C3-prime per residue, recognizes modified nucleotides
from their sugar backbone, and omits alternate conformations other than A.

Alignment now uses coordinates after each object's existing transform. Applying
an alignment first bakes the moving object's old transform, then either writes
the alignment as the new transform property or applies it to the coordinates.
The same correction covers LSQ and SSM. Their script adapters now update live
molecule queries, and `useprop=true` performs the requested transform.

Regression tests cover mixed protein/nucleic selections, duplicate pivot
conformations, statistics, existing object transforms, both application modes,
and undo between script calls. All 204 shared application tests, 165 native GUI
tests, 14 command compatibility tests and the molecule-analysis suite passed.
The browser US-align test also passed with the updated kernel and all three
script methods. These tests exposed a separate issue: queries immediately after
Undo within one script evaluation still see the prior environment. The shared
script transaction correction below resolves that case.


## Live script transactions

Script transaction boundaries now synchronize the working scene before capturing
history or applying Undo, Redo and Rollback. Queries through existing atom,
object, renderer, scene and application-data handles observe the restored state
within the same evaluation. Edits preceding `startUndoTxn` remain outside the
transaction, and multiple and nested transactions retain their own boundaries.
Uncaught script errors discard the evaluation's scene changes and history.

History stores the receiver scene and applies the properties changed by that
transaction. It preserves other scenes, unrelated objects and later changes to
other atom, renderer, view and render-setting properties. Recreated objects keep
their IDs, while new script objects use a monotonic allocation counter. Camera
replacements detach old camera references only in the restored scene.
Indexed trajectory sources remain shared; immutable history snapshots also use
shared ownership instead of being copied with each script environment.

Five new application regressions cover immediate queries, transformations,
cameras, app data, preceding edits, multiple/nested transactions, object and
renderer deletion, unrelated changes, detached objects and error atomicity.
All 209 shared application tests passed on Linux and macOS, excluding the seven
separate QIF audits. The molecule, scene-system and core suites passed on Linux
with the reference checkout available. Earlier comments describing the script
environment as read-only were inconsistent with its mutation APIs and have been
corrected. The reference's oversized `undo(n)` behavior is preserved: it applies
available entries and returns false when the requested count exceeds history.

All 165 native GUI tests, 14 command compatibility tests and 37 browser tests
passed. The browser regression checks immediate coordinate queries across two
transactions, Rollback, a second scene and a later script evaluation.

## Embedded trajectory storage sharing

QSC extension payloads now use immutable shared byte buffers. The embedded
trajectory index reads the same buffer that the molecule retains for scene
saving. Cloning a scene or recording Undo therefore does not copy the encoded
DCD payload in addition to sharing its frame index. Morph extension payloads
use the same storage, while the legacy byte-vector setter remains available.

Allocation-identity regressions cover the indexed reader, a binary QSC round
trip, application cloning and script Undo, followed by another frame seek.
The full molecule and MD suites passed with the reference checkout present;
209 application tests and 165 native GUI tests passed. Four browser tests
covering trajectory playback, embedded scene export and script transactions
also passed after rebuilding WASM.

## Streamed CCP4 and MRC maps

The shared reader now probes the fixed header before allocating voxel storage,
checks `max_voxels` after strict grid subsampling, and decodes selected rows into
one byte per stored voxel. It supports modes 0/1/2/6/12, IMOD signed bytes, both
byte orders, axis permutations, MRC origin precedence, and header-based map-kind
detection. Native gzip/bzip2/xz input is streamed into a temporary seekable file;
invalid or stale header statistics can therefore be remeasured without retaining
whole-map floating-point arrays. Compressed browser bytes require a decoded input
buffer; the reader then uses the same row-based quantizer.

`CCP4MapReader.probeHeader`, `subsample`, `max_voxels` and `use_header_stats` are
available to scripts. Both desktop file dialogs expose the reader settings.
Truncation retains real-valued thresholds, and zero header RMS skips normalization
as in the C++ implementation. The earlier Rust test expected an error instead.

The read-only C++ oracle compared 80 cases covering all supported modes, byte
orders, permutations, subsampling, transforms, map-kind/origin evidence, and stale
header statistics. Every stored byte, measured statistic, quantization parameter,
and tested grid coordinate matched. Reproduce with
`scripts/check-reference-ccp4.py --build-dir <reference-build> --config <sysconfig>`
using `uv run --no-project python`.

Two reference limitations are intentionally avoided. Nonseekable compressed C++
input clips samples outside stale header ranges; native Rust spools the decoded
input so those ranges can be repaired. Also, the reference QDF map reader derives
quantization from measured minimum/maximum, which loses a distinct header-derived
range. Optional `qbase`/`qstep` fields preserve that range in Rust QDF/QSC round
trips; older reference readers ignore these fields and retain their old behavior.
Old QDF headers remain readable. Binary map data sharing and the map
renderer/scalar scripting audit were completed in the following stages.

The shared core passed 209 tests and native egui passed 165 tests after this
change. The rebuilt browser passed the existing embedded-scene workflow and a
new half-float MRC/QSC round-trip regression. The latter also compares the
resulting density histogram. Linux could not check the optional Tauri shell
because its JavaScriptCore GTK system library was unavailable; that shell is
checked separately on Apple Silicon.
The Apple Silicon Tauri suite passed 142 tests with one optional test ignored.

## Shared density payloads

Density-map scene records now share immutable QDF/source bytes across native and
script undo snapshots. Binary payloads are no longer also converted into lossy
UTF-8 strings. A regression checks the payload and decoded-voxel allocation
identities across four script transactions, undo, a native state clone and an
embedded scene reload. The shared core passed 210 tests and native egui passed
165 tests. Two rebuilt-browser regressions covered transactions and MRC/QSC
persistence. Other raw surface/lightweight payload types were not changed here.

### Scalar-map indices and skew cells

Scalar access now uses local stored-grid indices, matching the C++ `atFloat`,
`atByte`, and boundary APIs. File starts are applied only in the coordinate
transform. Cartesian sampling uses the inverse of the full cell basis, and
surface gradients use its inverse transpose. This corrects the former
orthogonal-cell assumption for nonzero starts and skew crystallographic cells.
Density histograms use their stored byte counts and quantization range while
reporting the measured minimum and maximum.

The CCP4 oracle passed 80 cases including local sampling and quantization steps.
Shared-core tests passed 210 cases and native GUI tests passed 165 cases.
A skew-cell regression checks coordinate sampling, gradient components, and
histogram counts independently of the reference reader.

### Continuous potentials and live scalar-map properties

OpenDX and QDF potentials now retain immutable, shared float32 samples through
native loading, in-memory loading, script readers, rendering and embedded scene
export. Their `den_quant_step` is zero. Display-only byte samples do not replace
the original values. Potential sampling uses trilinear interpolation and the
C++ boundary convention. Saving uses `qdfpot`; the former shared GUI path
converted potentials to byte density maps and lost their `ElePotMap` class when
loading browser files. Both faults are corrected.

The density API now exposes read-only `origin`, `den_quant_step`, and
`map_type_resolved`, plus `getLevelAtTopFraction` and writable `map_type`.
Detached handles continue to observe the attached object's state. Changes
survive undo/redo and QSC round trips; invalid map kinds are rejected.
The scalar histogram properties are live read-only getters for both detached
and attached objects, rather than loader-updated snapshots.

`check-reference-scalars.py` compares float samples, byte conversion,
interpolation, statistics and histograms against a separately compiled C++
`ElePotMap`. Cases cover variable values, uniform values, a wide-range outlier,
exact binary fractions and very small nonzero values. For the uniform nonzero
case, the reference computes a slightly negative variance and returns NaN for
RMS (and invalid JSON for that histogram field). Rust deliberately clamps the
variance to zero. The comparison records this single exception explicitly.
The small-value case also catches the former absolute epsilon that collapsed
valid byte-conversion scales to zero.

Native/shared regressions cover potential identity, exact stored values,
interpolation, shared memory, held handles, map-kind validation, undo and
embedded QSC reload. Browser tests cover potential display and export/reload,
and CCP4 map-kind/origin/quantization/top-fraction metadata.

Validation passed: five C++ oracle cases, 211 shared-core tests, 165 native
GUI tests, and three rebuilt-browser tests (structure/scene export, CCP4
metadata, and potential display/scene export). These checks establish the
scalar-data changes; the later integrated audit covers the complete QIF surface.

### Map levels and default-state persistence

The shared map renderer now resolves crystallographic sigma levels, EM top-percent
levels, absolute levels, and the four UI bounds using the reference formulas.
The EM occupancy cap applies only while the native level remains at its default.
An explicit assignment of `1.1` therefore differs from the initial `1.1`; reset
and both external and embedded QSC storage retain that distinction. Legacy
`minLevel` and `maxLevel` remain sigma ratios even for EM maps, as in the C++
implementation. Uniform byte maps retain the reference quantization step of one.

Native and Tauri density controls use the resolved native/absolute level and UI
bounds. Editing color or transparency leaves the default flag intact. Absolute
Tauri edits use the shared conversion instead of dividing by RMS for all maps.
Both changes are covered by GUI regressions. The browser regression loads a
sparse MRC map, explicitly overrides its default, reloads the embedded scene,
and restores the original threshold through reset.

The scalar oracle now also compiles `reference_map_levels.cpp`. It compares 25
states across crystallographic, sparse EM, continuous potential, uniform EM,
and tiny-value maps. All states passed, as did 212 shared-core tests, 166 native
GUI tests and 143 Tauri tests (one optional test ignored). Rebuilt-browser CCP4,
potential and sparse-EM regressions passed. These results do not establish
complete map-renderer parity; region/LOD and volume rendering remain in the
ongoing implementation audit.

### Volume rendering

`gpu_mapvol` previously entered the isosurface branch in the shared GUI core.
It now produces a scalar volume, with immutable byte data shared when the full
grid is displayed. The GPU samples a 3D texture and the reference transfer table
on view-aligned slices. Slices from overlapping volumes are sorted together;
opaque geometry provides per-fragment depth rejection. Alpha and object/renderer
transforms are applied without replacing the underlying map samples. Transfer
edits update the small color texture while retaining the density texture.

CPU image rendering samples the same texture-center interpolation and transfer
table, then composites the samples with molecular intersections. Native and
browser viewports, Tauri image composition, camera-based PNG output and the
matrix-based scene image helpers include volumes. Volume samples are not opaque
surfaces and do not write the geometry depth map. Mesh-only exchange formats
continue to contain geometric primitives.

`check-reference-volume.py` extracts the unchanged `genXferFunMap` body from the
read-only reference source, captures its texture upload, and links the original
HSB conversion. All 256 RGBA entries matched for 15 combinations of mode and
threshold. The reference exposes `rainbow1`, `auto2`, and `multigrad` names, but
its implementation produces the same rainbow table for all three. Rust follows
that actual behavior; the names do not imply a separate custom gradient.

Actual Metal image tests compare CPU and GPU center pixels within five 8-bit
steps for opacity, interpolation, overlapping grids, rotated perspective views,
opaque occlusion, slab clipping, postprocessing and matrix-based image output.
Both WebGL and WebGPU browser tests check visible color, zero-opacity removal,
and embedded scene reload; the resulting WebGPU image was inspected. Shared
regressions verify scene persistence and shared allocations after level edits.

Validation passed: 72 render tests, 61 map-domain tests, 213 shared-core tests,
166 native GUI tests, 143 Tauri tests (one optional test ignored), the transfer
oracle, two actual Metal tests, and four browser tests covering both volume
backends plus existing molecule and label display. Browser texture dimensions
respect the adapter limit. Volume extraction is bounded to 16 Mi samples and
4096 view slices, so very large maps may be sampled more coarsely than the
uncapped reference. Region/LOD controls are the next map audit item.

### Map regions, LOD and view updates

The map renderers now apply `region_mode`, `lod`, `lod_budget`, and
`zoom_refine` to actual mesh/volume extraction. `auto` selects full-block display
for EM and the historical centered box for other scalar maps. Full regions
suppress periodic wrapping, ignore the old 100-node box limits, and align sample
nodes to block-relative stride multiples. Automatic sampling uses the smallest
power-of-two stride within the cell budget (16 Mi cells, or 2 Mi for the CPU
contour renderer). Explicit strides are 1, 2, 4 and 8. The separate volume memory
and adapter limits described above still apply.

Native, browser and Tauri navigation share a 150 ms debounce and the reference
1.5 view-box padding. Smaller pans retain geometry; zooming can select a finer
stride, and a view leaving the map retains its last useful region. Map centers
follow the automatic/drag-update controls, including the box mode. Boundary
molecule coordinates are converted through their object transform into the map
frame. Display properties retain their values through scripting, reset and QSC
storage, and the desktop property dialogs expose the region and LOD controls.

The C++ scalar comparison now also runs `reference_map_regions.cpp`: all 21
regions and 15 budget decisions match, including skewed cells, nonzero map
origin/start, object scaling/rotation, missed view boxes, explicit stride and
disabled refinement. These comparisons cover the full-region policy. The Rust
box path uses all eight transformed box corners as well; the legacy C++ box
path only transforms the center and two opposing corners. This deliberate
geometric correction can enlarge a box region in a skewed or transformed grid.
The older transform regression now fixes the region to `full` and uses a broad
molecular boundary so it tests geometry transforms independently of cropping.

A browser scene-save defect was found during the region round trip: updating
the live view did not create the scene's `__current` camera. Saving now records
that camera, and the browser test checks both restored view values and visible
geometry. A separate Tauri defect treated volume-only snapshots as empty;
volume primitives now participate in the image-availability checks. The actual
Metal test returns a PNG and checks colored volume pixels without any mesh.

Validation passed: the C++ region/LOD comparison, 215 shared-core tests, all map
crate tests with the reference checkout available, 166 native GUI tests, 143
Tauri tests, the explicit Tauri Metal volume test, and five rebuilt-browser
tests covering scene/camera storage, sparse EM levels, region/LOD edits and both
volume backends. Two Tauri tests require an explicit GPU run; the volume test
was run successfully, while the pre-existing animation export test remains
outside this phase. The broader QIF audit still tracks surface cap/coloring,
hover state and exporter settings separately.

### Map surface caps and molecular coloring

Map surfaces now distinguish the display mesh from `generateSurfObj`.
`cap_mode=auto` caps full-region displays and generated objects; a box display
stays open. `on` and `off` override this policy. Caps use the reference face
patterns and skip faces whose corners leave the molecular boundary. World-space
normals remain outward for skewed and reflected grids. Generated surface objects
bake the source map transform, including inverse-transpose normals and reflected
winding. Boundary and nearest-color searches build spatial indices rather than
scanning every atom for every sample or vertex.

The scalar comparison adds `reference_map_caps.cpp`. For all 256 corner
configurations, the number and area of cap triangles match C++. The two saddle
face patterns (5 and 10) intentionally produce no cap triangles in the reference
and here. Consequently, enabling caps does not guarantee a closed mesh for
those patterns. The reference table header describes generation only, but the
actual renderer also uses it for full-region display and `cap_mode=on`. The
Rust map documentation also incorrectly described tetrahedra after the mesher
had changed to marching cubes; that comment now names the actual algorithm.

`MapSurfRenderer` now defaults to neutral gray (`#d9d9d9`). Its `target`, `sel`
and `coloring` drive actual nearest-atom coloring, including object transforms,
scene styles, paint/CPK/rainbow schemes and inherited molecular colors. Map
renderer children are retained through script handles, edits and QSC storage.
Gradient lookup also respects both maps' object transforms. Desktop property
dialogs expose the cap policy and coloring target/selection; the native and
Tauri coloring panels can edit molecular schemes for map surfaces.

Validation passed: 216 shared-core tests, all map-domain tests with the reference
checkout available, 166 native GUI tests plus the new native coloring/undo
regression, and 143 Tauri tests plus the new Tauri coloring/undo regression
(two explicit GPU tests skipped). Four rebuilt-browser tests cover direct
surface gradients, sparse EM thresholds, map regions and the new cap/coloring
round trip. The new browser test checks both molecular colors, cap removal,
selection changes and embedded reload, and its resulting image was inspected.
A further shared regression checks generated surface coordinates, normals and
winding under reflection, scaling and translation. Hover state, molecular SES
backend properties and exporter settings remain in the separate QIF audit.


### Scripted SES backend selection

`MolSurfObj.sesbackend` now controls the shared MeshMS/BALL generation path for
molecule inputs, sphere arrays and regeneration. New objects default to `auto`;
the public script enum accepts `auto`, `meshms` and `ball`. Native command APIs
retain their additional `rust` option. Changing the requested backend leaves
existing geometry intact until generation is requested, and failed generation
preserves the previous mesh and provenance. Scene saves retain both the
requested backend and the backend that produced the current mesh.

The audit found two execution paths that contradicted the earlier backend
summary: scripted creation and regeneration still used the Rust grid builder,
and newly attached scripted surfaces could lose their original generation
conditions at the end of a script. These now use the common backend and save
the complete surface state after the object has been created. The old builder
comment also described a solvent-accessible marching-tetrahedra surface even
though it generated a solvent-excluded marching-cubes surface; that obsolete
helper and comment were removed.

Validation passed all surface-domain tests with the reference checkout present,
217 shared-core tests, 167 native GUI tests and 144 Tauri tests (two explicit GPU
tests skipped). The new domain regression checks backend changes, sphere-array
routing and error preservation. The scripting regression compares generated
meshes before and after changing backend, embedded reload and property reset.
Three rebuilt-browser tests passed for command-driven surface generation,
live surface gradients and scripted backend selection followed by reload and
regeneration. No public workflows or releases were run. Hover state and exporter
settings remain in the separate QIF audit.


### GPU picking, atom ownership and transient hover state

Native, WebGL, WebGPU and Tauri views now share the GPU ID pass and the reference
three-pass hover effect. The ID pass reuses sphere/cylinder fragment depth,
uses mesh atom ownership and a depth buffer, and renders at half the viewport
resolution. Cached GPU geometry and asynchronous image readback are invalidated
by scene, camera or viewport changes. A target resize also invalidates the hover
mask bindings, including a resize away from and back to the original dimensions.
Picking and hover do not rebuild the scene on each pointer move.

The GPU policy matches the reference: locked objects/renderers, sections with
opacity at or below 0.6 and auxiliary ring plates are excluded. Unsupported
renderers, including symmetry, use the renderer-aware CPU fallback. Disabling
GPU picking or enabling stereo uses the ordinary CPU projection and display
slab; GPU fallback queries use the GPU pass's fog clipping. Volume rendering
is outside the GPU ID pass. Script `View.hitTest` uses the current view's image,
and `setHoverHit`/`clearHoverHit` change transient presentation state. The input
service's color, GPU switch and property reset work through live script handles.

Mesh ownership now follows the generating renderer. Ribbon/tube triangles use
the C++ provoking vertex and spline parameter boundaries, including unselected
neighbors named by partial-selection caps. Nucleic bases retain the owning
residue pivot even when another pivot is closer; separately colored detail
bonds retain their actual atom IDs. Surface ownership uses the mesher's atom
serial, and ellipsoids retain their source atom. Color-split bond halves keep
one endpoint owner each. The CPU backbone candidates remain the selected pivots;
extra GPU owner atoms do not expand rectangle or lasso selection.

The independent C++ backbone oracle now emits atom ownership for each triangle.
All 48 geometry/winding cases also pass the ownership comparison, including
partial selections, caps, putty and secondary-structure junctions. The portable
`scripts/check-reference-backbone.py` runner builds the oracle against the
read-only reference checkout and runs these comparisons. Two explicit Metal
tests pass for sphere edges, front/back occlusion, color fill, resize caching,
bond halves, mesh ownership, auxiliary triangles and the opacity threshold.
The explicit Tauri Metal test also returns a visible hover image. The shared
script integration test verifies view identity, hit results, set/clear behavior
and service reset.

The native GUI tests pass (167); the Tauri suite passes (144, with three GPU
checks excluded from the ordinary run). The hover check was then run explicitly
on Metal. Shared-core tests pass with both scripting alone (217) and the
scripting/offscreen feature combination (241). All 74 molecular-renderer unit
tests pass. Browser navigation covers both rendering backends, configured hover
colors, opacity and lock changes, script set/clear, transformed backbones and
nucleic-base ownership. Pixel checks compare the configured 0.35-alpha fill to
the unhighlighted image, preserving the underlying molecular color.

A browser context-menu race was also corrected: invisible sizing passes no
longer publish button rectangles to the UI test interface. Previously a click
could use the preceding menu's coordinates and toggle a label instead of
centering. Remaining exporter declarations are tracked by the QIF audit; this
phase does not claim that the full reference API has already passed.

### Exporter settings, adaptive sampling and complete QIF declaration coverage

The Umbreon exporter now exposes the remaining 28 properties and three methods
through live typed bindings. `applyRenderSettings` uses the same settings mapper
as the command layer, retains the camera, converts physical dimensions with DPI,
and applies the reference AO/GI/NPR switches. Disabling AO retains its inactive
parameters. Non-GI lighting uses the reference ambient fraction, and NPR disables
GI. Native synchronous and asynchronous exports use the actual hatching renderer,
including independently selected paper and ink models, custom layers and tone
curves. PNG output includes physical pixel density. Invalid optional presets or
specifications fall back with diagnostics; `getRenderLog` drains these messages
and actual render timing instead of returning a placeholder.

Adaptive AA detects geometry, color, normal and depth boundaries and refines their
neighborhoods. Additional primary-ray probes preserve thin geometry. Reduced AO
gathers a separate grid, interpolates with depth/normal guidance and falls back to
per-hit AO when a sample is incompatible. GI uses grid AA and per-hit AO. Native
auxiliary buffers cover at most 128 output rows plus sampling margins; browser
tiles share global sample coordinates. Tests compare tiled and untiled output
exactly, including a 259-row image. This is the Rust BVH implementation; these
checks do not establish pixel identity with the external Embree integrator.

The seven reference-QIF audits now pass for all 166 registered classes, including
properties, defaults, methods, inheritance and script construction. These audits
check the declared API; individual behavior still requires the domain, script,
image and interaction checks recorded here. The complete shared scripting suite
passes 225 tests, native GUI passes 167, and Tauri passes 144 with three explicit
GPU checks excluded. Renderer tests pass 77 cases, including the independent C++
hatch comparisons, real PNG color/DPI output, settings retention, adaptive edges,
guided AO, buffer bounds and diagnostic draining. Command compatibility passes
14 cases. All 49 browser tests pass, including both GPU backends, OIDN, navigation,
trajectory access, maps, scripts, rendering cancellation and mock AI providers.

Browser image commands use the shared renderer and browser downloads. The script
exporter's native file thread API now returns a descriptive error on WASM instead
of attempting unsupported thread creation. Rebuilt browser checks exercise this
guard along with exporter settings, adaptive AA/AO, lighting and NPR output.
Public workflows and release installation had not executed at this checkpoint
because those changes had not yet been pushed.

### Partial spline selections

Spline geometry now derives its coefficients from the complete linked chain and
draws only the selected intervals. Previously it recomputed the curve using the
selected residues and omitted a selection containing exactly two residues.
An isolated selected residue and a whole chain of fewer than three residues still
produce no spline, matching C++. Pivot selection and end fading now reach the
line renderer, and smoothing uses the reference near-zero threshold.

The independent C++ oracle compares 45 cases covering full, partial, disjoint and
single-residue selections, three tessellation levels, smoothing, pivot changes,
stepped/interpolated colors, alpha fades and atom identities. Every line endpoint,
color and owner passes. The existing 48 ribbon/tube/cartoon geometry and ownership
cases also pass. The molecular-renderer suite passes 76 tests; shared scripting
passes 225 and native GUI passes 167. A browser pixel regression verifies that
two selected residues remain visible on the full-chain curve and that a single
selected residue disappears. An obsolete helper comment calling the natural
cubic interpolator Catmull-Rom was corrected to match its implementation.

### Multiple browser trajectory sources

The browser now retains ordered trajectory blocks for each molecule. Previously
one optional source was replaced whenever another trajectory was loaded, and
`set_frame` could resolve the wrong object through the active timeline. File
loads are queued in input order, including a topology followed by its trajectory
files. A block append retains the previous frame; each object has its own current
frame and timeline. Both ordinary commands and AI tool calls resolve global frame
indices into the correct block. Late frame reads are rejected after a scene edit.

History shares the `File` handles and indices, while playback still decodes one
frame at a time. QSC export returns fragments interleaved with every active
external file as Blob parts. Existing embedded blocks remain before appended
files, and distinct molecules keep distinct lists. Shared-core tests verify this
with three external payloads spanning two objects and an existing embedded block.
All 226 shared scripting tests and 167 native GUI tests pass.

Five browser integration checks pass for ordinary file loading, indexed playback,
AI cancellation and frame undo, and multiple sources. Three rebuilt-browser
checks also pass for MDCRD cells, AI seeking and the expanded multi-source case.
The latter loads two blocks with a topology in one file selection, appends a third,
undoes/redoes that append, seeks across boundaries for two molecules, downloads
and reloads the scene, then appends to the reloaded trajectory. A truncated input
leaves the existing blocks and coordinates intact, and the next valid file loads
successfully. Frame reset on QSC reload follows the reference's nonpersistent
`frame` property. Public release execution remains outside this local checkpoint.


### Affine molecular rendering and final integration checks

Object coordinates now receive their transform before molecular geometry is
constructed. CPK radii, bond thickness, backbone cross sections and anisotropic
thermal axes therefore retain their reference meanings. Backbone links are
resolved before coordinate changes, so scaling a molecule cannot split its
chain. The authoritative molecule is not mutated. Identity object transforms
avoid the temporary molecule copy; transformed copies are released after each
renderer is built.

Renderer transforms apply to complete glyphs. Similarity transforms preserve
sphere/cylinder primitives and scale their radii, while screen-space line widths
remain in pixels. Nonuniform scales and shear tessellate those primitives and
retain section materials, alpha, edge data and atom ownership. Normals use the
inverse transpose, with a cofactor fallback for singular transforms. Reflections
preserve face orientation. The primitive tessellator also had inward sphere and
cylinder faces; these now point outward, while outline faces point inward.

The independent C++ backbone oracle passes 60 mesh configurations and 57 spline
configurations, including nonuniform scaling, shear, reflection and rotation.
Position, normal, color and picking comparisons retain the previous tolerances.
Shared regressions verify object-versus-renderer scaling, unchanged thermal axes,
raw coordinate preservation, transformed picking metadata and material ranges.
WebGL and WebGPU checks verify circular object-scaled spheres, uniformly scaled
spheres, reflected ellipsoids, fragment hover, CPU rendering and scene reloads.

The complete workspace run with the native GUI feature passes 1,593 tests,
including 252 shared-core tests, 167 native GUI tests, 81 renderer tests and
76 molecular-renderer tests. Reference fixtures were available; optional external
oracles still require their explicit invocations. All seven QIF audits pass for
166 classes. The final integration run exposed a missing `tracestick` entry in
both Tauri renderer lists; both lists now include it, and all 69 shell checks
pass. Tauri's ordinary suite passes 144 tests. Its three GPU checks were then run
explicitly on Metal, including real numbered animation PNGs, hover and a volume-only
scene. Both external MDTraj trajectory checks ran successfully with the generator
installed. Distribution-helper tests pass six cases and all workflows pass
actionlint. Public release and Pages execution remain untested under the no-push
instruction.

The complete browser suite passes all 53 tests after rebuilding the final
geometry. This run also exposed a bottom-panel layout error: the Console pane
shrank by the unused input-row margin on successive frames, changing the viewport
and invalidating click coordinates. The panel now reserves its assigned height,
and Console/Output scroll areas retain it regardless of log length. Navigation,
trackpad scaling, log growth and affine glyph checks pass together.

### Bundled POV-Ray and real process validation

POV-Ray is now part of the native tools bundle. The source archive is pinned to
[the upstream 3.8.0 beta 2 release](https://github.com/POV-Ray/povray/releases/tag/v3.8.0-beta.2)
and verified by SHA-256 before building. Unix builds use the upstream console;
Windows uses its console project with the current MSVC toolset. Standard include
files, runtime libraries, licenses, original sources and the build helper travel
with the executable. The native release cache includes the new helper, and each
release platform must render a CueMol-exported scene before packaging.

The Unix build disables fast-math because the upstream parser uses infinity as
a sentinel; GCC's finite-math assumptions otherwise reject a default camera.
The macOS build supplies the missing 32-bit TrueType alias. Unix builds omit the
optional OpenEXR input codec; the CueMol PNG rendering path is unchanged. These
build changes are recorded in the helper shipped beside the original source.

The command launcher discovers standard include files relative to the renderer.
POV-Ray parses its own arguments, so input, output and include paths containing
spaces require quotes inside each argument. Both desktop frontends share this
handling. Tauri also resolves its default executable names through the shared
tool configuration, including APBS/PDB2PQR calculation and FFmpeg animation output.

A complete Linux tools bundle was rebuilt successfully. The Linux and macOS
binaries each rendered a CueMol-exported molecular scene to a 96 by 96 PNG with
62 distinct colors. The macOS bundle also passed after relocation to a directory
containing spaces, without build-library search paths. A real desktop process
test rendered a 48 by 48 PNG using input/output paths with spaces. Mock process
tests still cover cancellation and post-blend passes; they are not used as the
only evidence that the external renderer works. All 33 command tests and 144
ordinary Tauri tests pass; the real renderer test also passes explicitly on Linux
and macOS. At this checkpoint, Windows and the other native targets had not yet
executed in the release matrix. Later public results are recorded separately above.

### Shared command exports and complete browser embedding

The legacy `render-qsc` and `export-scene` commands previously duplicated scene
geometry construction. They now load through the shared command layer and use
`DesktopAppState` snapshots. This removes the obsolete combined object/renderer
transform path and keeps saved surface colors, material definitions, map regions,
and floating-point volume data consistent with the GUI. PNG output uses the
shared render settings. Triangle exports no longer substitute an isosurface for
a volume. Eleven CLI scene tests pass, including all eight output formats,
reflected nonuniform renderer transforms, invariant molecular radii, saved image
dimensions, surface coloring, camera visibility, and external-scene companions.
The other five CLI integration tests also pass with the reference fixtures present.
The integrated native/shared rerun passes 1,595 tests across 100 test executables;
the normally ignored real POV-Ray case also passes when invoked explicitly.
The browser checks cover 53 existing cases plus the new saved-camera visibility
case. The new case is also run independently against the rebuilt browser assets.

The common QSC loaders now resolve named camera visibility after maps and surfaces
have their runtime identifiers, and apply the saved camera on file and byte loads.
Unit-cell geometry retains its renderer name, alpha, material and edge group.
The map fixture explicitly requests the full region: a world-centered periodic box
can legitimately extend outside the translated map's original unit cell.

The installed-archive check found five missing embedded license/provenance files
that were present in the browser distribution manifest. The build now embeds all
manifest entries, or the generated `pkg` tree when building directly from the
source browser directory. Staging also requires these five files. The six
portable-distribution checks pass; the installed artifact check compares every
served asset's bytes against its manifest hash.

### Additional command and byte-loading audit

Generic command tools had several paths that reported success without changing
live state, or that only supported molecular objects. Common object transforms,
renderer material/alpha/edge properties, renderer groups and resets now use the
shared state for maps, saved surfaces and lightweight objects. Creating these
renderers preserves the requested name. Renderer lookup and nested gradient
updates use identifiers, so duplicate and empty names cannot redirect edits to a
sibling. Materializing a standalone surface preserves its implicit renderer,
while an explicit file-load setup replaces that default instead of adding a
second copy. Molecular selection/name resets update the typed fields as well as
serialized attributes. Reset-all preserves names, styles and group membership.

View command writes now update camera and view state, including stereo settings,
center marks and input momentum. Unknown view properties and read-only values
return errors. Stereo display is covered by the subsequent audit below.
`create_symmetry_copy` now uses the matrix API's one-based indices.
Atom-label commands validate the shared format language before mutation and
store live formats, so later coordinate/atom-property edits remain reflected.

The byte loader previously rejected standalone surfaces and interpreted every
`.qdf` as a molecule. It now dispatches all five QDF object types using the shared
sniffers, accepts OBJ/PLY, preserves those objects in embedded scenes, and handles
map-CIF and XPLOR aliases. PyMOL byte loads share the native import logic for
representations, labels, measurements and saved cameras. Native PyMOL imports
also parse their pickle once instead of three times. Browser object loads fit
the geometry while actual scene/session loads restore their saved camera.

Validation passes 43 command tests, 230 shared-core tests with the reference tree
present, 167 native GUI tests and 144 ordinary Tauri tests. Optional renderer/GPU
cases remain explicit, separate checks. Seven command property regressions cover
all auxiliary kinds, invalid values, resets, groups and duplicate identities.
Two byte-loader tests check QDF dispatch, surface geometry after embedded reload,
and equality with the native PyMOL import. Four rebuilt-browser cases pass,
including measured surface-width changes and opacity restoration, PyMOL pixels,
gradient identity and capped map coloring. Clippy completes with existing warnings;
the warning in the new property helper was corrected. The subsequent complete
browser rerun passes all 56 cases.

### Stereo display, export and view synchronization

Stored stereo properties previously left the actual viewport in mono. The shared
camera helper now follows `GUIView::setUpModelMat`: each eye rotates around the
view center by half the angular separation, in degrees. Parallel/cross modes,
eye swapping and odd viewport widths share this calculation in native, browser,
Tauri, CPU rendering and offscreen color/depth capture. Both eyes reuse scene
geometry but retain independent GPU uniforms, volume inputs and AO/AA state.
CPU filters, NPR and native/browser OIDN process each eye independently before
joining the rows, including the public prepared-OIDN API.

Labels use each eye's projection and depth, with clipping at the seam. Point and
region picking use the same eye cameras and deduplicate atoms selected in both
halves. Stereo uses CPU picking; GPU picking remains available in mono. View
options survive tab inheritance and activation, while console-script camera
changes now reach the native viewport. Tauri scene/tab changes restore their own
eye-order and momentum flags. The legacy Umbreon exporter also uses the actual
scene background instead of a fixed gray value.

Four actual Metal tests compare both halves with independently rendered eyes,
including perspective, volumes, labels, depth, supersampling, eye swapping and
an odd image width. Four command tests cover label anchors, both-eye picks,
deduplicated regions, script hits, view inheritance and the configured Umbreon
export. The CPU regression compares RGBA output after A-trous, OIDN and NPR,
and checks cancellation. The Tauri tab regression checks inheritance and reset
of eye order and momentum.

The headless workspace run passes 1,236 tests across 100 executables. Separate
scripting-enabled runs pass 47 command and 230 shared-core tests with the
reference checkout present. Native GUI tests pass 168 cases, and Tauri passes
144 ordinary cases. Actual Linux GUI checks pass label toggles, residue/range
selection, measurements, lasso, bond edits, undo and affine geometry; both eye
centers also accept clicks after parallel/cross switching and eye swapping.
The 56 existing browser cases and two additional WebGL/WebGPU cases pass.
The stereo cases inspect both image halves, toggled labels, region selection,
CPU image export and eye order. Clippy completes with existing style warnings.

The shared center mark draws a cross or three rotated axes in each eye. It now
uses the reference RGB inversion, as checked above. Hardware
quad-buffer stereo remains unavailable and reports a false capability; selecting
that mode produces mono. The single-point `View.projToScreen` API retains the
reference's central, full-width projection; interactive stereo picking does not
use that ambiguous projection.

### Script animation, stream I/O and memory audit

Animation frame export now evaluates the current scene and invokes the supplied
exporter. Previously, the script entry points advanced a placeholder exporter
without producing files. Frame setup retains the initial camera, overlapping
camera motions start from their own start-time camera, and renderer translations
retain their initial camera axes. Failed writes detach the exporter without
advancing the frame number. Script exports flush preceding edits before writing.

Script playback shares the application timer with command, native and browser
controls; Tauri ticks use the same state. Pause, resume, loop, seek and natural
completion retain the reference behavior. Stop restores only animated properties,
including absent attributes and the secondary alpha/transform changes from
show/hide and slide motions. Tests include molecular, map, surface and lightweight
objects, and morph frames across separate script calls.
Animation and focus commands now advertise their scene changes, so native and
browser callers refresh the displayed result after pause, stop and focus.
File-writing tools advertise their side effects separately from scene edits;
writing a file does not create a scene undo snapshot.
Native and Tauri toolbar playback also use the shared player. A GUI-started
animation can therefore be paused or stopped from a script without continuing
on a second timer or resetting before the script runs. Interactive playback no
longer clones the whole scene for restoration; it saves the initial camera and
only the properties that animation changes. Normal edits stop the animation
before recording undo, while script controls retain their current playback time.

Scene readers and writers complete synchronously and propagate errors to the
calling script. Object-load commands return the attached object's actual identity.
SDF selections filter atoms and remap bonds. Object streams apply gzip or XZ
before outer Base64 on write and reverse that order on read; auxiliary streams
remain raw. Unsupported compression fails before truncating the destination.
Source metadata and reader options survive linked-scene reloads, including
multiple models, density options, trajectory filters and NAMD topology paths.
Scene writing honors embedded auxiliary data, QDF versions and chunk encoding.
Scene readers preserve existing view cameras, while `LoadSceneCommand.set_camera`
controls whether the saved camera is applied to all existing views. Replacing a
scene also removes its old animation player and saved runtime properties.
Reader construction preserves the reference distinction between direct
constructors and factories: the latter reset declared defaults before applying
explicit options. QSC source readers use those factory defaults.

Script stream jobs now retain the chosen reader, its filters and options across
separate command calls. Previously, the asynchronous adapter guessed the format
and discarded jobs when the script context ended. Native loads parse on an I/O
worker; the browser collects chunks and parses them when `waitLoadAsync` is
called because its current WASM build has no worker threads. Both paths return
the requested object type and preserve source metadata. Completed or failed
jobs are removed, and invalid supply lengths return errors.

Per-frame animation evaluation now borrows molecular, map and surface data while
copying only the metadata required by the animation runtime. Camera-only playback
also retains the native/browser GPU geometry and Tauri geometry snapshot. GPU
uploads write converted instances and mesh vertices directly into staging memory,
removing the complete intermediate arrays and their extra copies.

Release-mode measurements use three runs per variant. A 100,000-atom, 60-frame
shared-core rotation benchmark decreases from a median 18.918 ms to 0.149 ms per
update, with an identical final camera quaternion. Median process peak RSS falls
from 70.34 MiB to 38.28 MiB on x86-64 Linux. These times measure animation state
updates, not full GUI frame time. Run it with:

```sh
cargo run --locked --release -p cuemol-desktop-core --example animation_bench -- 100000 60
```

A separate Metal transfer benchmark sends one million 80-byte records ten times,
including queue completion. Direct staging takes a median 12.512 ms per upload
versus 15.397 ms with an intermediate vector; median process peak RSS falls from
245.33 MiB to 169.00 MiB. GPU readback exactly matches the first, middle and last
records. This isolates upload conversion and transfer, not scene construction or
rendering. Both variants remain runnable on a supported native GPU:

```sh
cargo run --locked --release -p cuemol-desktop-core --features offscreen --example upload_bench -- collected 1000000 10
cargo run --locked --release -p cuemol-desktop-core --features offscreen --example upload_bench -- direct 1000000 10
```

Current source checks pass 1,241 headless workspace cases, 250 scripting-enabled
shared-core cases, 48 command cases, 170 native GUI cases and 146 Tauri cases.
The native and Tauri suites skip one and four optional external-runtime tests,
respectively. Clippy completes with style warnings. All 62 browser cases pass,
including chunked compressed input across commands, animation pixel motion,
pause/resume/stop, and geometry reuse during camera-only playback on WebGL and
WebGPU. Actual Linux GUI checks also exercise selection, measurements, undo,
affine transforms, stereo eye picking and restoration after animation stops.
Both script-started and toolbar-started animation move the displayed molecule,
pause stably through script controls, and restore its original pixel position
through the shared stop command.

### Documentation reconciled with implementation

The implementation takes precedence over stale descriptions. Corrections found
during this work are collected here; the read-only reference itself was not edited.

| Previous description | Behavior followed or documented |
| --- | --- |
| Proline donor exclusion depended on case. | Proline is excluded regardless of case. |
| DefaultRibbon section dimensions and all axial detail defaults were shared. | Style-specific dimensions are used; tube/spline default to six subdivisions, while named ribbon styles and cartoon use eight. |
| The current C++ trajectory reader was eager. | The reference also indexes frames; Rust playback and memory claims now state their measured scope. |
| Native layout and snapshot comments described XUL; a later entry still called Tauri a legacy layout. | Native, browser and Tauri shells follow Tritium, with toolkit-specific widgets. |
| Surface previews were uniformly light gray. | Preview geometry uses atom colors. |
| Object coloring implicitly supplied the historical palette. | `$molcol` follows the object's solid default or explicit coloring scheme. |
| README/browser help described earlier custom mouse bindings. | They now describe the reference presets and their configurable bindings. |
| Repository metadata used an old organization URL and the previous repository name. | Documentation, installers and update discovery follow `mogura-rs/cuemol-rs`; persisted camera/view format identifiers remain compatible. |
| Native annotations described only atom labels. | Measurement values and their positions are included. |
| A missing custom pivot produced no backbone. | The reference falls back to the residue's normal pivot. |
| Both `VALIDATION.md` and `validation.md` existed with different content. | Evidence is consolidated in `docs/validation.md`. |
| `Renderer.qif` described unnamed edge groups as object-specific. | The C++ implementation groups them by matching edge style. |
| The reference cap-table header described generated surfaces only. | The renderer also uses caps for full-region display and explicit `cap_mode=on`. |
| Map meshing comments still said marching tetrahedra. | The implementation uses marching cubes. |
| An old surface helper claimed solvent-accessible marching tetrahedra. | Its actual solvent-excluded marching-cubes behavior was superseded by the shared SES backend path. |
| A spline helper comment said Catmull-Rom. | The implementation uses a natural cubic spline. |
| Snapshot transform comments combined object and renderer transforms and described rotation-only normals. | Object coordinates are transformed before glyph construction; renderer geometry uses affine transforms and inverse-transpose normals. |
| Sphere/cylinder helper comments said flat shading. | Their radial vertex normals support smooth shading. |
| README called POV-Ray an optional external installation. | Native distributions now include its console renderer, runtime, standard files and sources. |
| Earlier validation entries described the registry audit as unfinished. | Checkpoints are marked as historical and linked to the completed integrated audit. |
| The legacy CLI claimed to honor all saved renderer geometry independently. | Both scene-export commands now use the shared GUI geometry and render settings. |
| The browser manifest listed assets that the executable's fixed list omitted. | Embedding follows the manifest, including the five missing license/provenance files. |
| The byte-loader comment described native-reader behavior without stating its molecule-only QDF dispatch and missing surface/session paths. | Byte loads now use the matching domain readers and shared PyMOL import logic. |
| The native playback comment claimed an empty timeline automatically spins. | Playback requires an animation with a positive duration. |
| `LoadSceneCommand.qif` offered `scene_name` as the new scene name. | Its C++ implementation does not use the property; loading uses the file-derived scene name. |
| `MmcifMolReader.qif` declared `loadsecstr=false`. | Direct C++ construction sets true; `StreamManager` resets it to false. Both entry points preserve their actual behavior. |
| README said every unsupported setting returned an error. | Legacy PNG `interlace` and `compresslev` remain accepted but ineffective, as in the C++ exporter. |
| SDF/MOL2 declarations set `chain_name="_"`, while their constructors set `"A"`. | Direct construction uses `"A"`; factories and QSC source readers reset to `"_"` before explicit options. |
| MCP annotations labeled animation, focus, history clearing and file writes as read-only. | Their side effects are declared; scene changes trigger viewport refresh separately from file writes and undo recording. |
| The first completion-audit example used `SimpleRenderer.linew`. | The actual property is `width`; the separate `alpha` loss was a valid failure. |
| Python was described only as a binding facade. | The native bridge now keeps interpreter state and connects Python objects and callbacks to the live scene. |
| Historical validation entries said publishing was still waiting for permission. | Those statements now identify their earlier checkpoint; current public validation is recorded separately. |
| The reference-render test bypassed the documented fixture discovery with a machine-specific path. | It uses `CUEMOL2_REF_DIR` or the sibling reference checkout, writes to a temporary directory and fails export errors other than a missing GPU adapter. |
| The HTTP pool description did not distinguish an idle worker from one already reserved by queued work. | The worker-allocation check and comment now account for queued connections. |
| The public GUI entry still reported a locked Mac desktop and untested Windows, ARM Linux and Intel Mac interaction. | Later OS input and window-capture checks passed on all five published native targets. |
| The Python usage entry only described the root binding. | It now lists the embedded helper modules and their live-scene coverage. |
| Linux startup instructions omitted an X11 runtime needed by the published GUI. | The Debian/Ubuntu dependency list now includes `libxkbcommon-x11-0`; the GUI matrix also provisions graphics drivers. |
| Browser parsing was described only by the synchronous script-reader boundary. | Ordinary file input now uses a cancellable parser worker; script `waitLoadAsync` retains its synchronous API. |
| Label documentation described only bundled fonts. | Native labels resolve installed families, and browser labels use the reference OffscreenCanvas path. |
| Native key-storage instructions omitted the Linux session service. | Saved Linux keys require Secret Service; environment and session-only keys work without it. |
| The release-cache description did not distinguish cache scopes. | Tags can reuse default-branch caches, but cannot read another tag's cache. A manual default-branch build prepares reusable caches. |
