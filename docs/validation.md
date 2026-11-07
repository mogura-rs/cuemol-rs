# Validation evidence

## Rendering controls preserve saved numeric values

The shared egui controls previously clamped stored image dimensions, DPI and
backend properties when displaying them. This also queued a scene edit and
could add an Undo entry without user input. Focused regressions reproduced the
mutations, including a change from 9,000 to 8,192 pixels and from 9,600 to 4,800
DPI. The controls now preserve existing values and apply bounds only to input.
Movie dimensions, frame rate and bitrate follow the same rule. Job validation
remains separate from displaying settings.

All 74 shared UI tests pass on Linux and macOS, and all 232 native GUI tests pass
on macOS. The headless workspace passes 1,630 tests with default features and
1,841 with the CI scripting feature set; both exclude five explicit external/GPU
checks. The read-only reference checkout is present for these workspace runs.
The default run exposed an obsolete test that rejected the now-supported POV-Ray backend;
it now verifies transactional rejection of an unknown backend. The shared UI
run found a simulated Select All event that sent only Control on macOS, causing
the axis test to insert into the old text. It now sends egui's logical command
modifier and retains the numerical and history assertions.

Both WebGL and WebGPU regressions preserve small physical dimensions, high DPI
and backend properties while viewing the controls, without adding an Undo entry.
Movie dimensions also remain unchanged until edited; an out-of-range typed edit
still clamps to the input limit, and Undo restores the previous stored value.
All 27 affected browser Rendering and movie checks pass against the rebuilt
WASM, including the NPR controls and backend/history workflows. The macOS native
binary builds and its transferred copy has an identical SHA-256. Actual macOS
screen/input checks still await operating-system permissions.
Twelve distribution-helper checks and Linux installer/self-upgrade fixtures pass,
including checksum rejection and replacement rollback. These fixture checks do
not constitute a newly published multi-platform release.

## Tauri startup options and native WebKit verification

Tauri startup arguments previously used a direct loading command and bypassed
the per-file reader/renderer form. Both the shell regression and an actual Linux
WebKit window reproduced the bypass. Startup now drains its pending paths once
and uses the same per-file workflow as menu opens, retaining the shell destination
policy. Cancelling a file leaves it unopened, and later accepted files share the
first successful destination. Cancelling the entire batch clears the loading
message. The older usage statement that every supported object open showed
options omitted this startup exception; the implementation now matches it.

All 85 Tauri shell/Rendering DOM checks and 68 shell contracts pass. Two new DOM
checks cover startup batching and cancellation; existing file/drop checks remain
green. Three focused Tauri file-option model checks pass. Both Linux and macOS
Tauri binaries build, and formatting and diff checks pass. The source contracts
now check the path-draining and options workflow instead of direct startup loads.

An isolated Linux WebKit session drives the actual Tauri binary through OS mouse
and keyboard input, with real IPC and filesystem output. It accepts startup
options, draws the molecule, edits NPR Strength/Curve, performs keyboard
Undo/Redo, resets the style and changes backends. It renders a 64-by-48 image,
and the system clipboard contains matching RGBA pixels. Screenshots are captured
after the WebView has painted and were inspected. The native GTK Save dialog
writes the chosen PNG path with identical bytes, including a filename with spaces.

Using an animation prepared through real IPC, the same window renders three
64-by-48 frames at 29.97 fps and encodes MP4. The bundled FFmpeg decodes three
distinct frames. OS slider input selects the exact first PNG, and native Save
Movie As writes identical movie bytes. Re-encoding at 12.5 fps after deleting the
animation preserves every source frame's bytes and timestamp. Cancelled Cleanup
keeps the files; confirmed Cleanup removes the matching outputs and retains the
cached preview. The live view stays unchanged. These checks use the actual
WebView and native pickers; neither IPC nor the picker results are substituted.
The earlier Chrome-adapter restrictions below therefore no longer apply to
these Linux paths. External Open/Reveal applications, macOS screen/input
permissions and other platform window integration remain separate checks.

## NPR Detail controls and value preservation

All three frontends now use the shared hatch-field definitions, including primary
Strength/Curve, tone levels, kind-specific layer controls and contextual
enablement. Effective-size hints account for mark density, width and
supersampling. Reset restores the selected style, the Edited indicator stays on
the same row, and leaving NPR returns an open Detail tab to Render. Opening the
editor preserves saved values outside the controls' ordinary input ranges.

The optional comparison against the actual reference TypeScript passes for all
53 field definitions and 2,016 dependency cases. Shared checks round-trip all
nine styles and preserve tone levels. Two regressions first reproduced the
obsolete Detail tab and egui's automatic range clamping, then passed after their
fixes. All 98 command checks pass with four explicit external/GPU checks excluded;
all 232 native and 204 Tauri model checks pass, with six explicit Tauri checks
excluded. Five shared Rendering checks, the separate range-preservation check
and five Tauri Rendering DOM checks pass. The final native build and optimized
WASM build pass.

The affected browser run passed 23 of 25 cases. The two new NPR cases read the
second edit before its debounce completed; after waiting for both stored values,
both WebGL and WebGPU cases pass against the final build. The entire 25-case run
was not repeated. A real Tauri Rust-model/Chrome run and an actual Linux egui
window run verify Strength/Curve edits, reopening, Undo/Redo, Reset and backend
switching; the former also verifies AO/color-fill dependencies. Screenshots were
inspected. These runs do not establish native Tauri WebView OS input, and the
authorized macOS host still reports missing screen/input permissions.

The reference TypeScript form permits removing the final layer, but the C++
renderer interprets an empty layer specification as keeping the style template.
The port therefore retains at least one layer, disables Remove at that boundary
and limits additions to the renderer's 64-layer capacity. Earlier documentation
described editable tone specifications without noting the missing primary tone
controls or contextual field rules; those omissions are now corrected.

## Browser movie output in the shared Rendering window

The browser now captures an isolated Rust movie scene, renders sequential frames
through the shared still pipeline and writes PNGs to origin storage or a chosen
folder. H.264/H.265 MP4/MOV encoding uses runtime capability checks and streams
muxed chunks to an atomic file writer. A pinned Mediabunny module, its MPL-2.0
license and checksum provenance are bundled by the unified web build; Node is not
required for packaging. Unsupported encoders fail before output replacement.
Raw MOV, WMV2 and animated GIF still require the native application.

The shared viewer supports movie history, selected-frame Save/Copy, movie
Save/Open, re-encoding without an animation and confirmed cleanup. Closing the
window leaves the job running, and Stop retains completed frames and concurrent
scene edits. Frame reads are serialized, with only the latest pending selection
retained. Each history entry stores one cached PNG within its byte budget;
replaced outputs cannot silently change an older result's displayed frame.
Cross-window output locks protect writes and cleanup. Managed temporary sessions
skip active owners, unknown content and all custom folders; expired frames and
movies use the native retention intervals and inactive-session limit.

Four storage checks pass, covering MP4/MOV metadata and frame bytes, fractional
rates, cancellation preserving a previous movie, locks across two windows,
unconfirmed replacement, unsupported encoding, preflight abort and retention.
Three application checks pass in WebGL/WebGPU: actual three-frame output,
selected-frame downloads, movie download/open, re-encoding without changing PNG
bytes or timestamps, confirmed cleanup with cached previews, closed-window work,
and partial cancellation after a separate scene edit. The final 27-case run
passes, including all 20 Rendering checks. Slider checks delay real reads and
verify one in-flight read, latest-selection precedence and preserved history
previews. Both generated MP4 files also decode through the bundled FFmpeg into
three distinct 64-by-48 frames.

A new mode-request regression first reproduced the browser toolbar retaining
movie mode. Toolbar and menu requests now select the requested mode and reapply
its default size, including repeated requests for the same mode. This follows
the current reference without immediately writing those defaults to the scene;
movie preferences and history-restored dimensions remain intact. Earlier browser
tests set dimensions before opening Rendering and relied on the old behavior;
they now set their requested dimensions after opening it.

All 96 shared command checks pass, with four explicit external/GPU checks
excluded; all 232 native and 203 Tauri model checks pass, with six explicit Tauri
external/GPU checks excluded. Shared history tests cover byte-budget eviction,
stale/failed asynchronous loads and preserving prior metadata on failed writes.
The optimized WASM build, workflow actionlint check, formatting and diff checks
pass. A fresh CLI with GUI and bundled web assets builds, and all 43 served files
match the staged manifest, including JavaScript/WASM MIME types. The embedded
application passes the mode-request regression; the preceding embedded build
also passed a complete movie output workflow. The browser tests
exercise real downloads and movie tabs; custom-folder tests use genuine browser
storage handles through a substituted picker. Actual OS directory/save dialogs
have not been driven. Browser storage availability and codec support remain
platform-dependent, and these checks do not establish complete reference parity.

## Scene-scoped application history

Native egui, Tauri and the browser now store shared document changes for the
scenes affected by an edit. Rendering-window Undo/Redo selects the latest entry
for its target scene, preserving unrelated edits, redo entries, live camera
navigation and host preferences. Multi-scene operations remain atomic and reject
out-of-order restoration across later changes to their linked scenes. Console
and Agent turns still accumulate one history entry as their steps finish.
Histories retain shared immutable molecular/map storage; grouped operations keep
one temporary baseline only while running.

Five shared regressions cover scene lifecycles and view restoration, monotonic
identifiers, other-scene edits, later same-scene properties and modified flags.
The modified-flag regression first failed after undoing two object creations;
allocation limits are now normalized only for content comparison. Object and
renderer identifiers remain reserved after Undo. The full shared core suite with
the reference checkout passes 326 checks. A pre-existing lightweight-surface test
still expected `molsurf 1` for an implicit renderer; its assertion now matches the
implemented `(molsurf)` fallback.

All 232 native model checks, 203 Tauri model checks and 68 shell contracts pass;
six explicitly excluded Tauri checks need external tools or a GPU. A native egui
key-event test and WebGL/WebGPU keyboard checks exercise inactive-target Undo and
Redo. An actual Linux window run repeats target selection and Ctrl-Z/Shift-Ctrl-Z
through OS input, preserving the other scene and camera, then verifies that a
new first-scene DPI edit leaves the second scene redoable. Its screenshot and
MCP-read values agree. A real Rust-model/Chrome run also verifies Tauri Ctrl-Z/Shift-Ctrl-Z across
two scenes, unchanged live camera/navigation, and independent redo after an edit
in the other scene. This adapter does not establish native WebView OS input.

The browser-wide regression run exposed a separate Density Cell failure. The
renderer command had reused the New Renderer dialog's filtered menu inventory,
which excludes internal unit-cell renderers. The command now accepts the map's
supported internal types while the dialog keeps its reference choices. The
failure was reproduced in a command test; all 18 map-loading and renderer-option
checks pass after the fix. All 21 browser Rendering and Density interaction
checks pass against a fresh optimized WASM bundle. The complete browser run finished with 351 passes and four failures in
40.7 minutes. Two were the Density Cell cases above. The other two exposed a
scene-style Undo regression: a later script call materialized a loaded style in
the runtime cache, and Undo removed its scene definition without removing that
cache entry. Rebuilding changed scene styles before updating their bindings now
removes the stale entry while preserving later local and global styles. A sixth
shared regression reproduces this sequence and checks scene deletion/restoration.
The reference-backed core suite passes all 327 checks; all 232 native and 203
Tauri model checks pass again. A freshly rebuilt WASM bundle passes all ten
style-editor tests, including both failed cases. Together with the 21 Rendering
and Density reruns, these resolve all four failures from the complete run; the
entire 355-case suite was not repeated after the two fixes. The current macOS
Tauri binary built before the cache fix.

## Restore still-image history after movie mode

The shared Rendering panel now reloads its editor when applying history. A
movie display can convert physical dimensions to pixels without changing the
stored scene settings; the previous cache check then kept those converted values
when restoring the same still-image settings. A regression first reproduced
`px` remaining in place of `in`, then passed with the editor reload.

The browser also restores the result's Still/Movie mode before reopening the
editor. Its new interaction check first failed because Movie controls remained
visible after applying still-image history. The repaired flow restores inches,
width, height and DPI, and renders the same PNG bytes. All five shared render
panel checks, 12 focused native checks and all 17 browser Rendering tests pass;
the WASM build passes.

## Tauri movie integration in the Rendering window

Tauri now routes Animation rendering into the same modeless Rendering window as
still images. The window uses the shared movie preferences, sequential isolated
job, managed output sessions and disk-backed result history. It supports pixel
size presets, fractional frame rates, all seven format choices, live preview,
frame selection, re-encoding, confirmed replacement/cleanup, and movie file
actions. The application loop polls the shared job after the window closes.
The previous animation form and its unused CSS have been removed; existing Rust
command endpoints remain compatible.

Four focused model checks pass, including the explicitly enabled FFmpeg test.
They cover browser number normalization, physical-size target conversion,
closed-window output, concurrent edits, frame-byte saves and stale selections,
cleanup confirmation, cached previews, cancellation counts, preference reload,
repeated mode presets and re-encoding without an animation. The full Tauri suite
passes 201 checks with six explicit external/GPU checks excluded. All 68 shell
contracts and 82 browser DOM tests pass. The macOS Tauri binary builds.

A real Rust-model/Chrome integration run renders three 64 x 48 PNGs at 29.97 fps
and encodes MP4; FFmpeg confirms three decoded frames. Frame selection returns
the exact first PNG bytes, and Save Movie As copies the encoded bytes exactly.
Re-encoding at 12.5 fps after removing the animation leaves PNG timestamps and
the active view unchanged. Cancelled replacement/cleanup keep the output files;
confirmed Cleanup removes only matching outputs and retains the cached preview.
A longer run is stopped after a separate scene is created, retaining the saved
partial frames and the concurrent edit. The UI reaches the cancelled state and
no encoded movie is reported for that partial run.

The real integration exposed a stale-settings guard that compared JSON integer
and floating-point representations directly. Browser `30` and Rust `30.0` now
pass through the same typed movie options before comparison. A focused regression
covers this case. Movie mode also converts physical dimensions to pixels when
switching target scenes. Restoring a still-image result reloads its original
physical units even if the target already stores those same settings.

The usage guide and implementation checklist previously described a separate
Tauri animation window; both now describe the unified window. The old source
contracts referenced that removed form and are updated to check the shared
controls and runtime. Native WebView OS input and the actual file chooser,
clipboard and Open/Reveal applications were not exercised by the Chrome adapter.
The scene-scoped history section above supersedes the target-Undo limitation.
The browser movie section above supersedes the execution limitation. Remaining
reference details are still open.

## Native movie integration in the Rendering window

Native egui now uses the shared movie options, managed sessions, sequential job
and result history inside the modeless Rendering window. Mode requests select
the still or movie size preset; movie controls use pixel dimensions and preserve
fractional frame rates. The window supports temporary/custom folders, seven
formats, existing-frame re-encoding, confirmed replacement/cleanup and movie
file actions. Closing the window does not stop output. History holds one cached
preview per result and loads only the requested frame. Replaced or removed files
invalidate the old frame controls and movie path without discarding that preview.

The shared command suite passes 94 cases with four explicit external/GPU checks
excluded. Shared rendering UI tests pass four cases. Native GUI tests pass 160
cases, including three movie regressions for mode presets, closed-window output,
concurrent edits, old-preview retention, and cancellation after a frame has
already been saved. Tauri passes 198 model tests with five existing checks
excluded; all 68 shell contracts also pass. The updated native binary and WASM
build succeed, and all 16 browser rendering tests pass with WebGL/WebGPU coverage.

Actual Linux window input renders three 64 x 48 frames at 29.97 fps and encodes
MP4. FFmpeg confirms three decoded frames. Clipboard pixels match the selected
first frame. Re-encoding at 12.5 fps after removing the animation preserves the
frame file timestamps and live view. Replacement cancellation leaves outputs
intact, while confirmed Cleanup removes the matching output and leaves the
cached preview copyable. A longer render is stopped through the window while
another scene is created; completed frames and the concurrent scene edit remain.
Save movie as uses an atomic copy and preserves source aliases and existing
destinations on failure; its file tests pass. Native file actions use OS dialogs
and the existing Open/Reveal helpers; their
external dialogs/applications have not been exercised in this window check.

The first UI pass exposed a height calculation that moved controls after the
new mode row was added. The panel now measures its action area, and the DPI,
backend and full browser regressions pass. The stop check also exposed a queued
completion race: a frame already saved before cancellation was omitted from the
reported count. The job now records that successful save before handling Stop.
A movie atomically saved before Stop is likewise retained as a completed result.
Focused regressions cover both orderings.

The usage guide previously described a separate animation window for both native
frontends. At that stage it described the unified egui window and the separate Tauri
window. The Tauri and browser integrations above supersede those limitations.
Remaining reference details are still open. The scene-scoped
history section above supersedes the earlier target-Undo limitation.

## Shared movie output and sequential rendering

The command layer now owns movie preferences, frame-file operations, managed
output sessions, a sequential movie job and movie metadata in render history.
The new job uses the same typed preparation as still rendering, writes one frame
at a time through a temporary file, and keeps one isolated scene. It retains
completed frames after errors or cancellation. Encoding replaces an existing
movie only after FFmpeg succeeds and produces a nonempty output; a failed
re-encode leaves the previous movie intact. A directory/base-name lock prevents
concurrent jobs or cleanup from changing the same output. Logs remain bounded.

Temporary sessions survive application shutdown. A session lock protects live
instances. The next session creation sweeps owned inactive folders: frames older
than one day, movies older than 30 days, and movie sessions beyond the newest
ten. Unknown content, symbolic links and custom output directories are skipped.
Movie preferences omit the temporary path and retain custom-folder choices.
History stores one preview per result and reads only the selected movie frame;
clearing history leaves movie output files in place.

Shared command tests pass 92 cases with four explicit external/GPU checks
excluded. All six movie-scene tests also pass when the rendering check is enabled,
including three numbered PNGs at exact pixel dimensions and concurrent live
edits. The FFmpeg check passes explicitly for all seven supported formats;
ffprobe verifies the resulting dimensions and three decoded frames for each.
Failure/cancellation tests check previous-output preservation, actual encoder
process exit, output locks and temporary-file cleanup. Native animation tests
pass 26 cases and Tauri passes 24 with one existing GPU check excluded. Three
file-helper tests moved from the desktop suite into the shared command suite.

The previous native and Tauri frame-count functions rounded fractional end
frames, while `AnimMgr::setup_render` floors the end frame. Both now use the
shared reference-compatible calculation, with fractional-time regressions.
The existing movie windows consume the shared file helpers and frame-count fix;
at that checkpoint, the new preferences, job and movie history still needed
connection to the unified Rendering window. The native connection is documented
above; the Tauri and browser integrations are now covered by the newer evidence.

## Movie state isolation

Native egui and Tauri now capture one shared `MovieScene` for each render job.
Frame preparation advances that private state without changing the active view,
live animation clock or current scene. Missing start cameras fall back to the
captured target camera inside the private state. Display choices and background
are captured once, and Tauri uses the requested scene's background even when
another scene is active. Completion, cancellation, frame failure and encoder
failure no longer restore an application-wide snapshot. Dropping a job signals
all of its remaining workers to stop.

Five shared regressions pass for inactive targets, valid and missing start cameras,
live playback, renderer property/visibility changes and subsequent edits. The
affected native suite passes 29 tests, including real PNG output, bounded workers
and concurrent edits during cancellation. Tauri passes 27 affected tests with
one GPU check excluded from the normal run. That GPU test also passes explicitly
with Metal access and writes two numbered PNG frames. Its initial sandboxed run
had no GPU adapter. Model tests cover frame success/failure, target preparation,
re-encoding cancellation and preservation of edits. Movie controls and output
history still need integration with the shared Rendering window; this change
fixes live-state safety in the existing movie paths.

The old movie test expected an application snapshot to be restored. That assertion
hid the loss of edits made while rendering. The behavioral tests and source
contract now require an isolated movie scene instead.

## Tauri shared Rendering window

Tauri now presents Image, Render and NPR Detail tabs over the shared typed editor,
render targets and image history. Both the Image Rendering menu and toolbar open
this window; the POV-Ray menu selects its backend. The old separate POV form is
removed from the shell, while its existing Rust commands remain compatible.
Settings include physical units, DPI, backend-specific quality controls and editable
NPR layers, shading and specification text. Each completed image records its source
settings, dimensions and elapsed time; Save, Copy, Settings and Use Settings operate
on the selected record. Closing the window retains its history and running worker.
Application shutdown cancels and briefly joins the worker.

