use cuemol_core::{Matrix4, Vector4};
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
    HandlerCategory, LoadedDensityMap, MolCoord, MolSceneManager, QscLoadStatus, StreamManager,
};
use cuemol_molvis::{
    anisou_mesh_snapshot_with_options, apply_object_coloring, apply_renderer_styles,
    apply_tube_renderer_styles, atom_intr_render_options, disorder_lines_snapshot,
    molecule_render_snapshot, na_renderer_snapshot, ribbon_mesh_snapshot, scene_style_manager,
    AnIsoURenderOptions, MoleculeRenderOptions, NARenderOptions, NARenderer, DEFAULT_STYLE_SCOPE,
};
use cuemol_qsys::{Camera, MultiGradient, ScalarObject};
use cuemol_render::{
    tessellate_snapshot, tessellate_snapshot_with_detail, LuxCoreSceneExporter,
    LuxRendSceneExporter, MqoSceneExporter, PovSceneExporter, RenderMesh, RenderSceneSnapshot,
    StlSceneExporter, UmbreonSceneExporter, WbpSceneExporter,
};
use cuemol_surface::{read_msms_surface_files, read_obj_file};
use cuemol_xtal::{
    density_map_from_cif, read_mtz_file, read_qdf_denmap_bytes, BrixMapReader, Ccp4MapReader,
    DensityMap, MapColorMode, MapMeshRenderer, MapSurfRenderer, MapSurfaceDrawMode, Mtz2MapReader,
    XplorMapReader,
};
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
        print_usage();
        return Ok(());
    };

    match command.as_str() {
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
            export_scene(&input, &output)
        }
        "render-qsc" => {
            let input = args
                .next()
                .ok_or_else(|| "missing input .qsc path".to_string())?;
            let output = args
                .next()
                .ok_or_else(|| "missing output path (.stl/.obj/.png)".to_string())?;
            render_qsc_scene(&input, &output)
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
                            object.atom_intr_measurements_json(renderer)
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
            print!("{}", scene.write_qsc_string());
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
        _ => {
            print_usage();
            Err(format!("unknown command: {command}"))
        }
    }
}

fn print_usage() {
    eprintln!("Usage:");
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
        "  cuemol render-qsc <file.qsc> <out.stl|.obj|.png>   (renders the scene with resolved styles)"
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

/// Bounding-box centre and diagonal size of a render snapshot, for framing a
/// scene exporter's camera. Falls back to origin / size 10 when bounds are absent.
fn scene_center_size(snapshot: &RenderSceneSnapshot) -> (cuemol_core::Vector4, f64) {
    match snapshot.bounds {
        Some((min, max)) => {
            let center = cuemol_core::Vector4::new3(
                (min.x + max.x) / 2.0,
                (min.y + max.y) / 2.0,
                (min.z + max.z) / 2.0,
            );
            let size =
                ((max.x - min.x).powi(2) + (max.y - min.y).powi(2) + (max.z - min.z).powi(2))
                    .sqrt()
                    .max(1.0);
            (center, size)
        }
        None => (cuemol_core::Vector4::new3(0.0, 0.0, 0.0), 10.0),
    }
}

fn camera_for_snapshot(snapshot: &RenderSceneSnapshot) -> Camera {
    let (center, size) = scene_center_size(snapshot);
    let mut camera = Camera::new();
    camera.set_center(center);
    camera.set_zoom(size * 1.2);
    camera.set_cam_dist(size * 2.5);
    camera.set_slab_depth(size * 3.0);
    camera.set_perspective(true);
    camera
}

/// Infer a molecule reader format from a path's file extension.
fn format_from_ext(path: &str) -> Result<&'static str, String> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".pdb") || lower.ends_with(".ent") {
        Ok("pdb")
    } else if lower.ends_with(".mmcif") || lower.ends_with(".cif") {
        Ok("mmcif")
    } else if lower.ends_with(".sdf") || lower.ends_with(".mol") {
        Ok("sdf")
    } else if lower.ends_with(".mol2") {
        Ok("mol2")
    } else {
        Err(format!(
            "cannot infer molecule format from extension: {path}"
        ))
    }
}

