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
`cuemol self-upgrade` command updates an installed binary. A published release
is required for network installation; a source checkout alone is insufficient.

## Molecular viewing

Open files on the command line, through the file dialog, or by drag and drop.
The initial representation uses a protein ribbon, a nucleic-acid cartoon, and
ball-and-stick geometry for other atoms. The renderer controls remain available
for adding or changing representations, selections, coloring, and materials.
The Rendering menu offers built-in GI and hatched PNG output. Animation
rendering also defaults to the bundled Rust renderer. Maps, molecular surfaces,
saved cameras, measurements, labels, and animation
share the same application state and command API.

| Input | Action |
| --- | --- |
| Left drag | Rotate |
| Middle drag | Pan |
| Wheel or right drag | Zoom |
| Shift + wheel/right drag | Adjust slab thickness |
| Ctrl + wheel/right drag | Move the view depth |
| Double-click an atom | Center on the atom |
| `C` | Center the selection |
| `F` | Fit the selection |
| `Home` | Fit the scene |

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
and license are retained. Release tool bundles contain APBS, PDB2PQR, and FFmpeg,
including a private PDB2PQR runtime. They do not require a system Python install.
Tool lookup checks configured paths, `CUEMOL_TOOLS_DIR`, the binary/application
bundle, the cache, and `PATH`. POV-Ray remains an optional external backend.

## Browser

Release binaries embed the browser application:

```sh
cuemol web serve
cuemol web serve --port 8080 --no-open
cuemol web serve --dir web
```

Alternatively, serve the extracted `cuemol-web.zip` from an HTTPS host. WebGPU
requires a secure context; localhost is supported. `?renderer=webgl` selects the
WebGL2 fallback. Files stay in the browser. File selection, drag and drop,
representations, properties, sequence, saved cameras, commands, undo, animation,
PNG export, and embedded QSC download use the shared Rust core. Browser settings
and startup scripts are stored in localStorage for that origin.

Browser MD playback keeps a `File` handle and reads selected DCD/XTC/TRR/NetCDF/MDCRD
frames with `File.slice`, without first loading the whole trajectory into JS
memory. The browser cannot access arbitrary native paths or spawn external
tools. USalign uses the same vendored calculation kernel as the native application,
compiled into a companion WebAssembly module. LSQ and SSM remain shared Rust code.
ICC soft proofing uses the embedded LittleCMS engine on both targets. Open an
ICC/ICM file in the browser to activate it; the profile is retained in browser
storage. APBS runs in the native application; the browser can import APBS DX maps. Scenes with external
file references must be saved with embedded data before transfer to the browser.

## Build and check

Rust, a C++20 compiler for the vendored numerical kernels, and the native platform's wgpu/winit
system dependencies are required. The default workspace build is headless.
Tauri is an optional alternate frontend and requires its platform SDKs.

```sh
cargo build --locked -p cuemol-cli --features gui
cargo run --locked -p cuemol-cli --features gui -- example.pdb
cargo run --locked -p cuemol-cli -- web build
cargo build --locked --release -p cuemol-cli --features gui,bundled-web
cargo run --locked -p cuemol-cli -- tools bundle

cargo fmt --all -- --check
cargo test --locked --workspace --exclude cuemol-desktop --exclude cuemol-web
cargo test --locked -p cuemol-desktop --features egui-gui --lib
scripts/test-install.sh target/debug/cuemol
```

`web build` installs the matching wasm-bindgen tool into the checkout cache when
necessary. It also builds the vendored browser kernels with Emscripten, using
`EMXX`, an installed `em++`, or a pinned SDK installed in the checkout cache.
The kernel build requires `uv`; compiled kernels and the SDK are cached in CI.
`tools bundle` uses `uv` with hashed dependency pins; ARM APBS builds
also need CMake, SuiteSparse, BLAS, and platform compiler libraries. See the
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
Cargo and wasm-bindgen and runs shared tests plus one browser build. Release
builds reuse that single browser artifact, cache platform tool bundles, and
package the native application for five targets. A version tag publishes the
archives and checksums; manual runs only upload build artifacts.

Compatibility tests may skip when the optional reference tree or external
fixture generator is absent. Git LFS pointer fixtures are reported as missing
payloads rather than being treated as molecular data. An explicit external
trajectory check is:

```sh
uv run --no-project --with mdtraj --with netCDF4 \
  cargo test --locked -p cuemol-mdtools --test gromacs_trajectory_compat
```

## Compatibility boundaries

The wgpu frontends share scene behavior but use egui controls rather than
replicating Tritium's React layout pixel for pixel. Molecular surfaces support
the MeshMS and BALL engines on native and browser targets, with `auto` selecting
MeshMS and BALL as its fallback. The `rust` backend retains grid-based SES.
The generation backend, density, and probe radius survive embedded scene saves. NPR rendering provides a Rust hatching pass;
upstream custom hatch-layer specifications and every upstream light setting
are not implemented. Native denoising uses bilateral or color-guided A-trous
filters. Intel OIDN is not bundled; old GUI preferences carrying that label
migrate to the bilateral filter, and new MCP OIDN requests return an error.
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
