# cuemol2-rs

Rust reimplementation of CueMol2.

This repository is being rebuilt from `../cuemol2` as a Rust-native workspace.
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
- `cuemol-mdtools`: GRO, NAMD, AMBER, and PSF readers
- `cuemol-lwview`: lightweight scene viewer
- `cuemol-molclient`: molecular client facade
- `cuemol-render`: render snapshots, GPU/headless backends, and ray tracing
- `cuemol-desktop-core`: shared desktop application state and offscreen rendering
- `cuemol-desktop`: GPUI desktop application
- `cuemol-cli`: command-line validation utility
- `cuemol-node`: Node.js binding facade
- `cuemol-py`: Python binding facade

The Dioxus desktop frontend is maintained in the excluded
`crates/cuemol-desktop-dioxus` package and is checked separately.

## Development

```sh
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo check --manifest-path crates/cuemol-desktop-dioxus/Cargo.toml --all-targets
cargo run -p cuemol-cli -- select-dump "chain A and elem C"
cargo run -p cuemol-cli -- pdb-info ../cuemol2/tests/test_data/1CRN.pdb
cargo run -p cuemol-cli -- qdfpdb-info example.qdfpdb
cargo run -p cuemol-cli -- qsc-info ../cuemol2/tests/test_data/number_chain_atomintr1.qsc
cargo run -p cuemol-cli -- qsc-info ../cuemol2/tests/test_data/1crn_test1.qsc
cargo run -p cuemol-cli -- mol-array-info pdb ../cuemol2/tests/test_data/1CRN.pdb crd
cargo run -p cuemol-cli -- psf-info example.psf
cargo run -p cuemol-cli -- amberprm-info example.prmtop
cargo run -p cuemol-cli -- ambermdcrd-info example.mdcrd 327 box
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
