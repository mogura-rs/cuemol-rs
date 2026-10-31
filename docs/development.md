# Development

[Back to README](../README.md) · [User guide](usage.md) · [Validation](validation.md)

Rust, a C++20 compiler for the vendored numerical kernels, and the platform's
wgpu/winit system dependencies are required. The default workspace build is headless.
The published winit crate is resolved through Cargo. A small native input filter
uses raw X11 button states to discard duplicate wheel-release events.
An X11 session also requires the runtime `libxkbcommon-x11` library, even when
the application was built successfully. Debian and Ubuntu provide it in
`libxkbcommon-x11-0`.
Tauri is an optional alternate frontend and requires its platform SDKs.
The sibling `../cuemol2` checkout is an optional, read-only reference; set
`CUEMOL2_REF_DIR` when using a different location.

## Build from source

For Debian/Ubuntu native source builds, install `build-essential`,
`libxkbcommon-dev`, `libxkbcommon-x11-0` and `libwayland-dev`. On macOS, install
the Xcode command-line tools. On Windows, use the MSVC Rust toolchain and
Visual Studio Build Tools with the C++ workload.

```sh
cargo build --locked --release -p cuemol-cli --features gui
cargo run --locked --release -p cuemol-cli --features gui -- example.pdb
cargo run --locked --release -p cuemol-desktop --bin cuemol-rs --features tauri-gui -- example.pdb
cargo run --locked -p cuemol-cli -- web build
cargo build --locked --release -p cuemol-cli --features gui,bundled-web
cargo run --locked -p cuemol-cli -- tools bundle
```

`web build` installs the matching wasm-bindgen tool into the checkout cache when
necessary. It builds both the application and a smaller parser-only WASM module;
the parser worker uses the same readers and lossless transfer types as native
byte loading. Both modules and the worker scripts belong in browser distributions.
It also builds the vendored browser kernels with Emscripten, using
`EMXX`, an installed `em++`, or a pinned SDK installed in the checkout cache.
The kernel build requires `uv`; compiled kernels and the SDK are cached in CI.
`tools bundle` uses `uv` with hashed dependency pins; APBS builds on macOS and
Linux ARM also need CMake, SuiteSparse, BLAS, and platform compiler libraries.
The upstream Windows APBS executable is 32-bit and receives its matching
embedded Python runtime beside the executable. This runtime is separate from
the application's Python scripting bridge. Linux ARM64
builds OIDN with pinned ISPC and oneTBB sources; other targets use verified
prebuilt OIDN libraries. The native tests require the OIDN runtime bundle. See the
[release workflow](../.github/workflows/release.yml) for platform dependencies.
Generated builds, downloaded tools, browser packages, and test output are ignored.

The root `pyproject.toml` uses Maturin's binary packaging to install `cuemol`
from a Git URL or a local checkout. It builds the `gui` feature with the checked-in
Cargo lockfile. For a local Python installation, run `uv venv`, then
`uv pip install .`. This source installation does not build browser assets or
bundle external executables. To validate the same PEP 517 path used by pip, run
`uv run --no-project --with pip python -m pip wheel --no-deps . --wheel-dir .cache/wheels`.

The root `cuemol-rs.png` is the logo source. Tauri builds generate the RGBA icon
and webview copy; browser builds copy it into `pkg`, and macOS packaging generates
the ICNS sizes. These derived assets are ignored. Use a release build when
measuring interactive performance.

The native and browser egui shells use the Phosphor regular icon font through
`egui-phosphor`. Its notices are retained in `licenses/phosphor.txt` and included
in both browser assets and native tool bundles. The shared Render panel owns
preview/display state; rendering and PNG encoding remain in the command layer.

`web/benchmarks/render.mjs` measures browser image export at fixed quality. It
records PNG and decoded RGBA hashes, frame gaps, and WebAssembly linear-memory
sizes; those memory values are not total browser-process memory. For example:

```sh
cd web
node benchmarks/render.mjs --url http://localhost:8097/ --fixture ../example.pdb \
  --output ../.cache/render-benchmark --samples 3 --sizes 640x480,1200x1200
```

## Run checks

