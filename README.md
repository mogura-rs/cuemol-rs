# cuemol2-rs

Rust reimplementation of CueMol2.

This repository is being rebuilt from `../cuemol2` as a Rust-native workspace.
The original C++ tree is used as a specification, fixture, and test reference
only.

## Current workspace

- `cuemol-core`: shared error, math, range, scene, ByteArray, and process primitives
- `cuemol-gfx`: color syntax parser and GenericCMYK proof-compatible cases
- `cuemol-molstr`: molecule model, PDB/QDF-PDB/mmCIF/SDF/MOL2/OpenDX/Xplor/CCP4/BRIX readers, gzip/bzip2/xz input helpers, selection parser, and QSC scene facade
- `cuemol-mdtools`: GRO, NAMD coordinate, AMBER topology/restart/mdcrd, and PSF topology readers
- `cuemol-xtal`: DensityMap model, map renderers, and MTZ/map reader facades
- `cuemol-render`: rendering abstraction and headless backend
- `cuemol-desktop`: desktop scene state facade for molecule and density map loading
- `cuemol-cli`: command-line validation utility
- `cuemol-node`: Node binding-facing facade for selection/color/PDB/stream entry points
- `cuemol-py`: Python bridge/facade for script execution plus selection/color/PDB/stream entry points

## Development

```sh
cargo test --workspace
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
