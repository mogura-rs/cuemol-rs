# User guide

[Back to README](../README.md) · [Development](development.md) · [Validation](validation.md)

Use this guide for viewing, scripting and rendering. For installation and the
first launch, start with the [README](../README.md#installation).

- [Molecular viewing](#molecular-viewing)
- [Commands, MCP and configuration](#commands-mcp-and-configuration)
- [AI Agent](#ai-agent)
- [Browser](#browser)
- [Compatibility boundaries](#compatibility-boundaries)

## Molecular viewing

The native and browser interfaces use Tritium's activity bar, Explorer,
scene tabs, right-hand Inspector, viewport tools, and bottom panels for console,
output, animation, sequence and trajectories. Both support light and dark themes.
When the central area is narrow, bottom-panel tabs use icons; hover to see their
names. If the icons still exceed the available width, scroll the tab strip
horizontally. The panel's collapse/expand button remains at the right edge, and
opening the panel restores its previous height.
The View menu provides parallel and cross stereo, eye swapping, and angular
separation. Each eye has its own projection, picking, labels and post-processing;
CPU image exports and offscreen depth captures use the same eye cameras.
Hardware quad-buffer stereo is unavailable on the wgpu frontends.
Center marks invert the displayed colors so they remain visible over molecular
geometry. Crosses, axes and labels retain their logical size on HiDPI displays.

### Representations and rendering

Open files on the command line, through the file dialog, or by drag and drop.
Browser file parsing runs in a worker so navigation and edits remain available.
Use **Cancel loading** or Escape to stop the current input and discard queued
files. **Get PDB** uses the same queue and also cancels its network request.
A failed load keeps existing edits and redo history. Changing the active scene
while an input is queued, downloading or being parsed prevents its late result
from entering the new scene. Loading waits for an active image render, and
cancelling queued inputs leaves that render running and cancellable.
The initial representation uses a protein ribbon, the nucleic-acid (`nucl`) renderer, and
ball-and-stick geometry for other atoms. The renderer controls remain available
for adding or changing representations, selections, coloring, and materials.
New molecules use CueMol's secondary-structure colors: SteelBlue for sheets,
khaki for helices, yellow for nucleic acids and FloralWhite otherwise.
A scene style can opt into molecule-order colors with
`<palette id="default_paint" value="okabe_ito"/>`; `classic` restores the default
for subsequently loaded molecules. Explicit palettes retain their named color
definitions in saved scenes. Renderer `$molcol` colors inherit the object's coloring scheme,
including hue, saturation, brightness and alpha modifiers.
The browser accepts Unicode text and IME composition in its text fields.
Its UI includes a Japanese fallback font; Console input and object names use
the same text-input path.
Saved and direct molecular surfaces use their renderer's coloring scheme,
including paint rules and imported atom colors.
Spline selections retain the whole chain's interpolation coefficients, so a
selected pair of residues follows the same curve as the complete backbone.
Native and browser viewports share GTAO, edge-aware AO filtering, FXAA, SMAA,
and 2–32 temporal samples. Spatial AA works independently of AO. Camera,
geometry, or setting changes reset temporal accumulation; unchanged frames reuse
the completed image. Optional half-resolution AO refines at full resolution when
the view stops changing. Scene properties include the SMAA edge threshold, and
these settings survive QSC saves. Camera PNG exports apply full-resolution AO
and supersampling while preserving transparent backgrounds.
The Rendering menu offers built-in GI and hatched PNG output. Animation
rendering also defaults to the bundled Rust renderer. Maps, molecular surfaces,
saved cameras, measurements, labels, and animation
share the same application state and command API.
Atom-based measurements retain their object and atom references across scene
saves, so their geometry and displayed values follow coordinate and transform
changes, including measurements between different molecules.
Atom labels support per-renderer and per-atom formats, horizontal and vertical
alignment, pixel offsets and optional display above molecular geometry.
The shared GPU path clips label pixels against scene depth in native, browser
and Tauri viewports. Label text and settings survive embedded scene saves.
Native labels use installed font families, with bundled proportional and
monospaced fallbacks. Browser labels use CSS family lists through OffscreenCanvas.
Font styles and weights accept either letter case. Glyph shape and hinting can
vary with the fonts installed on each platform.

### Mouse and keyboard

| Input | Action |
| --- | --- |
| Left drag | Rotate with the reference virtual trackball |
| Right drag | Pan |
| Wheel | Zoom without changing camera distance |
| Shift + left drag, horizontal | Adjust slab thickness |
| Shift + left drag, vertical | Zoom |
| Left + right drag, horizontal | Move the view depth |
| Left + right drag, vertical | Roll the view |
| Click an atom | Toggle its name label |
| Double-click an atom | Toggle its residue selection |
| Shift + double-click | Extend selection from the previous residue in the same chain |
| Atom context menu | Center, select, or expand the selection |
| `C` | Center the selection |
| `F` | Fit the selection |
| `Home` | Fit the scene |

The table describes the Default mouse preset. Settings also provide Coot, PyMOL,
iPhone and Trackpad presets, automatic device detection, trackball sensitivity
and optional momentum. Trackpad mode uses two-finger scrolling to pan and pinching
to zoom. Preset bindings and custom `UserViewConf` styles use the same resolver as
scripted `ViewInputConfig` changes. Dragging starts at two logical pixels; mixed
operations lock to the dominant axis until release. The native and browser
viewports share this input calculation.

The viewport palette and `N`, `B`, `L`, `D`, `A`, `T`, `E` keys select navigation,
rectangle selection, lasso selection, distance, angle, torsion, and bond editing.
Rectangle and lasso drags replace the selection; hold Shift to add atoms. A click
without a drag retains the atom-label behavior. Measurement tools collect two,
three, or four atoms, and Escape cancels a pending set. Bond editing requires two
different atoms in one molecule. The options buttons choose a measurement label
set or remove non-standard bonds. Each completed edit has one undo entry.
The palette cap and **View > Tool palette** share a persisted collapse setting.
Hover information and highlighting are configurable in Settings.

### Picking and hover feedback

Native, WebGL, WebGPU and Tauri views share geometry-based GPU picking and a
translucent hover fill with a two-tone outline. Picking follows visible sphere,
bond and mesh fragments, including ribbon and nucleic-base atom ownership.
`ViewInputConfig.gpu_pick` switches to the CPU point-picking path, and
`hover_hl_color` sets the fill color. `View.setHoverHit` and `clearHoverHit` update
transient highlighting without adding scene geometry or undo entries. Locked
objects/renderers and GPU sections with opacity at or below 0.6 do not
participate. Symmetry and other renderers without GPU hit names retain their
CPU picking fallback. The GPU ID buffer uses half the viewport resolution;
cached geometry and asynchronous readback avoid rebuilding it on pointer motion.

### Density maps and volumes

CCP4/MRC density maps support header inspection, strict grid subsampling and
voxel limits through `CCP4MapReader` and the desktop file dialogs. Native reads
use a row buffer and quantized voxel storage; compressed files spool to a
temporary seekable file. MRC origins and crystallographic/EM map-kind evidence
are preserved. QDF/QSC saves retain the reader's quantization range.
OpenDX and QDF electrostatic potentials retain shared 32-bit floating-point
samples and trilinear sampling; embedded scenes use `qdfpot` for these objects.
Scripts expose the quantization step, map origin, top-fraction density levels,
and an editable `map_type` with its resolved crystallographic or EM kind.
EM contours use the occupied top fraction, while crystallographic contours use
standard deviations. Default EM levels are capped by occupied voxels; explicitly
setting the same numeric level overrides that cap. Reset and embedded scene
storage preserve this distinction, and both desktop panels expose the matching
absolute or native level range.
`gpu_mapvol` uses interpolated 3D density textures and the reference rainbow
opacity table. Native, WebGL and WebGPU views share the same volume pipeline;
CPU rendering also composites volume samples with molecular geometry. Volume
opacity, transforms and scene storage remain editable through the shared API.
Map regions default to the whole block for EM and a centered box for other
scalar maps. `region_mode`, `lod`, `lod_budget` and `zoom_refine` control the
shared extraction. Automatic full-region sampling uses a cell budget, and view
refinement retains existing geometry while it covers the visible area. Native,
Tauri and browser navigation debounce region updates by 150 ms.
Isosurface `cap_mode` controls boundary faces independently of that region:
`auto` caps full regions and generated surface objects, `on` always caps, and
`off` leaves them open. Molecular coloring uses the nearest selected atom in
`target`, with the renderer's coloring scheme and the molecule's inherited
colors. Both selection and coloring survive embedded scene storage.

### Molecular dynamics

Load an MD topology and trajectory through the MD dialog or `load_trajectory`.
GRO, PDB, mmCIF, PSF, AMBER topology, SDF, and MOL2 topologies are accepted.
DCD, XTC, TRR, AMBER NetCDF and ASCII MDCRD trajectories retain a frame index and decode the
requested frame. Coordinates and periodic cells update together. Undo history
shares trajectory sources and map voxels, and frame playback does not add an
undo snapshot per frame. Compressed trajectories spool to a temporary seekable
file; this uses disk space proportional to the decompressed input. MDCRD reads
standard coordinate rows and detects a three-length periodic box row. For
ambiguous one-atom files, the diagnostic command accepts `--box` or `--no-box`.

```sh
cuemol trajectory-info trajectory.xtc
cuemol trajectory-info trajectory.dcd 10
cuemol trajectory-info trajectory.mdcrd 10 --atoms 20000 --box
```

The optional second argument is the frame stride. Legacy `dcd-info`, `xtc-info`,
`trr-info`, and `ambernetcdf-info` commands retain their diagnostic reader behavior.

## Commands, MCP and configuration

### Connect to the application with MCP

All installed entry points use `cuemol`; no separate JavaScript MCP launcher is
needed. The running native GUI exposes an authenticated loopback MCP endpoint.

Property commands edit molecular, map, surface and lightweight objects through
shared state. They preserve renderer identities even when names are empty or
duplicated, and resets restore the effective values used for rendering. View
properties update the active view and camera; unknown view properties fail.
Browser file loading accepts OBJ/PLY surfaces, QDF molecules/maps/potentials/
surfaces/lightweight objects, and PyMOL sessions in addition to the molecular and
map formats. QDF dispatch uses its content rather than assuming a molecule.

```sh
cuemol mcp config
cuemol mcp status
cuemol mcp connect
cuemol mcp serve
cuemol mcp serve --http 127.0.0.1:8081
cuemol mcp tools
cuemol mcp call get_scene_state '{}'
cuemol mcp connect --upstream
```

`connect` bridges stdio to the running GUI. `serve` owns an independent headless
scene, and `config --headless` prints the corresponding client configuration.
`connect --upstream` bridges to CueMol2's existing local API. The native catalog
includes all 124 operation names in the reference catalog used for this port,
plus convenience operations. This is command coverage, not a claim that every
parameter combination or rendering algorithm is identical; see the boundaries
below. `mcp tools` is the authoritative schema for the installed build.

### Run scripts and export files

```sh
cuemol exec get_scene_state '{}'
cuemol run commands.js first-argument second-argument
cuemol -c 'print(21 * 2);'
cuemol render example.pdb image.png --width 1200 --height 900
cuemol render example.qsc illustration.png --style npr
cuemol export-scene example.pdb model.obj
cuemol render-qsc example.qsc scene.pov
cuemol tools list
cuemol tools run pdb2pqr --help
cuemol tools run ffmpeg -version
```

`exec` starts a new headless scene; use a persistent MCP connection for a series
of edits. Relative output filenames are resolved from the process's working
directory. JavaScript runs in the embedded Boa engine and CueMol bindings, with
no Node.js runtime requirement. `getCmdArgs(-1)` returns the number of script
arguments; `getCmdArgs(0)` returns the first argument. Both `run` and `-c` preserve
quoted spaces and empty arguments. Read-only scripts leave Undo and Redo intact.

Native `PythonBridge` uses a persistent `uv`-managed interpreter per application.
Install `uv` to use it. Python variables survive `runString` and `runFile` calls;
`import cuemol` provides live scene objects, services, properties and callbacks.
For example, the JavaScript console accepts:

```javascript
var py = getService('PythonBridge');
py.runString('import cuemol\nscene = cuemol.scene()\nprint(scene.name)');
py.runString('print(scene.uid)');
```

Python execution requires a native process and is unavailable in the browser.
The embedded helpers include `fileio`, `mol_util`, `renderer`, `scene_util`,
`cmd`, `net_fetch`, `logging`, `UndoTxn` and the wrapper compatibility modules.
They operate on the live scene, including object copies, selections, transforms,
compressed input and scene exporters such as Umbreon. Third-party scientific
Python libraries are not bundled. The reference `povrender` helper is outside
this compatibility work.

`export-scene` and `render-qsc` use the same readers, default representations and
scene geometry as the GUI. They support STL, OBJ, POV-Ray, PNG, MQO, LuxRender,
LuxCore and Warabi output. PNG output uses the shared render settings, including
saved dimensions. Triangle formats export geometry; volume data remains available
to image renderers rather than being converted into an unrelated isosurface.
Scene Undo, Redo and Rollback update script queries immediately. Transactions
preserve edits made before they start, changes in other scenes and later changes
to unrelated properties. An uncaught script error discards that evaluation's
scene edits and history.

### Preferences and startup scripts

Preferences are stored in `$XDG_CONFIG_HOME/cuemol`, falling back to
`~/.config/cuemol`. `CUEMOL_CONFIG_DIR` overrides the directory, and
`CUEMOL_CONFIG_FILE` overrides the TSV preference file. GUI preferences,
`config.tsv`, `startup.js`, and the local API descriptor use this directory.
Put startup JavaScript in `startup.js`; it runs before command-line files open.
`cuemol config path`, `get KEY`, and `set KEY VALUE` inspect or edit preferences.
Configuration inspection does not execute startup scripts. Download caches use
`$XDG_CACHE_HOME/cuemol` or `~/.cache/cuemol`, with `CUEMOL_CACHE_DIR` as an override.

### Image rendering

Open Render to preview an image before saving it. The Image tab sets the size,
physical units, DPI and transparent background; the Render tab controls quality,
projection, lighting and NPR settings. Start Render uses the current view and the
saved scene settings. Stop cancels the running render and retains the previous
completed preview. Save PNG writes that preview, including its DPI metadata.
When dimensions use inches, centimeters or millimeters, changing DPI preserves
the physical size and updates the output pixel count.
The command APIs retain their direct image/download behavior.

Umbreon is implemented in Rust and uses a BVH for primary, shadow, AO, and GI
rays. Its script exporter applies typed scene render settings, including physical
image size, DPI, lighting, adaptive antialiasing, reduced-resolution AO and NPR
styles. `getRenderLog()` drains actual render diagnostics; `getHatchStyleSpec()`
returns an editable preset. Adaptive sampling preserves silhouette detail, and
AO interpolation rejects depth or normal discontinuities. Auxiliary sampling
buffers are bounded to strips. Native file exporters support asynchronous
rendering; browser image commands use the same renderer and browser downloads.
Script file-export threads require a native filesystem and report an error in
the browser. The Rust integrator does not promise pixel identity with Embree.

### Bundled tools

USalign is compiled from the vendored reference implementation; its source
and license are retained. Release tool bundles contain APBS, PDB2PQR, FFmpeg,
POV-Ray and the Intel OIDN runtime, including a private PDB2PQR runtime. They do
not require a system Python install.
Tool lookup checks configured paths, `CUEMOL_TOOLS_DIR`, the binary/application
bundle, the cache, and `PATH`. The POV-Ray console build includes its standard
include files, native runtime libraries, licenses and pinned source archive.
`cuemol tools run povray` and both desktop rendering dialogs use that bundle.

## AI Agent

Open **Tools > AI Agent** in the native application or **AI Agent** in the
browser. Enable it, choose `openai:model`, `anthropic:model`, or `google:model`,
and enter that provider's API key. The model field accepts arbitrary model IDs;
**Refresh available models** queries the provider's catalog. Settings are saved
in `agent.json` beside the other configuration files. Native keys can be stored
in the OS credential store, with `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or
`GEMINI_API_KEY` as a fallback. Browser keys remain in the current tab's memory.
On Linux, saved keys require a running Secret Service, such as GNOME Keyring,
in the desktop session. Environment variables or `--api-key-stdin` remain
available when that service is absent.

Sending a message shares the prompt, scene description, tool results, and any
captured images with the selected provider. Replies stream into the panel.
Specialized tool groups are enabled as needed; the full MCP catalog remains
available through those groups. Each turn is limited to 16 model rounds and
creates one undo entry for its successful scene edits. Cancel keeps edits
already shown. A manual scene edit cancels the pending turn, and late download
or APBS results are discarded. Network requests, image rendering, and native
APBS calculation run without blocking the panel. Browser filesystem and external
process restrictions still apply.

```sh
cuemol agent run example.pdb --prompt 'Show chain A as a ribbon' --save result.qsc
cuemol agent models --model openai:MODEL_ID
cuemol agent key set openai < key.txt
cuemol agent key delete openai
```

`key set` reads standard input. `run` and `models` also accept `--api-key-stdin`
for a session key, `--endpoint URL` for a compatible API server, and
`--reasoning default|low|medium|high`. `default` leaves reasoning settings to the
provider. Model availability, reasoning support, and API charges depend on the
provider and account. Conversation history stays in memory; **New conversation**
clears it.

The Rust adapters use [OpenAI Responses streaming](https://developers.openai.com/api/docs/guides/streaming-responses),
[Anthropic Messages streaming](https://platform.claude.com/docs/en/build-with-claude/streaming),
and [Google streaming generation](https://ai.google.dev/api/generate-content).
Provider-specific reasoning data and signatures are preserved between tool
rounds. Switching providers carries over only the user and assistant text.

## Browser

Release binaries embed the browser application:

```sh
cuemol web serve
cuemol web serve --port 8080 --no-open
cuemol web serve --dir web
```

Alternatively, serve the extracted `cuemol-web.zip` from an HTTPS host. WebGPU
requires a secure context; localhost is supported. `?renderer=webgl` selects the
WebGL2 fallback. Local file data stays in the browser unless shared through the optional AI Agent.
File selection, drag and drop,
representations, properties, sequence, saved cameras, commands, undo, animation,
PNG export, and embedded QSC download use the shared Rust core. Image rendering
yields between ray-tracing and denoising-guide batches so the viewport remains
interactive. Stop in the Render window or Cancel in the status bar cancels it.
Browser settings and startup scripts are stored in localStorage for that origin.

### Trajectories and browser storage

Browser MD playback keeps `File` handles and reads selected DCD/XTC/TRR/NetCDF/MDCRD
frames with `File.slice`, without first loading whole trajectories into JS
memory. Additional files append as ordered blocks for the selected molecule;
multiple molecules retain independent frame positions. Loading a topology and
several trajectories together preserves their input order. Undo shares the file
handles, and scene downloads interleave them with QSC fragments as browser Blobs.
### Native and browser differences

The browser cannot access arbitrary native paths or spawn external
tools. USalign uses the same vendored calculation kernel as the native application,
compiled into a companion WebAssembly module. LSQ and SSM remain shared Rust code.
ICC soft proofing uses the embedded LittleCMS engine on both targets. Open an
ICC/ICM file in the browser to activate it; the profile is retained in browser
storage. OIDN runs through WebGPU with a bundled neural-network model; it requires
WebGPU even when the interactive viewport uses WebGL2. Bilateral and A-trous
remain available without WebGPU. APBS runs in the native application; the browser can import APBS DX maps.
Scenes with external
file references must be saved with embedded data before transfer to the browser.

## Compatibility boundaries

The ribbon, cartoon and tube paths use the reference cross-section profiles,
natural and smoothing splines, junctions, helix-width modes and end caps.
Their geometry, triangle winding, partial selections and color boundaries are
checked against an optional C++ oracle. Object transforms apply to molecular
coordinates before glyph construction, preserving atom radii and thermal tensor
axes. Renderer transforms apply to the resulting geometry. Uniform scales keep
sphere/cylinder primitives; nonuniform scales and shear tessellate them while
retaining picking identities, section materials and transparency. Mesh normals
use the inverse transpose, and reflections preserve outward face orientation.
Secondary-structure assignment excludes proline donors and uses sparse
hydrogen-bond/bridge searches with bounded per-residue bond storage.

The native and browser wgpu frontends implement Tritium's layout with Rust egui
controls. The optional Tauri shell provides the same activity bar, inspector,
themes and bottom docks through its webview. All show the `cuemol-rs` application
name and the repository's PNG logo. Direct molecular
surfaces support distance fields, EDTSurf and MeshMS on native and browser
targets, with reference detail and surface-type controls. Saved molecular
surfaces support MeshMS and BALL, with `auto` selecting MeshMS and BALL as its
fallback; the `rust` backend retains grid-based SES. The generation backend,
density and probe radius survive embedded scene saves. Scripted
`MolSurfObj.sesbackend` selects `auto`, `meshms` or `ball` for both creation and
regeneration, including after scene reload. Render settings use typed
scene application data, including nested backend settings, script access and undo. NPR rendering includes the nine reference styles and editable line, dot, and
stipple layers, with shared native/browser controls for ink, paper, and tone.
Hatch and tone specifications use the reference syntax. The Rust ray tracer
uses bounded diffuse path tracing, metallic reflection, camera-relative lights,
and a sky/ground environment. AO supports multiple distance scales and bent
normals; compatible supersamples reuse lighting within each output pixel.
Contact/crease contours and both section-alpha blend modes are configurable. OIDN denoises
floating-point indirect irradiance with albedo and normal guides, then restores
the direct-light contribution and original alpha. Full-frame OIDN is also
available through `denoiser=2`. Native builds load the bundled CPU runtime,
while browser builds use a packaged WebGPU model. Bilateral and color-guided
A-trous filters are also selectable. `CUEMOL_OIDN_LIBRARY` overrides the native
library location. Missing runtimes or unsupported browser devices report errors.
The A-trous implementation uses increasing B3-spline sample spacing, following
the [wavelet filtering approach](https://doi.org/10.2312/EGGH/HPG10/067-075),
with color and alpha guidance rather than the paper's additional geometry data.
Unsupported render settings return errors. The legacy PNG exporter's `interlace`
and `compresslev` properties retain the C++ behavior: they are accepted but do
not change PNG encoding. Native MCP ray tracing and APBS run in
background workers; the MCP call returns when its result is available.
These boundaries remain relevant when comparing complex reference scenes.

The workspace consists of foundation crates (`core`, `gfx`, `qsys`), molecular
and domain crates, shared `desktop-core` and `command` crates, and the `desktop`,
`web`, and `cli` applications. The Node.js crate provides a binding facade; native Python connects to live
application state through a persistent worker. See [CLAUDE.md](../CLAUDE.md) for code organization and
[validation.md](validation.md) for measured checks and their scope.