/// Load a molecule, build its default ball-and-stick render snapshot, tessellate
/// it to a triangle mesh, and write it as STL / OBJ / POV-Ray geometry. The
/// headless end-to-end exerciser for the scene-export pipeline.
fn export_scene(input: &str, output: &str) -> Result<(), String> {
    let format = format_from_ext(input)?;
    let mol = read_mol_by_format(format, input).map_err(|err| err.to_string())?;
    let snapshot = molecule_render_snapshot(&mol, &MoleculeRenderOptions::default());
    let mesh = tessellate_snapshot(&snapshot);
    let name = std::path::Path::new(input)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("cuemol");
    let lower = output.to_ascii_lowercase();
    // LuxCore (.cfg + .scn + .ply) and Warabi (.wbp + .mqo) are multi-file
    // scenes, handled separately from the single-file formats below.
    if lower.ends_with(".cfg") {
        return export_luxcore(name, output, &mesh, &snapshot);
    }
    if lower.ends_with(".wbp") {
        return export_wbp(name, output, &mesh, &snapshot);
    }
    if lower.ends_with(".png") {
        let mut exporter = UmbreonSceneExporter::new();
        exporter.handler_mut().set_path(output);
        exporter.set_width(640);
        exporter.set_height(480);
        let camera = camera_for_snapshot(&snapshot);
        exporter
            .write_snapshot(&snapshot, &camera, [0.0, 0.0, 0.0, 1.0])
            .map_err(|error| error.to_string())?;
        eprintln!(
            "ray-traced {} atoms / {} bonds to {output}",
            snapshot.atom_count(),
            snapshot.bond_count()
        );
        return Ok(());
    }
    let bytes: Vec<u8> = if lower.ends_with(".stl") {
        StlSceneExporter::new()
            .write_ascii_stl_string(name, &mesh.positions(), &mesh.triangles())
            .into_bytes()
    } else if lower.ends_with(".obj") {
        mesh.to_obj_string(name).into_bytes()
    } else if lower.ends_with(".pov") {
        let (center, size) = scene_center_size(&snapshot);
        PovSceneExporter::new()
            .write_scene_string(&mesh, center, size, false)
            .into_bytes()
    } else if lower.ends_with(".mqo") {
        MqoSceneExporter::new()
            .write_mqo_string(name, &mesh)
            .into_bytes()
    } else if lower.ends_with(".lxs") {
        let (center, size) = scene_center_size(&snapshot);
        LuxRendSceneExporter::new()
            .write_scene_string(&mesh, center, size, false)
            .into_bytes()
    } else {
        return Err(format!(
            "unsupported output format (use .stl/.obj/.pov/.png/.mqo/.lxs): {output}"
        ));
    };
    std::fs::write(output, &bytes).map_err(|err| err.to_string())?;
    eprintln!(
        "exported {} atoms / {} bonds -> {} vertices / {} triangles to {output}",
        snapshot.atom_count(),
        snapshot.bond_count(),
        mesh.vertices.len(),
        mesh.triangle_count()
    );
    Ok(())
}

