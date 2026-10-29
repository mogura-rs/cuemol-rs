# Development

[Back to README](../README.md) · [User guide](usage.md) · [Validation](validation.md)

Rust, a C++20 compiler for the vendored numerical kernels, and the platform's
wgpu/winit system dependencies are required. The default workspace build is headless.
The published winit crate is resolved through Cargo. A small native input filter
uses raw X11 button states to discard duplicate wheel-release events.
Tauri is an optional alternate frontend and requires its platform SDKs.
The sibling `../cuemol2` checkout is an optional, read-only reference; set
`CUEMOL2_REF_DIR` when using a different location.

## Build from source

```sh
cargo build --locked -p cuemol-cli --features gui
cargo run --locked -p cuemol-cli --features gui -- example.pdb
cargo run --locked -p cuemol-cli -- web build
cargo build --locked --release -p cuemol-cli --features gui,bundled-web
cargo run --locked -p cuemol-cli -- tools bundle
```

`web build` installs the matching wasm-bindgen tool into the checkout cache when
necessary. It also builds the vendored browser kernels with Emscripten, using
`EMXX`, an installed `em++`, or a pinned SDK installed in the checkout cache.
The kernel build requires `uv`; compiled kernels and the SDK are cached in CI.
`tools bundle` uses `uv` with hashed dependency pins; ARM APBS builds
also need CMake, SuiteSparse, BLAS, and platform compiler libraries. Linux ARM64
builds OIDN with pinned ISPC and oneTBB sources; other targets use verified
prebuilt OIDN libraries. The native tests require the OIDN runtime bundle. See the
[release workflow](../.github/workflows/release.yml) for platform dependencies.
Generated builds, downloaded tools, browser packages, and test output are ignored.

## Run checks

```sh
cargo fmt --all -- --check
cargo run --locked -p cuemol-cli -- tools bundle --oidn-only
cargo test --locked --workspace --exclude cuemol-desktop --exclude cuemol-web
cargo test --locked -p cuemol-desktop --features egui-gui --lib
scripts/test-install.sh target/debug/cuemol
```

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
native interface tests run once; browser tests run once against the shared web
build. Release jobs reuse that browser artifact and cache platform tool bundles
for five native targets. A version tag publishes eight archives and checksums
only after every target succeeds. Failed uploads leave a draft that can be
resumed; an already published version is never overwritten. Manual release runs
build artifacts by default; select a version tag and enable `publish` to publish.

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