The model prepares on its own thread and runs the shared renderer in a cancellable
worker. It keeps completed images in the bounded disk history and returns PNG bytes
only when the selected record changes. Reading image dimensions does not decode a
second full-resolution pixel buffer. Initial rendering persists the displayed
defaults, and pending edits flush before Undo, Redo and scene saving. A scene switch
flushes edits to their original target. Stale edits and stale image actions fail
explicitly. The live check exposed a race when a field was edited immediately after
changing the target: the shell now disables editing until the target response and
rebuilds its controls for the new scene/view. Settings changes notify the main
shell so modified markers and Undo controls refresh; window shortcuts flush their
pending edits before Undo/Redo.

Six focused model regressions pass, including actual image generation, default
sizes, cancellation/failure preservation, worker cleanup, target guards, immediate
Undo, backend settings, units, NPR specifications and history output/restoration.
The full macOS Tauri model suite passes 200 tests with five pre-existing ignored
checks. All 81 Tauri DOM cases pass; the three Rendering cases pass again after
target gating, captured-settings viewing, initial fit, wheel/pinch controls and
Undo/Redo shortcuts were completed. The macOS Tauri application builds. A headless Chrome run connects the real Rust model to the actual Rendering
JavaScript through a local JSON test adapter. It passes transparent molecular PNG
output, byte-identical file saving, the model clipboard path, physical-unit
round-trips, a second target, source-settings restoration, cancellation with previous
results retained, NPR edits and unchanged active scene/camera. Screenshots were
inspected. This verifies the model/DOM boundary; it is not an OS clipboard or native
WebView input test. The local fixture lacks OIDN, so this live run uses raytracing
without GI; the missing-runtime error also preserves the prior image.

The source-string compatibility suite initially reported 23 failures both before
and after this change. Its assertions referred to old labels, previous dialogs
and code moved into modules. They have now been reconciled with the shared Rust
panels, the extracted View/Animation JavaScript and the current Get PDB, file,
renderer and settings flows. All 68 source-contract checks pass. These are source
packaging checks; the model and DOM tests above provide the behavioral evidence.
Usage previously said Tauri retained a separate POV-Ray window without render
history; it now describes the shared window. Later sections above cover movie
integration and scene-scoped Undo.

## Shared POV-Ray still-image rendering

The native egui Rendering window now runs POV-Ray alongside Umbreon and NPR.
The POV-Ray menu selects that backend in the shared window. Scene-owned typed
settings control projection, stereo, clipping, shadows, radiosity, lighting,
post-blending, transparency and pixel labels. Completed images use the same
history, settings snapshot, DPI, Save and Copy paths. Executable/include paths
resolve through the existing tool preferences and bundled defaults; unavailable
paths disable Start with an inline explanation. Browser settings persist but
execution explicitly requires the native application. The shared screenshot
and image commands honor the stored backend; an explicit style selects Umbreon
or NPR instead.

Preparation exports the selected scene/view into an owned temporary directory.
The process streams bounded diagnostics, supports cancellation before and during
execution, validates its PNG dimensions, overlays projected labels and returns
the common image payload. Failure leaves prior GUI results intact, and dropping
the native application signals and briefly joins its render worker. Label
rasterization is now a separate headless feature; only its GPU upload layer
requires the offscreen renderer.

The actual-window run exposed a crash when a 3000 by 2500 result exceeded the
device's 2048-pixel texture limit. Preview uploads now use bounded tiles without
downsampling. The viewer retains the original dimensions and image bytes, so
history, 100% framing and file/clipboard output keep their resolution. A focused
regression covers both initial display and reselecting an oversized image.

All 83 command tests pass, with three optional tests ignored in the ordinary run.
The new real-POV test was run explicitly and passes transparent output, label
composition and file/image equality. All 231 native model tests, 68 shared egui
tests and 194 macOS Tauri model tests pass; five existing Tauri tests remain
ignored. Three core POV tests and six CPU font tests pass. Six browser cases
cover backend settings, target behavior and the POV-Ray native requirement;
the two POV-Ray cases pass again after the preview repair. Native and WASM
builds pass. The Tauri rendering window and movie integration remain separate
follow-up requirements.

The final Linux OS-input run passes backend selection, real transparent rendering,
invalid-setting failure preservation and cancellation with verified child-process
exit. It then renders a 3000 by 2500 image, displays it without a texture-limit
panic and copies the original dimensions. The active camera remains unchanged;
screenshots were inspected. Earlier harness attempts did not wait for clipboard
replacement or observed only the main thread's child list; those attempts are
not counted as successful cancellation/output checks.

Usage previously described only Umbreon/NPR in the shared window and has been
updated. The current reference's POV-Ray backend also reads explicit lighting
values regardless of its stored lightDefault flag; this implementation follows
that behavior. Its reference exporter hardcodes pixel images off, whereas this
port retains its existing optional label/image composition support.

## Render targets and scene-local rendering

Native egui and browser Rendering windows now select any existing scene/view.
The selector follows active-view changes, retains manual choices across ordinary
scene updates, and falls back when a view closes. Target changes flush pending
settings to their original scene. Settings and history restoration edit the
selected target without activating it. The current-view size preset uses the
selected view's dimensions.

The old shared preparation always fetched active-scene geometry, background,
settings and color proofing. Preparation now resolves an optional sceneId/viewId
pair and consistently reads that target. The active view uses the host's live
camera; other views use their saved camera. Stereo eye order also belongs to
the selected view. Invalid, deleted or mismatched targets fail explicitly. The
public screenshot and image schemas expose these optional IDs, while omitted
IDs preserve the active-scene behavior. Render history captures the target
scene name and settings. Usage text previously described active-view rendering
only and has been updated.

Focused regressions cover target lifetime/selection, saved camera, geometry,
background, scene-local proofing, dimensions/DPI, invalid IDs and inactive-scene
Undo/Redo. All 79 command tests pass with two existing ignored tests, and all
230 native and 67 shared egui tests pass. Three browser cases pass: WebGL/WebGPU
target selection, real PNG output compared with the public command, Undo/Redo,
history restoration to a different target, unchanged active camera/scene, and
the small-window layout check. Native and WASM builds pass.

An actual Linux window run selects a different scene, edits its DPI, undoes and
redoes the edit, renders/copies both targets at distinct dimensions, and verifies
that the active scene and camera remain unchanged. Screenshots were inspected.
These checks cover native egui/browser still images at that stage. Later sections
above supersede its open Tauri, unified POV-Ray/movie and target-scoped Undo work.

## Shared render settings and scene edit transactions

Native egui and the browser now use a shared typed rendering editor. Values come
from the scene's RenderSettings classes; display metadata describes labels,
groups, bounds and choices. Backend switches retain independent Umbreon/NPR
blocks, including edits made immediately before another switch. The common
image/camera controls, independent quality axes, conditional AO/GI groups,
reference image-size presets and NPR Detail tab replace the previous flattened
lighting controls. Lighting changes retain supersampling and shadow choices;
returning to GI applies its default lighting balance. Manual changes derive a
Custom quality step from the actual properties. Unsupported NPR GI and
common fields unsupported by Umbreon are omitted.

Settings previously reached the scene only at Start Render, despite being edited
in a separate window. Changes now persist after a 300 ms quiet period, with one
Undo entry for an edit burst. Closing, rendering and target-scene changes flush
pending values. Each write captures its original scene ID, so switching scenes
cannot redirect the edit. Deleted, negative and zero targets fail without
falling back to the active scene. Undo/Redo and external settings changes reload
the editor without producing another write. Native scoped writes mark the
correct scene modified. The shared command accepts an optional sceneId; omitted
IDs retain the existing active-scene behavior.

The audit also exposed a legacy numeric AO override taking precedence over the
editor's gather selector. Canonical gather edits supersede that override, while
explicit numeric scripting overrides remain supported. The editor preserves
legacy per-backend/global values until that gather choice changes, and updates
only the affected backend. Browser auto-save originally replaced the message of
an unrelated completed command; successful auto-save now preserves that message.
The usage text claiming settings were saved only at Start was updated. Image
fields retain the existing Rust limit of 8192 pixels per dimension and support
small images; the reference editor's pixel field uses 100 through 10000. The
renderer additionally enforces its existing 32-megapixel allocation limit.

Validation: five focused command/model regressions cover defaults/catalog types,
quality independence, backend isolation, unit/DPI conversion, delayed writes,
scene switches, legacy AO precedence and atomic invalid-target handling. The
full shared command suite passes 77 tests with two existing ignored tests. All 67 shared UI tests,
229 native tests and 195 macOS Tauri model tests pass; five existing Tauri tests
remain ignored. Ten existing browser cases pass. Two additional WebGL/WebGPU
cases pass after the auto-save message repair, exercising real controls,
per-backend values, quality transitions, physical dimensions and Undo/Redo.

An actual Linux window run edits dimensions and DPI, waits for persistence,
uses Undo/Redo, renders and copies a PNG of the expected size, changes AO quality,
switches to NPR and back, opens Detail, and verifies unchanged camera state.
Screenshots of native and WebGPU settings were inspected. The native and WASM
builds pass. These checks cover the Umbreon/NPR still-image editor; the unified
POV-Ray, movie mode and Tauri window remain open; selectable targets are covered
in the later target audit above.

## Render image history and shared result viewer

Native egui and browser still-image windows now retain completed images with
source scene names, dimensions, elapsed times and settings snapshots. History is
limited to 50 entries. Native images live in an individually owned temporary
directory and are removed on eviction, Clear and normal application shutdown.
Instances cannot delete each other's files. Browser history shares compressed
bytes and additionally caps retained data at 128 MiB, always keeping the newest
image. A single decoded preview texture is retained. Abnormal-exit cleanup of
native temporary directories is not implemented in this store.

Previous/Next and image framing do not edit scenes or add Undo entries. Save and
Copy use the selected result, including its original PNG resolution metadata for
Save. Use settings restores common values and the captured backend as one scene
edit; settings for other backends remain intact. Render startup now also records
changed settings in GUI history instead of bypassing it. Failed reads, decoding,
rendering and cancellation preserve the selected completed image. Clear requires
confirmation. Fit, actual size, zoom buttons, pointer-anchored wheel/pinch and
bounded panning retain relative framing when image dimensions change.

The Linux window check exposed a missing repaint after an asynchronous result:
the image bytes and toolbar were ready, but the texture appeared only after a
subsequent input event. Preview replacement now requests another frame. The log
also has a fixed scrolling area so a long clipboard exception cannot expand the
window beyond the screen. Confirming Clear exposed a second defect: the native
wgpu integration destroyed egui textures before submitting the frame that still
referenced them. Texture release now follows queue submission. A GPU regression
renders a texture on its final frame while that texture is also marked for release.
The usage documentation's Save PNG label was stale;
the result toolbar now uses Save and exports the selected history entry.

Validation: 67 shared UI tests, 72 command unit tests and 228 native GUI tests
pass. Two pre-existing command tests remain ignored. The macOS Tauri model suite
passes 195 tests with five existing ignored cases after enabling native image
clipboard support. Three archive tests cover count/byte eviction, ownership,
failed navigation and cleanup; two viewer tests cover image/settings identity,
failed results, framing and pointer anchoring. The new native regression checks
backend isolation, unchanged camera and one-step restoration with Undo/Redo.
Ten browser cases pass, including WebGL/WebGPU selected-image exports, mocked
clipboard success/failure, settings restoration, cancellation, physical DPI,
small-window bounds and OIDN cancellation. Clipboard permission handling is
mocked there; native image transfer is checked against the actual X11 clipboard.
The Linux OS-input run also checks image visibility without subsequent input,
image dimensions and pixels, history browsing, settings restoration and
camera-preserving Undo/Redo, and cancelling/confirming history deletion without
terminating the application. Screenshots were inspected. Native and WASM builds
pass. The Tauri rendering-window and backend/movie integration audit remains open.

## APBS controls, cancellation and saved potential maps

Native egui and the browser share the APBS electrostatic-potential form. Tauri
exposes matching controls. Each opening resets selection, method, hydrogen,
nonlinear solving and numerical options, while retaining a valid target molecule
and reading the default force field from Settings. Changing the target replaces
the suggested potential name. Empty names use that suggestion when starting.
Executable paths are resolved from shared Settings, bundled tools and PATH; missing
required paths disable Start. The browser explains that APBS runs in the native
application and disables execution; importing DX maps remains supported.

The selection builder, method-dependent fields, numerical bounds and decimal
steps match the reference form. Errors remain inline. Running jobs disable inputs
and Close, expose Stop, and report PDB2PQR/APBS phases and streamed output. The log
retains at most 500 lines with bounded long-line fragments. Native selection
history is now recorded only after successful map insertion, and completing a
remote job no longer closes an unrelated tool window. Tauri request IDs reject
polls and cancellations from superseded dialogs; expected-scene checks reject
stale targets before starting a worker. Detaching a pending form cancels its own
job after startup, and cannot close a replacement dialog.

The source comment promised an embedded APBS result, but the implementation kept
the temporary DX path as an external source. The worker then deleted that path,
so an ordinary scene save could leave the potential unavailable on reopening.
Generated potentials now use embedded storage. A regression removes the original
DX before saving and restores the potential class, grid values and unit-cell
renderer from the saved scene. The Linux window check also exposed native scene
rows sharing a widget ID when two scenes had the same name; rows now include the
scene UID in their scope, and a headless UI regression checks for ID conflicts.

Validation: both APBS core cases, 226 native tests and 195 Tauri tests pass. Five
existing Tauri tests remain ignored. Process tests cover ordered external stages,
cancellation, output arriving before EOF and bounded log retention. Two browser
cases cover WebGL/WebGPU form defaults and the native execution boundary. Two
Tauri DOM cases cover reset/target/name rules, missing tools, inline validation,
live phases/logs, success-only history, stop/error recovery, pending-start
detachment and stale scenes. They use mock IPC; the Rust model tests separately
exercise request IDs, stale targets and map insertion. An initial guarded-dispatch
implementation exhausted the test thread stack through recursive dispatch; the
final code validates IDs before entering the existing nonrecursive dispatcher.

Linux OS input against the rebuilt native binary confirms invalid-selection
preservation, Stop without scene/history changes, a real bundled APBS calculation
using internal charges, blank-name fallback, unchanged camera, one-step Undo/Redo,
ordinary scene save/reload and reset options on reopening. The cancellation case
holds the executable at a test launcher barrier before the solver; the subsequent
successful run executes the real solver. Screenshots were inspected, including
the corrected same-name scene rows. Native and WASM builds and formatting pass.
Actual PDB2PQR execution and OS-level Tauri input are not newly exercised by these
checks; the external-stage process tests use small executable fixtures.

## Explicit Morph conversion and frame editing

Native and browser windows now share a dedicated Morph animation tool; Tauri
provides the corresponding controls. Opening the form selects the first eligible
molecule without changing the scene. Conversion is explicit. The frame list
protects `(this)`, inserts after the selected row or appends without a selection,
and clears selection after deletion. Add mol excludes existing MorphMol objects
and objects outside the active scene. PDB/ENT batch import accepts gzip files,
records each successful addition separately and stops at the first error while
retaining earlier additions. Errors stay in the form, and Close retains edits.
Name-column resizing and its saved width remain available.

The core audit also found that adding a frame to an uninitialized molecule omitted
its base coordinates, while explicit conversion left its runtime class as
MolCoord until scene serialization. Both paths now initialize the protected base
and update the class immediately. Existing object and renderer IDs remain stable.
The command catalog incorrectly promised a new object UID; that description now
matches the Rust implementation. Preserving IDs is a deliberate difference from
the reference service, which replaces the object during conversion. Unnamed
object-source frames now inherit their source object's name instead of `frame`.
Native workspace rows, camera visibility entries and the shared scene-state API
also now report the actual molecule class; their fixed MolCoord labels hid the
conversion in the native and browser object trees.

Browser PDB parsing uses the existing worker and PreparedFile boundary without
creating a temporary scene object. Import cursors retain ordered insertion across
files. Dialog generation, scene, target and revision checks discard results after
closure, replacement or another edit. Tauri supplies an expected scene to its
model requests and rejects stale targets before mutation. Existing callers that
omit that optional field retain their lower-level morph command behavior.

Validation: nine prepared-file tests, seven command-option tests, 224 native model
tests and 193 Tauri model tests pass. Five existing Tauri tests remain ignored.
Three browser cases cover WebGL/WebGPU editing, compressed batches, failures,
undo/redo, target changes, cancelled pickers and parsers, and a parse superseded
by Undo. Two Tauri DOM cases cover explicit conversion, protected frames, filtered
sources, ordered request payloads, saved column widths, partial failure and stale
picker replies. The DOM cases use a mock IPC layer; model tests exercise the Rust
handlers separately. Native and WASM builds pass. Early test assertions raced a
batch status update or selected both Close buttons; explicit completion checks and
the intended footer selector resolve those test failures.

Linux OS input against the rebuilt native binary confirms that opening the form
does not mutate the scene, conversion is explicit, the base cannot be deleted,
adding a molecule frame preserves the camera, and conversion/addition/deletion
undo and redo with the original object and renderer IDs. Screenshots were
inspected. This check also exposed an unsupported dropdown glyph, now replaced
with the shared icon font. Native multiple-file insertion is covered by the host
batch regression; the OS case exercises the molecule-source path.

## Native and Tauri New Renderer controls

Native egui now uses the browser's shared renderer form. Tauri mirrors its
compatible types and preset groups, editable object-name field, selection builder,
name following, group context, recenter default and Auto/preserve-view/fit-map
choices. Presets disable selection and cannot be nested inside another group.
Only successful creation updates type and selection history. The reference saves
history before dispatching creation; retaining the previous history on a failed
operation is a deliberate failure-safety difference.

Both hosts now use the shared create command. Creation and group assignment form
one undo step, failed drafts keep their input and inline error, and missing or
changed targets are rejected. Tauri validates the expected scene and object class
in the model, discards late option replies after cancellation/replacement, and
fits the camera inside the creation request instead of sending a second request.
Normal scene undo retains the current camera, matching the existing history
policy; failed creation leaves the camera unchanged. Native group creation keeps
its dedicated name form and uses the shared group command.

A native regression exposed an empty-lightweight-object issue: recentering tried
to decode absent QDF data after adding a renderer and rejected the entire action.
Creation now leaves the camera unchanged when the object has no geometry. Invalid
nonempty geometry still reports an error. Preset naming also now follows the
reference's short lowercase prefix (`Default1RendPreset` becomes `default1_1`),
including custom presets without that suffix. Scene-wide collision checks include
lightweight objects and auxiliary renderer groups. The old `<type><number>`-only
source comment was corrected. Tauri file options use the same naming rule.

Tauri's Inspector previously displayed a renderer-type selector that its Add
handler never read. It now exposes a single New Renderer button that opens the
actual options form. The obsolete native per-kind form and creation branches
were removed. The usage and status documents no longer limit the dedicated form
to the browser.

Validation: 222 native model tests, 192 Tauri model tests (five existing tests
remain ignored), six shared command-option tests, four browser renderer cases and
four Tauri DOM cases pass. Browser cases include WebGL/WebGPU creation, map/surface
groups and presets. DOM cases cover Inspector entry, history and name editing,
selection building, error retention, disabled preset selection, group restrictions,
map-policy payloads, stale scenes, cancelled asynchronous replies and file/preset
name collisions. Linux OS input against the current native build confirms invalid
selector retention, selection-builder use, corrected creation, selected-atom
framing, single-step undo/redo and reopening with saved history. Native and Tauri
screenshots were inspected. Formatting and whitespace checks pass. An initial
DOM assertion used an absent `h2` instead of the actual dialog-title element; the
corrected case passes. An initial model expectation attempted to undo camera
framing despite the established camera-preserving scene history policy; the test
now checks that policy explicitly.

## Browser renderer setup and scalar-map centering

The browser scene tree now opens New Renderer from objects, renderers and groups.
The shared form restores class-specific type/preset history, proposes names across
the scene, follows type changes until a manual edit and re-arms after clearing a
name. It starts from the molecule's current selection, includes the shared
selection builder and disables selection for presets. Group entry points inherit
their group and omit presets. Auxiliary-object group rows now appear in the tree.
The editable object-name field is ignored when attaching to an existing object,
matching the reference's actual attach flow.

Creation uses one command and one browser undo entry, including group membership
and preset children. A shared target description resolves the object's scene,
class, compatible options and existing groups. Missing groups, nested presets and
invalid molecular selectors are rejected before renderer creation. Failed browser
commands retain the draft and restore scene/camera state. Scene changes discard
the dialog; missing objects and renamed/removed groups disable submission. Only
successful creation saves class history. Cancelling leaves it unchanged.

The new UI check exposed an existing scalar-map discrepancy: the shared centering
helper returned early for ElePotMap, even with an explicit Move view center choice.
The reference's `emDefaults.ts` also deliberately ignores ElePotMap despite its
renderer form exposing map-center choices, and `createRendererOnObject.ts` does
not invoke the map policy helper. Rust now honors explicit policies for potential
maps, with Auto retaining the view. Only the new renderer receives the preserve-view
center; fitting a map leaves all previous renderer centers unchanged. This is an
intentional functional difference from the current reference. Map fitting uses
the full grid extent, not the average of voxel sample coordinates.