/// Render a loaded `.qsc` SCENE (objects + renderers, each with its `style="..."`
/// presets resolved against the bundled default styles) into a single mesh and
/// write it as STL/OBJ. Unlike `export-scene` (one molecule, default look), this
/// honours every renderer's type AND its resolved style geometry (bondw/sphr/
/// detail/axialdetail/ribbon cross-sections), so the exported mesh reflects the
/// scene's actual styling — the production consumer of the style-resolution chain.
fn render_qsc_scene(input: &str, output: &str) -> Result<(), String> {
    let mut manager = MolSceneManager::new();
    let scene_uid = manager
        .load_qsc_file(input)
        .map_err(|err| err.to_string())?;
    let scene = manager
        .scene(scene_uid)
        .ok_or_else(|| "loaded scene not found".to_string())?;

    let styles = scene_style_manager(scene);
    let mut combined = RenderMesh::new();
    let (mut total_atoms, mut total_bonds, mut renderer_count) = (0usize, 0usize, 0usize);

    for object in scene.objects() {
        // Object-level visibility gate (C++ `Scene.cpp:635`): a hidden object
        // contributes no geometry, regardless of its renderers' own visibility.
        if !object.is_visible() {
            continue;
        }
        for renderer in object.renderers() {
            if !renderer.is_visible() {
                continue;
            }
            let rtype = renderer.renderer_type();
            // Skip internal/overlay renderers (e.g. "*selection") which carry no
            // standalone geometry — drawing them would spuriously dump all atoms.
            if rtype.starts_with('*') {
                continue;
            }
            let mut options = base_render_options(rtype);
            apply_renderer_styles(&mut options, renderer, &styles, DEFAULT_STYLE_SCOPE);
            if matches!(rtype, "tube" | "nucl") {
                apply_tube_renderer_styles(&mut options, renderer, &styles, DEFAULT_STYLE_SCOPE);
            }
            // Per-renderer `sel="..."` draw restriction (shared behaviour with the GUI).
            if let Some(sel) = renderer
                .qsc_attr("sel")
                .map(str::trim)
                .filter(|sel| !sel.is_empty() && *sel != "*")
            {
                if let Ok(selector) = parse_selector(sel) {
                    options.draw_indices = Some(object.mol().select_indices(&selector));
                }
            }
            let renderer_coloring_type =
                renderer.resolved_child_type("coloring", &styles, DEFAULT_STYLE_SCOPE);
            if matches!(
                renderer_coloring_type.as_deref(),
                None | Some("CPKColoring")
            ) {
                apply_object_coloring(
                    &mut options,
                    object.coloring_type(),
                    |name| object.coloring_attr(name),
                    object.paint_coloring(),
                    object.mol().default_color(),
                );
            }
            let mut snapshot = if rtype == "atomintr" {
                // Measurement lines between the renderer's atom pairs, not the
                // whole molecule (the single source of truth shared with the GUI).
                cuemol_render::atom_intr_snapshot_with_options(
                    &object.atom_intr_segments(renderer),
                    None,
                    &atom_intr_render_options(renderer, &styles, DEFAULT_STYLE_SCOPE),
                )
            } else if rtype == "anisou" {
                let resolved =
                    |name: &str| renderer.resolved_attr(name, &styles, DEFAULT_STYLE_SCOPE);
                anisou_mesh_snapshot_with_options(
                    object.mol(),
                    &options,
                    &AnIsoURenderOptions {
                        draw_discs: resolved("drawdisc")
                            .as_deref()
                            .map(qsc_bool)
                            .unwrap_or(true),
                        disc_scale: resolved("discscale")
                            .and_then(|value| value.trim().parse().ok())
                            .unwrap_or(1.1),
                        disc_thickness: resolved("discthick")
                            .and_then(|value| value.trim().parse().ok())
                            .unwrap_or(0.1),
                        max_vertices: resolved("maxverts")
                            .and_then(|value| value.trim().parse().ok())
                            .unwrap_or(1_000_000),
                    },
                )
            } else if rtype == "disorder" {
                disorder_lines_snapshot(object.mol(), 1.0, 0.15, 2.0, -1.0)
            } else if rtype == "nucl" {
                let resolved =
                    |name: &str| renderer.resolved_attr(name, &styles, DEFAULT_STYLE_SCOPE);
                let show_tube = resolved("show_tube")
                    .as_deref()
                    .map(qsc_bool)
                    .unwrap_or(true);
                let show_basepair = resolved("show_basepair")
                    .as_deref()
                    .map(qsc_bool)
                    .unwrap_or(true);
                let base_size = resolved("base_size")
                    .and_then(|value| value.trim().parse::<f64>().ok())
                    .unwrap_or(0.5);
                let base_thick = resolved("base_thick")
                    .and_then(|value| value.trim().parse::<f64>().ok())
                    .unwrap_or(0.5);
                let base_detail = resolved("base_detail")
                    .and_then(|value| value.trim().parse::<i32>().ok())
                    .unwrap_or(3);
                let base_type = resolved("base_type")
                    .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                        "basepair" => NARenderer::NAREND_BP,
                        "simple1" => NARenderer::NAREND_SIMPLE1,
                        "detail1" => NARenderer::NAREND_DETAIL1,
                        "detail2" => NARenderer::NAREND_DETAIL2,
                        _ => value.trim().parse::<i32>().unwrap_or(NARenderer::NAREND_BP),
                    })
                    .unwrap_or(NARenderer::NAREND_BP);
                let mut snapshot = if show_tube {
                    ribbon_mesh_snapshot(object.mol(), &options)
                } else {
                    RenderSceneSnapshot::empty()
                };
                let bases = na_renderer_snapshot(
                    object.mol(),
                    &NARenderOptions {
                        base_size,
                        base_thick,
                        base_detail,
                        base_type,
                        show_basepair,
                    },
                );
                snapshot.atoms.extend(bases.atoms);
                snapshot.bonds.extend(bases.bonds);
                snapshot.meshes.extend(bases.meshes);
                snapshot
            } else if matches!(rtype, "dsurface" | "directsurf" | "dsurf2") {
                direct_surface_scene_snapshot(object, renderer, &options)
            } else if is_mainchain_trace_type(rtype) {
                ribbon_mesh_snapshot(object.mol(), &options)
            } else {
                molecule_render_snapshot(object.mol(), &options)
            };
            // Bake the object + renderer `xformMat` (C++ display-time transform)
            // into this renderer's geometry — effective transform `renderer · object`
            // — so render-qsc matches the GUI and the scene's saved placement.
            let xform = renderer.xform_matrix().mul_mat(object.xform_matrix());
            if xform != Matrix4::IDENTITY {
                snapshot.apply_xform(&xform);
            }
            total_atoms += snapshot.atom_count();
            total_bonds += snapshot.bond_count();
            let mesh = match options.sphere_detail {
                Some(detail) => tessellate_snapshot_with_detail(&snapshot, detail),
                None => tessellate_snapshot(&snapshot),
            };
            merge_mesh(&mut combined, &mesh);
            renderer_count += 1;
        }
    }

    for loaded in scene.loaded_density_maps() {
        let map = read_loaded_density_map(loaded)?;
        let Some(object_node) = loaded.qsc_node.as_ref() else {
            continue;
        };
        for renderer_node in object_node
            .children()
            .iter()
            .filter(|node| !node.is_attr() && node.tag_name() == "renderer")
        {
            let renderer_type = if renderer_node.type_name().is_empty() {
                renderer_node.str_attr("type").to_string()
            } else {
                renderer_node.type_name().to_string()
            };
            if !matches!(
                renderer_type.as_str(),
                "contour" | "gpu_mapmesh" | "isosurf"
            ) {
                continue;
            }
            if qsc_node_attr(renderer_node, "visible", &styles)
                .as_deref()
                .is_some_and(|value| !qsc_bool(value))
            {
                continue;
            }
            let number = |name: &str, default: f64| {
                qsc_node_attr(renderer_node, name, &styles)
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(default)
            };
            let bndry_molname =
                qsc_node_attr(renderer_node, "bndry_molname", &styles).unwrap_or_default();
            let bndry_sel = qsc_node_attr(renderer_node, "bndry_sel", &styles)
                .filter(|value| !value.trim().is_empty())
                .and_then(|value| parse_selector(&value).ok())
                .unwrap_or(cuemol_molstr::Selector::All);
            let boundary_points = if bndry_molname.is_empty() {
                None
            } else {
                scene
                    .objects()
                    .iter()
                    .find(|object| object.name() == bndry_molname)
                    .map(|object| {
                        object
                            .mol()
                            .select_indices(&bndry_sel)
                            .into_iter()
                            .map(|index| object.mol().atoms()[index].position)
                            .collect::<Vec<_>>()
                    })
            };
            let color_mode = match qsc_node_attr(renderer_node, "colormode", &styles)
                .as_deref()
                .unwrap_or("solid")
            {
                "multigrad" | "4" => MapColorMode::MultiGradient,
                "molecule" | "3" => MapColorMode::Molecule,
                _ => MapColorMode::Solid,
            };
            let color_mapname =
                qsc_node_attr(renderer_node, "color_mapname", &styles).unwrap_or_default();
            let multi_grad = qsc_multi_gradient(renderer_node, &styles);
            let base_rgba = qsc_renderer_rgba(renderer_node, &styles);
            let drawmode = match qsc_node_attr(renderer_node, "drawmode", &styles)
                .as_deref()
                .unwrap_or("fill")
            {
                "line" | "1" => MapSurfaceDrawMode::Line,
                "point" | "2" => MapSurfaceDrawMode::Point,
                _ => MapSurfaceDrawMode::Fill,
            };
            let color_map =
                if color_mode == MapColorMode::MultiGradient && !color_mapname.is_empty() {
                    if color_mapname == loaded.name {
                        Some(map.clone())
                    } else {
                        scene
                            .loaded_density_maps()
                            .iter()
                            .find(|candidate| candidate.name == color_mapname)
                            .map(read_loaded_density_map)
                            .transpose()?
                    }
                } else {
                    None
                };
            if matches!(renderer_type.as_str(), "contour" | "gpu_mapmesh") {
                let mut renderer = MapMeshRenderer::new();
                renderer.map_mut().set_siglevel(number("siglevel", 1.1));
                renderer.map_mut().set_extent(number("extent", 15.0));
                renderer.map_mut().set_use_pbc(
                    qsc_node_attr(renderer_node, "use_pbc", &styles)
                        .as_deref()
                        .is_none_or(qsc_bool),
                );
                renderer.map_mut().set_bndry_rng(number("bndry_rng", 5.0));
                renderer.map_mut().set_bndry_molname(bndry_molname.clone());
                renderer.map_mut().set_bndry_sel(Some(bndry_sel.clone()));
                renderer.map_mut().set_color_mode(color_mode);
                renderer.map_mut().set_color_mapname(color_mapname.clone());
                renderer.map_mut().multi_grad_mut().copy_from(&multi_grad);
                renderer.set_width(number("width", 1.0));
                if let Some(center) = qsc_node_attr(renderer_node, "center", &styles)
                    .as_deref()
                    .and_then(parse_qsc_vector3)
                {
                    renderer.map_mut().set_center(center);
                }
                let mut snapshot = RenderSceneSnapshot::empty();
                for [from, to] in
                    renderer.generate_contour_lines_with_boundary(&map, boundary_points.as_deref())
                {
                    snapshot.bonds.push(cuemol_render::RenderBond {
                        from,
                        to,
                        radius: (0.04 * renderer.width().max(0.1)) as f32,
                        color: Some(cuemol_gfx::ColorSpec {
                            base: cuemol_gfx::ColorBase::Html {
                                r: base_rgba.r,
                                g: base_rgba.g,
                                b: base_rgba.b,
                                a: base_rgba.a,
                            },
                            modifiers: Vec::new(),
                        }),
                        end_color: None,
                        object_uid: None,
                        atom_indices: None,
                        selected: false,
                        screen_width: None,
                        screen_stipple: false,
                        screen_offset: [0.0, 0.0],
                        screen_point: false,
                    });
                }
                total_bonds += snapshot.bond_count();
                merge_mesh(&mut combined, &tessellate_snapshot(&snapshot));
            } else {
                let mut renderer = MapSurfRenderer::new();
                renderer.map_mut().set_siglevel(number("siglevel", 1.1));
                renderer.map_mut().set_extent(number("extent", 15.0));
                renderer.map_mut().set_use_pbc(
                    qsc_node_attr(renderer_node, "use_pbc", &styles)
                        .as_deref()
                        .is_none_or(qsc_bool),
                );
                renderer.map_mut().set_bndry_rng(number("bndry_rng", 5.0));
                renderer.map_mut().set_bndry_molname(bndry_molname.clone());
                renderer.map_mut().set_bndry_sel(Some(bndry_sel.clone()));
                renderer.map_mut().set_color_mode(color_mode);
                renderer.map_mut().set_color_mapname(color_mapname.clone());
                renderer.map_mut().multi_grad_mut().copy_from(&multi_grad);
                renderer.set_binning(number("binning", 1.0).round() as i32);
                renderer.set_max_grids(number("max_grids", 100.0).round() as i32);
                renderer.set_drawmode(drawmode);
                if let Some(center) = qsc_node_attr(renderer_node, "center", &styles)
                    .as_deref()
                    .and_then(parse_qsc_vector3)
                {
                    renderer.map_mut().set_center(center);
                }
                let surface =
                    renderer.generate_surf_obj_with_boundary(&map, boundary_points.as_deref());
                let mesh = RenderMesh {
                    vertices: surface
                        .mesh()
                        .vertices()
                        .iter()
                        .map(|vertex| cuemol_render::MeshVertex {
                            position: vertex.position,
                            normal: vertex.normal,
                            color: color_map
                                .as_ref()
                                .map(|color_map| {
                                    let rgba =
                                        multi_grad.color(color_map.value_at(vertex.position));
                                    [rgba.r, rgba.g, rgba.b, rgba.a]
                                        .map(|component| f32::from(component) / 255.0)
                                })
                                .unwrap_or_else(|| {
                                    [base_rgba.r, base_rgba.g, base_rgba.b, base_rgba.a]
                                        .map(|component| f32::from(component) / 255.0)
                                }),
                        })
                        .collect(),
                    indices: surface
                        .mesh()
                        .triangles()
                        .iter()
                        .flat_map(|triangle| triangle.iter().copied())
                        .collect(),
                };
                let color_spec = |color: [f32; 4]| {
                    Some(cuemol_gfx::ColorSpec {
                        base: cuemol_gfx::ColorBase::Html {
                            r: (color[0] * 255.0).round() as u8,
                            g: (color[1] * 255.0).round() as u8,
                            b: (color[2] * 255.0).round() as u8,
                            a: (color[3] * 255.0).round() as u8,
                        },
                        modifiers: Vec::new(),
                    })
                };
                match drawmode {
                    MapSurfaceDrawMode::Fill => merge_mesh(&mut combined, &mesh),
                    MapSurfaceDrawMode::Line => {
                        let mut snapshot = RenderSceneSnapshot::empty();
                        let mut edges = std::collections::BTreeSet::new();
                        for triangle in mesh.indices.chunks_exact(3) {
                            for (lhs, rhs) in [
                                (triangle[0], triangle[1]),
                                (triangle[1], triangle[2]),
                                (triangle[2], triangle[0]),
                            ] {
                                edges.insert((lhs.min(rhs), lhs.max(rhs)));
                            }
                        }
                        for (lhs, rhs) in edges {
                            let lhs = &mesh.vertices[lhs as usize];
                            let rhs = &mesh.vertices[rhs as usize];
                            snapshot.bonds.push(cuemol_render::RenderBond {
                                from: lhs.position,
                                to: rhs.position,
                                radius: (0.04 * renderer.width().max(0.1)) as f32,
                                color: color_spec(lhs.color),
                                end_color: None,
                                object_uid: None,
                                atom_indices: None,
                                selected: false,
                                screen_width: None,
                                screen_stipple: false,
                                screen_offset: [0.0, 0.0],
                                screen_point: false,
                            });
                        }
                        total_bonds += snapshot.bond_count();
                        merge_mesh(&mut combined, &tessellate_snapshot(&snapshot));
                    }
                    MapSurfaceDrawMode::Point => {
                        let mut snapshot = RenderSceneSnapshot::empty();
                        for vertex in &mesh.vertices {
                            snapshot.atoms.push(cuemol_render::RenderAtom {
                                center: vertex.position,
                                radius: (0.06 * renderer.width().max(0.1)) as f32,
                                color: color_spec(vertex.color),
                                object_uid: None,
                                atom_index: None,
                                selected: false,
                                detail: Some(1),
                            });
                        }
                        total_atoms += snapshot.atom_count();
                        merge_mesh(&mut combined, &tessellate_snapshot(&snapshot));
                    }
                }
            }
            renderer_count += 1;
        }
    }

    if renderer_count == 0 {
        return Err("scene has no visible renderers to render".to_string());
    }

    let name = std::path::Path::new(input)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("cuemol");
    let lower = output.to_ascii_lowercase();
    if lower.ends_with(".png") {
        let mut snapshot = RenderSceneSnapshot {
            atoms: Vec::new(),
            bonds: Vec::new(),
            meshes: vec![combined.clone()],
            bounds: None,
        };
        snapshot.recompute_bounds();
        let mut exporter = UmbreonSceneExporter::new();
        exporter.handler_mut().set_path(output);
        exporter.set_width(640);
        exporter.set_height(480);
        exporter
            .write_snapshot(
                &snapshot,
                &camera_for_snapshot(&snapshot),
                [0.0, 0.0, 0.0, 1.0],
            )
            .map_err(|error| error.to_string())?;
        eprintln!(
            "ray-traced {renderer_count} renderers ({total_atoms} atoms / {total_bonds} bonds) to {output}"
        );
        return Ok(());
    }
    let bytes: Vec<u8> = if lower.ends_with(".stl") {
        StlSceneExporter::new()
            .write_ascii_stl_string(name, &combined.positions(), &combined.triangles())
            .into_bytes()
    } else if lower.ends_with(".obj") {
        combined.to_obj_string(name).into_bytes()
    } else {
        return Err(format!(
            "unsupported output format (use .stl/.obj/.png): {output}"
        ));
    };
    std::fs::write(output, &bytes).map_err(|err| err.to_string())?;
    eprintln!(
        "rendered {renderer_count} renderers ({total_atoms} atoms / {total_bonds} bonds) -> {} vertices / {} triangles to {output}",
        combined.vertices.len(),
        combined.triangle_count()
    );
    Ok(())
}

