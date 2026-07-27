# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`cuemol2-rs` is a Rust reimplementation of CueMol2 (a molecular graphics / structural
biology viewer). It is a Cargo workspace of ~19 `cuemol-*` crates.

The sibling C++ checkout `../cuemol2` is used **only** as specification, fixture source,
and test oracle. Treat it as read-only reference — never modify it, and do not assume it
is present (CI and most checkouts lack it; see "Compatibility tests" below).

## Commands

```sh
cargo build --workspace
cargo test --workspace                      # ~300 tests; compat tests self-skip if ../cuemol2 is absent
cargo test -p cuemol-molstr                 # one crate
cargo test -p cuemol-molstr selection       # one test (substring filter on test fn name)
cargo test -p cuemol-molstr --test selection_compat   # one integration-test file in tests/
cargo clippy --workspace --all-targets
cargo fmt                                    # no custom rustfmt/clippy config — defaults apply

# CLI validation utility (the fastest way to exercise readers end-to-end)
cargo run -p cuemol-cli -- pdb-info ../cuemol2/tests/test_data/1CRN.pdb
cargo run -p cuemol-cli                      # prints full subcommand list

# Desktop GUI — two feature-selected front ends over the shared core:
cargo run -p cuemol-desktop --bin cuemol-desktop --features egui-gui
cargo run -p cuemol-desktop --bin cuemol-desktop-tauri --features tauri-gui
```

To run compat tests against a reference tree in a non-default location, set
`CUEMOL2_REF_DIR=/path/to/cuemol2`.

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
- **Apps / bindings** (top of the DAG): `cuemol-cli` (validation tool),
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

### GUI: shared core + two front ends

GUI-agnostic logic lives in **`cuemol-desktop-core`** (no windowing toolkit): the headless
`DesktopAppState` facade, the `Gui*Summary` view-models, and the `offscreen` wgpu mesh
renderer (behind core's `offscreen` feature). `cuemol-desktop` exposes two optional
front ends:

- **`egui-gui`** — native egui/winit panels with direct wgpu rendering.
- **`tauri-gui`** — a Tauri webview shell whose Rust commands share `DesktopAppState`
  and composite the wgpu viewport as a PNG data URI.

The default workspace build stays headless (no GPU stack); keep new GUI code gated.