Renderer framing now uses the new molecular renderer's own selection and preset
membership instead of falling back to the molecule's current selection. Shared
regressions cover all-atoms creation with an unrelated current selection and a
restricted renderer. The public command description formerly limited centerView
to MolCoord even though the implementation also handles surfaces and lightweight
objects; the schema now describes the implemented behavior and the map override.

Validation passes: the shared form's two model tests, five command-option tests,
ten map tests and 220 native GUI model tests. Four cases pass against the final
browser artifact: molecular creation in WebGL and WebGPU; map/surface creation,
explicit centering and auxiliary groups; and actual preset creation with one-step
undo and remembered defaults. The cases exercise selection building, invalid
input retention, manual-name preservation and clearing, cancellation, scene
changes and group inheritance. Browser screenshots were inspected. Initial
harness errors used a nonexistent query tool and sampled status before startup;
these were corrected. A map expectation also initially used voxel sample centers
instead of the established grid extent.

At this stage native/Tauri renderer form controls remained open; the subsequent
cross-frontend entry above supersedes that limitation. The old browser
quick-add-only limitation is superseded by this entry.

## Renderer history per object class

A shared configuration record now remembers renderer types and presets by the
actual object class. File metadata distinguishes DensityMap from ElePotMap,
including compressed OpenDX and QDF potential data. Native/Tauri New Renderer
uses the selected object's class, shares its choices with file options and
ignores incompatible or removed presets. Without valid history, molecular files
retain Automatic representations; explicit renderer creation starts with simple.
Names are proposed across the scene and follow type changes until manually edited.

File confirmation records the offered choice before parsing, matching the current
reference hook. Cancel leaves history untouched. Failed parsing keeps the scene
unchanged while retaining the confirmed preference; types not offered by the
selector are not recorded by the Tauri file request. Scene Undo/Redo preserves
application preferences, and normal configuration persistence restores them on
restart. The browser file form also retains history when no scene exists yet.

The focused shared-core and shared-form tests pass. Native model tests pass 220
cases, and the macOS Tauri suite passes 191 with five existing ignored cases.
Three Tauri DOM cases verify per-file options, map/AMBER settings and class-aware
renderer requests, selected defaults and manual-name retention. Linux OS input
passes startup selection from stored history, two-file acceptance/cancellation,
Undo/Redo and persistence of independent molecule/map choices. The browser
baseline passes 11 existing file cases. Both new WebGL/WebGPU history cases pass
after correcting the harness to scroll hidden menu items and resolve the named
object instead of assuming list order or renderer fields on map summaries.
These cases also verify browser reload. Native, browser and Tauri screenshots
were inspected. The native constructor now starts with the shared configuration
path; tests use a temporary directory, so saving history cannot create a config
file in the source package.

The old single desktop renderer preference and the browser's unconditional first
choice did not implement the reference's per-class history. The old persistence
test name claimed reference behavior but only checked the legacy file round trip;
it now describes that limited scope. The usage guide previously described
Automatic as unconditional and now states the no-history default. At this checkpoint the dedicated browser New Renderer form was still absent;
that limitation is superseded by the later browser renderer setup entry above.

## Molecular surface forms, naming and read-only queries

The native and browser frontends now share generation, regeneration and plane-cut
forms, and Tauri exposes the same controls. Generation resets selection and probe
inputs on open, defaults density to 2, and proposes a unique `sf_<molecule>` name
on open or molecule change. Its selection builder is gated by Use selection.
Density is an integer from 1 to 10; probe radius ranges from 0.1 to 10 Å.
Generation and regeneration remember their backend independently within the
session. Regeneration preserves original molecule, selection and probe radius;
Auto leaves the stored backend unchanged. The cutter resets to Complete and
density 5, accepts densities from 0.1 to 50 and uses the current front slab plane.
Errors retain the draft, Cancel does not mutate, scene changes invalidate drafts,
and successful operations close with one scene undo entry.

Reference worker inspection found two additional generation differences: renderer
names now follow `molsurf`, `molsurf(1)`, etc., and new surfaces explicitly use
CPK element coloring instead of inheriting the molecule's Paint scheme. The CPK
carbon color is pale yellow (1, 1, 0.75), as specified in `CPKColoring.qif`.
A rendered-vertex regression checks that the molecule's explicit green coloring
does not replace that default. Cut-section names now start at `sect_<surface>`;
collisions advance to `sect1_<surface>`, including names used by other object
classes. Ordinary suggested object names also account for surfaces and lightweight
objects.

The browser undo test found that a read-only JavaScript query normalized every
loaded surface's metadata and renderer nodes, creating an extra undo entry.
Unchanged surface state now bypasses that write. The regression checks plain
queries, surface properties, raw stored state, later renderer/metadata edits and
reads after those edits. Browser tests read generation properties between edits
and Undo/Redo, so an extra query entry fails the real interaction check.

Supplying the optional reference fixture also exposed three previously skipped
Tauri checks. The legacy lowercase superposition method is now canonicalized
before shared-command validation. Sequence selection setup now uses the current
per-molecule state. The picking check distinguishes a visible clipped sphere cap
from geometry fully outside the slab instead of treating a clipped atom center
as an invisible sphere.

Validation passes: 219 Linux native model tests, 190 macOS Tauri model tests
with five existing ignored cases, 35 shared-core surface tests with scripting
explicitly enabled, and the shared form reset/session test. Five browser cases
on the final artifact cover WebGL/WebGPU forms, Tauri DOM controls, both analytic
backends and scripted scene regeneration. Linux OS input verifies failed-input
retention, generation/regeneration/cutting, separate-object naming, read-only
queries and one-step Undo/Redo. Native, browser and Tauri screenshots were
inspected. A default-feature core run does not enable scripting; it cannot
validate the read-only-query regression.

Documentation/source discrepancies found in the read-only reference: the
`MakeMolSurfDialog.tsx` header describes a plain probe-radius input, but its JSX
uses SliderField; `molSurfBackend.ts` describes SegmentField, but both dialogs
use SelectField; the regeneration service header mentions only density while
its current request also accepts a backend override. The implementation follows
the executable code. The reference checkout remains unchanged.

## Molecular superposition dialog and failure preservation