/// Renderer-type base render options (the hardcoded per-type look, mirroring the
/// desktop's `renderer_render_options`) onto which style resolution then layers.
fn base_render_options(renderer_type: &str) -> MoleculeRenderOptions {
    let mut options = MoleculeRenderOptions::default();
    match renderer_type {
        "cpk" => {
            options.element_radii = Some(cuemol_molvis::VdwRadii::default());
            options.sphere_detail = Some(3);
            options.bond_radius = 0.0;
            options.draw_bonds = false;
        }
        "ballstick" => {
            options.vdw_scale = 0.16;
            options.bond_radius = 0.08;
            options.sphere_detail = Some(3);
        }
        "trace" | "ribbon" | "cartoon" | "tube" | "spline" | "nucl" => {
            options.vdw_scale = 0.12;
            options.bond_radius = 0.12;
        }
        "molsurf" | "dsurface" | "directsurf" | "dsurf2" => {
            options.vdw_scale = 0.32;
            options.bond_radius = 0.04;
        }
        _ => {}
    }
    // `trace` is the angular Cα line strip (C++ TraceRenderer): straight between
    // pivots, no spline. tube/spline/ribbon/cartoon/nucl keep the smooth spline.
    options.linear_backbone = renderer_type == "trace";
    options
}

fn direct_surface_scene_snapshot(
    object: &cuemol_molstr::MolObject,
    renderer: &cuemol_molstr::MolRenderer,
    options: &MoleculeRenderOptions,
) -> RenderSceneSnapshot {
    let selector = renderer
        .qsc_attr("sel")
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "*")
        .and_then(|value| parse_selector(value).ok())
        .unwrap_or(cuemol_molstr::Selector::All);
    let mut surface_renderer = cuemol_surface::DirectSurfRenderer::new();
    surface_renderer.set_selection(selector.clone());
    if let Some(value) = renderer
        .qsc_attr("detail")
        .and_then(|value| value.parse().ok())
    {
        surface_renderer.set_detail(value);
    }
    if let Some(value) = renderer
        .qsc_attr("proberad")
        .and_then(|value| value.parse().ok())
    {
        surface_renderer.set_probe_radius(value);
    }
    if let Some(surface_type) = renderer.qsc_attr("surftype").and_then(|value| match value {
        "vdw" | "0" => Some(cuemol_surface::DirectSurfRenderer::DS_VDW),
        "sas" | "1" => Some(cuemol_surface::DirectSurfRenderer::DS_SAS),
        "ses" | "2" => Some(cuemol_surface::DirectSurfRenderer::DS_SES),
        _ => None,
    }) {
        surface_renderer.set_surface_type(surface_type);
    }
    let surface = surface_renderer.render_mesh(object.mol());
    let atom_colors = cuemol_molvis::resolve_atom_colors(object.mol(), options);
    let palette = object
        .mol()
        .select_indices(&selector)
        .into_iter()
        .filter_map(|index| {
            let atom = object.mol().atoms().get(index)?;
            let color = atom_colors
                .get(index)
                .and_then(|color| color.rgba8())
                .map(|color| {
                    [
                        f32::from(color.r) / 255.0,
                        f32::from(color.g) / 255.0,
                        f32::from(color.b) / 255.0,
                        f32::from(color.a) / 255.0,
                    ]
                })
                .unwrap_or([0.76, 0.79, 0.84, 1.0]);
            Some((atom.position, color))
        })
        .collect::<Vec<_>>();
    let vertices = surface
        .vertices()
        .iter()
        .map(|vertex| {
            let color = palette
                .iter()
                .min_by(|(lhs, _), (rhs, _)| {
                    lhs.sub(vertex.position)
                        .sqlen3d()
                        .partial_cmp(&rhs.sub(vertex.position).sqlen3d())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(_, color)| *color)
                .unwrap_or([0.76, 0.79, 0.84, 1.0]);
            cuemol_render::MeshVertex {
                position: vertex.position,
                normal: vertex.normal,
                color,
            }
        })
        .collect();
    let indices = surface
        .triangles()
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
        .collect();
    let mut snapshot = RenderSceneSnapshot::empty();
    snapshot.meshes.push(RenderMesh { vertices, indices });
    snapshot.recompute_bounds();
    snapshot
}

