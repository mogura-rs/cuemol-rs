# CueMol3

![](cuemol-rs.png)

CueMol3 is a molecular graphics application built primarily in Rust, with a
shared wgpu viewport for the native application and the browser. This workspace
ports CueMol2's scene, selection, molecular-rendering, crystallography,
trajectory, scripting, and command interfaces. The sibling `../cuemol2` checkout
is an optional, read-only compatibility reference.

## Install and launch

After a release is published, install on Linux or macOS with:

```sh
curl -LsSf https://raw.githubusercontent.com/mogura-rs/cuemol2-rs/main/install.sh | sh
cuemol example.pdb
cuemol self-upgrade
```

The installer verifies `SHA256SUMS` and installs `cuemol` and `cuemol-tools/` in
`~/.local/bin`. Add that directory to `PATH` if needed. Set `CUEMOL_INSTALL_DIR`
to choose another location, or `CUEMOL_VERSION=vX.Y.Z` to select a release.
The release includes Linux binaries, unsigned macOS application bundles and
binaries, a Windows binary archive, and a browser archive. Windows installation:

```powershell
irm https://raw.githubusercontent.com/mogura-rs/cuemol2-rs/main/install.ps1 | iex
```

The Windows destination is `%LOCALAPPDATA%/CueMol/bin`. The same
`cuemol self-upgrade` command updates an installed binary. Use `--check` to
check the latest version without installing it, or open **Help > Check for updates**
in the native application. A published release
is required for network installation; a source checkout alone is insufficient.

## Molecular viewing

The native and browser interfaces use Tritium's activity bar, Explorer,
scene tabs, right-hand Inspector, viewport tools, and bottom panels for console,
output, animation, sequence and trajectories. Both support light and dark themes.

Open files on the command line, through the file dialog, or by drag and drop.
The initial representation uses a protein ribbon, a nucleic-acid cartoon, and
ball-and-stick geometry for other atoms. The renderer controls remain available
for adding or changing representations, selections, coloring, and materials.
New molecules use Tritium's ordered palette, with their named colors embedded
in saved scenes. Renderer `$molcol` colors inherit the object's coloring scheme,
including hue, saturation, brightness and alpha modifiers.
Saved and direct molecular surfaces use their renderer's coloring scheme,
including paint rules and imported atom colors.
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

## Commands, MCP, and configuration

All installed entry points use `cuemol`; no separate JavaScript MCP launcher is
needed. The running native GUI exposes an authenticated loopback MCP endpoint.

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

```sh
cuemol exec get_scene_state '{}'
cuemol run commands.js
cuemol -c 'print(21 * 2);'
cuemol render example.pdb image.png --width 1200 --height 900
cuemol render example.qsc illustration.png --style npr
cuemol tools list
cuemol tools run pdb2pqr --help
cuemol tools run ffmpeg -version
```

`exec` starts a new headless scene; use a persistent MCP connection for a series
of edits. Relative output filenames are resolved from the process's working
directory. JavaScript runs in the embedded Boa engine and CueMol bindings, with
no Node.js runtime requirement.
Scene Undo, Redo and Rollback update script queries immediately. Transactions
preserve edits made before they start, changes in other scenes and later changes
to unrelated properties. An uncaught script error discards that evaluation's
scene edits and history.

Preferences are stored in `$XDG_CONFIG_HOME/cuemol`, falling back to
`~/.config/cuemol`. `CUEMOL_CONFIG_DIR` overrides the directory, and
`CUEMOL_CONFIG_FILE` overrides the TSV preference file. GUI preferences,
`config.tsv`, `startup.js`, and the local API descriptor use this directory.
Put startup JavaScript in `startup.js`; it runs before command-line files open.
`cuemol config path`, `get KEY`, and `set KEY VALUE` inspect or edit preferences.
Configuration inspection does not execute startup scripts. Download caches use
`$XDG_CACHE_HOME/cuemol` or `~/.cache/cuemol`, with `CUEMOL_CACHE_DIR` as an override.

Umbreon is implemented in Rust and uses a BVH for primary, shadow, AO, and GI
rays. USalign is compiled from the vendored reference implementation; its source
and license are retained. Release tool bundles contain APBS, PDB2PQR, FFmpeg, and the Intel OIDN runtime,
including a private PDB2PQR runtime. They do not require a system Python install.
Tool lookup checks configured paths, `CUEMOL_TOOLS_DIR`, the binary/application
bundle, the cache, and `PATH`. POV-Ray remains an optional external backend.

## AI Agent

