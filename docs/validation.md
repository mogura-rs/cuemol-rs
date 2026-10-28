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

The pinned winit XInput2 path emitted a wheel event on both button press and
release. A source patch drops only releases of wheel buttons 4 through 7;
valuator scrolling is unchanged. The upstream archive checksum, commit and
license are retained with the patch, and release packages include its notices.

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