fn is_mainchain_trace_type(renderer_type: &str) -> bool {
    matches!(
        renderer_type,
        "trace" | "ribbon" | "cartoon" | "tube" | "spline" | "nucl"
    )
}

fn qsc_bool(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    )
}

fn qsc_node_attr(
    node: &cuemol_core::LDom2Node,
    name: &str,
    styles: &cuemol_qsys::StyleManager,
) -> Option<String> {
    let inline = node.str_attr(name).to_string();
    if !inline.is_empty() {
        return Some(inline);
    }
    let style_list = node.str_attr("style").to_string();
    styles.resolve_style_attr(DEFAULT_STYLE_SCOPE, &style_list, name)
}

fn qsc_renderer_rgba(
    node: &cuemol_core::LDom2Node,
    styles: &cuemol_qsys::StyleManager,
) -> cuemol_gfx::Rgba8 {
    let value = qsc_node_attr(node, "color", styles).unwrap_or_else(|| "Blue".to_string());
    qsc_color_rgba(&value, styles).unwrap_or(cuemol_gfx::Rgba8 {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    })
}

fn qsc_color_rgba(value: &str, styles: &cuemol_qsys::StyleManager) -> Option<cuemol_gfx::Rgba8> {
    styles
        .get_color(value, DEFAULT_STYLE_SCOPE)
        .map(|color| color.rgba8())
        .or_else(|| cuemol_gfx::parse_color(value).ok()?.rgba8())
        .or_else(|| {
            Some(
                cuemol_gfx::NamedColor::from_name(value)
                    .resolved_color()
                    .rgba8(),
            )
        })
}

