# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`cuemol-rs` is a Rust reimplementation of CueMol2 (a molecular graphics / structural
biology viewer). It is a Cargo workspace with shared native, browser, and command application layers.

The sibling C++ checkout `../cuemol2` is used **only** as specification, fixture source,
and test oracle. Treat it as read-only reference — never modify it, and do not assume it
is present (CI and most checkouts lack it; see "Compatibility tests" below).

## Commands

```sh
cargo build --workspace
cargo run --locked -p cuemol-cli -- tools bundle --oidn-only
cargo test --locked --workspace --exclude cuemol-desktop --exclude cuemol-web
# Compatibility tests can skip when optional fixtures or tools are absent.
cargo test -p cuemol-molstr                 # one crate
cargo test -p cuemol-molstr selection       # one test (substring filter on test fn name)
cargo test -p cuemol-molstr --test selection_compat   # one integration-test file in tests/
cargo clippy --workspace --all-targets
cargo fmt                                    # no custom rustfmt/clippy config — defaults apply

# Unified command and headless reader diagnostics
cargo run -p cuemol-cli -- pdb-info ../cuemol2/tests/test_data/1CRN.pdb
cargo run -p cuemol-cli -- --help
cargo run -p cuemol-cli --features gui -- example.pdb
cargo run -p cuemol-cli -- web build
cargo build -p cuemol-cli --features gui,bundled-web

# Desktop GUI — two feature-selected front ends over the shared core:
cargo run -p cuemol-desktop --bin cuemol-desktop --features egui-gui
cargo run -p cuemol-desktop --bin cuemol-rs --features tauri-gui
```

To run compat tests against a reference tree in a non-default location, set
`CUEMOL2_REF_DIR=../cuemol2`.

## Architecture

### Crate dependency layers

Dependencies form a strict DAG. New crates must respect this ordering:

- **`cuemol-core`** — foundation, no internal deps. Math (`Matrix4`/`Vector4`/`Quat`),
  `ByteArray`, `RangeSet`, error type (`CueMolError`/`Result`), LDOM2 XML, streams,
  primitive scene types, script-compat shims.
- **`cuemol-gfx`** (← core) — color parsing, pixels, draw attributes, GPU prims.
- **`cuemol-qsys`** (← core, gfx) — the object/scene system: `Scene`, `SceneObject`,
  `Renderer`, `Camera`, `View`, `CmdMgr`, `UndoManager`, `StyleManager`, IO handlers.
  Port of the original CueMol2 `qsys` module.
- **`cuemol-molstr`** (← core, gfx, qsys) — molecule model + all molecular/map file
  readers. The largest crate and the dependency of most domain crates.
- **Domain crates** (← molstr/qsys and below): `cuemol-render`, `cuemol-molvis`,
  `cuemol-surface`, `cuemol-symm`, `cuemol-xtal`, `cuemol-mdtools`, `cuemol-molanl`,
  `cuemol-anim`, `cuemol-importers`, `cuemol-lwview`, `cuemol-molclient`.
- **Apps / bindings** (top of the DAG): `cuemol-cli` (unified command), `cuemol-web` (browser),
  `cuemol-web-loader` (browser parser worker),
  `cuemol-desktop` (GUI; integrates everything), `cuemol-node` / `cuemol-py`
  (binding-facing facades).

### Two-tier scene system (important)

There are intentionally **two** `Scene`/`Object`/`Renderer` definitions:

- `cuemol_core::scene` — lightweight value-type primitives (`Uid`, `Object`, `Scene`).
- `cuemol_qsys` — the full system with commands, undo, camera, view, events.

`cuemol-molstr::qsys` then layers a molecule-specific **facade** on top, prefixed `Mol*`:
`MolScene`, `MolObject`, `MolRenderer`, `MolSceneManager`, `MolCommandManager`, `MolView`.
`MolSceneManager` is what the CLI and desktop use to load `.qsc` scenes. When adding
scene-level features, decide which tier you are extending and keep the facade thin.

### C++-compatible dual naming

Each crate's `lib.rs` ends with a block of `pub type` aliases (e.g.
`PDBFileReader = PdbReader`, `Matrix = Matrix4`, `MainChainRenderer = TraceRenderer`) and
many types carry an `L` prefix (`LString`, `LScrObject`, `LMsgLog`, `LRegExpr`). These
mirror original C++ class names so the port stays greppable against `../cuemol2`. Prefer
the Rust-native name in new code; keep the alias when it maps to a real C++ symbol.

### File-reader API convention

Format readers in `molstr`/`mdtools`/`xtal` follow a consistent shape — match it for new
formats:

- `read_<fmt>_file(path)` / `read_<fmt>_str(s)` — convenience entry points.
- `read_<fmt>_file_with_options(path, opts)` / `..._str_with_options` — with a
  `<Fmt>ReadOptions` struct.
- A `<Fmt>Reader` struct holding the parse logic; readers return `MolCoord` (molecules)
  or a `GridMap`/map type (density), via `cuemol_core::Result`.

`StreamManager` (in `molstr::stream`) provides content-sniffing handler dispatch and
transparent gzip/bzip2/xz decompression.

### Compatibility tests & fixtures

Two test styles coexist:

- Inline `#[cfg(test)]` unit tests in each module.
- `tests/*_compat.rs` integration tests that validate output against the C++ reference,
  reading fixtures from `../cuemol2/tests/test_data/` (override via `CUEMOL2_REF_DIR`).

**Compat tests self-skip when the reference tree is absent** — they `return` early instead
of failing. A green `cargo test` does NOT prove compat tests ran. When changing a reader
or selection logic, confirm the relevant `*_compat.rs` actually executed (run with
`../cuemol2` present). Git LFS pointer fixtures (payload not pulled) are detected and
reported as metadata rather than parsed — see `QscLoadStatus::GitLfsPointer`.

### GUI: shared core and front ends

GUI-agnostic logic lives in **`cuemol-desktop-core`** (no windowing toolkit): the headless
`DesktopAppState` facade, the `Gui*Summary` view-models, and the `offscreen` wgpu mesh
renderer (behind core's `offscreen` feature). `cuemol-desktop` exposes two optional
front ends:

- **`egui-gui`** — native egui/winit panels with direct wgpu rendering.
- **`tauri-gui`** — a Tauri webview shell whose Rust commands share `DesktopAppState`
  and composite the wgpu viewport as a PNG data URI.

The default workspace build stays headless (no GPU stack); keep new GUI code gated.

`cuemol-command` sits above `desktop-core` and owns the shared tool catalog, MCP
protocol, settings, and headless commands. The native and browser frontends use
this layer for the same edits. `cuemol-web` uses the extracted wgpu viewport
pipelines directly; do not introduce PNG round trips into interactive rendering.
Keep filesystem and process operations behind native target gates. Browser file
loading uses bytes or `File.slice`; embedded QSC is the portable scene format.
File parsing uses the shared `PreparedFile` boundary in a dedicated worker.
Preserve floating-point bits, atom order/IDs, reader options and map statistics
in transfers. Apply scene IDs and undo entries on the UI thread only after
parsing succeeds, and reject results from cancelled or superseded requests.

Camera zoom is the full world-space view height, not its reciprocal. Preserve
this convention across saved cameras, CPU rendering, and both GPU frontends.
Use the shared projection rather than recreating camera math in each frontend.

Indexed trajectories share their source across undo snapshots. Frame playback
must not create full undo copies. Density-map voxels are shared immutable storage
until replacement. Prefer bounded decoding and reusable GPU buffers.

Native USalign uses the vendored C++17 implementation and retains its license;
the browser calls a companion Emscripten module built from the same sources. APBS, PDB2PQR, FFmpeg and POV-Ray are packaged as separate
executables using `cuemol tools bundle`. Keep generated downloads and runtimes in
the ignored cache, and preserve their license/provenance files in distributions. OIDN uses a bundled
native CPU runtime and a pinned WebGPU model in the browser. Its inputs are
floating-point color, albedo and signed normals; the browser bridge encodes
normals into the model input range. Keep alpha and direct lighting outside the
GI denoiser. Bundle OIDN before running the native render tests.

The native GUI build opens a window when invoked with no arguments. Headless
builds print usage. Installed interfaces should be exposed through `cuemol`.
Configuration belongs in the shared config directory and generated data in the
cache directory. Read-only config commands must not execute startup scripts.

Use `cargo fmt --all`, the affected shared tests, and native GUI tests for GUI
changes. Run browser tests locally after web changes. CI remains focused on
shared tests and builds; avoid adding browser downloads to every native matrix
job. Release builds consume one web artifact and cached platform tool bundles.

`cuemol-agent` owns the optional streaming provider adapters and turn loop. It
uses the shared command schemas and enables specialized groups lazily. Network
workers exchange tool requests with the UI; scene objects never cross threads.
Keep real API keys out of settings, logs, serialized status, and test fixtures. Test
with loopback mock providers and dummy keys; do not call paid APIs in tests.
Native keys use the OS credential store and browser keys stay in tab memory.
One turn owns one GUI undo entry. Cancellation keeps visible edits but must not
apply queued calls or late asynchronous downloads/APBS output. Manual edits
invalidate an active turn. Preserve opaque provider reasoning and signature
fields when replaying history.