```sh
cargo fmt --all -- --check
cargo run --locked -p cuemol-cli -- tools bundle --oidn-only
cargo test --locked --workspace --exclude cuemol-desktop --exclude cuemol-web \
  --features cuemol-desktop-core/scripting,cuemol-command/scripting,cuemol-cli/scripting
cargo test --locked -p cuemol-desktop --features egui-gui --lib
cargo build --locked -p cuemol-cli --features scripting
uv run --no-project python scripts/check-python.py --binary target/debug/cuemol
scripts/test-install.sh target/debug/cuemol
```

The vendored `tiny_http` patch prevents accepted connections from remaining
queued behind idle keep-alive clients. Its upstream revision and local change
are recorded in `vendor/tiny-http/SOURCE.json` and `PATCHES.md`. The Linux CLI
regression uses `taskset` from util-linux and `kill` to submit a connection burst
before the server can dispatch it; every client must receive a response while
all connections remain open.

### Browser checks

For browser regression checks, build the web target and install Node.js with npm,
then run:

```sh
cd web
npm ci
npx playwright install chromium
npm test
```

## CI and release packaging

The development/test profiles omit debug information. Native release builds
avoid LTO, while the browser uses a separate size-oriented profile. CI caches
Cargo, wasm-bindgen, Emscripten, scientific kernels and Chromium. Shared and
native interface tests run once. Browser tests split across four jobs, each
using one worker and the same web artifact. The build job also supplies the
native test server, so browser jobs do not compile Rust. Local runs remain
sequential; use `npm test -- --shard=1/4` to reproduce one shard. Set
`CUEMOL_WEB_TEST_PREBUILT=1` to serve with an existing `target/debug/cuemol`.
Release and Pages jobs wait for all browser shards to pass before delivery.
Release jobs reuse that browser artifact and cache platform tool bundles
for five native targets. Completed tool bundles are saved before application
compilation, and Cargo dependencies are retained after failed release jobs.
`cuemol --build-info` reports the compiled source commit, dirty state, target,
features and embedded browser manifest checksum. Release packaging requires
clean native and browser builds from the requested commit. Rebuild after
committing; staging cannot assign a new commit to older browser assets. Final
release checks verify executable hashes and require identical CLI/app binaries
and the same browser manifest across all platforms.
Each native release binary also runs Python helper imports, live-scene queries,
selected PDB output and persistent interpreter checks before packaging.
The pip source-install check runs before the final application build because
maturin writes its GUI-only binary to the same Cargo target output. The final
build restores the bundled-browser features before validation and packaging.
Intel macOS has a longer timeout for builds without a populated cache.
A version tag publishes eight archives and checksums
only after every target succeeds. Failed uploads leave a draft that can be
resumed; an already published version is never overwritten. Manual release runs
build artifacts by default; select a version tag and enable `publish` to publish.

GitHub cannot restore a cache from a different tag. To prepare caches that later
tags can reuse, run the Release workflow on the default branch with `publish`
disabled after native build dependencies change. This also runs the distribution
checks without publishing. See GitHub's
[cache access restrictions](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#restrictions-for-accessing-a-cache).

The manual `Published native GUI` workflow downloads a selected public release
on all five targets and checks captured window pixels, OS mouse input and Python
helpers. It also checks native credential save/read/update/delete and environment
fallback against a loopback model catalog. The credential driver requires a
disposable GitHub-hosted runner and refuses to replace an existing provider key.
Linux uses an isolated D-Bus session and Secret Service data directory. Dummy
keys are removed afterward; these checks do not contact a real AI provider.

## GitHub Pages

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

## Compatibility fixtures

Compatibility tests may skip when the optional reference tree or external
fixture generator is absent. Git LFS pointer fixtures are reported as missing
payloads rather than being treated as molecular data. An explicit external
trajectory check is:

```sh
uv run --no-project --with mdtraj --with netCDF4 \
  cargo test --locked -p cuemol-mdtools --test gromacs_trajectory_compat
```

## Code organization

The workspace separates foundation and domain crates from shared application
state (`cuemol-desktop-core`) and commands (`cuemol-command`). Native, browser
and CLI frontends use those shared layers. See [CLAUDE.md](../CLAUDE.md) for
dependency rules, reader conventions and frontend boundaries.
