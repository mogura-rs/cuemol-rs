mod agent;
mod export;
mod external_tools;
mod runtime;
mod web;
use cuemol_mdtools::{
    read_amber_mdcrd_file, read_amber_netcdf_file_with_options, read_amber_prmtop_file,
    read_amber_prmtop_with_restart_files, read_amber_restart_file, read_dcd_file_with_options,
    read_gro_file, read_namd_coor_file, read_namd_coor_with_psf_files, read_psf_file,
    read_trr_file_with_options, read_xtc_file_with_options, AmberNetCDFReadOptions, DcdReadOptions,
    TrrReadOptions, XtcReadOptions,
};
use cuemol_molstr::{
    parse_selector, read_brix_map_with_header_file, read_ccp4_map_with_header_file,
    read_mmcif_file, read_mol2_file, read_opendx_file, read_pdb_file, read_qdfpdb_file,
    read_sdf_file, read_xplor_map_with_header_file, write_pdb_string, write_sdf_string,
    HandlerCategory, MolCoord, MolSceneManager, QscLoadStatus, StreamManager,
};
use cuemol_molvis::scene_style_manager;
use cuemol_surface::{read_msms_surface_files, read_obj_file};
use cuemol_xtal::read_mtz_file;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return launch_default();
    };

    match command.as_str() {
        "--help" | "-h" | "help" => {
            print_usage();
            Ok(())
        }
        "gui" => launch_gui(args.map(std::path::PathBuf::from).collect()),
        "--version" | "-V" => {
            println!("cuemol {}", cuemol_command::update::current_version());
            Ok(())
        }
        "self-upgrade" => {
            let options: Vec<_> = args.collect();
            if options == ["--check"] {
                match cuemol_command::update::check()? {
                    Some(release) => println!(
                        "{}: {} ({})",
                        if release.newer {
                            "Update available"
                        } else {
                            "Up to date"
                        },
                        release.version,
                        release.url
                    ),
                    None => println!("No published release is available yet."),
                }
                Ok(())
            } else if options.is_empty() {
                self_upgrade()
            } else {
                Err("Usage: cuemol self-upgrade [--check]".into())
            }
        }
        "mcp" => runtime::mcp(args.collect()),
        "agent" => agent::run(args.collect()),
        "web" => web::run(args.collect()),
        "tools" => external_tools::run(args.collect()),
        "config" => runtime::config(args.collect()),
        "run" | "-c" | "--eval" => runtime::script(&command, args.collect()),
        "exec" => runtime::execute(args.collect()),
        "render" => runtime::render(args.collect()),
        "trajectory-info" => runtime::trajectory_info(args.collect()),
        "pdb-info" => {
            let path = args.next().ok_or_else(|| "missing PDB path".to_string())?;
            let mol = read_pdb_file(path).map_err(|err| err.to_string())?;
            print_mol_info(&mol);
            Ok(())
        }
        "qdfpdb-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing QDF-PDB path".to_string())?;
            let mol = read_qdfpdb_file(path).map_err(|err| err.to_string())?;
            print_mol_info(&mol);
            Ok(())
        }
        "pdb-write" => {
            let path = args.next().ok_or_else(|| "missing PDB path".to_string())?;
            let mol = read_pdb_file(path).map_err(|err| err.to_string())?;
            print!("{}", write_pdb_string(&mol));
            Ok(())
        }
        "export-scene" => {
            let input = args
                .next()
                .ok_or_else(|| "missing input molecule path".to_string())?;
            let output = args
                .next()
                .ok_or_else(|| "missing output path (.stl/.obj/.pov/.png)".to_string())?;
            export::run(&input, &output)
        }
        "render-qsc" => {
            let input = args
                .next()
                .ok_or_else(|| "missing input .qsc path".to_string())?;
            let output = args.next().ok_or_else(|| {
                "missing output path (.stl/.obj/.pov/.png/.mqo/.lxs/.cfg/.wbp)".to_string()
            })?;
            export::run(&input, &output)
        }
        "mmcif-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing mmCIF path".to_string())?;
            let mol = read_mmcif_file(path).map_err(|err| err.to_string())?;
            print_mol_info(&mol);
            Ok(())
        }
        "sdf-info" => {
            let path = args.next().ok_or_else(|| "missing SDF path".to_string())?;
            let mol = read_sdf_file(path).map_err(|err| err.to_string())?;
            print_mol_info(&mol);
            Ok(())
        }
        "mol2-info" => {
            let path = args.next().ok_or_else(|| "missing MOL2 path".to_string())?;
            let mol = read_mol2_file(path).map_err(|err| err.to_string())?;
            print_mol_info(&mol);
            Ok(())
        }
        "gro-info" => {
            let path = args.next().ok_or_else(|| "missing GRO path".to_string())?;
            let mol = read_gro_file(path).map_err(|err| err.to_string())?;
            print_mol_info(&mol);
            Ok(())
        }
        "psf-info" => {
            let path = args.next().ok_or_else(|| "missing PSF path".to_string())?;
            let mol = read_psf_file(path).map_err(|err| err.to_string())?;
            print_mol_info(&mol);
            Ok(())
        }
        "namdcoor-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing NAMD coor path".to_string())?;
            if let Some(psf_path) = args.next() {
                let load =
                    read_namd_coor_with_psf_files(path, psf_path).map_err(|err| err.to_string())?;
                print_mol_info(&load.molecule);
                println!("byte_swapped={}", load.frame.byte_swapped);
            } else {
                let frame = read_namd_coor_file(path).map_err(|err| err.to_string())?;
                println!("atoms={}", frame.atom_count());
                println!("byte_swapped={}", frame.byte_swapped);
                if let Some(position) = frame.positions.first() {
                    println!(
                        "first={:.6},{:.6},{:.6}",
                        position.x, position.y, position.z
                    );
                }
            }
            Ok(())
        }
        "ambercrd-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing AMBER restart path".to_string())?;
            let restart = read_amber_restart_file(path).map_err(|err| err.to_string())?;
            println!("atoms={}", restart.positions.len());
            println!("has_velocities={}", restart.has_velocities);
            println!("has_box={}", restart.cell.is_some());
            if let Some(position) = restart.positions.first() {
                println!(
                    "first={:.6},{:.6},{:.6}",
                    position.x, position.y, position.z
                );
            }
            Ok(())
        }
        "ambermdcrd-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing AMBER mdcrd path".to_string())?;
            let natom = args
                .next()
                .ok_or_else(|| "missing AMBER mdcrd atom count".to_string())?
                .parse::<usize>()
                .map_err(|_| "invalid AMBER mdcrd atom count".to_string())?;
            let has_box = args
                .next()
                .map(|value| matches!(value.as_str(), "box" | "has-box" | "true" | "1"))
                .unwrap_or(false);
            let traj =
                read_amber_mdcrd_file(path, natom, has_box).map_err(|err| err.to_string())?;
            println!("frames={}", traj.frame_count());
            println!("atoms={}", traj.atom_count());
            println!("has_box={}", traj.has_box);
            if let Some(frame) = traj.frames.first() {
                if let Some(position) = frame.positions.first() {
                    println!(
                        "first={:.6},{:.6},{:.6}",
                        position.x, position.y, position.z
                    );
                }
            }
            Ok(())
        }
        "dcd-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing DCD trajectory path".to_string())?;
            let skip_no = args
                .next()
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "invalid DCD frame skip".to_string())
                })
                .transpose()?
                .unwrap_or(1);
            let block = read_dcd_file_with_options(
                path,
                &DcdReadOptions {
                    skip_no,
                    ..DcdReadOptions::default()
                },
            )
            .map_err(|err| err.to_string())?;
            println!("frames={}", block.frame_count());
            println!("atoms={}", block.atom_count());
            println!(
                "has_cell={}",
                (0..block.frame_count()).any(|index| block.cell(index).is_some())
            );
            if let Some(frame) = block.frame(0) {
                if let Some(first) = frame.get(..3) {
                    println!("first={:.6},{:.6},{:.6}", first[0], first[1], first[2]);
                }
            }
            Ok(())
        }
        "trr-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing TRR trajectory path".to_string())?;
            let skip_no = args
                .next()
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "invalid TRR frame skip".to_string())
                })
                .transpose()?
                .unwrap_or(1);
            let block = read_trr_file_with_options(
                path,
                &TrrReadOptions {
                    skip_no,
                    ..TrrReadOptions::default()
                },
            )
            .map_err(|err| err.to_string())?;
            println!("frames={}", block.frame_count());
            println!("atoms={}", block.atom_count());
            println!(
                "has_cell={}",
                (0..block.frame_count()).any(|index| block.cell(index).is_some())
            );
            if let Some(frame) = block.frame(0) {
                if let Some(first) = frame.get(..3) {
                    println!("first={:.6},{:.6},{:.6}", first[0], first[1], first[2]);
                }
            }
            Ok(())
        }
        "xtc-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing XTC trajectory path".to_string())?;
            let skip_no = args
                .next()
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "invalid XTC frame skip".to_string())
                })
                .transpose()?
                .unwrap_or(1);
            let block = read_xtc_file_with_options(
                path,
                &XtcReadOptions {
                    skip_no,
                    ..XtcReadOptions::default()
                },
            )
            .map_err(|err| err.to_string())?;
            println!("frames={}", block.frame_count());
            println!("atoms={}", block.atom_count());
            println!(
                "has_cell={}",
                (0..block.frame_count()).any(|index| block.cell(index).is_some())
            );
            if let Some(frame) = block.frame(0) {
                if let Some(first) = frame.get(..3) {
                    println!("first={:.6},{:.6},{:.6}", first[0], first[1], first[2]);
                }
            }
            Ok(())
        }
        "ambernetcdf-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing AMBER NetCDF trajectory path".to_string())?;
            let skip_no = args
                .next()
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "invalid AMBER NetCDF frame skip".to_string())
                })
                .transpose()?
                .unwrap_or(1);
            let block = read_amber_netcdf_file_with_options(
                path,
                &AmberNetCDFReadOptions {
                    skip_no,
                    ..AmberNetCDFReadOptions::default()
                },
            )
            .map_err(|err| err.to_string())?;
            println!("frames={}", block.frame_count());
            println!("atoms={}", block.atom_count());
            println!(
                "has_cell={}",
                (0..block.frame_count()).any(|index| block.cell(index).is_some())
            );
            if let Some(frame) = block.frame(0) {
                if let Some(first) = frame.get(..3) {
                    println!("first={:.6},{:.6},{:.6}", first[0], first[1], first[2]);
                }
            }
            Ok(())
        }
        "amberprm-info" => {
            let prmtop_path = args
                .next()
                .ok_or_else(|| "missing AMBER prmtop path".to_string())?;
            let mol = if let Some(restart_path) = args.next() {
                read_amber_prmtop_with_restart_files(prmtop_path, restart_path)
                    .map_err(|err| err.to_string())?
                    .molecule
            } else {
                read_amber_prmtop_file(prmtop_path).map_err(|err| err.to_string())?
            };
            print_mol_info(&mol);
            Ok(())
        }
        "sdf-write" => {
            let path = args.next().ok_or_else(|| "missing SDF path".to_string())?;
            let mol = read_sdf_file(path).map_err(|err| err.to_string())?;
            print!("{}", write_sdf_string(&mol));
            Ok(())
        }
        "cnspar-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing CNS parameter file path".to_string())?;
            let db =
                cuemol_molstr::cns_par::read_cns_par_file(path).map_err(|err| err.to_string())?;
            println!("bonds={}", db.bond_count());
            println!("angles={}", db.angle_count());
            println!("dihedrals={}", db.dihedral_count());
            println!("impropers={}", db.improper_count());
            println!("nonbonded={}", db.nonbonded_count());
            if let Some(b) = db.search_bond("C", "N") {
                println!("bond C-N: kf={} r0={}", b.kf, b.r0);
            }
            Ok(())
        }
        "opendx-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing OpenDX path".to_string())?;
            let map = read_opendx_file(path).map_err(|err| err.to_string())?;
            println!(
                "counts={}x{}x{}",
                map.counts[0], map.counts[1], map.counts[2]
            );
            println!("values={}", map.value_count());
            println!("origin={},{},{}", map.origin.x, map.origin.y, map.origin.z);
            Ok(())
        }
        "ccp4map-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing CCP4/MRC map path".to_string())?;
            let map = read_ccp4_map_with_header_file(path).map_err(|err| err.to_string())?;
            println!(
                "counts={}x{}x{}",
                map.grid.counts[0], map.grid.counts[1], map.grid.counts[2]
            );
            println!("values={}", map.grid.value_count());
            println!(
                "origin={},{},{}",
                map.grid.origin.x, map.grid.origin.y, map.grid.origin.z
            );
            println!("mode={}", map.header.mode);
            println!("byte_swapped={}", map.header.byte_swapped);
            println!(
                "stats={:.6},{:.6},{:.6},{:.6}",
                map.header.stats.min,
                map.header.stats.max,
                map.header.stats.mean,
                map.header.stats.rms
            );
            Ok(())
        }
        "xplormap-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing Xplor/CNS map path".to_string())?;
            let map = read_xplor_map_with_header_file(path).map_err(|err| err.to_string())?;
            println!(
                "counts={}x{}x{}",
                map.grid.counts[0], map.grid.counts[1], map.grid.counts[2]
            );
            println!("values={}", map.grid.value_count());
            println!(
                "origin={},{},{}",
                map.grid.origin.x, map.grid.origin.y, map.grid.origin.z
            );
            println!("titles={}", map.header.title_count);
            Ok(())
        }
        "brixmap-info" => {
            let path = args
                .next()
                .ok_or_else(|| "missing BRIX/DSN6 map path".to_string())?;
            let map = read_brix_map_with_header_file(path).map_err(|err| err.to_string())?;
            println!(
                "counts={}x{}x{}",
                map.grid.counts[0], map.grid.counts[1], map.grid.counts[2]
            );
            println!("values={}", map.grid.value_count());
            println!(
                "origin={},{},{}",
                map.grid.origin.x, map.grid.origin.y, map.grid.origin.z
            );
            println!("dsn6={}", map.header.dsn6);
            Ok(())
        }
        "mtz-info" => {
            let path = args.next().ok_or_else(|| "missing MTZ path".to_string())?;
            let mtz = read_mtz_file(path).map_err(|err| err.to_string())?;
            println!("columns={}", mtz.column_count);
            println!("reflections={}", mtz.reflection_count);
            println!(
                "column_names={}",
                mtz.columns
                    .iter()
                    .map(|column| column.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            if let Some(cell) = mtz.cell {
                println!(
                    "cell={:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
                    cell[0], cell[1], cell[2], cell[3], cell[4], cell[5]
                );
            }
            if let Some(space_group) = mtz.space_group {
                println!("space_group={space_group}");
            }
            Ok(())
        }
        "msms-info" => {
            let face_path = args
                .next()
                .ok_or_else(|| "missing MSMS face path".to_string())?;
            let vert_path = args
                .next()
                .ok_or_else(|| "missing MSMS vert path".to_string())?;
            let mesh =
                read_msms_surface_files(face_path, vert_path).map_err(|err| err.to_string())?;
            println!("vertices={}", mesh.vertex_count());
            println!("triangles={}", mesh.triangle_count());
            Ok(())
        }
        "obj-info" => {
            let path = args.next().ok_or_else(|| "missing OBJ path".to_string())?;
            let mesh = read_obj_file(path).map_err(|err| err.to_string())?;
            println!("vertices={}", mesh.vertex_count());
            println!("triangles={}", mesh.triangle_count());
            Ok(())
        }
        "qsc-info" => {
            let path = args.next().ok_or_else(|| "missing QSC path".to_string())?;
            let mut manager = MolSceneManager::new();
            let status = manager
                .load_qsc_file_checked(path)
                .map_err(|err| err.to_string())?;
            let scene_uid = match status {
                QscLoadStatus::Loaded { scene_uid } => scene_uid,
                QscLoadStatus::GitLfsPointer { oid, size } => {
                    println!("git_lfs_pointer=true");
                    println!("oid={oid}");
                    println!("size={size}");
                    return Ok(());
                }
            };
            let scene = manager
                .scene(scene_uid)
                .ok_or_else(|| "loaded scene not found".to_string())?;
            let styles = scene_style_manager(scene);
            let scope = cuemol_molvis::DEFAULT_STYLE_SCOPE;
            println!("objects={}", scene.objects().len());
            for object in scene.objects() {
                println!("object={}", object.name());
                println!("atoms={}", object.mol().atom_count());
                for renderer in object.renderers() {
                    println!("renderer={}:{}", renderer.renderer_type(), renderer.name());
                    if let Some(style) = renderer.style() {
                        println!("style={style}");
                        // Geometry properties the style preset contributes
                        // (resolved through the inline-overrides-style cascade).
                        for attr in ["bondw", "sphr", "detail", "axialdetail", "width"] {
                            if let Some(value) = renderer.resolved_attr(attr, &styles, scope) {
                                println!("resolved.{attr}={value}");
                            }
                        }
                    }
                    if renderer.renderer_type() == "atomintr" {
                        println!("defs={}", renderer.get_defs_json());
                        println!(
                            "measurements={}",
                            object.atom_intr_measurements_json(
                                &scene.resolve_atom_intr_renderer(object, renderer)
                            )
                        );
                    }
                }
            }
            Ok(())
        }
        "qsc-write" => {
            let path = args.next().ok_or_else(|| "missing QSC path".to_string())?;
            let mut manager = MolSceneManager::new();
            let scene_uid = manager.load_qsc_file(path).map_err(|err| err.to_string())?;
            let scene = manager
                .scene(scene_uid)
                .ok_or_else(|| "loaded scene not found".to_string())?;
            std::io::Write::write_all(&mut std::io::stdout(), &scene.write_qsc_bytes())
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        "mol-array-info" => {
            let format = args
                .next()
                .ok_or_else(|| "missing molecule format".to_string())?;
            let path = args
                .next()
                .ok_or_else(|| "missing molecule path".to_string())?;
            let array_name = args
                .next()
                .ok_or_else(|| "missing array name".to_string())?;
            let mol = read_mol_by_format(&format, &path).map_err(|err| err.to_string())?;
            let array = match array_name.as_str() {
                "crd" => mol.crd_array(),
                "bfac" | "occ" => mol.prop_array(&array_name),
                other => return Err(format!("unknown molecule array: {other}")),
            };
            println!("type={}", array.elem_type_code());
            println!("length={}", array.elem_count());
            println!("bytes={}", array.byte_len());
            if matches!(array_name.as_str(), "crd" | "bfac" | "occ") {
                let values = array.copy_to_f32().map_err(|err| err.to_string())?;
                println!(
                    "first={}",
                    values
                        .iter()
                        .take(6)
                        .map(|value| format!("{value:.6}"))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
            Ok(())
        }
        "mol-sel-array" => {
            let format = args
                .next()
                .ok_or_else(|| "missing molecule format".to_string())?;
            let path = args
                .next()
                .ok_or_else(|| "missing molecule path".to_string())?;
            let expression = args.collect::<Vec<_>>().join(" ");
            if expression.is_empty() {
                return Err("missing selection expression".to_string());
            }
            let mol = read_mol_by_format(&format, &path).map_err(|err| err.to_string())?;
            let selector = parse_selector(&expression).map_err(|err| err.to_string())?;
            let array = mol.sel_array(&selector);
            let values = array.copy_to_i32().map_err(|err| err.to_string())?;
            println!("type={}", array.elem_type_code());
            println!("length={}", array.elem_count());
            println!(
                "values={}",
                values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            Ok(())
        }
        "mol-delete-info" => {
            let format = args
                .next()
                .ok_or_else(|| "missing molecule format".to_string())?;
            let path = args
                .next()
                .ok_or_else(|| "missing molecule path".to_string())?;
            let expression = args.collect::<Vec<_>>().join(" ");
            if expression.is_empty() {
                return Err("missing selection expression".to_string());
            }
            let mut mol = read_mol_by_format(&format, &path).map_err(|err| err.to_string())?;
            let selector = parse_selector(&expression).map_err(|err| err.to_string())?;
            let before_atoms = mol.atom_count();
            let before_bonds = mol.bond_count();
            let deleted = mol.delete_selected_atoms(&selector);
            println!("deleted={deleted}");
            println!("atoms={before_atoms}->{}", mol.atom_count());
            println!("bonds={before_bonds}->{}", mol.bond_count());
            Ok(())
        }
        "mol-copy-info" => {
            let format = args
                .next()
                .ok_or_else(|| "missing molecule format".to_string())?;
            let path = args
                .next()
                .ok_or_else(|| "missing molecule path".to_string())?;
            let expression = args.collect::<Vec<_>>().join(" ");
            if expression.is_empty() {
                return Err("missing selection expression".to_string());
            }
            let source = read_mol_by_format(&format, &path).map_err(|err| err.to_string())?;
            let selector = parse_selector(&expression).map_err(|err| err.to_string())?;
            let mut copied_mol = MolCoord::new();
            let copied = copied_mol
                .copy_selected_atoms_from(&source, &selector)
                .map_err(|err| err.to_string())?;
            println!("copied={copied}");
            println!("atoms={}", copied_mol.atom_count());
            println!("bonds={}", copied_mol.bond_count());
            Ok(())
        }
        "stream-info" => {
            let manager = StreamManager::new();
            match args.next().as_deref() {
                None | Some("all") => println!("{}", manager.get_info_json2()),
                Some("readers") => print!("{}", manager.get_reader_info_json()),
                Some("writers") => print!("{}", manager.get_writer_info_json()),
                Some(other) => return Err(format!("unknown stream-info target: {other}")),
            }
            Ok(())
        }
        "stream-search" => {
            let category = args
                .next()
                .ok_or_else(|| "missing stream handler category".to_string())
                .and_then(|value| parse_stream_category(&value))?;
            let path = args
                .next()
                .ok_or_else(|| "missing stream search path".to_string())?;
            let nicknames_csv = args.next().unwrap_or_default();
            let manager = StreamManager::new();
            let first = manager.search_reader_by_content(&path, &nicknames_csv, category, false, 0);
            let all = manager.search_readers_by_content(&path, &nicknames_csv, category, false, 0);
            println!("first={first}");
            println!("all={all}");
            Ok(())
        }
        "select-dump" => {
            let expression = args.collect::<Vec<_>>().join(" ");
            if expression.is_empty() {
                return Err("missing selection expression".to_string());
            }
            let selector = parse_selector(&expression).map_err(|err| err.to_string())?;
            println!("{}", selector.dump());
            Ok(())
        }
        _ if is_input_path(&command) => {
            let paths = std::iter::once(command)
                .chain(args)
                .map(std::path::PathBuf::from)
                .collect();
            launch_gui(paths)
        }
        _ => {
            print_usage();
            Err(format!("unknown command: {command}"))
        }
    }
}

fn is_input_path(value: &str) -> bool {
    let path = std::path::Path::new(value);
    !value.starts_with('-')
        && (path.exists() || path.components().count() > 1 || path.extension().is_some())
}

#[cfg(feature = "gui")]
fn launch_default() -> Result<(), String> {
    launch_gui(Vec::new())
}

#[cfg(not(feature = "gui"))]
fn launch_default() -> Result<(), String> {
    print_usage();
    Ok(())
}

#[cfg(feature = "gui")]
fn launch_gui(paths: Vec<std::path::PathBuf>) -> Result<(), String> {
    cuemol_desktop::gui::run_with_paths(paths).map_err(|error| error.to_string())
}

#[cfg(not(feature = "gui"))]
fn launch_gui(_: Vec<std::path::PathBuf>) -> Result<(), String> {
    Err("GUI support is not enabled in this build".to_string())
}

#[cfg(unix)]
fn self_upgrade() -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = executable
        .parent()
        .ok_or("cannot determine installation directory")?;
    let mut command = Command::new("sh");
    if directory.file_name().is_some_and(|name| name == "MacOS") {
        if let Some(contents) = directory.parent() {
            if contents.join("Resources/cuemol-tools").is_dir() {
                command.env(
                    "CUEMOL_TOOLS_INSTALL_DIR",
                    contents.join("Resources/cuemol-tools"),
                );
            }
        }
    }
    command
        .arg("-s")
        .env("CUEMOL_INSTALL_DIR", directory)
        .env(
            "CUEMOL_INSTALL_NAME",
            executable.file_name().ok_or("missing executable name")?,
        )
        .stdin(Stdio::piped());
    if std::env::var_os("CUEMOL_RELEASE_REPOSITORY").is_none() {
        if let Some(repository) = option_env!("CUEMOL_RELEASE_REPOSITORY") {
            command.env("CUEMOL_RELEASE_REPOSITORY", repository);
        }
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start installer: {error}"))?;
    let mut input = child.stdin.take().ok_or("installer stdin is unavailable")?;
    input
        .write_all(include_bytes!("../../../install.sh"))
        .map_err(|error| format!("failed to send installer: {error}"))?;
    drop(input);
    let status = child.wait().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("self-upgrade failed: {status}"))
    }
}

#[cfg(windows)]
fn self_upgrade() -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", "-"])
        .env(
            "CUEMOL_INSTALL_DIR",
            executable
                .parent()
                .ok_or("missing installation directory")?,
        )
        .stdin(Stdio::piped());
    if std::env::var_os("CUEMOL_RELEASE_REPOSITORY").is_none() {
        if let Some(repository) = option_env!("CUEMOL_RELEASE_REPOSITORY") {
            command.env("CUEMOL_RELEASE_REPOSITORY", repository);
        }
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("installer stdin unavailable")?
        .write_all(include_bytes!("../../../install.ps1"))
        .map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("self-upgrade failed: {status}"))
    }
}