fn qsc_multi_gradient(
    renderer_node: &cuemol_core::LDom2Node,
    styles: &cuemol_qsys::StyleManager,
) -> MultiGradient {
    let inline = renderer_node
        .find_child("multi_grad")
        .filter(|node| !node.is_attr());
    let style_list = renderer_node.str_attr("style").to_string();
    let node = inline.or_else(|| {
        styles.resolve_style_child_node(DEFAULT_STYLE_SCOPE, &style_list, "multi_grad")
    });
    let mut gradient = MultiGradient::new();
    if let Some(node) = node {
        gradient.read_from_ldom2_node(node, |value| qsc_color_rgba(value, styles));
    }
    gradient
}

fn parse_qsc_vector3(value: &str) -> Option<Vector4> {
    let values = value
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .map(|part| part.trim().parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    (values.len() == 3).then(|| Vector4::new3(values[0], values[1], values[2]))
}

fn read_loaded_density_map(loaded: &LoadedDensityMap) -> Result<DensityMap, String> {
    let bytes = &loaded.raw_data;
    let map = match loaded.srctype.as_str() {
        "qdfmap" => read_qdf_denmap_bytes(bytes).map_err(|error| error.to_string())?,
        "mtzmap" => {
            let mut reader = Mtz2MapReader::new();
            reader
                .read_bytes(bytes)
                .map_err(|error| error.to_string())?;
            reader
                .build_density_map()
                .map_err(|error| error.to_string())?
        }
        "qdfpot" => {
            let potential = cuemol_surface::qdf::read_qdf_potential_bytes(bytes)
                .map_err(|error| error.to_string())?;
            let counts = [potential.col_no(), potential.row_no(), potential.sec_no()];
            let mut map = DensityMap::new();
            map.set_map_float_array(
                potential.values(),
                counts,
                [0, 0, 0],
                counts.map(|count| count.max(1)),
                potential.origin(),
                [
                    Vector4::new3(potential.col_grid_size(), 0.0, 0.0),
                    Vector4::new3(0.0, potential.row_grid_size(), 0.0),
                    Vector4::new3(0.0, 0.0, potential.sec_grid_size()),
                ],
            );
            map
        }
        "mmcifmap" => density_map_from_cif(
            std::str::from_utf8(bytes).map_err(|error| error.to_string())?,
            -1.0,
            0.33,
        )
        .map_err(|error| error.to_string())?,
        format => match format {
            "apbs" => DensityMap::from_grid_map(
                cuemol_molstr::read_opendx_str(
                    std::str::from_utf8(bytes).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?,
            ),
            "ccp4map" => Ccp4MapReader::new()
                .read_density_map_bytes(bytes)
                .map_err(|error| error.to_string())?,
            "xplormap" => XplorMapReader::new()
                .read_density_map_str(
                    std::str::from_utf8(bytes).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?,
            "brix" => BrixMapReader::new()
                .read_density_map_bytes(bytes)
                .map_err(|error| error.to_string())?,
            other => return Err(format!("unsupported QSC density map format {other}")),
        },
    };
    Ok(map)
}

/// Append `src` into `dst`, offsetting indices (the merge `tessellate_snapshot`
/// uses for pre-built meshes).
fn merge_mesh(dst: &mut RenderMesh, src: &RenderMesh) {
    let base = dst.vertices.len() as u32;
    dst.vertices.extend(src.vertices.iter().cloned());
    dst.indices
        .extend(src.indices.iter().map(|index| index + base));
}

/// LuxCore export — a multi-file scene: `<base>.cfg` (render config) +
/// `<base>.scn` (scene: camera/lights/material/object) + `<base>.ply`
/// (geometry), mirroring C++ `LuxCoreDisplayContext`'s split output.
fn export_luxcore(
    name: &str,
    output: &str,
    mesh: &RenderMesh,
    snapshot: &RenderSceneSnapshot,
) -> Result<(), String> {
    let lux = LuxCoreSceneExporter::new();
    let base = output
        .strip_suffix(".cfg")
        .or_else(|| output.strip_suffix(".CFG"))
        .unwrap_or(output);
    let out_stem = std::path::Path::new(base)
        .file_name()
        .and_then(|stem| stem.to_str())
        .unwrap_or("scene");
    let ply_path = format!("{base}.ply");
    let scn_path = format!("{base}.scn");
    std::fs::write(&ply_path, lux.write_ply_string(mesh)).map_err(|err| err.to_string())?;
    std::fs::write(
        &scn_path,
        lux.write_scn_string(name, &format!("{out_stem}.ply")),
    )
    .map_err(|err| err.to_string())?;
    std::fs::write(output, lux.write_cfg_string(out_stem, 640, 480))
        .map_err(|err| err.to_string())?;
    eprintln!(
        "exported {} atoms / {} bonds -> {} vertices / {} triangles to {output} (+ {scn_path} + {ply_path})",
        snapshot.atom_count(),
        snapshot.bond_count(),
        mesh.vertices.len(),
        mesh.triangle_count()
    );
    Ok(())
}

/// Warabi export — a multi-file scene: `<base>.wbp` (project referencing the
/// geometry) + `<base>.mqo` (the MQO geometry it imports), mirroring C++
/// `WbpSceneExporter` (which writes the geometry via an MqoDisplayContext).
fn export_wbp(
    name: &str,
    output: &str,
    mesh: &RenderMesh,
    snapshot: &RenderSceneSnapshot,
) -> Result<(), String> {
    let base = output
        .strip_suffix(".wbp")
        .or_else(|| output.strip_suffix(".WBP"))
        .unwrap_or(output);
    let out_stem = std::path::Path::new(base)
        .file_name()
        .and_then(|stem| stem.to_str())
        .unwrap_or("scene");
    let mqo_path = format!("{base}.mqo");
    std::fs::write(
        &mqo_path,
        MqoSceneExporter::new().write_mqo_string(name, mesh),
    )
    .map_err(|err| err.to_string())?;
    std::fs::write(
        output,
        WbpSceneExporter::new().write_wbp_string(name, mesh, &format!("{out_stem}.mqo")),
    )
    .map_err(|err| err.to_string())?;
    eprintln!(
        "exported {} atoms / {} bonds -> {} vertices / {} triangles to {output} (+ {mqo_path})",
        snapshot.atom_count(),
        snapshot.bond_count(),
        mesh.vertices.len(),
        mesh.triangle_count()
    );
    Ok(())
}

fn print_mol_info(mol: &cuemol_molstr::MolCoord) {
    println!("atoms={}", mol.atom_count());
    println!("bonds={}", mol.bond_count());
    println!("residues={}", mol.residue_count());
    println!("chains={}", mol.chain_ids().join(","));
}