Open **Tools > AI Agent** in the native application or **AI Agent** in the
browser. Enable it, choose `openai:model`, `anthropic:model`, or `google:model`,
and enter that provider's API key. The model field accepts arbitrary model IDs;
**Refresh available models** queries the provider's catalog. Settings are saved
in `agent.json` beside the other configuration files. Native keys can be stored
in the OS credential store, with `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or
`GEMINI_API_KEY` as a fallback. Browser keys remain in the current tab's memory.

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
cuemol agent models --model openai:gpt-5.6
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
WebGL2 fallback. Local file data stays in the browser unless shared through the optional AI Agent. File selection, drag and drop,
representations, properties, sequence, saved cameras, commands, undo, animation,
PNG export, and embedded QSC download use the shared Rust core. Image rendering
yields between row batches so the viewport remains interactive, and can be
cancelled from the status bar. Browser settings
and startup scripts are stored in localStorage for that origin.

Browser MD playback keeps a `File` handle and reads selected DCD/XTC/TRR/NetCDF/MDCRD
frames with `File.slice`, without first loading the whole trajectory into JS
memory. The browser cannot access arbitrary native paths or spawn external
tools. USalign uses the same vendored calculation kernel as the native application,
compiled into a companion WebAssembly module. LSQ and SSM remain shared Rust code.
ICC soft proofing uses the embedded LittleCMS engine on both targets. Open an
ICC/ICM file in the browser to activate it; the profile is retained in browser
storage. OIDN runs through WebGPU with a bundled neural-network model; it requires
WebGPU even when the interactive viewport uses WebGL2. Bilateral and A-trous
remain available without WebGPU. APBS runs in the native application; the browser can import APBS DX maps. Scenes with external
file references must be saved with embedded data before transfer to the browser.

## Build and check

Rust, a C++20 compiler for the vendored numerical kernels, and the native platform's wgpu/winit
system dependencies are required. The default workspace build is headless.
The published winit crate is resolved through Cargo. A small native input filter
uses raw X11 button states to discard duplicate wheel-release events.
Tauri is an optional alternate frontend and requires its platform SDKs.

```sh
cargo build --locked -p cuemol-cli --features gui
cargo run --locked -p cuemol-cli --features gui -- example.pdb
cargo run --locked -p cuemol-cli -- web build
cargo build --locked --release -p cuemol-cli --features gui,bundled-web
cargo run --locked -p cuemol-cli -- tools bundle

cargo fmt --all -- --check
cargo run --locked -p cuemol-cli -- tools bundle --oidn-only
cargo test --locked --workspace --exclude cuemol-desktop --exclude cuemol-web
cargo test --locked -p cuemol-desktop --features egui-gui --lib
scripts/test-install.sh target/debug/cuemol
```

`web build` installs the matching wasm-bindgen tool into the checkout cache when
necessary. It also builds the vendored browser kernels with Emscripten, using
`EMXX`, an installed `em++`, or a pinned SDK installed in the checkout cache.
The kernel build requires `uv`; compiled kernels and the SDK are cached in CI.
`tools bundle` uses `uv` with hashed dependency pins; ARM APBS builds
also need CMake, SuiteSparse, BLAS, and platform compiler libraries. Linux ARM64
builds OIDN with pinned ISPC and oneTBB sources; other targets use verified
prebuilt OIDN libraries. The native tests require the OIDN runtime bundle. See the
release workflow for platform dependencies. Generated builds, downloaded tools,
browser packages, and test output are ignored.

For browser regression checks, build the web target, then:

```sh
cd web
npm ci
npx playwright install chromium
npm test
```

The development/test profiles omit debug information. Native release builds
avoid LTO, while the browser uses a separate size-oriented profile. CI caches
Cargo, wasm-bindgen, Emscripten, scientific kernels and Chromium. Shared and
native interface tests run once; browser tests run once against the shared web
build. Release jobs reuse that browser artifact and cache platform tool bundles
for five native targets. A version tag publishes eight archives and checksums
only after every target succeeds. Failed uploads leave a draft that can be
resumed; an already published version is never overwritten. Manual release runs
build artifacts by default; select a version tag and enable `publish` to publish.

The Pages workflow deploys the browser artifact from successful `main` CI runs,
verifies its source commit, and skips superseded automatic builds. It does not
rebuild WASM or execute code from pull-request artifacts. Enable GitHub Actions
as the Pages source in repository settings before the first deployment. A manual
Pages run accepts a successful `main` push CI run ID, which also supports rollback.
After deployment, the workflow checks the live `build-info.json` commit. Relative
asset URLs work at both a domain root and a repository subpath.

The workflow implementation follows the official
[Pages deployment action](https://github.com/actions/deploy-pages) and
[cross-workflow artifact download](https://github.com/actions/download-artifact)
interfaces. Local validation does not publish a release or a Pages deployment.

Compatibility tests may skip when the optional reference tree or external
fixture generator is absent. Git LFS pointer fixtures are reported as missing
payloads rather than being treated as molecular data. An explicit external
trajectory check is:

```sh
uv run --no-project --with mdtraj --with netCDF4 \
  cargo test --locked -p cuemol-mdtools --test gromacs_trajectory_compat
```

## Compatibility boundaries

The ribbon, cartoon and tube paths use the reference cross-section profiles,
natural and smoothing splines, junctions, helix-width modes and end caps.
Their geometry, triangle winding, partial selections and color boundaries are
checked against an optional C++ oracle.
Secondary-structure assignment excludes proline donors and uses sparse
hydrogen-bond/bridge searches with bounded per-residue bond storage.

The wgpu frontends implement Tritium's layout with Rust egui controls.
The optional Tauri shell retains the older CueMol2 layout. Direct molecular
surfaces support distance fields, EDTSurf and MeshMS on native and browser
targets, with reference detail and surface-type controls. Saved molecular
surfaces support MeshMS and BALL, with `auto` selecting MeshMS and BALL as its
fallback; the `rust` backend retains grid-based SES. The generation backend,
density and probe radius survive embedded scene saves. Render settings use typed
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
Unsupported settings return errors. Native MCP ray tracing and APBS run in
background workers; the MCP call returns when its result is available.
These boundaries remain relevant when comparing complex reference scenes.

The workspace consists of foundation crates (`core`, `gfx`, `qsys`), molecular
and domain crates, shared `desktop-core` and `command` crates, and the `desktop`,
`web`, and `cli` applications. The Node.js and Python crates provide binding
facades. See [CLAUDE.md](CLAUDE.md) for code organization and
[validation.md](docs/validation.md) for measured checks and their scope.
