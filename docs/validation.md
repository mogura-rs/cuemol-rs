# Validation evidence

These measurements describe local validation of the implementation, not a
published release or a complete CueMol2 equivalence claim. Generated inputs and
logs are kept in the ignored checkout cache. No remote workflow was triggered.

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

## Remaining verification scope

Release workflows are configured for Linux x86_64/ARM64, macOS Intel/Apple
Silicon, and Windows x86_64. Local Linux and Apple Silicon builds plus browser checks were
executed during this work; the other native release targets and published
installation must be verified when pushing and running Actions is authorized.
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
All five workflow files pass actionlint. GitHub-hosted release and Pages runs
remain unexecuted because these changes have not been pushed.

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
browser assets also passed. These remain local checks; release publication and
GitHub Pages execution are intentionally pending authorization to push.


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

The registry now covers all 166 reference classes. The wider QIF audit still
reports missing members/defaults in maps, labels, trajectory loading
and exporters. Registry coverage alone does not establish their behavior.

All 32 browser tests passed after adding command acknowledgments, including
all three direct-surface engines and application-data persistence through script
edits and QSC reloads. The class-set test passed with the reference present;
13 command integration tests, 117 shared core unit tests and 165 native GUI unit
tests also passed. The latter two counts describe the selected feature sets,
not the separate unfinished QIF member/default audit.

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
verify partial occlusion, overlay display and alignment on Metal. The font
rasterizer uses bundled proportional and monospaced fonts, with synthetic bold
and italic; arbitrary operating-system fonts and exact reference glyph pixels
are not reproduced.

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
member/default audit remains unfinished for maps, trajectory readers and
exporters; those gaps are not covered by these passing suites.
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
14 and the native GUI passed 165. These counts exclude the separate unfinished
registry audit for map controls, trajectory scripting, hover and exporters.
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
audit still requires map controls, alignment methods, hover and exporter work.
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
Old QDF headers remain readable. Binary map data sharing and the remaining map
renderer/scalar scripting audit are separate follow-up work.

The shared core passed 209 tests and native egui passed 165 tests after this
change. The rebuilt browser passed the existing embedded-scene workflow and a
new half-float MRC/QSC round-trip regression. The latter also compares the
resulting density histogram. Linux could not check the optional Tauri shell
because its JavaScriptCore GTK system library was unavailable; that shell is
checked separately on Apple Silicon.
The Apple Silicon Tauri suite passed 142 tests with one optional test ignored.