Native egui and the browser now share the current Molecular superposition form;
Tauri exposes matching controls. US-align is the default, with Auto (CA / C3')
pivots, empty selections, Auto recenter on and xformMat storage off. Method and
pivot choices are validated, selection builders use shared history, and inactive
pivot/report controls cannot affect an unrelated algorithm. Only successful
choices persist under `cuemol2.ui.histories.mol_superpose`. Reopening clears both
selections; cancellation and failures retain the previous saved settings.
Scene changes close or reject stale drafts. Native/Tauri retain optional LSQ
RMSD-report export without adding it to the browser.

All three C++ algorithms allow the reference and moving roles to use the same
molecule. The shared Rust preparation now does likewise, using independent
world-coordinate copies before committing the moving transformation. Synthetic
chain tests exercise LSQ and US-align with coordinate and property storage;
the real 1CRN fixture also exercises same-object LSQ, SSM and US-align.
Auto recenter frames the selected moving atoms, not unrelated distant atoms.
A browser reproducer found that empty selections were normalized for fitting
but rejected by framing; framing now uses all atoms consistently. Recenter is
best effort after a successful transformation, matching the reference worker.

A failed RMSD-output regression exposed snapshot restoration marking all
molecules modified while preserving their allocated atom IDs. Restoration now
retains each molecule's original modified flag. Clean and dirty snapshot tests
cover this independently; failed superposition/report output preserves state,
camera and history. Accepted changes record one undoable action; no-change
operations do not add scene history.

Validation passes: 187 shared-core tests with the reference checkout present,
218 Linux native GUI tests, 218 macOS native model tests, 189 macOS Tauri model
tests with five existing ignored cases, and 67 shared command tests with two
existing ignored cases. The focused empty-selection command regression and
shared history-default test also pass. Two browser cases on real WebGL/WebGPU
cover invalid selections, CA and RNA P pivots, selected-region framing,
xformMat, Undo/Redo, same-object LSQ, reset-on-reopen, cancellation and stale
scenes. The Tauri DOM case passes. Linux OS input verifies inline-error retention,
a successful property transform, one-step Undo/Redo and saved settings.
Native, browser and Tauri screenshots were inspected.

The old native form defaulted to LSQ/name-CA, retained draft selections and
framed the whole scene. The browser opened a generic form, and native/Tauri
omitted pivot selection. The shared core rejected same-object fits even though
the C++ reference permits them. These differences are corrected. The old core
method comment listed only LSQ/SSM despite existing US-align support; its wording
now matches the implementation. The guide and checklist record the dedicated
form while retaining the remaining morphing, surface and rendering audit.

## Options before every object-file open

Following the explicit user choice, native egui, browser and Tauri file opens
now display setup for every supported object file. This includes ordinary
molecules, command-line inputs and dropped files. Scene, camera, ICC and
trajectory inputs retain their dedicated flows. Each batch waits for acceptance
or cancellation before advancing. Automatic molecular representations remain
the initial choice, so accepting the defaults creates protein ribbons,
nucleotide cartoons and ball-and-stick for other atoms. Automatic mode disables
the single renderer-name field; explicit representations keep it editable.
Object names remove the final filename extension and receive unique suffixes.

The native suite passes 217 cases, and the Tauri model suite passes 188 with
five existing ignored cases. Linux OS input confirms deferred startup,
automatic ligand rendering, two-file acceptance/cancellation and Undo/Redo.
The existing XDND destination/failure/redo check also passes after adapting it
to the per-file options flow. Native, browser and Tauri screenshots were
inspected. The 315-case browser baseline passed 312 and exposed three outdated harness
assumptions: dragging before options were accepted, the old generic molecule
edit form, and the old generic symmetry form. The harness now waits for field
focus, accepts options before releasing the parser, enters an explicit deletion
selection and uses the symmetry pane. All three failed cases pass in the eight
focused loading, file-option, navigation and shell reruns. The loading regression
also passed three consecutive isolated runs before the final suite.

The former guide described immediate ordinary molecule loading and optional
setup. It now describes the selected every-open behavior; the earlier validation
entry is explicitly historical. Existing browser fixtures deliberately set
object names to their previous test targets while accepting the new dialog.
The native OS checker also accepts startup options before waiting for objects.
These harness changes preserve the existing behavior assertions. Per-object-class
renderer history remains a separate open item.

## Secondary-structure dialog

Native egui and the browser share the Reassign secondary structure form. Its
fields match the current reference dialog: Recalculate / Assign, retained valid
molecule, reset selection and parameters, Ignore beta bulge, optional gap-fill
angle (default 120 degrees), and five secondary-structure types. Each frontend
provides the selection builder, ignores inactive-mode fields, keeps failed edits
visible and rejects stale scene targets. Empty selection means the entire
molecule. Reference worker services return success for a valid zero-result
assignment; this closes the form without creating empty history.

The shared core now rejects non-finite or out-of-range recalculation angles
before changing molecular properties. This matches the existing Tauri boundary
and applies to native, browser and command callers. Manual assignments also
clamp the neighboring-residue range at integer boundaries, preventing overflow
for supported minimum/maximum residue indices; a focused molecular-structure
regression covers all five types at both boundaries. Tauri and native avoid undo
entries when residue properties remain unchanged. Actual assignments and
recalculations retain one-step Undo/Redo. Successful manual selections enter the
shared history. Native parser errors no longer open a second modal above these
inline forms; the same correction applies to the interaction-analysis form.

Validation passes: 185 shared-core tests, 217 native GUI tests, 188 Tauri model
tests with five existing ignored cases, two browser cases spanning real WebGL
and WebGPU, and one Tauri DOM case. These cover malformed and empty selections,
all five types, mode gating and reset, angle bounds, remembered molecules,
selection building, scene guards and Undo/Redo. Browser and Tauri screenshots
were inspected. Linux OS mouse/keyboard input confirms invalid selection
retention without a second modal, manual assignment and its Undo/Redo, geometric
recalculation and its undo, reset-on-reopen and cancellation. Native screenshots
were inspected. The OS fixture uses separated residue indices because the
reference intentionally clears conflicting assignments on neighboring indices.

The earlier browser entry opened a generic command form. Native fields were
retained across reopening, used a different title, omitted selection building
and kept successful forms open; native/Tauri also recorded empty undo steps.
Tauri defaulted to the active selection and closed after failures. These gaps
are corrected, and the guide/checklist now distinguish secondary structure from
the remaining transformation and surface dialogs. The previously documented
immediate molecule-open default was subsequently replaced by the reader-options
change above, following the explicit user choice to show options every time.


## Molecule edit dialogs and residue-index bounds

Native egui and the browser now share dedicated Merge molecule, Delete mol atoms,
Change chain ID and Change residue index forms. Tauri uses matching fields and
validation. All reset selections and options while retaining valid molecule
choices, provide selection builders, reject stale scene/object targets and keep
failed edits visible. Merge supports copy or move under one undo entry. Delete
requires an explicit selection. Chain and residue changes accept an empty
selection as the entire molecule.

Chain input matches `chainNameInput.ts`: empty is invalid, whitespace converts
to `_` after confirmation, and a trimmed value longer than one UTF-16 code unit
requires a PDB-format confirmation. Residue input matches `resIndexInput.ts`:
decimal-prefix parsing, nonzero shifts, Shift by / Start from, Renumber, and
confirmation outside the PDB start-number range. Unlike unchecked C++ integer
arithmetic, Rust checks both the parsed value and every destination residue
number. Overflow or an unselected destination collision is reported before any
atom changes. The legacy boolean manager methods retain their signatures and
return false on rejection; the shared application layer propagates the error.

Reference worker services were also checked. Chain changes with no matching
residues complete successfully, while an occupied destination is an error.
Delete and residue changes reject no-result operations. The frontends now match
those outcomes. A chain no-op does not create an empty history entry. Failed
copy/move, deletion, chain or residue edits preserve the molecule and history.

Validation passes: 184 shared-core tests, 31 molecular-analysis tests, two
shared input-resolution tests, all 216 native GUI tests, and 187 Tauri model
tests with five existing ignored cases. Six browser cases use real WebGL and WebGPU applications to
exercise every form, invalid and empty selections, collision/overflow errors,
confirmation cancellation/acceptance, decimal-prefix parsing, copy/move,
Undo/Redo, remembered molecules and scene cancellation. The Tauri DOM case
checks matching controls and command arguments, selection building, inline error
retention and stale scenes. Linux OS mouse/keyboard input confirms blank-chain
confirmation/No/Yes, chain Undo/Redo, residue-prefix parsing, residue undo,
empty-result deletion preservation and deletion Undo/Redo. Native, browser and
Tauri screenshots were inspected.

The earlier browser Edit entries opened generic command forms. Native/Tauri
chain and residue dialogs required a nonempty selection, used different field
resets and lacked the reference confirmations. Tauri closed these forms even
when a command failed. Existing residue arithmetic could overflow before
validation, and chain collisions were reported as zero changes. These gaps are
corrected. The guide and current checklist now distinguish these completed
forms from secondary-structure, transformation, surface and rendering dialogs
that remain under audit. Historical parity rows retain their original scope.

## Interaction analysis dialogs

Native egui and the browser now share a dedicated Interaction analysis dialog.
Tauri exposes the same defaults, second-molecule/selection gates and validation.
All use the reference defaults: 2.5–3.5 Å, 30 labels, hydrogen-bond filtering off
and the `measure` label set. Reopening resets the conditions while retaining
valid molecule choices. Selection builders edit the form without changing the
molecule's current selection. Successful expressions enter selection history.

A distinct second molecule activates the second selection even when its switch
is off. Selecting the first molecule again only uses a second selection when
that switch is on, matching the reference service. Invalid distances disable
acceptance. Invalid expressions and empty results stay in the dialog and do not
change the scene or history. Successful analysis appends to the named renderer,
closes the form and creates one undo entry. Scene changes reject stale drafts.

The native GUI suite passes 214 tests, including cross-molecule selection and
empty/invalid analysis regressions. Three browser cases pass on WebGL/WebGPU,
covering real analysis, renderer creation, Undo/Redo, selection history,
same-molecule dispatch and scene cancellation. The Tauri DOM case covers
selection building, default/reset values, gating, invalid inputs, inline errors,
same-molecule arguments and stale scenes. Linux OS mouse/keyboard input also
confirms empty-result preservation, one label, Undo/Redo and reopening. Native,
browser and Tauri screenshots were inspected; the label-set field's width was
corrected after the first visual check.

The earlier native/Tauri forms used 1.5 Å, 100 labels and hydrogen-bond filtering,
and ignored selection 2 unless its own switch was on. The browser menu opened
the generic command form. Those interfaces did not match the current reference
dialog, despite the shared command already supporting interaction analysis.
The guide now documents the dedicated forms; the broader dialog audit remains
open. The command catalog's own defaults are unchanged.

## Browser reader options and native deferred imports

At this stage, native egui and the browser introduced optional file setup. The browser dialog
covers PDB/mmCIF flags, renderer/preset selection and names, selection, map-center
policies, CCP4 strides/kinds/limits, MTZ columns and companion files. Required
NAMD/MSMS companions open the dialog automatically. AMBER companions are optional.
Worker metadata and parsed results are guarded by request generation and source
scene. Invalid reader/renderer choices roll back the load and batch destination;
successful imports form one history entry. Native reader sections now collapse
and reset, and action buttons stay outside their scroll area.

The native drop/startup queue retains its batch while a companion dialog is
open. Ordinary molecules initially loaded immediately; the later per-file
options change above replaces that default. Maps receive a contour
renderer and the shared Auto policy. Cancelling options skips that file, and
changing scenes discards the pending companion batch. Tests cover preserved
first destinations, no insertion before acceptance, cancellation, stale scene
changes, map renderers and undo/redo.

All 184 default-feature shared-core tests pass, including the load-failure
integration tests. The native suite passes 212 cases; the later stale-scene
regression passes with all six focused drop cases. The rebuilt browser passes
23 reader/loading/destination cases across WebGL/WebGPU. Three MTZ cases also pass after the compressed-reader follow-up, including
column selection, map creation and one-step history. The real Linux XDND check
confirms renderer creation, EM centering, visible contour pixels, one-step
Undo/Redo and deferred companions. Native companion and browser molecule/map screenshots were inspected.

CCP4 inspection reads its bounded header. Uncompressed MTZ inspection skips the
reflection body. Compressed MTZ metadata decodes twice and retains the prefix
and header tail; six MTZ unit tests cover gzip/bzip2/xz, metadata, full read parity
and truncated streams. A browser regression inspects a compressed MTZ containing
a 64 MiB decoded reflection region while keeping WASM linear memory below that
size. Compressed CIF inspection discards preamble chunks and recognizes table
tags across chunk boundaries. The sampled CCP4 transfer test also checks the
actual stored grid dimension.

The previous guide said browser reader options were unfinished; the new guide
describes the implemented controls and companion behavior. The earlier drop
checks covered molecules and surfaces, so they missed maps inserted without a
renderer and companion files bypassing setup. This audit also found compressed
MTZ metadata and direct reads using raw bytes; both now handle compression. A
load-failure test literal had missed the earlier map-center policy field and now
compiles with the complete shared-core suite. Renderer history by object class
and the remaining dialog comparisons stay open.

## Shared file-open map centering

Native egui and Tauri now offer Auto, keep-view and fit-map policies. Auto
resolves after reader map-kind overrides: EM fits the loaded volume; crystalline
map regions follow the existing view. The shared typed policy also backs command
loading and renderer creation. Unit-cell renderers do not move the camera.
Legacy callers without a policy retain their previous recenter behavior.

Four reader tests pass, including all map-kind/policy combinations and portrait
viewport fitting. All 209 native tests pass; an extended native load/undo test
also passes. All 186 Tauri model tests pass with five optional cases ignored.
The Tauri load regression covers legacy requests, explicit Auto/fit behavior and
one-step undo. The focused DOM case checks the default Auto selection and
explicit fit submission alongside map-kind and stride choices. All 69 command
unit tests pass; two optional tests remain ignored.

The earlier file-options implementation only represented map centering as a
boolean, and the guide therefore lacked the current reference Auto behavior.
The native interfaces and guide now describe the resolved policy. Browser
file-option and remaining dialog comparisons stay open.

## AMBER companion coordinates and map-reader controls

The shared file setup now exposes optional AMBER coordinates, CCP4/MRC map kind
and bounded header metadata. Native egui and Tauri pass these choices to the
readers before creating renderers. Header inspection reads only the prefix;
storage estimates round each sampled grid dimension separately. Large-map
suggestions remain bounded to strides 1, 2, 4 and 8.

Three shared tests cover AMBER coordinates, linked/embedded scene round trips,
failed-load rollback, map-kind effects on renderer regions, map save/reload and
stride estimates. The AMBER test includes compressed topology input and relative
companion paths. The existing 209 native GUI tests pass. The Tauri suite passes
185 cases with five optional tests ignored; an added model test also passes for
AMBER input, coordinates and one-step Undo/Redo. The focused Tauri DOM test
checks coordinate-path submission, map kind, metadata and stride warnings. Its
final screenshot was inspected.

The audit found that the QSC AMBER callback ignored `subPath_coord`. It now
loads that companion relative to the scene when necessary. New file imports
record a resolved companion source and the topology compression mode so later
scene saves do not silently relocate a relative coordinate path. The previous
guide omitted these reader controls; it now describes them and explicitly
identifies browser options as unfinished. The later map-centering checks above supersede that part of this snapshot;
the remaining reader/dialog comparisons are still open.

## Component Catalog examples

The native and browser frontends share three collapsible Catalog panes. Tauri
exposes the corresponding form, list and property examples. Controls include
password/invalid/disabled fields, numeric drags, time/vector editing, selection
builders, color, sliders, gated controls, tree/list rows, segments and radio
groups. Sample values remain local to the gallery. Editing a property to its
default value still marks it modified; only Reset clears that flag. The isolated
property rows and the mock Inspector use independent sample state.

Shared property/reset and scene-isolation checks pass. All 209 native GUI tests
pass, and the Linux plugin scenario opens the real Catalog window without
changing scene objects. Six WebGL/WebGPU plugin/category/Catalog cases pass.
The full Tauri DOM run passed 61 cases and exposed two ambiguous accessible
selectors after the gallery grew. Explicit pane names and visible reset-button
selection fix those failures; the two affected cases pass. The Tauri gallery
checks numeric dragging, gating, local property flags, separate mock resets,
list selection, pane collapse and activity-switch persistence. Screenshots from
the native window and Tauri DOM were inspected.

The previous usage guide described only a generic forms/lists/properties sample;
it now describes the three panes and sample-state/reset behavior. Remaining
dialog comparisons are tracked in the implementation checklist.

## Shared settings categories and native Console access

Native egui and the browser share the category tree and its settings controls;
Tauri exposes the same groups and search. The shared `Preferences` model replaces
the duplicated native/Tauri mapping and restores label styles and input values
from configuration. Validation happens before any preference changes. Explicit
plugin settings retain precedence over legacy enablement. The controls cover
theme, labels, mouse/picking, files, tool paths and force fields. Native dialogs
keep an apply/cancel draft; browser controls save immediately. Configuration and
startup-script editing remains available in the native egui and browser category.

Native startup checks now honor the saved option, run asynchronously after a
short delay and notify only for a newer version. Manual checks report absence
or failure; installation remains an explicit user action. Tauri and egui share
the update runtime. Its injected task tests check disabled startup, silent
no-release results, notification and operation exclusivity. No actual installed
binary was replaced during this validation.

`cuemol console --connect` uses an independently enabled authenticated Console
endpoint. The listener advertises only enabled endpoints; enabling/disabling
Console keeps an active MCP endpoint and token. Requests recheck current access
on the UI/model thread. Submitted commands preserve native input drafts, use
one GUI history transaction and inherit the client's working directory.
Disabling access cancels remaining commands. Tauri avoids emitting unchanged
scene snapshots while a remote command waits for background work.

The shared settings round-trip and atomic-validation tests pass, including
external paths, force field, theme and update/Console switches. The transport
checks cover unauthorized requests, isolated endpoints, independent toggles and
listener lifecycle. Native GUI tests pass (209), including remote submissions,
input preservation, one-step undo and cancellation. All 185 Tauri model tests pass; five optional tests remain ignored. The full
61-case Tauri DOM suite and 13 affected browser cases passed before the final
theme fix. After that fix, three focused WebGL/WebGPU/Tauri cases pass, covering
theme controls, reload, cross-category search, tool-path drafts and manual
update status. Nine final protocol/listener tests also pass. Screenshots from
all three frontends were inspected. The final native GUI binary builds.

Linux OS input confirms theme cancellation/persistence, update preferences,
CLI edits and one-step undo, relative working-directory propagation, disabling
Console while MCP stays usable, and category search. The updated Picking and
Plugin OS scenarios also pass. Native light-theme/search and category screenshots
were inspected. Native screen/input checks on macOS remain pending permissions.

The comparison found that the old Options Misc/Key/Mouse navigation no longer
matched the current reference. The usage guide now describes categories and
places plugin switches under Installed. The old native OS probes used tab
coordinates; they now operate the category UI. The settings migration also
exposed an old browser theme-menu path that only wrote its legacy storage key;
it now updates the shared configuration, invalidates stale drafts and survives
reload. A layout regression inherited a horizontal layout in both columns;
explicit vertical layouts and a bounded-column test keep controls on screen.
The reference blendpng path is intentionally replaced by the existing built-in
composition; the native/browser help now states that behavior. Remaining dialog comparisons are still open.


## Shared Agent runtime and Tauri conversation interface

The provider, conversation, download, cancellation and model-list state moved
from the egui panel into `cuemol-ui::agent_runtime::AgentSession`. The optional
`console-runtime` feature exposes this code without egui. The native and browser
panels retain their existing adapters; Tauri now uses the same session and
provides model/settings controls, transient or OS-stored keys, a transcript,
image thumbnails, prompt recall, Stop and Clear chat. Provider keys are absent
from snapshots and saved settings. The reference transcript is plain text and
collapses tool input/results by default; both UI implementations follow it.

Tauri model callbacks wake the model worker and refresh the conversation without
forcing a viewport redraw for every text event. Scene changes refresh the shared
snapshot. Image renders and APBS run in workers. A turn adds one scene-undo entry;
manual scene edits or disabling the plugin stop it. Delayed download and APBS
results cannot affect a later turn. Tauri shutdown requests cancellation of
background work and stops its MCP listener before the model thread exits.
Native egui and Tauri now load Agent settings beside an explicit TSV config path,
and a stored plugin choice overrides legacy Agent enablement.

The composer supports Ctrl/Cmd+Enter, optional plain Enter, Shift+Enter newlines,
IME confirmation and a 50-entry distinct prompt history. Native/browser input
queues preserve text before a send/recall key. Focus and multiline cursor tests
cover repeated recall, restoring an unsent draft and retaining caret movement.
The browser checks send through the actual canvas on both WebGL and WebGPU,
then reload and recall the saved prompts without persisting the API key.

Two shared composer tests, all 208 native GUI tests and all 184 Tauri model tests
pass. Five optional Tauri tests remain ignored in the normal model run. The
Tauri Agent tests use loopback provider/structure servers and a mock APBS
executable; they exercise multiple edits, PNG image results, one-step undo,
manual interruption, plugin disablement and cancellation followed by a new turn.
The full 61-case Tauri DOM suite and six browser Agent cases pass. The final
eight focused browser/Tauri checks pass after the transcript and composer
refinement. Browser egui and Tauri DOM screenshots were inspected; the Tauri composer and
status now fit above the application status bar. The shared egui transcript
uses the remaining panel height while reserving room for its composer. Its
three affected browser image/composer checks also pass after that layout change.

The earlier implementation checklist correctly identified the Tauri Agent
interface as missing; the prior usage guide described only native egui and the
browser. Those sections now cover Tauri and distinguish settings storage across
frontends. On the authorized macOS host, both screen-capture and accessibility
preflight checks still return false. Model/Metal results do not establish an
actual macOS window-input pass, and no live paid-provider request was made.


## Built-in plugin preferences and GUI MCP lifecycle

The current reference's compiled plugin registry supplies defaults and permanent
contributions. Get PDB and Sequence remain enabled. Console, PyMOL and ChimeraX
start enabled; MD Tools, Agent, Catalog and MCP start disabled. Shared settings
apply plugin maps atomically. Explicit old `mcp:enabled` values remain readable,
and explicit new Agent preferences override legacy `agent.json` enablement.

Native and browser shells filter activity buttons, menus, tabs and console
dialects. Hiding the selected contribution falls back to Explorer or Output.
MD data remains loaded when its controls are disabled. Console cancellation
does not disable Action Library. Pending agent frames and APBS results retain
the cancellation token of their originating turn. The browser omits the native
MCP listener switch. Tauri separates its Output and Console tabs and keeps
plugin preferences in its existing cancellable Options dialog. A regression
found CSS overriding the hidden Catalog button; its hidden state now wins.

Native and Tauri GUI MCP listeners share start, stop and port configuration.
Tauri calls record edits in its GUI history, preserving preferences and view
navigation. Both frontends validate initialization, IDs and argument schemas
before dispatching special GUI operations. Shared responses carry image content
as MCP image blocks. A failing loopback test reproduced a disabled listener
waiting for a pending result. The result wait now observes shutdown, returns
503 to that request and releases the listener. Stop/restart creates a new token
and removes the old connection descriptor. A port conflict reports the failure
without changing the saved enabled preference or retrying on every UI frame;
changing the port or toggling MCP retries the listener.

Tauri MCP APBS runs through the existing process worker. It imports the result
into the originating molecule's scene even after a scene switch, with one undo
entry. Disabling MCP or cancelling the job rejects the output, including an
already completed output that has not yet been applied. The same late-output
guard is applied to native egui APBS. Mock executables cover success, scene
switching, undo and cancellation without external services.

Native MCP edits and asynchronous APBS output restore active numeric previews
before capturing history. The native density-drag regression verifies that a
remote edit survives pointer release and that undo restores the committed value.
Tauri also tags numeric requests with a remote revision, rejects queued preview,
commit and abort requests after an MCP edit, and tolerates a removed preview
target. Its model regression verifies the original default value after undo.

The shared plugin and ConsoleRunner tests, nine protocol/lifecycle cases,
207 native GUI tests and 180 Tauri model tests pass. Five optional Tauri tests
remain ignored in the ordinary model run. WebGL and WebGPU plugin scenarios
check saved switches, dialect retention and fallback after reload. The affected
135 browser regressions passed before the final cancellation-token refinement.
The final ten focused browser/Tauri DOM checks include delayed indexed-frame
cancellation across a new turn and numeric-preview interactions. Linux OS
input tests cancel/apply plugin settings, inspect saved preferences, disable
MCP while checking its listener, reconnect and retain the active scene. Catalog
and Options screenshots were inspected. Run the native window check with
`scripts/check-native-gui.py --plugins-only`.

The usage guide previously described the GUI endpoint as always available and
still quoted an older 124-operation catalog. It now explains enabling the MCP
plugin and reports the current 129 registrations/127 MCP operations. The Agent
instructions also now include enabling its plugin before opening the panel.
Category navigation, full catalog examples and the Tauri Agent interface remain
separate open requirements; these checks do not establish complete parity.

## Picking preferences and scene-history separation

The existing GPU picking pass had no settings control. A failing core regression
also showed that loading a stored GPU preference left the runtime enabled.
The shared config setter now updates `ViewInputConfig.gpu_pick`, so CLI/browser
config loads and all three GUI preference editors use the same runtime value.
Explicit later script changes remain possible without rewriting the saved key.

A Tauri model regression reproduced scene undo reverting the newly accepted
preference. Native, browser, Tauri and MCP scene-history restoration now retains
current application configuration and the view-input service. Transactional
rollback still restores its complete snapshot. Tauri also exposes both hover
switches, avoids disabled hover requests and rejects an in-flight hover image
after disabling feedback. Disabling hover leaves click picking available.

All five config round-trip tests, nine MCP protocol cases, 204 native GUI tests
and 177 Tauri model tests pass; five optional Tauri tests remain ignored in the
ordinary model run. The Metal geometry test was then run explicitly and passed:
GPU-off picking misses sphere edges outside the atom-center radius, centered
clicks still work, highlighting can be disabled, and hover-off clears feedback
without rebuilding the scene. The sandbox could not enumerate a Metal adapter;
the explicit GPU run used normal device access.

WebGL and WebGPU scenarios exercise actual settings toggles, geometry versus
atom-center picking, hover suppression, live script values, restart persistence
and undo/redo. The Tauri shell checks cancellation, saved controls and absence
of hover requests. The 62 affected browser scenarios pass after focused reruns;
an initial HiDPI fixture failure was corrected by keeping the new preference
fixture limited to the picking test. Linux OS input accepts/cancels the native dialog and checks
its saved file and runtime after undo/redo. Its screenshot was inspected.
Run `scripts/check-native-gui.py --picking-only` for that window check.

The browser settings window now scrolls within the available height; the
previous unrestricted layout placed Save below the viewport. Both backend tests
check the window bounds and reach Save by scrolling. Current reference
`SettingsPane.tsx` still describes non-theme fields as mock-only in its header,
although its contexts persist and apply those fields. The port follows that
implementation, not the stale header.


## File destinations and native file drops

A failing native regression showed that `DroppedFile` events never reached the
loader. The native event loop now retains incoming paths through initialization
and opens each batch using the configured destination. Tauri also subscribes to
its native drag/drop API instead of relying on the browser-only `File.path`.
The current reference separates drop and shell destinations, reuses untouched
scenes and pins later object files to the first successful load. A shared
`FileOpenBatch` now applies these rules, and a closed destination or failed load
leaves the scene unchanged. Tauri resolves the target after accepting reader
options. Its failed renderer setup leaves no new tab or undo entry.

Browser imports retain a batch with their `File` objects while the parser worker
runs. Scene files retain their independent opening rule, including empty-scene
reuse; menu files use the current scene. Cancelled or superseded parsing retains
the existing generation and scene checks. Native and Tauri preferences persist
the independent `files:drop-target` and `files:shell-target` values; the browser
stores its drop choice locally. The native path wrapper reuses its outer
transaction instead of making another full application snapshot.

A further failing surface-drop regression showed that native object imports
cleared the unsaved marker because surface loads share a result variant with
scene files. The handler now distinguishes saved scenes from imported objects
and PyMOL sessions, preserving the marker and undo/redo.

Eight core file/loading regressions pass, as do all 203 native GUI tests. The
macOS Tauri model passed 175 tests with five optional cases ignored; the added
reader-option destination case and updated preferences round trip also pass.
All 73 affected browser cases pass across the loading suite, Tauri shell and
WebGL/WebGPU destination scenarios. The initial destination test clicked the
combined field label; targeting the actual combo box fixed the test, after which
both backends passed batching, persistent settings, menu destinations and
failure-preserving redo.

The X11 OS-input probe transfers real `text/uri-list` data through XDND and
checks a two-file drop, its separate scene, undo and redo after an invalid drop.
It passes with the current native build, and the captured window was inspected.
Run it with `scripts/check-native-gui.py --file-drop-only` on an X11 display.
These checks do not cover other platforms' native file-drop events.


## Animation authoring cameras and default state

Failing regressions showed that shared `add_anim` commands did not save the
authoring view, while the Tauri playback path overwrote an existing `__current`
camera. The current reference initializes this camera only when absent, adopts
it only for an empty start-camera choice, and excludes that initialization from
the element's undo transaction. The command, native dialog and Tauri add paths
now follow these rules. History snapshots retain the initialization for panel,
console, MCP and agent adds. An invalid add still leaves the scene unchanged.

Tauri now synchronizes the live view separately from export-camera persistence.
Playback, script/console execution, view/scene switching and scene opening no
longer overwrite a saved animation start. A model regression covers first-add
undo/redo, playback after moving the view, read-only scripts, console execution
and scene switching. The native regression exercises both the shared panel
command and actual console execution before undo.

Show and Slide-in additions now record their explicit `hide=false` defaults,
matching the reference Add behavior. Empty reset lists and names joined only by
spaces are rejected before changing the animation; reset lists use commas.
The earlier Tauri Inspector test expected an exact pre-add scene after undo.
Its updated assertion verifies unchanged elements and the retained start camera,
consistent with the reference service.

The shared command suite passed 221 checks with two unrelated optional cases
ignored. Native GUI tests passed 200 checks, and the macOS Tauri model passed
174 with five optional cases ignored. Rebuilt WebGL/WebGPU passed 12 affected
browser cases, including actual Add-menu clicks, console adds, first-add undo,
playback anchoring, invalid reset lists and the existing timeline/value tests.
These finite checks do not establish complete appearance parity.

## Animation command value conversions

Four failing regressions exposed boolean aliases being misread, non-decimal
numbers being rejected, generic values bypassing native type checks, and
near-zero spin axes losing the prior direction. The command layer now converts
structured and generic values separately. Renderer target names also use the
reference comma/slash normalization, including incomplete target exclusion.
Serialized near-zero spin axes retain the constructor's default direction.

`scripts/check-reference-animation-values.py` reads the current reference's
TypeScript conversion functions and applies their typed outputs to its C++
animation wrappers. All 98 cases pass, including finite radix numbers beyond
exact integer precision, generic type rejection, axis thresholds and target
normalization. Generated inputs, binaries and results stay in the ignored cache.
The runner needs Node with `stripTypeScriptTypes`, the reference build and its
configuration; ordinary Rust tests skip the external oracle when its paths are
unset. Independent regressions still execute without the reference tree.

Shared animation/command tests passed 233 checks with two unrelated optional
cases ignored. This run included the existing 24-case C++ property/default
comparison and the new scene-deserialization regression. Native animation tests
passed 28 checks; the macOS Tauri model passed 25 with one unrelated optional
encoder check ignored. Rebuilt WebGL/WebGPU passed all ten affected browser
checks: eight timeline/Inspector cases and two command-value scenarios. The
new scenarios confirm that rejected generic writes preserve redo.

The reference generic-command description advertises vectors as `"x y z"`, but
its C++ string conversion accepts parenthesized comma-separated vectors.
The implementation and usage guide follow the parser; the structured command
continues to accept space-separated axes.

## Shared View transforms

A failing reproducer showed that changing camera distance also reduced the slab
in the Rust View implementation. The current C++ `View::setViewDist` changes
only the distance; `setSlabDepth` applies the distance-dependent clamp when the
slab is edited. A second reproducer found that the Rust View could retain slabs above the
Camera limit of 10,000 Å. Both fixes pass 26 C++ oracle cases covering distance
boundaries, slab clamps, zoom limits and rotated pan. The camera-reference runner now builds
and runs this oracle alongside its existing comparisons.

Native and browser windows share a View pane with relative rotation, viewing-axis
translation, absolute text coordinates, zoom/slab/distance and projection/center
marks. A shared transform helper also serves Tauri and retains unrelated camera
metadata. Inputs use horizontal drag, reference fine/coarse modifiers and
Enter-chained coordinates. The shared input tests exposed missing text selection
when focusing the next coordinate; explicit selection now preserves consecutive
entry. View changes do not create scene undo entries or discard redo.

Tauri uses single numeric fields instead of separate wheel buttons. Its Camera
pane now precedes View and supports the existing pane resizing/collapse controls.
Queued camera movement is tagged with the active view so it cannot be replayed
onto another view. Draft controls also retain their original view identity.

The shared UI suite passed 53 checks, including three actual-input View cases;
the final cases also check Shift and Ctrl snapping. The transform-helper
regression, all Qsys tests (146 unit and three integration checks), and five
shared view-command checks passed. The native GUI suite passed 199 checks, and
the macOS Tauri model passed 173 with five unrelated optional checks ignored.
WebGL and WebGPU each passed the new View-pane scenario, including rotated pan,
chained coordinates, distance/slab separation, projection and redo preservation.
All 55 existing Tauri shell cases passed after the pane-layout change; the added
stale-view queue scenario also passed separately.

The Linux OS-input probe passed rotation, relative pan, chained coordinate input,
distance/slab behavior, projection and redo preservation in a real 1024×768
window. Native, browser and Tauri screenshots were inspected. These checks cover
the changed View controls and do not imply complete application appearance parity.

## Saved camera semantics and shared controls

Reproducers showed three discrepancies: plain camera saves retained old
show/hide flags, applying a saved camera copied its source and flags into the
live view, and reordering the camera vector was undone by sorting unchanged
stored order values. The current C++ `Scene::saveViewToCam` and
`View::setCameraAnim` clear the saved/view flags respectively. The Rust paths
now follow those rules, preserve an overwritten camera's source and slot, and
write the new order explicitly. The `__current` row remains pinned. Separate
replacement/import helpers preserve metadata for properties, rename and files;
invalid targets and duplicate names fail before changing the scene.

The shared native/browser Camera pane and Tauri shell provide row selection,
keyboard rename/navigation, drag reordering, four save/apply combinations,
clipboard actions, properties, show/hide edits and camera-file controls. Plain
apply no longer creates an undo entry or clears redo in the native window.
A Linux OS-input failure exposed list focus moving away after Up/Down; a full
native-layout regression reproduced it. Consuming the list navigation now also
cancels egui's pending spatial focus move. Rows show camera icons, left-aligned
names and separate visibility/link badges; save/apply arrows distinguish the
four operations.

Browser `.cam` imports validate the XML before changing the scene and reject
late results after cancellation or a scene change. Imports use the same bounded
history as other file edits. Subsequent shared edits and imports prune file
handles that are absent from live and undo/redo scenes. Reload uses the tab's previously opened File and
preserves the current view. Save creates a download. Reading later filesystem
changes requires opening that file again.

The shared core/command run passed 583 checks with two unrelated optional
checks ignored. Qsys camera/view coverage passed 146 checks; the shared UI
passed 50, and the native GUI passed 199, including the keyboard-focus
regression. The macOS Tauri model passed 173 with five optional cases ignored.
All 60 affected browser checks passed: four Camera-pane cases and two existing
camera-history cases across WebGL/WebGPU, plus all 54 Tauri-shell cases. The
Linux OS-input probe passed navigation retaining redo, visibility undo,
keyboard rename, pinned ordering and one-step drag undo. Native, Tauri and
browser screenshots were inspected. Final browser resource-retention changes
were rebuilt and the two camera-file cases were repeated successfully.

The current reference's `features/camera/cameraOps.ts` comment still says
`listCameras` excludes `__current`, but its list service and Camera pane include
that row.
The implementation follows the service and keeps the row pinned.

## Shared crystal-symmetry controls

The browser's former command shortcuts now use the same Symmetry pane and cell
editor as the native window. Molecule/map selection, fixed-decimal crystal
readouts, the extent menu and unit-cell action follow the current reference.
The editor applies all seven crystal-system restrictions, including the
rhombohedral case, and uses the current reference's 1 Å initial cell. Scene
changes and target removal discard its draft. Unchanged acceptance and Cancel
do not add undo entries or discard redo.

The shared unit-cell command now accepts density maps as its existing catalog
description promised; previously its implementation only accepted molecules.
Repeated calls reuse the renderer without changing the scene or camera. A
separate reproducer showed that zero cell angles were accepted. They now fail
before replacing crystal data, consistent with the editor's positive-angle
validation. Tauri's cell editor also rejects stale space-group responses,
retains the selected scene/object, provides Cancel, and skips unchanged edits.
Dialog selects now have explicit accessible names.
Focused reproducers also showed that applying a lattice restriction overwrote
the unconstrained draft. The editors now derive the displayed values separately,
so turning the restriction off restores the original inputs before acceptance.
The Symmetry panel's initial disclosure state and single-line action labels
also match its visible contents.

The shared UI passed four focused checks, including actual mouse events through
creation, cancellation and renderer actions. All nine map-command checks passed,
including map unit-cell reuse and zero-angle rollback. The native GUI suite
passed 197 checks; the new shared-panel interaction check proves that repeated
cell display is one undo entry and an unchanged modal edit retains redo. The
macOS Tauri model passed 172 checks with five optional cases ignored. All 53
Tauri shell checks passed, including delayed space-group replies. WebGL and
WebGPU passed all four new symmetry cases and both adjacent density-pane cases.
The Linux OS-input probe passed unit-cell creation/reuse, one-step undo, modal
cancellation preserving redo, the 50 Å extent menu and camera preservation.
Native, browser and Tauri screenshots were inspected. The affected crystal
summary regression also ran against the reference fixture and passed.

## Animation property metadata and reset state

The animation model now retains declared defaults separately from explicit
values. Its property list covers all eight current element classes, including
read-only absolute times. A C++ oracle checks property names, types, read-only
flags, default availability and values across initial, explicit-equal and reset
states: 24 class/state cases pass. A serialization regression confirms that
explicit values equal to a default remain present, while reset values are
omitted and regain default state when loaded.

A command reproducer showed that resetting a mixed list could clear a renderer
target that has no declared default. Such requests now fail atomically. The
Inspectors enable per-property and Reset-all controls only for modified declared
defaults. Numeric previews restore the default flag on cancellation and before
the final undo snapshot. The legacy native/Tauri reset dialogs and JavaScript
`cuemol.resetProp` use the same model semantics. A stale Tauri form cannot rename
the current element during a reset. Typed Generic fields also keep a name such
as `true` as text, and real-property values retain their unrestricted range.

The shared animation, command and UI suites passed 306 checks with two unrelated
optional checks ignored. This run explicitly enabled both the geometry and
property oracles. All 15 affected desktop-core animation checks and 28 native
animation checks passed. The macOS Tauri model passed 25 checks with one optional
GPU case ignored; its final reset regression passed again with a stale form.
All four affected Tauri shell cases passed, including default-state cancellation,
reset/undo, string field types and unrestricted real-property inputs.
The rebuilt WebGL/WebGPU viewer passed all 12 affected animation cases, including
cancelled default-value drags, disabled resets preserving redo, string names and
saved reference chains. The current Linux OS-input probe also passed the strip
and timing preview/cancel/undo checks. The final screenshots were inspected.

The reference `propModel.ts` comment describes scalar default annotations as
future work, but its current C++ `getPropsJSON` already emits those defaults.
The implementation follows the emitted property metadata. The earlier animation
milestones below retain their historical scope.

## Animation preview transactions and Tauri strips

The native, browser and Tauri element Inspectors now preview paired start/end
timing and generic numeric edits through the shared command layer. Previews
preserve redo, cancellation restores the anchor, and release restores the anchor
before recording one ordinary undo entry. Scene changes, target changes and
hidden fields cancel the edit. Tauri coalesces previews and waits for an in-flight
request before applying abort or commit. Stable UIDs also protect the legacy
property dialog from editing a different element after a reorder.

All three frontends provide axis presets, Cartesian components and cardinal
slide directions. Structured angles wrap to the reference range, and only
MolAnim fractions use the 0..1 range. A command reproducer showed that non-finite
scalar values were accepted; these now fail without changing the animation.
Clock parsing also rejects overflow after conversion to milliseconds.

Tauri now uses colored strip lanes, edge grips, a ruler, local scrubbing, zoom,
Fit and UID-based selection. Moving, resizing, reordering, deletion and immediate
insertion use the shared commands and existing history boundary. A screenshot
check exposed a CSS specificity conflict that hid the timeline summary; the
corrected grid keeps the ribbon, summary and ruler separate. Native panel tests
also reproduced progressive height shrinkage and loss of the expanded height
after collapsing. The panel now fills its assigned height and keeps separate
expanded/collapsed state.

The shared command, animation and UI run passed 302 checks with two unrelated
optional checks ignored. The UI run included 42 checks and explicitly executed
the reference geometry oracle. Native animation checks passed 27, including a
real Inspector drag through preview, abort, commit and undo/redo. All six native
layout/history checks passed after the height fix. The macOS Tauri animation
model passed 25 checks with one optional GPU case ignored. The full Tauri shell
run passed 50 checks; after final CSS and insertion assertions, all three affected
animation cases passed again. WebGL and WebGPU realtime timing/generic cases
also passed, including Escape preserving redo and one-step undo/redo.
The final Linux OS-input run repeated strip and Inspector duration drags,
cancellation and keyboard undo/redo successfully. Native and Tauri screenshots
were inspected after the layout fixes. Formatting, whitespace and script syntax
checks passed; generated evidence remains in the ignored cache.

The earlier strip-timeline milestone below describes its state at that commit.
Its remaining Tauri, preset and realtime-preview items are covered above.
Default-state metadata was still under audit at this milestone and is covered
above. Remaining Inspector details and broader appearance parity stay open.

## Animation strip timeline and shared element Inspector

The default native and browser windows now share the current reference's strip
layout: a fixed channel column, a scrollable time ruler, colored element lanes,
resize grips and a playhead. Body/edge drags and ruler/current-time scrubs preview
locally and commit one edit on release. Escape, focus loss, scene changes and an
external change to a dragged span cancel it. Selection follows the stable UID
through reorder and history. Deletion selects the next or previous surviving
element. The transport includes start/end, play/pause, stop, current time,
duration, start camera, loop, element count, FPS, Fit and zoom. The timeline read
model is cached by scene/revision; playback updates its transport values without
rebuilding the element graph on every frame.

A shared right-side element Inspector exposes structured and generic fields,
validated reference choices, camera/renderer/MorphMol targets, signed legacy
starts, duration and property resets. Numeric fields commit on release. Generic
start/end remain read-only because the paired timing command owns them. A
regression reproduced rejection of a negative relative start; the command now
matches the reference's signed timing contract. The typed spin-axis operation
also accepts the documented three whitespace-separated finite numbers.

The read-only geometry oracle compared 128 cases against the current reference's
scale limits, fitting, width, tick spacing, labels and integer-millisecond clock
formatting. Fractional clock rounding intentionally carries into the next second:
59,999.6 ms displays `1:00.000`, whereas the reference helper produces `0:59.1000`.
The reference AnimationPanel source comment still says per-type editing belongs
to a later phase, but its current AnimElementInspector implements it. The UI
follows that implementation; the reference checkout remains unchanged.

The full animation/command suites passed 216 tests with two unrelated optional
checks ignored. All 39 shared UI tests passed, including the explicitly enabled
geometry oracle. Native animation checks passed 26, including a strip drag
through the real application command/history boundary. A Linux window also
passed OS mouse/keyboard checks with
`scripts/check-native-gui.py --animation-timeline-only`: holding a drag left the
scene unchanged, Escape preserved redo, and one keyboard undo/redo changed the
start between 0 and 300 ms. The screenshots were inspected.

Ten affected WebGL/WebGPU cases passed: four new strip/Inspector cases, four
reference-chain/QSC cases and two pixel-based playback/pause/stop cases. The
final UI build repeated the four new cases and both playback cases successfully.
Checks cover moving/resizing, local scrubbing, cancellation, undo/redo, renaming,
paired duration edits, rejected cyclic candidates, reordering and deletion.
Final browser and native screenshots were inspected, and formatting, whitespace
and script syntax checks passed. Generated evidence remains in the ignored cache.

Tauri's strip layout, structured axis/direction presets and the reference's
realtime Inspector timing previews remain in the animation UI audit. This
milestone does not close the complete feature/appearance requirement.

## Density map pane and shared numeric controls

The browser's former three-button placeholder now uses the same Rust Density
map pane as the native window. Both select all four map renderer families,
switch native/absolute units, show the corresponding level hint and range,
edit solid colors, and disable extent for a full region. The native pane's
former immediate slider writes are replaced by the Inspector's shared numeric
editor: opacity previews without history and commits one undo step; level and
extent commit on release. Tauri uses the same transient property protocol,
coalesces opacity requests and keeps density edits separate from the Inspector.
Hidden panes and changed targets cancel active previews.

A command regression reproduced an EM unit cell moving the view from its
chosen center/zoom to the map. Unit-cell creation now bypasses the map renderer
centering policy; mixed renderer creation also leaves unit-cell centers alone.
The focused command suite passed all 13 cases. All 32 shared UI, 192 native GUI
and 169 macOS Tauri model checks passed; five optional Tauri checks remained
ignored. The native input-event regression drives an actual density field and
checks preview, default restoration, cancellation and one-step undo/redo.

WebGL and WebGPU mouse checks cover level typing, unit switching, opacity
preview/cancel/commit/undo/redo and Cell insertion without a camera change.
Tauri shell checks cover a delayed preview cancellation, renderer switching,
EM percent limits, disabled full-region extent and unit-cell state. All 62
Inspector/Tauri shell regression cases passed. The two WebGL/WebGPU density
cases passed after correcting their gesture, for 64 passing affected cases;
the full run's two initial failures were those uncorrected test gestures.
The final screenshots were inspected. A checked
menu item's decorative marker and the numeric drag threshold required test
selector/motion corrections; these were not runtime defects. Generated logs
and screenshots remain in the ignored cache.

## Inspector numeric drag previews

Structured renderer controls now follow the current reference's realtime flags:
opacity, CPK radii, simple/trace and map/surface line widths, sphere/stick sizes,
spline smoothness/width and label offsets preview while dragging. Expensive
geometry controls retain commit-on-release behavior. Preview writes do not
capture scene snapshots or clear redo. Release first restores the original
value and default flag, then records one ordinary property edit. Escape, lost
window focus, a hidden field or a changed target restores the anchor. An abort
can restore the original scene after the active scene changes. Later unrelated
edits are preserved. Invalid numbers, read-only properties and missing renderers
are rejected without changing another target.

Tauri numeric fields and their labels support horizontal dragging. Worker
requests are coalesced to one in-flight preview and the latest pending value.
Commit and abort discard pending previews and wait for the in-flight request,
so delayed work cannot overwrite a cancellation. Normal typed values, native
number steppers, fine/coarse modifiers and existing numeric limits remain usable.

The shared command run passed 228 checks with two optional checks ignored; all
32 shared UI, 191 native GUI and 169 macOS Tauri model checks passed. Five optional
Tauri checks remained ignored. The rebuilt browser passed all 60 Inspector and
Tauri shell cases, including WebGL/WebGPU mouse drags, cancellation restoring
defaults and redo, one-step undo/redo, and scene creation during a drag. Tauri's
shell test deliberately held a preview response while sending more motion and
Escape, then verified that only the original preview and abort were applied.
The actual model test separately verifies history and default flags.

A Linux native window also passed OS mouse/keyboard and pixel checks using
`scripts/check-native-gui.py --inspector-preview-only`: carbon radius changed
from 1.7 to 2.4 while the button was held, rendered pixels changed, Escape
restored the default, and keyboard undo/redo restored the expected values.
The screenshots were inspected. The native and WASM builds, JavaScript syntax,
formatting and whitespace checks passed. This covers renderer numeric edits;
animation timing and the separate Density map pane are audited independently.

## Atom identity and molecule-edit history

The low-level deletion record previously copied atoms back through the ordinary
merge path, which rejects a partially occupied residue. Regressions reproduced
failed partial-residue undo, missing alternate conformations and colliding IDs
when merging molecules with overlapping input serials. New edit records retain
only affected atoms, their original positions, residue metadata and incident
bonds. Restoration checks every reserved ID and atom name before changing the
molecule. Redo removes the recorded IDs without reevaluating a spatial selection.

Molecules now retain an allocation limit across deletion, script transactions,
application snapshots and parser-worker transfers. Copy and append allocate
fresh IDs; undo/redo restore the IDs used by later coordinate edits. A separate
application-snapshot regression failed before the final repair: after undoing a
merge it allocated ID 6 again, instead of the unused ID 8. The transfer payload
is private to matching builds and its version is now 5. The low-level
`MolMergeEditInfo::new_copy` constructor records the destination after copying,
matching the reference's copy-record creation order.

The molecule-specific undo manager now also clears both history stacks after a
failed callback or native unwind panic and restores its previous recording
state. Panic-abort builds cannot recover a panic. Subsequent edits remain
recordable. All 295 molecular/analysis checks passed with the reference fixtures
present, including eight new regression cases. The script and application-history
checks passed in the shared core/command run: 558 passed, two optional checks
ignored. All 191 native GUI and 168 macOS Tauri model checks passed; five optional
Tauri checks remained ignored. The read-only C++ probe ran the current reference's
four deletion/merge tests successfully. Rebuilt WebGL and WebGPU checks both
passed worker loading, overlapping input IDs, partial deletion with alternate
conformations, coordinate undo/redo and allocating a fresh ID after merge undo.
The additional 27 loading, clipboard, selection and camera-history browser
checks passed. Formatting, whitespace, syntax checks and all 12 distribution
helper tests passed; the installer/self-upgrade fixture suite also passed.

The reference's `SavedAtoms` comment describes residue properties as needed for
newly recreated residues, but its restoration loop writes them to every restored
residue, including an existing one. The Rust low-level record follows that code.

## Current backbone-width inspector ranges

The current reference narrowed tube, ribbon and cartoon section-width controls
to a maximum of 1.5 angstrom with 0.01 steps. The shared inspector schema now
uses those limits. Tube semi-axes retain their 0.01 minimum, while ribbon and
cartoon section widths allow zero. These are UI limits; direct property writes
and the separate legacy property dialogs retain their existing behavior.

All 32 shared UI and 191 native GUI tests passed. Rebuilt-WASM and Tauri shell
checks passed the affected numeric controls, including clamping an entered tube
width of 2 to 1.5, preserving its other semi-axis through one atomic edit, and
undo/redo. The WASM build, formatting and whitespace checks passed.

## About build metadata and clipboard

The current reference added runtime details and a Copy button. Native egui and
WASM now share this dialog, and Tauri displays the same metadata through its
application-info response. The listed versions describe the actual Rust build:
compiler, target, source commit, UTC build time, wgpu, the selected GUI toolkit
and the enabled Boa engine. OS release or browser/WebView details are resolved
at runtime. Build dependencies come from Cargo.lock; no network lookup runs
when opening the dialog. SOURCE_DATE_EPOCH controls reproducible timestamps.

All 32 shared UI tests, 191 native GUI tests and 168 macOS Tauri model tests
passed; five optional Tauri checks remained ignored. A Linux native window was
opened through OS mouse input, its screenshot inspected, and its Copy result
read from the system clipboard with all expected runtime fields present.
WebGL and WebGPU checks copied the generated metadata, verified the WASM target
and browser details, and checked that footer buttons remain visible at 700 by
650. Their screenshot was inspected. The Tauri shell check verified row display,
copy output and clipboard failure reporting. The broader browser shell, Tauri
shell and camera-history run passed all 55 checks. The WASM and native binary
builds, formatting and whitespace checks passed.

## Failed undo, redo and rollback callbacks

The low-level UndoManager previously moved a failed transaction to the other
history stack, and rollback ignored failed callbacks. Regressions reproduced
both defects. A callback that has already changed state cannot safely leave
older history available, so the manager now stops on the first failure,
discards both history stacks and restores its previous recording state.
Native unwind panics follow the same path; panic-abort builds cannot recover
from a panic.

The scene wrapper emits one undo-information event when a failure discards
history. Refusing a step before executing it keeps its history. Later edits
remain recordable and undoable. All 148 qsys tests passed, including partial
failure, undo/redo/rollback, native panic recovery, refused steps and event
delivery. A fresh probe against the current C++ reference also passed all six
combinations of the three operations and false-return/exception failures,
including one-shot history-drop flags and recording after failure. The shared
core and command suites passed 556 tests with two optional checks ignored.
The qsys WASM check, formatting and whitespace checks also passed.

## NAMD coordinate sniffing and binary map headers

The current reference rejects subnormal values when sniffing NAMD coordinates.
The Rust sniffer still accepted them, so a CCP4 header made from small integer
words passed as coordinates near zero. The new regression failed before the
repair with a NAMD verdict of Yes for the CCP4 header.

The sniffer now accepts zero or normal finite coordinates within its existing
magnitude bound. All 18 stream compatibility tests passed with the reference
fixture directory present. The added case checks a CCP4 header and unrestricted
reader selection, both byte orders, signed zero, the smallest normal values,
subnormal values, oversized coordinates, infinity and NaN. This changes format
identification; explicitly selected NAMD readers retain their existing decoding
rules.

## Get PDB search, entry metadata and EMDB loading

The current reference's Get PDB flow was missing keyword search and entry-aware
download choices in the Rust frontends. A shared protocol now prepares RCSB
search/GraphQL and EMDB metadata requests, restores search ranking after metadata
lookup, handles HTTP 204 and optional administrative metadata, and bounds native
metadata responses. The live public-service check was explicitly run and passed;
ordinary tests keep that network-dependent check ignored.

Native egui and WASM share a dialog state machine. Tauri exposes the same query
protocol through asynchronous IPC. Checks cover pagination, method/sort changes,
missing entries, mmCIF-only entries, direct EMDB IDs, recommended contour levels,
metadata failures, stale searches and cancellation. Both the browser worker and
the native loader preserve EM map dimensions/statistics, select the isosurface
renderer, and apply the recommended absolute level. The worker also receives an
explicit coordinate/map kind so mmCIF coordinates cannot be mistaken for map
coefficients.

The shared core, command and UI run passed 588 tests with two optional checks
ignored. The live metadata check passed separately. Native egui passed 191 tests;
the macOS Tauri model run passed 168 with five optional checks ignored. A later
shared UI run also passed after constraining dialog width. The egui layout
regression exposed an offscreen download footer and a dialog that expanded to
the screen width; it now checks visible footer controls and bounded width at
1280 by 900 and 760 by 650. Tauri's grid styling also overrode hidden metadata
sections; the explicit hidden rule fixes that case.

The focused WebGL/WebGPU and Tauri shell checks passed, including worker loading,
undo and cancellation of pending metadata and file downloads. Tauri shell checks
mock IPC and are complemented by the native model and live protocol checks.
The broader run passed 59 checks. Three older loading checks still intercepted
uppercase download URLs and clicked before the asynchronous dialog layout
settled. Their fixtures now use the normalized URL, stub metadata, and wait for
the entry choices before clicking; all three passed afterward. The 62 checks
therefore cover the affected loading, Get PDB and Tauri shell suites. The final
dialog screenshot was inspected at the bounded width. The WASM build,
formatting and whitespace checks passed.

## Paint sidebar width and current default appearance

The current browser baseline completed 214 checks: 207 passed and seven failed.
Three failures still asserted the older palette or default dimensions. The other
four exposed a layout defect: long named Paint colors expanded the Explorer and
removed the usable viewport at narrow widths. Fixed mouse coordinates then fell
outside the viewport even in some wider-window checks.

Paint selection and color rows now stay within their assigned columns in both
egui frontends. Truncated color names retain full-text tooltips. The native
inline editors also reserve space for their color picker. A native regression
uses long selections and color modifiers with the application theme and checks
both selected and unselected rows. All 190 native GUI tests passed.

The appearance assertions now use the current CueMol3 styles and canonical
color strings. The swatch values were checked with the current C++ color
conversion functions, and the trace fixture includes a valid peptide backbone.
The WASM build passed. All 62 checks in the affected default-appearance,
render-panel and viewer suites passed, including all seven previous failures.
The 760-pixel screenshot was inspected: the viewport remains visible alongside
the Explorer and Inspector, and the bottom tabs remain reachable.

## Native Python Umbreon helper and dictionary results

The current reference includes cuemol.umbreon_render; importing that module
previously failed in the Rust application. The native bridge now embeds its
apply_scene_settings, render and render_file interfaces. They use the shared
Rust exporter, preserving stored Umbreon/NPR settings, optional size overrides,
saved-camera selection and the fallback projection when no render app data
exists. Applying defaults leaves the scene without a new settings holder.

The live render_file check exposed a separate binding defect: CmdMgr.runCmdArgs
returned a proxy where the reference QIF declares a dictionary. The Python
bridge now recursively converts ordinary data objects into dictionaries while
retaining native objects and callable handles. Arrays retain their list form.
Non-enumerable fields are excluded; cyclic or excessively nested data returns
an error without desynchronizing the worker.

The Linux shared core, command and Python suites passed 560 tests, with one
optional real-POV test ignored. The two focused dictionary tests also passed in
the offscreen-enabled build. They check dictionary keys and nulls, live scene
handles retained across Python submissions, nested data and vectors, and recovery
after a cyclic result. The existing native Python smoke check passed.

scripts/check-python-umbreon.py rendered actual molecular PNGs. Helper and
direct-exporter pixels match exactly, including after QSC reload. The check
also covers NPR output, size overrides, Unicode/space paths, saved settings,
missing-camera errors and preserving an existing file on that error. The
generated image was inspected. The same live helper check passed on Apple-silicon
macOS with a fresh native build. Release verification now includes this smoke
check; the updated workflow has not been run remotely.



## POV-Ray pixel labels and bounded PNG composition

Following dd06fd4, a live render reproduced missing name labels when a Python
script enabled usePixImgs. The script exporter had not supplied the pixel images.
It now collects live lightweight-object images and, in GUI-enabled builds, name
and measurement labels. Font, color, alignment, displacement and camera settings
come from the shared scene. Explicit world-space corners keep each label's pixel
size in orthographic and perspective views. Disabling usePixImgs clears the
exporter's image list. Both Linux and macOS script regressions pass.

The real bundled renderer now passes name-label and distance-label checks,
transparent-layer composition and changing animation frames. These assertions
compare decoded pixels; PNG metadata alone cannot satisfy them. The generated
label images were also inspected. The macOS Tauri model suite passed 167 tests
with five optional checks ignored; the native Linux GUI suite passed 189.
WASM compilation also passed. GUI-enabled release binaries include label
rasterization; a headless source build without the offscreen feature does not.

The Rust post-blend compositor now reads and writes non-interlaced RGB8/RGBA8
PNGs one row at a time. The existing buffered path remains for interlaced input.
The tests cover multiple rows, RGB/RGBA, Adam7 input, dimension mismatch, process
failure, truncated output and cancellation. Post-blend metadata is scanned with
a buffered reader instead of allocating the full scene text.

The following comparison uses three sequential fresh Linux debug processes per
condition, 2048 by 2048 RGB images and four transparency layers. A fixture copies
pre-generated PNGs in place of a ray tracer. Timings include native process
startup, copied passes, composition, encoding and output validation; they exclude
fixture generation and pixel-hash verification. Hash verification runs in a
separate Python process so its allocations cannot inflate the next sample's
process memory measurement.

| Implementation | Median elapsed time | Median peak RSS |
| --- | ---: | ---: |
| Buffered PNG composition | 4721.75 ms | 112.00 MiB |
| Row-based PNG composition | 4420.06 ms | 40.25 MiB |

Every decoded output has the same SHA-256. This measures PNG composition rather
than total molecular ray-tracing time. Reproduce the current condition with:

```sh
uv run --no-project python scripts/benchmark-pov-blending.py \
  --binary target/debug/cuemol --output .cache/pov-blending.json
```

The shared render, core and command suites passed 646 tests, with one
optional real-renderer test skipped in that aggregate run. The actual renderer
was executed separately by the Python smoke check.

## Apple-silicon stereo capability check

On macOS 15.6 with an Apple M4 and a connected 3840 by 1600 display, accelerated
double-buffered OpenGL pixel-format selection succeeded. Adding the stereo
attribute returned CGL error 10000 and no formats. This setup therefore cannot
validate quad-buffer presentation, whose backend remains unimplemented.

The native binary produced normal, parallel and cross-eye 96 by 96 PNGs through
the wgpu offscreen exporter. Normal and stereo images contained molecular
shading, and the cross-eye image exactly exchanged the parallel image's two
halves. This validates image generation, not window presentation or physical
stereo output. Screen-capture and accessibility permissions were still denied
to the remote test process; the user authorized granting them, but the OS grant
had not yet taken effect.


## Native Python POV-Ray rendering

The native Python bridge now embeds both the current povrender.PovRender class
and the legacy povrender.render function. Stills and animation frames call the
shared Rust POV-Ray process runner through cuemol tools render-pov; the direct
egui and Tauri development executables also dispatch that request before opening
a window. Render options include fog, clipping, perspective, lighting, radiosity,
thread count and transparent-layer post-blending. An explicit external blendpng
executable is honored; otherwise Rust performs the composition.

The output is staged beside its destination, fully decoded as PNG, checked
against the requested dimensions and then atomically replaced. Process errors
and truncated PNGs preserve an existing destination. Captured stdout/stderr are
drained while retaining at most one MiB each; JSON requests are bounded to one
MiB. The temporary exporter directory is removed on success and ordinary errors.

Validation on the Linux development build:

- Shared command, CLI and Python suites: 255 passed, one optional real-renderer
  test ignored in the default run.
- Native GUI models: 189 passed. The POV process tests moved into the shared
  command crate, so the native count is lower than the previous 192-test run.
- macOS Tauri models: 167 passed, five optional checks ignored.
- scripts/check-python-povray.py used the bundled real renderer for the class
  and legacy function, transparent-layer composition, two changing animation
  frames, Unicode/space paths, and failed-render output preservation.
- The same live helper check also passed with the direct egui executable as its
  renderer host, proving that the Python child does not open another GUI.
- Release verification now runs the helper smoke check against each platform's
  bundled renderer. This workflow change has not been executed remotely.

The follow-up pixel-label checks above cover name and measurement labels;
these finite checks do not establish parity for every renderer setting. Earlier exclusions of the Python POV helper
below describe historical scope; the current all-feature request includes it.

Entries record successive checkpoints, so earlier test counts and distribution
results are historical. Generated inputs and logs stay in the ignored checkout
cache. The current repairs and their limits appear in the completion audit;
passing tests do not establish equivalence for every parameter combination.

## Current CueMol3 defaults and live paint colors

The shared default style manager now loads the current reference's
`cuemol3_style.xml` after its base stylesheet. The override changes ribbon
sections, nucleotide bases and backbone sections, the PyMOL preset and the
available palette themes. Lower-priority properties still fall back individually
to the base styles; user and scene definitions override both built-in sets.
The explicit `classic` palette extension retains the earlier color scheme.

New molecules now take the current Okabe-Ito palette in molecule UID order.
Previously, custom global palettes were ignored because initialization read a
separate static style manager, and each new color created another scene style.
Initialization now reads live global definitions, sorts numeric suffixes,
reuses a writable scene style and retains existing local colors. Saved scenes
carry the used RGB definitions independently of subsequent global edits.

A read-only TypeScript comparison executed the reference default-paint helper
for 132 cases across all eight current themes, custom numeric order and fallback
choices. Shared regressions also cover local overrides, read-only style sets,
scene reload and property fallback. Paint scripting exposed another defect:
object/renderer `getColorAt`, insertion and attachment converted named colors
to fixed RGB. These paths now keep names and modifiers, and retained color
values respond to scene palette notifications. The native script regression
also verifies serialized color expressions.

The final shared core/command checks passed 545 cases, including the explicitly
enabled TypeScript oracle. The affected qsys/molvis/core/command run passed 769
cases before the additional script regression. Native GUI checks passed 192
cases with one optional case ignored, and macOS Tauri passed 170 with six
optional cases ignored; the final paint change also passed its four console
checks. Linux OS input passed ChimeraX, camera-preserving history and combined
Zoom after accounting for the additional style rows. Rebuilt WebGL2 and WebGPU
checks passed current palette/geometry values, visible ribbon pixels, and QSC
save/reload. Their screenshots were inspected. The first synthetic structure
had no peptide bonds and produced no ribbon; the corrected fixture includes a
complete backbone and excludes UI controls from the pixel check.
The final browser console, Inspector and selection regression run passed all 35
cases. The complete Linux OS-input suite also passed its navigation, selection,
clipboard, Python, console, font and Inspector checks on the updated binary.

Reproduce the optional palette comparison with an absolute reference directory:

```sh
CUEMOL_DEFAULT_PAINT_REFERENCE="$(cd ../cuemol2 && pwd)" \
  cargo test --locked -p cuemol-desktop-core --features scripting --test current_styles
```

## Scene save failure and linked files

A failed QSC write previously overwrote writable linked styles before reporting
that the scene could not be saved. Native and script scene writers now prepare
all destination files before replacing any original. Each replacement uses a
temporary file in the destination directory. An ordinary replacement error rolls
back earlier replacements; a rollback error retains its backup and reports the
recovery path. A process or system crash between replacements can still leave
mixed versions across files.

Existing file permissions and symlink destinations are preserved. Conflicting
scene/style destinations are rejected before writing. The final scene file uses
one atomic replacement, so its old contents do not require a backup copy. Earlier
style files use hard-link backups when available and a bounded copy fallback.
The reference's same-source modified-flag behavior remains unchanged.

Six regressions passed on Linux and macOS for invalid scene/style destinations,
script writers, successful saves, path collisions, symlinks and file permissions.
A separate test forces a destination to become a directory after preparation and
checks that an already replaced file is restored without temporary-file leaks.
The complete shared core and command suites passed 300 and 218 tests respectively.
The browser target also passed compilation with native temporary-file handling
excluded.

## Live views across scene history

A scene edit followed by camera navigation previously restored the older camera
on undo in MCP, native GUI, Tauri and WASM. Scene history now retains live views
in every surviving scene, including views created after the snapshot. Restoring
or removing a scene selects that scene's retained camera. Named camera bookmarks
remain part of scene history. Snapshot replacement preserves allocated style
identifiers without making an additional copy of the scene.

JavaScript camera-only changes no longer create scene history entries. Camera
changes inside a scene transaction survive undo, redo and rollback, matching the
current C++ oracle. The optional camera-fit script now runs this comparison too.
Failure rollback outside the scene-history API retains its existing behavior.
The Tauri view-properties dialog no longer creates a scene edit. Starting an
agent turn synchronizes its visible camera before capturing scene state.

The C++ oracle and two shared view-history regressions passed. The complete
shared core suite passed 299 tests; the shared command suite passed 218. Native
GUI tests passed 192 cases with one optional case ignored, and macOS Tauri passed
170 with six optional cases ignored. Rebuilt WASM passed 16 console/history
checks, including the original WebGL/WebGPU reproduction. Five checks on the final
WASM build also passed for scripted camera history, read-only scripts and agent
turns. All 27 CLI tests passed.

## Current operation catalog and EMDB loading

The read-only catalog check covers the current reference's 129 registrations,
127 MCP operations, 444 parameters and 159 console names/aliases. All are
present with compatible input types in the shared Rust catalog. Rust retains
additional operations and existing aliases. This inventory does not establish
behavioral equivalence for every argument or response.

```sh
node scripts/check-reference-catalog.mjs ../cuemol2
```

`focus_selection` and `orient_view` use live selections, transformed coordinates
and the actual viewport dimensions without replacing the object's selection.
`copy_node` serializes molecule/map/surface/lightweight objects and renderer
hierarchies as text-safe clipboard payloads. `save_scene` honors `embedAll`,
QDF0/QDF1, compression and base64 options; browser command saves retain external
trajectory data. Failed native saves preserve the active camera and scene
metadata. Closing a saved embedded scene checks scene changes rather than the
independent modified flag on its embedded objects.

PDB fetches pass all six reader options through native and asynchronous byte
loading. `fetch_emdb` accepts EMDB accessions or resolves a PDB entry through RCSB,
retrieves the primary recommended contour when available, and downloads the
compressed map. Missing optional metadata falls back to the top one percent of
density values; a missing entry fails. Map centering uses its kind and explicit
policy. New renderers do not change an existing renderer's contour center.
The reference exposes `create_renderer.mapCenterPolicy` but does not apply it
in that operation; Rust applies the advertised policy. Map load `centerView`
is separate from this policy, matching the reference implementation.

Native regressions cover selected fitting, orientation, binary-safe copy/paste,
all scene encodings, failed saves, alignment recentering, PDB reader options and
map policies. EMDB tests cover metadata resolution, primary contour selection,
missing metadata, explicit levels and HTTP errors. Contour comparisons allow
one density quantization step because the reference converts raw levels through
its percentile histogram. A live CLI load of EMD-1010 read its 130-cubed map,
requested the recommended 4.07 contour, and wrote an embedded QSC that reloaded
with the map and its isosurface renderer. Histogram conversion quantizes the
actual contour as described above. The deterministic
browser tests use intercepted metadata and map responses rather than relying
on the public service.

The complete shared command suite passed 218 tests. Agent and shared egui UI
suites passed 37 tests, the native GUI suite passed 192 tests with one optional
case ignored, and four macOS Tauri console tests passed. The rebuilt WASM passed
four focused WebGL/WebGPU cases for focus, clipboard, encoded saves and EMDB
loading. A browser trajectory round trip retained both objects and their six
and two frames after saving through the shared command. The complete console
and viewer browser suites passed 66 tests, including trajectories, rendering,
stereo and agent cancellation.

## Current-reference ChimeraX console

The shared Rust console implements all 59 command registrations in the current
reference's ChimeraX plugin. Native egui, Tauri and WASM select the same dialect;
CLI `run` detects `.cxc` and `.pml`, and `console --dialect` retains a session
across standard-input submissions. Existing JavaScript arguments are unchanged.
The implementation reads live selections, coloring and renderer properties.
Model numbers follow workspace order when first assigned and survive undo and
object deletion without reuse. Surface visibility does not replace the atoms
used to calculate the surface. Residue anchors and chain groups use indexed
lookups instead of scanning every existing group for each atom.

The optional read-only TypeScript oracle compares the command catalog, binding,
atom specifications, colors, palettes, model/chain colors, numeric formatting,
automatic-style boundaries and transparency. Run it with:

```sh
CUEMOL_CHIMERAX_REFERENCE=../cuemol2 cargo test --locked -p cuemol-command \
  --lib console::chimerax -- --nocapture
```

The current matrix contains 539 cases, with one documented command-name
correction: the reference parser accepts `surface h` but rejects the full
camelCase `surface hidePatches`; Rust accepts both. `save ... format pdb` also
honors the explicit format when the filename extension differs, fixing the
reference command's discarded format argument.

Thirteen focused Rust tests cover aliases, live GUI edits, selections, surfaces,
labels, distance deletion, cross-model measurements, transformed LSQ alignment,
opening, automatic styling, exports, scripts, completion and stable model
numbering. The complete shared command suite passed 208 tests, including the final
model-order regression. Three native console tests and four macOS Tauri console
tests passed. CLI dialect tests pass with and without the JavaScript feature;
existing JavaScript argument and startup tests pass. A CLI `matchmaker` smoke
fitted 30 C-alpha pairs with the built-in US-align implementation and reported
RMSD below 0.00001 angstrom. The rebuilt WASM passed 51 console and Tauri-shell
browser checks. These include actual WebGL/WebGPU ChimeraX selection, surfaces,
labels, measurements, PDB downloads and undo/redo, plus Tauri dialect persistence.

The read-only reference documentation contains these conflicts with its code:

- Section 2 says `rgb()` and `hsl()` are unsupported, whereas the parser and
  the later command table support them, their alpha variants and `gray()`.
- The residue description permits a leading `+`, but the atom-spec lexer does
  not accept it; `:+10` is rejected.
- The wildcard description includes `[!...]`, but the lexer does not consume
  `!` there. Rust follows the observed parser behavior.
- The common `save_object` description advertises an mmCIF writer that is not
  registered, as recorded in the object-export section below.

## View direction and depth dragging

The public `View::forward_vec` now points out of the screen, matching the C++
`getForwardVector` implementation. The internal depth-drag conversion retains
its previous sign. The read-only C++ TTY view confirmed identity and 90-degree
Y rotations. Fifteen view tests passed, including a direction/depth regression,
and the existing C++ input oracle compared 3,862 camera states successfully.
The public script vector was previously reversed; existing drag behavior is
preserved.

## Current-reference object export options

`save_object` and native `write` now accept `selection`, `format` and `link`.
The shared exporter validates the writer and selection before writing. A partial
write keeps the object's source and modified state even when `link` is true.
Null, empty and `*` selections are whole-object writes; those link by default,
and `link=false` preserves the source. Failed writes do not update the source.
The native PDB writer retains raw coordinates, as the reference writer does;
PyMOL exports retain their existing transformed-coordinate behavior.

Five focused export regressions cover PDB, SDF, PQR, XYZR and molecule QDF,
map QDF, surface QDF and lightweight QDF, including selected atoms, explicit
formats with a different filename extension, round trips, failed writes and
source linking. The complete shared command suite passed 195 tests. Rebuilt
WASM passed a browser test that downloads selected and complete molecules,
checks their contents and verifies the source after each link option.

The old `save_object` description advertised `.cif`, but the reference source
and the Rust writer registry provide no mmCIF writer. The updated local schema
lists supported writers and rejects `.cif` before creating a file. The read-only
reference description remains inconsistent with its available writers.

## Current-reference camera command options

`set_view` now accepts the reference scalar-first quaternion `[a, x, y, z]`,
normalizes it, and validates arguments before changing the camera. The existing
`get_view` rotation result remains `[x, y, z, w]` for compatibility with current
consumers. `fitSlab` uses transformed all-atom bounds and the current view center
and rotation, before applying any simultaneous center/rotation arguments. It
skips maps and keeps the view center and zoom. Distance is applied before slab
clamping. Empty geometry and malformed arguments leave the camera unchanged.

Camera commands no longer consume a scene undo entry or clear redo. They still
update native/browser viewports and remain recordable in QAX action logs.
Shared console checkpoints distinguish camera updates from scene edits, and
all three frontends acknowledge each update before the next command.

The shared command suite passed, including three camera-option regressions and
an MCP redo/action-log regression. Two native console tests and three macOS
Tauri console tests passed. Rebuilt WASM passed 15 console and selection tests,
including scalar-first rotation, current-center slab fitting, workspace focus,
geometry/picking refresh and redo preservation on WebGL and WebGPU. These are
finite behavior checks; they do not establish complete camera/UI parity.

## Current-reference selection and fit-box APIs

The current CueMol 3.0.0.554 source (`794fe4ac`) adds selection rewriting and
view-frame bounds after the published compatibility baseline. The script
registry now exposes every method/property name in that checkout's QIF files.
This inventory check establishes API presence, not equivalence of every method.

`SelCommand.simplify`, `invertSimplified`, `toggleSidechain` and `rewriteAround`
now share a Rust syntax-tree rewriter. It preserves the typed expression while
folding boolean identities, duplicate terms, common factors and compatible leaf
unions. Repeated representation changes no longer grow redundant selections.
Native, browser and Tauri selection actions use the same parser instead of
removing prefixes or parentheses from arbitrary text. Serialization retains
quoted property values, named references, insertion codes and f64 thresholds.
A C++ oracle compared 125 expressions and 625 rewrite/serialization results.

Molecules and density maps expose `getFitBoxJSON`; views expose `fitToBox`.
Bounds use the view's rotated axes, molecule margins, map cell origins and
object transforms. Empty selections leave the view unchanged, and thin axes
retain the reference minimum extent. Molecule fitting now resolves scene-scoped
names and cross-object operands and borrows molecule storage instead of cloning
it. All-atom fitting avoids allocating a full vector of atom indices. Direct
map fitting also retains its origin and object transform.

The rebuilt current C++ reference matched 136 molecule camera/bounds cases and
16 density-map bounds cases, including skew cells, origins and rotations.
Script regressions cover named selections, a different target view, empty and
inverted boxes, and map transforms. The affected final source passed 1,633
workspace tests, 191 native GUI model tests (one optional test ignored), and 169
Tauri model tests on macOS (six optional tests ignored). The Tauri DOM suite
passed 39 existing cases plus the new parser-routing/error case. Rebuilt WASM
passed 21 WebGL/WebGPU selection, navigation and sequence cases. These model and
browser checks do not replace OS-input validation or prove complete UI parity.

```sh
uv run --no-project python scripts/check-reference-selection.py \
  --build-dir .cache/reference-build --config .cache/reference-prefix/share/sysconfig.xml
uv run --no-project python scripts/check-reference-camera-fit.py \
  --build-dir .cache/reference-build --config .cache/reference-prefix/share/sysconfig.xml
```

The C++ checks require a compatible current reference library in the selected
build directory. Keep the reference source read-only and generated files in the
build/cache directories.

## MDCRD decoding and current-reference audit

An MDCRD regression accepted whitespace-separated rows during indexing but
failed when decoding the selected frame. The index and both coordinate readers
now use the same allocation-free field iterator. It retains fixed-width packed
negative values, short trailing fields, CRLF and Fortran D exponents. Indexed
frame decoding writes directly to the final coordinate vector; it no longer
copies the frame text or constructs intermediate f64 and Vector4 arrays.
Ordinary decimal tokens no longer allocate a normalized string. Line indexing
copies chunks up to newline boundaries instead of appending every byte.

The optimized Linux x86-64 benchmark used four synthetic boxed frames and 32
nonsequential frame reads per fresh process. Both executables used the same
Rust release profile and checksum work. Each cell below is the median of three
sequential processes; filesystem cache state and other host workloads were not
controlled. Peak RSS includes index construction, decoded input and checksums.

| Atoms | Stage | Before | After |
| ---: | --- | ---: | ---: |
| 20,000 | Index construction | 7.80 ms | 6.89 ms |
| 20,000 | Decode and checksum, 32 reads | 190.57 ms | 91.13 ms |
| 20,000 | Maximum process RSS | 4,704 KiB | 3,560 KiB |
| 200,000 | Index construction | 61.18 ms | 52.37 ms |
| 200,000 | Decode and checksum, 32 reads | 1,926.30 ms | 895.84 ms |
| 200,000 | Maximum process RSS | 23,104 KiB | 9,836 KiB |

Coordinate-bit and cell-bit hashes match before and after in every sample.
These numbers describe MDCRD indexing and frame decoding, not GUI frame time
or a comparison against the C++ application. Regressions also cover invalid
atom counts, truncated/excess values, nonfinite coordinates, invalid boxes and
arbitrarily split input chunks.

The current reference QIF audit identified missing selection-simplification
and fit-box methods introduced after the older compatibility baseline.
[Implementation status](implementation-status.md) tracks those additions and
the remaining full-feature checks.

## Scene saved state and optional execution follow-up

Scene status is now shared by commands, scripts and all three frontends.
The C++ scene oracle checks nine transitions: creation, an untracked edit,
a pending transaction, commit, undo, redo, clear, rollback and an empty
transaction. The reference reports untracked and pending edits as unmodified;
committed history or a modified scene-local style makes the scene modified.
Global style definitions remain outside a scene's saved state. The previously
documented same-source style-save behavior is preserved.

The initial regressions reproduced missing status changes after command and
script object edits, and a script flag that survived successful scene saving.
The runtime flag no longer enters saved QSC data. Frontend history records the
scenes actually edited by a script, including a non-active scene. Undo and redo
restore that status, and clearing history retains unsaved local style changes.

Failed native scene writes previously retained prepared Morph metadata or a
new current-camera entry. The common writer now prepares a scene separately
and applies it only after successful output. Script writers and embedded byte
exports use the same preparation boundary. Byte export itself does not change
document status. A browser download failure restores scene and camera state;
an initiated download clears history and embedded style flags. Browsers cannot
confirm that the download was finally written to disk.

The scene-export fixture lookup now resolves its default from the crate
directory. Previously it skipped despite the sibling reference fixture being
present. The optional Tauri animation export test now constructs its own
molecule, and the macOS workflow explicitly executes the five optional GPU
tests with a supplied profile input. Their execution results are recorded
separately from the ordinary model suite.

Those GPU tests create separate renderers because each test model starts its
snapshot revision from zero. Sharing the application's renderer across test
models reused a previous model's picking geometry. A GPU probe confirmed that
the original hover coordinate hits the CPK sphere with fresh geometry; the
hover assertions retain that coordinate and the unchanged-scene check.

The Windows window probe later observed the selection popup close while switching
to its Term tab. The driver now moves the pointer and waits for the tab's hover
pixels before pressing the button. Delaying pointer movement by 0.8 seconds on
the unchanged Linux CI binary reproduced the same timeout with the old driver;
the corrected driver passed the complete window check under that delay. A small
viewport test with movement, press and release in one egui frame also passed.
This isolates a missing input acknowledgement in the probe; the Windows event
ordering itself was not captured.

A later Windows probe timed out after moving from the Inspector to the viewport
and sending a wheel event. Delaying that movement by 0.8 seconds reproduced the
same zoom assertion failure on the unchanged Linux binary. The driver now
observes a toolbar hover transition before and after each pointer move that
precedes scrolling or dragging. This confirms delivery without changing text
focus. The full window check passes with delays on all three Inspector-to-viewport
wheel transitions, including the numeric and text draft assertions. Actual
Windows event ordering remains outside this controlled comparison.

A subsequent Windows check opened the named-selection definition popup but
reached its assertion with an empty name field. Delaying that popup by 1.2
seconds reproduced the same missing-name timeout with the previous driver.
The driver now waits for the popup border, input focus and clipboard contents
before submitting the name. Coordinate clicks also acknowledge pointer
delivery, while menu, tab and scrolling transitions wait for displayed changes.
The corrected full window check passes at both 944 by 608 and 1024 by 768
client sizes with eleven injected delays per run: seven menu or tab clicks
delayed by 1.2 seconds and four pointer moves delayed by 0.8 seconds. These
checks use the unchanged Linux CI binary; they do not capture the original
Windows event ordering. The model, clipboard, undo and draft assertions remain
in place.

The next Windows run passed the definition check but measured the Selection
pane while it was still scrolling. Its saved measurement and failure screenshot
place the same tab 55 pixels apart. The driver now requires the tab position to
remain unchanged for 0.6 seconds before using it for input coordinates. Splitting
the wheel input into two events 0.35 seconds apart reproduced a stale-coordinate
failure with the previous driver. The corrected full window check passes with
this condition and the eleven earlier injected delays. Its result records whether
scrolling was needed, the measured offset and the observed stable duration.

Run the scene-state C++ comparison with:

```sh
uv run --no-project python scripts/check-reference-scene-state.py \
  --build-dir .cache/reference-build \
  --config .cache/reference-prefix/share/sysconfig.xml
```

The reference build and configuration are optional local inputs. Ordinary
tests explicitly report a skipped oracle when they are unavailable; the
comparison helper requires both inputs and fails if execution fails.

The executed C++ checks include 136 camera fits/orientations, 651 label formats,
3,862 input camera states, 80 CCP4/MRC inputs, five continuous-potential cases,
25 map-level states, 21 regions, 15 LOD budgets, cube-cap counts and areas,
27 direct-surface configurations, all five backbone/spline tests, and 15 volume
transfer tables of 256 colors each. Backbone checks use the distributed 2.4.1
style configuration. The 1CRN scene-export check also executes with its fixture
present, and the real bundled POV-Ray test passes explicitly. These are finite
comparisons with the unchanged reference checkout.

## Configuration, palette and distribution follow-up

This follow-up preserves the distinction between palette-file colors
and live named-color references. XML imports resolve colors in the source scope
once; interactive definitions retain their references and modifiers. Tests cover
local shadowing, global edits across scene undo, and rejected palette loads that
leave existing scenes unchanged. Existing palette changes were retained during
this follow-up.

Configuration serialization now preserves empty values, leading/trailing spaces,
literal backslashes, tabs and line endings. Native files and browser storage use
the same versioned TSV encoding. Legacy files remain readable. Parsing completes
before applying any entry, so a malformed later row cannot partially change
preferences. CLI tests also confirm that reading preferences does not execute
`startup.js`.

A second review reproduced another loss: keys beginning with `#`, and a
whitespace-only key with an empty value, disappeared on reload. The decoder now
distinguishes versioned data rows from comments and blank lines. The encoder
escapes hash marks, and old unescaped versioned keys remain readable. Native
file, browser-text and CLI regression checks pass. The browser settings test
also checks that unrelated keys survive a settings edit.
The rebuilt browser passes all nine focused WebGL/WebGPU, palette and persisted
input checks, including the added configuration regression.

The native browser server now streams filesystem assets and embedded gzip data.
It no longer allocates the whole asset for a HEAD request or makes a temporary
decompressed copy for an uncompressed embedded response. On x86-64 Linux, three
sequential debug-build measurements of HEAD requests for a 128 MiB sparse file
gave these medians:

| Measurement | Before | After |
| --- | ---: | ---: |
| Process peak RSS | 144.81 MiB | 16.63 MiB |
| HEAD response time | 94.96 ms | 1.08 ms |

These measurements compare the Rust server before and after the change; they do
not measure browser rendering or establish a speedup over C++ CueMol. The Linux
regression bounds additional peak RSS below 32 MiB for that request. Separate
checks retain concurrent request handling and both embedded transfer encodings.

Browser CI now builds WASM and its native test server once, then distributes all
193 tests across four jobs with one worker each. Test discovery verified that
the shards contain no missing or duplicate tests. Workflow validation passes,
but the new GitHub Actions wall time has not yet been measured.

The headless workspace passes 1,597 tests. The native GUI and shell suites pass
257 tests; their one optional POV-Ray test also passes separately with the
bundled executable. A real Linux window passes mouse rotation, zoom, slab,
centering, selection, undo, text input and Inspector checks. The independent C++
input oracle compares 3,862 camera states. All five backbone oracle tests pass
with the distributed 2.4.1 style configuration, including geometry, colors,
triangle winding and atom ownership. Using the newer source style file instead
produces the expected DefaultRibbon dimension mismatch; it is a different
appearance baseline. The reference checkout remains unchanged.

Before the second review, all 193 Chromium tests passed in 21.2 minutes locally.
After the first configuration repair, the rebuilt, installed binary passed nine
focused browser checks for
WebGL/WebGPU palette references, global-style undo and persisted input settings.
The installed native window also passes the OS input and pixel checks. The Unix
installer, self-upgrade rollback tests and a fresh pip source installation pass.
Outside the checkout, the packaged command exposes 146 MCP tools, returns its
offline skill, runs startup scripts, and preserves configuration values.

That Linux archive contained the rebuilt native application and browser
assets. All 38 embedded assets matched their manifest hashes in both gzip and
identity responses, and HEAD requests return no body. Bundled Python helpers,
OIDN and POV-Ray rendering, and the APBS calculation pass from the installation;
PDB2PQR and FFmpeg also start from their installed bundle. The APBS output has
35,937 finite potential samples. Distribution staging and rejection checks pass
11 cases.

The second review also found that packaging assigned the current commit to
an executable without checking when it was built. Native binaries now expose
compiled source and feature metadata through `--build-info`. Browser builds
record their source state before staging. Packaging rejects dirty or mismatched
sources, and embedding verifies browser asset hashes. Final release validation
compares executable hashes, CLI/app identity and embedded browser manifests
across the platform matrix. These checks verify provenance and integrity; they
do not establish functional equivalence with every C++ feature.

The first live release run at `e71088f` caught a pipeline error on both Linux
targets: the pip source-install check overwrote the completed release executable
with its GUI-only build. Local inspection confirmed that the target executable
had no `bundled-web` feature and null browser metadata, while the archive made
before pip retained both. The workflow now runs pip validation before the final
application build. Distribution validation rejects this feature loss explicitly;
it does not relax the source or browser checks.

The next live browser run exposed continuous Selection sidebar growth. Trace
samples showed the value field gaining eight pixels per frame until the sidebar
reached its maximum width. The shared term editor had treated a text field's
inner width as its total allocation. Its outer size is now bounded, and the
candidate button's space follows the active theme. A deterministic egui check
failed at 268 pixels before the fix for a 260-pixel sidebar. The regression
checks five term layouts over 40 frames with both default and application
spacing; the browser also checks that choosing an atom-name term preserves the
pane width. The existing stability timeout remains unchanged.
CI previously built the UI crate only as a dependency when enabling egui, so
its gated unit tests did not run. The native-interface job now selects that
crate explicitly and runs its 20 tests as well as the desktop suite.
Local validation passed all 20 UI tests, 257 desktop tests and nine browser
selection executions. The new browser width assertion failed against the old
CI artifact with a 230-to-246-pixel increase before passing on the repaired build.

The first packaged ARM Linux window check then exposed a missing CI runtime:
`libxkbcommon-x11.so` was absent, although packaging and the command-line checks
had passed. The release and shared-test dependency lists now include
`libxkbcommon-x11-0`, matching the existing installation instructions and the
published-GUI workflow. This is a runner dependency fix; application code is
unchanged from the sidebar repair.

The same packaged-window run exposed incorrect test coordinates on Windows and
macOS. The test clicked the scene row instead of the molecule row because it
included the native title bar in its origin. It now obtains the client area from
the platform and verifies its requested dimensions while retaining the outer
window containment check. At small heights, it scrolls the Selection pane before
clicking controls below the fold and measures the actual scroll displacement
from the tab pixels. These are test repairs; the failed clicks did not establish
an application clipboard defect.
The repaired Linux check passes the full input/Inspector sequence at 1024 by
768 and 944 by 561 client pixels, plus the Selection sequence at 944 by 608.

Windows subsequently stopped while entering an optional numeric value. The
failure image showed the control disabled, but the test had checked only the
model value before typing. It now waits for the control pixels to change and
checks the copied numeric text before and after typing. It also verifies that
the draft remains uncommitted until Enter. These checks establish the actual
input target and preserve the existing property-value and undo assertions.

The next Windows run stopped earlier in the console. Its log contained
`close_logu` followed by `ndo`: Enter and the next command's first character
arrived in one frame. The shared editor consumed Enter before TextEdit processed
the remaining characters. A deterministic egui regression reproduced that
command corruption. Both frontends now partition raw input around console
action keys before egui processes it, retaining the remaining events in order
for subsequent frames. Regressions cover successive submissions, trailing draft
text, history edits, Shift+Enter and IME confirmation. A browser regression sends
two submissions and another draft synchronously through DOM input events.
The old browser joined all three strings into one command; the repaired browser
passes that case and all six existing console cases. Local checks also pass all
28 shared UI tests and 257 native GUI/shell tests.

The following Windows run passed the console checks but stopped at the lock
property. The failure image still showed the previous `name` filter, so the
checkbox click targeted a text field. Shared egui probes retained the filter
text for both batched clicks and batched Tab navigation. The OS check now waits
for the filter's focus border, verifies its copied text, and waits for focus to
leave after Tab before clicking a property. It retains the original property
change and undo assertions; the failed click alone did not prove a lock defect.
The View numeric check now waits for the expected numeric clipboard value, since
filter verification replaces the previous console text in the clipboard.

Windows then reached the font-property check and exposed a decoding error in the
test's MCP client. The Rust command writes UTF-8, but Python used the runner's
CP1252 default and rejected a font name. The client now decodes UTF-8 explicitly.
A subprocess probe with Japanese font names reproduces the failure under a
simulated CP1252 default and preserves the names after the repair.

The next Windows run reached the selection picker, but its captures showed no
popup before subsequent clicks. The driver had waited a fixed 0.2 seconds.
Pausing the local application for 0.8 seconds at the opening click reproduces
the missing popup and failed selection commit. The driver now brings the window
forward and waits for the popup pixels before using its controls. The same
delayed-frame run then passes, including the final selection and undo checks.

Windows subsequently passed the popup checks but barely moved the Selection
pane on a wheel event. PyAutoGUI 0.9.54 forwards its scroll argument as raw
Windows wheel data, while one notch is
[120 units](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mouse_event).
The test now converts notches to those units on Windows and keeps the existing
step count on Linux and macOS. A probe of the pinned backend reproduces the old
two-unit input and confirms the corrected 240-unit input for two notches.

Another Windows run exposed the same timing assumption inside the selection
popup: the keyword remained `hier` after the driver tried to choose `chain`.
Delaying the keyword menu frame by 0.8 seconds reproduces that failure locally.
The driver now confirms the term controls, menu and hovered row before clicking,
then checks the changed input layout, copied value and displayed draft. It also
restores foreground focus before dismissing the popup and checks that it closes.
The complete local window test passes with the same injected menu delay.

Generated archives and `dist/validation.json` remain local validation artifacts,
not a published release. The release workflow now checks actual packaged windows
on all five native targets. Local test results do not establish that those CI
jobs or the optional Tauri runtime passed; their results must be checked for the
specific pushed commit. The sharded CI wall time also requires a completed run.

## Render interface and image-export performance

The shell was compared with the distributed CueMol 2.4.1 application. Activity
and toolbar icons now use the same Phosphor family. Scene rows use hierarchy
arrows and visibility eyes, and the browser Color panel exposes paint targets,
named-color swatches, entry editing, reordering and deletion. Bottom-panel tabs
use the reference order and underline treatment. These checks cover the changed
panels; they do not establish exact parity for every dialog or console dialect.

The native and browser Render windows now share a preview, Image/Render settings
tabs, Start, Stop, Save PNG and a render log. They read the stored dimensions
instead of resetting them when opened. Cancellation retains the last completed
image. PNG image commands include the selected DPI in their physical-resolution
metadata; direct command calls retain their existing download/result behavior.
Physical size fields retain their unrounded values when DPI changes. A browser
regression reproduced a 1-inch image staying 64 pixels after changing 64 DPI to
128 DPI; the repair now changes the pixel dimensions while preserving inches.
The regression also checks fractional inch dimensions and saved PNG metadata.
The review follow-up reproduced a second DPI defect: a stored 64.5 DPI was read
as an integer, fell back to 600, and rendered a 1.3 by 0.7 inch image at 780 x 420
instead of 84 x 45. The panel and PNG encoder now preserve fractional DPI.

`web/benchmarks/render.mjs` compared the public browser build at `5d0d695` with
the repair on the same macOS host using Chrome 156. Each row is the median of
three runs, with the default scene geometry, perspective projection, 32 GI
samples, 3x supersampling and OIDN unchanged. The browser processes ran
sequentially. The measurements cover command-based Render image export, not
interactive viewport rendering or a comparison against the C++ renderer.

| Input and image size | Before | After | Speedup | Maximum WASM memory before / after |
| --- | ---: | ---: | ---: | ---: |
| 1CRN, 327 atoms, 640 x 480 | 5.41 s | 2.90 s | 1.86x | 63.75 / 64.63 MiB |
| 1CRN, 327 atoms, 1200 x 1200 | 26.02 s | 14.09 s | 1.85x | 168.25 / 169.13 MiB |
| 4HHB, 4,779 atoms, 640 x 480 | 18.76 s | 11.19 s | 1.68x | 114.69 / 100.06 MiB |

All decoded RGBA hashes matched across both builds and all samples. PNG byte
hashes differ because the new files contain DPI metadata. The largest observed
frame gap fell from 300 to 50.1 ms for the 1200-square case and from 166.7 to
83.4 ms for 4HHB. These memory values describe WebAssembly linear memory, not
whole-browser peak RSS; small-scene memory did not decrease.

The changes optimize the hot render/core WebAssembly code, reuse ray inverses,
visit nearer BVH nodes first and retain the original tie order for coincident
surfaces. The browser reuses its ray index across passes and yields through
MessageChannel between ray and denoising-guide batches. Regression checks cover
parallel/boundary rays, distance limits, accepted-hit filters and coincident
surfaces. The native GUI suite passed 174 tests with one explicit POV-Ray
exclusion, and all 69 shell compatibility checks passed. An input regression
checks selection, drag payloads, visibility, locking and tree expansion without
overlapping drag handlers. A Linux source window passed OS drag/wheel input,
font-family and multiline rendering, and Python helper export. Separate OS mouse
checks toggled object visibility and started a render; the displayed 64 x 48
preview matched the command-generated image pixel for pixel.
A further native window check preserved 1.3 by 0.7 inches at 64.5 DPI and
produced an 84 x 45 preview and PNG. The PNG stores integer pixels per metre,
so reading its resolution back gives 64.4906 DPI.

Browser checks cover the preview/save/cancel flow, a small window, physical units,
visibility and undo, and paint entry editing, addition, reordering and deletion.
They also reproduce two repaired paint defects: adding an entry passed an
unsupported argument, and selection was clamped against the old entry list before
the new entry could be moved. Named-color swatches are checked against the active
style definitions. Cross-browser input checks keep the Chromium-only IME
protocol case explicitly skipped on Firefox and WebKit.
An additional Chromium check delays the real OIDN model request, presses Stop,
then releases the request. Cancellation rejects the OIDN promise through the
JavaScript bridge; the earlier preview remains unchanged and no late download
occurs. This check did not reproduce the review's suspected cancellation leak.

Inspection of the v0.1.7 distribution exposed clipped bottom-panel tabs when the
Inspector reduced the central area in a small window. A 900-pixel browser window
reproduced a tab ending at x=681 while its panel ended at x=600. The native GUI
also failed to operate the hidden panel toggle. Both frontends now use icon tabs
when the labels would exceed the available width, retain names in tooltips and
reserve space for the collapse/expand control. Regression checks click all five
browser tabs and both panel controls, and exercise the native trajectory tab and
panel controls with the Inspector open. The tab strip also scrolls horizontally
when even the icons exceed its width. A separate browser check reproduced a
200-pixel panel reopening at 100 pixels. Expanded and collapsed layouts now keep
separate sizing state, so reopening preserves the expanded height.

The public Firefox run also exposed a timing assumption in two concurrency tests.
A render completed normally in about 0.77 seconds, before the PDB-dialog input
finished; the test incorrectly expected it still to be busy. These tests now hold
the real render task at a MessageChannel scheduling boundary, verify that queued
input cannot release its busy state, then cancel and release the task. The PDB
case checks that no download starts during the render and that HTTP failure is
handled afterward. The two tests passed three repetitions each on Chrome,
Firefox and WebKit, for 18 successful executions, without changing application
code for this timing issue.

## Browser text input and default appearance

The post-v0.1.6 browser repair follows the reported input and appearance defects.
The page's `input { display:none }` rule also hid eframe's text-input element,
preventing focus and Unicode/IME events. It now hides only the file chooser.
After that repair, actual screenshots exposed missing Japanese UI glyphs.
The browser now loads Noto Sans JP as a fallback for proportional and monospace
UI text. The build pins the upstream revision and checksums, instantiates regular
weight with a pinned fontTools version, and packages the font, SIL OFL license
and source record together. Generated fonts and downloads stay ignored.

The appearance baseline is the distributed CueMol 2.4.1 configuration. The
reference source checkout had newer Tritium defaults, which differed from that
application. Newly loaded molecules now use SteelBlue sheets, khaki helices,
yellow nucleic acids and FloralWhite coils/other atoms. Explicit molecule-order
palettes and saved paint entries remain supported. `tracestick` now receives
`DefaultTraceStick,DefaultHSCPaint`, and nucleic acids initially use `nucl`.
`DefaultRibbon` uses helix/sheet aspect ratios 6/7 with width 0.2 and coil width
0.35; `DefaultNucl` uses an elliptical section of width 0.5 and base size/thickness
0.5. These values were also read from the distributed C++ runtime after applying
the same styles.
These match the distributed style sheet, rather than the later source changes.

Local source validation passed 492 affected shared tests and 171 native GUI tests
with one explicit POV-Ray exclusion. Chrome passed seven focused browser cases,
including real Unicode input events, IME preedit replacement, Console execution,
Undo, visible Japanese glyphs, and default colors/styles on WebGL and WebGPU.
Firefox and WebKit each passed the five applicable text, glyph, Inspector and
WebGL-appearance cases. The Japanese glyph comparison failed before the font
repair. Regular-font builds on macOS and Linux produced identical SHA-256 hashes,
and staging verified all 36 browser assets with their licenses and provenance.

## Public release and installation

### Current release: v0.1.8

The [v0.1.8 release](https://github.com/mogura-rs/cuemol-rs/releases/tag/v0.1.8)
was published on 2026-10-30 UTC from application revision `f4ed01c`.
Its [release workflow](https://github.com/mogura-rs/cuemol-rs/actions/runs/41014610981)
passed shared and browser tests and all five native builds. The distribution
includes the Render interface, image-export optimization, Japanese input and
font repairs, reference default appearance, and compact-panel fixes above.
Each native build exercised Python helpers before packaging.

The [application CI](https://github.com/mogura-rs/cuemol-rs/actions/runs/41014327091)
passed 1,658 Rust tests with one explicit POV-Ray exclusion and 109
browser cases with one optional ICC-fixture exclusion. Ten browser cases use
a mock Tauri bridge. Separate public Pages checks passed 100
application cases in Chrome and 76 applicable cases across
Firefox and WebKit; the Chromium-only IME protocol case was skipped once in
each of the latter browsers. The [Pages deployment](https://github.com/mogura-rs/cuemol-rs/actions/runs/41015938022)
published all 37 assets at the expected source revision; each file matched its
manifest hash. The public checks include the real OIDN cancellation
path, all five compact bottom tabs, restored panel height, text entry, file
loading, rendering, picking, trajectories and stereo display.

All eight public archives matched `SHA256SUMS`, source/target/version metadata
and required license/provenance files, including the new Phosphor notice.
Fresh isolated Linux x86-64 and macOS Apple Silicon installations passed release
discovery, installation and self-update. Both passed APBS calculations, PDB2PQR
conversion of 1CRN to 642 atoms, FFmpeg encoding/decoding of four distinct frames,
native OIDN, MCP and PNG checks. All 37 embedded browser assets matched their
manifest, and the installed Linux server passed 17 application cases. The macOS
app bundle also restored deliberately removed resources and stale metadata
through self-update from a path containing spaces, retaining the published
binary hash.

The [published GUI matrix](https://github.com/mogura-rs/cuemol-rs/actions/runs/41016980111) passed on Windows x86-64,
Linux x86-64/ARM64 and macOS Intel/Apple Silicon. The macOS checks launched the
executable inside the published app bundle. Each target passed OS drag and
wheel input, displayed-pixel checks, font styles and multiline text, and Python
output. Captures were inspected for the revised icons and compact panel tabs.
The published Linux window separately passed visibility input and the 84 x 45
Render preview/PNG check at 1.3 by 0.7 inches and 64.5 DPI. Credential-store
checks used loopback providers; the real-cloud and hardware-stereo boundaries
below remain open.

The published browser renderer also retained the baseline decoded RGBA hashes
for 1CRN at 640 x 480 and 1200 x 1200 and for 4HHB at 640 x 480. The three-sample
performance comparison and its memory limits are recorded above.

An additional Apple M4 Mac mini with 16 GiB memory and macOS 15.6 passed the
public v0.1.8 command-line and app-bundle installation/update checks, bundled-tool
calculations, Python helpers and all 100 public browser cases in Chrome
156.0.8078.74. All 37 Pages assets matched their manifest. WebGPU reported the
vendor `apple`, architecture `metal-3` and `isFallbackAdapter=false`; WebGL
reported ANGLE Metal on Apple M4. Three sequential image-export samples per condition gave
medians of 2.82 s for 1CRN at 640 x 480, 13.81 s at 1200 x 1200 and 10.79 s for
4HHB at 640 x 480. Maximum observed WASM linear memory was 60.81, 165.31 and
101.56 MiB, respectively; these are not whole-browser RSS values. All nine
outputs retained the earlier baseline decoded RGBA hashes.

Native OS-input and screen-capture validation on that host remained unperformed:
the desktop was locked and the SSH test process lacked Accessibility and Screen
Recording permission. A separate AppKit pixel-format probe obtained accelerated,
double-buffered monoscopic formats, but adding
[`NSOpenGLPFAStereo`](https://developer.apple.com/documentation/appkit/nsopenglpfastereo)
returned no format, both with and without an explicit acceleration requirement.
This records the available API configuration on that Mac; it does not validate
a quad-buffer presentation backend. Real cloud-provider credentials were also
absent from the CueMol credential store and supported environment variables.

### Historical release: v0.1.6

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
on the same Linux x86_64 host on 2026-10-29 UTC. Both this comparison and the
molecular-I/O comparison below used these public executables:

| Release | Executable SHA-256 |
| --- | --- |
| v0.1.1 | `45d2b45509ed6ea7c84e186637d58a0f40ffb5520168d707a128f38dcd5f6900` |
| v0.1.6 | `859562a2312ab06d1236ddce0428290ae17ca6d934dbca7de2615dc180266da7` |

Both hashes matched the verified public archives. The retained comparison reports
include the individual samples, input dimensions, checksums and executable
provenance. Each byte-array sample allocated an array and
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
Separately from the hosted GUI matrix, the installed public v0.1.6 binary passed
the same checks in a 944 x 588 Linux window. Its additional capture and the five
hosted-platform captures confirm that counts appear only in the bottom status bar.

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
At this checkpoint the default load palette followed the newer Tritium source's
molecule order and persisted its color definitions in embedded scenes. Tests
covered noncarbon pivots, paint fallbacks, two distinct molecule colors and an
embedded save/reload. The later browser-input repair above restores the distributed
CueMol colors by default while retaining explicit palettes. Older synthetic coloring
fixtures omitted the explicit `$molcol` styles required by the C++ implementation;
those inputs now include the styles.

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
| Usage and an earlier validation entry described molecule-order colors as the CueMol default. | Distributed CueMol secondary-structure colors are the default; the newer source palette remains an explicit scene-style option. |
| Usage described the initial nucleic representation as cartoon. | It now uses the reference `nucl` renderer, including its base representation. |
| DefaultRibbon comments and checks used the newer source's section dimensions. | The bundled default styles now match the distributed CueMol 2.4.1 dimensions; explicit renderer properties still override styles. |
| The style constant comment claimed the bundled XML was vendored verbatim. | Its geometry defaults follow the distributed 2.4.1 styles, while newer molecule palettes remain optional. |
| The CI description said browser tests ran in one job. | Browser tests use four shards over one shared WASM build and native test server. |
| Map loading advertised `setMapCenter`, `moveViewCenter` and `auto`, while dispatch recognized only `view`. | Local and downloaded maps share the reference policy: crystallographic maps draw around the current view, EM maps fit the view, and explicit policies override that choice. `centerView=false` preserves the view. |
| The CCP4 command described keeping every nth grid point but resampled an already loaded map. | The command passes the stride to the reader, preserving its validation, sampled statistics and grid placement. |
| File-loading guidance described applying state only after success. | Native file and renderer-setup failures now restore the previous state, including a scene created during the failed operation. |
| Reader APIs return `Result`, but malformed GRO, PSF and AMBER counts or delimiters could panic or be ignored. | These malformed inputs return parse errors, and count fields do not trigger eager allocation. |

## Additional input and state audit

The previous green application and distribution checks did not cover several
failure paths. New regressions reproduced map-policy and sampled-statistics
errors, failed trajectory edits that retained changed block order, failed file
loads that left an empty scene, and failed renderer setup that retained a new
molecule. Separate malformed-input cases reproduced five parser panics and a
PSF bond-count error that was silently ignored.

The command regressions exercise both file and memory loading, the map-kind
default, disabled recentering, and an embedded QSC save/reload after sampling.
The trajectory regression truncates an already indexed file, then verifies
that failed removal, reordering and frame selection preserve block metadata
and molecular coordinates. Rollback copies trajectory metadata while retaining
the shared frame sources. Ordinary frame playback does not take a new snapshot.

Run these focused cases with:

```sh
cargo test --locked -p cuemol-command --test map_loading
cargo test --locked -p cuemol-desktop-core --test load_failure
cargo test --locked -p cuemol-mdtools --test malformed_input
```

That audit left saved-state tracking outside its repairs; the follow-up above
addresses that difference. This audit does not establish complete feature
parity or the absence of all defects. Generated validation records identify
the tested revision and the executed shared, native, browser and package checks.

A subsequent Windows package check timed out waiting for Center while the
Selection pane was not visible. The OS driver now waits for the selected
activity highlight and confirms draft text
through the clipboard before proceeding. Delaying the initial pane frame by
1.5 seconds reproduced the Center timeout with the previous driver. The same
binary passed the focused pane check and the full native window check with
the corrected driver and the same delay.

Another Windows check requested filter focus while the Inspector target was
changing from an object to the scene. The driver now waits for the displayed
target heading to change before filtering Scene or View properties. A
1.5-second delay reproduced editing the old object with the previous driver;
the corrected driver passed the full window check with that delay and preserved
the assertion that scene edits must not rename the selected molecule.


## Combined node focus against the current reference

`focus_nodes` frames the union of molecule selections and map cells in the
current view axes. Object targets use every atom; renderer and group targets
use their own selections. Invalid identifiers and nodes without fit boxes are
skipped. A single remaining map uses the existing map fitting path, including
its slab and distance behavior. The shared native console resolves a list of
names, UID references and patterns once, preserves order and removes duplicates.
Native, Tauri and browser tree selections invoke the same numerical operation.

Validation includes four shared geometry/console regressions within 186 passing
command crate tests, 24 native workspace tests and the Tauri workspace fit test.
A browser interaction test selects multiple objects in the Tauri shell. Actual
WebGL2 and WebGPU runs select a renderer and another transformed molecule, click
Focus, and verify the camera center, zoom, slab and viewport aspect ratio. Both
browser backends passed. The broader browser regression run passed all 69 tests
covering navigation, clipboard, scene tabs, selections and the Tauri shell.
Existing camera fit-box oracle coverage remains
applicable to each box before the union.


## Current native window and workspace checks

After the camera-history and atomic-save changes, the shared workspace run passed
1,681 tests with no failures or ignored tests. Optional reference checks still
require their documented fixtures; the current C++ camera/selection oracles and
ChimeraX/catalog comparisons were also run separately as described above.

The actual Linux native window passed `scripts/check-native-gui.py`, including
rotation, zoom, slab, centering, clipboard, console completion/multiline undo,
Python helpers, font pixels, Inspector editing and the Selection pane. The
additional `--current-features-only` run clicked the ChimeraX dialect, selected
an atom, changed representation/color, undid the submission, and redid it through
the OS keyboard shortcut. Both history operations retained the newer camera.
It then selected two transformed objects with the OS mouse and modifier key,
clicked Zoom, and verified their combined fit center and viewport-scaled zoom.
Captured pixels showed the red sphere representation and both selected rows.

These runs used Xvfb and the system's software graphics path. The display tools
were unpacked into the ignored cache without changing system packages. They
verify Linux OS event delivery and window rendering, not hardware acceleration.
The current macOS binary and Tauri model tests pass, but a fresh local macOS
OS-input/screen-capture run remains unverified because this process lacks those
permissions. Historical published multi-platform checks remain historical.


## MD coordinate updates without intermediate frame copies

Native and shared scene playback now copies directly from stored vectors or
decoded packed coordinates into the molecule. Selected trajectories validate
their indices first and apply only those coordinates, including nested selection
order. File-backed loading validates the first frame once before applying it,
instead of decoding that frame twice for atom-count checks. Source frames and
undo snapshots remain shared. Averaged playback retains its existing calculation.

`trajectory_playback_bench` and `scripts/benchmark-trajectory-playback.py` compare
four synthetic frames with 32 nonsequential updates. The table reports medians
from three fresh Linux x86-64 debug processes per condition, before and after the
change. Apply time includes file decoding where applicable; coordinate hashing
is outside that timer. Every coordinate bit in every visited frame contributed
to the same hash before and after, and the in-memory and indexed inputs matched.
These are scene-coordinate timings, not rendered frame rates. Other work was
running on the host, so small timing differences should not be overinterpreted.

| Storage | Atoms | 32 updates (ms) | Attach/create (ms) | Process peak RSS (MiB) |
| --- | ---: | ---: | ---: | ---: |
| memory | 20,000 | 29.33 -> 21.82 | 3.11 -> 2.77 | 17.09 -> 16.41 |
| indexed_dcd | 20,000 | 221.29 -> 175.62 | 24.70 -> 15.20 | 16.41 -> 15.97 |
| memory | 200,000 | 509.76 -> 457.73 | 30.62 -> 30.78 | 102.16 -> 96.03 |
| indexed_dcd | 200,000 | 2504.09 -> 2099.50 | 249.74 -> 159.02 | 84.87 -> 78.75 |

At 200,000 atoms, indexed DCD updates fell from 2.50 s to 2.10 s, attachment from
250 ms to 159 ms, and peak RSS from 84.87 MiB to 78.75 MiB. In-memory updates fell
from 510 ms to 458 ms and peak RSS from 102.16 MiB to 96.03 MiB. At 20,000 atoms,
the process peak did not resolve the smaller eliminated allocation.

A separate reproducer showed integer overflow when the eager trajectory's
averaging radius was `usize::MAX`. Saturating the upper frame bound now averages
the available frames without panicking. Invalid frame counts, indices and reads
continue to preserve coordinates, cell parameters and the current frame.

The affected shared suite passed 617 tests. Native GUI model tests passed 192
with one existing optional test ignored; Tauri passed 170 with six optional
tests ignored. The final coordinate iterator passed its two focused regressions
and the WASM target check. After rebuilding the browser application, all three affected browser MD checks
passed: indexed File seeking, agent-driven seeking with undo, and multi-object
trajectory append/undo/export/reload. Both corrected UI expectations also passed.
The preceding full browser run passed 210 of 212 cases; its two failures were
the older camera-undo and selection-string expectations corrected in that run.

The current Linux GUI binary also passed the real Python helper, APBS potential,
POV-Ray image and OIDN image checks. PDB2PQR and FFmpeg reported their versions.
Unix installer checks passed on Linux and macOS, including checksum rejection,
failed-replacement rollback and binary/app self-upgrade in temporary locations.
The macOS GUI binary built successfully; these checks did not grant OS input or
screen-capture permissions.

The `set_object_trajectory` comment previously claimed mismatched frames were
applied up to the shorter length. The code already rejected them; the comment
now describes the actual behavior.


## Current animation references and edit transactions

The current reference treats animation names as links to the first element
carrying that name. Renaming that element now updates its dependents without
changing their identities or absolute times. Deleting it makes its direct
children absolute at their current positions. A later duplicate name does not
own those links. Changing a reference preserves the current span and clamps a
negative new offset to zero while retaining the duration. Native and Tauri
property dialogs also preserve millisecond offsets when only the reference
changes. Explicit timing edits round to milliseconds like the reference editor.

A shared iterative graph reports missing, cyclic and upstream failures for each
element. Loaded QSC files can contain such chains and remain editable; playback
and seeking report the cause until the chain is repaired. An unrelated invalid
chain does not reject valid edits elsewhere. Native, Tauri and browser panels
show the unresolved rows and their cause. Tauri derives editable duration from
relative times, so a loaded unresolved element retains its authored duration.

A reproducer also showed invalid reference edits stopping native and Tauri
playback before validation. Both now validate first. Conversely, shared command
edits previously left playback running after an accepted change; they now stop
and restore the animated scene before committing the new manager. Runtime
snapshots are then discarded so later playback cannot restore obsolete values.
Rejected edits preserve playback and history. Generic timing writes must use
`set_anim_time`, unknown property resets fail atomically, and the structured
`tgtAlpha` spelling updates the persistent `tgt_alpha` property.

The read-only `check-reference-animation.mjs` oracle compared 128 generated
reference graphs, including every candidate name and time reference. All node
states, absolute spans and validation errors matched the current TypeScript
implementation. Run this optional comparison with:

```sh
CUEMOL_ANIM_REFERENCE="$(cd ../cuemol2 && pwd)" \
  cargo test --locked -p cuemol-command --lib animation_graph -- --nocapture
```

The animation and command crate suites passed 215 tests; the two ignored tests
require public metadata services or a real POV-Ray executable and are unrelated
to this change. The explicit graph oracle was also run separately. Native GUI
animation tests passed 25, and macOS Tauri model tests passed 24 with one existing
optional render test ignored. Nine affected browser cases passed: both WebGL
and WebGPU preserve time references through rename, removal, undo/redo and QSC
reload; both load and repair a broken chain; both retain pixel-verified playback,
pause and stop behavior; the shared startup path and two Tauri panel checks also
pass. The broken-chain browser screenshot was inspected. This verifies the
listed animation behaviors, not complete application parity.
