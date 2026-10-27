# cuemol2-rs

![](cuemol-rs.png)

Rust reimplementation of CueMol2.

This repository implements `../cuemol2` as a Rust-native workspace.
The original C++ tree is used as a specification, fixture, and test reference
only.

## Current workspace

- `cuemol-core`: errors, math, ranges, processes, streams, and shared primitives
- `cuemol-gfx`: colors, meshes, shaders, materials, hit testing, text, and GPU primitives
- `cuemol-qsys`: scenes, views, cameras, renderers, commands, styles, events, and I/O
- `cuemol-molstr`: molecular models, selections, readers/writers, and renderers
- `cuemol-importers`: external molecular-data import facades
- `cuemol-molvis`: interaction, measurement, label, and molecular renderers
- `cuemol-surface`: molecular-surface and grid-surface generation
- `cuemol-symm`: crystallographic symmetry operations
- `cuemol-xtal`: density maps, crystallographic readers, and map renderers
- `cuemol-molanl`: molecular analysis, RMSD, and structural superposition
- `cuemol-anim`: scene, camera, property, and renderer animation
- `cuemol-mdtools`: GRO, NAMD, AMBER, PSF, DCD, XTC, TRR, and NetCDF trajectory readers
- `cuemol-lwview`: lightweight scene viewer
- `cuemol-molclient`: molecular client facade
- `cuemol-render`: render snapshots, GPU/headless backends, and ray tracing
- `cuemol-desktop-core`: shared desktop application state and offscreen rendering
- `cuemol-desktop`: egui and Tauri desktop applications
- `cuemol-cli`: command-line validation utility
- `cuemol-node`: Node.js binding facade
- `cuemol-py`: Python binding facade

## Installation

After a release is published, install the command on Linux or macOS with:

```sh
curl -LsSf https://raw.githubusercontent.com/mogura-rs/cuemol2-rs/main/install.sh | sh
```

The installer verifies the release checksum and installs to `~/.local/bin`.
Set `CUEMOL_INSTALL_DIR` to change the destination. Run `cuemol self-upgrade`
to update the installed Unix binary. Release artifacts also include a macOS
application bundle and a Windows binary.

## Launch

Build the unified command with the native wgpu interface:

```sh
cargo build -p cuemol-cli --features gui
cargo run -p cuemol-cli --features gui -- example.pdb
cargo run -p cuemol-cli -- --help
```

Without the `gui` feature the command provides the headless utilities only.
The default configuration directory is `$XDG_CONFIG_HOME/cuemol`, or
`~/.config/cuemol` when `XDG_CONFIG_HOME` is unset. `CUEMOL_CONFIG_DIR`
overrides that directory. Place JavaScript commands in `startup.js` in that
directory to run them in the embedded script engine when the GUI starts.
Startup runs before command-line files are opened.

## Development

```sh
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo run -p cuemol-desktop --bin cuemol-desktop --features egui-gui
cargo run -p cuemol-desktop --bin cuemol-desktop-tauri --features tauri-gui
cargo run -p cuemol-cli -- select-dump "chain A and elem C"
cargo run -p cuemol-cli -- pdb-info ../cuemol2/tests/test_data/1CRN.pdb
cargo run -p cuemol-cli -- qdfpdb-info example.qdfpdb
cargo run -p cuemol-cli -- qsc-info ../cuemol2/tests/test_data/number_chain_atomintr1.qsc
cargo run -p cuemol-cli -- qsc-info ../cuemol2/tests/test_data/1crn_test1.qsc
cargo run -p cuemol-cli -- mol-array-info pdb ../cuemol2/tests/test_data/1CRN.pdb crd
cargo run -p cuemol-cli -- psf-info example.psf
cargo run -p cuemol-cli -- amberprm-info example.prmtop
cargo run -p cuemol-cli -- ambermdcrd-info example.mdcrd 327 box
cargo run -p cuemol-cli -- trr-info example.trr
cargo run -p cuemol-cli -- xtc-info example.xtc
cargo run -p cuemol-cli -- ambernetcdf-info example.nc
cargo run -p cuemol-cli -- opendx-info example.dx
cargo run -p cuemol-cli -- mol-sel-array pdb ../cuemol2/tests/test_data/1CRN.pdb "chain A and name CA"
cargo run -p cuemol-cli -- pdb-write ../cuemol2/tests/test_data/1CRN.pdb
cargo run -p cuemol-cli -- mol-delete-info cif ../cuemol2/tests/test_data/1crn.cif "name CA"
cargo run -p cuemol-cli -- mol-copy-info cif ../cuemol2/tests/test_data/1crn.cif "chain A and resi 1"
cargo run -p cuemol-cli -- stream-info all
cargo run -p cuemol-cli -- stream-search objreader ../cuemol2/tests/test_data/1CRN.pdb
```

Git LFS pointer fixtures are detected and reported as metadata when the actual
fixture payload is not present locally.