fn print_usage() {
    println!("  cuemol web build | serve [--port PORT] [--dir DIR]");
    println!("CueMol3: cuemol [FILE ...] | mcp | config | run SCRIPT.js [ARGS...] | -c JAVASCRIPT [ARGS...] | exec TOOL JSON | render INPUT OUTPUT | trajectory-info FILE [STRIDE]");
    eprintln!("Usage:");
    eprintln!("  cuemol self-upgrade [--check]");
    eprintln!("  cuemol tools list | path NAME | run NAME [ARG ...] | bundle [--out DIR]");
    eprintln!("  cuemol [file ...]");
    eprintln!("  cuemol gui [file ...]");
    eprintln!("  cuemol pdb-info <file.pdb>");
    eprintln!("  cuemol qdfpdb-info <file.qdfpdb>");
    eprintln!("  cuemol pdb-write <file.pdb>");
    eprintln!("  cuemol mmcif-info <file.cif>");
    eprintln!("  cuemol sdf-info <file.sdf>");
    eprintln!("  cuemol mol2-info <file.mol2>");
    eprintln!("  cuemol gro-info <file.gro>");
    eprintln!("  cuemol psf-info <file.psf>");
    eprintln!("  cuemol namdcoor-info <file.coor> [file.psf]");
    eprintln!("  cuemol ambercrd-info <file.inpcrd|file.rst7>");
    eprintln!("  cuemol ambermdcrd-info <file.mdcrd|file.crd> <natom> [box]");
    eprintln!("  cuemol dcd-info <file.dcd> [nevery]");
    eprintln!("  cuemol trr-info <file.trr> [nevery]");
    eprintln!("  cuemol xtc-info <file.xtc> [nevery]");
    eprintln!("  cuemol ambernetcdf-info <file.nc> [nevery]");
    eprintln!("  cuemol amberprm-info <file.prmtop|file.parm7> [file.inpcrd]");
    eprintln!("  cuemol sdf-write <file.sdf>");
    eprintln!("  cuemol opendx-info <file.dx>");
    eprintln!("  cuemol ccp4map-info <file.map|file.ccp4|file.mrc>");
    eprintln!("  cuemol xplormap-info <file.map|file.cns>");
    eprintln!("  cuemol brixmap-info <file.brix|file.omap>");
    eprintln!("  cuemol mtz-info <file.mtz>");
    eprintln!("  cuemol msms-info <file.face> <file.vert>");
    eprintln!("  cuemol obj-info <file.obj>");
    eprintln!("  cuemol qsc-info <file.qsc>");
    eprintln!("  cuemol mol-array-info <pdb|qdfpdb|cif|mmcif|sdf|mol|mol2|gro|psf|amberprm> <file> <crd|bfac|occ>");
    eprintln!(
        "  cuemol mol-sel-array <pdb|qdfpdb|cif|mmcif|sdf|mol|mol2|gro|psf|amberprm> <file> <selection-expression>"
    );
    eprintln!(
        "  cuemol mol-delete-info <pdb|qdfpdb|cif|mmcif|sdf|mol|mol2|gro|psf|amberprm> <file> <selection-expression>"
    );
    eprintln!(
        "  cuemol mol-copy-info <pdb|qdfpdb|cif|mmcif|sdf|mol|mol2|gro|psf|amberprm> <file> <selection-expression>"
    );
    eprintln!("  cuemol stream-info [all|readers|writers]");
    eprintln!("  cuemol stream-search <objreader|objwriter|scereader|scewriter|0-4> <file> [nicknames_csv]");
    eprintln!("  cuemol select-dump <selection-expression>");
    eprintln!(
        "  cuemol export-scene <file.pdb|.cif|.sdf|.mol2> <out.stl|.obj|.pov|.png(Umbreon)|.mqo|.lxs|.cfg(LuxCore)|.wbp(Warabi)>"
    );
    eprintln!(
        "  cuemol render-qsc <file.qsc> <out.stl|.obj|.pov|.png|.mqo|.lxs|.cfg|.wbp>   (renders the scene with resolved styles)"
    );
}

