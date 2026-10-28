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

The final Linux x86_64 archive was approximately 123 MiB and included the native
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