fn parse_stream_category(value: &str) -> Result<HandlerCategory, String> {
    match value {
        "objreader" | "obj-reader" => Ok(HandlerCategory::ObjReader),
        "objwriter" | "obj-writer" => Ok(HandlerCategory::ObjWriter),
        "rendtofile" | "rend-to-file" => Ok(HandlerCategory::RendToFile),
        "scereader" | "scene-reader" => Ok(HandlerCategory::SceneReader),
        "scewriter" | "scene-writer" => Ok(HandlerCategory::SceneWriter),
        _ => value
            .parse::<i32>()
            .map_err(|_| format!("unknown stream handler category: {value}"))
            .and_then(|code| HandlerCategory::from_code(code).map_err(|err| err.to_string())),
    }
}

fn read_mol_by_format(format: &str, path: &str) -> cuemol_core::Result<MolCoord> {
    match format {
        "pdb" => read_pdb_file(path),
        "qdfpdb" => read_qdfpdb_file(path),
        "mmcif" | "cif" => read_mmcif_file(path),
        "sdf" | "mol" => read_sdf_file(path),
        "mol2" => read_mol2_file(path),
        "gro" => read_gro_file(path),
        "psf" => read_psf_file(path),
        "amberprm" | "prmtop" | "parm7" | "top" => read_amber_prmtop_file(path),
        other => Err(cuemol_core::CueMolError::invalid_argument(format!(
            "unknown format {other}"
        ))),
    }
}

fn print_mol_info(mol: &cuemol_molstr::MolCoord) {
    println!("atoms={}", mol.atom_count());
    println!("bonds={}", mol.bond_count());
    println!("residues={}", mol.residue_count());
    println!("chains={}", mol.chain_ids().join(","));
}
