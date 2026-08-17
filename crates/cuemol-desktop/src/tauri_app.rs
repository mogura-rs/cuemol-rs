//! Tauri desktop shell backed by the shared [`DesktopAppState`].

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};

use cuemol_anim::{
    AnimMgr, AnimMotion, CamMotion, MolAnim, NoopAnimObj, ShowHideAnim, SimpleSpin, SlideInOutAnim,
};
use cuemol_core::{LDom2Node, Quat, Uid, Vector4, NANOS_PER_SECOND};
use cuemol_qsys::{Camera, CenterMark, StereoMode, View};
use cuemol_render::{
    tessellate_snapshot, PovSceneExporter, RenderBond, RenderMesh, RenderSceneSnapshot,
    UmbreonDenoiseMode, UmbreonSceneExporter,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::offscreen::{camera_depth_range, camera_mvp, encode_rgba_png, SceneRenderer};
use crate::{
    atoms_in_screen_rect, collect_scene_pov_labels, create_apbs_work_dir, create_pov_work_dir,
    overlay_pov_labels, pick_atom_screen, run_apbs_process, run_pov_process,
    run_pov_process_with_poll, AnimKind, ApbsProcessOutput, ApbsProcessTask, DesktopAppState,
    GuiApbsChargeMethod, GuiApbsOptions, GuiApbsPaths, GuiAtomLabel, GuiPovSceneOptions,
    PovProcessOptions, PovProcessOutput, PovProcessTask, WorkspaceCameraClipboard,
    WorkspaceObjectClipboard, WorkspaceRendererClipboard, WorkspaceStyleClipboard,
    MOLECULE_RENDERER_TYPES,
};

const MAX_VIEW_DIM: u32 = 1_600;
const MIN_INTERACTIVE_RENDER_SCALE: f64 = 0.5;

static SHARED_SCENE_RENDERER: OnceLock<Option<Arc<SceneRenderer>>> = OnceLock::new();

fn shared_scene_renderer() -> Option<Arc<SceneRenderer>> {
    SHARED_SCENE_RENDERER
        .get_or_init(|| SceneRenderer::new().map(Arc::new))
        .clone()
}

fn scaled_viewport_dimensions(width: u32, height: u32, scale: f64) -> (u32, u32) {
    let scale = if scale.is_finite() {
        scale.clamp(MIN_INTERACTIVE_RENDER_SCALE, 1.0)
    } else {
        1.0
    };
    let scaled = |value: u32| {
        let value = value.clamp(64, MAX_VIEW_DIM);
        ((f64::from(value) * scale).round() as u32).clamp(64, MAX_VIEW_DIM)
    };
    (scaled(width), scaled(height))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneDto {
    uid: u64,
    name: String,
    source: String,
    source_type: String,
    base_path: String,
    modified: bool,
    object_count: usize,
    map_count: usize,
    active: bool,
    background: String,
    ao_enabled: bool,
    ao_radius: f64,
    ao_intensity: f64,
    ao_slices: i32,
    ao_steps: i32,
    ao_half_res: bool,
    aa_method: String,
    aa_jitter_level: i32,
    onload: String,
    object_uids: String,
    active_object_id: u64,
    active_renderer_id: u64,
    view_uids: String,
    active_view_id: u64,
    use_color_proof: bool,
    icc_filename: String,
    icc_intent: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewDto {
    uid: u64,
    scene_uid: u64,
    name: String,
    active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct AtomIntrDefDto {
    id: usize,
    mode: u8,
    atoms: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CameraDto {
    scene_uid: u64,
    name: String,
    source: String,
    visibility_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CameraVisibilityEntryDto {
    target_uid: u64,
    parent_uid: Option<u64>,
    name: String,
    type_name: String,
    is_object: bool,
    included: bool,
    visible: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CameraVisibilitySettingInput {
    target_uid: u64,
    is_object: bool,
    visible: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CameraVisibilityUpdateInput {
    scene_uid: u64,
    name: String,
    entries: Vec<CameraVisibilitySettingInput>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StyleDto {
    owner_scene_uid: u64,
    scene_uid: u64,
    local_index: Option<usize>,
    name: String,
    source: String,
    read_only: bool,
    modified: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StylePropertiesDto {
    scene_uid: u64,
    local_index: usize,
    name: String,
    read_only: bool,
    xml: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererStyleOptionDto {
    name: String,
    description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererSetupOptionDto {
    name: String,
    description: String,
    preset: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererSetupOptionsDto {
    renderer_type: Option<String>,
    recenter: bool,
    options: Vec<RendererStyleOptionDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererSetupDto {
    target_kind: String,
    object_name: String,
    renderer_type: String,
    renderer_name: String,
    recenter: bool,
    options: Vec<RendererSetupOptionDto>,
    reader_options: Option<MoleculeReaderOptionsDto>,
    namd_coor_options: Option<NamdCoorReaderOptionsDto>,
    msms_options: Option<MsmsReaderOptionsDto>,
    ccp4_map_options: Option<Ccp4MapReaderOptionsDto>,
    mtz_map_options: Option<MtzMapReaderOptionsDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct MoleculeReaderOptionsDto {
    kind: String,
    ignore_multiple_models: bool,
    ignore_anisou: bool,
    ignore_alt_conf: bool,
    load_segid_as_chain: bool,
    calculate_secondary_structure: bool,
    auto_generate_topology: bool,
}

impl From<&crate::GuiMoleculeReaderOptions> for MoleculeReaderOptionsDto {
    fn from(value: &crate::GuiMoleculeReaderOptions) -> Self {
        Self {
            kind: match value.kind {
                crate::GuiMoleculeReaderKind::Pdb => "pdb",
                crate::GuiMoleculeReaderKind::Mmcif => "mmcif",
            }
            .to_string(),
            ignore_multiple_models: value.ignore_multiple_models,
            ignore_anisou: value.ignore_anisou,
            ignore_alt_conf: value.ignore_alt_conf,
            load_segid_as_chain: value.load_segid_as_chain,
            calculate_secondary_structure: value.calculate_secondary_structure,
            auto_generate_topology: value.auto_generate_topology,
        }
    }
}

impl TryFrom<MoleculeReaderOptionsDto> for crate::GuiMoleculeReaderOptions {
    type Error = String;

    fn try_from(value: MoleculeReaderOptionsDto) -> Result<Self, Self::Error> {
        let kind = match value.kind.as_str() {
            "pdb" => crate::GuiMoleculeReaderKind::Pdb,
            "mmcif" => crate::GuiMoleculeReaderKind::Mmcif,
            kind => return Err(format!("unsupported molecule reader kind: {kind}")),
        };
        Ok(Self {
            kind,
            ignore_multiple_models: value.ignore_multiple_models,
            ignore_anisou: value.ignore_anisou,
            ignore_alt_conf: value.ignore_alt_conf,
            load_segid_as_chain: value.load_segid_as_chain,
            calculate_secondary_structure: value.calculate_secondary_structure,
            auto_generate_topology: value.auto_generate_topology,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NamdCoorReaderOptionsDto {
    topology_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct QscSaveOptionsDto {
    embed_all: bool,
    version: String,
    compress: String,
    base64: bool,
}

impl TryFrom<QscSaveOptionsDto> for crate::GuiSceneSaveOptions {
    type Error = String;

    fn try_from(value: QscSaveOptionsDto) -> Result<Self, Self::Error> {
        let version = match value.version.as_str() {
            "QDF0" => crate::GuiQscVersion::Qdf0,
            "QDF1" => crate::GuiQscVersion::Qdf1,
            version => return Err(format!("unsupported QSC version: {version}")),
        };
        let compression = match value.compress.as_str() {
            "none" => crate::GuiQscCompression::None,
            "gzip" => crate::GuiQscCompression::Gzip,
            "xzip" | "xz" => crate::GuiQscCompression::Xz,
            compression => return Err(format!("unsupported QSC compression: {compression}")),
        };
        Ok(Self {
            embed_all: value.embed_all,
            version,
            compression,
            base64: value.base64,
        })
    }
}

impl From<crate::GuiNamdCoorReaderOptions> for NamdCoorReaderOptionsDto {
    fn from(value: crate::GuiNamdCoorReaderOptions) -> Self {
        Self {
            topology_path: value.topology_path,
        }
    }
}

impl From<NamdCoorReaderOptionsDto> for crate::GuiNamdCoorReaderOptions {
    fn from(value: NamdCoorReaderOptionsDto) -> Self {
        Self {
            topology_path: value.topology_path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct MsmsReaderOptionsDto {
    vertex_path: String,
}

impl From<crate::GuiMsmsReaderOptions> for MsmsReaderOptionsDto {
    fn from(value: crate::GuiMsmsReaderOptions) -> Self {
        Self {
            vertex_path: value.vertex_path,
        }
    }
}

impl From<MsmsReaderOptionsDto> for crate::GuiMsmsReaderOptions {
    fn from(value: MsmsReaderOptionsDto) -> Self {
        Self {
            vertex_path: value.vertex_path,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Ccp4MapReaderOptionsDto {
    normalize: bool,
    truncate_min: bool,
    min: f64,
    truncate_max: bool,
    max: f64,
}

impl From<crate::GuiCcp4MapReaderOptions> for Ccp4MapReaderOptionsDto {
    fn from(value: crate::GuiCcp4MapReaderOptions) -> Self {
        Self {
            normalize: value.normalize,
            truncate_min: value.truncate_min,
            min: value.min,
            truncate_max: value.truncate_max,
            max: value.max,
        }
    }
}

impl From<Ccp4MapReaderOptionsDto> for crate::GuiCcp4MapReaderOptions {
    fn from(value: Ccp4MapReaderOptionsDto) -> Self {
        Self {
            normalize: value.normalize,
            truncate_min: value.truncate_min,
            min: value.min,
            truncate_max: value.truncate_max,
            max: value.max,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct MtzColumnOptionDto {
    name: String,
    kind: char,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MtzMapReaderOptionsDto {
    columns: Vec<MtzColumnOptionDto>,
    column_f: String,
    use_phase: bool,
    column_phi: String,
    use_weight: bool,
    column_weight: String,
    resolution: f64,
    min_resolution: f64,
    max_resolution: f64,
    grid_size: f64,
}

impl From<crate::GuiMtzMapReaderOptions> for MtzMapReaderOptionsDto {
    fn from(value: crate::GuiMtzMapReaderOptions) -> Self {
        Self {
            columns: value
                .columns
                .into_iter()
                .map(|column| MtzColumnOptionDto {
                    name: column.name,
                    kind: column.kind,
                })
                .collect(),
            column_f: value.column_f,
            use_phase: value.use_phase,
            column_phi: value.column_phi,
            use_weight: value.use_weight,
            column_weight: value.column_weight,
            resolution: value.resolution,
            min_resolution: value.min_resolution,
            max_resolution: value.max_resolution,
            grid_size: value.grid_size,
        }
    }
}

impl From<MtzMapReaderOptionsDto> for crate::GuiMtzMapReaderOptions {
    fn from(value: MtzMapReaderOptionsDto) -> Self {
        Self {
            columns: value
                .columns
                .into_iter()
                .map(|column| crate::GuiMtzColumnOption {
                    name: column.name,
                    kind: column.kind,
                })
                .collect(),
            column_f: value.column_f,
            use_phase: value.use_phase,
            column_phi: value.column_phi,
            use_weight: value.use_weight,
            column_weight: value.column_weight,
            resolution: value.resolution,
            min_resolution: value.min_resolution,
            max_resolution: value.max_resolution,
            grid_size: value.grid_size,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RendererSetupInput {
    object_name: String,
    renderer_type: String,
    renderer_name: String,
    selection: Option<String>,
    recenter: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileRendererSetupInput {
    target_kind: String,
    reader_options: Option<MoleculeReaderOptionsDto>,
    namd_coor_options: Option<NamdCoorReaderOptionsDto>,
    msms_options: Option<MsmsReaderOptionsDto>,
    ccp4_map_options: Option<Ccp4MapReaderOptionsDto>,
    mtz_map_options: Option<MtzMapReaderOptionsDto>,
    renderer_setup: RendererSetupInput,
}

impl From<RendererSetupInput> for crate::GuiRendererSetup {
    fn from(value: RendererSetupInput) -> Self {
        Self {
            object_name: value.object_name,
            renderer_type: value.renderer_type,
            renderer_name: value.renderer_name,
            selection: value.selection,
            recenter: value.recenter,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StylePropertiesInput {
    scene_uid: u64,
    local_index: usize,
    name: String,
    read_only: bool,
    xml: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RendererStyleCreateInput {
    scene_uid: u64,
    local_index: usize,
    object_uid: u64,
    renderer_uid: u64,
    name_prefix: String,
    overwrite: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererDto {
    uid: u64,
    name: String,
    renderer_type: String,
    group: Option<String>,
    ui_collapsed: bool,
    selection: String,
    visible: bool,
    locked: bool,
    style: String,
    material: String,
    alpha: f64,
    edge_type: String,
    edge_width: f64,
    edge_color: String,
    line_width: f64,
    detail: i32,
    bond_width: f64,
    sphere_radius: f64,
    show_ring: bool,
    ring_thickness: f64,
    ring_color: String,
    supports_coloring: bool,
    supports_solid_coloring: bool,
    supports_electrostatic_coloring: bool,
    supports_multi_gradient: bool,
    coloring_type: Option<String>,
    scheme_coloring_type: Option<String>,
    coloring_attrs: BTreeMap<String, String>,
    paint_entries: Vec<PaintEntryDto>,
    gradient_entries: Vec<GradientEntryDto>,
    default_color: String,
    specific: BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaintEntryDto {
    selection: String,
    color: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GradientEntryDto {
    value: f64,
    color: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ColoringUpdateInput {
    kind: String,
    object_uid: u64,
    #[serde(default)]
    renderer_uid: Option<u64>,
    #[serde(default)]
    coloring_type: Option<String>,
    #[serde(default)]
    attrs: BTreeMap<String, String>,
    #[serde(default)]
    paint_entries: Vec<PaintEntryDto>,
    #[serde(default)]
    gradient_entries: Vec<GradientEntryDto>,
    #[serde(default)]
    default_color: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectDto {
    uid: u64,
    scene_uid: u64,
    ui_order: i32,
    ui_collapsed: bool,
    name: String,
    source: String,
    source_type: String,
    selection: String,
    visible: bool,
    locked: bool,
    atom_count: usize,
    residue_count: usize,
    bond_count: usize,
    coloring_type: Option<String>,
    coloring_attrs: BTreeMap<String, String>,
    paint_entries: Vec<PaintEntryDto>,
    default_color: String,
    renderers: Vec<RendererDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructAtomDto {
    atom_index: usize,
    serial: i32,
    name: String,
    element: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructResidueDto {
    res_name: String,
    res_id: i32,
    ins_code: Option<char>,
    atoms: Vec<StructAtomDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructChainDto {
    chain_id: String,
    residues: Vec<StructResidueDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructObjectDto {
    object_uid: u64,
    object_name: String,
    chains: Vec<StructChainDto>,
}

fn struct_object_dto(object: crate::GuiStructObject) -> StructObjectDto {
    StructObjectDto {
        object_uid: object.object_uid.get(),
        object_name: object.object_name,
        chains: object
            .chains
            .into_iter()
            .map(|chain| StructChainDto {
                chain_id: chain.chain_id,
                residues: chain
                    .residues
                    .into_iter()
                    .map(|residue| StructResidueDto {
                        res_name: residue.res_name,
                        res_id: residue.res_id,
                        ins_code: residue.ins_code,
                        atoms: residue
                            .atoms
                            .into_iter()
                            .map(|atom| StructAtomDto {
                                atom_index: atom.atom_index,
                                serial: atom.serial,
                                name: atom.name,
                                element: atom.element,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SequenceResidueDto {
    letter: String,
    res_id: i32,
    res_name: String,
    selection: String,
    selected: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SequenceChainDto {
    object_uid: u64,
    object_name: String,
    chain_id: String,
    residues: Vec<SequenceResidueDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CrystalDto {
    object_uid: u64,
    object_name: String,
    a: f64,
    b: f64,
    c: f64,
    alpha: f64,
    beta: f64,
    gamma: f64,
    space_group_name: String,
    space_group_number: Option<i32>,
    lattice: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DensityMapDto {
    uid: u64,
    scene_uid: u64,
    ui_order: i32,
    ui_collapsed: bool,
    name: String,
    class_name: String,
    format: String,
    visible: bool,
    locked: bool,
    source: String,
    counts: [usize; 3],
    min_density: f64,
    max_density: f64,
    mean_density: f64,
    rmsd_density: f64,
    renderers: Vec<RendererDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SurfaceDto {
    uid: u64,
    scene_uid: u64,
    ui_order: i32,
    ui_collapsed: bool,
    name: String,
    class_name: String,
    format: String,
    visible: bool,
    locked: bool,
    source: String,
    original_molecule: String,
    original_selection: String,
    original_density: f64,
    original_probe_radius: f64,
    renderers: Vec<RendererDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LwObjectDto {
    uid: u64,
    scene_uid: u64,
    ui_order: i32,
    ui_collapsed: bool,
    name: String,
    class_name: String,
    format: String,
    visible: bool,
    locked: bool,
    source: String,
    renderers: Vec<LwRendererDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LwRendererDto {
    uid: u64,
    name: String,
    renderer_type: String,
    group: Option<String>,
    ui_collapsed: bool,
    visible: bool,
    locked: bool,
    style: String,
    material: String,
    alpha: f64,
    edge_type: String,
    edge_width: f64,
    edge_color: String,
    data_id: i32,
    specific: BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnimationObjectDto {
    index: usize,
    name: String,
    type_name: String,
    disabled: bool,
    time_ref_name: String,
    relative_start: f64,
    start: f64,
    end: f64,
    duration: f64,
    quadric: f64,
    angle: Option<f64>,
    axis: Option<[f64; 3]>,
    target_camera: Option<String>,
    ignore_center: Option<bool>,
    ignore_rotate: Option<bool>,
    ignore_zoom: Option<bool>,
    ignore_slab: Option<bool>,
    keep_quaternion_positive: Option<bool>,
    renderer_names: Option<String>,
    hide: Option<bool>,
    fade: Option<bool>,
    target_alpha: Option<f64>,
    direction: Option<f64>,
    distance: Option<f64>,
    molecule_name: Option<String>,
    property_name: Option<String>,
    start_value: Option<f64>,
    end_value: Option<f64>,
    start_position: Option<[f64; 3]>,
    end_position: Option<[f64; 3]>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MorphFrameDto {
    index: usize,
    name: String,
    source: String,
    source_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MorphObjectDto {
    object_uid: u64,
    frame: f64,
    frames: Vec<MorphFrameDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrajectoryBlockDto {
    uid: u64,
    name: String,
    source: String,
    format: String,
    frame_count: usize,
    start_index: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrajectoryDto {
    object_uid: u64,
    object_name: String,
    frame_count: usize,
    current_frame: usize,
    blocks: Vec<TrajectoryBlockDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    scenes: Vec<SceneDto>,
    views: Vec<ViewDto>,
    cameras: Vec<CameraDto>,
    styles: Vec<StyleDto>,
    objects: Vec<ObjectDto>,
    structure_key: String,
    sequence_key: String,
    crystals: Vec<CrystalDto>,
    density_maps: Vec<DensityMapDto>,
    surfaces: Vec<SurfaceDto>,
    lw_objects: Vec<LwObjectDto>,
    active_scene_uid: Option<u64>,
    selection: String,
    measure_result: Option<String>,
    perspective: bool,
    background: String,
    show_unit_cell: bool,
    show_symmetry_mates: bool,
    show_density_contour: bool,
    color_proofing: bool,
    density_level: f64,
    animation_objects: Vec<AnimationObjectDto>,
    animation_elapsed: f64,
    animation_duration: f64,
    animation_playing: bool,
    animation_loop: bool,
    animation_start_camera: String,
    morphs: Vec<MorphObjectDto>,
    trajectories: Vec<TrajectoryDto>,
    can_undo: bool,
    can_redo: bool,
    undo_descriptions: Vec<String>,
    redo_descriptions: Vec<String>,
    opened_paths: Vec<String>,
    material_names: Vec<String>,
    log_lines: Vec<String>,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MoleculeDetailsDto {
    structure_key: String,
    structure_object_uid: Option<u64>,
    sequence_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    structure: Option<Vec<StructObjectDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sequence: Option<Vec<SequenceChainDto>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewportLabelDto {
    x: f32,
    y: f32,
    text: String,
    color: String,
    font_size: f64,
    font_name: String,
    font_style: String,
    font_weight: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewportLineDto {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    color: String,
    end_color: Option<String>,
    stipple: bool,
    point: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewportDto {
    image: Option<String>,
    width: u32,
    height: u32,
    perspective: bool,
    center_mark: String,
    lines: Vec<ViewportLineDto>,
    labels: Vec<ViewportLabelDto>,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PickedAtomDto {
    object_uid: u64,
    aid: i32,
}

fn project_viewport_point(
    mvp: [[f32; 4]; 4],
    point: Vector4,
    width: u32,
    height: u32,
) -> Option<(f32, f32)> {
    let value = [point.x as f32, point.y as f32, point.z as f32, 1.0];
    let mut clip = [0.0_f32; 4];
    for (row, component) in clip.iter_mut().enumerate() {
        for (column, value) in value.iter().enumerate() {
            *component += mvp[column][row] * value;
        }
    }
    if clip[3] <= 0.0 {
        return None;
    }
    Some((
        (clip[0] / clip[3] + 1.0) * 0.5 * width as f32,
        (1.0 - clip[1] / clip[3]) * 0.5 * height as f32,
    ))
}

fn viewport_css_color(color: cuemol_gfx::Rgba8) -> String {
    if color.a == u8::MAX {
        format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.r, color.g, color.b, color.a
        )
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitialStateDto {
    snapshot: AppSnapshot,
    viewport: ViewportDto,
    preferences: PreferencesDto,
    startup_paths_pending: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MolStructUpdateDto {
    snapshot: AppSnapshot,
    viewport: ViewportDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnimationFrameDto {
    animation_elapsed: f64,
    animation_duration: f64,
    animation_playing: bool,
    status: String,
    viewport: ViewportDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrajectoryFrameDto {
    object_uid: u64,
    frame: usize,
    status: String,
    viewport: ViewportDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginDto {
    name: String,
    description: String,
    extensions: String,
    category: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigEntryDto {
    key: String,
    value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationInfoDto {
    product_name: &'static str,
    version: &'static str,
    target: String,
    features: Vec<&'static str>,
    plugins: Vec<PluginDto>,
    config: Vec<ConfigEntryDto>,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewPropertiesDto {
    name: String,
    uid: u64,
    active: bool,
    zoom: f64,
    distance: f64,
    slab: f64,
    center: [f64; 3],
    rotation: [f64; 4],
    perspective: bool,
    width: u32,
    height: u32,
    trans_mms: bool,
    rot_mms: bool,
    stereo_mode: String,
    has_hw_stereo: bool,
    stereo_dist: f64,
    swap_stereo_eyes: bool,
    up_vec: [f64; 3],
    right_vec: [f64; 3],
    forward_vec: [f64; 3],
    center_mark: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewPropertiesInput {
    name: String,
    zoom: f64,
    distance: f64,
    slab: f64,
    center: [f64; 3],
    rotation: [f64; 4],
    perspective: bool,
    trans_mms: bool,
    rot_mms: bool,
    stereo_mode: String,
    stereo_dist: f64,
    swap_stereo_eyes: bool,
    center_mark: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenePropertiesInput {
    scene_uid: u64,
    background: String,
    ao_enabled: bool,
    ao_radius: f64,
    ao_intensity: f64,
    ao_slices: i32,
    ao_steps: i32,
    ao_half_res: bool,
    aa_method: String,
    aa_jitter_level: i32,
    onload: String,
    use_color_proof: bool,
    icc_filename: String,
    icc_intent: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewPropertiesUpdateDto {
    properties: ViewPropertiesDto,
    snapshot: AppSnapshot,
    viewport: ViewportDto,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ObjectPropertiesInput {
    object_uid: u64,
    name: String,
    visible: bool,
    locked: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LwRendererPropertiesInput {
    object_uid: u64,
    renderer_uid: u64,
    name: String,
    visible: bool,
    locked: bool,
    material: String,
    alpha: f64,
    edge_type: String,
    edge_width: f64,
    edge_color: String,
    data_id: i32,
    specific: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceDeleteInput {
    kind: String,
    object_uid: u64,
    #[serde(default)]
    renderer_uid: Option<u64>,
    #[serde(default)]
    scene_uid: Option<u64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    local_index: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceToggleInput {
    kind: String,
    object_uid: u64,
    #[serde(default)]
    renderer_uid: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceFitInput {
    kind: String,
    object_uid: u64,
    #[serde(default)]
    renderer_uid: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceMoveRendererInput {
    object_uid: u64,
    renderer_uid: u64,
    target_uid: u64,
    position: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceMoveRenderersInput {
    object_uid: u64,
    renderer_uids: Vec<u64>,
    target_uid: u64,
    position: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererCollapsedInput {
    object_uid: u64,
    renderer_uid: u64,
    collapsed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceObjectCollapsedInput {
    object_uid: u64,
    collapsed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceMoveObjectInput {
    scene_uid: u64,
    source_kind: String,
    source_uid: u64,
    target_kind: String,
    target_uid: u64,
    position: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererSelectionInput {
    object_uid: u64,
    renderer_uid: u64,
    selection: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererTypeInput {
    object_uid: u64,
    renderer_uid: u64,
    renderer_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceNewRendererInput {
    kind: String,
    object_uid: u64,
    renderer_type: String,
    renderer_name: String,
    selection: Option<String>,
    group_name: Option<String>,
    density_centering: Option<String>,
    recenter: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererStyleInput {
    object_uid: u64,
    renderer_uid: u64,
    style_name: String,
    component: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererStylesInput {
    object_uid: u64,
    renderer_uid: u64,
    style_names: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererColoringInput {
    object_uid: u64,
    renderer_uid: u64,
    preset: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererPaintInput {
    object_uid: u64,
    renderer_uid: u64,
    color: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceObjectPaintInput {
    object_uid: u64,
    color: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererCopyInput {
    object_uid: u64,
    renderer_uid: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererPasteInput {
    object_uid: u64,
    #[serde(default)]
    destination_group_uid: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceObjectCopyInput {
    object_uid: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceObjectPasteInput {
    scene_uid: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceCameraCopyInput {
    scene_uid: u64,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceCameraPasteInput {
    scene_uid: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceCameraRenameInput {
    scene_uid: u64,
    old_name: String,
    new_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceStyleCopyInput {
    scene_uid: u64,
    local_index: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceStylePasteInput {
    scene_uid: u64,
    replace: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRendererGroupInput {
    object_uid: u64,
    #[serde(default)]
    group_uid: Option<u64>,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRenameInput {
    kind: String,
    object_uid: u64,
    #[serde(default)]
    renderer_uid: Option<u64>,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RendererPropertiesInput {
    object_uid: u64,
    renderer_uid: u64,
    name: String,
    selection: String,
    visible: bool,
    locked: bool,
    material: String,
    alpha: f64,
    edge_type: String,
    edge_width: f64,
    edge_color: String,
    line_width: f64,
    detail: i32,
    bond_width: f64,
    sphere_radius: f64,
    show_ring: bool,
    ring_thickness: f64,
    ring_color: String,
    #[serde(default)]
    specific: BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShortcutDto {
    id: String,
    key: String,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferencesDto {
    atom_label_font: String,
    atom_label_size: f64,
    atom_label_color: String,
    atom_label_italic: bool,
    atom_label_bold: bool,
    ui_language: String,
    use_hidpi: bool,
    mouse_preset: String,
    trackball_radius: f64,
    hit_precision: f64,
    momentum_scroll: bool,
    multitouch_pad: bool,
    emulate_right_button: bool,
    shortcuts: Vec<ShortcutDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreferencesUpdateDto {
    preferences: PreferencesDto,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApbsCalculationInput {
    object_uid: u64,
    selection: String,
    potential_name: String,
    charge_method: String,
    force_field: String,
    use_hydrogen: bool,
    use_npbe: bool,
    temperature: f64,
    grid_spacing: f64,
    water_dielectric: f64,
    protein_dielectric: f64,
    apbs_executable: String,
    pdb2pqr_executable: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PovRenderInput {
    scene_uid: u64,
    view_uid: u64,
    width: u32,
    height: u32,
    threads: u32,
    perspective: bool,
    stereo: i32,
    stereo_depth: f64,
    transparent: bool,
    clip_plane: bool,
    post_blend: bool,
    edge_lines: bool,
    pixel_labels: bool,
    executable: String,
    include_dir: String,
    radiosity: i32,
    shadow: bool,
    default_lighting: bool,
    light_spread: u32,
    light_intensity: f64,
    flash_fraction: f64,
    ambient_fraction: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnimationRenderInput {
    scene_uid: u64,
    output_dir: String,
    base_name: String,
    width: u32,
    height: u32,
    frame_rate: f64,
    perspective: bool,
    duplicate_last_frame: bool,
    concurrency: u32,
    backend: String,
    povray_executable: String,
    povray_include_dir: String,
    pov_clip_plane: bool,
    pov_post_blend: bool,
    pov_shadow: bool,
    pov_edge_lines: bool,
    pov_labels: bool,
    pov_radiosity: i32,
    make_movie: bool,
    ffmpeg_executable: String,
    movie_format: String,
    bitrate_kbps: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnimationObjectPropertiesInput {
    index: usize,
    name: String,
    disabled: bool,
    time_ref_name: String,
    start: f64,
    duration: f64,
    quadric: f64,
    angle: Option<f64>,
    axis: Option<[f64; 3]>,
    target_camera: Option<String>,
    ignore_center: Option<bool>,
    ignore_rotate: Option<bool>,
    ignore_zoom: Option<bool>,
    ignore_slab: Option<bool>,
    keep_quaternion_positive: Option<bool>,
    renderer_names: Option<String>,
    hide: Option<bool>,
    fade: Option<bool>,
    target_alpha: Option<f64>,
    direction: Option<f64>,
    distance: Option<f64>,
    molecule_name: Option<String>,
    property_name: Option<String>,
    start_value: Option<f64>,
    end_value: Option<f64>,
    start_position: Option<[f64; 3]>,
    end_position: Option<[f64; 3]>,
}

struct AppModel {
    desktop: DesktopAppState,
    config_path: Option<PathBuf>,
    camera: Camera,
    renderer: Option<Arc<SceneRenderer>>,
    renderer_warmup: Option<std::thread::JoinHandle<Option<Arc<SceneRenderer>>>>,
    renderer_init_attempted: bool,
    mesh: Option<RenderMesh>,
    screen_lines: Vec<RenderBond>,
    mesh_refit_pending: bool,
    selection: String,
    background: String,
    show_unit_cell: bool,
    show_symmetry_mates: bool,
    show_density_contour: bool,
    color_proofing: bool,
    density_level: f64,
    animation_elapsed: f64,
    animation_playing: bool,
    animation_start_camera: Option<Camera>,
    animation_start_desktop: Option<DesktopAppState>,
    view_width: u32,
    view_height: u32,
    trans_mms: bool,
    rot_mms: bool,
    swap_stereo_eyes: bool,
    opened_paths: Vec<String>,
    structure_revision: u64,
    modified_scenes: BTreeSet<Uid>,
    workspace_camera_clipboard: Option<WorkspaceCameraClipboard>,
    workspace_object_clipboard: Option<WorkspaceObjectClipboard>,
    workspace_renderer_clipboard: Option<WorkspaceRendererClipboard>,
    workspace_style_clipboard: Option<WorkspaceStyleClipboard>,
    undo_stack: Vec<AppModelUndoEntry>,
    redo_stack: Vec<AppModelUndoEntry>,
    apbs_job: Option<TauriApbsJob>,
    pov_render_job: Option<TauriPovRenderJob>,
    pov_render_result: Option<TauriPovRenderResult>,
    animation_render_job: Option<TauriAnimationRenderJob>,
    log_lines: Vec<String>,
    status: String,
}

struct TauriApbsJob {
    receiver: mpsc::Receiver<Result<ApbsProcessOutput, String>>,
    cancel: Arc<AtomicBool>,
    potential_name: String,
}

struct TauriPovRenderJob {
    receiver: mpsc::Receiver<Result<PovProcessOutput, String>>,
    cancel: Arc<AtomicBool>,
    work_dir: PathBuf,
    labels: Vec<GuiAtomLabel>,
}

struct TauriPovRenderResult {
    work_dir: PathBuf,
    image_path: PathBuf,
}

struct TauriAnimationRenderJob {
    input: AnimationRenderInput,
    restore: AppModelUndo,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
    total_frames: u32,
    next_frame: u32,
    completed_frames: u32,
    frame_receivers: Vec<mpsc::Receiver<Result<TauriAnimationTaskOutput, String>>>,
    movie_receiver: Option<mpsc::Receiver<Result<TauriAnimationTaskOutput, String>>>,
    encoding_started: bool,
}

struct TauriAnimationFrameTask {
    input: AnimationRenderInput,
    index: u32,
    total_frames: u32,
    snapshot: RenderSceneSnapshot,
    camera: Camera,
    labels: Vec<GuiAtomLabel>,
    background: [f32; 4],
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
}

struct TauriAnimationMovieTask {
    input: AnimationRenderInput,
    total_frames: u32,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
}

enum TauriAnimationTaskOutput {
    Frame {
        index: u32,
        path: PathBuf,
        log: String,
    },
    Movie {
        path: PathBuf,
        log: String,
    },
}

#[derive(Clone)]
struct AppModelUndo {
    desktop: DesktopAppState,
    camera: Camera,
    structure_revision: u64,
    selection: String,
    background: String,
    show_unit_cell: bool,
    show_symmetry_mates: bool,
    show_density_contour: bool,
    color_proofing: bool,
    density_level: f64,
    animation_elapsed: f64,
    trans_mms: bool,
    rot_mms: bool,
    swap_stereo_eyes: bool,
    modified_scenes: BTreeSet<Uid>,
}

#[derive(Clone)]
struct AppModelUndoEntry {
    snapshot: AppModelUndo,
    description: String,
}

impl AppModel {
    #[cfg(test)]
    fn new(open_paths: Vec<PathBuf>) -> Self {
        Self::new_with_config(open_paths, None)
    }

    fn new_with_config(open_paths: Vec<PathBuf>, config_path: Option<PathBuf>) -> Self {
        let config_path_to_load = config_path.as_ref().filter(|path| path.exists()).cloned();
        let (config_loader, config_fallback) = config_path_to_load.map_or((None, None), |path| {
            let worker_path = path.clone();
            match std::thread::Builder::new()
                .name("cuemol-tauri-config-init".to_string())
                .spawn(move || fs::read_to_string(&worker_path).map_err(|error| error.to_string()))
            {
                Ok(loader) => (Some((path, loader)), None),
                Err(_) => (None, Some(path)),
            }
        });
        let mut desktop = DesktopAppState::new();
        let mut failures = Vec::new();
        let config_source = if let Some((path, loader)) = config_loader {
            Some((
                path,
                loader
                    .join()
                    .unwrap_or_else(|_| Err("configuration loader panicked".to_string())),
            ))
        } else {
            config_fallback.map(|path| {
                let source = fs::read_to_string(&path).map_err(|error| error.to_string());
                (path, source)
            })
        };
        if let Some((path, source)) = config_source {
            match source {
                Ok(input) => {
                    if let Err(error) = desktop.load_config_text(&input) {
                        failures.push(format!("{}: {error}", path.display()));
                    }
                }
                Err(error) => failures.push(format!("{}: {error}", path.display())),
            }
        }
        desktop.ensure_active_scene();
        let mut loaded = 0usize;
        for path in open_paths {
            match desktop.open_path(&path) {
                Ok(_) => loaded += 1,
                Err(error) => failures.push(format!("{}: {error}", path.display())),
            }
        }
        let status = if failures.is_empty() {
            if loaded == 0 {
                "Ready".to_string()
            } else {
                format!("Loaded {loaded} file(s)")
            }
        } else {
            format!("Load failed: {}", failures.join("; "))
        };
        let mut model = Self {
            desktop,
            config_path,
            camera: Camera::new(),
            renderer: None,
            renderer_warmup: None,
            renderer_init_attempted: false,
            mesh: None,
            screen_lines: Vec::new(),
            mesh_refit_pending: loaded > 0,
            selection: String::new(),
            background: "black".to_string(),
            show_unit_cell: false,
            show_symmetry_mates: false,
            show_density_contour: false,
            color_proofing: false,
            density_level: 1.5,
            animation_elapsed: 0.0,
            animation_playing: false,
            animation_start_camera: None,
            animation_start_desktop: None,
            view_width: 100,
            view_height: 100,
            trans_mms: false,
            rot_mms: false,
            swap_stereo_eyes: false,
            opened_paths: Vec::new(),
            structure_revision: 1,
            modified_scenes: BTreeSet::new(),
            workspace_camera_clipboard: None,
            workspace_object_clipboard: None,
            workspace_renderer_clipboard: None,
            workspace_style_clipboard: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            apbs_job: None,
            pov_render_job: None,
            pov_render_result: None,
            animation_render_job: None,
            log_lines: Vec::new(),
            status,
        };
        model.apply_stored_preferences();
        model.sync_active_scene_properties();
        model
    }

    fn snapshot(&self) -> AppSnapshot {
        let scene_summaries = self.desktop.scene_summaries();
        let view_summaries = self.desktop.view_summaries();
        let scenes = scene_summaries
            .iter()
            .map(|summary| {
                let scene = self
                    .desktop
                    .scene_manager()
                    .scene(summary.uid)
                    .expect("scene summary must reference a live scene");
                let object_uids = scene
                    .objects()
                    .iter()
                    .map(|object| object.uid().get().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let view_uids = view_summaries
                    .iter()
                    .filter(|view| view.scene_uid == summary.uid)
                    .map(|view| view.uid.get().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                SceneDto {
                    uid: summary.uid.get(),
                    name: summary.name.clone(),
                    source: scene.source().to_string(),
                    source_type: scene.source_type().to_string(),
                    base_path: scene.base_path().to_string(),
                    modified: self.modified_scenes.contains(&summary.uid),
                    object_count: summary.object_count,
                    map_count: summary.map_count,
                    active: summary.active,
                    background: scene
                        .scene_attribute("bgcolor")
                        .unwrap_or("black")
                        .to_string(),
                    ao_enabled: scene_attr_bool(scene.scene_attribute("aoEnabled"), false),
                    ao_radius: scene_attr_f64(scene.scene_attribute("aoRadius"), 4.0),
                    ao_intensity: scene_attr_f64(scene.scene_attribute("aoIntensity"), 2.2),
                    ao_slices: scene_attr_i32(scene.scene_attribute("aoSlices"), 9),
                    ao_steps: scene_attr_i32(scene.scene_attribute("aoSteps"), 3),
                    ao_half_res: scene_attr_bool(scene.scene_attribute("aoHalfRes"), false),
                    aa_method: scene
                        .scene_attribute("aa_method")
                        .unwrap_or("fxaa")
                        .to_string(),
                    aa_jitter_level: scene_attr_i32(scene.scene_attribute("aaJitterLevel"), 0),
                    onload: scene.scene_attribute("onload").unwrap_or("").to_string(),
                    object_uids,
                    active_object_id: scene.active_object_uid().map_or(0, Uid::get),
                    active_renderer_id: scene.active_renderer_uid().map_or(0, Uid::get),
                    view_uids,
                    active_view_id: scene.active_view_uid().map_or(0, Uid::get),
                    use_color_proof: scene_attr_bool(scene.scene_attribute("use_colproof"), false),
                    icc_filename: scene
                        .scene_attribute("icc_filename")
                        .unwrap_or("")
                        .to_string(),
                    icc_intent: scene
                        .scene_attribute("icc_intent")
                        .unwrap_or("perceptual")
                        .to_string(),
                }
            })
            .collect::<Vec<_>>();
        let views = view_summaries
            .into_iter()
            .map(|view| ViewDto {
                uid: view.uid.get(),
                scene_uid: view.scene_uid.get(),
                name: view.name,
                active: view.active,
            })
            .collect();
        let mut objects = Vec::new();
        let cameras = scene_summaries
            .iter()
            .flat_map(|scene| self.desktop.camera_summaries(scene.uid))
            .map(|camera| CameraDto {
                scene_uid: camera.scene_uid.get(),
                name: camera.name,
                source: camera.source,
                visibility_count: camera.visibility_count,
            })
            .collect();
        let styles = scene_summaries
            .iter()
            .flat_map(|scene| {
                self.desktop
                    .style_summaries(scene.uid)
                    .into_iter()
                    .map(move |style| StyleDto {
                        owner_scene_uid: scene.uid.get(),
                        scene_uid: style.scene_uid.get(),
                        local_index: style.local_index,
                        name: style.name,
                        source: style.source,
                        read_only: style.read_only,
                        modified: style.modified,
                    })
            })
            .collect();
        for scene in scene_summaries {
            let styles = self
                .desktop
                .scene_manager()
                .scene(scene.uid)
                .map(cuemol_molvis::scene_style_manager)
                .unwrap_or_else(cuemol_molvis::default_style_manager);
            for object in self.desktop.object_summaries(scene.uid) {
                let object_state = self
                    .desktop
                    .scene_manager()
                    .object(object.uid)
                    .expect("object summary must reference a live object");
                let renderers = self
                    .desktop
                    .renderer_summaries(object.uid)
                    .into_iter()
                    .map(|renderer| {
                        let state = object_state
                            .renderer(renderer.uid)
                            .expect("renderer summary must reference a live renderer");
                        let specific = renderer_specific_attributes(
                            state,
                            renderer.renderer_type.as_str(),
                            &styles,
                        );
                        let (mut coloring_type, mut coloring_attrs, paint_entries) =
                            renderer_coloring_details(state, &styles, scene.uid.get());
                        let direct_surface = matches!(
                            renderer.renderer_type.as_str(),
                            "dsurface" | "directsurf" | "dsurf2"
                        );
                        if direct_surface
                            && specific
                                .get("colormode")
                                .is_some_and(|mode| mode == "potential" || mode == "1")
                        {
                            coloring_type = Some("ElectrostaticColoring".to_string());
                            coloring_attrs = coloring_attrs_from_specific(
                                &specific,
                                &[
                                    "elepot",
                                    "lowpar",
                                    "midpar",
                                    "highpar",
                                    "lowcol",
                                    "midcol",
                                    "highcol",
                                    "ramp_above",
                                    "ramp_value",
                                ],
                            );
                        }
                        let supports_coloring = !matches!(
                            renderer.renderer_type.as_str(),
                            "selection"
                                | "sel"
                                | "*selection"
                                | "atomintr"
                                | "*namelabel"
                                | "name_label"
                        );
                        RendererDto {
                            uid: renderer.uid.get(),
                            name: renderer.name,
                            renderer_type: renderer.renderer_type,
                            group: renderer.group,
                            ui_collapsed: renderer.ui_collapsed,
                            selection: if matches!(
                                state.renderer_type(),
                                "*namelabel" | "name_label"
                            ) {
                                String::new()
                            } else if matches!(state.renderer_type(), "*selection" | "selection") {
                                let selection = object_state.mol().selection_text();
                                if selection.is_empty() {
                                    "!*".to_string()
                                } else {
                                    selection.to_string()
                                }
                            } else {
                                state.qsc_attr("sel").unwrap_or("*").to_string()
                            },
                            visible: renderer.visible,
                            locked: state.qsc_attr("locked").is_some_and(config_value_is_true),
                            style: state.style().unwrap_or_default().to_string(),
                            material: state.qsc_attr("material").unwrap_or("").to_string(),
                            alpha: state
                                .qsc_attr("alpha")
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(1.0),
                            edge_type: state.qsc_attr("egtype").unwrap_or("none").to_string(),
                            edge_width: state
                                .qsc_attr("eglinew")
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(0.01),
                            edge_color: state.qsc_attr("egcolor").unwrap_or("#000000").to_string(),
                            line_width: state
                                .qsc_attr(if state.renderer_type() == "spline" {
                                    "line_width"
                                } else {
                                    "width"
                                })
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(1.2),
                            detail: state
                                .qsc_attr("detail")
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(3),
                            bond_width: state
                                .qsc_attr("bondw")
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(0.2),
                            sphere_radius: state
                                .qsc_attr("sphr")
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(0.3),
                            show_ring: state.qsc_attr("ring").is_some_and(config_value_is_true),
                            ring_thickness: state
                                .qsc_attr("thickness")
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(0.2),
                            ring_color: state
                                .qsc_attr("ringcolor")
                                .unwrap_or("#ffffbf")
                                .to_string(),
                            supports_coloring,
                            supports_solid_coloring: false,
                            supports_electrostatic_coloring: direct_surface,
                            supports_multi_gradient: false,
                            scheme_coloring_type: coloring_type.clone(),
                            coloring_type,
                            coloring_attrs,
                            paint_entries,
                            gradient_entries: Vec::new(),
                            default_color: state
                                .resolved_attr("defaultcolor", &styles, scene.uid.get())
                                .unwrap_or_else(|| state.default_color().script_string()),
                            specific,
                        }
                    })
                    .collect();
                objects.push(ObjectDto {
                    uid: object.uid.get(),
                    scene_uid: scene.uid.get(),
                    ui_order: object.ui_order,
                    ui_collapsed: object.ui_collapsed,
                    name: object.name,
                    source: object_state.source().to_string(),
                    source_type: object_state.source_type().to_string(),
                    selection: object_state.mol().selection_text().to_string(),
                    visible: object_state.is_visible(),
                    locked: object_state.is_ui_locked(),
                    atom_count: object.atom_count,
                    residue_count: object.residue_count,
                    bond_count: object.bond_count,
                    coloring_type: self.desktop.object_coloring_type(object.uid),
                    coloring_attrs: object_state.coloring_attrs().clone(),
                    paint_entries: object_state
                        .paint_coloring()
                        .iter()
                        .map(|(selection, color)| PaintEntryDto {
                            selection: selection.clone(),
                            color: color.clone(),
                        })
                        .collect(),
                    default_color: object_state.mol().default_color().script_string(),
                    renderers,
                });
            }
        }
        let crystals = self
            .desktop
            .active_crystal_summaries()
            .into_iter()
            .map(|crystal| CrystalDto {
                object_uid: crystal.object_uid.get(),
                object_name: crystal.object_name,
                a: crystal.a,
                b: crystal.b,
                c: crystal.c,
                alpha: crystal.alpha,
                beta: crystal.beta,
                gamma: crystal.gamma,
                space_group_name: crystal.space_group_name,
                space_group_number: crystal.space_group_number,
                lattice: crystal.lattice,
            })
            .collect();
        let density_maps = self
            .desktop
            .active_density_map_summaries()
            .into_iter()
            .map(|map| {
                let mut renderers: Vec<_> = map
                    .renderers
                    .into_iter()
                    .map(|renderer| {
                        let ui_order = renderer.ui_order;
                        let uid = renderer.uid.get();
                        let coloring_type = scalar_coloring_type(&renderer.specific);
                        let coloring_attrs = scalar_coloring_attrs(&renderer.specific);
                        let dto = RendererDto {
                            uid,
                            name: renderer.name,
                            renderer_type: renderer.renderer_type,
                            group: renderer.group,
                            ui_collapsed: false,
                            selection: String::new(),
                            visible: renderer.visible,
                            locked: renderer.locked,
                            style: renderer.specific.get("style").cloned().unwrap_or_default(),
                            material: renderer.material,
                            alpha: renderer.alpha,
                            edge_type: renderer.edge_type,
                            edge_width: renderer.edge_width,
                            edge_color: renderer.edge_color,
                            line_width: 1.2,
                            detail: 3,
                            bond_width: 0.2,
                            sphere_radius: 0.3,
                            show_ring: false,
                            ring_thickness: 0.2,
                            ring_color: "#ffffbf".to_string(),
                            supports_coloring: false,
                            supports_solid_coloring: true,
                            supports_electrostatic_coloring: false,
                            supports_multi_gradient: true,
                            scheme_coloring_type: None,
                            coloring_type,
                            coloring_attrs,
                            paint_entries: Vec::new(),
                            gradient_entries: renderer
                                .multi_gradient
                                .into_iter()
                                .map(|(value, color)| GradientEntryDto { value, color })
                                .collect(),
                            default_color: renderer
                                .specific
                                .get("color")
                                .cloned()
                                .unwrap_or_else(|| "#c0c0ff".to_string()),
                            specific: renderer.specific,
                        };
                        (ui_order, uid, dto)
                    })
                    .collect();
                renderers.extend(map.renderer_groups.into_iter().map(|group| {
                    let ui_order = group.ui_order;
                    let uid = group.uid.get();
                    (ui_order, uid, aux_renderer_group_dto(group))
                }));
                let renderers = ordered_renderer_dtos(renderers);
                DensityMapDto {
                    uid: map.uid.get(),
                    scene_uid: map.scene_uid.get(),
                    ui_order: map.ui_order,
                    ui_collapsed: map.ui_collapsed,
                    name: map.name,
                    class_name: map.class_name,
                    format: map.format,
                    visible: map.visible,
                    locked: map.locked,
                    source: map.source,
                    counts: map.counts,
                    min_density: map.min_density,
                    max_density: map.max_density,
                    mean_density: map.mean_density,
                    rmsd_density: map.rmsd_density,
                    renderers,
                }
            })
            .collect();
        let surfaces = self
            .desktop
            .active_surface_summaries()
            .into_iter()
            .map(|surface| {
                let mut renderers: Vec<_> = surface
                    .renderers
                    .into_iter()
                    .map(|renderer| {
                        let ui_order = renderer.ui_order;
                        let uid = renderer.uid.get();
                        let coloring_type = scalar_coloring_type(&renderer.specific);
                        let coloring_attrs = scalar_coloring_attrs(&renderer.specific);
                        let scheme_coloring = self
                            .desktop
                            .renderer_coloring_state(surface.uid, renderer.uid);
                        let dto = RendererDto {
                            uid,
                            name: renderer.name,
                            renderer_type: renderer.renderer_type,
                            group: renderer.group,
                            ui_collapsed: false,
                            selection: renderer.selection,
                            visible: renderer.visible,
                            locked: renderer.locked,
                            style: renderer.specific.get("style").cloned().unwrap_or_default(),
                            material: renderer.material,
                            alpha: renderer.alpha,
                            edge_type: renderer.edge_type,
                            edge_width: renderer.edge_width,
                            edge_color: renderer.edge_color,
                            line_width: 1.2,
                            detail: 3,
                            bond_width: 0.2,
                            sphere_radius: 0.3,
                            show_ring: false,
                            ring_thickness: 0.2,
                            ring_color: "#ffffbf".to_string(),
                            supports_coloring: true,
                            supports_solid_coloring: true,
                            supports_electrostatic_coloring: true,
                            supports_multi_gradient: true,
                            scheme_coloring_type: scheme_coloring
                                .as_ref()
                                .map(|(coloring_type, _, _)| coloring_type.clone()),
                            coloring_type,
                            coloring_attrs,
                            paint_entries: scheme_coloring
                                .map(|(_, _, entries)| {
                                    entries
                                        .into_iter()
                                        .map(|(selection, color)| PaintEntryDto {
                                            selection,
                                            color,
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            gradient_entries: renderer
                                .multi_gradient
                                .into_iter()
                                .map(|(value, color)| GradientEntryDto { value, color })
                                .collect(),
                            default_color: renderer
                                .specific
                                .get("defaultcolor")
                                .cloned()
                                .unwrap_or_else(|| "#c0c0ff".to_string()),
                            specific: renderer.specific,
                        };
                        (ui_order, uid, dto)
                    })
                    .collect();
                renderers.extend(surface.renderer_groups.into_iter().map(|group| {
                    let ui_order = group.ui_order;
                    let uid = group.uid.get();
                    (ui_order, uid, aux_renderer_group_dto(group))
                }));
                let renderers = ordered_renderer_dtos(renderers);
                SurfaceDto {
                    uid: surface.uid.get(),
                    scene_uid: surface.scene_uid.get(),
                    ui_order: surface.ui_order,
                    ui_collapsed: surface.ui_collapsed,
                    name: surface.name,
                    class_name: surface.class_name,
                    format: surface.format,
                    visible: surface.visible,
                    locked: surface.locked,
                    source: surface.source,
                    original_molecule: surface.original_molecule,
                    original_selection: surface.original_selection,
                    original_density: surface.original_density,
                    original_probe_radius: surface.original_probe_radius,
                    renderers,
                }
            })
            .collect();
        let lw_objects = self
            .desktop
            .active_lw_object_summaries()
            .into_iter()
            .map(|object| {
                let mut renderers: Vec<_> = object
                    .renderers
                    .into_iter()
                    .map(|renderer| {
                        let ui_order = renderer.ui_order;
                        let uid = renderer.uid.get();
                        (
                            ui_order,
                            uid,
                            LwRendererDto {
                                uid,
                                name: renderer.name,
                                renderer_type: renderer.renderer_type,
                                group: renderer.group,
                                ui_collapsed: false,
                                visible: renderer.visible,
                                locked: renderer.locked,
                                style: renderer.style.clone(),
                                material: renderer.material,
                                alpha: renderer.alpha,
                                edge_type: renderer.edge_type,
                                edge_width: renderer.edge_width,
                                edge_color: renderer.edge_color,
                                data_id: renderer.data_id,
                                specific: BTreeMap::from([
                                    ("data_id".to_string(), renderer.data_id.to_string()),
                                    ("style".to_string(), renderer.style),
                                ]),
                            },
                        )
                    })
                    .collect();
                renderers.extend(object.renderer_groups.into_iter().map(|group| {
                    let ui_order = group.ui_order;
                    let uid = group.uid.get();
                    (ui_order, uid, aux_lw_renderer_group_dto(group))
                }));
                let renderers = ordered_renderer_dtos(renderers);
                LwObjectDto {
                    uid: object.uid.get(),
                    scene_uid: object.scene_uid.get(),
                    ui_order: object.ui_order,
                    ui_collapsed: object.ui_collapsed,
                    name: object.name,
                    class_name: object.class_name,
                    format: object.format,
                    visible: object.visible,
                    locked: object.locked,
                    source: object.source,
                    renderers,
                }
            })
            .collect();
        let animation = self.active_animation_manager().unwrap_or_default();
        let animation_objects = animation
            .motions()
            .iter()
            .enumerate()
            .map(|(index, motion)| animation_object_dto(index, motion))
            .collect();
        let morphs = objects
            .iter()
            .filter_map(|object| {
                let object_uid = Uid::new(object.uid);
                let frames = self.desktop.morph_frame_summaries(object_uid);
                (!frames.is_empty()).then(|| MorphObjectDto {
                    object_uid: object.uid,
                    frame: self.desktop.morph_frame_value(object_uid).unwrap_or(0.0),
                    frames: frames
                        .into_iter()
                        .map(|frame| MorphFrameDto {
                            index: frame.index,
                            name: frame.name,
                            source: frame.source,
                            source_type: frame.source_type,
                        })
                        .collect(),
                })
            })
            .collect();
        let trajectories = self
            .desktop
            .active_trajectory_summaries()
            .into_iter()
            .map(|trajectory| TrajectoryDto {
                object_uid: trajectory.object_uid.get(),
                object_name: trajectory.object_name,
                frame_count: trajectory.frame_count,
                current_frame: trajectory.current_frame,
                blocks: trajectory
                    .blocks
                    .into_iter()
                    .map(|block| TrajectoryBlockDto {
                        uid: block.uid,
                        name: block.name,
                        source: block.source,
                        format: block.format,
                        frame_count: block.frame_count,
                        start_index: block.start_index,
                    })
                    .collect(),
            })
            .collect();
        let active_scene_uid = self.desktop.active_scene().map(Uid::get);
        let structure_key = self.molecule_structure_key_from_objects(active_scene_uid, &objects);
        let sequence_key = format!("{structure_key}|{}", self.selection);
        AppSnapshot {
            scenes,
            views,
            cameras,
            styles,
            objects,
            structure_key,
            sequence_key,
            crystals,
            density_maps,
            surfaces,
            lw_objects,
            active_scene_uid,
            selection: self.selection.clone(),
            measure_result: self.desktop.active_measure(&self.selection),
            perspective: self.camera.is_perspective(),
            background: self.background.clone(),
            show_unit_cell: self.show_unit_cell,
            show_symmetry_mates: self.show_symmetry_mates,
            show_density_contour: self.show_density_contour,
            color_proofing: self.color_proofing,
            density_level: self.density_level,
            animation_objects,
            animation_elapsed: self.animation_elapsed,
            animation_duration: animation_seconds(animation.length()),
            animation_playing: self.animation_playing,
            animation_loop: animation.loop_enabled(),
            animation_start_camera: animation.startcam().to_string(),
            morphs,
            trajectories,
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            undo_descriptions: self
                .undo_stack
                .iter()
                .rev()
                .map(|entry| entry.description.clone())
                .collect(),
            redo_descriptions: self
                .redo_stack
                .iter()
                .rev()
                .map(|entry| entry.description.clone())
                .collect(),
            opened_paths: self.opened_paths.clone(),
            material_names: self
                .desktop
                .material_names(self.desktop.active_scene().map_or(0, Uid::get)),
            log_lines: self.log_lines.clone(),
            status: self.status.clone(),
        }
    }

    fn molecule_structure_key(&self) -> String {
        let object_key = self
            .desktop
            .active_object_summaries()
            .into_iter()
            .map(|object| {
                format!(
                    "{}:{}:{}:{}:{}",
                    object.uid.get(),
                    object.name,
                    object.atom_count,
                    object.residue_count,
                    object.bond_count
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "{}|{}|{object_key}",
            self.structure_revision,
            self.desktop.active_scene().map_or(0, Uid::get)
        )
    }

    fn molecule_structure_key_from_objects(
        &self,
        active_scene_uid: Option<u64>,
        objects: &[ObjectDto],
    ) -> String {
        let object_key = objects
            .iter()
            .filter(|object| Some(object.scene_uid) == active_scene_uid)
            .map(|object| {
                format!(
                    "{}:{}:{}:{}:{}",
                    object.uid,
                    object.name,
                    object.atom_count,
                    object.residue_count,
                    object.bond_count
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "{}|{}|{object_key}",
            self.structure_revision,
            active_scene_uid.unwrap_or(0)
        )
    }

    fn touch_structure_details(&mut self) {
        self.structure_revision = self.structure_revision.wrapping_add(1).max(1);
    }

    fn molecule_details(
        &self,
        known_structure_key: &str,
        known_structure_object_uid: Option<u64>,
        known_sequence_key: &str,
        include_sequence: bool,
        object_uid: Option<u64>,
    ) -> MoleculeDetailsDto {
        let structure_key = self.molecule_structure_key();
        let structure_changed = known_structure_key != structure_key;
        let object_changed = object_uid.is_some() && known_structure_object_uid != object_uid;
        let structure = (structure_changed || object_changed).then(|| {
            if let Some(object_uid) = object_uid {
                self.desktop
                    .active_structure_tree_for_object(Uid::new(object_uid))
                    .into_iter()
                    .map(struct_object_dto)
                    .collect()
            } else {
                self.desktop
                    .active_structure_tree()
                    .into_iter()
                    .map(struct_object_dto)
                    .collect()
            }
        });
        let sequence_key = format!("{structure_key}|{}", self.selection);
        let sequence = (include_sequence && known_sequence_key != sequence_key).then(|| {
            self.desktop
                .active_sequence_view(&self.selection)
                .into_iter()
                .map(|chain| SequenceChainDto {
                    object_uid: chain.object_uid.get(),
                    object_name: chain.object_name,
                    chain_id: chain.chain_id,
                    residues: chain
                        .residues
                        .into_iter()
                        .map(|residue| SequenceResidueDto {
                            letter: residue.letter,
                            res_id: residue.res_id,
                            res_name: residue.res_name,
                            selection: residue.selection,
                            selected: residue.selected,
                        })
                        .collect(),
                })
                .collect()
        });
        MoleculeDetailsDto {
            sequence_key,
            structure_key,
            structure_object_uid: object_uid,
            structure,
            sequence,
        }
    }

    fn application_info(&self) -> ApplicationInfoDto {
        let mut features = Vec::new();
        if cfg!(feature = "tauri-gui") {
            features.push("Tauri GUI");
        }
        if cfg!(feature = "egui-gui") {
            features.push("egui GUI");
        }
        features.push("wgpu offscreen renderer");
        features.push("JavaScript scripting");
        let plugins = self
            .desktop
            .stream_manager()
            .handlers()
            .iter()
            .map(|handler| PluginDto {
                name: handler.nickname.clone(),
                description: handler.descr.clone(),
                extensions: handler.fext.clone(),
                category: match handler.category.code() {
                    0 => "Object reader",
                    1 => "Object writer",
                    2 => "Renderer exporter",
                    3 => "Scene reader",
                    4 => "Scene writer",
                    _ => "Unknown",
                }
                .to_string(),
            })
            .collect();
        let config = self
            .desktop
            .config_entries()
            .into_iter()
            .map(|entry| ConfigEntryDto {
                key: entry.key,
                value: entry.value,
            })
            .collect();
        ApplicationInfoDto {
            product_name: "CueMol2",
            version: env!("CARGO_PKG_VERSION"),
            target: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            features,
            plugins,
            config,
            status: self.status.clone(),
        }
    }

    fn preferences(&self) -> PreferencesDto {
        let font = self.desktop.style_value(0, "", "DefaultLabel.font_name");
        let size = self.desktop.style_value(0, "", "DefaultLabel.font_size");
        let color = self.desktop.style_value(0, "", "DefaultLabel.color");
        let font_style = self.desktop.style_value(0, "", "DefaultLabel.font_style");
        let font_weight = self.desktop.style_value(0, "", "DefaultLabel.font_weight");
        let input = self.desktop.view_input_config();
        PreferencesDto {
            atom_label_font: nonempty_or(font, "san-serif"),
            atom_label_size: size.parse().unwrap_or(12.0),
            atom_label_color: nonempty_or(color, "Yellow"),
            atom_label_italic: matches!(font_style.as_str(), "italic" | "oblique"),
            atom_label_bold: font_weight == "bold",
            ui_language: nonempty_or(
                self.desktop.config_value("general.useragent.locale"),
                "en-US",
            ),
            use_hidpi: config_bool(&self.desktop, "cuemol2.ui.view.use_hidpi", false),
            mouse_preset: nonempty_or(
                self.desktop.config_value("cuemol2.ui.viewinconf"),
                "DefaultViewInConf",
            ),
            trackball_radius: input.tbrad(),
            hit_precision: input.hitprec(),
            momentum_scroll: config_bool(&self.desktop, "cuemol2.ui.mouse-momentum-scroll", false),
            multitouch_pad: config_bool(&self.desktop, "cuemol2.ui.mouse-multitouch-pad", false),
            emulate_right_button: config_bool(
                &self.desktop,
                "cuemol2.ui.mouse-emulate-rbutton",
                false,
            ),
            shortcuts: ["view-trans-plusx", "view-trans-minusx"]
                .into_iter()
                .map(|id| ShortcutDto {
                    id: id.to_string(),
                    key: self.desktop.config_value(&format!("shortcut:{id}:key")),
                    ctrl: config_bool(&self.desktop, &format!("shortcut:{id}:ctrl"), true),
                    alt: config_bool(&self.desktop, &format!("shortcut:{id}:alt"), false),
                    shift: config_bool(&self.desktop, &format!("shortcut:{id}:shift"), false),
                })
                .collect(),
        }
    }

    fn set_preferences(&mut self, preferences: PreferencesDto) -> Result<(), String> {
        if preferences.atom_label_font.trim().is_empty() {
            return Err("an atom-label font is required".to_string());
        }
        if !preferences.atom_label_size.is_finite()
            || !(4.0..=144.0).contains(&preferences.atom_label_size)
        {
            return Err("atom-label size must be between 4 and 144".to_string());
        }
        if preferences.atom_label_color.trim().is_empty() {
            return Err("an atom-label color is required".to_string());
        }
        if !matches!(preferences.ui_language.as_str(), "en-US" | "ja") {
            return Err(format!(
                "unsupported UI language: {}",
                preferences.ui_language
            ));
        }
        if !matches!(
            preferences.mouse_preset.as_str(),
            "DefaultViewInConf"
                | "CootViewInConf"
                | "PymolViewInConf"
                | "IphoneViewInConf"
                | "TrackpadViewInConf"
        ) {
            return Err(format!(
                "unknown mouse preset: {}",
                preferences.mouse_preset
            ));
        }
        if !preferences.trackball_radius.is_finite()
            || !(0.1..=10.0).contains(&preferences.trackball_radius)
        {
            return Err("trackball radius must be between 0.1 and 10".to_string());
        }
        if !preferences.hit_precision.is_finite()
            || !(0.0..=100.0).contains(&preferences.hit_precision)
        {
            return Err("pick precision must be between 0 and 100".to_string());
        }
        validate_shortcuts(&preferences.shortcuts)?;

        self.desktop.set_style_value(
            0,
            "user",
            "DefaultLabel.font_name",
            preferences.atom_label_font.trim(),
        );
        self.desktop.set_style_value(
            0,
            "user",
            "DefaultLabel.font_size",
            preferences.atom_label_size.to_string(),
        );
        self.desktop.set_style_value(
            0,
            "user",
            "DefaultLabel.color",
            preferences.atom_label_color.trim(),
        );
        self.desktop.set_style_value(
            0,
            "user",
            "DefaultLabel.font_style",
            if preferences.atom_label_italic {
                "italic"
            } else {
                "normal"
            },
        );
        self.desktop.set_style_value(
            0,
            "user",
            "DefaultLabel.font_weight",
            if preferences.atom_label_bold {
                "bold"
            } else {
                "normal"
            },
        );
        {
            let input = self.desktop.view_input_config_mut();
            input.set_style(preferences.mouse_preset.clone());
            input.set_tbrad(preferences.trackball_radius);
            input.set_hitprec(preferences.hit_precision);
        }
        for (key, value) in [
            (
                "style:DefaultLabel.font_name",
                preferences.atom_label_font.clone(),
            ),
            (
                "style:DefaultLabel.font_size",
                preferences.atom_label_size.to_string(),
            ),
            (
                "style:DefaultLabel.color",
                preferences.atom_label_color.clone(),
            ),
            (
                "style:DefaultLabel.font_style",
                if preferences.atom_label_italic {
                    "italic".to_string()
                } else {
                    "normal".to_string()
                },
            ),
            (
                "style:DefaultLabel.font_weight",
                if preferences.atom_label_bold {
                    "bold".to_string()
                } else {
                    "normal".to_string()
                },
            ),
            ("general.useragent.locale", preferences.ui_language.clone()),
            (
                "cuemol2.ui.view.use_hidpi",
                preferences.use_hidpi.to_string(),
            ),
            ("cuemol2.ui.viewinconf", preferences.mouse_preset.clone()),
            ("view_input:tbrad", preferences.trackball_radius.to_string()),
            ("view_input:hitprec", preferences.hit_precision.to_string()),
            (
                "cuemol2.ui.mouse-momentum-scroll",
                preferences.momentum_scroll.to_string(),
            ),
            (
                "cuemol2.ui.mouse-multitouch-pad",
                preferences.multitouch_pad.to_string(),
            ),
            (
                "cuemol2.ui.mouse-emulate-rbutton",
                preferences.emulate_right_button.to_string(),
            ),
        ] {
            self.desktop.put_config_value(key, value);
        }
        for shortcut in &preferences.shortcuts {
            self.desktop.put_config_value(
                &format!("shortcut:{}:key", shortcut.id),
                shortcut.key.clone(),
            );
            self.desktop.put_config_value(
                &format!("shortcut:{}:ctrl", shortcut.id),
                shortcut.ctrl.to_string(),
            );
            self.desktop.put_config_value(
                &format!("shortcut:{}:alt", shortcut.id),
                shortcut.alt.to_string(),
            );
            self.desktop.put_config_value(
                &format!("shortcut:{}:shift", shortcut.id),
                shortcut.shift.to_string(),
            );
        }
        self.save_config()?;
        self.status = "Options updated".to_string();
        Ok(())
    }

    fn set_object_properties(&mut self, properties: ObjectPropertiesInput) -> Result<(), String> {
        let name = properties.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err("object name must be non-empty and cannot contain ':'".to_string());
        }
        self.desktop
            .set_object_common_properties(
                Uid::new(properties.object_uid),
                name,
                properties.visible,
                properties.locked,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Object properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_density_map_properties(
        &mut self,
        properties: ObjectPropertiesInput,
    ) -> Result<(), String> {
        let name = properties.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err("density map name must be non-empty and cannot contain ':'".to_string());
        }
        self.desktop
            .set_density_map_common_properties(
                Uid::new(properties.object_uid),
                name,
                properties.visible,
                properties.locked,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Density map properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_density_renderer_visible(
        &mut self,
        map_uid: u64,
        renderer_uid: u64,
        visible: bool,
    ) -> Result<(), String> {
        self.desktop
            .set_density_renderer_visible(Uid::new(map_uid), Uid::new(renderer_uid), visible)
            .map_err(|error| error.to_string())?;
        self.status = "Density renderer visibility updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn add_density_renderer(
        &mut self,
        map_uid: u64,
        renderer_type: &str,
        group_name: Option<&str>,
    ) -> Result<(), String> {
        let renderer_uid = self
            .desktop
            .add_density_renderer(Uid::new(map_uid), renderer_type)
            .map_err(|error| error.to_string())?;
        self.desktop
            .set_workspace_renderer_group_name(Uid::new(map_uid), renderer_uid, group_name)
            .map_err(|error| error.to_string())?;
        self.status = format!("Added {renderer_type} renderer");
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_density_renderer_properties(
        &mut self,
        properties: RendererPropertiesInput,
    ) -> Result<(), String> {
        let name = properties.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err(
                "density renderer name must be non-empty and cannot contain ':'".to_string(),
            );
        }
        if !properties.alpha.is_finite() || properties.alpha <= 0.0 || properties.alpha > 1.0 {
            return Err("opacity must be between 0 and 1".to_string());
        }
        if !properties.edge_width.is_finite() || !(0.0..=1.0).contains(&properties.edge_width) {
            return Err("edge width must be between 0 and 1".to_string());
        }
        if !matches!(
            properties.edge_type.as_str(),
            "none" | "edges" | "silhouette"
        ) {
            return Err(format!("unsupported edge type: {}", properties.edge_type));
        }
        if cuemol_gfx::parse_color(properties.edge_color.trim()).is_err() {
            return Err(format!("invalid edge color: {}", properties.edge_color));
        }
        if self.set_workspace_group_properties(
            properties.object_uid,
            properties.renderer_uid,
            name,
            properties.visible,
            properties.locked,
            properties
                .specific
                .get("style")
                .map(String::as_str)
                .unwrap_or_default(),
            &properties.material,
            properties.alpha,
            &properties.edge_type,
            properties.edge_width,
            properties.edge_color.trim(),
        )? {
            return Ok(());
        }
        for (key, minimum, maximum) in [
            ("siglevel", -1_000_000.0, 1_000_000.0),
            ("extent", f64::EPSILON, 10_000.0),
            ("width", f64::EPSILON, 10.0),
            ("bndry_rng", f64::EPSILON, 100.0),
        ] {
            let Some(raw) = properties.specific.get(key) else {
                continue;
            };
            let value = raw
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{key} must be a number"))?;
            if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                return Err(format!("{key} must be between {minimum} and {maximum}"));
            }
        }
        for (key, minimum, maximum) in [
            ("bufsize", 50, 200),
            ("binning", 1, 16),
            ("max_grids", 10, 2_000),
        ] {
            let Some(raw) = properties.specific.get(key) else {
                continue;
            };
            let value = raw
                .trim()
                .parse::<i32>()
                .map_err(|_| format!("{key} must be an integer"))?;
            if !(minimum..=maximum).contains(&value) {
                return Err(format!("{key} must be between {minimum} and {maximum}"));
            }
        }
        for key in ["use_pbc", "autoupdate", "dragupdate", "cullface"] {
            if properties
                .specific
                .get(key)
                .is_some_and(|value| !matches!(value.as_str(), "true" | "false"))
            {
                return Err(format!("{key} must be true or false"));
            }
        }
        if properties
            .specific
            .get("drawmode")
            .is_some_and(|value| !matches!(value.as_str(), "fill" | "line" | "point"))
        {
            return Err("drawmode must be fill, line, or point".to_string());
        }
        if properties
            .specific
            .get("colormode")
            .is_some_and(|value| !matches!(value.as_str(), "solid" | "molecule" | "multigrad"))
        {
            return Err("colormode must be solid, molecule, or multigrad".to_string());
        }
        if let Some(color) = properties.specific.get("color") {
            if cuemol_gfx::parse_color(color.trim()).is_err() {
                return Err(format!("invalid density color: {color}"));
            }
        }
        self.desktop
            .set_density_renderer_properties(
                Uid::new(properties.object_uid),
                Uid::new(properties.renderer_uid),
                name,
                properties.visible,
                properties.locked,
                properties.material,
                properties.alpha,
                properties.edge_type,
                properties.edge_width,
                properties.edge_color.trim(),
                &properties.specific,
            )
            .map_err(|error| error.to_string())?;
        if let Some(styles) = properties.specific.get("style") {
            self.desktop
                .set_density_renderer_styles(
                    Uid::new(properties.object_uid),
                    Uid::new(properties.renderer_uid),
                    styles,
                )
                .map_err(|error| error.to_string())?;
        }
        self.status = "Density renderer properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_surface_properties(&mut self, properties: ObjectPropertiesInput) -> Result<(), String> {
        let name = properties.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err("surface name must be non-empty and cannot contain ':'".to_string());
        }
        self.desktop
            .set_surface_common_properties(
                Uid::new(properties.object_uid),
                name,
                properties.visible,
                properties.locked,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Surface properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_lw_object_properties(
        &mut self,
        properties: ObjectPropertiesInput,
    ) -> Result<(), String> {
        let name = properties.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err(
                "lightweight object name must be non-empty and cannot contain ':'".to_string(),
            );
        }
        self.desktop
            .set_lw_object_common_properties(
                Uid::new(properties.object_uid),
                name,
                properties.visible,
                properties.locked,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Lightweight object properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_lw_renderer_properties(
        &mut self,
        properties: LwRendererPropertiesInput,
    ) -> Result<(), String> {
        let name = properties.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err(
                "lightweight renderer name must be non-empty and cannot contain ':'".to_string(),
            );
        }
        let styles = properties
            .specific
            .get("style")
            .map(String::as_str)
            .unwrap_or_default();
        if self.set_workspace_group_properties(
            properties.object_uid,
            properties.renderer_uid,
            name,
            properties.visible,
            properties.locked,
            styles,
            &properties.material,
            properties.alpha,
            &properties.edge_type,
            properties.edge_width,
            &properties.edge_color,
        )? {
            return Ok(());
        }
        self.desktop
            .set_lw_renderer_properties(
                Uid::new(properties.object_uid),
                Uid::new(properties.renderer_uid),
                name,
                properties.visible,
                properties.locked,
                styles,
                properties.material,
                properties.alpha,
                properties.edge_type,
                properties.edge_width,
                properties.edge_color,
                properties.data_id,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Lightweight renderer properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_surface_renderer_visible(
        &mut self,
        surface_uid: u64,
        renderer_uid: u64,
        visible: bool,
    ) -> Result<(), String> {
        self.desktop
            .set_surface_renderer_visible(Uid::new(surface_uid), Uid::new(renderer_uid), visible)
            .map_err(|error| error.to_string())?;
        self.status = "Surface renderer visibility updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn add_surface_renderer(
        &mut self,
        surface_uid: u64,
        group_name: Option<&str>,
    ) -> Result<(), String> {
        let renderer_uid = self
            .desktop
            .add_surface_renderer(Uid::new(surface_uid))
            .map_err(|error| error.to_string())?;
        self.desktop
            .set_workspace_renderer_group_name(Uid::new(surface_uid), renderer_uid, group_name)
            .map_err(|error| error.to_string())?;
        self.status = "Added molsurf renderer".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn add_lw_renderer(&mut self, object_uid: u64, group_name: Option<&str>) -> Result<(), String> {
        let renderer_uid = self
            .desktop
            .add_lw_renderer(Uid::new(object_uid))
            .map_err(|error| error.to_string())?;
        self.desktop
            .set_workspace_renderer_group_name(Uid::new(object_uid), renderer_uid, group_name)
            .map_err(|error| error.to_string())?;
        self.status = "Added lightweight renderer".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_workspace_visible(
        &mut self,
        target: WorkspaceToggleInput,
        visible: bool,
    ) -> Result<(), String> {
        let object_uid = Uid::new(target.object_uid);
        let renderer_uid = || {
            target
                .renderer_uid
                .map(Uid::new)
                .ok_or_else(|| "renderer UID is required".to_string())
        };
        match target.kind.as_str() {
            "object" => self.desktop.set_object_visible(object_uid, visible),
            "renderer" => self
                .desktop
                .set_renderer_visible(object_uid, renderer_uid()?, visible),
            "rendererGroup" => {
                self.desktop
                    .set_renderer_group_visible(object_uid, renderer_uid()?, visible)
            }
            "densityMap" => self.desktop.set_density_map_visible(object_uid, visible),
            "densityRenderer" => {
                self.desktop
                    .set_density_renderer_visible(object_uid, renderer_uid()?, visible)
            }
            "surface" => self.desktop.set_surface_visible(object_uid, visible),
            "surfaceRenderer" => {
                self.desktop
                    .set_surface_renderer_visible(object_uid, renderer_uid()?, visible)
            }
            "lwObject" => self.desktop.set_lw_object_visible(object_uid, visible),
            "lwRenderer" => {
                self.desktop
                    .set_lw_renderer_visible(object_uid, renderer_uid()?, visible)
            }
            "camera" | "style" => Ok(()),
            kind => return Err(format!("unsupported Workspace item type: {kind}")),
        }
        .map_err(|error| error.to_string())?;
        self.status = if visible {
            "Workspace item shown".to_string()
        } else {
            "Workspace item hidden".to_string()
        };
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_workspace_items_visible(
        &mut self,
        targets: Vec<WorkspaceToggleInput>,
        visible: bool,
    ) -> Result<(), String> {
        if targets.is_empty() {
            return Err("at least one Workspace item is required".to_string());
        }
        let count = targets.len();
        for target in targets {
            if target.kind == "rendererGroup" {
                continue;
            }
            self.set_workspace_visible(target, visible)?;
        }
        self.status = if visible {
            format!("Show {count} Workspace items")
        } else {
            format!("Hide {count} Workspace items")
        };
        Ok(())
    }

    fn set_workspace_locked(
        &mut self,
        target: WorkspaceToggleInput,
        locked: bool,
    ) -> Result<(), String> {
        let object_uid = Uid::new(target.object_uid);
        let renderer_uid = || {
            target
                .renderer_uid
                .map(Uid::new)
                .ok_or_else(|| "renderer UID is required".to_string())
        };
        match target.kind.as_str() {
            "object" => self.desktop.set_object_locked(object_uid, locked),
            "renderer" => self
                .desktop
                .set_renderer_locked(object_uid, renderer_uid()?, locked),
            "densityMap" => self.desktop.set_density_map_locked(object_uid, locked),
            "densityRenderer" => {
                self.desktop
                    .set_density_renderer_locked(object_uid, renderer_uid()?, locked)
            }
            "surface" => self.desktop.set_surface_locked(object_uid, locked),
            "surfaceRenderer" => {
                self.desktop
                    .set_surface_renderer_locked(object_uid, renderer_uid()?, locked)
            }
            "lwObject" => self.desktop.set_lw_object_locked(object_uid, locked),
            "lwRenderer" => {
                self.desktop
                    .set_lw_renderer_locked(object_uid, renderer_uid()?, locked)
            }
            "rendererGroup" => return Err("renderer group locking is not supported".to_string()),
            kind => return Err(format!("unsupported Workspace item type: {kind}")),
        }
        .map_err(|error| error.to_string())?;
        self.status = if locked {
            "Workspace item locked".to_string()
        } else {
            "Workspace item unlocked".to_string()
        };
        Ok(())
    }

    fn delete_workspace_item(&mut self, target: WorkspaceDeleteInput) -> Result<(), String> {
        let object_uid = Uid::new(target.object_uid);
        let auxiliary_group_target = target.kind == "rendererGroup"
            && self.desktop.scene_manager().object(object_uid).is_none();
        if auxiliary_group_target
            || matches!(
                target.kind.as_str(),
                "densityRenderer" | "surfaceRenderer" | "lwRenderer"
            )
        {
            if let Some(renderer_uid) = target.renderer_uid.map(Uid::new) {
                if self.workspace_renderer_is_group(object_uid, renderer_uid) {
                    let (_, group_name, _, _) = self
                        .desktop
                        .workspace_renderer_style_state(object_uid, renderer_uid)
                        .map_err(|error| error.to_string())?;
                    if !self
                        .desktop
                        .destroy_renderer_group_in_object(object_uid, renderer_uid)
                        .map_err(|error| error.to_string())?
                    {
                        return Err(format!(
                            "renderer group {} could not be deleted",
                            renderer_uid.get()
                        ));
                    }
                    self.status = format!("Delete renderer group: {group_name}");
                    self.invalidate_mesh(false);
                    return Ok(());
                }
            }
        }
        match target.kind.as_str() {
            "object" => {
                let name = self
                    .desktop
                    .scene_manager()
                    .object(object_uid)
                    .map(|object| object.name().to_string())
                    .ok_or_else(|| format!("object {} not found", target.object_uid))?;
                if !self
                    .desktop
                    .destroy_object(object_uid)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!("object {} could not be deleted", target.object_uid));
                }
                self.status = format!("Destroy object {name}");
            }
            "renderer" | "rendererGroup" => {
                let renderer_uid = Uid::new(
                    target
                        .renderer_uid
                        .ok_or_else(|| "renderer UID is required".to_string())?,
                );
                let object = self
                    .desktop
                    .scene_manager()
                    .object(object_uid)
                    .ok_or_else(|| format!("object {} not found", target.object_uid))?;
                let object_name = object.name().to_string();
                let renderer_name = object
                    .renderer(renderer_uid)
                    .map(|renderer| renderer.name().to_string())
                    .ok_or_else(|| format!("renderer {} not found", renderer_uid.get()))?;
                let is_group = object
                    .renderer(renderer_uid)
                    .is_some_and(|renderer| renderer.renderer_type() == "*group");
                let deleted = if is_group {
                    self.desktop
                        .destroy_renderer_group_in_object(object_uid, renderer_uid)
                } else {
                    self.desktop
                        .destroy_renderer_in_object(object_uid, renderer_uid)
                }
                .map_err(|error| error.to_string())?;
                if !deleted {
                    return Err(format!(
                        "renderer {} could not be deleted",
                        renderer_uid.get()
                    ));
                }
                self.status = format!("Delete renderer: {object_name}/{renderer_name}");
            }
            "densityMap" => {
                let name = self
                    .desktop
                    .density_map_summary(object_uid)
                    .map(|map| map.name)
                    .ok_or_else(|| format!("density map {} not found", target.object_uid))?;
                if !self
                    .desktop
                    .destroy_density_map(object_uid)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!(
                        "density map {} could not be deleted",
                        target.object_uid
                    ));
                }
                self.status = format!("Destroy object {name}");
            }
            "densityRenderer" => {
                let renderer_uid = Uid::new(
                    target
                        .renderer_uid
                        .ok_or_else(|| "renderer UID is required".to_string())?,
                );
                let map = self
                    .desktop
                    .density_map_summary(object_uid)
                    .ok_or_else(|| format!("density map {} not found", target.object_uid))?;
                let renderer_name = map
                    .renderers
                    .iter()
                    .find(|renderer| renderer.uid == renderer_uid)
                    .map(|renderer| renderer.name.clone())
                    .ok_or_else(|| format!("density renderer {} not found", renderer_uid.get()))?;
                if !self
                    .desktop
                    .destroy_density_renderer(object_uid, renderer_uid)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!(
                        "density renderer {} could not be deleted",
                        renderer_uid.get()
                    ));
                }
                self.status = format!("Delete renderer: {}/{renderer_name}", map.name);
            }
            "surface" => {
                let name = self
                    .desktop
                    .surface_summary(object_uid)
                    .map(|surface| surface.name)
                    .ok_or_else(|| format!("surface {} not found", target.object_uid))?;
                if !self
                    .desktop
                    .destroy_surface(object_uid)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!(
                        "surface {} could not be deleted",
                        target.object_uid
                    ));
                }
                self.status = format!("Destroy object {name}");
            }
            "surfaceRenderer" => {
                let renderer_uid = Uid::new(
                    target
                        .renderer_uid
                        .ok_or_else(|| "renderer UID is required".to_string())?,
                );
                let surface = self
                    .desktop
                    .surface_summary(object_uid)
                    .ok_or_else(|| format!("surface {} not found", target.object_uid))?;
                let renderer_name = surface
                    .renderers
                    .iter()
                    .find(|renderer| renderer.uid == renderer_uid)
                    .map(|renderer| renderer.name.clone())
                    .ok_or_else(|| format!("surface renderer {} not found", renderer_uid.get()))?;
                if !self
                    .desktop
                    .destroy_surface_renderer(object_uid, renderer_uid)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!(
                        "surface renderer {} could not be deleted",
                        renderer_uid.get()
                    ));
                }
                self.status = format!("Delete renderer: {}/{renderer_name}", surface.name);
            }
            "lwObject" => {
                let name = self
                    .desktop
                    .lw_object_summary(object_uid)
                    .map(|object| object.name)
                    .ok_or_else(|| format!("lightweight object {} not found", target.object_uid))?;
                if !self
                    .desktop
                    .destroy_lw_object(object_uid)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!(
                        "lightweight object {} could not be deleted",
                        target.object_uid
                    ));
                }
                self.status = format!("Destroy object {name}");
            }
            "lwRenderer" => {
                let renderer_uid = Uid::new(
                    target
                        .renderer_uid
                        .ok_or_else(|| "renderer UID is required".to_string())?,
                );
                let object = self
                    .desktop
                    .lw_object_summary(object_uid)
                    .ok_or_else(|| format!("lightweight object {} not found", target.object_uid))?;
                let renderer_name = object
                    .renderers
                    .iter()
                    .find(|renderer| renderer.uid == renderer_uid)
                    .map(|renderer| renderer.name.clone())
                    .ok_or_else(|| {
                        format!("lightweight renderer {} not found", renderer_uid.get())
                    })?;
                if !self
                    .desktop
                    .destroy_lw_renderer(object_uid, renderer_uid)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!(
                        "lightweight renderer {} could not be deleted",
                        renderer_uid.get()
                    ));
                }
                self.status = format!("Delete renderer: {}/{renderer_name}", object.name);
            }
            "camera" => {
                let scene_uid = Uid::new(
                    target
                        .scene_uid
                        .ok_or_else(|| "scene UID is required".to_string())?,
                );
                let name = target
                    .name
                    .as_deref()
                    .ok_or_else(|| "camera name is required".to_string())?;
                if !self
                    .desktop
                    .destroy_scene_camera(scene_uid, name)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!("camera not found: {name}"));
                }
                self.status = format!("Destroyed camera {name}");
            }
            "style" => {
                let scene_uid = Uid::new(
                    target
                        .scene_uid
                        .ok_or_else(|| "scene UID is required".to_string())?,
                );
                let local_index = target
                    .local_index
                    .ok_or_else(|| "local style index is required".to_string())?;
                if !self
                    .desktop
                    .destroy_scene_style(scene_uid, local_index)
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!("style {local_index} not found"));
                }
                self.status = "Destroyed style".to_string();
                self.invalidate_mesh(false);
            }
            kind => return Err(format!("unsupported Workspace item type: {kind}")),
        }
        self.invalidate_mesh(false);
        Ok(())
    }

    fn delete_workspace_items(
        &mut self,
        mut targets: Vec<WorkspaceDeleteInput>,
    ) -> Result<(), String> {
        if targets.is_empty() {
            return Err("at least one Workspace item is required".to_string());
        }
        let count = targets.len();
        let delete_rank = |target: &WorkspaceDeleteInput| match target.kind.as_str() {
            "style" => 0,
            "camera" => 1,
            "renderer" | "densityRenderer" | "surfaceRenderer" | "lwRenderer" => 2,
            "rendererGroup" => 3,
            "object" | "densityMap" | "surface" | "lwObject" => 4,
            _ => 5,
        };
        targets.sort_by(|left, right| {
            let rank = delete_rank(left).cmp(&delete_rank(right));
            if rank != std::cmp::Ordering::Equal {
                return rank;
            }
            match left.kind.as_str() {
                "style" => left
                    .scene_uid
                    .cmp(&right.scene_uid)
                    .then_with(|| right.local_index.cmp(&left.local_index)),
                _ => std::cmp::Ordering::Equal,
            }
        });
        for target in targets {
            self.delete_workspace_item(target)?;
        }
        self.status = format!("Destroy {count} Workspace items");
        Ok(())
    }

    fn move_workspace_renderer(
        &mut self,
        input: WorkspaceMoveRendererInput,
    ) -> Result<bool, String> {
        let moved = self
            .desktop
            .move_renderer_in_object(
                Uid::new(input.object_uid),
                Uid::new(input.renderer_uid),
                Uid::new(input.target_uid),
                &input.position,
            )
            .map_err(|error| error.to_string())?;
        if moved {
            self.status = "Move renderer".to_string();
        }
        Ok(moved)
    }

    fn move_workspace_renderers(
        &mut self,
        input: WorkspaceMoveRenderersInput,
    ) -> Result<bool, String> {
        let renderer_uids = input
            .renderer_uids
            .into_iter()
            .map(Uid::new)
            .collect::<Vec<_>>();
        let moved = self
            .desktop
            .move_renderers_in_object(
                Uid::new(input.object_uid),
                &renderer_uids,
                Uid::new(input.target_uid),
                &input.position,
            )
            .map_err(|error| error.to_string())?;
        if moved {
            self.status = format!("Move {} renderers", renderer_uids.len());
        }
        Ok(moved)
    }

    fn set_workspace_object_collapsed(
        &mut self,
        input: WorkspaceObjectCollapsedInput,
    ) -> Result<(), String> {
        let object_uid = Uid::new(input.object_uid);
        let scene_uid = self
            .desktop
            .scene_manager()
            .scene_uid_for_object(object_uid)
            .or_else(|| {
                self.desktop
                    .density_map_summary(object_uid)
                    .map(|map| map.scene_uid)
            })
            .or_else(|| {
                self.desktop
                    .scene_manager()
                    .scene_uid_for_surface(object_uid)
            })
            .or_else(|| {
                self.desktop
                    .scene_manager()
                    .scene_uid_for_lw_object(object_uid)
            })
            .ok_or_else(|| format!("object {} not found", object_uid.get()))?;
        self.desktop
            .set_workspace_object_collapsed(object_uid, input.collapsed)
            .map_err(|error| error.to_string())?;
        self.modified_scenes.insert(scene_uid);
        self.status = if input.collapsed {
            "Collapse object".to_string()
        } else {
            "Expand object".to_string()
        };
        Ok(())
    }

    fn set_workspace_renderer_collapsed(
        &mut self,
        input: WorkspaceRendererCollapsedInput,
    ) -> Result<(), String> {
        let object_uid = Uid::new(input.object_uid);
        let renderer_uid = Uid::new(input.renderer_uid);
        let (scene_uid, _, renderer_type, _) = self
            .desktop
            .workspace_renderer_style_state(object_uid, renderer_uid)
            .map_err(|error| error.to_string())?;
        if renderer_type != "*group" {
            return Err("only renderer groups can be collapsed".to_string());
        }
        self.desktop
            .set_renderer_group_collapsed(object_uid, renderer_uid, input.collapsed)
            .map_err(|error| error.to_string())?;
        self.modified_scenes.insert(scene_uid);
        self.status = if input.collapsed {
            "Collapse renderer group".to_string()
        } else {
            "Expand renderer group".to_string()
        };
        Ok(())
    }

    fn move_workspace_object(&mut self, input: WorkspaceMoveObjectInput) -> Result<bool, String> {
        let kind = |value: &str| match value {
            "object" => Ok(crate::GuiWorkspaceObjectKind::Molecule),
            "densityMap" => Ok(crate::GuiWorkspaceObjectKind::DensityMap),
            "surface" => Ok(crate::GuiWorkspaceObjectKind::Surface),
            "lwObject" => Ok(crate::GuiWorkspaceObjectKind::Lightweight),
            _ => Err(format!("unsupported Workspace object type: {value}")),
        };
        let after = match input.position.as_str() {
            "before" => false,
            "after" => true,
            value => return Err(format!("unsupported object drop position: {value}")),
        };
        let moved = self
            .desktop
            .move_workspace_object(
                Uid::new(input.scene_uid),
                kind(&input.source_kind)?,
                Uid::new(input.source_uid),
                kind(&input.target_kind)?,
                Uid::new(input.target_uid),
                after,
            )
            .map_err(|error| error.to_string())?;
        if moved {
            self.status = "Move object".to_string();
        }
        Ok(moved)
    }

    fn set_workspace_renderer_selection(
        &mut self,
        input: WorkspaceRendererSelectionInput,
    ) -> Result<(), String> {
        self.desktop
            .set_renderer_selection_text(
                Uid::new(input.object_uid),
                Uid::new(input.renderer_uid),
                &input.selection,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Change renderer selection".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn create_workspace_renderer(
        &mut self,
        input: WorkspaceNewRendererInput,
    ) -> Result<(), String> {
        let object_uid = Uid::new(input.object_uid);
        let renderer_type = input.renderer_type.trim();
        let renderer_name = input.renderer_name.trim();
        if renderer_name.is_empty() || renderer_name.contains(':') {
            return Err("renderer name must be non-empty and cannot contain ':'".to_string());
        }
        if let Some(group_name) = input.group_name.as_deref() {
            let group_exists = match input.kind.as_str() {
                "renderer" => self
                    .desktop
                    .renderer_summaries(object_uid)
                    .iter()
                    .any(|renderer| {
                        renderer.renderer_type == "*group" && renderer.name == group_name
                    }),
                "densityRenderer" => {
                    self.desktop
                        .density_map_summary(object_uid)
                        .is_some_and(|map| {
                            map.renderer_groups
                                .iter()
                                .any(|group| group.name == group_name)
                        })
                }
                "surfaceRenderer" => {
                    self.desktop
                        .surface_summary(object_uid)
                        .is_some_and(|surface| {
                            surface
                                .renderer_groups
                                .iter()
                                .any(|group| group.name == group_name)
                        })
                }
                "lwRenderer" => self
                    .desktop
                    .lw_object_summary(object_uid)
                    .is_some_and(|object| {
                        object
                            .renderer_groups
                            .iter()
                            .any(|group| group.name == group_name)
                    }),
                _ => false,
            };
            if !group_exists {
                return Err(format!("renderer group not found: {group_name}"));
            }
        }

        let _renderer_uid = match input.kind.as_str() {
            "renderer" => {
                let scene_uid = self
                    .desktop
                    .scene_manager()
                    .scene_uid_for_object(object_uid)
                    .ok_or_else(|| format!("object {} not found", input.object_uid))?;
                let renderer_uid = self
                    .desktop
                    .new_renderer_from_setup(
                        scene_uid,
                        object_uid,
                        renderer_type,
                        renderer_name,
                        input.selection.as_deref(),
                    )
                    .map_err(|error| error.to_string())?;
                if let Some(group_name) = input.group_name.as_deref() {
                    self.desktop
                        .set_renderer_property(object_uid, renderer_uid, "group", group_name)
                        .map_err(|error| error.to_string())?;
                }
                renderer_uid
            }
            "densityRenderer" => {
                let density_centering = input.density_centering.as_deref().unwrap_or("map");
                if !matches!(density_centering, "map" | "view") {
                    return Err(format!(
                        "unsupported density renderer centering: {density_centering}"
                    ));
                }
                let renderer_uid = self
                    .desktop
                    .add_density_renderer(object_uid, renderer_type)
                    .map_err(|error| error.to_string())?;
                self.desktop
                    .rename_density_renderer(object_uid, renderer_uid, renderer_name)
                    .map_err(|error| error.to_string())?;
                self.desktop
                    .set_workspace_renderer_group_name(
                        object_uid,
                        renderer_uid,
                        input.group_name.as_deref(),
                    )
                    .map_err(|error| error.to_string())?;
                if density_centering == "map" {
                    self.desktop
                        .set_density_renderer_center(object_uid, renderer_uid, self.camera.center())
                        .map_err(|error| error.to_string())?;
                }
                renderer_uid
            }
            "surfaceRenderer" => {
                if renderer_type != "molsurf" {
                    return Err(format!("unsupported surface renderer: {renderer_type}"));
                }
                let renderer_uid = self
                    .desktop
                    .add_surface_renderer(object_uid)
                    .map_err(|error| error.to_string())?;
                self.desktop
                    .rename_surface_renderer(object_uid, renderer_uid, renderer_name)
                    .map_err(|error| error.to_string())?;
                self.desktop
                    .set_workspace_renderer_group_name(
                        object_uid,
                        renderer_uid,
                        input.group_name.as_deref(),
                    )
                    .map_err(|error| error.to_string())?;
                renderer_uid
            }
            "lwRenderer" => {
                if renderer_type != "lwrend" {
                    return Err(format!("unsupported lightweight renderer: {renderer_type}"));
                }
                let renderer_uid = self
                    .desktop
                    .add_lw_renderer(object_uid)
                    .map_err(|error| error.to_string())?;
                self.desktop
                    .rename_lw_renderer(object_uid, renderer_uid, renderer_name)
                    .map_err(|error| error.to_string())?;
                self.desktop
                    .set_workspace_renderer_group_name(
                        object_uid,
                        renderer_uid,
                        input.group_name.as_deref(),
                    )
                    .map_err(|error| error.to_string())?;
                renderer_uid
            }
            kind => return Err(format!("unsupported renderer target: {kind}")),
        };
        self.status = format!("Added {renderer_type} renderer: {renderer_name}");
        self.invalidate_mesh(false);
        Ok(())
    }

    fn change_workspace_renderer_type(
        &mut self,
        input: WorkspaceRendererTypeInput,
    ) -> Result<(), String> {
        self.desktop
            .change_renderer_type_in_object(
                Uid::new(input.object_uid),
                Uid::new(input.renderer_uid),
                &input.renderer_type,
            )
            .map_err(|error| error.to_string())?;
        self.status = format!("Change renderer type to {}", input.renderer_type);
        self.invalidate_mesh(false);
        Ok(())
    }

    fn apply_workspace_renderer_style(
        &mut self,
        input: WorkspaceRendererStyleInput,
    ) -> Result<(), String> {
        self.desktop
            .apply_renderer_style_option(
                Uid::new(input.object_uid),
                Uid::new(input.renderer_uid),
                &input.style_name,
                &input.component,
            )
            .map_err(|error| error.to_string())?;
        self.status = if input.component == "coloring" {
            "Change coloring style".to_string()
        } else {
            "Change style".to_string()
        };
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_workspace_renderer_styles(
        &mut self,
        input: WorkspaceRendererStylesInput,
    ) -> Result<(), String> {
        let styles = input
            .style_names
            .iter()
            .map(|style| style.trim())
            .filter(|style| !style.is_empty())
            .collect::<Vec<_>>()
            .join(",");
        self.desktop
            .set_workspace_renderer_styles(
                Uid::new(input.object_uid),
                Uid::new(input.renderer_uid),
                &styles,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Change style".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_workspace_renderer_coloring(
        &mut self,
        input: WorkspaceRendererColoringInput,
    ) -> Result<(), String> {
        self.desktop
            .set_workspace_renderer_coloring_preset(
                Uid::new(input.object_uid),
                Uid::new(input.renderer_uid),
                &input.preset,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Change coloring".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn insert_workspace_renderer_paint(
        &mut self,
        input: WorkspaceRendererPaintInput,
    ) -> Result<(), String> {
        self.desktop
            .insert_renderer_paint_entry(
                Uid::new(input.object_uid),
                Uid::new(input.renderer_uid),
                &input.color,
            )
            .map_err(|error| error.to_string())?;
        self.status = "Insert paint entry".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn insert_workspace_object_paint(
        &mut self,
        input: WorkspaceObjectPaintInput,
    ) -> Result<bool, String> {
        let object_uid = Uid::new(input.object_uid);
        let Some(object) = self.desktop.scene_manager().object(object_uid) else {
            return Err(format!("object {} not found", input.object_uid));
        };
        if object.mol().selection_text().trim().is_empty()
            || object.coloring_type() != Some("PaintColoring")
        {
            return Ok(false);
        }
        self.desktop
            .insert_object_paint_entry(object_uid, &input.color)
            .map_err(|error| error.to_string())?;
        self.status = "Insert paint entry".to_string();
        self.invalidate_mesh(false);
        Ok(true)
    }

    fn copy_workspace_renderer(
        &mut self,
        input: WorkspaceRendererCopyInput,
    ) -> Result<String, String> {
        let clipboard = self
            .desktop
            .copy_workspace_renderer(Uid::new(input.object_uid), Uid::new(input.renderer_uid))
            .map_err(|error| error.to_string())?;
        let count = clipboard.renderer_count();
        self.workspace_renderer_clipboard = Some(clipboard);
        self.status = format!("Copied {count} renderer(s)");
        Ok(self.status.clone())
    }

    fn copy_workspace_renderers(
        &mut self,
        inputs: Vec<WorkspaceRendererCopyInput>,
    ) -> Result<String, String> {
        let targets = inputs
            .into_iter()
            .map(|input| (Uid::new(input.object_uid), Uid::new(input.renderer_uid)))
            .collect::<Vec<_>>();
        let clipboard = self
            .desktop
            .copy_workspace_renderers(&targets)
            .map_err(|error| error.to_string())?;
        let count = clipboard.renderer_count();
        self.workspace_renderer_clipboard = Some(clipboard);
        self.status = format!("Copied {count} renderer(s)");
        Ok(self.status.clone())
    }

    fn copy_workspace_object(&mut self, input: WorkspaceObjectCopyInput) -> Result<String, String> {
        let clipboard = self
            .desktop
            .copy_workspace_object(Uid::new(input.object_uid))
            .map_err(|error| error.to_string())?;
        let name = clipboard.name().to_string();
        self.workspace_object_clipboard = Some(clipboard);
        self.status = format!("Copied object {name}");
        Ok(self.status.clone())
    }

    fn paste_workspace_object(&mut self, input: WorkspaceObjectPasteInput) -> Result<(), String> {
        let clipboard = self
            .workspace_object_clipboard
            .clone()
            .ok_or_else(|| "No copied object".to_string())?;
        let scene_uid = Uid::new(input.scene_uid);
        self.desktop
            .paste_workspace_object(scene_uid, &clipboard)
            .map_err(|error| error.to_string())?;
        self.modified_scenes.insert(scene_uid);
        self.status = "Paste object".to_string();
        self.touch_structure_details();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn copy_workspace_camera(&mut self, input: WorkspaceCameraCopyInput) -> Result<String, String> {
        let clipboard = self
            .desktop
            .copy_workspace_camera(Uid::new(input.scene_uid), &input.name)
            .map_err(|error| error.to_string())?;
        let name = clipboard.name().to_string();
        self.workspace_camera_clipboard = Some(clipboard);
        self.status = format!("Copied camera {name}");
        Ok(self.status.clone())
    }

    fn paste_workspace_camera(&mut self, input: WorkspaceCameraPasteInput) -> Result<(), String> {
        let clipboard = self
            .workspace_camera_clipboard
            .clone()
            .ok_or_else(|| "No copied camera".to_string())?;
        let scene_uid = Uid::new(input.scene_uid);
        let name = self
            .desktop
            .paste_workspace_camera(scene_uid, &clipboard)
            .map_err(|error| error.to_string())?;
        self.modified_scenes.insert(scene_uid);
        self.status = format!("Pasted camera {name}");
        Ok(())
    }

    fn copy_workspace_style(&mut self, input: WorkspaceStyleCopyInput) -> Result<String, String> {
        let clipboard = self
            .desktop
            .copy_workspace_style(Uid::new(input.scene_uid), input.local_index)
            .map_err(|error| error.to_string())?;
        let name = clipboard.name();
        self.workspace_style_clipboard = Some(clipboard);
        self.status = format!("Copied style {name}");
        Ok(self.status.clone())
    }

    fn paste_workspace_style(&mut self, input: WorkspaceStylePasteInput) -> Result<(), String> {
        let clipboard = self
            .workspace_style_clipboard
            .clone()
            .ok_or_else(|| "No copied style".to_string())?;
        let scene_uid = Uid::new(input.scene_uid);
        self.desktop
            .paste_workspace_style(scene_uid, &clipboard, input.replace)
            .map_err(|error| error.to_string())?;
        self.modified_scenes.insert(scene_uid);
        self.status = format!("Pasted style {}", clipboard.name());
        self.invalidate_mesh(false);
        Ok(())
    }

    fn paste_workspace_renderers(
        &mut self,
        input: WorkspaceRendererPasteInput,
    ) -> Result<bool, String> {
        let clipboard = self
            .workspace_renderer_clipboard
            .clone()
            .ok_or_else(|| "No copied renderer".to_string())?;
        let object_uid = Uid::new(input.object_uid);
        let scene_uid = self
            .desktop
            .scene_manager()
            .scene_uid_for_object(object_uid)
            .or_else(|| {
                self.desktop
                    .density_map_summary(object_uid)
                    .map(|map| map.scene_uid)
            })
            .or_else(|| {
                self.desktop
                    .surface_summary(object_uid)
                    .map(|surface| surface.scene_uid)
            })
            .or_else(|| {
                self.desktop
                    .lw_object_summary(object_uid)
                    .map(|object| object.scene_uid)
            });
        let result = self
            .desktop
            .paste_workspace_renderers(
                object_uid,
                input.destination_group_uid.map(Uid::new),
                &clipboard,
            )
            .map_err(|error| error.to_string())?;
        let changed = !result.renderer_uids.is_empty() || result.group_uid.is_some();
        if changed {
            if let Some(scene_uid) = scene_uid {
                self.modified_scenes.insert(scene_uid);
            }
        }
        self.status = format!("Pasted {} renderer(s)", result.renderer_uids.len());
        self.invalidate_mesh(false);
        Ok(changed)
    }

    fn create_workspace_renderer_group(
        &mut self,
        input: WorkspaceRendererGroupInput,
    ) -> Result<(), String> {
        self.desktop
            .create_renderer_group(Uid::new(input.object_uid), input.name)
            .map_err(|error| error.to_string())?;
        self.status = "Create renderer group".to_string();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn set_workspace_group_properties(
        &mut self,
        object_uid: u64,
        renderer_uid: u64,
        name: &str,
        visible: bool,
        locked: bool,
        style: &str,
        material: &str,
        alpha: f64,
        edge_type: &str,
        edge_width: f64,
        edge_color: &str,
    ) -> Result<bool, String> {
        let object_uid = Uid::new(object_uid);
        let renderer_uid = Uid::new(renderer_uid);
        if !self.workspace_renderer_is_group(object_uid, renderer_uid) {
            return Ok(false);
        }
        self.desktop
            .set_renderer_group_common_properties(
                object_uid,
                renderer_uid,
                name,
                visible,
                locked,
                material,
                alpha,
                edge_type,
                edge_width,
                edge_color,
            )
            .map_err(|error| error.to_string())?;
        self.desktop
            .set_workspace_renderer_styles(object_uid, renderer_uid, style)
            .map_err(|error| error.to_string())?;
        self.status = "Renderer group properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(true)
    }

    fn workspace_renderer_is_group(&self, object_uid: Uid, renderer_uid: Uid) -> bool {
        self.desktop
            .workspace_renderer_style_state(object_uid, renderer_uid)
            .is_ok_and(|(_, _, renderer_type, _)| renderer_type == "*group")
    }

    fn rename_workspace_renderer_group(
        &mut self,
        input: WorkspaceRendererGroupInput,
    ) -> Result<(), String> {
        let group_uid = input
            .group_uid
            .ok_or_else(|| "renderer group UID is required".to_string())?;
        self.desktop
            .rename_renderer_group(Uid::new(input.object_uid), Uid::new(group_uid), input.name)
            .map_err(|error| error.to_string())?;
        self.status = "Rename renderer group".to_string();
        Ok(())
    }

    fn rename_workspace_camera(
        &mut self,
        input: WorkspaceCameraRenameInput,
    ) -> Result<bool, String> {
        let new_name = input.new_name.trim();
        if new_name.is_empty() || new_name == input.old_name {
            return Ok(false);
        }
        let scene_uid = Uid::new(input.scene_uid);
        let Some(scene) = self.desktop.scene_manager().scene(scene_uid) else {
            return Ok(false);
        };
        if scene.camera_by_name(new_name).is_some() {
            return Err(format!("camera already exists: {new_name}"));
        }
        let Some(mut camera) = scene.camera_by_name(&input.old_name).cloned() else {
            return Ok(false);
        };
        self.desktop
            .destroy_scene_camera(scene_uid, &input.old_name)
            .map_err(|error| error.to_string())?;
        camera.set_name(new_name);
        self.desktop
            .save_scene_camera(scene_uid, camera, false)
            .map_err(|error| error.to_string())?;
        self.status = format!("Rename to {new_name}");
        Ok(true)
    }

    fn rename_workspace_node(&mut self, input: WorkspaceRenameInput) -> Result<(), String> {
        let name = input.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err("Workspace item name must be non-empty and cannot contain ':'".to_string());
        }
        let object_uid = Uid::new(input.object_uid);
        match input.kind.as_str() {
            "scene" => {
                let scene = self
                    .desktop
                    .scene_manager_mut()
                    .scene_mut(object_uid)
                    .ok_or_else(|| format!("scene {} not found", input.object_uid))?;
                scene.set_name(name);
                self.status = format!("Rename to {name}");
            }
            "object" => {
                let object = self
                    .desktop
                    .scene_manager_mut()
                    .object_mut(object_uid)
                    .ok_or_else(|| format!("object {} not found", input.object_uid))?;
                object.set_name(name);
                self.touch_structure_details();
                self.status = format!("Rename to {name}");
            }
            "renderer" => {
                let renderer_uid = Uid::new(
                    input
                        .renderer_uid
                        .ok_or_else(|| "renderer UID is required".to_string())?,
                );
                let renderer = self
                    .desktop
                    .scene_manager_mut()
                    .object_mut(object_uid)
                    .and_then(|object| object.renderer_mut(renderer_uid))
                    .ok_or_else(|| format!("renderer {} not found", renderer_uid.get()))?;
                if renderer.renderer_type() == "*group" {
                    return Err("renderer groups require the group rename operation".to_string());
                }
                renderer.set_name(name);
                self.status = format!("Rename to {name}");
            }
            "rendererGroup" => {
                let group_uid = input
                    .renderer_uid
                    .ok_or_else(|| "renderer group UID is required".to_string())?;
                self.desktop
                    .rename_renderer_group(object_uid, Uid::new(group_uid), name)
                    .map_err(|error| error.to_string())?;
                self.status = format!("Rename to {name}");
            }
            "densityMap" => {
                self.desktop
                    .rename_density_map(object_uid, name)
                    .map_err(|error| error.to_string())?;
                self.status = format!("Rename to {name}");
            }
            "densityRenderer" => {
                let renderer_uid = input
                    .renderer_uid
                    .ok_or_else(|| "density renderer UID is required".to_string())?;
                let renderer_uid = Uid::new(renderer_uid);
                if self.workspace_renderer_is_group(object_uid, renderer_uid) {
                    self.desktop
                        .rename_renderer_group(object_uid, renderer_uid, name)
                        .map_err(|error| error.to_string())?;
                } else {
                    self.desktop
                        .rename_density_renderer(object_uid, renderer_uid, name)
                        .map_err(|error| error.to_string())?;
                }
                self.status = format!("Rename to {name}");
            }
            "surface" => {
                self.desktop
                    .rename_surface(object_uid, name)
                    .map_err(|error| error.to_string())?;
                self.status = format!("Rename to {name}");
            }
            "surfaceRenderer" => {
                let renderer_uid = input
                    .renderer_uid
                    .ok_or_else(|| "surface renderer UID is required".to_string())?;
                let renderer_uid = Uid::new(renderer_uid);
                if self.workspace_renderer_is_group(object_uid, renderer_uid) {
                    self.desktop
                        .rename_renderer_group(object_uid, renderer_uid, name)
                        .map_err(|error| error.to_string())?;
                } else {
                    self.desktop
                        .rename_surface_renderer(object_uid, renderer_uid, name)
                        .map_err(|error| error.to_string())?;
                }
                self.status = format!("Rename to {name}");
            }
            "lwObject" => {
                self.desktop
                    .rename_lw_object(object_uid, name)
                    .map_err(|error| error.to_string())?;
                self.status = format!("Rename to {name}");
            }
            "lwRenderer" => {
                let renderer_uid = input
                    .renderer_uid
                    .ok_or_else(|| "lightweight renderer UID is required".to_string())?;
                let renderer_uid = Uid::new(renderer_uid);
                if self.workspace_renderer_is_group(object_uid, renderer_uid) {
                    self.desktop
                        .rename_renderer_group(object_uid, renderer_uid, name)
                        .map_err(|error| error.to_string())?;
                } else {
                    self.desktop
                        .rename_lw_renderer(object_uid, renderer_uid, name)
                        .map_err(|error| error.to_string())?;
                }
                self.status = format!("Rename to {name}");
            }
            kind => return Err(format!("unsupported Workspace item type: {kind}")),
        }
        Ok(())
    }

    fn fit_workspace_item(
        &mut self,
        target: WorkspaceFitInput,
        width: u32,
        height: u32,
    ) -> Result<bool, String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        let mut view = View::new(Uid::new(1), scene_uid);
        view.set_camera(&self.camera);
        view.set_view_size(width, height);
        let object_uid = Uid::new(target.object_uid);
        let fitted = match target.kind.as_str() {
            "object" => self
                .desktop
                .fit_molecule_view(object_uid, None, &mut view)
                .map_err(|error| error.to_string())?,
            "renderer" => {
                let renderer_uid = Uid::new(
                    target
                        .renderer_uid
                        .ok_or_else(|| "renderer UID is required".to_string())?,
                );
                self.desktop
                    .fit_molecule_view(object_uid, Some(renderer_uid), &mut view)
                    .map_err(|error| error.to_string())?
            }
            "rendererGroup" => false,
            "densityMap" | "densityRenderer" => {
                self.desktop
                    .fit_density_map_view(object_uid, &mut view)
                    .map_err(|error| error.to_string())?;
                true
            }
            "surface" | "surfaceRenderer" | "lwObject" | "lwRenderer" => false,
            kind => return Err(format!("unsupported Workspace item type: {kind}")),
        };
        if fitted {
            self.camera = view.camera();
            self.status = "View fitted".to_string();
        }
        Ok(fitted)
    }

    fn apply_molecule_selection_action(
        &mut self,
        object_uid: u64,
        selection: &str,
        mode: &str,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if !matches!(mode, "select" | "center" | "zoom") {
            return Err(format!("unsupported MolStruct action: {mode}"));
        }
        let object_uid = Uid::new(object_uid);
        self.desktop
            .set_molecule_selection_text(object_uid, selection)
            .map_err(|error| error.to_string())?;
        self.selection = selection.to_string();

        match mode {
            "select" => {}
            "center" => {
                if let Some(center) = self
                    .desktop
                    .selected_molecule_center(object_uid)
                    .map_err(|error| error.to_string())?
                {
                    self.camera.set_center(center);
                }
            }
            "zoom" => {
                let scene_uid = self
                    .desktop
                    .active_scene()
                    .ok_or_else(|| "no active scene".to_string())?;
                let mut view = View::new(Uid::new(1), scene_uid);
                view.set_camera(&self.camera);
                view.set_view_size(width, height);
                if self
                    .desktop
                    .fit_selected_molecule_view(object_uid, &mut view)
                    .map_err(|error| error.to_string())?
                {
                    self.camera = view.camera();
                }
            }
            _ => unreachable!("MolStruct action was validated"),
        }

        self.status = "Change mol selection".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_surface_renderer_properties(
        &mut self,
        properties: RendererPropertiesInput,
    ) -> Result<(), String> {
        let name = properties.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err(
                "surface renderer name must be non-empty and cannot contain ':'".to_string(),
            );
        }
        if !properties.alpha.is_finite() || properties.alpha <= 0.0 || properties.alpha > 1.0 {
            return Err("opacity must be between 0 and 1".to_string());
        }
        if !properties.edge_width.is_finite() || !(0.0..=1.0).contains(&properties.edge_width) {
            return Err("edge width must be between 0 and 1".to_string());
        }
        if !matches!(
            properties.edge_type.as_str(),
            "none" | "edges" | "silhouette"
        ) {
            return Err(format!("unsupported edge type: {}", properties.edge_type));
        }
        if cuemol_gfx::parse_color(properties.edge_color.trim()).is_err() {
            return Err(format!("invalid edge color: {}", properties.edge_color));
        }
        if self.set_workspace_group_properties(
            properties.object_uid,
            properties.renderer_uid,
            name,
            properties.visible,
            properties.locked,
            properties
                .specific
                .get("style")
                .map(String::as_str)
                .unwrap_or_default(),
            &properties.material,
            properties.alpha,
            &properties.edge_type,
            properties.edge_width,
            properties.edge_color.trim(),
        )? {
            return Ok(());
        }
        if properties
            .specific
            .get("drawmode")
            .is_some_and(|value| !matches!(value.as_str(), "fill" | "line" | "point"))
        {
            return Err("drawmode must be fill, line, or point".to_string());
        }
        if properties.specific.get("colormode").is_some_and(|value| {
            !matches!(
                value.as_str(),
                "solid" | "molecule" | "potential" | "multigrad"
            )
        }) {
            return Err("colormode must be solid, molecule, potential, or multigrad".to_string());
        }
        for key in ["cullface", "ramp_above"] {
            if properties
                .specific
                .get(key)
                .is_some_and(|value| !matches!(value.as_str(), "true" | "false"))
            {
                return Err(format!("{key} must be true or false"));
            }
        }
        for (key, minimum, maximum) in [
            ("width", f64::EPSILON, 10.0),
            ("lowpar", -1_000_000.0, 1_000_000.0),
            ("midpar", -1_000_000.0, 1_000_000.0),
            ("highpar", -1_000_000.0, 1_000_000.0),
            ("ramp_value", 0.0, 1_000.0),
        ] {
            let Some(raw) = properties.specific.get(key) else {
                continue;
            };
            let value = raw
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{key} must be a number"))?;
            if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                return Err(format!("{key} must be between {minimum} and {maximum}"));
            }
        }
        for key in ["defaultcolor", "lowcol", "midcol", "highcol"] {
            if let Some(color) = properties.specific.get(key) {
                if cuemol_gfx::parse_color(color.trim()).is_err() {
                    return Err(format!("invalid surface color: {color}"));
                }
            }
        }
        self.desktop
            .set_surface_renderer_properties(
                Uid::new(properties.object_uid),
                Uid::new(properties.renderer_uid),
                name,
                properties.selection.trim(),
                properties.visible,
                properties.locked,
                properties.material,
                properties.alpha,
                properties.edge_type,
                properties.edge_width,
                properties.edge_color.trim(),
                &properties.specific,
            )
            .map_err(|error| error.to_string())?;
        if let Some(styles) = properties.specific.get("style") {
            self.desktop
                .set_surface_renderer_styles(
                    Uid::new(properties.object_uid),
                    Uid::new(properties.renderer_uid),
                    styles,
                )
                .map_err(|error| error.to_string())?;
        }
        self.status = "Surface renderer properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn set_renderer_properties(
        &mut self,
        properties: RendererPropertiesInput,
    ) -> Result<(), String> {
        let object_uid = Uid::new(properties.object_uid);
        let renderer_uid = Uid::new(properties.renderer_uid);
        let renderer_type = self
            .desktop
            .scene_manager()
            .object(object_uid)
            .and_then(|object| object.renderer(renderer_uid))
            .map(|renderer| renderer.renderer_type().to_string())
            .ok_or_else(|| format!("renderer {} not found", properties.renderer_uid))?;
        let name = properties.name.trim();
        if (name.is_empty() && !matches!(renderer_type.as_str(), "*selection" | "selection"))
            || name.contains(':')
        {
            return Err("renderer name must be non-empty and cannot contain ':'".to_string());
        }
        if !properties.alpha.is_finite() || properties.alpha <= 0.0 || properties.alpha > 1.0 {
            return Err("opacity must be between 0 and 1".to_string());
        }
        if !properties.edge_width.is_finite() || !(0.0..=1.0).contains(&properties.edge_width) {
            return Err("edge width must be between 0 and 1".to_string());
        }
        if !matches!(
            properties.edge_type.as_str(),
            "none" | "edges" | "silhouette"
        ) {
            return Err(format!("unsupported edge type: {}", properties.edge_type));
        }
        if cuemol_gfx::parse_color(properties.edge_color.trim()).is_err() {
            return Err(format!("invalid edge color: {}", properties.edge_color));
        }
        if matches!(renderer_type.as_str(), "simple" | "trace" | "spline")
            && (!properties.line_width.is_finite()
                || properties.line_width <= 0.0
                || properties.line_width > 10.0)
        {
            return Err("line width must be greater than 0 and at most 10".to_string());
        }
        if matches!(renderer_type.as_str(), "ballstick" | "anisou") {
            if !(2..=20).contains(&properties.detail) {
                return Err("detail must be between 2 and 20".to_string());
            }
            for (label, value) in [
                ("bond width", properties.bond_width),
                ("atom radius", properties.sphere_radius),
                ("ring thickness", properties.ring_thickness),
            ] {
                if !value.is_finite() || !(0.0..=10.0).contains(&value) {
                    return Err(format!("{label} must be between 0 and 10"));
                }
            }
            if cuemol_gfx::parse_color(properties.ring_color.trim()).is_err() {
                return Err(format!("invalid ring color: {}", properties.ring_color));
            }
        }
        let mut specific_updates = Vec::<(String, String, bool)>::new();
        match renderer_type.as_str() {
            "*namelabel" | "name_label" => {
                if let Some(raw) = properties.specific.get("maxlabel") {
                    let value = raw
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| "maximum labels must be an integer".to_string())?;
                    if !(0..=1_000_000).contains(&value) {
                        return Err("maximum labels must be between 0 and 1000000".to_string());
                    }
                    specific_updates.push(("maxlabel".to_string(), value.to_string(), value == 50));
                }
                for (key, label) in [
                    ("dispx", "label X displacement"),
                    ("dispy", "label Y displacement"),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{label} must be a number"))?;
                    if !value.is_finite() || !(-10_000.0..=10_000.0).contains(&value) {
                        return Err(format!("invalid {label}: {value}"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        value.abs() < f64::EPSILON,
                    ));
                }
                if let Some(raw) = properties.specific.get("color") {
                    let value = raw.trim();
                    let color = cuemol_gfx::parse_color(value)
                        .map_err(|_| format!("invalid label color: {value}"))?;
                    let is_default = color.rgba8()
                        == Some(cuemol_gfx::Rgba8 {
                            r: 255,
                            g: 255,
                            b: 0,
                            a: 255,
                        });
                    specific_updates.push(("color".to_string(), value.to_string(), is_default));
                }
                if let Some(raw) = properties.specific.get("font_size") {
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "label font size must be a number".to_string())?;
                    if !value.is_finite() || !(1.0..=512.0).contains(&value) {
                        return Err("label font size must be between 1 and 512".to_string());
                    }
                    specific_updates.push((
                        "font_size".to_string(),
                        value.to_string(),
                        (value - 12.0).abs() < f64::EPSILON,
                    ));
                }
                for (key, label, default) in [
                    ("font_name", "label font name", "sans-serif"),
                    ("font_style", "label font style", "normal"),
                    ("font_weight", "label font weight", "normal"),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw.trim();
                    if value.is_empty() || value.len() > 128 {
                        return Err(format!("{label} must contain 1 to 128 characters"));
                    }
                    specific_updates.push((key.to_string(), value.to_string(), value == default));
                }
            }
            "*selection" | "selection" => {
                if let Some(raw) = properties.specific.get("mode") {
                    let value = raw.trim();
                    if !matches!(value, "stick" | "point") {
                        return Err(format!("unsupported selection mode: {value}"));
                    }
                    specific_updates.push((
                        "mode".to_string(),
                        value.to_string(),
                        value == "stick",
                    ));
                }
                for (key, label, default) in [
                    ("linew", "selection line width", 3.0),
                    ("dispx", "selection X displacement", 3.0),
                    ("dispy", "selection Y displacement", 3.0),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{label} must be a number"))?;
                    if !value.is_finite()
                        || (key == "linew" && !(0.0..=100.0).contains(&value))
                        || (key != "linew" && !(-10_000.0..=10_000.0).contains(&value))
                    {
                        return Err(format!("invalid {label}: {value}"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        (value - default).abs() < f64::EPSILON,
                    ));
                }
                if let Some(raw) = properties.specific.get("color") {
                    let value = raw.trim();
                    let color = cuemol_gfx::parse_color(value)
                        .map_err(|_| format!("invalid selection color: {value}"))?;
                    let is_default = color.rgba8()
                        == Some(cuemol_gfx::Rgba8 {
                            r: 0,
                            g: 255,
                            b: 255,
                            a: 128,
                        });
                    specific_updates.push(("color".to_string(), value.to_string(), is_default));
                }
            }
            "spline" => {
                if let Some(raw) = properties.specific.get("axialdetail") {
                    let value = raw
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| "spline axial detail must be an integer".to_string())?;
                    if !(1..=100).contains(&value) {
                        return Err("spline axial detail must be between 1 and 100".to_string());
                    }
                    specific_updates.push((
                        "axialdetail".to_string(),
                        value.to_string(),
                        value == 6,
                    ));
                }
                if let Some(raw) = properties.specific.get("smooth") {
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "spline smoothing must be a number".to_string())?;
                    if !value.is_finite() {
                        return Err("spline smoothing must be finite".to_string());
                    }
                    specific_updates.push((
                        "smooth".to_string(),
                        value.to_string(),
                        value.abs() < f64::EPSILON,
                    ));
                }
                if let Some(raw) = properties.specific.get("smoothcolor") {
                    let value = match raw.trim() {
                        "true" => true,
                        "false" => false,
                        _ => return Err("spline smooth color must be true or false".to_string()),
                    };
                    specific_updates.push(("smoothcolor".to_string(), value.to_string(), value));
                }
                if let Some(raw) = properties.specific.get("pivotatom") {
                    let value = raw.trim();
                    if value.len() > 16 {
                        return Err("spline pivot atom must be at most 16 characters".to_string());
                    }
                    specific_updates.push((
                        "pivotatom".to_string(),
                        value.to_string(),
                        value.is_empty(),
                    ));
                }
            }
            "atomintr" => {
                if let Some(raw) = properties.specific.get("mode") {
                    let value = raw.trim();
                    if !matches!(value, "simple" | "fancy") {
                        return Err(format!("unsupported interaction mode: {value}"));
                    }
                    specific_updates.push((
                        "mode".to_string(),
                        value.to_string(),
                        value == "fancy",
                    ));
                }
                if let Some(raw) = properties.specific.get("showlabel") {
                    let value = config_value_is_true(raw);
                    specific_updates.push(("showlabel".to_string(), value.to_string(), !value));
                }
                if let Some(raw) = properties.specific.get("detail") {
                    let value = raw
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| "interaction detail must be an integer".to_string())?;
                    if !(0..=20).contains(&value) {
                        return Err("interaction detail must be between 0 and 20".to_string());
                    }
                    specific_updates.push(("detail".to_string(), value.to_string(), value == 5));
                }
                if let Some(raw) = properties.specific.get("width") {
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "interaction width must be a number".to_string())?;
                    if !value.is_finite() || !(0.0..=10.0).contains(&value) {
                        return Err("interaction width must be between 0 and 10".to_string());
                    }
                    specific_updates.push((
                        "width".to_string(),
                        value.to_string(),
                        (value - 0.1).abs() < f64::EPSILON,
                    ));
                }
                if let Some(value) = properties.specific.get("color") {
                    if cuemol_gfx::parse_color(value.trim()).is_err() {
                        return Err(format!("invalid interaction color: {value}"));
                    }
                    specific_updates.push((
                        "color".to_string(),
                        value.trim().to_string(),
                        value.trim().eq_ignore_ascii_case("#ffff00"),
                    ));
                }
                for (key, default) in [
                    ("stipple0", 1.0),
                    ("stipple1", 1.0),
                    ("stipple2", -1.0),
                    ("stipple3", -1.0),
                    ("stipple4", -1.0),
                    ("stipple5", -1.0),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{key} must be a number"))?;
                    if !value.is_finite() || !(-1.0..=100.0).contains(&value) {
                        return Err(format!("{key} must be between -1 and 100"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        (value - default).abs() < f64::EPSILON,
                    ));
                }
                for key in ["captype_start", "captype_end"] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw.trim();
                    if !matches!(value, "flat" | "sphere" | "arrow") {
                        return Err(format!("unsupported interaction cap: {value}"));
                    }
                    specific_updates.push((key.to_string(), value.to_string(), value == "sphere"));
                }
            }
            "anisou" => {
                if let Some(raw) = properties.specific.get("drawdisc") {
                    let value = config_value_is_true(raw);
                    specific_updates.push(("drawdisc".to_string(), value.to_string(), value));
                }
                for (key, label, minimum, default) in [
                    ("discscale", "disc scale", 0.0, 1.1),
                    ("discthick", "disc thickness", 0.0, 0.1),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{label} must be a number"))?;
                    if !value.is_finite() || !(minimum..=10.0).contains(&value) {
                        return Err(format!("{label} must be between {minimum} and 10"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        (value - default).abs() < f64::EPSILON,
                    ));
                }
                if let Some(raw) = properties.specific.get("maxverts") {
                    let value = raw
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| "maximum vertices must be an integer".to_string())?;
                    if !(1..=100_000_000).contains(&value) {
                        return Err("maximum vertices must be between 1 and 100000000".to_string());
                    }
                    specific_updates.push((
                        "maxverts".to_string(),
                        value.to_string(),
                        value == 1_000_000,
                    ));
                }
            }
            "cpk" => {
                for (key, label, default) in [
                    ("vdwr_H", "hydrogen radius", 1.2),
                    ("vdwr_C", "carbon radius", 1.7),
                    ("vdwr_N", "nitrogen radius", 1.55),
                    ("vdwr_O", "oxygen radius", 1.52),
                    ("vdwr_S", "sulfur radius", 1.8),
                    ("vdwr_P", "phosphorus radius", 1.8),
                    ("vdwr_X", "other atom radius", 1.7),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{label} must be a number"))?;
                    if !value.is_finite() || !(0.0..=3.0).contains(&value) {
                        return Err(format!("{label} must be between 0 and 3"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        (value - default).abs() < f64::EPSILON,
                    ));
                }
                if let Some(raw) = properties.specific.get("detail") {
                    let value = raw
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| "detail must be an integer".to_string())?;
                    if !(2..=20).contains(&value) {
                        return Err("detail must be between 2 and 20".to_string());
                    }
                    specific_updates.push(("detail".to_string(), value.to_string(), value == 10));
                }
                if let Some(raw) = properties.specific.get("glrender_mode") {
                    let value = raw.trim();
                    if !matches!(value, "default" | "shader" | "vbo" | "glu") {
                        return Err(format!("unsupported CPK rendering mode: {value}"));
                    }
                    specific_updates.push((
                        "glrender_mode".to_string(),
                        value.to_string(),
                        value == "default",
                    ));
                }
            }
            "disorder" => {
                for (key, label, minimum, default) in [
                    ("width", "dot size", 0.0, 0.3),
                    ("dotsep", "dot separation", 0.0, 1.0),
                    ("loopsize", "loop size", 0.0, 2.0),
                    ("loopsize2", "loop size 2", -1.0, -1.0),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{label} must be a number"))?;
                    if !value.is_finite() || !(minimum..=10.0).contains(&value) {
                        return Err(format!("{label} must be between {minimum} and 10"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        (value - default).abs() < f64::EPSILON,
                    ));
                }
                if let Some(raw) = properties.specific.get("detail") {
                    let value = raw
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| "detail must be an integer".to_string())?;
                    if !(2..=20).contains(&value) {
                        return Err("detail must be between 2 and 20".to_string());
                    }
                    specific_updates.push(("detail".to_string(), value.to_string(), value == 5));
                }
                if let Some(value) = properties.specific.get("target") {
                    specific_updates.push(("target".to_string(), value.clone(), value.is_empty()));
                }
                if let Some(value) = properties.specific.get("defaultcolor") {
                    if cuemol_gfx::parse_color(value.trim()).is_err() {
                        return Err(format!("invalid disorder color: {value}"));
                    }
                    specific_updates.push((
                        "defaultcolor".to_string(),
                        value.trim().to_string(),
                        value.trim().eq_ignore_ascii_case("#ffffff"),
                    ));
                }
            }
            "dsurface" | "dsurf2" => {
                for (key, default, allowed) in [
                    ("drawmode", "fill", &["fill", "line", "point"][..]),
                    ("surftype", "ses", &["vdw", "sas", "ses"][..]),
                    ("colormode", "molecule", &["molecule", "potential"][..]),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw.trim();
                    if !allowed.contains(&value) {
                        return Err(format!("unsupported direct surface {key}: {value}"));
                    }
                    specific_updates.push((key.to_string(), value.to_string(), value == default));
                }
                if let Some(raw) = properties.specific.get("detail") {
                    let value = raw
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| "surface detail must be an integer".to_string())?;
                    if !(1..=21).contains(&value) {
                        return Err("surface detail must be between 1 and 21".to_string());
                    }
                    specific_updates.push(("detail".to_string(), value.to_string(), value == 6));
                }
                if let Some(raw) = properties.specific.get("width") {
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "surface width must be a number".to_string())?;
                    if !value.is_finite() || value <= 0.0 || value > 10.0 {
                        return Err(
                            "surface width must be greater than 0 and at most 10".to_string()
                        );
                    }
                    specific_updates.push((
                        "width".to_string(),
                        value.to_string(),
                        (value - 1.2).abs() < f64::EPSILON,
                    ));
                }
                for (key, default) in [("target", ""), ("showsel", "*")] {
                    if let Some(raw) = properties.specific.get(key) {
                        let value = raw.trim();
                        specific_updates.push((
                            key.to_string(),
                            value.to_string(),
                            value == default,
                        ));
                    }
                }
            }
            "ribbon" | "cartoon" => {
                let cartoon = renderer_type == "cartoon";
                for (key, label, minimum, maximum, default) in [
                    ("helix.width", "helix width", 0.001, 10.0, 0.35),
                    ("helix.tuber", "helix width ratio", 0.1, 10.0, 1.0),
                    ("helix.sharp", "helix sharpness", 0.0, 1.0, 0.4),
                    ("sheet.width", "sheet width", 0.001, 10.0, 0.35),
                    ("sheet.tuber", "sheet width ratio", 0.1, 10.0, 1.0),
                    ("sheet.sharp", "sheet sharpness", 0.0, 1.0, 0.4),
                    ("coil.width", "coil width", 0.001, 10.0, 0.35),
                    ("coil.tuber", "coil width ratio", 0.1, 10.0, 1.0),
                    ("coil.sharp", "coil sharpness", 0.0, 1.0, 0.4),
                    ("ribhelix.width", "ribbon helix width", 0.001, 10.0, 0.35),
                    ("ribhelix.tuber", "ribbon helix width ratio", 0.1, 10.0, 1.0),
                    ("ribhelix.sharp", "ribbon helix sharpness", 0.0, 1.0, 0.4),
                    ("helixhead.gamma", "helix head gamma", 0.1, 10.0, 2.2),
                    ("helixhead.basw", "helix head base width", 0.0, 1.0, 0.0),
                    ("helixhead.arrow", "helix head arrow width", 1.0, 3.0, 1.8),
                    ("helixtail.gamma", "helix tail gamma", 0.1, 10.0, 2.2),
                    ("helixtail.basw", "helix tail base width", 0.0, 1.0, 0.0),
                    ("helixtail.arrow", "helix tail arrow width", 1.0, 3.0, 1.8),
                    ("sheethead.gamma", "sheet head gamma", 0.1, 10.0, 2.2),
                    ("sheethead.basw", "sheet head base width", 0.0, 1.0, 0.0),
                    ("sheethead.arrow", "sheet head arrow width", 1.0, 3.0, 1.8),
                    ("sheettail.gamma", "sheet tail gamma", 0.1, 10.0, 2.2),
                    ("sheettail.basw", "sheet tail base width", 0.0, 1.0, 0.0),
                    ("sheettail.arrow", "sheet tail arrow width", 1.0, 3.0, 1.8),
                    (
                        "ribhelix_head.gamma",
                        "ribbon helix head gamma",
                        0.1,
                        10.0,
                        2.2,
                    ),
                    (
                        "ribhelix_head.basw",
                        "ribbon helix head base width",
                        0.0,
                        1.0,
                        0.0,
                    ),
                    (
                        "ribhelix_head.arrow",
                        "ribbon helix head arrow width",
                        1.0,
                        3.0,
                        1.8,
                    ),
                    (
                        "ribhelix_tail.gamma",
                        "ribbon helix tail gamma",
                        0.1,
                        10.0,
                        2.2,
                    ),
                    (
                        "ribhelix_tail.basw",
                        "ribbon helix tail base width",
                        0.0,
                        1.0,
                        0.0,
                    ),
                    (
                        "ribhelix_tail.arrow",
                        "ribbon helix tail arrow width",
                        1.0,
                        3.0,
                        1.8,
                    ),
                    ("anchor_weight", "anchor weight", 0.0, 20.0, 10.0),
                    ("helix_width", "helix width", 0.001, 10.0, 2.3),
                    ("helix_extend", "helix extension", 0.0, 3.0, 0.5),
                    ("helix_wplus", "helix width adjustment", -2.0, 3.0, 0.2),
                    ("helix_wsmooth", "helix width smoothing", -5.0, 5.0, 5.0),
                    ("sheet_wsmooth", "sheet width smoothing", -5.0, 5.0, 5.0),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{label} must be a number"))?;
                    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                        return Err(format!("{label} must be between {minimum} and {maximum}"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        (value - default).abs() < f64::EPSILON,
                    ));
                }
                for (key, label, minimum, maximum, default) in [
                    (
                        "helix_smooth",
                        "helix smoothing",
                        if cartoon { -5.0 } else { 0.0 },
                        5.0,
                        if cartoon { 3.0 } else { 0.0 },
                    ),
                    (
                        "sheet_smooth",
                        "sheet smoothing",
                        if cartoon { -5.0 } else { 0.0 },
                        5.0,
                        if cartoon { 2.0 } else { 0.5 },
                    ),
                    (
                        "coil_smooth",
                        "coil smoothing",
                        if cartoon { -5.0 } else { 0.0 },
                        5.0,
                        if cartoon { -1.0 } else { 0.0 },
                    ),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{label} must be a number"))?;
                    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                        return Err(format!("{label} must be between {minimum} and {maximum}"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        (value - default).abs() < f64::EPSILON,
                    ));
                }
                for (key, default) in [
                    ("helix.detail", 16),
                    ("sheet.detail", 16),
                    ("coil.detail", 16),
                    ("ribhelix.detail", 16),
                    ("axialdetail", if cartoon { 8 } else { 6 }),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| format!("{key} must be an integer"))?;
                    if !(2..=20).contains(&value) {
                        return Err(format!("{key} must be between 2 and 20"));
                    }
                    specific_updates.push((key.to_string(), value.to_string(), value == default));
                }
                for key in ["helix.type", "sheet.type", "coil.type", "ribhelix.type"] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw.trim();
                    if !matches!(value, "elliptical" | "roundsquare" | "rectangle" | "fancy1") {
                        return Err(format!("unsupported section type: {value}"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        value == "elliptical",
                    ));
                }
                for key in [
                    "helixhead.type",
                    "helixtail.type",
                    "sheethead.type",
                    "sheettail.type",
                    "ribhelix_head.type",
                    "ribhelix_tail.type",
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw.trim();
                    if !matches!(value, "smooth" | "flat" | "arrow") {
                        return Err(format!("unsupported junction type: {value}"));
                    }
                    specific_updates.push((key.to_string(), value.to_string(), value == "smooth"));
                }
                for (key, default) in [
                    ("smoothcolor", !cartoon),
                    ("segend_fade", false),
                    ("helix_usebackcol", false),
                    ("sheet_usesidecol", false),
                    ("helix_ribbon", false),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = match raw.trim() {
                        "true" => true,
                        "false" => false,
                        _ => return Err(format!("{key} must be true or false")),
                    };
                    specific_updates.push((key.to_string(), value.to_string(), value == default));
                }
                for (key, default) in [
                    ("start_captype", if cartoon { "flat" } else { "sphere" }),
                    ("end_captype", if cartoon { "flat" } else { "sphere" }),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw.trim();
                    if !matches!(value, "flat" | "sphere" | "none") {
                        return Err(format!("unsupported cap type: {value}"));
                    }
                    specific_updates.push((key.to_string(), value.to_string(), value == default));
                }
                if let Some(raw) = properties.specific.get("helix_width_mode") {
                    let value = raw.trim();
                    if !matches!(value, "const" | "average" | "wavy") {
                        return Err(format!("unsupported helix width mode: {value}"));
                    }
                    specific_updates.push((
                        "helix_width_mode".to_string(),
                        value.to_string(),
                        value == "average",
                    ));
                }
                for key in ["pivotatom", "anchor_sel"] {
                    if let Some(raw) = properties.specific.get(key) {
                        let value = raw.trim();
                        specific_updates.push((
                            key.to_string(),
                            value.to_string(),
                            value.is_empty(),
                        ));
                    }
                }
                for key in ["helix_backcol", "sheet_sidecol"] {
                    if let Some(raw) = properties.specific.get(key) {
                        let value = raw.trim();
                        if cuemol_gfx::parse_color(value).is_err() {
                            return Err(format!("invalid ribbon color: {value}"));
                        }
                        specific_updates.push((
                            key.to_string(),
                            value.to_string(),
                            value.eq_ignore_ascii_case("#ffffff"),
                        ));
                    }
                }
            }
            "tube" | "nucl" => {
                for (key, label, minimum, maximum, default) in [
                    ("section.width", "section width", f64::EPSILON, 10.0, 0.35),
                    (
                        "section.tuber",
                        "section width ratio",
                        f64::EPSILON,
                        100.0,
                        1.0,
                    ),
                    ("section.sharp", "section sharpness", 0.0, 1.0, 0.4),
                    ("smooth", "smoothing", 0.0, 1.0, 0.0),
                    ("putty_hiscl", "putty high scale", 0.01, 10.0, 3.0),
                    ("putty_loscl", "putty low scale", 0.01, 10.0, 3.0),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{label} must be a number"))?;
                    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                        return Err(format!("{label} must be between {minimum} and {maximum}"));
                    }
                    specific_updates.push((
                        key.to_string(),
                        value.to_string(),
                        (value - default).abs() < f64::EPSILON,
                    ));
                }
                for (key, label, default) in [
                    ("section.detail", "section detail", 16),
                    ("axialdetail", "axial detail", 6),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| format!("{label} must be an integer"))?;
                    if !(2..=20).contains(&value) {
                        return Err(format!("{label} must be between 2 and 20"));
                    }
                    specific_updates.push((key.to_string(), value.to_string(), value == default));
                }
                for (key, label, default, allowed) in [
                    (
                        "section.type",
                        "section type",
                        "elliptical",
                        &["elliptical", "roundsquare", "rectangle", "fancy1"][..],
                    ),
                    (
                        "start_captype",
                        "start cap type",
                        "sphere",
                        &["flat", "sphere", "none"][..],
                    ),
                    (
                        "end_captype",
                        "end cap type",
                        "sphere",
                        &["flat", "sphere", "none"][..],
                    ),
                    (
                        "putty_mode",
                        "putty mode",
                        "none",
                        &["none", "linear1", "scale1"][..],
                    ),
                    ("putty_tgt", "putty target", "bfac", &["bfac", "occ"][..]),
                ] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = raw.trim();
                    if !allowed.contains(&value) {
                        return Err(format!("unsupported {label}: {value}"));
                    }
                    specific_updates.push((key.to_string(), value.to_string(), value == default));
                }
                for (key, default) in [("smoothcolor", true), ("segend_fade", false)] {
                    let Some(raw) = properties.specific.get(key) else {
                        continue;
                    };
                    let value = match raw.trim() {
                        "true" => true,
                        "false" => false,
                        _ => return Err(format!("{key} must be true or false")),
                    };
                    specific_updates.push((key.to_string(), value.to_string(), value == default));
                }
                if let Some(value) = properties.specific.get("pivotatom") {
                    let value = value.trim();
                    specific_updates.push((
                        "pivotatom".to_string(),
                        value.to_string(),
                        value.is_empty(),
                    ));
                }
                if renderer_type == "nucl" {
                    for (key, label, minimum, maximum, default) in [
                        ("base_size", "base size", 0.0, 10.0, 0.5),
                        ("base_thick", "base thickness", 0.0, 10.0, 0.5),
                    ] {
                        let Some(raw) = properties.specific.get(key) else {
                            continue;
                        };
                        let value = raw
                            .trim()
                            .parse::<f64>()
                            .map_err(|_| format!("{label} must be a number"))?;
                        if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                            return Err(format!("{label} must be between {minimum} and {maximum}"));
                        }
                        specific_updates.push((
                            key.to_string(),
                            value.to_string(),
                            (value - default).abs() < f64::EPSILON,
                        ));
                    }
                    if let Some(raw) = properties.specific.get("base_detail") {
                        let value = raw
                            .trim()
                            .parse::<i32>()
                            .map_err(|_| "base detail must be an integer".to_string())?;
                        if !(2..=20).contains(&value) {
                            return Err("base detail must be between 2 and 20".to_string());
                        }
                        specific_updates.push((
                            "base_detail".to_string(),
                            value.to_string(),
                            value == 3,
                        ));
                    }
                    if let Some(raw) = properties.specific.get("base_type") {
                        let value = raw.trim();
                        if !matches!(value, "basepair" | "simple1" | "detail1" | "detail2") {
                            return Err(format!("unsupported base type: {value}"));
                        }
                        specific_updates.push((
                            "base_type".to_string(),
                            value.to_string(),
                            value == "basepair",
                        ));
                    }
                    for (key, default) in [("show_tube", true), ("show_basepair", true)] {
                        let Some(raw) = properties.specific.get(key) else {
                            continue;
                        };
                        let value = match raw.trim() {
                            "true" => true,
                            "false" => false,
                            _ => return Err(format!("{key} must be true or false")),
                        };
                        specific_updates.push((
                            key.to_string(),
                            value.to_string(),
                            value == default,
                        ));
                    }
                }
            }
            _ => {}
        }
        if renderer_type == "*group" {
            self.desktop
                .set_renderer_group_common_properties(
                    object_uid,
                    renderer_uid,
                    name,
                    properties.visible,
                    properties.locked,
                    properties.material,
                    properties.alpha,
                    properties.edge_type,
                    properties.edge_width,
                    properties.edge_color,
                )
                .map_err(|error| error.to_string())?;
        } else {
            self.desktop
                .set_renderer_common_properties(
                    object_uid,
                    renderer_uid,
                    name,
                    properties.selection,
                    properties.visible,
                    properties.locked,
                    properties.material,
                    properties.alpha,
                    properties.edge_type,
                    properties.edge_width,
                    properties.edge_color,
                )
                .map_err(|error| error.to_string())?;
        }
        let mut set_property = |key: &str, value: String, is_default: bool| {
            let result = if let Some((child, attribute)) = key.split_once('.') {
                if is_default {
                    self.desktop.reset_renderer_child_property(
                        object_uid,
                        renderer_uid,
                        child,
                        attribute,
                    )
                } else {
                    self.desktop.set_renderer_child_property(
                        object_uid,
                        renderer_uid,
                        child,
                        attribute,
                        value,
                    )
                }
            } else if is_default {
                self.desktop
                    .reset_renderer_property(object_uid, renderer_uid, key)
            } else {
                self.desktop
                    .set_renderer_property(object_uid, renderer_uid, key, value)
            };
            result.map_err(|error| error.to_string())
        };
        match renderer_type.as_str() {
            "simple" | "trace" => set_property(
                "width",
                properties.line_width.to_string(),
                (properties.line_width - 1.2).abs() < f64::EPSILON,
            )?,
            "spline" => set_property(
                "line_width",
                properties.line_width.to_string(),
                (properties.line_width - 1.2).abs() < f64::EPSILON,
            )?,
            "ballstick" | "anisou" => {
                set_property(
                    "detail",
                    properties.detail.to_string(),
                    properties.detail == 3,
                )?;
                set_property(
                    "bondw",
                    properties.bond_width.to_string(),
                    (properties.bond_width - 0.2).abs() < f64::EPSILON,
                )?;
                set_property(
                    "sphr",
                    properties.sphere_radius.to_string(),
                    (properties.sphere_radius - 0.3).abs() < f64::EPSILON,
                )?;
                set_property(
                    "ring",
                    properties.show_ring.to_string(),
                    !properties.show_ring,
                )?;
                set_property(
                    "thickness",
                    properties.ring_thickness.to_string(),
                    (properties.ring_thickness - 0.2).abs() < f64::EPSILON,
                )?;
                set_property(
                    "ringcolor",
                    properties.ring_color.trim().to_string(),
                    properties.ring_color.trim().eq_ignore_ascii_case("#ffffbf"),
                )?;
            }
            _ => {}
        }
        for (key, value, is_default) in specific_updates {
            set_property(&key, value, is_default)?;
        }
        if let Some(styles) = properties.specific.get("style") {
            self.desktop
                .set_renderer_styles(object_uid, renderer_uid, styles)
                .map_err(|error| error.to_string())?;
        }
        self.status = "Renderer properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn apply_stored_preferences(&mut self) {
        let font_values = [
            ("DefaultLabel.font_name", "style:DefaultLabel.font_name"),
            ("DefaultLabel.font_size", "style:DefaultLabel.font_size"),
            ("DefaultLabel.color", "style:DefaultLabel.color"),
            ("DefaultLabel.font_style", "style:DefaultLabel.font_style"),
            ("DefaultLabel.font_weight", "style:DefaultLabel.font_weight"),
        ];
        for (style_key, config_key) in font_values {
            let value = self.desktop.config_value(config_key);
            if !value.is_empty() {
                self.desktop.set_style_value(0, "user", style_key, value);
            }
        }
        let style = self.desktop.config_value("cuemol2.ui.viewinconf");
        let tbrad = self
            .desktop
            .config_value("view_input:tbrad")
            .parse()
            .unwrap_or(0.8);
        let hitprec = self
            .desktop
            .config_value("view_input:hitprec")
            .parse()
            .unwrap_or(10.0);
        let input = self.desktop.view_input_config_mut();
        input.set_style(nonempty_or(style, "DefaultViewInConf"));
        input.set_tbrad(tbrad);
        input.set_hitprec(hitprec);
    }

    fn save_config(&self) -> Result<(), String> {
        let Some(path) = self.config_path.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create config directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        self.desktop
            .save_config_file(path)
            .map_err(|error| format!("failed to save config {}: {error}", path.display()))
    }

    fn sync_active_scene_properties(&mut self) {
        let Some(scene) = self
            .desktop
            .active_scene()
            .and_then(|uid| self.desktop.scene_manager().scene(uid))
        else {
            self.background = "black".to_string();
            self.color_proofing = false;
            return;
        };
        self.background = scene
            .scene_attribute("bgcolor")
            .unwrap_or("black")
            .to_string();
        self.color_proofing = scene_attr_bool(scene.scene_attribute("use_colproof"), false);
    }

    fn set_scene_properties(&mut self, properties: ScenePropertiesInput) -> Result<(), String> {
        let scene_uid = Uid::new(properties.scene_uid);
        if self.desktop.scene_manager().scene(scene_uid).is_none() {
            return Err(format!("scene {} was not found", properties.scene_uid));
        }
        let background = properties.background.trim();
        if background.is_empty() {
            return Err("scene background color is required".to_string());
        }
        background_rgba(background)?;
        if !properties.ao_radius.is_finite() || properties.ao_radius <= 0.0 {
            return Err("AO radius must be a positive finite number".to_string());
        }
        if !properties.ao_intensity.is_finite() || properties.ao_intensity <= 0.0 {
            return Err("AO intensity must be a positive finite number".to_string());
        }
        if !(1..=64).contains(&properties.ao_slices) {
            return Err("AO slices must be between 1 and 64".to_string());
        }
        if !(1..=32).contains(&properties.ao_steps) {
            return Err("AO steps must be between 1 and 32".to_string());
        }
        if !matches!(properties.aa_method.as_str(), "none" | "fxaa" | "smaa") {
            return Err(format!("unsupported AA method: {}", properties.aa_method));
        }
        if !(0..=5).contains(&properties.aa_jitter_level) {
            return Err("AA jitter level must be between 0 and 5".to_string());
        }
        if icc_intent_index(&properties.icc_intent).is_none() {
            return Err(format!("unsupported ICC intent: {}", properties.icc_intent));
        }

        let scene = self
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .expect("validated scene must remain live");
        set_scene_attr_default(scene, "bgcolor", background, "black");
        set_scene_attr_default(
            scene,
            "aoEnabled",
            &properties.ao_enabled.to_string(),
            "false",
        );
        set_scene_attr_default(scene, "aoRadius", &properties.ao_radius.to_string(), "4");
        set_scene_attr_default(
            scene,
            "aoIntensity",
            &properties.ao_intensity.to_string(),
            "2.2",
        );
        set_scene_attr_default(scene, "aoSlices", &properties.ao_slices.to_string(), "9");
        set_scene_attr_default(scene, "aoSteps", &properties.ao_steps.to_string(), "3");
        set_scene_attr_default(
            scene,
            "aoHalfRes",
            &properties.ao_half_res.to_string(),
            "false",
        );
        set_scene_attr_default(scene, "aa_method", &properties.aa_method, "fxaa");
        set_scene_attr_default(
            scene,
            "aaJitterLevel",
            &properties.aa_jitter_level.to_string(),
            "0",
        );
        set_scene_attr_default(scene, "onload", &properties.onload, "");
        set_scene_attr_default(
            scene,
            "use_colproof",
            &properties.use_color_proof.to_string(),
            "false",
        );
        set_scene_attr_default(scene, "icc_filename", &properties.icc_filename, "");
        set_scene_attr_default(scene, "icc_intent", &properties.icc_intent, "perceptual");

        if self.desktop.active_scene() == Some(scene_uid) {
            self.sync_active_scene_properties();
        }
        self.status = "Scene properties updated".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn view_properties(&self) -> ViewPropertiesDto {
        self.camera_properties(&self.camera)
    }

    fn camera_properties(&self, camera: &Camera) -> ViewPropertiesDto {
        let scene_uid = self.desktop.active_scene().unwrap_or(Uid::new(1));
        let mut view = View::new(Uid::new(1), scene_uid);
        view.set_camera(camera);
        view.set_view_size(self.view_width, self.view_height);
        view.set_trans_mms(self.trans_mms);
        view.set_rot_mms(self.rot_mms);
        view.set_swap_stereo_eyes(self.swap_stereo_eyes);
        let center = camera.center();
        let rotation = camera.rotation();
        ViewPropertiesDto {
            name: camera.name().to_string(),
            uid: view.uid().get(),
            active: view.is_active(),
            zoom: camera.zoom(),
            distance: camera.cam_dist(),
            slab: camera.slab_depth(),
            center: [center.x, center.y, center.z],
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
            perspective: camera.is_perspective(),
            width: self.view_width,
            height: self.view_height,
            trans_mms: self.trans_mms,
            rot_mms: self.rot_mms,
            stereo_mode: stereo_mode_name(camera.stereo_mode()).to_string(),
            has_hw_stereo: view.has_hw_stereo(),
            stereo_dist: camera.stereo_distance(),
            swap_stereo_eyes: self.swap_stereo_eyes,
            up_vec: vector3_array(view.up_vector()),
            right_vec: vector3_array(view.right_vector()),
            forward_vec: vector3_array(view.forward_vector()),
            center_mark: center_mark_name(camera.center_mark()).to_string(),
        }
    }

    fn apply_camera_properties(
        camera: &mut Camera,
        input: &ViewPropertiesInput,
    ) -> Result<(), String> {
        if !input.zoom.is_finite() || input.zoom <= 0.0 {
            return Err("view zoom must be a positive finite number".to_string());
        }
        if !input.distance.is_finite() || input.distance <= 0.0 {
            return Err("view distance must be a positive finite number".to_string());
        }
        if !input.slab.is_finite() || input.slab <= 0.0 {
            return Err("view slab must be a positive finite number".to_string());
        }
        if !input.stereo_dist.is_finite() || input.stereo_dist < 0.0 {
            return Err("stereo distance must be a non-negative finite number".to_string());
        }
        if !input.center.iter().all(|value| value.is_finite())
            || !input.rotation.iter().all(|value| value.is_finite())
        {
            return Err("view vectors must contain finite numbers".to_string());
        }
        let rotation = Quat::new(
            input.rotation[0],
            input.rotation[1],
            input.rotation[2],
            input.rotation[3],
        )
        .normalize()
        .map_err(|error| error.to_string())?;
        let stereo_mode = parse_stereo_mode(&input.stereo_mode)?;
        let center_mark = parse_center_mark(&input.center_mark)?;
        camera.set_name(input.name.trim());
        camera.set_zoom(input.zoom);
        camera.set_cam_dist(input.distance);
        camera.set_slab_depth(input.slab);
        camera.set_center(Vector4::new3(
            input.center[0],
            input.center[1],
            input.center[2],
        ));
        camera.set_rotation(rotation);
        camera.set_perspective(input.perspective);
        camera.set_stereo_mode(stereo_mode);
        camera.set_stereo_distance(input.stereo_dist);
        camera.set_center_mark(center_mark);
        Ok(())
    }

    fn set_view_properties(&mut self, input: ViewPropertiesInput) -> Result<(), String> {
        Self::apply_camera_properties(&mut self.camera, &input)?;
        self.trans_mms = input.trans_mms;
        self.rot_mms = input.rot_mms;
        self.swap_stereo_eyes = input.swap_stereo_eyes;
        self.status = "View properties updated".to_string();
        Ok(())
    }

    fn named_camera_properties(&self, name: &str) -> Result<ViewPropertiesDto, String> {
        let scene = self
            .desktop
            .active_scene()
            .and_then(|scene_uid| self.desktop.scene_manager().scene(scene_uid))
            .ok_or_else(|| "no active scene".to_string())?;
        let camera = scene
            .camera_by_name(name)
            .ok_or_else(|| format!("camera not found: {name}"))?;
        Ok(self.camera_properties(camera))
    }

    fn set_named_camera_properties(
        &mut self,
        old_name: &str,
        input: ViewPropertiesInput,
    ) -> Result<(), String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        let scene = self
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .ok_or_else(|| "no active scene".to_string())?;
        let mut camera = scene
            .camera_by_name(old_name)
            .cloned()
            .ok_or_else(|| format!("camera not found: {old_name}"))?;
        let new_name = input.name.trim().to_string();
        if new_name.is_empty() {
            return Err("camera name is required".to_string());
        }
        if new_name != old_name && scene.camera_by_name(&new_name).is_some() {
            return Err(format!("camera already exists: {new_name}"));
        }
        Self::apply_camera_properties(&mut camera, &input)?;
        if new_name != old_name {
            self.desktop
                .destroy_scene_camera(scene_uid, old_name)
                .map_err(|error| error.to_string())?;
        }
        self.desktop
            .save_scene_camera(scene_uid, camera, false)
            .map_err(|error| error.to_string())?;
        self.status = format!("Saved camera {new_name}");
        Ok(())
    }

    fn rebuild_mesh(&mut self, refit: bool) -> Result<(), String> {
        let mut snapshot = self
            .desktop
            .active_render_snapshot(&self.selection)
            .map_err(|error| error.to_string())?;
        self.desktop.append_crystal_geometry(
            &mut snapshot,
            self.show_unit_cell,
            self.show_symmetry_mates,
        );
        self.desktop.append_density_geometry(
            &mut snapshot,
            self.density_level,
            self.show_density_contour,
        );
        if self.color_proofing {
            let scene = self
                .desktop
                .active_scene()
                .and_then(|uid| self.desktop.scene_manager().scene(uid));
            let profile = scene
                .and_then(|scene| scene.scene_attribute("icc_filename"))
                .unwrap_or("");
            let intent = scene
                .and_then(|scene| scene.scene_attribute("icc_intent"))
                .and_then(icc_intent_index)
                .unwrap_or(0);
            if self
                .desktop
                .apply_color_proof_with(&mut snapshot, profile, intent)
                .is_err()
            {
                cuemol_render::apply_generic_color_proof(&mut snapshot);
            }
        }
        if refit {
            self.camera = self
                .desktop
                .active_scene_saved_camera()
                .unwrap_or_else(|| frame_camera(&snapshot));
        }
        self.screen_lines = snapshot
            .bonds
            .iter()
            .filter(|bond| bond.screen_width.is_some())
            .cloned()
            .collect();
        snapshot.bonds.retain(|bond| bond.screen_width.is_none());
        self.mesh = Some(tessellate_snapshot(&snapshot));
        self.mesh_refit_pending = false;
        Ok(())
    }

    fn invalidate_mesh(&mut self, refit: bool) {
        self.mesh = None;
        self.screen_lines.clear();
        self.mesh_refit_pending |= refit;
    }

    fn prepare_mesh_if_needed(&mut self) -> Result<(), String> {
        if self.mesh.is_none() {
            self.rebuild_mesh(self.mesh_refit_pending)?;
        }
        Ok(())
    }

    fn open_paths(&mut self, paths: Vec<String>) -> Result<usize, String> {
        self.opened_paths.clear();
        let mut failures = Vec::new();
        for path in paths {
            match self.desktop.open_path(&path) {
                Ok(_) => self.opened_paths.push(path),
                Err(error) => failures.push(format!("{path}: {error}")),
            }
        }
        let loaded = self.opened_paths.len();
        self.selection.clear();
        self.sync_active_scene_properties();
        self.status = if failures.is_empty() {
            format!("Loaded {loaded} file(s)")
        } else {
            format!("Loaded {loaded}; failed: {}", failures.join("; "))
        };
        self.invalidate_mesh(true);
        if loaded == 0 {
            return Err(self.status.clone());
        }
        Ok(loaded)
    }

    fn active_scene_is_just_created(&self) -> bool {
        self.desktop.active_scene_is_just_created()
            && self
                .desktop
                .active_scene()
                .is_some_and(|scene_uid| !self.modified_scenes.contains(&scene_uid))
    }

    fn open_scene(&mut self, path: String) -> Result<(), String> {
        self.start_renderer_warmup();
        self.restore_animation_playback(false)?;
        self.animation_elapsed = 0.0;
        let reusable_scene = self
            .active_scene_is_just_created()
            .then(|| self.desktop.active_scene())
            .flatten();
        if reusable_scene.is_none() && self.desktop.active_scene().is_some() {
            self.persist_camera()?;
        }
        let scene_uid = self
            .desktop
            .open_scene_file(&path, reusable_scene)
            .map_err(|error| error.to_string())?;
        self.opened_paths.clear();
        let saved_camera = self.desktop.active_scene_saved_camera();
        self.camera = saved_camera
            .clone()
            .or_else(|| self.desktop.active_view_camera())
            .unwrap_or_default();
        self.opened_paths.push(path.clone());
        self.selection.clear();
        self.reset_animation_playback();
        self.sync_active_scene_properties();
        self.status = format!("Scene file: [{path}] is loaded.");
        self.undo_stack.clear();
        self.redo_stack.clear();
        if self.desktop.active_scene_source_path().is_some() {
            self.modified_scenes.remove(&scene_uid);
        } else {
            self.modified_scenes.insert(scene_uid);
        }
        self.touch_structure_details();
        self.invalidate_mesh(saved_camera.is_none());
        Ok(())
    }

    fn set_trajectory_frame(&mut self, object_uid: u64, frame: usize) -> Result<(), String> {
        self.desktop
            .apply_trajectory_frame(Uid::new(object_uid), frame)
            .map_err(|error| error.to_string())?;
        self.status = format!("Trajectory frame: {frame}");
        self.invalidate_mesh(false);
        Ok(())
    }

    fn render_mesh_rgba(
        &self,
        renderer: &SceneRenderer,
        mesh: &RenderMesh,
        width: u32,
        height: u32,
    ) -> Option<Vec<u8>> {
        let scene = self
            .desktop
            .active_scene()
            .and_then(|uid| self.desktop.scene_manager().scene(uid));
        let jitter_level = scene_attr_i32(
            scene.and_then(|scene| scene.scene_attribute("aaJitterLevel")),
            0,
        )
        .clamp(0, 5);
        let aa_method = scene
            .and_then(|scene| scene.scene_attribute("aa_method"))
            .unwrap_or("fxaa");
        let mvp = camera_mvp(&self.camera, f64::from(width) / f64::from(height));
        let mut clear = background_rgba(&self.background).unwrap_or([0.0, 0.0, 0.0, 1.0]);
        clear[3] = 1.0;
        let mut rgba = renderer.render_supersampled(
            std::slice::from_ref(mesh),
            mvp,
            width,
            height,
            clear,
            jitter_level,
        )?;

        if scene_attr_bool(
            scene.and_then(|scene| scene.scene_attribute("aoEnabled")),
            false,
        ) {
            let (near, far) = camera_depth_range(&self.camera);
            if let Some(depth) = renderer.render_depth(
                std::slice::from_ref(mesh),
                mvp,
                width,
                height,
                near,
                far,
                self.camera.is_perspective(),
            ) {
                let radius = scene_attr_f64(
                    scene.and_then(|scene| scene.scene_attribute("aoRadius")),
                    4.0,
                );
                let intensity = scene_attr_f64(
                    scene.and_then(|scene| scene.scene_attribute("aoIntensity")),
                    2.2,
                );
                let slices =
                    scene_attr_i32(scene.and_then(|scene| scene.scene_attribute("aoSlices")), 9);
                let steps =
                    scene_attr_i32(scene.and_then(|scene| scene.scene_attribute("aoSteps")), 3);
                let radius_pixels = ((radius / self.camera.slab_depth())
                    * f64::from(width.min(height)))
                .round()
                .clamp(1.0, 64.0) as usize;
                apply_screen_space_ao(
                    &mut rgba,
                    &depth,
                    width,
                    height,
                    radius_pixels,
                    intensity,
                    slices,
                    steps,
                );
            }
        }
        if aa_method != "none" {
            apply_fast_post_aa(&mut rgba, width, height, aa_method == "smaa");
        }
        Some(rgba)
    }

    fn render(&mut self, width: u32, height: u32) -> ViewportDto {
        if let Err(error) = self.prepare_mesh_if_needed() {
            self.status = format!("Render state failed: {error}");
        }
        let mut viewport = self.viewport_metadata(width, height);
        let width = viewport.width;
        let height = viewport.height;
        if self.renderer.is_none()
            && self
                .mesh
                .as_ref()
                .is_some_and(|mesh| !mesh.indices.is_empty())
        {
            self.finish_renderer_warmup();
        }
        viewport.image =
            self.renderer
                .as_ref()
                .zip(self.mesh.as_ref())
                .and_then(|(renderer, mesh)| {
                    if mesh.indices.is_empty() {
                        return None;
                    }
                    self.render_mesh_rgba(renderer, mesh, width, height)
                        .map(|rgba| {
                            let png = encode_rgba_png(&rgba, width, height);
                            format!("data:image/png;base64,{}", base64_encode(&png))
                        })
                });
        viewport
    }

    fn viewport_metadata(&mut self, width: u32, height: u32) -> ViewportDto {
        let width = width.clamp(64, MAX_VIEW_DIM);
        let height = height.clamp(64, MAX_VIEW_DIM);
        self.view_width = width;
        self.view_height = height;
        let mvp = camera_mvp(&self.camera, f64::from(width) / f64::from(height));
        let lines = self
            .screen_lines
            .iter()
            .filter_map(|line| {
                let (x1, y1) = project_viewport_point(mvp, line.from, width, height)?;
                let (x2, y2) = project_viewport_point(mvp, line.to, width, height)?;
                Some(ViewportLineDto {
                    x1: x1 + line.screen_offset[0],
                    y1: y1 + line.screen_offset[1],
                    x2: x2 + line.screen_offset[0],
                    y2: y2 + line.screen_offset[1],
                    width: line.screen_width.unwrap_or(1.0),
                    color: line
                        .color
                        .as_ref()
                        .and_then(cuemol_gfx::ColorSpec::rgba8)
                        .map(viewport_css_color)
                        .unwrap_or_else(|| "#ffff00".to_string()),
                    end_color: line
                        .end_color
                        .as_ref()
                        .and_then(cuemol_gfx::ColorSpec::rgba8)
                        .map(viewport_css_color),
                    stipple: line.screen_stipple,
                    point: line.screen_point,
                })
            })
            .take(200_000)
            .collect();
        let mut labels =
            self.desktop
                .active_atom_intr_labels(mvp, width as f32, height as f32, 1_000);
        let remaining = 1_000_usize.saturating_sub(labels.len());
        labels.extend(
            self.desktop
                .active_name_labels(mvp, width as f32, height as f32, remaining),
        );
        ViewportDto {
            image: None,
            width,
            height,
            perspective: self.camera.is_perspective(),
            center_mark: center_mark_name(self.camera.center_mark()).to_string(),
            lines,
            labels: labels
                .into_iter()
                .map(|label| ViewportLabelDto {
                    x: label.x,
                    y: label.y,
                    text: label.text,
                    color: label.color,
                    font_size: label.font_size,
                    font_name: label.font_name,
                    font_style: label.font_style,
                    font_weight: label.font_weight,
                })
                .collect(),
            status: self.status.clone(),
        }
    }

    fn start_renderer_warmup(&mut self) {
        self.start_renderer_warmup_with(shared_scene_renderer);
    }

    fn start_renderer_warmup_with(
        &mut self,
        initialize: impl FnOnce() -> Option<Arc<SceneRenderer>> + Send + 'static,
    ) {
        if self.renderer.is_some() || self.renderer_warmup.is_some() || self.renderer_init_attempted
        {
            return;
        }
        if let Ok(handle) = std::thread::Builder::new()
            .name("cuemol-renderer-warmup".to_string())
            .spawn(initialize)
        {
            self.renderer_warmup = Some(handle);
            self.renderer_init_attempted = true;
        }
    }

    fn finish_renderer_warmup(&mut self) {
        if self.renderer.is_some() {
            return;
        }
        if let Some(handle) = self.renderer_warmup.take() {
            self.renderer = handle.join().ok().flatten();
            return;
        }
        if !self.renderer_init_attempted {
            self.renderer_init_attempted = true;
            self.renderer = shared_scene_renderer();
        }
    }

    fn animation_render_frame_count(&self, input: &AnimationRenderInput) -> Result<u32, String> {
        let animation = self.scene_animation_manager(Uid::new(input.scene_uid))?;
        if animation.size() == 0 {
            return Err("no animation objects are defined".to_string());
        }
        if !input.frame_rate.is_finite() || !(1.0..=240.0).contains(&input.frame_rate) {
            return Err("frame rate must be between 1 and 240 fps".to_string());
        }
        if !(100..=10_000).contains(&input.width) || !(100..=10_000).contains(&input.height) {
            return Err("animation dimensions must be between 100 and 10000".to_string());
        }
        if input.output_dir.trim().is_empty() {
            return Err("animation output directory is required".to_string());
        }
        let base_name = input.base_name.trim();
        if base_name.is_empty()
            || base_name.contains('/')
            || base_name.contains('\\')
            || base_name == "."
            || base_name == ".."
        {
            return Err("output base name must be a simple file name".to_string());
        }
        if !(1..=32).contains(&input.concurrency) {
            return Err("animation concurrency must be between 1 and 32".to_string());
        }
        if !matches!(input.backend.as_str(), "povray" | "umbreon") {
            return Err("animation backend must be POV-Ray or Umbreon".to_string());
        }
        if input.backend == "povray" {
            if input.povray_executable.trim().is_empty() {
                return Err("POV-Ray executable is required".to_string());
            }
            if !input.povray_include_dir.trim().is_empty()
                && !Path::new(input.povray_include_dir.trim()).is_dir()
            {
                return Err("POV-Ray include directory does not exist".to_string());
            }
            if !(-1..=10).contains(&input.pov_radiosity) {
                return Err("POV-Ray radiosity mode must be between -1 and 10".to_string());
            }
        }
        if input.make_movie {
            if input.ffmpeg_executable.trim().is_empty() {
                return Err("FFmpeg executable is required".to_string());
            }
            if !matches!(
                input.movie_format.as_str(),
                "mov_h264" | "mov_h265" | "mov_raw" | "mp4_h264" | "mp4_h265" | "wmv2" | "gifanim"
            ) {
                return Err("unsupported animation movie format".to_string());
            }
            if !(1..=200_000).contains(&input.bitrate_kbps) {
                return Err("animation movie bitrate must be between 1 and 200000 kbps".to_string());
            }
        }
        let total = animation_seconds(animation.length());
        let intervals = (total * input.frame_rate).round().max(1.0) as u32;
        let frame_count = intervals.saturating_add(u32::from(input.duplicate_last_frame));
        if frame_count > 9_999 {
            return Err("animation rendering is limited to 9999 frames".to_string());
        }
        Ok(frame_count)
    }

    fn begin_animation_render(&mut self, input: &AnimationRenderInput) -> Result<(), String> {
        fs::create_dir_all(Path::new(input.output_dir.trim()))
            .map_err(|error| error.to_string())?;
        let scene_uid = Uid::new(input.scene_uid);
        if self.desktop.active_scene() != Some(scene_uid) {
            if !self.desktop.set_active_scene(Some(scene_uid)) {
                return Err(format!("scene {} was not found", input.scene_uid));
            }
            self.camera = self
                .desktop
                .active_scene_saved_camera()
                .or_else(|| self.desktop.active_view_camera())
                .unwrap_or_default();
            self.sync_active_scene_properties();
        }
        self.camera.set_perspective(input.perspective);
        self.animation_elapsed = 0.0;
        self.animation_playing = true;
        self.animation_start_camera = Some(self.camera.clone());
        self.invalidate_mesh(false);
        Ok(())
    }

    fn prepare_animation_frame_task(
        &mut self,
        input: &AnimationRenderInput,
        index: u32,
        total_frames: u32,
        cancel: Arc<AtomicBool>,
        progress: Arc<AtomicU32>,
    ) -> Result<TauriAnimationFrameTask, String> {
        let total = self.animation_duration();
        let target = (f64::from(index) / input.frame_rate).min(total);
        self.apply_active_animation_time(target)?;
        self.animation_elapsed = target;
        let mut snapshot = self
            .desktop
            .active_render_snapshot(&self.selection)
            .map_err(|error| error.to_string())?;
        self.desktop.append_crystal_geometry(
            &mut snapshot,
            self.show_unit_cell,
            self.show_symmetry_mates,
        );
        self.desktop.append_density_geometry(
            &mut snapshot,
            self.density_level,
            self.show_density_contour,
        );
        if self.color_proofing {
            let scene = self
                .desktop
                .active_scene()
                .and_then(|uid| self.desktop.scene_manager().scene(uid));
            let profile = scene
                .and_then(|scene| scene.scene_attribute("icc_filename"))
                .unwrap_or("");
            let intent = scene
                .and_then(|scene| scene.scene_attribute("icc_intent"))
                .and_then(icc_intent_index)
                .unwrap_or(0);
            if self
                .desktop
                .apply_color_proof_with(&mut snapshot, profile, intent)
                .is_err()
            {
                cuemol_render::apply_generic_color_proof(&mut snapshot);
            }
        }
        let background = background_rgba(&self.background)?;
        let labels = collect_scene_pov_labels(
            &self.desktop,
            Uid::new(input.scene_uid),
            &self.camera,
            input.width,
            input.height,
            input.backend == "povray" && input.pov_labels,
        );
        Ok(TauriAnimationFrameTask {
            input: input.clone(),
            index,
            total_frames,
            snapshot,
            camera: self.camera.clone(),
            labels,
            background: [
                background[0] as f32,
                background[1] as f32,
                background[2] as f32,
                1.0,
            ],
            cancel,
            progress,
        })
    }

    fn update_camera(
        &mut self,
        action: &str,
        x: f64,
        y: f64,
        viewport_height: u32,
    ) -> Result<(), String> {
        match action {
            "rotate" => {
                let rotation = self
                    .camera
                    .rotation()
                    .rotate_y(x)
                    .and_then(|rotation| rotation.rotate_x(y))
                    .map_err(|error| error.to_string())?;
                self.camera.set_rotation(rotation);
            }
            "pan" => {
                self.camera
                    .set_center(pan_center(&self.camera, x, y, viewport_height.max(1)));
            }
            "zoom" => {
                self.camera
                    .set_zoom((self.camera.zoom() * x.clamp(0.2, 5.0)).clamp(0.5, 1.0e6));
            }
            "slab" => {
                self.camera.set_slab_depth(
                    (self.camera.slab_depth() * x.clamp(0.2, 5.0)).clamp(0.1, 10_000.0),
                );
            }
            "fit" => {
                let snapshot = self
                    .desktop
                    .active_render_snapshot(&self.selection)
                    .map_err(|error| error.to_string())?;
                self.camera = frame_camera(&snapshot);
                self.status = "View fitted".to_string();
            }
            "projection" => {
                self.camera.set_perspective(!self.camera.is_perspective());
                self.status = if self.camera.is_perspective() {
                    "Projection: perspective".to_string()
                } else {
                    "Projection: orthographic".to_string()
                };
            }
            "center-mark-cross" => {
                self.camera.set_center_mark(CenterMark::Cross);
                self.status = "Center mark: crosshair".to_string();
            }
            "center-mark-axis" => {
                self.camera.set_center_mark(CenterMark::Axis);
                self.status = "Center mark: axis".to_string();
            }
            "center-mark-none" => {
                self.camera.set_center_mark(CenterMark::None);
                self.status = "Center mark: none".to_string();
            }
            _ => return Err(format!("unknown camera action: {action}")),
        }
        Ok(())
    }

    fn persist_camera(&mut self) -> Result<(), String> {
        let mut camera = self.camera.clone();
        camera.set_name("__current");
        if self.desktop.active_view_summary().is_some() {
            self.desktop
                .set_active_view_camera(&camera)
                .map_err(|error| error.to_string())?;
        }
        self.desktop
            .set_active_scene_camera(camera)
            .map_err(|error| error.to_string())
    }

    fn save_named_camera(&mut self, name: &str, save_visibility: bool) -> Result<(), String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        let name = name.trim();
        if name.is_empty() {
            return Err("camera name is required".to_string());
        }
        let mut camera = self.camera.clone();
        camera.set_name(name);
        self.desktop
            .save_scene_camera(scene_uid, camera, save_visibility)
            .map_err(|error| error.to_string())?;
        self.status = format!("Saved camera {name}");
        Ok(())
    }

    fn load_named_camera(&mut self, name: &str, load_visibility: bool) -> Result<(), String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        self.camera = self
            .desktop
            .load_scene_camera(scene_uid, name, load_visibility)
            .map_err(|error| error.to_string())?;
        self.invalidate_mesh(false);
        self.status = format!("Loaded camera {name}");
        Ok(())
    }

    fn destroy_named_camera(&mut self, name: &str) -> Result<(), String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        if !self
            .desktop
            .destroy_scene_camera(scene_uid, name)
            .map_err(|error| error.to_string())?
        {
            return Err(format!("camera not found: {name}"));
        }
        self.status = format!("Destroyed camera {name}");
        Ok(())
    }

    fn create_named_style(&mut self, name: &str) -> Result<(), String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        self.desktop
            .create_scene_style(scene_uid, name)
            .map_err(|error| error.to_string())?;
        self.status = format!("Created style {}", name.trim());
        Ok(())
    }

    fn create_renderer_style(&mut self, input: RendererStyleCreateInput) -> Result<(), String> {
        let style_name = self
            .desktop
            .create_scene_renderer_style(
                Uid::new(input.scene_uid),
                input.local_index,
                Uid::new(input.object_uid),
                Uid::new(input.renderer_uid),
                &input.name_prefix,
                input.overwrite,
            )
            .map_err(|error| error.to_string())?;
        self.status = format!("Created style {style_name}");
        Ok(())
    }

    fn style_properties(
        &self,
        scene_uid: u64,
        local_index: usize,
    ) -> Result<StylePropertiesDto, String> {
        let scene_uid = Uid::new(scene_uid);
        let style = self
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .and_then(|scene| scene.loaded_styles().get(local_index))
            .ok_or_else(|| format!("style {local_index} not found"))?;
        let name = style.node.str_attr("id").to_string();
        Ok(StylePropertiesDto {
            scene_uid: scene_uid.get(),
            local_index,
            name: if name.is_empty() {
                "(anonymous)".to_string()
            } else {
                name
            },
            read_only: style.node.str_attr("readonly").equals_ignore_case("true"),
            xml: style.data.clone(),
        })
    }

    fn set_style_properties(&mut self, input: StylePropertiesInput) -> Result<(), String> {
        self.desktop
            .update_scene_style(
                Uid::new(input.scene_uid),
                input.local_index,
                &input.name,
                input.read_only,
                &input.xml,
            )
            .map_err(|error| error.to_string())?;
        self.status = format!("Saved style {}", input.name.trim());
        self.invalidate_mesh(false);
        Ok(())
    }

    fn destroy_style(&mut self, scene_uid: u64, local_index: usize) -> Result<(), String> {
        if !self
            .desktop
            .destroy_scene_style(Uid::new(scene_uid), local_index)
            .map_err(|error| error.to_string())?
        {
            return Err(format!("style {local_index} not found"));
        }
        self.status = "Destroyed style".to_string();
        self.invalidate_mesh(false);
        Ok(())
    }

    fn atom_at_screen(
        &self,
        selection: &str,
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    ) -> Result<Option<crate::GuiAtomSummary>, String> {
        let atoms = self
            .desktop
            .active_pick_atoms(selection)
            .map_err(|error| error.to_string())?;
        let mvp = camera_mvp(
            &self.camera,
            f64::from(width.max(1)) / f64::from(height.max(1)),
        );
        let Some((object_uid, atom_index)) = pick_atom_screen(
            &atoms,
            mvp,
            (x, y),
            (f64::from(width), f64::from(height)),
            self.desktop.view_input_config().hitprec(),
        ) else {
            return Ok(None);
        };
        self.desktop
            .atom_summary(Uid::new(object_uid), atom_index)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    fn pick_atom(&mut self, x: f64, y: f64, width: u32, height: u32) -> Result<(), String> {
        let Some(atom) = self.atom_at_screen(&self.selection, x, y, width, height)? else {
            self.status = "No atom at cursor".to_string();
            return Ok(());
        };
        self.selection = atom.selection_text;
        self.status = format!(
            "Selected atom {} {}{}:{}",
            atom.serial, atom.residue_name, atom.residue_id, atom.name
        );
        self.invalidate_mesh(false);
        Ok(())
    }

    fn pick_atom_identifier(
        &self,
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    ) -> Result<Option<PickedAtomDto>, String> {
        Ok(self
            .atom_at_screen("", x, y, width, height)?
            .map(|atom| PickedAtomDto {
                object_uid: atom.object_uid.get(),
                aid: atom.serial,
            }))
    }

    fn select_screen_rect(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let atoms = self
            .desktop
            .active_pick_atoms(&self.selection)
            .map_err(|error| error.to_string())?;
        let mvp = camera_mvp(
            &self.camera,
            f64::from(width.max(1)) / f64::from(height.max(1)),
        );
        let hits = atoms_in_screen_rect(
            &atoms,
            mvp,
            (x1, y1),
            (x2, y2),
            (f64::from(width), f64::from(height)),
        );
        let serials = hits
            .into_iter()
            .filter_map(|(object_uid, atom_index)| {
                self.desktop
                    .atom_summary(Uid::new(object_uid), atom_index)
                    .ok()
                    .map(|atom| atom.serial)
            })
            .collect::<std::collections::BTreeSet<_>>();
        self.selection = if serials.is_empty() {
            String::new()
        } else {
            format!(
                "aid {}",
                serials
                    .iter()
                    .map(i32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        self.status = format!("Rectangle selected {} atom(s)", serials.len());
        self.invalidate_mesh(false);
        Ok(())
    }

    fn scene_animation_manager(&self, scene_uid: Uid) -> Result<AnimMgr, String> {
        if self.desktop.scene_manager().scene(scene_uid).is_none() {
            return Err(format!("scene {} was not found", scene_uid.get()));
        }
        self.desktop
            .scene_animation(scene_uid)
            .map_err(|error| error.to_string())
            .map(Option::unwrap_or_default)
    }

    fn active_animation_manager(&self) -> Result<AnimMgr, String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        self.scene_animation_manager(scene_uid)
    }

    fn set_active_animation_manager(&mut self, animation: &AnimMgr) -> Result<(), String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        self.desktop
            .set_scene_animation(scene_uid, Some(animation))
            .map_err(|error| error.to_string())
    }

    fn add_animation_object(
        &mut self,
        kind: &str,
        after_index: Option<usize>,
        properties: Option<AnimationObjectPropertiesInput>,
    ) -> Result<AnimKind, String> {
        let kind = animation_kind(kind).ok_or_else(|| format!("unsupported animation: {kind}"))?;
        self.restore_animation_playback(false)?;
        self.animation_elapsed = 0.0;
        let undo = self.capture_undo();
        let mut animation = self.active_animation_manager()?;
        if animation.startcam().is_empty() {
            self.persist_camera()?;
            animation.set_startcam("__current");
        }
        let prefix = animation_name_prefix(kind);
        let name = unique_animation_name(&animation, prefix);
        let insertion_index = after_index
            .map(|index| index.saturating_add(1).min(animation.size()))
            .unwrap_or_else(|| animation.size());
        let reference = insertion_index
            .checked_sub(1)
            .and_then(|index| animation.motions().get(index))
            .map(AnimMotion::base)
            .map(|object| object.name().to_string())
            .unwrap_or_default();
        let motion = new_animation_motion(kind, name, reference);
        if insertion_index == animation.size() {
            animation.append_motion(motion);
        } else {
            animation.insert_motion_before(insertion_index, motion);
        }
        self.set_active_animation_manager(&animation)?;
        if let Some(mut properties) = properties {
            properties.index = insertion_index;
            if let Err(error) = self.set_animation_object_properties(properties) {
                self.restore_undo(undo)?;
                return Err(error);
            }
        }
        self.reset_animation_playback();
        self.status = format!("Added {} animation", kind.label());
        self.push_undo(undo);
        Ok(kind)
    }

    fn set_animation_object_properties(
        &mut self,
        input: AnimationObjectPropertiesInput,
    ) -> Result<(), String> {
        let name = input.name.trim().to_string();
        let time_ref_name = input.time_ref_name.trim().to_string();
        if name.is_empty() {
            return Err("animation object name is required".to_string());
        }
        if !input.start.is_finite() || !(-86_400.0..=86_400.0).contains(&input.start) {
            return Err(
                "animation start time must be between -86400 and 86400 seconds".to_string(),
            );
        }
        if !input.duration.is_finite() || !(0.001..=86_400.0).contains(&input.duration) {
            return Err("animation duration must be between 0.001 and 86400 seconds".to_string());
        }
        if !input.quadric.is_finite() || !(0.0..=0.5).contains(&input.quadric) {
            return Err("animation quadric must be between 0 and 0.5".to_string());
        }
        let mut animation = self.active_animation_manager()?;
        let current = animation
            .motions()
            .get(input.index)
            .ok_or_else(|| format!("animation object {} was not found", input.index))?;
        let uid = current.base().uid();
        let old_name = current.base().name().to_string();
        if animation
            .motions()
            .iter()
            .any(|motion| motion.base().uid() != uid && motion.base().name() == name)
        {
            return Err(format!("animation object name {name} is already used"));
        }
        if !time_ref_name.is_empty() {
            let reference = animation
                .motions()
                .iter()
                .find(|motion| motion.base().name() == time_ref_name)
                .ok_or_else(|| format!("animation reference {time_ref_name} was not found"))?;
            if reference.base().uid() == uid {
                return Err("animation object cannot reference itself".to_string());
            }
        }
        if let Some(value) = input.angle {
            if !value.is_finite() {
                return Err("rotation angle must be finite".to_string());
            }
        }
        if let Some(axis) = input.axis {
            if axis.iter().any(|value| !value.is_finite())
                || axis.iter().map(|value| value * value).sum::<f64>() < 1.0e-12
            {
                return Err("spin axis must be a non-zero finite vector".to_string());
            }
        }
        for (label, value) in [
            ("target opacity", input.target_alpha),
            ("direction angle", input.direction),
            ("distance", input.distance),
            ("start value", input.start_value),
            ("end value", input.end_value),
        ] {
            if value.is_some_and(|value| !value.is_finite()) {
                return Err(format!("{label} must be finite"));
            }
        }
        if input
            .target_alpha
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        {
            return Err("target opacity must be between 0 and 1".to_string());
        }
        for (label, value) in [
            ("start position", input.start_position),
            ("end position", input.end_position),
        ] {
            if value.is_some_and(|value| value.iter().any(|component| !component.is_finite())) {
                return Err(format!("{label} must be finite"));
            }
        }
        if let Some(target_camera) = input.target_camera.as_deref() {
            let target_camera = target_camera.trim();
            let scene_uid = self.desktop.active_scene().unwrap();
            if !target_camera.is_empty()
                && !self
                    .desktop
                    .camera_summaries(scene_uid)
                    .iter()
                    .any(|camera| camera.name == target_camera)
            {
                return Err(format!("camera {target_camera} was not found"));
            }
        }
        let start = animation_time(input.start);
        let duration = animation_time(input.duration);
        animation.edit_motion_by_uid(uid, "properties", "", |motion| {
            let object = motion.base_mut();
            object.set_name(name.clone());
            object.set_disabled(input.disabled);
            object.set_time_ref_name(time_ref_name.clone());
            object.set_rel_start(start);
            object.set_rel_end(start.saturating_add(duration));
            object.set_quadric(input.quadric);
            match motion {
                AnimMotion::Camera(value) => {
                    if let Some(target) = input.target_camera.as_deref() {
                        value.set_endcam(target.trim());
                    }
                    if let Some(value_input) = input.ignore_center {
                        value.set_ignorecenter(value_input);
                    }
                    if let Some(value_input) = input.ignore_rotate {
                        value.set_ignorerotate(value_input);
                    }
                    if let Some(value_input) = input.ignore_zoom {
                        value.set_ignorezoom(value_input);
                    }
                    if let Some(value_input) = input.ignore_slab {
                        value.set_ignoreslab(value_input);
                    }
                    if let Some(value_input) = input.keep_quaternion_positive {
                        value.set_keepquatpositive(value_input);
                    }
                }
                AnimMotion::RealProperty(value) => {
                    if let Some(targets) = input.renderer_names.as_deref() {
                        value.set_rend(targets.trim());
                    }
                    if let Some(property) = input.property_name.as_deref() {
                        value.set_prop(property.trim());
                    }
                    if let Some(value_input) = input.start_value {
                        value.set_start_value(value_input);
                    }
                    if let Some(value_input) = input.end_value {
                        value.set_end_value(value_input);
                    }
                }
                AnimMotion::Molecule(value) => {
                    if let Some(target) = input.molecule_name.as_deref() {
                        value.set_mol(target.trim());
                    }
                    if let Some(property) = input.property_name.as_deref() {
                        value.set_prop(property.trim());
                    }
                    if let Some(value_input) = input.start_value {
                        value.set_start_value(value_input);
                    }
                    if let Some(value_input) = input.end_value {
                        value.set_end_value(value_input);
                    }
                }
                AnimMotion::RendererTransform(value) => {
                    if let Some(targets) = input.renderer_names.as_deref() {
                        value.set_rend(targets.trim());
                    }
                    if let Some(position) = input.start_position {
                        value.set_start_tran(Vector4::new3(position[0], position[1], position[2]));
                    }
                    if let Some(position) = input.end_position {
                        value.set_end_tran(Vector4::new3(position[0], position[1], position[2]));
                    }
                }
                AnimMotion::ShowHide(value) => {
                    if let Some(targets) = input.renderer_names.as_deref() {
                        value.set_rend(targets.trim());
                    }
                    if let Some(value_input) = input.hide {
                        value.set_hide(value_input);
                    }
                    if let Some(value_input) = input.fade {
                        value.set_fade(value_input);
                    }
                    if let Some(value_input) = input.target_alpha {
                        value.set_tgt_alpha(value_input);
                    }
                }
                AnimMotion::Spin(value) => {
                    if let Some(value_input) = input.angle {
                        value.set_angle(value_input);
                    }
                    if let Some(axis) = input.axis {
                        value.set_axis(Vector4::new3(axis[0], axis[1], axis[2]));
                    }
                }
                AnimMotion::Slide(value) => {
                    if let Some(targets) = input.renderer_names.as_deref() {
                        value.set_rend(targets.trim());
                    }
                    if let Some(value_input) = input.hide {
                        value.set_hide(value_input);
                    }
                    if let Some(value_input) = input.direction {
                        value.set_direction(value_input);
                    }
                    if let Some(value_input) = input.distance {
                        value.set_distance(value_input);
                    }
                }
                AnimMotion::Noop(_) | AnimMotion::Unknown { .. } => {}
            }
        });
        if old_name != name {
            let referencing = animation
                .motions()
                .iter()
                .filter(|motion| motion.base().time_ref_name() == old_name)
                .map(|motion| motion.base().uid())
                .collect::<Vec<_>>();
            for sibling_uid in referencing {
                animation.edit_motion_by_uid(sibling_uid, "timeRefName", "", |motion| {
                    motion.base_mut().set_time_ref_name(name.clone());
                });
            }
        }
        animation
            .try_resolve_rel_time()
            .map_err(|error| error.to_string())?;
        self.set_active_animation_manager(&animation)?;
        self.reset_animation_playback();
        Ok(())
    }

    fn animation_duration(&self) -> f64 {
        self.active_animation_manager()
            .map(|animation| animation_seconds(animation.length()))
            .unwrap_or(0.0)
    }

    fn delete_animation_objects(&mut self, indices: &[usize]) -> Result<(), String> {
        self.restore_animation_playback(false)?;
        self.animation_elapsed = 0.0;
        let mut animation = self.active_animation_manager()?;
        let mut indices = indices.to_vec();
        indices.sort_unstable();
        indices.dedup();
        if indices.is_empty() {
            return Err("no animation objects are selected".to_string());
        }
        if let Some(index) = indices.iter().find(|index| **index >= animation.size()) {
            return Err(format!("animation object {index} was not found"));
        }
        let undo = self.capture_undo();
        let deleted_names = indices
            .iter()
            .filter_map(|index| animation.motions().get(*index))
            .map(|motion| motion.base().name().to_string())
            .collect::<BTreeSet<_>>();
        let deleted_references = indices
            .iter()
            .filter_map(|index| animation.motions().get(*index))
            .map(|motion| {
                (
                    motion.base().name().to_string(),
                    motion.base().time_ref_name().to_string(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let rewrites = animation
            .motions()
            .iter()
            .filter(|motion| !deleted_names.contains(motion.base().name()))
            .filter_map(|motion| {
                let original = motion.base().time_ref_name();
                if !deleted_names.contains(original) {
                    return None;
                }
                let mut reference = original.to_string();
                let mut visited = BTreeSet::new();
                while deleted_names.contains(&reference) && visited.insert(reference.clone()) {
                    reference = deleted_references
                        .get(&reference)
                        .cloned()
                        .unwrap_or_default();
                }
                Some((motion.base().uid(), reference))
            })
            .collect::<Vec<_>>();
        for (uid, reference) in rewrites {
            animation.edit_motion_by_uid(uid, "timeRefName", "", |motion| {
                motion.base_mut().set_time_ref_name(reference);
            });
        }
        for index in indices.iter().rev() {
            animation.remove_at(*index);
        }
        self.set_active_animation_manager(&animation)?;
        self.reset_animation_playback();
        self.status = if indices.len() == 1 {
            "Animation object deleted".to_string()
        } else {
            format!("Deleted {} animation objects", indices.len())
        };
        self.push_undo(undo);
        Ok(())
    }

    fn move_animation_objects(&mut self, indices: &[usize], direction: i32) -> Result<(), String> {
        self.restore_animation_playback(false)?;
        self.animation_elapsed = 0.0;
        if direction == 0 {
            return Err("animation move direction must be non-zero".to_string());
        }
        let mut animation = self.active_animation_manager()?;
        let mut indices = indices.to_vec();
        indices.sort_unstable();
        indices.dedup();
        if indices.is_empty() {
            return Err("no animation objects are selected".to_string());
        }
        if let Some(index) = indices.iter().find(|index| **index >= animation.size()) {
            return Err(format!("animation object {index} was not found"));
        }
        if (direction < 0 && indices[0] == 0)
            || (direction > 0 && indices[indices.len() - 1] + 1 == animation.size())
        {
            return Err("animation objects cannot move farther".to_string());
        }

        let undo = self.capture_undo();
        let mut motions = animation.motions().to_vec();
        let mut selected = vec![false; motions.len()];
        for index in &indices {
            selected[*index] = true;
        }
        if direction < 0 {
            for index in 1..motions.len() {
                if selected[index] && !selected[index - 1] {
                    motions.swap(index, index - 1);
                    selected.swap(index, index - 1);
                }
            }
        } else {
            for index in (0..motions.len().saturating_sub(1)).rev() {
                if selected[index] && !selected[index + 1] {
                    motions.swap(index, index + 1);
                    selected.swap(index, index + 1);
                }
            }
        }
        animation.clear();
        for motion in motions {
            animation.append_motion(motion);
        }
        self.set_active_animation_manager(&animation)?;
        self.reset_animation_playback();
        self.status = if indices.len() == 1 {
            "Animation object moved".to_string()
        } else {
            format!("Moved {} animation objects", indices.len())
        };
        self.push_undo(undo);
        Ok(())
    }

    fn reset_animation_playback(&mut self) {
        self.animation_elapsed = 0.0;
        self.animation_playing = false;
        self.animation_start_camera = None;
        self.animation_start_desktop = None;
    }

    fn begin_animation_playback(&mut self) -> Result<(), String> {
        if self.animation_start_desktop.is_none() {
            self.persist_camera()?;
            self.animation_start_camera = Some(self.camera.clone());
            self.animation_start_desktop = Some(self.desktop.clone());
        }
        Ok(())
    }

    fn restore_animation_playback(&mut self, preserve_camera: bool) -> Result<(), String> {
        let final_camera = self.camera.clone();
        if let Some(desktop) = self.animation_start_desktop.take() {
            self.desktop = desktop;
        }
        if preserve_camera {
            self.camera = final_camera;
            if self.desktop.active_view_summary().is_some() {
                self.desktop
                    .set_active_view_camera(&self.camera)
                    .map_err(|error| error.to_string())?;
            }
        } else if let Some(camera) = self.animation_start_camera.take() {
            self.camera = camera;
            if self.desktop.active_view_summary().is_some() {
                self.desktop
                    .set_active_view_camera(&self.camera)
                    .map_err(|error| error.to_string())?;
            }
        }
        self.animation_playing = false;
        self.invalidate_mesh(false);
        Ok(())
    }

    fn apply_active_animation_time(&mut self, elapsed: f64) -> Result<(), String> {
        let scene_uid = self
            .desktop
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        let view_uid = self
            .desktop
            .active_view_summary()
            .map(|view| view.uid)
            .ok_or_else(|| "no active view".to_string())?;
        let animation = self.scene_animation_manager(scene_uid)?;
        if animation.startcam().is_empty() {
            if let Some(camera) = self.animation_start_camera.as_ref() {
                self.desktop
                    .set_active_view_camera(camera)
                    .map_err(|error| error.to_string())?;
            }
        }
        self.camera = self
            .desktop
            .apply_scene_animation_time(scene_uid, animation_time(elapsed), view_uid)
            .map_err(|error| error.to_string())?;
        self.invalidate_mesh(false);
        Ok(())
    }

    fn tick_animation(&mut self, dt: f64) -> Result<(), String> {
        if !self.animation_playing {
            return Ok(());
        }
        let total = self.animation_duration();
        if total <= 0.0 {
            self.animation_playing = false;
            self.status = "No animation objects".to_string();
            return Ok(());
        }
        let animation = self.active_animation_manager()?;
        let step = dt.clamp(0.0, 0.25);
        let reaches_end = !animation.loop_enabled() && self.animation_elapsed + step >= total;
        let elapsed = if reaches_end {
            total
        } else if animation.loop_enabled() {
            (self.animation_elapsed + step) % total
        } else {
            self.animation_elapsed + step
        };
        self.apply_active_animation_time(elapsed)?;
        self.animation_elapsed = elapsed;
        if reaches_end {
            self.restore_animation_playback(true)?;
            self.animation_elapsed = total;
            self.status = "Animation finished".to_string();
        }
        Ok(())
    }

    fn capture_undo(&mut self) -> AppModelUndo {
        if self.animation_start_desktop.is_some() {
            let _ = self.restore_animation_playback(false);
            self.animation_elapsed = 0.0;
        }
        AppModelUndo {
            desktop: self.desktop.clone(),
            camera: self.camera.clone(),
            structure_revision: self.structure_revision,
            selection: self.selection.clone(),
            background: self.background.clone(),
            show_unit_cell: self.show_unit_cell,
            show_symmetry_mates: self.show_symmetry_mates,
            show_density_contour: self.show_density_contour,
            color_proofing: self.color_proofing,
            density_level: self.density_level,
            animation_elapsed: self.animation_elapsed,
            trans_mms: self.trans_mms,
            rot_mms: self.rot_mms,
            swap_stereo_eyes: self.swap_stereo_eyes,
            modified_scenes: self.modified_scenes.clone(),
        }
    }

    fn push_undo(&mut self, snapshot: AppModelUndo) {
        let description = if self.status.trim().is_empty() {
            "Edit".to_string()
        } else {
            self.status.clone()
        };
        self.undo_stack.push(AppModelUndoEntry {
            snapshot,
            description,
        });
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        if let Some(scene_uid) = self.desktop.active_scene() {
            self.modified_scenes.insert(scene_uid);
        }
        self.redo_stack.clear();
    }

    fn restore_undo(&mut self, snapshot: AppModelUndo) -> Result<(), String> {
        self.desktop = snapshot.desktop;
        self.camera = snapshot.camera;
        self.structure_revision = snapshot.structure_revision;
        self.selection = snapshot.selection;
        self.background = snapshot.background;
        self.show_unit_cell = snapshot.show_unit_cell;
        self.show_symmetry_mates = snapshot.show_symmetry_mates;
        self.show_density_contour = snapshot.show_density_contour;
        self.color_proofing = snapshot.color_proofing;
        self.density_level = snapshot.density_level;
        self.animation_elapsed = snapshot.animation_elapsed;
        self.trans_mms = snapshot.trans_mms;
        self.rot_mms = snapshot.rot_mms;
        self.swap_stereo_eyes = snapshot.swap_stereo_eyes;
        self.modified_scenes = snapshot.modified_scenes;
        self.animation_playing = false;
        self.animation_start_camera = None;
        self.animation_start_desktop = None;
        self.invalidate_mesh(false);
        Ok(())
    }

    fn undo(&mut self, depth: usize) -> Result<(), String> {
        let count = depth.saturating_add(1).min(self.undo_stack.len());
        if count == 0 {
            self.status = "Nothing to undo".to_string();
            return Ok(());
        }
        let mut description = String::new();
        for _ in 0..count {
            let entry = self.undo_stack.pop().expect("undo count is bounded");
            let redo = self.capture_undo();
            description = entry.description.clone();
            self.restore_undo(entry.snapshot)?;
            self.redo_stack.push(AppModelUndoEntry {
                snapshot: redo,
                description: entry.description,
            });
        }
        self.status = format!("Undo: {description}");
        Ok(())
    }

    fn redo(&mut self, depth: usize) -> Result<(), String> {
        let count = depth.saturating_add(1).min(self.redo_stack.len());
        if count == 0 {
            self.status = "Nothing to redo".to_string();
            return Ok(());
        }
        let mut description = String::new();
        for _ in 0..count {
            let entry = self.redo_stack.pop().expect("redo count is bounded");
            let undo = self.capture_undo();
            description = entry.description.clone();
            self.restore_undo(entry.snapshot)?;
            self.undo_stack.push(AppModelUndoEntry {
                snapshot: undo,
                description: entry.description,
            });
        }
        self.status = format!("Redo: {description}");
        Ok(())
    }

    fn clear_undo(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.status = "Undo data cleared".to_string();
    }
}

enum ModelRequest {
    StartRendererWarmup,
    GetSnapshot,
    GetMoleculeDetails {
        structure_key: String,
        structure_object_uid: Option<u64>,
        sequence_key: String,
        include_sequence: bool,
        object_uid: Option<u64>,
    },
    GetInitialState {
        width: u32,
        height: u32,
    },
    GetApplicationInfo,
    GetPreferences,
    SetPreferences(PreferencesDto),
    GetViewProperties,
    GetCameraProperties(String),
    SetViewProperties {
        properties: ViewPropertiesInput,
        width: u32,
        height: u32,
    },
    SetCameraProperties {
        old_name: String,
        properties: ViewPropertiesInput,
    },
    SetSceneProperties(ScenePropertiesInput),
    SetObjectProperties(ObjectPropertiesInput),
    SetRendererProperties(RendererPropertiesInput),
    SetDensityMapProperties(ObjectPropertiesInput),
    SetDensityRendererProperties(RendererPropertiesInput),
    SetDensityRendererVisible {
        map_uid: u64,
        renderer_uid: u64,
        visible: bool,
    },
    AddDensityRenderer {
        map_uid: u64,
        renderer_type: String,
        group_name: Option<String>,
    },
    GenerateDensitySurface {
        map_uid: u64,
        renderer_uid: u64,
    },
    SetSurfaceProperties(ObjectPropertiesInput),
    SetSurfaceRendererProperties(RendererPropertiesInput),
    SetLwObjectProperties(ObjectPropertiesInput),
    SetLwRendererProperties(LwRendererPropertiesInput),
    AddLwRenderer {
        object_uid: u64,
        group_name: Option<String>,
    },
    SetSurfaceRendererVisible {
        surface_uid: u64,
        renderer_uid: u64,
        visible: bool,
    },
    AddSurfaceRenderer {
        surface_uid: u64,
        group_name: Option<String>,
    },
    RegenerateSurface {
        surface_uid: u64,
        density: f64,
    },
    CreateCamera(String),
    SaveCamera(String),
    LoadCamera(String),
    SaveCameraView(String),
    LoadCameraView(String),
    LoadCameraFile {
        scene_uid: u64,
        path: String,
    },
    ReloadCameraFile {
        scene_uid: u64,
        name: String,
    },
    SaveCameraFile {
        scene_uid: u64,
        name: String,
        path: String,
    },
    DestroyCamera(String),
    GetCameraVisibility {
        scene_uid: u64,
        name: String,
    },
    SetCameraVisibility(CameraVisibilityUpdateInput),
    ClearCameraVisibility {
        scene_uid: u64,
        name: String,
    },
    CreateStyle(String),
    LoadStyleFile {
        scene_uid: u64,
        path: String,
    },
    ReloadStyleFile {
        scene_uid: u64,
        local_index: usize,
    },
    SaveStyleFile {
        scene_uid: u64,
        local_index: usize,
        path: String,
    },
    CreateRendererStyle(RendererStyleCreateInput),
    GetStyleProperties {
        scene_uid: u64,
        local_index: usize,
    },
    GetRendererStyleOptions {
        scene_uid: u64,
        renderer_type: String,
    },
    GetWorkspaceRendererSetupOptions {
        scene_uid: u64,
        kind: String,
    },
    GetAtomIntrDefs {
        object_uid: u64,
        renderer_uid: u64,
    },
    RemoveAtomIntrDefs {
        object_uid: u64,
        renderer_uid: u64,
        ids: Vec<usize>,
    },
    SetStyleProperties(StylePropertiesInput),
    DestroyStyle {
        scene_uid: u64,
        local_index: usize,
    },
    SetWorkspaceVisible {
        target: WorkspaceToggleInput,
        visible: bool,
    },
    SetWorkspaceItemsVisible {
        targets: Vec<WorkspaceToggleInput>,
        visible: bool,
    },
    SetWorkspaceLocked {
        target: WorkspaceToggleInput,
        locked: bool,
    },
    DeleteWorkspaceItem(WorkspaceDeleteInput),
    DeleteWorkspaceItems(Vec<WorkspaceDeleteInput>),
    MoveWorkspaceObject(WorkspaceMoveObjectInput),
    MoveWorkspaceRenderer(WorkspaceMoveRendererInput),
    MoveWorkspaceRenderers(WorkspaceMoveRenderersInput),
    SetWorkspaceObjectCollapsed(WorkspaceObjectCollapsedInput),
    SetWorkspaceRendererCollapsed(WorkspaceRendererCollapsedInput),
    SetWorkspaceRendererSelection(WorkspaceRendererSelectionInput),
    ChangeWorkspaceRendererType(WorkspaceRendererTypeInput),
    CreateWorkspaceRenderer(WorkspaceNewRendererInput),
    ApplyWorkspaceRendererStyle(WorkspaceRendererStyleInput),
    SetWorkspaceRendererStyles(WorkspaceRendererStylesInput),
    SetWorkspaceRendererColoring(WorkspaceRendererColoringInput),
    InsertWorkspaceRendererPaint(WorkspaceRendererPaintInput),
    InsertWorkspaceObjectPaint(WorkspaceObjectPaintInput),
    CopyWorkspaceRenderer(WorkspaceRendererCopyInput),
    CopyWorkspaceRenderers(Vec<WorkspaceRendererCopyInput>),
    PasteWorkspaceRenderers(WorkspaceRendererPasteInput),
    CopyWorkspaceObject(WorkspaceObjectCopyInput),
    PasteWorkspaceObject(WorkspaceObjectPasteInput),
    CopyWorkspaceCamera(WorkspaceCameraCopyInput),
    PasteWorkspaceCamera(WorkspaceCameraPasteInput),
    RenameWorkspaceCamera(WorkspaceCameraRenameInput),
    CopyWorkspaceStyle(WorkspaceStyleCopyInput),
    PasteWorkspaceStyle(WorkspaceStylePasteInput),
    CreateWorkspaceRendererGroup(WorkspaceRendererGroupInput),
    RenameWorkspaceRendererGroup(WorkspaceRendererGroupInput),
    RenameWorkspaceNode(WorkspaceRenameInput),
    FitWorkspaceItem {
        target: WorkspaceFitInput,
        width: u32,
        height: u32,
    },
    SetConfigValue {
        key: String,
        value: String,
    },
    Undo(usize),
    Redo(usize),
    ClearUndo,
    OpenPathsViewport {
        paths: Vec<String>,
        width: u32,
        height: u32,
    },
    OpenSceneViewport {
        path: String,
        width: u32,
        height: u32,
    },
    ReloadScene,
    GetFileRendererSetup {
        path: String,
    },
    OpenPathWithRendererSetup {
        path: String,
        target: crate::GuiFileRendererTarget,
        reader_options: Option<crate::GuiMoleculeReaderOptions>,
        namd_coor_options: Option<crate::GuiNamdCoorReaderOptions>,
        density_map_options: Option<crate::GuiDensityMapReaderOptions>,
        msms_options: Option<crate::GuiMsmsReaderOptions>,
        renderer_setup: crate::GuiRendererSetup,
        width: u32,
        height: u32,
    },
    GetPdbRendererSetup {
        pdb_id: String,
    },
    OpenPdb {
        pdb_id: String,
        files: Vec<crate::PdbDownloadedFile>,
        renderer_setup: Option<crate::GuiRendererSetup>,
        width: u32,
        height: u32,
    },
    LoadTrajectory {
        topology_path: String,
        trajectory_paths: Vec<String>,
        nevery: usize,
        renderer_setup: crate::GuiRendererSetup,
    },
    SetTrajectoryFrame {
        object_uid: u64,
        frame: usize,
    },
    SetTrajectoryFrameViewport {
        object_uid: u64,
        frame: usize,
        width: u32,
        height: u32,
    },
    AppendTrajectoryBlock {
        object_uid: u64,
        path: String,
        nevery: usize,
    },
    RemoveTrajectoryBlock {
        object_uid: u64,
        index: usize,
    },
    MoveTrajectoryBlock {
        object_uid: u64,
        from: usize,
        to: usize,
    },
    NewScene(Option<String>),
    NewTab {
        mode: String,
        name: String,
        inherit_view_props: bool,
    },
    ActivateScene(u64),
    ActivateView(u64),
    CloseActiveScene,
    SetSelection(String),
    MolStructAction {
        object_uid: u64,
        selection: String,
        mode: String,
        width: u32,
        height: u32,
    },
    SetRendererVisible {
        object_uid: u64,
        renderer_uid: u64,
        visible: bool,
    },
    AddRenderer {
        object_uid: u64,
        renderer_type: String,
        group_name: Option<String>,
    },
    SetObjectColoring {
        object_uid: u64,
        coloring: String,
    },
    SetColoring {
        input: ColoringUpdateInput,
        width: u32,
        height: u32,
    },
    DeleteAtoms {
        object_uid: u64,
        selection: String,
    },
    ChangeChain {
        object_uid: u64,
        selection: String,
        chain_name: String,
    },
    RenumberResidues {
        object_uid: u64,
        selection: String,
        start: i32,
    },
    MergeMolecules {
        source_object_uid: u64,
        selection: String,
        target_object_uid: u64,
        copy: bool,
    },
    SuperposeObjects {
        algorithm: String,
        reference_object_uid: u64,
        reference_selection: String,
        moving_object_uid: u64,
        moving_selection: String,
        auto_recenter: bool,
        use_property: bool,
    },
    EditBond {
        object_uid: u64,
        aid1: i32,
        aid2: i32,
        make: bool,
    },
    GetNonstandardBonds {
        object_uid: u64,
    },
    RemoveNonstandardBonds {
        object_uid: u64,
        pairs: Vec<[i32; 2]>,
    },
    CalculateContacts {
        object_uid: u64,
        selection1: String,
        object2_uid: Option<u64>,
        selection2: Option<String>,
        r_min: f64,
        r_max: f64,
        hbond: bool,
        max_count: usize,
        renderer_name: String,
    },
    GetSpaceGroups {
        lattice: String,
    },
    ChangeCrystalInfo {
        object_uid: u64,
        a: f64,
        b: f64,
        c: f64,
        alpha: f64,
        beta: f64,
        gamma: f64,
        space_group_number: i32,
    },
    AssignSecondaryStructure {
        object_uid: u64,
        ignore_bulge: bool,
        angle: f64,
    },
    CreateMolecularSurface {
        object_uid: u64,
        selection: String,
        name: String,
        density: f64,
        probe_radius: f64,
    },
    WriteSurface {
        object_uid: u64,
        selection: String,
        density: f64,
        probe_radius: f64,
        path: String,
    },
    WriteCutSurface {
        object_uid: u64,
        selection: String,
        density: f64,
        probe_radius: f64,
        normal: [f64; 3],
        position: [f64; 3],
        path: String,
    },
    CutSurface {
        surface_uid: u64,
        density: f64,
        mode: String,
    },
    WriteApbsFiles {
        object_uid: u64,
        selection: String,
        pqr_path: String,
        input_path: String,
        dx_path: String,
    },
    StartApbsCalculation(ApbsCalculationInput),
    PollApbsCalculation,
    CancelApbsCalculation,
    AppendMorphFrame {
        target_object_uid: u64,
        source_object_uid: u64,
        frame_name: String,
    },
    RemoveMorphFrame {
        object_uid: u64,
        index: usize,
    },
    ApplyMorphFrame {
        object_uid: u64,
        frame: f64,
    },
    SetDisplayOption {
        option: String,
        value: bool,
    },
    SetDensityLevel(f64),
    SetBackground(String),
    PickAtom {
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    },
    PickAtomIdentifier {
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    },
    SelectScreenRect {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        width: u32,
        height: u32,
    },
    AddAnimationObject {
        kind: String,
        after_index: Option<usize>,
    },
    AddConfiguredAnimationObject {
        kind: String,
        after_index: Option<usize>,
        properties: AnimationObjectPropertiesInput,
    },
    DeleteAnimationObject(usize),
    DeleteAnimationObjects(Vec<usize>),
    MoveAnimationObject {
        index: usize,
        direction: i32,
    },
    MoveAnimationObjects {
        indices: Vec<usize>,
        direction: i32,
    },
    SetAnimationDuration {
        index: usize,
        duration: f64,
    },
    SetAnimationObjectProperties(AnimationObjectPropertiesInput),
    AnimationControl(String),
    SetAnimationLoop(bool),
    SetAnimationStartCamera(String),
    SeekAnimation(f64),
    TickAnimation(f64),
    TickAnimationFrame {
        dt: f64,
        width: u32,
        height: u32,
    },
    ClearLog,
    ExecuteScriptFile(String),
    ExportPov {
        path: String,
    },
    StartPovRender(PovRenderInput),
    PollPovRender,
    CancelPovRender,
    SavePovRenderImage {
        path: String,
    },
    ClearPovRenderResult,
    StartAnimationRender(AnimationRenderInput),
    StartAnimationMovie {
        input: AnimationRenderInput,
        total_frames: u32,
    },
    PollAnimationRender,
    CancelAnimationRender,
    AnimationRenderPreview {
        output_dir: String,
        base_name: String,
        frame_index: u32,
    },
    Render {
        width: u32,
        height: u32,
    },
    UpdateCamera {
        action: String,
        x: f64,
        y: f64,
        width: u32,
        height: u32,
        render_scale: f64,
    },
    SaveScene {
        path: String,
        options: Option<crate::GuiSceneSaveOptions>,
    },
    SaveObject {
        object_uid: u64,
        object_kind: String,
        path: String,
    },
    SaveView(String),
    LoadView(String),
    ExportPng {
        path: String,
        width: u32,
        height: u32,
        resolution_dpi: f64,
        transparent: bool,
    },
}

enum ModelResponse {
    Snapshot(Box<AppSnapshot>),
    MoleculeDetails(Box<MoleculeDetailsDto>),
    InitialState(Box<InitialStateDto>),
    MolStructUpdate(Box<MolStructUpdateDto>),
    Viewport(ViewportDto),
    AnimationFrame(AnimationFrameDto),
    TrajectoryFrame(TrajectoryFrameDto),
    ApplicationInfo(ApplicationInfoDto),
    Preferences(Box<PreferencesDto>),
    PreferencesUpdate(Box<PreferencesUpdateDto>),
    ViewProperties(ViewPropertiesDto),
    ViewPropertiesUpdate(Box<ViewPropertiesUpdateDto>),
    StyleProperties(StylePropertiesDto),
    RendererStyleOptions(Vec<RendererStyleOptionDto>),
    AtomIntrDefs(Vec<AtomIntrDefDto>),
    FileRendererSetup(Option<RendererSetupDto>),
    PdbRendererSetup(RendererSetupDto),
    CameraVisibility(Vec<CameraVisibilityEntryDto>),
    PickedAtom(Option<PickedAtomDto>),
    Message(String),
}

struct ModelEnvelope {
    request: ModelRequest,
    response: mpsc::Sender<Result<ModelResponse, String>>,
}

struct ManagedState {
    sender: mpsc::Sender<ModelEnvelope>,
    startup_paths: Mutex<Option<Vec<PathBuf>>>,
}

fn nonempty_or(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn config_bool(desktop: &DesktopAppState, key: &str, fallback: bool) -> bool {
    match desktop.config_value(key).trim() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => fallback,
    }
}

fn config_value_is_true(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

fn scene_attr_bool(value: Option<&str>, fallback: bool) -> bool {
    value.map(config_value_is_true).unwrap_or(fallback)
}

fn scene_attr_f64(value: Option<&str>, fallback: f64) -> f64 {
    value
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

fn scene_attr_i32(value: Option<&str>, fallback: i32) -> i32 {
    value
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(fallback)
}

fn coloring_node_details(
    node: Option<&LDom2Node>,
) -> (Option<String>, BTreeMap<String, String>, Vec<PaintEntryDto>) {
    let Some(node) = node else {
        return (None, BTreeMap::new(), Vec::new());
    };
    let coloring_type = if node.type_name().is_empty() {
        let value = node.str_attr("type").to_string();
        (!value.is_empty()).then_some(value)
    } else {
        Some(node.type_name().to_string())
    };
    let mut attrs = node
        .children()
        .iter()
        .filter(|child| child.is_attr() && child.tag_name() != "type")
        .map(|child| (child.tag_name().to_string(), child.value().to_string()))
        .collect::<BTreeMap<_, _>>();
    if coloring_type.as_deref() == Some("ScriptColoring") {
        if let Some(script) = node
            .children()
            .iter()
            .find(|child| !child.is_attr() && child.tag_name() == "script")
        {
            let source = if !script.value().is_empty() {
                script.value()
            } else {
                script.contents()
            };
            attrs.insert("script".to_string(), source.to_string());
        }
    }
    let paint_entries = node
        .children()
        .iter()
        .filter(|child| !child.is_attr() && child.tag_name() == "paint")
        .filter_map(|child| {
            let selection = child.str_attr("sel").to_string();
            let color = child.str_attr("color").to_string();
            (!selection.is_empty() && !color.is_empty())
                .then_some(PaintEntryDto { selection, color })
        })
        .collect();
    (coloring_type, attrs, paint_entries)
}

fn renderer_coloring_details(
    renderer: &cuemol_molstr::MolRenderer,
    styles: &cuemol_qsys::StyleManager,
    scope_id: u64,
) -> (Option<String>, BTreeMap<String, String>, Vec<PaintEntryDto>) {
    let node = renderer
        .qsc_children()
        .iter()
        .find(|node| node.tag_name() == "coloring")
        .or_else(|| {
            renderer
                .style()
                .and_then(|style| styles.resolve_style_child_node(scope_id, style, "coloring"))
        });
    coloring_node_details(node)
}

fn renderer_specific_attributes(
    renderer: &cuemol_molstr::MolRenderer,
    renderer_type: &str,
    styles: &cuemol_qsys::StyleManager,
) -> BTreeMap<String, String> {
    let defaults: Vec<(&str, &str)> = match renderer_type {
        "*namelabel" | "name_label" => vec![
            ("maxlabel", "50"),
            ("dispx", "0"),
            ("dispy", "0"),
            ("color", "#ffff00"),
            ("font_size", "12"),
            ("font_name", "sans-serif"),
            ("font_style", "normal"),
            ("font_weight", "normal"),
        ],
        "*selection" | "selection" => vec![
            ("mode", "stick"),
            ("linew", "3"),
            ("dispx", "3"),
            ("dispy", "3"),
            ("color", "rgba(0,255,255,0.5)"),
        ],
        "atomintr" => vec![
            ("mode", "fancy"),
            ("showlabel", "false"),
            ("detail", "5"),
            ("stipple0", "1"),
            ("stipple1", "1"),
            ("stipple2", "-1"),
            ("stipple3", "-1"),
            ("stipple4", "-1"),
            ("stipple5", "-1"),
            ("width", "0.1"),
            ("color", "#ffff00"),
            ("captype_start", "sphere"),
            ("captype_end", "sphere"),
            ("arrowheight", "1"),
            ("arrowwidth", "2"),
            ("font_size", "12"),
            ("font_name", "sans-serif"),
            ("font_style", "normal"),
            ("font_weight", "normal"),
        ],
        "anisou" => vec![
            ("drawdisc", "true"),
            ("discscale", "1.1"),
            ("discthick", "0.1"),
            ("maxverts", "1000000"),
        ],
        "cpk" => vec![
            ("vdwr_H", "1.2"),
            ("vdwr_C", "1.7"),
            ("vdwr_N", "1.55"),
            ("vdwr_O", "1.52"),
            ("vdwr_S", "1.8"),
            ("vdwr_P", "1.8"),
            ("vdwr_X", "1.7"),
            ("detail", "10"),
            ("glrender_mode", "default"),
        ],
        "disorder" => vec![
            ("target", ""),
            ("detail", "5"),
            ("width", "0.3"),
            ("dotsep", "1"),
            ("loopsize", "2"),
            ("loopsize2", "-1"),
            ("defaultcolor", "#ffffff"),
        ],
        "dsurface" | "dsurf2" => vec![
            ("colormode", "molecule"),
            ("target", ""),
            ("detail", "6"),
            ("proberad", "1.4"),
            ("surftype", "ses"),
            ("cullface", "false"),
            ("drawmode", "fill"),
            ("width", "1.2"),
            ("showsel", "*"),
            ("vdwr_H", "1.2"),
            ("vdwr_C", "1.7"),
            ("vdwr_N", "1.55"),
            ("vdwr_O", "1.52"),
            ("vdwr_S", "1.8"),
            ("vdwr_P", "1.8"),
            ("vdwr_X", "1.7"),
            ("elepot", ""),
            ("lowpar", "-10"),
            ("midpar", "0"),
            ("highpar", "10"),
            ("highcol", "#0000ff"),
            ("midcol", "#ffffff"),
            ("lowcol", "#ff0000"),
            ("ramp_above", "false"),
            ("ramp_value", "1.4"),
        ],
        "ribbon" => vec![
            ("helix.type", "elliptical"),
            ("helix.width", "0.35"),
            ("helix.tuber", "1"),
            ("helix.sharp", "0.4"),
            ("helix.detail", "16"),
            ("sheet.type", "elliptical"),
            ("sheet.width", "0.35"),
            ("sheet.tuber", "1"),
            ("sheet.sharp", "0.4"),
            ("sheet.detail", "16"),
            ("coil.type", "elliptical"),
            ("coil.width", "0.35"),
            ("coil.tuber", "1"),
            ("coil.sharp", "0.4"),
            ("coil.detail", "16"),
            ("helixhead.type", "smooth"),
            ("helixhead.gamma", "2.2"),
            ("helixhead.basw", "0"),
            ("helixhead.arrow", "1.8"),
            ("helixtail.type", "smooth"),
            ("helixtail.gamma", "2.2"),
            ("helixtail.basw", "0"),
            ("helixtail.arrow", "1.8"),
            ("sheethead.type", "smooth"),
            ("sheethead.gamma", "2.2"),
            ("sheethead.basw", "0"),
            ("sheethead.arrow", "1.8"),
            ("sheettail.type", "smooth"),
            ("sheettail.gamma", "2.2"),
            ("sheettail.basw", "0"),
            ("sheettail.arrow", "1.8"),
            ("axialdetail", "6"),
            ("smoothcolor", "true"),
            ("pivotatom", ""),
            ("start_captype", "sphere"),
            ("end_captype", "sphere"),
            ("segend_fade", "false"),
            ("helix_smooth", "0"),
            ("sheet_smooth", "0.5"),
            ("coil_smooth", "0"),
            ("helix_usebackcol", "false"),
            ("helix_backcol", "#ffffff"),
            ("sheet_usesidecol", "false"),
            ("sheet_sidecol", "#ffffff"),
        ],
        "cartoon" => vec![
            ("helix.type", "elliptical"),
            ("helix.tuber", "1"),
            ("helix.sharp", "0.4"),
            ("helix.detail", "16"),
            ("sheet.type", "elliptical"),
            ("sheet.width", "0.35"),
            ("sheet.tuber", "1"),
            ("sheet.sharp", "0.4"),
            ("sheet.detail", "16"),
            ("coil.type", "elliptical"),
            ("coil.width", "0.35"),
            ("coil.tuber", "1"),
            ("coil.sharp", "0.4"),
            ("coil.detail", "16"),
            ("sheethead.type", "smooth"),
            ("sheethead.gamma", "2.2"),
            ("sheethead.basw", "0"),
            ("sheethead.arrow", "1.8"),
            ("ribhelix.type", "elliptical"),
            ("ribhelix.width", "0.35"),
            ("ribhelix.tuber", "1"),
            ("ribhelix.sharp", "0.4"),
            ("ribhelix.detail", "16"),
            ("ribhelix_head.type", "smooth"),
            ("ribhelix_head.gamma", "2.2"),
            ("ribhelix_head.basw", "0"),
            ("ribhelix_head.arrow", "1.8"),
            ("ribhelix_tail.type", "smooth"),
            ("ribhelix_tail.gamma", "2.2"),
            ("ribhelix_tail.basw", "0"),
            ("ribhelix_tail.arrow", "1.8"),
            ("axialdetail", "8"),
            ("smoothcolor", "false"),
            ("pivotatom", ""),
            ("start_captype", "flat"),
            ("end_captype", "flat"),
            ("segend_fade", "false"),
            ("anchor_sel", ""),
            ("anchor_weight", "10"),
            ("helix_ribbon", "false"),
            ("helix_smooth", "3"),
            ("helix_width_mode", "average"),
            ("helix_width", "2.3"),
            ("helix_extend", "0.5"),
            ("helix_wsmooth", "5"),
            ("helix_wplus", "0.2"),
            ("sheet_smooth", "2"),
            ("sheet_wsmooth", "5"),
            ("coil_smooth", "-1"),
        ],
        "tube" | "nucl" => {
            let mut defaults = vec![
                ("section.type", "elliptical"),
                ("section.width", "0.35"),
                ("section.tuber", "1"),
                ("section.sharp", "0.4"),
                ("section.detail", "16"),
                ("axialdetail", "6"),
                ("smooth", "0"),
                ("smoothcolor", "true"),
                ("pivotatom", ""),
                ("start_captype", "sphere"),
                ("end_captype", "sphere"),
                ("segend_fade", "false"),
                ("putty_mode", "none"),
                ("putty_tgt", "bfac"),
                ("putty_hiscl", "3"),
                ("putty_loscl", "3"),
            ];
            if renderer_type == "nucl" {
                defaults.extend([
                    ("show_tube", "true"),
                    ("show_basepair", "true"),
                    ("base_size", "0.5"),
                    ("base_detail", "3"),
                    ("base_thick", "0.5"),
                    ("base_type", "basepair"),
                ]);
            }
            defaults
        }
        _ => Vec::new(),
    };
    defaults
        .iter()
        .map(|(name, default)| {
            let value = if let Some((child, attribute)) = name.split_once('.') {
                if attribute == "type" {
                    renderer.resolved_child_type(child, styles, cuemol_molvis::DEFAULT_STYLE_SCOPE)
                } else {
                    renderer.resolved_child_attr(
                        child,
                        attribute,
                        styles,
                        cuemol_molvis::DEFAULT_STYLE_SCOPE,
                    )
                }
            } else {
                renderer.resolved_attr(name, styles, cuemol_molvis::DEFAULT_STYLE_SCOPE)
            };
            (
                (*name).to_string(),
                value.unwrap_or_else(|| (*default).to_string()),
            )
        })
        .collect()
}

fn background_rgba(value: &str) -> Result<[f64; 4], String> {
    let named = match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some([0, 0, 0, 255]),
        "white" => Some([255, 255, 255, 255]),
        "red" => Some([255, 0, 0, 255]),
        "green" => Some([0, 128, 0, 255]),
        "lime" => Some([0, 255, 0, 255]),
        "blue" => Some([0, 0, 255, 255]),
        "yellow" => Some([255, 255, 0, 255]),
        "cyan" | "aqua" => Some([0, 255, 255, 255]),
        "magenta" | "fuchsia" => Some([255, 0, 255, 255]),
        "gray" | "grey" => Some([128, 128, 128, 255]),
        "orange" => Some([255, 165, 0, 255]),
        _ => None,
    };
    let rgba = if let Some(rgba) = named {
        rgba
    } else {
        let color = cuemol_gfx::parse_color(value.trim()).map_err(|error| error.to_string())?;
        let rgba = color
            .rgba8()
            .ok_or_else(|| format!("unresolved background color: {value}"))?;
        [rgba.r, rgba.g, rgba.b, rgba.a]
    };
    Ok([
        f64::from(rgba[0]) / 255.0,
        f64::from(rgba[1]) / 255.0,
        f64::from(rgba[2]) / 255.0,
        f64::from(rgba[3]) / 255.0,
    ])
}

fn icc_intent_index(value: &str) -> Option<i32> {
    match value {
        "perceptual" => Some(0),
        "relative_colorimetric" => Some(1),
        "saturation" => Some(2),
        "absolute_colorimetric" => Some(3),
        _ => None,
    }
}

fn set_scene_attr_default(
    scene: &mut cuemol_molstr::MolScene,
    name: &str,
    value: &str,
    default_value: &str,
) {
    if value == default_value {
        scene.remove_scene_attribute(name);
    } else {
        scene.set_scene_attribute(name, value);
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_screen_space_ao(
    rgba: &mut [u8],
    depth: &[u8],
    width: u32,
    height: u32,
    radius_pixels: usize,
    intensity: f64,
    slices: i32,
    steps: i32,
) {
    let width = width as usize;
    let height = height as usize;
    if width < 3 || height < 3 || rgba.len() != width * height * 4 || depth.len() != rgba.len() {
        return;
    }
    let slices = slices.clamp(1, 64) as usize;
    let steps = steps.clamp(1, 32) as usize;
    let radius_pixels = radius_pixels.clamp(1, 64);
    let mut offsets = BTreeSet::new();
    for slice in 0..slices {
        let angle = std::f64::consts::TAU * slice as f64 / slices as f64;
        for step in 1..=steps {
            let distance = (radius_pixels * step).div_ceil(steps) as f64;
            let dx = (angle.cos() * distance).round() as isize;
            let dy = (angle.sin() * distance).round() as isize;
            if dx != 0 || dy != 0 {
                offsets.insert((dx, dy));
            }
        }
    }
    if offsets.is_empty() {
        return;
    }
    let strength = (intensity / 2.2).clamp(0.1, 3.0);
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 4;
            let center = depth[index] as f64;
            if center <= 1.0 {
                continue;
            }
            let mut occlusion = 0.0;
            let mut samples = 0usize;
            for &(dx, dy) in &offsets {
                let sx = x as isize + dx;
                let sy = y as isize + dy;
                if sx < 0 || sy < 0 || sx >= width as isize || sy >= height as isize {
                    continue;
                }
                let sample = depth[(sy as usize * width + sx as usize) * 4] as f64;
                if sample > center + 2.0 {
                    occlusion += ((sample - center) / 64.0).min(1.0);
                }
                samples += 1;
            }
            if samples == 0 {
                continue;
            }
            let shade = (1.0 - 0.65 * strength * occlusion / samples as f64).clamp(0.15, 1.0);
            for channel in &mut rgba[index..index + 3] {
                *channel = (f64::from(*channel) * shade).round() as u8;
            }
        }
    }
}

fn apply_fast_post_aa(rgba: &mut [u8], width: u32, height: u32, high_quality: bool) {
    let width = width as usize;
    let height = height as usize;
    if width < 3 || height < 3 || rgba.len() != width * height * 4 {
        return;
    }
    let row_stride = width * 4;
    let mut north = rgba[..row_stride].to_vec();
    let mut current = rgba[row_stride..row_stride * 2].to_vec();
    let mut south = rgba[row_stride * 2..row_stride * 3].to_vec();
    let threshold = if high_quality { 10 } else { 18 };
    let luma = |row: &[u8], index: usize| -> i32 {
        (i32::from(row[index]) * 54
            + i32::from(row[index + 1]) * 183
            + i32::from(row[index + 2]) * 19)
            / 256
    };
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let column = x * 4;
            let center_luma = luma(&current, column);
            let neighbor_luma = [
                luma(&current, column - 4),
                luma(&current, column + 4),
                luma(&north, column),
                luma(&south, column),
            ];
            let min_luma = neighbor_luma.into_iter().fold(center_luma, i32::min);
            let max_luma = neighbor_luma.into_iter().fold(center_luma, i32::max);
            if max_luma - min_luma < threshold {
                continue;
            }
            let target = (y * width + x) * 4;
            for channel in 0..3 {
                let sum = u32::from(current[column - 4 + channel])
                    + u32::from(current[column + 4 + channel])
                    + u32::from(north[column + channel])
                    + u32::from(south[column + channel]);
                rgba[target + channel] =
                    ((u32::from(current[column + channel]) * 2 + sum) / 6) as u8;
            }
        }
        std::mem::swap(&mut north, &mut current);
        std::mem::swap(&mut current, &mut south);
        if y + 2 < height {
            let start = (y + 2) * row_stride;
            south.copy_from_slice(&rgba[start..start + row_stride]);
        }
    }
}

fn validate_shortcuts(shortcuts: &[ShortcutDto]) -> Result<(), String> {
    const IDS: [&str; 2] = ["view-trans-plusx", "view-trans-minusx"];
    const KEYS: [&str; 4] = ["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"];
    if shortcuts.len() != IDS.len()
        || !IDS
            .iter()
            .all(|id| shortcuts.iter().any(|shortcut| shortcut.id == *id))
    {
        return Err("the complete reference shortcut set is required".to_string());
    }
    for shortcut in shortcuts {
        if !KEYS.contains(&shortcut.key.as_str()) {
            return Err(format!("unsupported shortcut key: {}", shortcut.key));
        }
    }
    for (index, shortcut) in shortcuts.iter().enumerate() {
        if shortcuts.iter().skip(index + 1).any(|other| {
            shortcut.key == other.key
                && shortcut.ctrl == other.ctrl
                && shortcut.alt == other.alt
                && shortcut.shift == other.shift
        }) {
            return Err(format!("duplicate shortcut: {}", shortcut.key));
        }
    }
    Ok(())
}

fn default_config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CUEMOL2_CONFIG_FILE") {
        return Some(PathBuf::from(path));
    }
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("CueMol2").join("config.tsv"));
    }
    if cfg!(target_os = "macos") {
        return std::env::var_os("HOME").map(PathBuf::from).map(|path| {
            path.join("Library")
                .join("Application Support")
                .join("CueMol2")
                .join("config.tsv")
        });
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|path| path.join(".config"))
        })
        .map(|path| path.join("cuemol2").join("config.tsv"))
}

fn vector3_array(value: Vector4) -> [f64; 3] {
    [value.x, value.y, value.z]
}

fn stereo_mode_name(value: StereoMode) -> &'static str {
    match value {
        StereoMode::None => "none",
        StereoMode::Parallel => "para",
        StereoMode::Cross => "cross",
        StereoMode::Hardware => "hardware",
    }
}

fn parse_stereo_mode(value: &str) -> Result<StereoMode, String> {
    match value {
        "none" => Ok(StereoMode::None),
        "para" | "parallel" => Ok(StereoMode::Parallel),
        "cross" => Ok(StereoMode::Cross),
        "hardware" => Ok(StereoMode::Hardware),
        _ => Err(format!("unknown stereo mode: {value}")),
    }
}

fn center_mark_name(value: CenterMark) -> &'static str {
    match value {
        CenterMark::None => "none",
        CenterMark::Cross => "crosshair",
        CenterMark::Axis => "axis",
    }
}

fn parse_center_mark(value: &str) -> Result<CenterMark, String> {
    match value {
        "none" => Ok(CenterMark::None),
        "cross" | "crosshair" => Ok(CenterMark::Cross),
        "axis" => Ok(CenterMark::Axis),
        _ => Err(format!("unknown center mark: {value}")),
    }
}

fn spawn_tauri_animation_frame(
    task: TauriAnimationFrameTask,
) -> Result<mpsc::Receiver<Result<TauriAnimationTaskOutput, String>>, String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("cuemol-tauri-animation-frame".to_string())
        .spawn(move || {
            let _ = sender.send(render_tauri_animation_frame(&task));
        })
        .map_err(|error| format!("failed to start animation frame renderer: {error}"))?;
    Ok(receiver)
}

fn render_tauri_animation_frame(
    task: &TauriAnimationFrameTask,
) -> Result<TauriAnimationTaskOutput, String> {
    if task.cancel.load(Ordering::Relaxed) {
        return Err("Animation rendering cancelled".to_string());
    }
    if task.input.backend == "povray" {
        render_tauri_pov_animation_frame(task)
    } else {
        render_tauri_umbreon_animation_frame(task)
    }
}

fn tauri_animation_frame_path(
    input: &AnimationRenderInput,
    index: u32,
    extension: &str,
) -> PathBuf {
    Path::new(input.output_dir.trim()).join(format!(
        "{}_frm_{index:04}.{extension}",
        input.base_name.trim()
    ))
}

fn update_tauri_animation_progress(task: &TauriAnimationFrameTask, frame_progress: f64) {
    let frames_weight = if task.input.make_movie { 0.95 } else { 1.0 };
    let completed = f64::from(task.index) + frame_progress.clamp(0.0, 1.0);
    let progress = ((completed / f64::from(task.total_frames.max(1))) * frames_weight * 1_000_000.0)
        .round()
        .clamp(0.0, 1_000_000.0) as u32;
    task.progress.fetch_max(progress, Ordering::Relaxed);
}

fn render_tauri_umbreon_animation_frame(
    task: &TauriAnimationFrameTask,
) -> Result<TauriAnimationTaskOutput, String> {
    let path = tauri_animation_frame_path(&task.input, task.index, "png");
    let mut exporter = UmbreonSceneExporter::new();
    exporter
        .handler_mut()
        .set_path(path.to_string_lossy().into_owned());
    exporter.set_width(task.input.width.min(i32::MAX as u32) as i32);
    exporter.set_height(task.input.height.min(i32::MAX as u32) as i32);
    exporter.set_denoise_mode(UmbreonDenoiseMode::default());
    exporter.set_render_input(task.snapshot.clone(), task.camera.clone(), task.background);
    exporter.begin_render().map_err(|error| error.to_string())?;
    while !exporter.is_render_done() {
        if task.cancel.load(Ordering::Relaxed) {
            exporter.cancel_render();
        }
        update_tauri_animation_progress(task, exporter.get_render_progress());
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let frame = exporter.end_render().map_err(|error| error.to_string())?;
    if frame.is_none() || task.cancel.load(Ordering::Relaxed) {
        return Err("Animation rendering cancelled".to_string());
    }
    update_tauri_animation_progress(task, 1.0);
    Ok(TauriAnimationTaskOutput::Frame {
        index: task.index,
        path,
        log: "Umbreon frame complete".to_string(),
    })
}

fn render_tauri_pov_animation_frame(
    task: &TauriAnimationFrameTask,
) -> Result<TauriAnimationTaskOutput, String> {
    let pov_path = tauri_animation_frame_path(&task.input, task.index, "pov");
    let png_path = tauri_animation_frame_path(&task.input, task.index, "png");
    let mesh = tessellate_snapshot(&task.snapshot);
    let mut exporter = PovSceneExporter::new();
    exporter.set_use_clip_z(task.input.pov_clip_plane);
    exporter.set_use_post_blend(task.input.pov_post_blend);
    exporter.set_show_edge_lines(task.input.pov_edge_lines);
    exporter
        .handler_mut()
        .set_path(pov_path.to_string_lossy().into_owned());
    exporter
        .write_scene_file_with_camera(&mesh, &task.camera)
        .map_err(|error| error.to_string())?;
    if task.cancel.load(Ordering::Relaxed) {
        return Err("Animation rendering cancelled".to_string());
    }
    update_tauri_animation_progress(task, 0.25);
    let process_task = tauri_animation_pov_process_task(&task.input, &pov_path, &png_path);
    let output = run_pov_process_with_poll(process_task, Arc::clone(&task.cancel), || {
        update_tauri_animation_progress(task, 0.5);
    })
    .map_err(|error| {
        if error == "POV-Ray rendering cancelled" {
            "Animation rendering cancelled".to_string()
        } else {
            error
        }
    })?;
    if task.input.pov_labels && !task.labels.is_empty() {
        overlay_pov_labels(&output.output_path, &task.labels)?;
    }
    let _ = fs::remove_file(pov_path);
    update_tauri_animation_progress(task, 1.0);
    Ok(TauriAnimationTaskOutput::Frame {
        index: task.index,
        path: output.output_path,
        log: if output.log.trim().is_empty() {
            "POV-Ray frame complete".to_string()
        } else {
            output.log
        },
    })
}

fn tauri_animation_pov_process_task(
    input: &AnimationRenderInput,
    pov_path: &Path,
    png_path: &Path,
) -> PovProcessTask {
    PovProcessTask {
        executable: input.povray_executable.clone(),
        include_dir: input.povray_include_dir.clone(),
        input_path: pov_path.to_path_buf(),
        output_path: png_path.to_path_buf(),
        options: PovProcessOptions {
            width: input.width,
            height: input.height,
            threads: 1,
            perspective: input.perspective,
            transparent: false,
            shadow: input.pov_shadow,
            radiosity: input.pov_radiosity,
            verbose: false,
            ..PovProcessOptions::default()
        },
    }
}

fn tauri_animation_movie_extension(format: &str) -> &'static str {
    match format {
        "mov_h264" | "mov_h265" | "mov_raw" => "mov",
        "wmv2" => "wmv",
        "gifanim" => "gif",
        _ => "mp4",
    }
}

fn spawn_tauri_animation_movie(
    task: TauriAnimationMovieTask,
) -> Result<mpsc::Receiver<Result<TauriAnimationTaskOutput, String>>, String> {
    for index in 0..task.total_frames {
        let path = tauri_animation_frame_path(&task.input, index, "png");
        if !path.is_file() {
            return Err(format!(
                "Movie encoding requires frame {}",
                path.to_string_lossy()
            ));
        }
    }
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("cuemol-tauri-animation-movie".to_string())
        .spawn(move || {
            let _ = sender.send(encode_tauri_animation_movie(&task));
        })
        .map_err(|error| format!("failed to start FFmpeg worker: {error}"))?;
    Ok(receiver)
}

fn encode_tauri_animation_movie(
    task: &TauriAnimationMovieTask,
) -> Result<TauriAnimationTaskOutput, String> {
    let extension = tauri_animation_movie_extension(&task.input.movie_format);
    let output_dir = PathBuf::from(task.input.output_dir.trim());
    let output_file = format!("{}.{}", task.input.base_name.trim(), extension);
    let output_path = output_dir.join(&output_file);
    let input_pattern = format!("{}_frm_%04d.png", task.input.base_name.trim());
    let mut command = Command::new(task.input.ffmpeg_executable.trim());
    command
        .current_dir(&output_dir)
        .args(["-y", "-framerate"])
        .arg(task.input.frame_rate.to_string())
        .args(["-start_number", "0", "-i"])
        .arg(input_pattern)
        .args(["-an", "-frames:v"])
        .arg(task.total_frames.to_string());
    match task.input.movie_format.as_str() {
        "mov_h265" => {
            command.args(["-c:v", "libx265", "-tag:v", "hvc1", "-pix_fmt", "yuv420p"]);
        }
        "mp4_h265" => {
            command.args(["-c:v", "libx265", "-pix_fmt", "yuv420p"]);
        }
        "mov_raw" => {
            command.args(["-c:v", "rawvideo"]);
        }
        "wmv2" => {
            command.args(["-c:v", "wmv2"]);
        }
        "gifanim" => {
            command.args(["-loop", "0"]);
        }
        _ => {
            command.args(["-c:v", "libx264", "-pix_fmt", "yuv420p"]);
        }
    }
    if task.input.movie_format != "mov_raw" {
        command
            .arg("-b:v")
            .arg(format!("{}k", task.input.bitrate_kbps));
    }
    command
        .arg(&output_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start FFmpeg: {error}"))?;
    let status = loop {
        if task.cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Animation rendering cancelled".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(error) => return Err(format!("FFmpeg wait failed: {error}")),
        }
    };
    if !status.success() {
        return Err(format!("FFmpeg exited with {status}"));
    }
    if !output_path.is_file() {
        return Err(format!(
            "FFmpeg did not create {}",
            output_path.to_string_lossy()
        ));
    }
    task.progress.store(1_000_000, Ordering::Relaxed);
    Ok(TauriAnimationTaskOutput::Movie {
        path: output_path,
        log: format!("FFmpeg exited with {status}"),
    })
}

fn animation_object_dto(index: usize, motion: &AnimMotion) -> AnimationObjectDto {
    let object = motion.base();
    let start = animation_seconds(object.abs_start());
    let end = animation_seconds(object.abs_end());
    let mut dto = AnimationObjectDto {
        index,
        name: object.name().to_string(),
        type_name: motion.type_name().to_string(),
        disabled: object.disabled(),
        time_ref_name: object.time_ref_name().to_string(),
        relative_start: animation_seconds(object.rel_start()),
        start,
        end,
        duration: (end - start).max(0.0),
        quadric: object.quadric(),
        angle: None,
        axis: None,
        target_camera: None,
        ignore_center: None,
        ignore_rotate: None,
        ignore_zoom: None,
        ignore_slab: None,
        keep_quaternion_positive: None,
        renderer_names: None,
        hide: None,
        fade: None,
        target_alpha: None,
        direction: None,
        distance: None,
        molecule_name: None,
        property_name: None,
        start_value: None,
        end_value: None,
        start_position: None,
        end_position: None,
    };
    match motion {
        AnimMotion::Camera(value) => {
            dto.target_camera = Some(value.endcam().to_string());
            dto.ignore_center = Some(value.ignorecenter());
            dto.ignore_rotate = Some(value.ignorerotate());
            dto.ignore_zoom = Some(value.ignorezoom());
            dto.ignore_slab = Some(value.ignoreslab());
            dto.keep_quaternion_positive = Some(value.keepquatpositive());
        }
        AnimMotion::RealProperty(value) => {
            dto.renderer_names = Some(value.rend().to_string());
            dto.property_name = Some(value.prop().to_string());
            dto.start_value = Some(value.start_value());
            dto.end_value = Some(value.end_value());
        }
        AnimMotion::Molecule(value) => {
            dto.molecule_name = Some(value.mol().to_string());
            dto.property_name = Some(value.prop().to_string());
            dto.start_value = Some(value.start_value());
            dto.end_value = Some(value.end_value());
        }
        AnimMotion::RendererTransform(value) => {
            let start_position = value.start_tran();
            let end_position = value.end_tran();
            dto.renderer_names = Some(value.rend().to_string());
            dto.property_name = Some(value.prop().to_string());
            dto.start_position = Some([start_position.x, start_position.y, start_position.z]);
            dto.end_position = Some([end_position.x, end_position.y, end_position.z]);
        }
        AnimMotion::ShowHide(value) => {
            dto.renderer_names = Some(value.rend().to_string());
            dto.hide = Some(value.hide());
            dto.fade = Some(value.fade());
            dto.target_alpha = Some(value.tgt_alpha());
        }
        AnimMotion::Spin(value) => {
            let axis = value.axis();
            dto.angle = Some(value.angle());
            dto.axis = Some([axis.x, axis.y, axis.z]);
        }
        AnimMotion::Slide(value) => {
            dto.renderer_names = Some(value.rend().to_string());
            dto.hide = Some(value.hide());
            dto.direction = Some(value.direction());
            dto.distance = Some(value.distance());
        }
        AnimMotion::Noop(_) | AnimMotion::Unknown { .. } => {}
    }
    dto
}

fn animation_seconds(value: i64) -> f64 {
    value as f64 / NANOS_PER_SECOND as f64
}

fn animation_time(seconds: f64) -> i64 {
    (seconds * NANOS_PER_SECOND as f64).round() as i64
}

fn animation_name_prefix(kind: AnimKind) -> &'static str {
    match kind {
        AnimKind::Spin => "SimpleSpin",
        AnimKind::CameraMotion => "CamMotion",
        AnimKind::Rock => "Rock",
        AnimKind::Pause => "NoopAnimObj",
        AnimKind::SlideIn => "SlideInAnim",
        AnimKind::SlideOut => "SlideOutAnim",
        AnimKind::ShowAnim => "ShowAnim",
        AnimKind::HideAnim => "HideAnim",
        AnimKind::MolAnim => "MolAnim",
    }
}

fn unique_animation_name(animation: &AnimMgr, prefix: &str) -> String {
    (1_u32..)
        .map(|index| format!("{prefix}{index}"))
        .find(|name| animation.get_by_name(name).is_none())
        .expect("animation name index is bounded by memory")
}

fn new_animation_motion(kind: AnimKind, name: String, reference: String) -> AnimMotion {
    let mut motion = match kind {
        AnimKind::Spin | AnimKind::Rock => {
            let mut value = SimpleSpin::new();
            value.set_angle(if kind == AnimKind::Rock { 45.0 } else { 360.0 });
            AnimMotion::Spin(value)
        }
        AnimKind::CameraMotion => AnimMotion::Camera(CamMotion::new()),
        AnimKind::Pause => AnimMotion::Noop(NoopAnimObj::new()),
        AnimKind::SlideIn | AnimKind::SlideOut => {
            let mut value = SlideInOutAnim::new();
            value.set_hide(kind == AnimKind::SlideOut);
            AnimMotion::Slide(value)
        }
        AnimKind::ShowAnim | AnimKind::HideAnim => {
            let mut value = ShowHideAnim::new();
            value.set_hide(kind == AnimKind::HideAnim);
            AnimMotion::ShowHide(value)
        }
        AnimKind::MolAnim => {
            let mut value = MolAnim::new();
            value.set_prop("frame");
            value.set_start_value(0.0);
            value.set_end_value(1.0);
            AnimMotion::Molecule(value)
        }
    };
    let object = motion.base_mut();
    object.set_name(name);
    object.set_time_ref_name(reference);
    object.set_rel_start(0);
    object.set_rel_end(NANOS_PER_SECOND);
    motion
}

fn animation_kind(value: &str) -> Option<AnimKind> {
    match value {
        "simple-spin" => Some(AnimKind::Spin),
        "camera-motion" => Some(AnimKind::CameraMotion),
        "rock" => Some(AnimKind::Rock),
        "show" => Some(AnimKind::ShowAnim),
        "hide" => Some(AnimKind::HideAnim),
        "slide-in" => Some(AnimKind::SlideIn),
        "slide-out" => Some(AnimKind::SlideOut),
        "mol-anim" => Some(AnimKind::MolAnim),
        "noop" => Some(AnimKind::Pause),
        _ => None,
    }
}

fn coloring_attrs_from_specific(
    specific: &BTreeMap<String, String>,
    names: &[&str],
) -> BTreeMap<String, String> {
    names
        .iter()
        .filter_map(|name| {
            specific
                .get(*name)
                .map(|value| ((*name).to_string(), value.clone()))
        })
        .collect()
}

fn scalar_coloring_type(specific: &BTreeMap<String, String>) -> Option<String> {
    match specific.get("colormode").map(String::as_str) {
        Some("potential" | "1") => Some("ElectrostaticColoring".to_string()),
        Some("multigrad" | "4") => Some("MultiGradientColoring".to_string()),
        _ => Some("SolidColoring".to_string()),
    }
}

fn scalar_coloring_attrs(specific: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    match specific.get("colormode").map(String::as_str) {
        Some("potential" | "1") => coloring_attrs_from_specific(
            specific,
            &[
                "elepot",
                "lowpar",
                "midpar",
                "highpar",
                "lowcol",
                "midcol",
                "highcol",
                "ramp_above",
                "ramp_value",
            ],
        ),
        Some("multigrad" | "4") => {
            coloring_attrs_from_specific(specific, &["color_mapname", "ramp_above", "ramp_value"])
        }
        _ => BTreeMap::new(),
    }
}

fn aux_renderer_group_dto(group: crate::GuiAuxRendererGroupSummary) -> RendererDto {
    RendererDto {
        uid: group.uid.get(),
        name: group.name,
        renderer_type: "*group".to_string(),
        group: None,
        ui_collapsed: group.ui_collapsed,
        selection: String::new(),
        visible: group.visible,
        locked: group.locked,
        style: group.style,
        material: group.material,
        alpha: group.alpha,
        edge_type: group.edge_type,
        edge_width: group.edge_width,
        edge_color: group.edge_color,
        line_width: 1.2,
        detail: 3,
        bond_width: 0.2,
        sphere_radius: 0.3,
        show_ring: false,
        ring_thickness: 0.2,
        ring_color: "#ffffbf".to_string(),
        supports_coloring: false,
        supports_solid_coloring: false,
        supports_electrostatic_coloring: false,
        supports_multi_gradient: false,
        coloring_type: None,
        scheme_coloring_type: None,
        coloring_attrs: BTreeMap::new(),
        paint_entries: Vec::new(),
        gradient_entries: Vec::new(),
        default_color: "#c0c0ff".to_string(),
        specific: BTreeMap::new(),
    }
}

fn aux_lw_renderer_group_dto(group: crate::GuiAuxRendererGroupSummary) -> LwRendererDto {
    LwRendererDto {
        uid: group.uid.get(),
        name: group.name,
        renderer_type: "*group".to_string(),
        group: None,
        ui_collapsed: group.ui_collapsed,
        visible: group.visible,
        locked: group.locked,
        style: group.style.clone(),
        material: group.material,
        alpha: group.alpha,
        edge_type: group.edge_type,
        edge_width: group.edge_width,
        edge_color: group.edge_color,
        data_id: 0,
        specific: BTreeMap::from([("style".to_string(), group.style)]),
    }
}

fn ordered_renderer_dtos<T>(mut entries: Vec<(i32, u64, T)>) -> Vec<T> {
    entries.sort_by_key(|(ui_order, uid, _)| (*ui_order, *uid));
    entries
        .into_iter()
        .map(|(_, _, renderer)| renderer)
        .collect()
}

fn validate_coloring_update(input: &ColoringUpdateInput) -> Result<(), String> {
    if !matches!(
        input.kind.as_str(),
        "object" | "renderer" | "surfaceRenderer" | "densityRenderer"
    ) {
        return Err(format!("unsupported coloring target: {}", input.kind));
    }
    if input.kind != "object" && input.renderer_uid.is_none() {
        return Err("renderer coloring requires a renderer UID".to_string());
    }
    let Some(coloring_type) = input.coloring_type.as_deref() else {
        if !input.attrs.is_empty()
            || !input.paint_entries.is_empty()
            || !input.gradient_entries.is_empty()
            || input.default_color.is_some()
        {
            return Err("reset coloring cannot contain properties".to_string());
        }
        return Ok(());
    };
    let scalar_target = matches!(input.kind.as_str(), "surfaceRenderer" | "densityRenderer");
    if scalar_target
        && !matches!(
            coloring_type,
            "SolidColoring" | "ElectrostaticColoring" | "MultiGradientColoring"
        )
    {
        return Err(format!("unsupported scalar coloring type: {coloring_type}"));
    }
    if input.kind == "object"
        && matches!(
            coloring_type,
            "ElectrostaticColoring" | "MultiGradientColoring"
        )
    {
        return Err(format!("unsupported object coloring type: {coloring_type}"));
    }
    if input.kind == "densityRenderer" && coloring_type == "ElectrostaticColoring" {
        return Err("density renderers do not support electrostatic coloring".to_string());
    }
    if input.kind == "renderer" && coloring_type == "MultiGradientColoring" {
        return Err("molecular renderers do not support multi-gradient coloring".to_string());
    }
    let allowed_attrs: &[&str] = match coloring_type {
        "SolidColoring" | "PaintColoring" => &[],
        "CPKColoring" => &[
            "col_H", "col_C", "col_N", "col_O", "col_P", "col_S", "col_X",
        ],
        "RainbowColoring" => &["mode", "incr_mode", "start_hue", "end_hue", "sat", "bri"],
        "BfacColoring" => &["mode", "auto", "lowcol", "highcol", "lowpar", "highpar"],
        "ScriptColoring" => &["script"],
        "ElectrostaticColoring" => &[
            "elepot",
            "lowpar",
            "midpar",
            "highpar",
            "lowcol",
            "midcol",
            "highcol",
            "ramp_above",
            "ramp_value",
        ],
        "MultiGradientColoring" => &["color_mapname", "ramp_above", "ramp_value"],
        _ => return Err(format!("unsupported coloring type: {coloring_type}")),
    };
    if let Some(name) = input
        .attrs
        .keys()
        .find(|name| !allowed_attrs.contains(&name.as_str()))
    {
        return Err(format!("unsupported {coloring_type} property: {name}"));
    }
    for name in [
        "col_H", "col_C", "col_N", "col_O", "col_P", "col_S", "col_X", "lowcol", "midcol",
        "highcol",
    ] {
        if let Some(color) = input.attrs.get(name) {
            cuemol_gfx::parse_color(color).map_err(|error| error.to_string())?;
        }
    }
    for name in [
        "start_hue",
        "end_hue",
        "sat",
        "bri",
        "lowpar",
        "midpar",
        "highpar",
        "ramp_value",
    ] {
        if let Some(value) = input.attrs.get(name) {
            let value = value
                .parse::<f64>()
                .map_err(|_| format!("{name} must be numeric"))?;
            if !value.is_finite() {
                return Err(format!("{name} must be finite"));
            }
            if matches!(name, "start_hue" | "end_hue") && !(0.0..=360.0).contains(&value) {
                return Err(format!("{name} must be between 0 and 360"));
            }
            if matches!(name, "sat" | "bri") && !(0.0..=1.0).contains(&value) {
                return Err(format!("{name} must be between 0 and 1"));
            }
        }
    }
    if let Some(mode) = input.attrs.get("mode") {
        let valid = match coloring_type {
            "RainbowColoring" => matches!(mode.as_str(), "mol" | "chain"),
            "BfacColoring" => matches!(mode.as_str(), "bfac" | "occ" | "center"),
            _ => true,
        };
        if !valid {
            return Err(format!("invalid {coloring_type} mode: {mode}"));
        }
    }
    if let Some(mode) = input.attrs.get("incr_mode") {
        if !matches!(mode.as_str(), "chain" | "resid" | "protss") {
            return Err(format!("invalid rainbow increment mode: {mode}"));
        }
    }
    if let Some(mode) = input.attrs.get("auto") {
        if !matches!(mode.as_str(), "none" | "mol" | "rend") {
            return Err(format!("invalid B-factor auto mode: {mode}"));
        }
    }
    if let Some(value) = input.attrs.get("ramp_above") {
        if !matches!(value.as_str(), "true" | "false") {
            return Err("ramp_above must be true or false".to_string());
        }
    }
    if let Some(color) = input.default_color.as_deref() {
        cuemol_gfx::parse_color(color).map_err(|error| error.to_string())?;
    }
    if coloring_type != "PaintColoring" && !input.paint_entries.is_empty() {
        return Err("paint entries require PaintColoring".to_string());
    }
    for entry in &input.paint_entries {
        cuemol_molstr::parse_selector(&entry.selection).map_err(|error| error.to_string())?;
        cuemol_gfx::parse_color(&entry.color).map_err(|error| error.to_string())?;
    }
    if coloring_type != "MultiGradientColoring" && !input.gradient_entries.is_empty() {
        return Err("gradient entries require MultiGradientColoring".to_string());
    }
    let mut gradient_values = BTreeSet::new();
    for entry in &input.gradient_entries {
        if !entry.value.is_finite() {
            return Err("multi-gradient values must be finite".to_string());
        }
        if !gradient_values.insert(entry.value.to_bits()) {
            return Err(format!("duplicate multi-gradient value: {}", entry.value));
        }
        cuemol_gfx::parse_color(&entry.color).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn apply_open_paths(model: &mut AppModel, paths: Vec<String>) -> Result<(), String> {
    model.start_renderer_warmup();
    model.restore_animation_playback(false)?;
    model.animation_elapsed = 0.0;
    let previous_scene = model.desktop.active_scene();
    let undo = model.capture_undo();
    model.open_paths(paths)?;
    if model.desktop.active_scene() != previous_scene {
        model.reset_animation_playback();
    }
    model.sync_active_scene_properties();
    let loaded_scene = model
        .desktop
        .active_scene_source_path()
        .map(|source| source.to_string_lossy())
        .is_some_and(|source| model.opened_paths.iter().any(|path| path == &source));
    if loaded_scene {
        model.undo_stack.clear();
        model.redo_stack.clear();
        if let Some(scene_uid) = model.desktop.active_scene() {
            model.modified_scenes.remove(&scene_uid);
        }
    } else {
        model.push_undo(undo);
    }
    model.touch_structure_details();
    Ok(())
}

fn renderer_setup_defaults(
    model: &mut AppModel,
    base_object_name: &str,
    reader_options: Option<&crate::GuiMoleculeReaderOptions>,
    namd_coor_options: Option<crate::GuiNamdCoorReaderOptions>,
) -> Result<RendererSetupDto, String> {
    let scene_uid = model.desktop.ensure_active_scene();
    let options = model
        .desktop
        .molecule_renderer_setup_options(scene_uid)
        .map_err(|error| error.to_string())?;
    let preferences = crate::load_renderer_setup_preferences();
    let renderer_type = preferences
        .renderer_type
        .filter(|renderer_type| options.iter().any(|option| option.name == *renderer_type))
        .or_else(|| options.first().map(|option| option.name.clone()))
        .ok_or_else(|| "no compatible molecular renderers are available".to_string())?;
    let object_name = model
        .desktop
        .suggest_object_name(scene_uid, base_object_name)
        .map_err(|error| error.to_string())?;
    let renderer_name = model
        .desktop
        .suggest_renderer_name(scene_uid, &renderer_type)
        .map_err(|error| error.to_string())?;
    Ok(RendererSetupDto {
        target_kind: "molecule".to_string(),
        object_name,
        renderer_type,
        renderer_name,
        recenter: preferences.recenter,
        reader_options: reader_options.map(MoleculeReaderOptionsDto::from),
        namd_coor_options: namd_coor_options.map(NamdCoorReaderOptionsDto::from),
        msms_options: None,
        ccp4_map_options: None,
        mtz_map_options: None,
        options: options
            .into_iter()
            .map(|option| RendererSetupOptionDto {
                name: option.name,
                description: option.description,
                preset: option.preset,
            })
            .collect(),
    })
}

fn density_map_renderer_setup_defaults(
    model: &mut AppModel,
    base_object_name: &str,
    reader_options: Option<crate::GuiDensityMapReaderOptions>,
) -> Result<RendererSetupDto, String> {
    let scene_uid = model.desktop.ensure_active_scene();
    let options = model.desktop.density_map_renderer_setup_options();
    let preferences = crate::load_renderer_setup_preferences();
    let renderer_type = preferences
        .renderer_type
        .filter(|renderer_type| options.iter().any(|option| option.name == *renderer_type))
        .or_else(|| options.first().map(|option| option.name.clone()))
        .ok_or_else(|| "no compatible density-map renderers are available".to_string())?;
    let object_name = model
        .desktop
        .suggest_object_name(scene_uid, base_object_name)
        .map_err(|error| error.to_string())?;
    let renderer_name = model
        .desktop
        .suggest_renderer_name(scene_uid, &renderer_type)
        .map_err(|error| error.to_string())?;
    let (ccp4_map_options, mtz_map_options) = match reader_options {
        Some(crate::GuiDensityMapReaderOptions::Ccp4(options)) => {
            (Some(Ccp4MapReaderOptionsDto::from(options)), None)
        }
        Some(crate::GuiDensityMapReaderOptions::Mtz(options)) => {
            (None, Some(MtzMapReaderOptionsDto::from(options)))
        }
        None => (None, None),
    };
    Ok(RendererSetupDto {
        target_kind: "densityMap".to_string(),
        object_name,
        renderer_type,
        renderer_name,
        recenter: false,
        reader_options: None,
        namd_coor_options: None,
        msms_options: None,
        ccp4_map_options,
        mtz_map_options,
        options: options
            .into_iter()
            .map(|option| RendererSetupOptionDto {
                name: option.name,
                description: option.description,
                preset: option.preset,
            })
            .collect(),
    })
}

fn surface_renderer_setup_defaults(
    model: &mut AppModel,
    base_object_name: &str,
    reader_options: Option<crate::GuiMsmsReaderOptions>,
) -> Result<RendererSetupDto, String> {
    let scene_uid = model.desktop.ensure_active_scene();
    let options = model.desktop.surface_renderer_setup_options();
    let preferences = crate::load_renderer_setup_preferences();
    let renderer_type = options
        .first()
        .map(|option| option.name.clone())
        .ok_or_else(|| "no compatible surface renderers are available".to_string())?;
    let object_name = model
        .desktop
        .suggest_object_name(scene_uid, base_object_name)
        .map_err(|error| error.to_string())?;
    let renderer_name = model
        .desktop
        .suggest_renderer_name(scene_uid, &renderer_type)
        .map_err(|error| error.to_string())?;
    Ok(RendererSetupDto {
        target_kind: "surface".to_string(),
        object_name,
        renderer_type,
        renderer_name,
        recenter: preferences.recenter,
        reader_options: None,
        namd_coor_options: None,
        msms_options: reader_options.map(MsmsReaderOptionsDto::from),
        ccp4_map_options: None,
        mtz_map_options: None,
        options: options
            .into_iter()
            .map(|option| RendererSetupOptionDto {
                name: option.name,
                description: option.description,
                preset: option.preset,
            })
            .collect(),
    })
}

fn handle_model_request(
    model: &mut AppModel,
    request: ModelRequest,
) -> Result<ModelResponse, String> {
    match request {
        ModelRequest::StartRendererWarmup => {
            model.start_renderer_warmup();
            Ok(ModelResponse::Message(
                "Renderer warmup started".to_string(),
            ))
        }
        ModelRequest::GetSnapshot => Ok(ModelResponse::Snapshot(Box::new(model.snapshot()))),
        ModelRequest::GetMoleculeDetails {
            structure_key,
            structure_object_uid,
            sequence_key,
            include_sequence,
            object_uid,
        } => Ok(ModelResponse::MoleculeDetails(Box::new(
            model.molecule_details(
                &structure_key,
                structure_object_uid,
                &sequence_key,
                include_sequence,
                object_uid,
            ),
        ))),
        ModelRequest::GetInitialState { width, height } => {
            let viewport = model.viewport_metadata(width, height);
            Ok(ModelResponse::InitialState(Box::new(InitialStateDto {
                snapshot: model.snapshot(),
                viewport,
                preferences: model.preferences(),
                startup_paths_pending: false,
            })))
        }
        ModelRequest::GetApplicationInfo => {
            Ok(ModelResponse::ApplicationInfo(model.application_info()))
        }
        ModelRequest::GetPreferences => {
            Ok(ModelResponse::Preferences(Box::new(model.preferences())))
        }
        ModelRequest::SetPreferences(preferences) => {
            model.set_preferences(preferences)?;
            Ok(ModelResponse::PreferencesUpdate(Box::new(
                PreferencesUpdateDto {
                    preferences: model.preferences(),
                    status: model.status.clone(),
                },
            )))
        }
        ModelRequest::GetViewProperties => {
            Ok(ModelResponse::ViewProperties(model.view_properties()))
        }
        ModelRequest::GetCameraProperties(name) => Ok(ModelResponse::ViewProperties(
            model.named_camera_properties(&name)?,
        )),
        ModelRequest::SetViewProperties {
            properties,
            width,
            height,
        } => {
            let undo = model.capture_undo();
            model.set_view_properties(properties)?;
            model.push_undo(undo);
            let viewport = model.render(width, height);
            Ok(ModelResponse::ViewPropertiesUpdate(Box::new(
                ViewPropertiesUpdateDto {
                    properties: model.view_properties(),
                    snapshot: model.snapshot(),
                    viewport,
                },
            )))
        }
        ModelRequest::SetCameraProperties {
            old_name,
            properties,
        } => {
            let undo = model.capture_undo();
            model.set_named_camera_properties(&old_name, properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetSceneProperties(properties) => {
            let undo = model.capture_undo();
            model.set_scene_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetObjectProperties(properties) => {
            let undo = model.capture_undo();
            model.set_object_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetRendererProperties(properties) => {
            let undo = model.capture_undo();
            model.set_renderer_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetDensityMapProperties(properties) => {
            let undo = model.capture_undo();
            model.set_density_map_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetDensityRendererProperties(properties) => {
            let undo = model.capture_undo();
            model.set_density_renderer_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetDensityRendererVisible {
            map_uid,
            renderer_uid,
            visible,
        } => {
            let undo = model.capture_undo();
            model.set_density_renderer_visible(map_uid, renderer_uid, visible)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AddDensityRenderer {
            map_uid,
            renderer_type,
            group_name,
        } => {
            let undo = model.capture_undo();
            model.add_density_renderer(map_uid, &renderer_type, group_name.as_deref())?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::GenerateDensitySurface {
            map_uid,
            renderer_uid,
        } => {
            let undo = model.capture_undo();
            let surface_uid = model
                .desktop
                .generate_density_surface_object(Uid::new(map_uid), Uid::new(renderer_uid))
                .map_err(|error| error.to_string())?;
            model.status = format!("Generated surface object {}", surface_uid.get());
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetSurfaceProperties(properties) => {
            let undo = model.capture_undo();
            model.set_surface_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetSurfaceRendererProperties(properties) => {
            let undo = model.capture_undo();
            model.set_surface_renderer_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetLwObjectProperties(properties) => {
            let undo = model.capture_undo();
            model.set_lw_object_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetLwRendererProperties(properties) => {
            let undo = model.capture_undo();
            model.set_lw_renderer_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AddLwRenderer {
            object_uid,
            group_name,
        } => {
            let undo = model.capture_undo();
            model.add_lw_renderer(object_uid, group_name.as_deref())?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetSurfaceRendererVisible {
            surface_uid,
            renderer_uid,
            visible,
        } => {
            let undo = model.capture_undo();
            model.set_surface_renderer_visible(surface_uid, renderer_uid, visible)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AddSurfaceRenderer {
            surface_uid,
            group_name,
        } => {
            let undo = model.capture_undo();
            model.add_surface_renderer(surface_uid, group_name.as_deref())?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RegenerateSurface {
            surface_uid,
            density,
        } => {
            let undo = model.capture_undo();
            let triangle_count = model
                .desktop
                .regenerate_surface_object_with_density(Uid::new(surface_uid), density)
                .map_err(|error| error.to_string())?;
            model.status = format!("Regenerated surface: {triangle_count} triangle(s)");
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CreateCamera(name) => {
            let name = name.trim();
            if name.is_empty() {
                return Err("camera name is required".to_string());
            }
            let scene = model
                .desktop
                .active_scene()
                .and_then(|scene_uid| model.desktop.scene_manager().scene(scene_uid))
                .ok_or_else(|| "no active scene".to_string())?;
            if scene.camera_by_name(name).is_some() {
                return Err(format!("camera already exists: {name}"));
            }
            let undo = model.capture_undo();
            model.save_named_camera(name, false)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SaveCamera(name) => {
            let undo = model.capture_undo();
            model.save_named_camera(&name, true)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::LoadCamera(name) => {
            let has_visibility = model
                .desktop
                .active_camera_summaries()
                .iter()
                .find(|camera| camera.name == name)
                .is_some_and(|camera| camera.visibility_count > 0);
            let mut undo = has_visibility.then(|| model.capture_undo());
            model.load_named_camera(&name, true)?;
            if let Some(undo) = undo.as_mut() {
                undo.camera = model.camera.clone();
            }
            if let Some(undo) = undo {
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SaveCameraView(name) => {
            let undo = model.capture_undo();
            model.save_named_camera(&name, false)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::LoadCameraView(name) => {
            model.load_named_camera(&name, false)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::LoadCameraFile { scene_uid, path } => {
            let scene_uid = Uid::new(scene_uid);
            if model.desktop.active_scene() != Some(scene_uid) {
                return Err("camera file target scene is not active".to_string());
            }
            let mut undo = model.capture_undo();
            let name = model
                .desktop
                .load_scene_camera_file(scene_uid, &path)
                .map_err(|error| error.to_string())?;
            model.camera = model
                .desktop
                .load_scene_camera(scene_uid, &name, true)
                .map_err(|error| error.to_string())?;
            undo.camera = model.camera.clone();
            model.invalidate_mesh(false);
            model.status = format!("Load camera file {name}");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ReloadCameraFile { scene_uid, name } => {
            let scene_uid = Uid::new(scene_uid);
            let undo = model.capture_undo();
            model
                .desktop
                .reload_scene_camera_file(scene_uid, &name)
                .map_err(|error| error.to_string())?;
            model.status = format!("Reload camera file {name}");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SaveCameraFile {
            scene_uid,
            name,
            path,
        } => {
            let scene_uid = Uid::new(scene_uid);
            let undo = model.capture_undo();
            model
                .desktop
                .save_scene_camera_file(scene_uid, &name, &path)
                .map_err(|error| error.to_string())?;
            model.status = format!("Save camera file {name}");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::DestroyCamera(name) => {
            let undo = model.capture_undo();
            model.destroy_named_camera(&name)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::GetCameraVisibility { scene_uid, name } => {
            let entries = model
                .desktop
                .camera_visibility_entries(Uid::new(scene_uid), &name)
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|entry| CameraVisibilityEntryDto {
                    target_uid: entry.target_uid.get(),
                    parent_uid: entry.parent_uid.map(Uid::get),
                    name: entry.name,
                    type_name: entry.type_name,
                    is_object: entry.is_object,
                    included: entry.included,
                    visible: entry.visible,
                })
                .collect();
            Ok(ModelResponse::CameraVisibility(entries))
        }
        ModelRequest::SetCameraVisibility(input) => {
            let undo = model.capture_undo();
            let scene_uid = Uid::new(input.scene_uid);
            let settings = input
                .entries
                .into_iter()
                .map(|entry| (Uid::new(entry.target_uid), entry.visible, entry.is_object))
                .collect::<Vec<_>>();
            let count = model
                .desktop
                .set_scene_camera_visibility_settings(scene_uid, &input.name, &settings)
                .map_err(|error| error.to_string())?;
            model.modified_scenes.insert(scene_uid);
            model.status = format!("Change visflags of {}: {count} item(s)", input.name);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ClearCameraVisibility { scene_uid, name } => {
            let undo = model.capture_undo();
            let scene_uid = Uid::new(scene_uid);
            let count = model
                .desktop
                .clear_scene_camera_visibility_settings(scene_uid, &name)
                .map_err(|error| error.to_string())?;
            model.modified_scenes.insert(scene_uid);
            model.status = format!("Clear visibility flags in {name}: {count} item(s)");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CreateStyle(name) => {
            let undo = model.capture_undo();
            model.create_named_style(&name)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::LoadStyleFile { scene_uid, path } => {
            let scene_uid = Uid::new(scene_uid);
            let undo = model.capture_undo();
            let local_index = model
                .desktop
                .load_scene_style_file(scene_uid, &path, true)
                .map_err(|error| error.to_string())?;
            model.status = format!("Loaded style file {path}");
            if model.desktop.active_scene() == Some(scene_uid) {
                model.invalidate_mesh(false);
            }
            model.push_undo(undo);
            debug_assert!(model
                .desktop
                .style_summaries(scene_uid)
                .iter()
                .any(|style| style.local_index == Some(local_index)));
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ReloadStyleFile {
            scene_uid,
            local_index,
        } => {
            let scene_uid = Uid::new(scene_uid);
            let undo = model.capture_undo();
            let source = model
                .desktop
                .style_summaries(scene_uid)
                .into_iter()
                .find(|style| style.local_index == Some(local_index))
                .ok_or_else(|| format!("style {local_index} not found"))?
                .source;
            model
                .desktop
                .reload_scene_style_file(scene_uid, local_index)
                .map_err(|error| error.to_string())?;
            model.status = format!("Reloaded style file {source}");
            if model.desktop.active_scene() == Some(scene_uid) {
                model.invalidate_mesh(false);
            }
            model.modified_scenes.insert(scene_uid);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SaveStyleFile {
            scene_uid,
            local_index,
            path,
        } => {
            let scene_uid = Uid::new(scene_uid);
            let undo = model.capture_undo();
            model
                .desktop
                .save_scene_style_file(scene_uid, local_index, &path)
                .map_err(|error| error.to_string())?;
            model.status = format!("Saved style file {path}");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CreateRendererStyle(input) => {
            let undo = model.capture_undo();
            model.create_renderer_style(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::GetStyleProperties {
            scene_uid,
            local_index,
        } => Ok(ModelResponse::StyleProperties(
            model.style_properties(scene_uid, local_index)?,
        )),
        ModelRequest::GetRendererStyleOptions {
            scene_uid,
            renderer_type,
        } => Ok(ModelResponse::RendererStyleOptions(
            model
                .desktop
                .renderer_style_options(Uid::new(scene_uid), &renderer_type)
                .into_iter()
                .map(|style| RendererStyleOptionDto {
                    name: style.name,
                    description: style.description,
                })
                .collect(),
        )),
        ModelRequest::GetWorkspaceRendererSetupOptions { scene_uid, kind } => {
            let options = match kind.as_str() {
                "renderer" => model
                    .desktop
                    .molecule_renderer_setup_options(Uid::new(scene_uid))
                    .map_err(|error| error.to_string())?,
                "densityRenderer" => model.desktop.density_map_renderer_setup_options(),
                "surfaceRenderer" => model.desktop.surface_renderer_setup_options(),
                "lwRenderer" => vec![crate::GuiRendererSetupOption {
                    name: "lwrend".to_string(),
                    description: "lwrend".to_string(),
                    preset: false,
                }],
                _ => return Err(format!("unsupported renderer target: {kind}")),
            };
            Ok(ModelResponse::RendererStyleOptions(
                options
                    .into_iter()
                    .map(|option| RendererStyleOptionDto {
                        name: option.name,
                        description: option.description,
                    })
                    .collect(),
            ))
        }
        ModelRequest::GetAtomIntrDefs {
            object_uid,
            renderer_uid,
        } => Ok(ModelResponse::AtomIntrDefs(
            model
                .desktop
                .atom_intr_def_summaries(Uid::new(object_uid), Uid::new(renderer_uid))
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|entry| AtomIntrDefDto {
                    id: entry.id,
                    mode: entry.mode,
                    atoms: entry.atoms,
                })
                .collect(),
        )),
        ModelRequest::RemoveAtomIntrDefs {
            object_uid,
            renderer_uid,
            ids,
        } => {
            let scene_uid = model
                .desktop
                .scene_manager()
                .scene_uid_for_object(Uid::new(object_uid))
                .ok_or_else(|| format!("object {object_uid} was not found"))?;
            let undo = model.capture_undo();
            let removed = model
                .desktop
                .remove_atom_intr_defs(Uid::new(object_uid), Uid::new(renderer_uid), &ids)
                .map_err(|error| error.to_string())?;
            if removed > 0 {
                model.modified_scenes.insert(scene_uid);
                model.status = format!("Removed {removed} interaction(s)");
                model.invalidate_mesh(false);
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetStyleProperties(properties) => {
            let undo = model.capture_undo();
            model.set_style_properties(properties)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::DestroyStyle {
            scene_uid,
            local_index,
        } => {
            let undo = model.capture_undo();
            model.destroy_style(scene_uid, local_index)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetWorkspaceVisible { target, visible } => {
            let undo = model.capture_undo();
            model.set_workspace_visible(target, visible)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetWorkspaceItemsVisible { targets, visible } => {
            let undo = model.capture_undo();
            match model.set_workspace_items_visible(targets, visible) {
                Ok(()) => model.push_undo(undo),
                Err(error) => {
                    model.restore_undo(undo)?;
                    return Err(error);
                }
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetWorkspaceLocked { target, locked } => {
            let undo = model.capture_undo();
            model.set_workspace_locked(target, locked)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::DeleteWorkspaceItem(target) => {
            let undo = model.capture_undo();
            model.delete_workspace_item(target)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::DeleteWorkspaceItems(targets) => {
            let undo = model.capture_undo();
            match model.delete_workspace_items(targets) {
                Ok(()) => model.push_undo(undo),
                Err(error) => {
                    model.restore_undo(undo)?;
                    return Err(error);
                }
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MoveWorkspaceObject(input) => {
            let undo = model.capture_undo();
            if model.move_workspace_object(input)? {
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MoveWorkspaceRenderer(input) => {
            let undo = model.capture_undo();
            if model.move_workspace_renderer(input)? {
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MoveWorkspaceRenderers(input) => {
            let undo = model.capture_undo();
            match model.move_workspace_renderers(input) {
                Ok(true) => model.push_undo(undo),
                Ok(false) => {}
                Err(error) => {
                    model.restore_undo(undo)?;
                    return Err(error);
                }
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetWorkspaceObjectCollapsed(input) => {
            model.set_workspace_object_collapsed(input)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetWorkspaceRendererCollapsed(input) => {
            model.set_workspace_renderer_collapsed(input)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetWorkspaceRendererSelection(input) => {
            let undo = model.capture_undo();
            model.set_workspace_renderer_selection(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ChangeWorkspaceRendererType(input) => {
            let undo = model.capture_undo();
            model.change_workspace_renderer_type(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CreateWorkspaceRenderer(input) => {
            let undo = model.capture_undo();
            match model.create_workspace_renderer(input) {
                Ok(()) => model.push_undo(undo),
                Err(error) => {
                    model.restore_undo(undo)?;
                    return Err(error);
                }
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ApplyWorkspaceRendererStyle(input) => {
            let undo = model.capture_undo();
            model.apply_workspace_renderer_style(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetWorkspaceRendererStyles(input) => {
            let undo = model.capture_undo();
            model.set_workspace_renderer_styles(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetWorkspaceRendererColoring(input) => {
            let undo = model.capture_undo();
            model.set_workspace_renderer_coloring(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::InsertWorkspaceRendererPaint(input) => {
            let undo = model.capture_undo();
            model.insert_workspace_renderer_paint(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::InsertWorkspaceObjectPaint(input) => {
            let undo = model.capture_undo();
            if model.insert_workspace_object_paint(input)? {
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CopyWorkspaceRenderer(input) => Ok(ModelResponse::Message(
            model.copy_workspace_renderer(input)?,
        )),
        ModelRequest::CopyWorkspaceRenderers(inputs) => Ok(ModelResponse::Message(
            model.copy_workspace_renderers(inputs)?,
        )),
        ModelRequest::PasteWorkspaceRenderers(input) => {
            let undo = model.capture_undo();
            if model.paste_workspace_renderers(input)? {
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CopyWorkspaceObject(input) => {
            Ok(ModelResponse::Message(model.copy_workspace_object(input)?))
        }
        ModelRequest::PasteWorkspaceObject(input) => {
            let undo = model.capture_undo();
            model.paste_workspace_object(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CopyWorkspaceCamera(input) => {
            Ok(ModelResponse::Message(model.copy_workspace_camera(input)?))
        }
        ModelRequest::PasteWorkspaceCamera(input) => {
            let undo = model.capture_undo();
            model.paste_workspace_camera(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RenameWorkspaceCamera(input) => {
            let undo = model.capture_undo();
            if model.rename_workspace_camera(input)? {
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CopyWorkspaceStyle(input) => {
            Ok(ModelResponse::Message(model.copy_workspace_style(input)?))
        }
        ModelRequest::PasteWorkspaceStyle(input) => {
            let undo = model.capture_undo();
            model.paste_workspace_style(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CreateWorkspaceRendererGroup(input) => {
            let undo = model.capture_undo();
            model.create_workspace_renderer_group(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RenameWorkspaceRendererGroup(input) => {
            let undo = model.capture_undo();
            model.rename_workspace_renderer_group(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RenameWorkspaceNode(input) => {
            let undo = model.capture_undo();
            model.rename_workspace_node(input)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::FitWorkspaceItem {
            target,
            width,
            height,
        } => {
            model.fit_workspace_item(target, width, height)?;
            Ok(ModelResponse::Viewport(model.render(width, height)))
        }
        ModelRequest::SetConfigValue { key, value } => {
            if key.trim().is_empty() {
                return Err("a config key is required".to_string());
            }
            model.desktop.put_config_value(key.trim(), value);
            model.save_config()?;
            model.status = format!("Config updated: {}", key.trim());
            Ok(ModelResponse::ApplicationInfo(model.application_info()))
        }
        ModelRequest::Undo(depth) => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            model.undo(depth)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::Redo(depth) => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            model.redo(depth)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ClearUndo => {
            model.clear_undo();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::OpenPathsViewport {
            paths,
            width,
            height,
        } => {
            apply_open_paths(model, paths)?;
            let viewport = model.viewport_metadata(width, height);
            Ok(ModelResponse::MolStructUpdate(Box::new(
                MolStructUpdateDto {
                    snapshot: model.snapshot(),
                    viewport,
                },
            )))
        }
        ModelRequest::OpenSceneViewport {
            path,
            width,
            height,
        } => {
            model.open_scene(path)?;
            let viewport = model.viewport_metadata(width, height);
            Ok(ModelResponse::MolStructUpdate(Box::new(
                MolStructUpdateDto {
                    snapshot: model.snapshot(),
                    viewport,
                },
            )))
        }
        ModelRequest::GetFileRendererSetup { path } => {
            let path = Path::new(&path);
            let Some(file_setup) = model
                .desktop
                .file_renderer_setup_for_path(path)
                .map_err(|error| error.to_string())?
            else {
                return Ok(ModelResponse::FileRendererSetup(None));
            };
            let object_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.trim().is_empty())
                .ok_or_else(|| "the file name is invalid".to_string())?;
            let setup = match file_setup.target {
                crate::GuiFileRendererTarget::Molecule => renderer_setup_defaults(
                    model,
                    object_name,
                    file_setup.molecule_reader_options.as_ref(),
                    file_setup.namd_coor_reader_options,
                )?,
                crate::GuiFileRendererTarget::DensityMap => density_map_renderer_setup_defaults(
                    model,
                    object_name,
                    file_setup.density_map_reader_options,
                )?,
                crate::GuiFileRendererTarget::Surface => surface_renderer_setup_defaults(
                    model,
                    object_name,
                    file_setup.msms_reader_options,
                )?,
            };
            Ok(ModelResponse::FileRendererSetup(Some(setup)))
        }
        ModelRequest::OpenPathWithRendererSetup {
            path,
            target,
            reader_options,
            namd_coor_options,
            density_map_options,
            msms_options,
            renderer_setup,
            width,
            height,
        } => {
            model.start_renderer_warmup();
            let undo = model.capture_undo();
            let recenter = renderer_setup.recenter;
            let map_renderer_center = (!recenter).then(|| model.camera.center());
            let result = match target {
                crate::GuiFileRendererTarget::Molecule
                    if density_map_options.is_some() || msms_options.is_some() =>
                {
                    Err(cuemol_core::CueMolError::invalid_argument(
                        "non-molecule reader options require their matching target",
                    ))
                }
                crate::GuiFileRendererTarget::Molecule => {
                    model.desktop.open_path_with_file_reader_and_renderer_setup(
                        &path,
                        reader_options.as_ref(),
                        namd_coor_options.as_ref(),
                        &renderer_setup,
                    )
                }
                crate::GuiFileRendererTarget::DensityMap
                    if reader_options.is_some()
                        || namd_coor_options.is_some()
                        || msms_options.is_some() =>
                {
                    Err(cuemol_core::CueMolError::invalid_argument(
                        "non-density-map reader options require their matching target",
                    ))
                }
                crate::GuiFileRendererTarget::DensityMap => {
                    model.desktop.open_density_map_path_with_renderer_setup(
                        &path,
                        density_map_options.as_ref(),
                        map_renderer_center,
                        &renderer_setup,
                    )
                }
                crate::GuiFileRendererTarget::Surface
                    if reader_options.is_some()
                        || namd_coor_options.is_some()
                        || density_map_options.is_some() =>
                {
                    Err(cuemol_core::CueMolError::invalid_argument(
                        "non-surface reader options require their matching target",
                    ))
                }
                crate::GuiFileRendererTarget::Surface => {
                    model.desktop.open_surface_path_with_renderer_setup(
                        &path,
                        msms_options.as_ref(),
                        &renderer_setup,
                    )
                }
            };
            if let Err(error) = result {
                model.restore_undo(undo)?;
                return Err(error.to_string());
            }
            model.opened_paths.clear();
            model.opened_paths.push(path.clone());
            model.selection.clear();
            model.sync_active_scene_properties();
            model.status = format!("File: [{path}] is loaded.");
            model.invalidate_mesh(recenter);
            model.push_undo(undo);
            model.touch_structure_details();
            Ok(ModelResponse::MolStructUpdate(Box::new(
                MolStructUpdateDto {
                    snapshot: model.snapshot(),
                    viewport: model.viewport_metadata(width, height),
                },
            )))
        }
        ModelRequest::ReloadScene => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let path = model
                .desktop
                .active_scene_source_path()
                .ok_or_else(|| "active scene has no source path".to_string())?
                .to_path_buf();
            model
                .desktop
                .reload_active_scene_qsc()
                .map_err(|error| error.to_string())?;
            model.selection.clear();
            model.reset_animation_playback();
            model.undo_stack.clear();
            model.redo_stack.clear();
            if let Some(scene_uid) = model.desktop.active_scene() {
                model.modified_scenes.remove(&scene_uid);
            }
            model.sync_active_scene_properties();
            model.invalidate_mesh(true);
            model.status = format!("Scene file: [{}] is reloaded.", path.display());
            model.log_lines.push(model.status.clone());
            model.touch_structure_details();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::GetPdbRendererSetup { pdb_id } => Ok(ModelResponse::PdbRendererSetup(
            renderer_setup_defaults(model, &pdb_id.to_ascii_lowercase(), None, None)?,
        )),
        ModelRequest::OpenPdb {
            pdb_id,
            files,
            renderer_setup,
            width,
            height,
        } => {
            model.start_renderer_warmup();
            let undo = model.capture_undo();
            let input = files
                .iter()
                .map(|file| (file.kind, file.path.clone()))
                .collect::<Vec<_>>();
            let recenter = renderer_setup.as_ref().is_some_and(|setup| setup.recenter);
            let summary =
                match model
                    .desktop
                    .open_pdb_files(&pdb_id, &input, renderer_setup.as_ref())
                {
                    Ok(summary) => summary,
                    Err(error) => {
                        model.restore_undo(undo)?;
                        for file in files {
                            let _ = fs::remove_file(file.path);
                        }
                        return Err(error.to_string());
                    }
                };
            model.selection.clear();
            model.status = format!("Opened PDB {pdb_id}: {} file(s)", files.len());
            model.invalidate_mesh(recenter);
            model.push_undo(undo);
            if summary.object_uid.is_some() {
                model.touch_structure_details();
            }
            let viewport = model.viewport_metadata(width, height);
            Ok(ModelResponse::MolStructUpdate(Box::new(
                MolStructUpdateDto {
                    snapshot: model.snapshot(),
                    viewport,
                },
            )))
        }
        ModelRequest::LoadTrajectory {
            topology_path,
            trajectory_paths,
            nevery,
            renderer_setup,
        } => {
            let undo = model.capture_undo();
            let scene_uid = model.desktop.ensure_active_scene();
            let summary = model
                .desktop
                .load_md_trajectory_with_renderer_setup(
                    scene_uid,
                    &topology_path,
                    &trajectory_paths,
                    nevery,
                    &renderer_setup,
                )
                .map_err(|error| error.to_string())?;
            model.selection.clear();
            model.status = format!(
                "Loaded trajectory {}: {} frame(s)",
                summary.object_name, summary.frame_count
            );
            model.invalidate_mesh(renderer_setup.recenter);
            model.push_undo(undo);
            model.touch_structure_details();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetTrajectoryFrame { object_uid, frame } => {
            model.set_trajectory_frame(object_uid, frame)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetTrajectoryFrameViewport {
            object_uid,
            frame,
            width,
            height,
        } => {
            model.set_trajectory_frame(object_uid, frame)?;
            let status = model.status.clone();
            let viewport = model.render(width, height);
            Ok(ModelResponse::TrajectoryFrame(TrajectoryFrameDto {
                object_uid,
                frame,
                status,
                viewport,
            }))
        }
        ModelRequest::AppendTrajectoryBlock {
            object_uid,
            path,
            nevery,
        } => {
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .append_trajectory_block(Uid::new(object_uid), &path, nevery)
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "Added trajectory block: {} frame(s), {} block(s)",
                summary.frame_count,
                summary.blocks.len()
            );
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RemoveTrajectoryBlock { object_uid, index } => {
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .remove_trajectory_block(Uid::new(object_uid), index)
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "Removed trajectory block: {} block(s) remaining",
                summary.blocks.len()
            );
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MoveTrajectoryBlock {
            object_uid,
            from,
            to,
        } => {
            let undo = model.capture_undo();
            model
                .desktop
                .move_trajectory_block(Uid::new(object_uid), from, to)
                .map_err(|error| error.to_string())?;
            model.status = format!("Moved trajectory block: {} → {}", from + 1, to + 1);
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::NewScene(name) => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let undo = model.capture_undo();
            let name = name
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Untitled 1".to_string());
            model.desktop.new_scene(name, true);
            model.selection.clear();
            model.reset_animation_playback();
            model.sync_active_scene_properties();
            model.status = "New scene".to_string();
            model.invalidate_mesh(true);
            model.push_undo(undo);
            model.touch_structure_details();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::NewTab {
            mode,
            name,
            inherit_view_props,
        } => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let undo = model.capture_undo();
            let previous_scene = model.desktop.active_scene();
            if model.desktop.active_scene().is_some() {
                model.persist_camera()?;
            }
            let name = name.trim();
            match mode.as_str() {
                "new-scene" => {
                    let name = if name.is_empty() { "Untitled 1" } else { name };
                    model.desktop.new_scene(name, true);
                    model.camera = Camera::new();
                    model.status = format!("New scene: {name}");
                    model.touch_structure_details();
                }
                "new-view" => {
                    let scene_uid = model
                        .desktop
                        .active_scene()
                        .ok_or_else(|| "no active scene".to_string())?;
                    let name = if name.is_empty() { "1" } else { name };
                    let inherited = inherit_view_props.then(|| model.camera.clone());
                    model
                        .desktop
                        .new_view(scene_uid, name, inherited.as_ref())
                        .map_err(|error| error.to_string())?;
                    model.camera = model.desktop.active_view_camera().unwrap_or_default();
                    model.status = format!("New view: {name}");
                }
                other => return Err(format!("unsupported new tab mode: {other}")),
            }
            model.selection.clear();
            if model.desktop.active_scene() != previous_scene {
                model.reset_animation_playback();
            }
            model.sync_active_scene_properties();
            model.invalidate_mesh(mode == "new-scene");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ActivateScene(scene_uid) => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let previous_scene = model.desktop.active_scene();
            if model.desktop.active_scene().is_some() {
                model.persist_camera()?;
            }
            if !model.desktop.set_active_scene(Some(Uid::new(scene_uid))) {
                return Err(format!("scene {scene_uid} was not found"));
            }
            if let Some(camera) = model.desktop.active_view_camera() {
                model.camera = camera;
            }
            if model.desktop.active_scene() != previous_scene {
                model.reset_animation_playback();
            }
            model.selection.clear();
            model.sync_active_scene_properties();
            model.status = format!("Activated scene {scene_uid}");
            model.invalidate_mesh(false);
            model.touch_structure_details();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ActivateView(view_uid) => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let previous_scene = model.desktop.active_scene();
            model.persist_camera()?;
            model.camera = model
                .desktop
                .activate_view(Uid::new(view_uid))
                .map_err(|error| error.to_string())?;
            if model.desktop.active_scene() != previous_scene {
                model.reset_animation_playback();
            }
            model.selection.clear();
            model.sync_active_scene_properties();
            model.status = format!("Activated view {view_uid}");
            model.invalidate_mesh(false);
            model.touch_structure_details();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CloseActiveScene => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let undo = model.capture_undo();
            let closed_scene = model.desktop.active_scene();
            model
                .desktop
                .close_active_view()
                .map_err(|error| error.to_string())?;
            model.camera = model.desktop.active_view_camera().unwrap_or_default();
            if model.desktop.active_scene() != closed_scene {
                model.reset_animation_playback();
            }
            model.selection.clear();
            model.sync_active_scene_properties();
            model.status = "Tab closed".to_string();
            model.invalidate_mesh(false);
            model.push_undo(undo);
            if let Some(scene_uid) = closed_scene
                .filter(|scene_uid| model.desktop.scene_manager().scene(*scene_uid).is_none())
            {
                model.modified_scenes.remove(&scene_uid);
            }
            model.touch_structure_details();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetSelection(selection) => {
            let selection = selection.trim();
            if !selection.is_empty() {
                cuemol_molstr::parse_selector(selection).map_err(|error| error.to_string())?;
            }
            model.selection = selection.to_string();
            model.invalidate_mesh(false);
            model.status = if model.selection.is_empty() {
                "Selection cleared".to_string()
            } else {
                format!("Selection: {}", model.selection)
            };
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MolStructAction {
            object_uid,
            selection,
            mode,
            width,
            height,
        } => {
            let undo = model.capture_undo();
            model.apply_molecule_selection_action(object_uid, &selection, &mode, width, height)?;
            model.push_undo(undo);
            let viewport = model.render(width, height);
            Ok(ModelResponse::MolStructUpdate(Box::new(
                MolStructUpdateDto {
                    snapshot: model.snapshot(),
                    viewport,
                },
            )))
        }
        ModelRequest::SetRendererVisible {
            object_uid,
            renderer_uid,
            visible,
        } => {
            let undo = model.capture_undo();
            let object_uid = Uid::new(object_uid);
            let renderer_uid = Uid::new(renderer_uid);
            let is_group = model
                .desktop
                .scene_manager()
                .object(object_uid)
                .and_then(|object| object.renderer(renderer_uid))
                .is_some_and(|renderer| renderer.renderer_type() == "*group");
            if is_group {
                model
                    .desktop
                    .set_renderer_group_visible(object_uid, renderer_uid, visible)
            } else {
                model
                    .desktop
                    .set_renderer_visible(object_uid, renderer_uid, visible)
            }
            .map_err(|error| error.to_string())?;
            model.status = if visible {
                "Renderer shown".to_string()
            } else {
                "Renderer hidden".to_string()
            };
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AddRenderer {
            object_uid,
            renderer_type,
            group_name,
        } => {
            if !MOLECULE_RENDERER_TYPES.contains(&renderer_type.as_str()) {
                return Err(format!("unsupported renderer type: {renderer_type}"));
            }
            let undo = model.capture_undo();
            let renderer_name = model
                .desktop
                .suggest_renderer_name_in_active_scene(&renderer_type)
                .map_err(|error| error.to_string())?;
            if let Some(group_name) = group_name.as_deref() {
                let group_exists = model
                    .desktop
                    .renderer_summaries(Uid::new(object_uid))
                    .iter()
                    .any(|renderer| {
                        renderer.renderer_type == "*group" && renderer.name == group_name
                    });
                if !group_exists {
                    return Err(format!("renderer group not found: {group_name}"));
                }
            }
            let renderer_uid = model
                .desktop
                .new_renderer_in_active_scene(
                    Uid::new(object_uid),
                    renderer_type.clone(),
                    renderer_name,
                )
                .map_err(|error| error.to_string())?
                .result_renderer;
            if let Some(group_name) = group_name {
                model
                    .desktop
                    .set_renderer_property(Uid::new(object_uid), renderer_uid, "group", group_name)
                    .map_err(|error| error.to_string())?;
            }
            model.status = format!("Added {renderer_type} renderer");
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetObjectColoring {
            object_uid,
            coloring,
        } => {
            let undo = model.capture_undo();
            let (coloring_type, attrs) = match coloring.as_str() {
                "element" => (None, std::collections::BTreeMap::new()),
                "rainbow" => {
                    let mut attrs = std::collections::BTreeMap::new();
                    attrs.insert("mode".to_string(), "mol".to_string());
                    attrs.insert("start_hue".to_string(), "240".to_string());
                    attrs.insert("end_hue".to_string(), "0".to_string());
                    attrs.insert("sat".to_string(), "1".to_string());
                    attrs.insert("bri".to_string(), "1".to_string());
                    (Some("RainbowColoring".to_string()), attrs)
                }
                "bfac" => (
                    Some("BfacColoring".to_string()),
                    std::collections::BTreeMap::new(),
                ),
                _ => return Err(format!("unsupported coloring type: {coloring}")),
            };
            model
                .desktop
                .set_object_coloring(Uid::new(object_uid), coloring_type, attrs)
                .map_err(|error| error.to_string())?;
            model.status = format!("Applied {coloring} coloring");
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetColoring {
            input,
            width,
            height,
        } => {
            validate_coloring_update(&input)?;
            let undo = model.capture_undo();
            let paint_entries = input
                .paint_entries
                .into_iter()
                .map(|entry| (entry.selection, entry.color))
                .collect();
            let gradient_entries = input
                .gradient_entries
                .into_iter()
                .map(|entry| (entry.value, entry.color))
                .collect::<Vec<_>>();
            let renderer_uid = input
                .renderer_uid
                .map(Uid::new)
                .unwrap_or_else(|| Uid::new(0));
            match input.kind.as_str() {
                "object" => model
                    .desktop
                    .set_object_coloring_details(
                        Uid::new(input.object_uid),
                        input.coloring_type,
                        input.attrs,
                        paint_entries,
                        input.default_color,
                    )
                    .map_err(|error| error.to_string())?,
                "renderer" if input.coloring_type.as_deref() == Some("ElectrostaticColoring") => {
                    model
                        .desktop
                        .set_renderer_electrostatic_coloring(
                            Uid::new(input.object_uid),
                            renderer_uid,
                            &input.attrs,
                        )
                        .map_err(|error| error.to_string())?;
                }
                "renderer" => model
                    .desktop
                    .set_renderer_coloring_details(
                        Uid::new(input.object_uid),
                        renderer_uid,
                        input.coloring_type,
                        input.attrs,
                        paint_entries,
                        input.default_color,
                    )
                    .map_err(|error| error.to_string())?,
                "surfaceRenderer" => {
                    let color_mode = match input.coloring_type.as_deref() {
                        Some("ElectrostaticColoring") => "potential",
                        Some("MultiGradientColoring") => "multigrad",
                        _ => "solid",
                    };
                    let mut attrs = input.attrs;
                    if color_mode == "solid" {
                        if let Some(color) = input.default_color {
                            attrs.insert("defaultcolor".to_string(), color);
                        }
                    }
                    model
                        .desktop
                        .set_surface_renderer_coloring(
                            Uid::new(input.object_uid),
                            renderer_uid,
                            color_mode,
                            &attrs,
                            &gradient_entries,
                        )
                        .map_err(|error| error.to_string())?;
                }
                "densityRenderer" => {
                    let color_mode =
                        if input.coloring_type.as_deref() == Some("MultiGradientColoring") {
                            "multigrad"
                        } else {
                            "solid"
                        };
                    let mut attrs = input.attrs;
                    attrs.remove("ramp_above");
                    attrs.remove("ramp_value");
                    if color_mode == "solid" {
                        if let Some(color) = input.default_color {
                            attrs.insert("color".to_string(), color);
                        }
                    }
                    model
                        .desktop
                        .set_density_renderer_coloring(
                            Uid::new(input.object_uid),
                            renderer_uid,
                            color_mode,
                            &attrs,
                            &gradient_entries,
                        )
                        .map_err(|error| error.to_string())?;
                }
                _ => unreachable!("coloring target was validated"),
            }
            model.status = "Changed coloring".to_string();
            model.rebuild_mesh(false)?;
            model.push_undo(undo);
            let viewport = model.render(width, height);
            Ok(ModelResponse::MolStructUpdate(Box::new(
                MolStructUpdateDto {
                    snapshot: model.snapshot(),
                    viewport,
                },
            )))
        }
        ModelRequest::DeleteAtoms {
            object_uid,
            selection,
        } => {
            let selection = selection.trim();
            if selection.is_empty() {
                return Err("an atom selection is required".to_string());
            }
            let undo = model.capture_undo();
            let count = model
                .desktop
                .delete_atoms_in_object(Uid::new(object_uid), selection)
                .map_err(|error| error.to_string())?;
            model.status = format!("Deleted {count} atom(s)");
            if count > 0 {
                model.selection.clear();
                model.invalidate_mesh(true);
                model.push_undo(undo);
                model.touch_structure_details();
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ChangeChain {
            object_uid,
            selection,
            chain_name,
        } => {
            let selection = selection.trim();
            if selection.is_empty() {
                return Err("an atom selection is required".to_string());
            }
            let undo = model.capture_undo();
            let count = model
                .desktop
                .change_chain_in_object(Uid::new(object_uid), selection, &chain_name)
                .map_err(|error| error.to_string())?;
            model.status = format!("Changed chain ID for {count} atom(s)");
            if count > 0 {
                model.invalidate_mesh(false);
                model.push_undo(undo);
                model.touch_structure_details();
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RenumberResidues {
            object_uid,
            selection,
            start,
        } => {
            let selection = selection.trim();
            if selection.is_empty() {
                return Err("an atom selection is required".to_string());
            }
            let undo = model.capture_undo();
            let count = model
                .desktop
                .renumber_residues_in_object(Uid::new(object_uid), selection, start)
                .map_err(|error| error.to_string())?;
            model.status = format!("Renumbered residues for {count} atom(s)");
            if count > 0 {
                model.invalidate_mesh(false);
                model.push_undo(undo);
                model.touch_structure_details();
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MergeMolecules {
            source_object_uid,
            selection,
            target_object_uid,
            copy,
        } => {
            let undo = model.capture_undo();
            let count = model
                .desktop
                .merge_molecules(
                    Uid::new(source_object_uid),
                    &selection,
                    Uid::new(target_object_uid),
                    copy,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "{} {count} atom(s) into the target molecule",
                if copy { "Copied" } else { "Moved" }
            );
            model.invalidate_mesh(true);
            model.push_undo(undo);
            model.touch_structure_details();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SuperposeObjects {
            algorithm,
            reference_object_uid,
            reference_selection,
            moving_object_uid,
            moving_selection,
            auto_recenter,
            use_property,
        } => {
            if reference_object_uid == moving_object_uid {
                return Err("reference and moving objects must differ".to_string());
            }
            let undo = model.capture_undo();
            model
                .desktop
                .superpose_objects_with_algorithm(
                    Uid::new(reference_object_uid),
                    &reference_selection,
                    Uid::new(moving_object_uid),
                    &moving_selection,
                    &algorithm,
                    use_property,
                )
                .map_err(|error| error.to_string())?;
            model.status = "Molecular superposition applied".to_string();
            model.invalidate_mesh(auto_recenter);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::EditBond {
            object_uid,
            aid1,
            aid2,
            make,
        } => {
            if aid1 == aid2 {
                return Err("two different atom serials are required".to_string());
            }
            let undo = model.capture_undo();
            let changed = if make {
                model
                    .desktop
                    .make_bond_in_object(Uid::new(object_uid), aid1, aid2)
            } else {
                model
                    .desktop
                    .remove_bond_in_object(Uid::new(object_uid), aid1, aid2)
            }
            .map_err(|error| error.to_string())?;
            model.status = if changed {
                if make {
                    "Bond created"
                } else {
                    "Bond removed"
                }
            } else {
                "Bond was unchanged"
            }
            .to_string();
            if changed {
                model.invalidate_mesh(false);
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::GetNonstandardBonds { object_uid } => {
            let json = model
                .desktop
                .nonstandard_bonds_json(Uid::new(object_uid))
                .map_err(|error| error.to_string())?;
            Ok(ModelResponse::Message(json))
        }
        ModelRequest::RemoveNonstandardBonds { object_uid, pairs } => {
            let pairs = pairs
                .into_iter()
                .filter(|[aid1, aid2]| aid1 != aid2)
                .map(|[aid1, aid2]| (aid1.min(aid2), aid1.max(aid2)))
                .collect::<BTreeSet<_>>();
            if pairs.is_empty() {
                return Err("select at least one non-standard bond".to_string());
            }
            let undo = model.capture_undo();
            let mut removed = 0;
            for (aid1, aid2) in pairs {
                if model
                    .desktop
                    .remove_bond_in_object(Uid::new(object_uid), aid1, aid2)
                    .map_err(|error| error.to_string())?
                {
                    removed += 1;
                }
            }
            model.status = format!("Removed {removed} non-standard bond(s)");
            if removed > 0 {
                model.invalidate_mesh(false);
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CalculateContacts {
            object_uid,
            selection1,
            object2_uid,
            selection2,
            r_min,
            r_max,
            hbond,
            max_count,
            renderer_name,
        } => {
            let renderer_name = if renderer_name.trim().is_empty() {
                "measure".to_string()
            } else {
                renderer_name.trim().to_string()
            };
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .define_atom_contacts(
                    Uid::new(object_uid),
                    &selection1,
                    object2_uid.map(Uid::new),
                    selection2.as_deref(),
                    r_min,
                    r_max,
                    hbond,
                    max_count.clamp(1, 10_000),
                    &renderer_name,
                )
                .map_err(|error| error.to_string())?;
            if summary.count == 0 {
                model.status = "No interaction was found".to_string();
            } else {
                model.status = format!(
                    "Defined {} interaction(s) in {}",
                    summary.count, renderer_name
                );
                model.invalidate_mesh(false);
                model.push_undo(undo);
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::GetSpaceGroups { lattice } => Ok(ModelResponse::Message(
            model.desktop.space_group_summaries_json(&lattice),
        )),
        ModelRequest::ChangeCrystalInfo {
            object_uid,
            a,
            b,
            c,
            alpha,
            beta,
            gamma,
            space_group_number,
        } => {
            let undo = model.capture_undo();
            model
                .desktop
                .change_crystal_info(
                    Uid::new(object_uid),
                    a,
                    b,
                    c,
                    alpha,
                    beta,
                    gamma,
                    space_group_number,
                )
                .map_err(|error| error.to_string())?;
            model.status = "Changed symmetry information".to_string();
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AssignSecondaryStructure {
            object_uid,
            ignore_bulge,
            angle,
        } => {
            if !angle.is_finite() || !(0.0..=180.0).contains(&angle) {
                return Err("helix angle must be between 0 and 180 degrees".to_string());
            }
            let undo = model.capture_undo();
            let count = model
                .desktop
                .assign_secondary_structure_in_object(Uid::new(object_uid), ignore_bulge, angle)
                .map_err(|error| error.to_string())?;
            model.status = format!("Secondary structure assigned for {count} atom(s)");
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CreateMolecularSurface {
            object_uid,
            selection,
            name,
            density,
            probe_radius,
        } => {
            let undo = model.capture_undo();
            let _surface_uid = model
                .desktop
                .create_molecular_surface_object(
                    Uid::new(object_uid),
                    &selection,
                    &name,
                    density,
                    probe_radius,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("Created molecular surface {name}");
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::WriteSurface {
            object_uid,
            selection,
            density,
            probe_radius,
            path,
        } => {
            if !density.is_finite() || density < 0.1 {
                return Err("surface density must be at least 0.1".to_string());
            }
            if !probe_radius.is_finite() || probe_radius < 0.1 {
                return Err("probe radius must be at least 0.1 Å".to_string());
            }
            if path.trim().is_empty() {
                return Err("an output OBJ path is required".to_string());
            }
            let summary = model
                .desktop
                .write_surface_obj(
                    Uid::new(object_uid),
                    &selection,
                    density,
                    probe_radius,
                    &path,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "Surface OBJ written: {} vertices / {} triangles",
                summary.vertex_count, summary.triangle_count
            );
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::WriteCutSurface {
            object_uid,
            selection,
            density,
            probe_radius,
            normal,
            position,
            path,
        } => {
            if !density.is_finite() || density < 0.1 {
                return Err("surface density must be at least 0.1".to_string());
            }
            if !probe_radius.is_finite() || probe_radius < 0.1 {
                return Err("probe radius must be at least 0.1 Å".to_string());
            }
            if normal
                .iter()
                .chain(position.iter())
                .any(|value| !value.is_finite())
            {
                return Err("surface cut plane must be finite".to_string());
            }
            if normal.iter().map(|value| value * value).sum::<f64>() < 1.0e-12 {
                return Err("surface cut normal must be non-zero".to_string());
            }
            if path.trim().is_empty() {
                return Err("an output OBJ path is required".to_string());
            }
            let summary = model
                .desktop
                .write_cut_surface_obj(
                    Uid::new(object_uid),
                    &selection,
                    density,
                    probe_radius,
                    Vector4::new3(normal[0], normal[1], normal[2]),
                    Vector4::new3(position[0], position[1], position[2]),
                    &path,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "Cut surface OBJ written: {} vertices / {} triangles",
                summary.vertex_count, summary.triangle_count
            );
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CutSurface {
            surface_uid,
            density,
            mode,
        } => {
            let undo = model.capture_undo();
            let (normal, position) = DesktopAppState::clipping_plane_from_camera(&model.camera);
            let summary = model
                .desktop
                .cut_loaded_surface_by_plane(
                    Uid::new(surface_uid),
                    density,
                    normal,
                    position,
                    &mode,
                )
                .map_err(|error| error.to_string())?;
            model.status = if summary.section_uid.is_some() {
                format!(
                    "Surface cut: {} body / {} section triangles",
                    summary.target_triangle_count, summary.section_triangle_count
                )
            } else {
                format!("Surface cut: {} triangles", summary.target_triangle_count)
            };
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::WriteApbsFiles {
            object_uid,
            selection,
            pqr_path,
            input_path,
            dx_path,
        } => {
            if [pqr_path.as_str(), input_path.as_str(), dx_path.as_str()]
                .iter()
                .any(|path| path.trim().is_empty())
            {
                return Err("PQR, input, and DX paths are required".to_string());
            }
            let summary = model
                .desktop
                .write_apbs_files(
                    Uid::new(object_uid),
                    &selection,
                    &pqr_path,
                    &input_path,
                    &dx_path,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("APBS files written for {} atom(s)", summary.atom_count);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::StartApbsCalculation(input) => {
            if model.apbs_job.is_some() {
                return Err("an APBS calculation is already running".to_string());
            }
            let charge_method = match input.charge_method.as_str() {
                "pdb2pqr" => GuiApbsChargeMethod::Pdb2pqr,
                "internal" => GuiApbsChargeMethod::Internal,
                value => return Err(format!("unsupported APBS charge method: {value}")),
            };
            let object_uid = Uid::new(input.object_uid);
            let potential_name = if input.potential_name.trim().is_empty() {
                model
                    .desktop
                    .suggested_apbs_potential_name(object_uid)
                    .map_err(|error| error.to_string())?
            } else {
                input.potential_name.trim().to_string()
            };
            let work_dir = create_apbs_work_dir()?;
            let paths = GuiApbsPaths {
                pdb_path: work_dir.join("apbs_tmp.pdb"),
                pqr_path: work_dir.join("apbs_tmp.pqr"),
                input_path: work_dir.join("apbs_tmp.in"),
                dx_path: work_dir.join("apbs_pot.dx"),
            };
            let options = GuiApbsOptions {
                charge_method,
                use_hydrogen: input.use_hydrogen,
                use_npbe: input.use_npbe,
                temperature: input.temperature,
                grid_spacing: input.grid_spacing,
                solvent_dielectric: input.water_dielectric,
                solute_dielectric: input.protein_dielectric,
            };
            let summary = match model.desktop.prepare_apbs_files(
                object_uid,
                &input.selection,
                &options,
                &paths,
            ) {
                Ok(summary) => summary,
                Err(error) => {
                    let _ = fs::remove_dir_all(&work_dir);
                    return Err(error.to_string());
                }
            };
            let task = ApbsProcessTask {
                apbs_executable: input.apbs_executable,
                pdb2pqr_executable: input.pdb2pqr_executable,
                force_field: input.force_field,
                charge_method,
                paths,
                work_dir: work_dir.clone(),
            };
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = Arc::clone(&cancel);
            let (sender, receiver) = mpsc::channel();
            if let Err(error) = std::thread::Builder::new()
                .name("cuemol-tauri-apbs".to_string())
                .spawn(move || {
                    let _ = sender.send(run_apbs_process(task, worker_cancel));
                })
            {
                let _ = fs::remove_dir_all(work_dir);
                return Err(format!("failed to start APBS worker: {error}"));
            }
            model.apbs_job = Some(TauriApbsJob {
                receiver,
                cancel,
                potential_name,
            });
            model.status = format!(
                "APBS calculation started for {} atom(s)",
                summary.atom_count
            );
            Ok(ModelResponse::Message("running".to_string()))
        }
        ModelRequest::PollApbsCalculation => {
            let Some(job) = model.apbs_job.take() else {
                return Ok(ModelResponse::Message("idle".to_string()));
            };
            match job.receiver.try_recv() {
                Err(mpsc::TryRecvError::Empty) => {
                    model.apbs_job = Some(job);
                    Ok(ModelResponse::Message("running".to_string()))
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    model.status = "APBS worker disconnected".to_string();
                    Ok(ModelResponse::Message(
                        "error:APBS worker disconnected".to_string(),
                    ))
                }
                Ok(Err(error)) => {
                    model.status = error.clone();
                    Ok(ModelResponse::Message(format!("error:{error}")))
                }
                Ok(Ok(output)) => {
                    let undo = model.capture_undo();
                    let result = model
                        .desktop
                        .open_apbs_potential_path(&output.dx_path, &job.potential_name);
                    let _ = fs::remove_dir_all(output.work_dir);
                    let map_uid = result.map_err(|error| error.to_string())?;
                    if !output.log.trim().is_empty() {
                        model
                            .log_lines
                            .extend(output.log.lines().map(|line| format!("APBS> {line}")));
                    }
                    model.status = format!("APBS calculation complete: {}", job.potential_name);
                    model.invalidate_mesh(false);
                    model.push_undo(undo);
                    Ok(ModelResponse::Message(format!(
                        "complete:{}",
                        map_uid.get()
                    )))
                }
            }
        }
        ModelRequest::CancelApbsCalculation => {
            let running = if let Some(job) = &model.apbs_job {
                job.cancel.store(true, Ordering::Relaxed);
                model.status = "Stopping APBS calculation...".to_string();
                true
            } else {
                false
            };
            Ok(ModelResponse::Message(if running {
                "stopping".to_string()
            } else {
                "idle".to_string()
            }))
        }
        ModelRequest::AppendMorphFrame {
            target_object_uid,
            source_object_uid,
            frame_name,
        } => {
            if frame_name.trim().is_empty() {
                return Err("a morph frame name is required".to_string());
            }
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .append_morph_frame_from_object(
                    Uid::new(target_object_uid),
                    Uid::new(source_object_uid),
                    frame_name.trim(),
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("Morph frame appended: {} frame(s)", summary.frame_count);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RemoveMorphFrame { object_uid, index } => {
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .remove_morph_frame(Uid::new(object_uid), index)
                .map_err(|error| error.to_string())?;
            model.status = format!("Morph frame removed: {} frame(s)", summary.frame_count);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ApplyMorphFrame { object_uid, frame } => {
            if !frame.is_finite() {
                return Err("morph frame must be finite".to_string());
            }
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .apply_morph_frame(Uid::new(object_uid), frame)
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "Morph frame {:.2} applied to {} atom(s)",
                frame, summary.atom_count
            );
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetDisplayOption { option, value } => {
            let undo = model.capture_undo();
            match option.as_str() {
                "unit-cell" => model.show_unit_cell = value,
                "symmetry-mates" => model.show_symmetry_mates = value,
                "density-contour" => model.show_density_contour = value,
                "color-proof" => {
                    model.color_proofing = value;
                    if let Some(scene) = model
                        .desktop
                        .active_scene()
                        .and_then(|uid| model.desktop.scene_manager_mut().scene_mut(uid))
                    {
                        set_scene_attr_default(scene, "use_colproof", &value.to_string(), "false");
                    }
                }
                _ => return Err(format!("unsupported display option: {option}")),
            }
            model.status = format!("{option}: {}", if value { "shown" } else { "hidden" });
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetDensityLevel(level) => {
            if !level.is_finite() {
                return Err("density level must be finite".to_string());
            }
            let undo = model.capture_undo();
            model.density_level = level.clamp(0.1, 10.0);
            model.status = format!("Density level: {:.2} σ", model.density_level);
            model.invalidate_mesh(false);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetBackground(background) => {
            background_rgba(&background)?;
            let undo = model.capture_undo();
            model.background = background.trim().to_string();
            if let Some(scene) = model
                .desktop
                .active_scene()
                .and_then(|uid| model.desktop.scene_manager_mut().scene_mut(uid))
            {
                set_scene_attr_default(scene, "bgcolor", &model.background, "black");
            }
            model.status = format!("Background: {}", model.background);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::PickAtom {
            x,
            y,
            width,
            height,
        } => {
            model.pick_atom(x, y, width, height)?;
            let viewport = model.render(width, height);
            Ok(ModelResponse::MolStructUpdate(Box::new(
                MolStructUpdateDto {
                    snapshot: model.snapshot(),
                    viewport,
                },
            )))
        }
        ModelRequest::PickAtomIdentifier {
            x,
            y,
            width,
            height,
        } => Ok(ModelResponse::PickedAtom(
            model.pick_atom_identifier(x, y, width, height)?,
        )),
        ModelRequest::SelectScreenRect {
            x1,
            y1,
            x2,
            y2,
            width,
            height,
        } => {
            model.select_screen_rect(x1, y1, x2, y2, width, height)?;
            let viewport = model.render(width, height);
            Ok(ModelResponse::MolStructUpdate(Box::new(
                MolStructUpdateDto {
                    snapshot: model.snapshot(),
                    viewport,
                },
            )))
        }
        ModelRequest::AddAnimationObject { kind, after_index } => {
            model.add_animation_object(&kind, after_index, None)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AddConfiguredAnimationObject {
            kind,
            after_index,
            properties,
        } => {
            model.add_animation_object(&kind, after_index, Some(properties))?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::DeleteAnimationObject(index) => {
            model.delete_animation_objects(&[index])?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::DeleteAnimationObjects(indices) => {
            model.delete_animation_objects(&indices)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MoveAnimationObject { index, direction } => {
            model.move_animation_objects(&[index], direction)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MoveAnimationObjects { indices, direction } => {
            model.move_animation_objects(&indices, direction)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetAnimationDuration { index, duration } => {
            if !duration.is_finite() {
                return Err("animation duration must be finite".to_string());
            }
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let mut animation = model.active_animation_manager()?;
            let Some(uid) = animation.get_at(index).map(|object| object.uid()) else {
                return Err(format!("animation object {index} was not found"));
            };
            let undo = model.capture_undo();
            let duration = animation_time(duration.clamp(0.1, 3600.0));
            animation.edit_motion_by_uid(uid, "end", "", |motion| {
                let start = motion.base().rel_start();
                motion
                    .base_mut()
                    .set_rel_end(start.saturating_add(duration));
            });
            model.set_active_animation_manager(&animation)?;
            model.animation_elapsed = model.animation_elapsed.min(model.animation_duration());
            model.status = "Animation duration changed".to_string();
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetAnimationObjectProperties(input) => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let undo = model.capture_undo();
            model.set_animation_object_properties(input)?;
            model.status = "Animation object properties changed".to_string();
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AnimationControl(action) => {
            match action.as_str() {
                "play" => {
                    if model.active_animation_manager()?.size() == 0 {
                        model.status = "No animation objects".to_string();
                    } else {
                        if model.animation_elapsed >= model.animation_duration() {
                            model.animation_elapsed = 0.0;
                        }
                        model.begin_animation_playback()?;
                        model.animation_playing = true;
                        model.status = "Animation playing".to_string();
                    }
                }
                "pause" => {
                    model.animation_playing = false;
                    model.status = "Animation paused".to_string();
                }
                "stop" => {
                    model.restore_animation_playback(false)?;
                    model.animation_elapsed = 0.0;
                    model.status = "Animation stopped".to_string();
                }
                _ => return Err(format!("unsupported animation control: {action}")),
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetAnimationLoop(value) => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let undo = model.capture_undo();
            let mut animation = model.active_animation_manager()?;
            animation.set_loop(value);
            model.set_active_animation_manager(&animation)?;
            model.status = format!("Animation loop: {value}");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetAnimationStartCamera(name) => {
            model.restore_animation_playback(false)?;
            model.animation_elapsed = 0.0;
            let scene_uid = model
                .desktop
                .active_scene()
                .ok_or_else(|| "no active scene".to_string())?;
            let name = name.trim();
            if name.is_empty() {
                return Err("animation start camera is required".to_string());
            }
            if !model
                .desktop
                .camera_summaries(scene_uid)
                .iter()
                .any(|camera| camera.name == name)
            {
                return Err(format!("camera {name} was not found"));
            }
            let undo = model.capture_undo();
            let mut animation = model.active_animation_manager()?;
            animation.set_startcam(name);
            model.set_active_animation_manager(&animation)?;
            model.reset_animation_playback();
            model.status = format!("Animation start camera: {name}");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SeekAnimation(elapsed) => {
            if !elapsed.is_finite() {
                return Err("animation time must be finite".to_string());
            }
            let elapsed = elapsed.clamp(0.0, model.animation_duration());
            model.begin_animation_playback()?;
            model.apply_active_animation_time(elapsed)?;
            model.animation_elapsed = elapsed;
            model.animation_playing = false;
            model.status = format!("Animation time: {:.2} s", model.animation_elapsed);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::TickAnimation(dt) => {
            model.tick_animation(dt)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::TickAnimationFrame { dt, width, height } => {
            model.tick_animation(dt)?;
            let animation_elapsed = model.animation_elapsed;
            let animation_duration = model.animation_duration();
            let animation_playing = model.animation_playing;
            let status = model.status.clone();
            let viewport = model.render(width, height);
            Ok(ModelResponse::AnimationFrame(AnimationFrameDto {
                animation_elapsed,
                animation_duration,
                animation_playing,
                status,
                viewport,
            }))
        }
        ModelRequest::ClearLog => {
            model.log_lines.clear();
            model.status = "Output log cleared".to_string();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ExecuteScriptFile(path) => {
            let script = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            if script.trim().is_empty() {
                return Err("a script is required".to_string());
            }
            let undo = model.capture_undo();
            let (log, selection) = model
                .desktop
                .run_script_js(&script)
                .map_err(|error| error.to_string())?;
            model
                .log_lines
                .extend(log.lines().map(|line| line.to_string()));
            if let Some(selection) = selection {
                model.selection = selection;
            }
            model
                .log_lines
                .push(format!("Exec internal JS: {path} done."));
            model.invalidate_mesh(false);
            model.status = "Script executed".to_string();
            model.push_undo(undo);
            model.touch_structure_details();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ExportPov { path } => {
            if path.trim().is_empty() {
                return Err("a POV-Ray output path is required".to_string());
            }
            model.persist_camera()?;
            let count = model
                .desktop
                .export_active_scene_pov(&path, &model.selection)
                .map_err(|error| error.to_string())?;
            model.status = format!("POV-Ray scene written: {count} atom(s)");
            model.log_lines.push(format!("POV-Ray scene: {path}"));
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::StartPovRender(input) => {
            if model.pov_render_job.is_some() {
                return Err("a POV-Ray rendering job is already running".to_string());
            }
            if input.width == 0
                || input.height == 0
                || input.width > 10_000
                || input.height > 10_000
            {
                return Err("POV-Ray image dimensions must be between 1 and 10000".to_string());
            }
            if input.executable.trim().is_empty() {
                return Err("POV-Ray executable path is required".to_string());
            }
            if !input.include_dir.trim().is_empty() && !Path::new(&input.include_dir).is_dir() {
                return Err("POV-Ray include directory does not exist".to_string());
            }
            model.persist_camera()?;
            let scene_uid = Uid::new(input.scene_uid);
            let view_uid = Uid::new(input.view_uid);
            let target_view = model
                .desktop
                .view_summaries()
                .into_iter()
                .find(|view| view.uid == view_uid && view.scene_uid == scene_uid)
                .ok_or_else(|| "POV-Ray render target no longer exists".to_string())?;
            let target_camera = if target_view.active {
                model.camera.clone()
            } else {
                model
                    .desktop
                    .view_camera(view_uid)
                    .ok_or_else(|| "POV-Ray target view no longer exists".to_string())?
            };
            if let Some(result) = model.pov_render_result.take() {
                let _ = fs::remove_dir_all(result.work_dir);
            }
            let work_dir = create_pov_work_dir()?;
            let input_path = work_dir.join("render.pov");
            let output_path = work_dir.join("render.png");
            let scene_options = GuiPovSceneOptions {
                perspective: Some(input.perspective),
                use_clip_z: input.clip_plane,
                use_post_blend: input.post_blend,
                show_edge_lines: input.edge_lines,
                use_pixel_labels: input.pixel_labels,
            };
            if let Err(error) = model.desktop.export_scene_pov_with_camera(
                &input_path,
                scene_uid,
                &model.selection,
                &target_camera,
                scene_options,
            ) {
                let _ = fs::remove_dir_all(work_dir);
                return Err(error.to_string());
            }
            let (light_spread, light_intensity, flash_fraction, ambient_fraction) =
                if input.default_lighting {
                    if input.radiosity < 0 {
                        (1, 1.3, 0.8 / 1.3, 0.0)
                    } else {
                        (1, 1.6, 0.0, 0.7)
                    }
                } else {
                    (
                        input.light_spread,
                        input.light_intensity,
                        input.flash_fraction,
                        input.ambient_fraction,
                    )
                };
            let task = PovProcessTask {
                executable: input.executable.clone(),
                include_dir: input.include_dir.clone(),
                input_path,
                output_path,
                options: PovProcessOptions {
                    width: input.width,
                    height: input.height,
                    threads: input.threads,
                    stereo: input.stereo,
                    stereo_depth: input.stereo_depth,
                    perspective: input.perspective,
                    transparent: input.transparent,
                    shadow: input.shadow,
                    light_spread,
                    light_intensity,
                    flash_fraction,
                    ambient_fraction,
                    radiosity: input.radiosity,
                    verbose: true,
                },
            };
            let labels = collect_scene_pov_labels(
                &model.desktop,
                scene_uid,
                &target_camera,
                input.width,
                input.height,
                input.pixel_labels,
            );
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = Arc::clone(&cancel);
            let (sender, receiver) = mpsc::channel();
            let failure_work_dir = work_dir.clone();
            if let Err(error) = std::thread::Builder::new()
                .name("cuemol-tauri-pov".to_string())
                .spawn(move || {
                    let _ = sender.send(run_pov_process(task, worker_cancel));
                })
            {
                let _ = fs::remove_dir_all(failure_work_dir);
                return Err(format!("failed to start POV-Ray worker: {error}"));
            }
            model.desktop.put_config_value(
                "cuemol2.ui.render.pov-exe-path",
                input.executable.trim().to_string(),
            );
            model.desktop.put_config_value(
                "cuemol2.ui.render.pov-inc-path",
                input.include_dir.trim().to_string(),
            );
            if let Err(error) = model.save_config() {
                model.log_lines.push(format!("WARNING: {error}"));
            }
            model.pov_render_job = Some(TauriPovRenderJob {
                receiver,
                cancel,
                work_dir,
                labels,
            });
            model.status = format!(
                "POV-Ray rendering started: {}x{}",
                input.width, input.height
            );
            Ok(ModelResponse::Message("running".to_string()))
        }
        ModelRequest::PollPovRender => {
            let Some(job) = model.pov_render_job.take() else {
                return Ok(ModelResponse::Message("idle".to_string()));
            };
            match job.receiver.try_recv() {
                Err(mpsc::TryRecvError::Empty) => {
                    model.pov_render_job = Some(job);
                    Ok(ModelResponse::Message("running".to_string()))
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = fs::remove_dir_all(job.work_dir);
                    model.status = "POV-Ray worker disconnected".to_string();
                    Ok(ModelResponse::Message(
                        "error:POV-Ray worker disconnected".to_string(),
                    ))
                }
                Ok(Err(error)) => {
                    let _ = fs::remove_dir_all(job.work_dir);
                    model.status = error.clone();
                    Ok(ModelResponse::Message(format!("error:{error}")))
                }
                Ok(Ok(output)) => {
                    if !job.labels.is_empty() {
                        if let Err(error) = overlay_pov_labels(&output.output_path, &job.labels) {
                            let _ = fs::remove_dir_all(job.work_dir);
                            model.status = error.clone();
                            return Ok(ModelResponse::Message(format!("error:{error}")));
                        }
                    }
                    let image = match fs::read(&output.output_path) {
                        Ok(image) => image,
                        Err(error) => {
                            let _ = fs::remove_dir_all(job.work_dir);
                            return Err(format!("failed to read POV-Ray result: {error}"));
                        }
                    };
                    if !output.log.trim().is_empty() {
                        model.log_lines.extend(
                            output
                                .log
                                .lines()
                                .filter(|line| !line.trim().is_empty())
                                .map(|line| format!("POV-Ray> {line}")),
                        );
                    }
                    let data_uri = format!("data:image/png;base64,{}", base64_encode(&image));
                    model.pov_render_result = Some(TauriPovRenderResult {
                        work_dir: job.work_dir,
                        image_path: output.output_path,
                    });
                    model.status = "POV-Ray rendering complete".to_string();
                    Ok(ModelResponse::Message(format!("complete:{data_uri}")))
                }
            }
        }
        ModelRequest::CancelPovRender => {
            let running = if let Some(job) = &model.pov_render_job {
                job.cancel.store(true, Ordering::Relaxed);
                model.status = "Stopping POV-Ray rendering...".to_string();
                true
            } else {
                false
            };
            Ok(ModelResponse::Message(if running {
                "stopping".to_string()
            } else {
                "idle".to_string()
            }))
        }
        ModelRequest::SavePovRenderImage { path } => {
            if path.trim().is_empty() {
                return Err("a POV-Ray image output path is required".to_string());
            }
            let result = model
                .pov_render_result
                .as_ref()
                .ok_or_else(|| "no POV-Ray image is available".to_string())?;
            if result.image_path != Path::new(&path) {
                fs::copy(&result.image_path, &path)
                    .map_err(|error| format!("failed to save rendered image: {error}"))?;
            }
            model.status = format!("Saved rendered image: {path}");
            Ok(ModelResponse::Message("saved".to_string()))
        }
        ModelRequest::ClearPovRenderResult => {
            if model.pov_render_job.is_some() {
                return Err("cannot clear a running POV-Ray render".to_string());
            }
            if let Some(result) = model.pov_render_result.take() {
                fs::remove_dir_all(result.work_dir)
                    .map_err(|error| format!("failed to clear POV-Ray result: {error}"))?;
            }
            Ok(ModelResponse::Message("cleared".to_string()))
        }
        ModelRequest::StartAnimationRender(input) => {
            if model.animation_render_job.is_some() {
                return Err("an animation rendering job is already running".to_string());
            }
            model.restore_animation_playback(false)?;
            let total_frames = model.animation_render_frame_count(&input)?;
            let restore = model.capture_undo();
            if let Err(error) = model.begin_animation_render(&input) {
                model.restore_undo(restore)?;
                return Err(error);
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let progress = Arc::new(AtomicU32::new(0));
            model.animation_render_job = Some(TauriAnimationRenderJob {
                input,
                restore,
                cancel,
                progress,
                total_frames,
                next_frame: 0,
                completed_frames: 0,
                frame_receivers: Vec::new(),
                movie_receiver: None,
                encoding_started: false,
            });
            model.status = format!("Animation rendering started: {total_frames} frame(s)");
            Ok(ModelResponse::Message(format!(
                "running:0:{total_frames}:0:frames"
            )))
        }
        ModelRequest::StartAnimationMovie {
            input,
            total_frames,
        } => {
            if model.animation_render_job.is_some() {
                return Err("an animation rendering job is already running".to_string());
            }
            model.restore_animation_playback(false)?;
            if !input.make_movie {
                return Err("movie encoding is disabled".to_string());
            }
            let expected_frames = model.animation_render_frame_count(&input)?;
            if total_frames != expected_frames {
                return Err(format!(
                    "movie frame count must be {expected_frames} for the current animation"
                ));
            }
            let progress = Arc::new(AtomicU32::new(950_000));
            model.animation_render_job = Some(TauriAnimationRenderJob {
                input,
                restore: model.capture_undo(),
                cancel: Arc::new(AtomicBool::new(false)),
                progress,
                total_frames,
                next_frame: total_frames,
                completed_frames: total_frames,
                frame_receivers: Vec::new(),
                movie_receiver: None,
                encoding_started: false,
            });
            model.status = "Animation movie encoding started".to_string();
            Ok(ModelResponse::Message(format!(
                "running:{total_frames}:{total_frames}:950000:movie"
            )))
        }
        ModelRequest::PollAnimationRender => {
            let Some(mut job) = model.animation_render_job.take() else {
                return Ok(ModelResponse::Message("idle".to_string()));
            };
            let mut pending_receivers = Vec::with_capacity(job.frame_receivers.len());
            let mut failure = None;
            for receiver in std::mem::take(&mut job.frame_receivers) {
                match receiver.try_recv() {
                    Err(mpsc::TryRecvError::Empty) => {
                        pending_receivers.push(receiver);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if !job.cancel.load(Ordering::Relaxed) {
                            failure.get_or_insert_with(|| {
                                "Animation rendering worker disconnected".to_string()
                            });
                        }
                    }
                    Ok(Err(error)) => {
                        if !job.cancel.load(Ordering::Relaxed) {
                            failure.get_or_insert(error);
                        }
                    }
                    Ok(Ok(TauriAnimationTaskOutput::Frame { index, path, log })) => {
                        job.completed_frames = job.completed_frames.saturating_add(1);
                        model
                            .log_lines
                            .push(format!("Animation frame {index:04}: {}", path.display()));
                        model.log_lines.extend(
                            log.lines()
                                .filter(|line| !line.trim().is_empty())
                                .map(|line| format!("Animation> {line}")),
                        );
                    }
                    Ok(Ok(TauriAnimationTaskOutput::Movie { .. })) => {
                        failure.get_or_insert_with(|| {
                            "Animation frame worker returned a movie".to_string()
                        });
                    }
                }
            }
            job.frame_receivers = pending_receivers;
            if let Some(error) = failure {
                job.cancel.store(true, Ordering::Relaxed);
                model.restore_undo(job.restore)?;
                model.status = error.clone();
                return Ok(ModelResponse::Message(format!("error:{error}")));
            }

            if let Some(receiver) = job.movie_receiver.take() {
                match receiver.try_recv() {
                    Err(mpsc::TryRecvError::Empty) => {
                        job.movie_receiver = Some(receiver);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        let error = "Animation movie worker disconnected".to_string();
                        model.restore_undo(job.restore)?;
                        model.status = error.clone();
                        return Ok(ModelResponse::Message(format!("error:{error}")));
                    }
                    Ok(Err(error)) => {
                        let error = if job.cancel.load(Ordering::Relaxed) {
                            "Animation rendering cancelled".to_string()
                        } else {
                            error
                        };
                        model.restore_undo(job.restore)?;
                        model.status = error.clone();
                        return Ok(ModelResponse::Message(format!("error:{error}")));
                    }
                    Ok(Ok(TauriAnimationTaskOutput::Movie { path, log })) => {
                        model
                            .log_lines
                            .push(format!("Animation movie: {}", path.display()));
                        if !log.trim().is_empty() {
                            model.log_lines.push(format!("Animation> {log}"));
                        }
                        let frame_count = job.completed_frames;
                        model.restore_undo(job.restore)?;
                        model.status = format!("Animation movie rendered: {frame_count} frame(s)");
                        return Ok(ModelResponse::Message(format!("complete:{frame_count}")));
                    }
                    Ok(Ok(TauriAnimationTaskOutput::Frame { .. })) => {
                        let error = "Animation movie worker returned a frame".to_string();
                        model.restore_undo(job.restore)?;
                        model.status = error.clone();
                        return Ok(ModelResponse::Message(format!("error:{error}")));
                    }
                }
            }
            if job.cancel.load(Ordering::Relaxed) {
                if job.frame_receivers.is_empty() && job.movie_receiver.is_none() {
                    model.restore_undo(job.restore)?;
                    let message = "Animation rendering cancelled".to_string();
                    model.status = message.clone();
                    return Ok(ModelResponse::Message(format!("error:{message}")));
                }
                model.animation_render_job = Some(job);
                return Ok(ModelResponse::Message("stopping".to_string()));
            }

            while job.next_frame < job.total_frames
                && job.frame_receivers.len() < job.input.concurrency as usize
            {
                let frame_index = job.next_frame;
                let task = match model.prepare_animation_frame_task(
                    &job.input,
                    frame_index,
                    job.total_frames,
                    Arc::clone(&job.cancel),
                    Arc::clone(&job.progress),
                ) {
                    Ok(task) => task,
                    Err(error) => {
                        job.cancel.store(true, Ordering::Relaxed);
                        model.restore_undo(job.restore)?;
                        model.status = error.clone();
                        return Ok(ModelResponse::Message(format!("error:{error}")));
                    }
                };
                let receiver = match spawn_tauri_animation_frame(task) {
                    Ok(receiver) => receiver,
                    Err(error) => {
                        job.cancel.store(true, Ordering::Relaxed);
                        model.restore_undo(job.restore)?;
                        model.status = error.clone();
                        return Ok(ModelResponse::Message(format!("error:{error}")));
                    }
                };
                job.next_frame += 1;
                job.frame_receivers.push(receiver);
            }
            if !job.frame_receivers.is_empty() || job.movie_receiver.is_some() {
                let progress = job.progress.load(Ordering::Relaxed);
                let phase = if job.encoding_started {
                    "movie"
                } else {
                    "frames"
                };
                let response = format!(
                    "running:{}:{}:{progress}:{phase}",
                    job.completed_frames, job.total_frames
                );
                model.animation_render_job = Some(job);
                return Ok(ModelResponse::Message(response));
            }

            if job.input.make_movie && !job.encoding_started {
                let task = TauriAnimationMovieTask {
                    input: job.input.clone(),
                    total_frames: job.total_frames,
                    cancel: Arc::clone(&job.cancel),
                    progress: Arc::clone(&job.progress),
                };
                match spawn_tauri_animation_movie(task) {
                    Ok(receiver) => {
                        job.encoding_started = true;
                        job.movie_receiver = Some(receiver);
                        job.progress.store(950_000, Ordering::Relaxed);
                        model
                            .log_lines
                            .push("Animation: encoding movie with FFmpeg".to_string());
                        let response = format!(
                            "running:{}:{}:950000:movie",
                            job.completed_frames, job.total_frames
                        );
                        model.animation_render_job = Some(job);
                        return Ok(ModelResponse::Message(response));
                    }
                    Err(error) => {
                        model.restore_undo(job.restore)?;
                        model.status = error.clone();
                        return Ok(ModelResponse::Message(format!("error:{error}")));
                    }
                }
            }

            if job.next_frame >= job.total_frames {
                let frame_count = job.completed_frames;
                let output_pattern = Path::new(&job.input.output_dir)
                    .join(format!("{}_frm_%04d.png", job.input.base_name.trim()));
                model.restore_undo(job.restore)?;
                model.status = format!("Animation rendered: {frame_count} frame(s)");
                model.log_lines.push(format!(
                    "Animation frames: {}",
                    output_pattern.to_string_lossy()
                ));
                return Ok(ModelResponse::Message(format!("complete:{frame_count}")));
            }
            let progress = job.progress.load(Ordering::Relaxed);
            let response = format!(
                "running:{}:{}:{progress}:frames",
                job.completed_frames, job.total_frames
            );
            model.animation_render_job = Some(job);
            Ok(ModelResponse::Message(response))
        }
        ModelRequest::CancelAnimationRender => {
            let running = if let Some(job) = &model.animation_render_job {
                job.cancel.store(true, Ordering::Relaxed);
                model.status = "Stopping animation rendering...".to_string();
                true
            } else {
                false
            };
            Ok(ModelResponse::Message(if running {
                "stopping".to_string()
            } else {
                "idle".to_string()
            }))
        }
        ModelRequest::AnimationRenderPreview {
            output_dir,
            base_name,
            frame_index,
        } => {
            let base_name = base_name.trim();
            if base_name.is_empty()
                || base_name.contains('/')
                || base_name.contains('\\')
                || base_name == "."
                || base_name == ".."
            {
                return Err("output base name must be a simple file name".to_string());
            }
            if frame_index > 9_999 {
                return Err("animation preview frame must not exceed 9999".to_string());
            }
            let path =
                Path::new(output_dir.trim()).join(format!("{base_name}_frm_{frame_index:04}.png"));
            if !path.is_file() {
                return Ok(ModelResponse::Message("missing".to_string()));
            }
            let image = fs::read(&path).map_err(|error| {
                format!(
                    "failed to read animation preview {}: {error}",
                    path.display()
                )
            })?;
            Ok(ModelResponse::Message(format!(
                "data:image/png;base64,{}",
                base64_encode(&image)
            )))
        }
        ModelRequest::Render { width, height } => {
            Ok(ModelResponse::Viewport(model.render(width, height)))
        }
        ModelRequest::UpdateCamera {
            action,
            x,
            y,
            width,
            height,
            render_scale,
        } => {
            model.update_camera(&action, x, y, height)?;
            let (render_width, render_height) =
                scaled_viewport_dimensions(width, height, render_scale);
            Ok(ModelResponse::Viewport(
                model.render(render_width, render_height),
            ))
        }
        ModelRequest::SaveScene { path, options } => {
            model.persist_camera()?;
            let path_buf = PathBuf::from(&path);
            let save_in_place = model
                .desktop
                .active_scene_source_path()
                .is_some_and(|source| source == path_buf.as_path());
            if save_in_place {
                if let Err(error) = backup_scene_file(&path_buf) {
                    model
                        .log_lines
                        .push(format!("WARNING: Cannot create backup file, {error}"));
                }
            }
            match options {
                Some(options) => model
                    .desktop
                    .save_active_scene_qsc_with_options(&path_buf, options),
                None => model.desktop.save_active_scene_qsc(&path_buf),
            }
            .map_err(|error| error.to_string())?;
            model.undo_stack.clear();
            model.redo_stack.clear();
            if let Some(scene_uid) = model.desktop.active_scene() {
                model.modified_scenes.remove(&scene_uid);
            }
            model.status = format!("Scene file: [{path}] is saved.");
            model.log_lines.push(model.status.clone());
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SaveObject {
            object_uid,
            object_kind,
            path,
        } => {
            if path.trim().is_empty() {
                return Err("an object output path is required".to_string());
            }
            let kind = match object_kind.as_str() {
                "object" => crate::GuiWorkspaceObjectKind::Molecule,
                "densityMap" => crate::GuiWorkspaceObjectKind::DensityMap,
                "surface" => crate::GuiWorkspaceObjectKind::Surface,
                "lwObject" => crate::GuiWorkspaceObjectKind::Lightweight,
                value => return Err(format!("unsupported Workspace object type: {value}")),
            };
            model
                .desktop
                .save_workspace_object_as(kind, Uid::new(object_uid), &path)
                .map_err(|error| error.to_string())?;
            model.status = format!("Saved object: {path}");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SaveView(path) => {
            if path.trim().is_empty() {
                return Err("a view output path is required".to_string());
            }
            fs::write(&path, camera_to_view_data(&model.camera))
                .map_err(|error| error.to_string())?;
            model.status = format!("Saved current view: {path}");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::LoadView(path) => {
            let data = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            model.camera = camera_from_view_data(&data)?;
            model.status = format!("Loaded view: {path}");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ExportPng {
            path,
            width,
            height,
            resolution_dpi,
            transparent,
        } => {
            model.persist_camera()?;
            model
                .desktop
                .export_active_scene_png_with_options(
                    &path,
                    crate::GuiPngExportOptions {
                        width: width.max(1),
                        height: height.max(1),
                        resolution_dpi,
                        transparent,
                    },
                    &model.selection,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("Exported image: {path}");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
    }
}

fn spawn_model(initial_paths: Vec<PathBuf>) -> ManagedState {
    let (sender, receiver) = mpsc::channel::<ModelEnvelope>();
    let config_path = default_config_path();
    std::thread::Builder::new()
        .name("cuemol-tauri-model".to_string())
        .spawn(move || {
            let mut model = AppModel::new_with_config(Vec::new(), config_path);
            while let Ok(envelope) = receiver.recv() {
                let result = handle_model_request(&mut model, envelope.request);
                let _ = envelope.response.send(result);
            }
        })
        .expect("spawn CueMol Tauri model thread");
    ManagedState {
        sender,
        startup_paths: Mutex::new(Some(initial_paths)),
    }
}

fn request_model(
    state: &State<'_, ManagedState>,
    request: ModelRequest,
) -> Result<ModelResponse, String> {
    let (sender, receiver) = mpsc::channel();
    state
        .sender
        .send(ModelEnvelope {
            request,
            response: sender,
        })
        .map_err(|_| "application model thread stopped".to_string())?;
    receiver
        .recv()
        .map_err(|_| "application model did not respond".to_string())?
}

fn snapshot_response(response: ModelResponse) -> Result<AppSnapshot, String> {
    match response {
        ModelResponse::Snapshot(snapshot) => Ok(*snapshot),
        ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected non-snapshot response".to_string()),
    }
}

fn molecule_details_response(response: ModelResponse) -> Result<MoleculeDetailsDto, String> {
    match response {
        ModelResponse::MoleculeDetails(details) => Ok(*details),
        ModelResponse::Snapshot(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected non-molecule-details response".to_string()),
    }
}

fn viewport_response(response: ModelResponse) -> Result<ViewportDto, String> {
    match response {
        ModelResponse::Viewport(viewport) => Ok(viewport),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected non-viewport response".to_string()),
    }
}

fn animation_frame_response(response: ModelResponse) -> Result<AnimationFrameDto, String> {
    match response {
        ModelResponse::AnimationFrame(frame) => Ok(frame),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected animation frame response".to_string()),
    }
}

fn trajectory_frame_response(response: ModelResponse) -> Result<TrajectoryFrameDto, String> {
    match response {
        ModelResponse::TrajectoryFrame(frame) => Ok(frame),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected trajectory frame response".to_string()),
    }
}

fn application_info_response(response: ModelResponse) -> Result<ApplicationInfoDto, String> {
    match response {
        ModelResponse::ApplicationInfo(info) => Ok(info),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected application info response".to_string()),
    }
}

fn preferences_response(response: ModelResponse) -> Result<PreferencesDto, String> {
    match response {
        ModelResponse::Preferences(preferences) => Ok(*preferences),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected non-preferences response".to_string()),
    }
}

fn preferences_update_response(response: ModelResponse) -> Result<PreferencesUpdateDto, String> {
    match response {
        ModelResponse::PreferencesUpdate(update) => Ok(*update),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => {
            Err("unexpected non-preferences-update response".to_string())
        }
    }
}

fn initial_state_response(response: ModelResponse) -> Result<InitialStateDto, String> {
    match response {
        ModelResponse::InitialState(initial) => Ok(*initial),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected non-initial-state response".to_string()),
    }
}

fn molecule_selection_update_response(
    response: ModelResponse,
) -> Result<MolStructUpdateDto, String> {
    match response {
        ModelResponse::MolStructUpdate(update) => Ok(*update),
        _ => Err("unexpected non-MolStruct-update response".to_string()),
    }
}

fn view_properties_response(response: ModelResponse) -> Result<ViewPropertiesDto, String> {
    match response {
        ModelResponse::ViewProperties(properties) => Ok(properties),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewPropertiesUpdate(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => Err("unexpected non-view-properties response".to_string()),
    }
}

fn style_properties_response(response: ModelResponse) -> Result<StylePropertiesDto, String> {
    match response {
        ModelResponse::StyleProperties(properties) => Ok(properties),
        _ => Err("unexpected non-style-properties response".to_string()),
    }
}

fn renderer_style_options_response(
    response: ModelResponse,
) -> Result<Vec<RendererStyleOptionDto>, String> {
    match response {
        ModelResponse::RendererStyleOptions(options) => Ok(options),
        _ => Err("unexpected non-renderer-style-options response".to_string()),
    }
}

fn atom_intr_defs_response(response: ModelResponse) -> Result<Vec<AtomIntrDefDto>, String> {
    match response {
        ModelResponse::AtomIntrDefs(entries) => Ok(entries),
        _ => Err("unexpected non-atom-interaction-definitions response".to_string()),
    }
}

fn camera_visibility_response(
    response: ModelResponse,
) -> Result<Vec<CameraVisibilityEntryDto>, String> {
    match response {
        ModelResponse::CameraVisibility(entries) => Ok(entries),
        _ => Err("unexpected non-camera-visibility response".to_string()),
    }
}

fn picked_atom_response(response: ModelResponse) -> Result<Option<PickedAtomDto>, String> {
    match response {
        ModelResponse::PickedAtom(atom) => Ok(atom),
        _ => Err("unexpected non-picked-atom response".to_string()),
    }
}

fn view_properties_update_response(
    response: ModelResponse,
) -> Result<ViewPropertiesUpdateDto, String> {
    match response {
        ModelResponse::ViewPropertiesUpdate(update) => Ok(*update),
        ModelResponse::Snapshot(_)
        | ModelResponse::MoleculeDetails(_)
        | ModelResponse::InitialState(_)
        | ModelResponse::MolStructUpdate(_)
        | ModelResponse::Viewport(_)
        | ModelResponse::AnimationFrame(_)
        | ModelResponse::TrajectoryFrame(_)
        | ModelResponse::ApplicationInfo(_)
        | ModelResponse::Preferences(_)
        | ModelResponse::PreferencesUpdate(_)
        | ModelResponse::ViewProperties(_)
        | ModelResponse::StyleProperties(_)
        | ModelResponse::RendererStyleOptions(_)
        | ModelResponse::AtomIntrDefs(_)
        | ModelResponse::FileRendererSetup(_)
        | ModelResponse::PdbRendererSetup(_)
        | ModelResponse::CameraVisibility(_)
        | ModelResponse::PickedAtom(_)
        | ModelResponse::Message(_) => {
            Err("unexpected non-view-properties-update response".to_string())
        }
    }
}

fn message_response(response: ModelResponse) -> Result<String, String> {
    match response {
        ModelResponse::Message(message) => Ok(message),
        _ => Err("unexpected non-message response".to_string()),
    }
}

#[tauri::command]
fn get_snapshot(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::GetSnapshot)?)
}

#[tauri::command]
fn get_molecule_details(
    structure_key: String,
    structure_object_uid: Option<u64>,
    sequence_key: String,
    include_sequence: bool,
    object_uid: Option<u64>,
    state: State<'_, ManagedState>,
) -> Result<MoleculeDetailsDto, String> {
    molecule_details_response(request_model(
        &state,
        ModelRequest::GetMoleculeDetails {
            structure_key,
            structure_object_uid,
            sequence_key,
            include_sequence,
            object_uid,
        },
    )?)
}

#[tauri::command]
fn get_initial_state(
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<InitialStateDto, String> {
    let mut initial = initial_state_response(request_model(
        &state,
        ModelRequest::GetInitialState { width, height },
    )?)?;
    initial.startup_paths_pending = state
        .startup_paths
        .lock()
        .map_err(|_| "startup path state is unavailable".to_string())?
        .as_ref()
        .is_some_and(|paths| !paths.is_empty());
    Ok(initial)
}

#[tauri::command]
fn get_application_info(state: State<'_, ManagedState>) -> Result<ApplicationInfoDto, String> {
    application_info_response(request_model(&state, ModelRequest::GetApplicationInfo)?)
}

#[tauri::command]
fn get_preferences(state: State<'_, ManagedState>) -> Result<PreferencesDto, String> {
    preferences_response(request_model(&state, ModelRequest::GetPreferences)?)
}

#[tauri::command]
fn set_preferences(
    preferences: PreferencesDto,
    state: State<'_, ManagedState>,
) -> Result<PreferencesUpdateDto, String> {
    preferences_update_response(request_model(
        &state,
        ModelRequest::SetPreferences(preferences),
    )?)
}

#[tauri::command]
fn get_view_properties(state: State<'_, ManagedState>) -> Result<ViewPropertiesDto, String> {
    view_properties_response(request_model(&state, ModelRequest::GetViewProperties)?)
}

#[tauri::command]
fn get_camera_properties(
    name: String,
    state: State<'_, ManagedState>,
) -> Result<ViewPropertiesDto, String> {
    view_properties_response(request_model(
        &state,
        ModelRequest::GetCameraProperties(name),
    )?)
}

#[tauri::command]
fn set_view_properties(
    properties: ViewPropertiesInput,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<ViewPropertiesUpdateDto, String> {
    view_properties_update_response(request_model(
        &state,
        ModelRequest::SetViewProperties {
            properties,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn set_camera_properties(
    old_name: String,
    properties: ViewPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetCameraProperties {
            old_name,
            properties,
        },
    )?)
}

#[tauri::command]
fn set_scene_properties(
    properties: ScenePropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetSceneProperties(properties),
    )?)
}

#[tauri::command]
fn set_object_properties(
    properties: ObjectPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetObjectProperties(properties),
    )?)
}

#[tauri::command]
fn set_renderer_properties(
    properties: RendererPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetRendererProperties(properties),
    )?)
}

#[tauri::command]
fn set_density_map_properties(
    properties: ObjectPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetDensityMapProperties(properties),
    )?)
}

#[tauri::command]
fn set_density_renderer_properties(
    properties: RendererPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetDensityRendererProperties(properties),
    )?)
}

#[tauri::command]
fn set_density_renderer_visible(
    map_uid: u64,
    renderer_uid: u64,
    visible: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetDensityRendererVisible {
            map_uid,
            renderer_uid,
            visible,
        },
    )?)
}

#[tauri::command]
fn add_density_renderer(
    map_uid: u64,
    renderer_type: String,
    group_name: Option<String>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AddDensityRenderer {
            map_uid,
            renderer_type,
            group_name,
        },
    )?)
}

#[tauri::command]
fn generate_density_surface(
    map_uid: u64,
    renderer_uid: u64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::GenerateDensitySurface {
            map_uid,
            renderer_uid,
        },
    )?)
}

#[tauri::command]
fn set_surface_properties(
    properties: ObjectPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetSurfaceProperties(properties),
    )?)
}

#[tauri::command]
fn set_surface_renderer_properties(
    properties: RendererPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetSurfaceRendererProperties(properties),
    )?)
}

#[tauri::command]
fn set_lw_object_properties(
    properties: ObjectPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetLwObjectProperties(properties),
    )?)
}

#[tauri::command]
fn set_lw_renderer_properties(
    properties: LwRendererPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetLwRendererProperties(properties),
    )?)
}

#[tauri::command]
fn add_lw_renderer(
    object_uid: u64,
    group_name: Option<String>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AddLwRenderer {
            object_uid,
            group_name,
        },
    )?)
}

#[tauri::command]
fn set_surface_renderer_visible(
    surface_uid: u64,
    renderer_uid: u64,
    visible: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetSurfaceRendererVisible {
            surface_uid,
            renderer_uid,
            visible,
        },
    )?)
}

#[tauri::command]
fn add_surface_renderer(
    surface_uid: u64,
    group_name: Option<String>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AddSurfaceRenderer {
            surface_uid,
            group_name,
        },
    )?)
}

#[tauri::command]
fn regenerate_surface(
    surface_uid: u64,
    density: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RegenerateSurface {
            surface_uid,
            density,
        },
    )?)
}

#[tauri::command]
fn create_camera(name: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::CreateCamera(name))?)
}

#[tauri::command]
fn save_camera(name: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SaveCamera(name))?)
}

#[tauri::command]
fn load_camera(name: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::LoadCamera(name))?)
}

#[tauri::command]
fn save_camera_view(name: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SaveCameraView(name))?)
}

#[tauri::command]
fn load_camera_view(name: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::LoadCameraView(name))?)
}

#[tauri::command]
fn load_camera_file(
    scene_uid: u64,
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::LoadCameraFile { scene_uid, path },
    )?)
}

#[tauri::command]
fn reload_camera_file(
    scene_uid: u64,
    name: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ReloadCameraFile { scene_uid, name },
    )?)
}

#[tauri::command]
fn save_camera_file(
    scene_uid: u64,
    name: String,
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SaveCameraFile {
            scene_uid,
            name,
            path,
        },
    )?)
}

#[tauri::command]
fn destroy_camera(name: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::DestroyCamera(name))?)
}

#[tauri::command]
fn get_camera_visibility(
    scene_uid: u64,
    name: String,
    state: State<'_, ManagedState>,
) -> Result<Vec<CameraVisibilityEntryDto>, String> {
    camera_visibility_response(request_model(
        &state,
        ModelRequest::GetCameraVisibility { scene_uid, name },
    )?)
}

#[tauri::command]
fn set_camera_visibility(
    input: CameraVisibilityUpdateInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetCameraVisibility(input),
    )?)
}

#[tauri::command]
fn clear_camera_visibility(
    scene_uid: u64,
    name: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ClearCameraVisibility { scene_uid, name },
    )?)
}

#[tauri::command]
fn create_style(name: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::CreateStyle(name))?)
}

#[tauri::command]
fn load_style_file(
    scene_uid: u64,
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::LoadStyleFile { scene_uid, path },
    )?)
}

#[tauri::command]
fn reload_style_file(
    scene_uid: u64,
    local_index: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ReloadStyleFile {
            scene_uid,
            local_index,
        },
    )?)
}

#[tauri::command]
fn save_style_file(
    scene_uid: u64,
    local_index: usize,
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SaveStyleFile {
            scene_uid,
            local_index,
            path,
        },
    )?)
}

#[tauri::command]
fn create_renderer_style(
    input: RendererStyleCreateInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::CreateRendererStyle(input),
    )?)
}

#[tauri::command]
fn get_style_properties(
    scene_uid: u64,
    local_index: usize,
    state: State<'_, ManagedState>,
) -> Result<StylePropertiesDto, String> {
    style_properties_response(request_model(
        &state,
        ModelRequest::GetStyleProperties {
            scene_uid,
            local_index,
        },
    )?)
}

#[tauri::command]
fn get_renderer_style_options(
    scene_uid: u64,
    renderer_type: String,
    state: State<'_, ManagedState>,
) -> Result<Vec<RendererStyleOptionDto>, String> {
    renderer_style_options_response(request_model(
        &state,
        ModelRequest::GetRendererStyleOptions {
            scene_uid,
            renderer_type,
        },
    )?)
}

#[tauri::command]
fn get_workspace_renderer_setup_options(
    scene_uid: u64,
    kind: String,
    state: State<'_, ManagedState>,
) -> Result<WorkspaceRendererSetupOptionsDto, String> {
    let options = renderer_style_options_response(request_model(
        &state,
        ModelRequest::GetWorkspaceRendererSetupOptions { scene_uid, kind },
    )?)?;
    let preferences = crate::load_renderer_setup_preferences();
    let renderer_type = preferences
        .renderer_type
        .filter(|renderer_type| options.iter().any(|option| option.name == *renderer_type));
    Ok(WorkspaceRendererSetupOptionsDto {
        renderer_type,
        recenter: preferences.recenter,
        options,
    })
}

#[tauri::command]
fn get_atom_intr_defs(
    object_uid: u64,
    renderer_uid: u64,
    state: State<'_, ManagedState>,
) -> Result<Vec<AtomIntrDefDto>, String> {
    atom_intr_defs_response(request_model(
        &state,
        ModelRequest::GetAtomIntrDefs {
            object_uid,
            renderer_uid,
        },
    )?)
}

#[tauri::command]
fn remove_atom_intr_defs(
    object_uid: u64,
    renderer_uid: u64,
    ids: Vec<usize>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RemoveAtomIntrDefs {
            object_uid,
            renderer_uid,
            ids,
        },
    )?)
}

#[tauri::command]
fn set_style_properties(
    properties: StylePropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetStyleProperties(properties),
    )?)
}

#[tauri::command]
fn destroy_style(
    scene_uid: u64,
    local_index: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::DestroyStyle {
            scene_uid,
            local_index,
        },
    )?)
}

#[tauri::command]
fn set_workspace_visible(
    target: WorkspaceToggleInput,
    visible: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetWorkspaceVisible { target, visible },
    )?)
}

#[tauri::command]
fn set_workspace_items_visible(
    targets: Vec<WorkspaceToggleInput>,
    visible: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetWorkspaceItemsVisible { targets, visible },
    )?)
}

#[tauri::command]
fn set_workspace_locked(
    target: WorkspaceToggleInput,
    locked: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetWorkspaceLocked { target, locked },
    )?)
}

#[tauri::command]
fn delete_workspace_item(
    target: WorkspaceDeleteInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::DeleteWorkspaceItem(target),
    )?)
}

#[tauri::command]
fn delete_workspace_items(
    targets: Vec<WorkspaceDeleteInput>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::DeleteWorkspaceItems(targets),
    )?)
}

#[tauri::command]
fn move_workspace_renderer(
    input: WorkspaceMoveRendererInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MoveWorkspaceRenderer(input),
    )?)
}

#[tauri::command]
fn move_workspace_renderers(
    input: WorkspaceMoveRenderersInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MoveWorkspaceRenderers(input),
    )?)
}

#[tauri::command]
fn set_workspace_object_collapsed(
    input: WorkspaceObjectCollapsedInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetWorkspaceObjectCollapsed(input),
    )?)
}

#[tauri::command]
fn set_workspace_renderer_collapsed(
    input: WorkspaceRendererCollapsedInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetWorkspaceRendererCollapsed(input),
    )?)
}

#[tauri::command]
fn move_workspace_object(
    input: WorkspaceMoveObjectInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MoveWorkspaceObject(input),
    )?)
}

#[tauri::command]
fn set_workspace_renderer_selection(
    input: WorkspaceRendererSelectionInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetWorkspaceRendererSelection(input),
    )?)
}

#[tauri::command]
fn change_workspace_renderer_type(
    input: WorkspaceRendererTypeInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ChangeWorkspaceRendererType(input),
    )?)
}

#[tauri::command]
fn create_workspace_renderer(
    input: WorkspaceNewRendererInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    let renderer_type = input.renderer_type.clone();
    let recenter = input
        .recenter
        .unwrap_or_else(|| crate::load_renderer_setup_preferences().recenter);
    let snapshot = snapshot_response(request_model(
        &state,
        ModelRequest::CreateWorkspaceRenderer(input),
    )?)?;
    let _ = crate::save_renderer_setup_preferences(&renderer_type, recenter);
    Ok(snapshot)
}

#[tauri::command]
fn apply_workspace_renderer_style(
    input: WorkspaceRendererStyleInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ApplyWorkspaceRendererStyle(input),
    )?)
}

#[tauri::command]
fn set_workspace_renderer_styles(
    input: WorkspaceRendererStylesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetWorkspaceRendererStyles(input),
    )?)
}

#[tauri::command]
fn set_workspace_renderer_coloring(
    input: WorkspaceRendererColoringInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetWorkspaceRendererColoring(input),
    )?)
}

#[tauri::command]
fn insert_workspace_renderer_paint(
    input: WorkspaceRendererPaintInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::InsertWorkspaceRendererPaint(input),
    )?)
}

#[tauri::command]
fn insert_workspace_object_paint(
    input: WorkspaceObjectPaintInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::InsertWorkspaceObjectPaint(input),
    )?)
}

#[tauri::command]
fn copy_workspace_renderer(
    input: WorkspaceRendererCopyInput,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::CopyWorkspaceRenderer(input),
    )?)
}

#[tauri::command]
fn copy_workspace_renderers(
    inputs: Vec<WorkspaceRendererCopyInput>,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::CopyWorkspaceRenderers(inputs),
    )?)
}

#[tauri::command]
fn paste_workspace_renderers(
    input: WorkspaceRendererPasteInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::PasteWorkspaceRenderers(input),
    )?)
}

#[tauri::command]
fn copy_workspace_object(
    input: WorkspaceObjectCopyInput,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::CopyWorkspaceObject(input),
    )?)
}

#[tauri::command]
fn paste_workspace_object(
    input: WorkspaceObjectPasteInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::PasteWorkspaceObject(input),
    )?)
}

#[tauri::command]
fn copy_workspace_camera(
    input: WorkspaceCameraCopyInput,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::CopyWorkspaceCamera(input),
    )?)
}

#[tauri::command]
fn paste_workspace_camera(
    input: WorkspaceCameraPasteInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::PasteWorkspaceCamera(input),
    )?)
}

#[tauri::command]
fn rename_workspace_camera(
    input: WorkspaceCameraRenameInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RenameWorkspaceCamera(input),
    )?)
}

#[tauri::command]
fn copy_workspace_style(
    input: WorkspaceStyleCopyInput,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::CopyWorkspaceStyle(input),
    )?)
}

#[tauri::command]
fn paste_workspace_style(
    input: WorkspaceStylePasteInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::PasteWorkspaceStyle(input),
    )?)
}

#[tauri::command]
fn create_workspace_renderer_group(
    input: WorkspaceRendererGroupInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::CreateWorkspaceRendererGroup(input),
    )?)
}

#[tauri::command]
fn rename_workspace_renderer_group(
    input: WorkspaceRendererGroupInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RenameWorkspaceRendererGroup(input),
    )?)
}

#[tauri::command]
fn rename_workspace_node(
    input: WorkspaceRenameInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RenameWorkspaceNode(input),
    )?)
}

#[tauri::command]
fn fit_workspace_item(
    target: WorkspaceFitInput,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<ViewportDto, String> {
    viewport_response(request_model(
        &state,
        ModelRequest::FitWorkspaceItem {
            target,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn set_config_value(
    key: String,
    value: String,
    state: State<'_, ManagedState>,
) -> Result<ApplicationInfoDto, String> {
    application_info_response(request_model(
        &state,
        ModelRequest::SetConfigValue { key, value },
    )?)
}

#[tauri::command]
fn undo(depth: usize, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::Undo(depth))?)
}

#[tauri::command]
fn redo(depth: usize, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::Redo(depth))?)
}

#[tauri::command]
fn clear_undo(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ClearUndo)?)
}

#[tauri::command]
fn open_paths(
    paths: Vec<String>,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    molecule_selection_update_response(request_model(
        &state,
        ModelRequest::OpenPathsViewport {
            paths,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn open_scene(
    path: String,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    molecule_selection_update_response(request_model(
        &state,
        ModelRequest::OpenSceneViewport {
            path,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn open_initial_paths(
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    let paths = state
        .startup_paths
        .lock()
        .map_err(|_| "startup path state is unavailable".to_string())?
        .take()
        .unwrap_or_default();
    if paths.is_empty() {
        let response = request_model(&state, ModelRequest::GetInitialState { width, height })?;
        return match response {
            ModelResponse::InitialState(initial) => Ok(MolStructUpdateDto {
                snapshot: initial.snapshot,
                viewport: initial.viewport,
            }),
            _ => Err("unexpected initial-path response".to_string()),
        };
    }
    let _ = request_model(&state, ModelRequest::StartRendererWarmup)?;
    molecule_selection_update_response(request_model(
        &state,
        ModelRequest::OpenPathsViewport {
            paths: paths
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn reload_scene(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ReloadScene)?)
}

#[tauri::command]
fn get_pdb_history() -> Vec<String> {
    crate::load_pdb_history()
}

#[tauri::command]
fn get_file_renderer_setup(
    path: String,
    state: State<'_, ManagedState>,
) -> Result<Option<RendererSetupDto>, String> {
    if path.trim().is_empty() {
        return Err("a file path is required".to_string());
    }
    let _ = request_model(&state, ModelRequest::StartRendererWarmup)?;
    match request_model(&state, ModelRequest::GetFileRendererSetup { path })? {
        ModelResponse::FileRendererSetup(setup) => Ok(setup),
        _ => Err("unexpected file renderer setup response".to_string()),
    }
}

#[tauri::command]
fn open_path_with_renderer_setup(
    path: String,
    file_setup: FileRendererSetupInput,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    let target = match file_setup.target_kind.as_str() {
        "molecule" => crate::GuiFileRendererTarget::Molecule,
        "densityMap" => crate::GuiFileRendererTarget::DensityMap,
        "surface" => crate::GuiFileRendererTarget::Surface,
        target => return Err(format!("unsupported renderer setup target: {target}")),
    };
    let reader_options = file_setup
        .reader_options
        .map(crate::GuiMoleculeReaderOptions::try_from)
        .transpose()?;
    let namd_coor_options = file_setup
        .namd_coor_options
        .map(crate::GuiNamdCoorReaderOptions::from);
    let msms_options = file_setup
        .msms_options
        .map(crate::GuiMsmsReaderOptions::from);
    let density_map_options = match (file_setup.ccp4_map_options, file_setup.mtz_map_options) {
        (Some(_), Some(_)) => {
            return Err("only one density-map reader option set is allowed".to_string())
        }
        (Some(options), None) => Some(crate::GuiDensityMapReaderOptions::Ccp4(
            crate::GuiCcp4MapReaderOptions::from(options),
        )),
        (None, Some(options)) => Some(crate::GuiDensityMapReaderOptions::Mtz(
            crate::GuiMtzMapReaderOptions::from(options),
        )),
        (None, None) => None,
    };
    let renderer_setup = crate::GuiRendererSetup::from(file_setup.renderer_setup);
    let _ = crate::save_renderer_setup_preferences(
        &renderer_setup.renderer_type,
        renderer_setup.recenter,
    );
    molecule_selection_update_response(request_model(
        &state,
        ModelRequest::OpenPathWithRendererSetup {
            path,
            target,
            reader_options,
            namd_coor_options,
            density_map_options,
            msms_options,
            renderer_setup,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn get_pdb_renderer_setup(
    pdb_id: String,
    state: State<'_, ManagedState>,
) -> Result<RendererSetupDto, String> {
    let pdb_id = crate::normalize_pdb_id(&pdb_id)?;
    let _ = request_model(&state, ModelRequest::StartRendererWarmup)?;
    match request_model(&state, ModelRequest::GetPdbRendererSetup { pdb_id })? {
        ModelResponse::PdbRendererSetup(setup) => Ok(setup),
        _ => Err("unexpected PDB renderer setup response".to_string()),
    }
}

fn pdb_fetch_options(
    pdb_id: String,
    fetch_coordinates: bool,
    coordinate_source: String,
    fetch_two_fofc: bool,
    fetch_fofc: bool,
    map_source: String,
) -> crate::PdbFetchOptions {
    crate::PdbFetchOptions {
        pdb_id,
        fetch_coordinates,
        coordinate_source,
        fetch_two_fofc,
        fetch_fofc,
        map_source,
    }
}

#[tauri::command]
async fn check_pdb_availability(
    pdb_id: String,
    fetch_coordinates: bool,
    coordinate_source: String,
    fetch_two_fofc: bool,
    fetch_fofc: bool,
    map_source: String,
) -> Result<Vec<String>, String> {
    let options = pdb_fetch_options(
        pdb_id,
        fetch_coordinates,
        coordinate_source,
        fetch_two_fofc,
        fetch_fofc,
        map_source,
    );
    let unavailable = tauri::async_runtime::spawn_blocking(move || crate::probe_pdb_files(options))
        .await
        .map_err(|error| format!("PDB availability task failed: {error}"))??;
    Ok(unavailable
        .into_iter()
        .map(|kind| match kind {
            crate::GuiPdbFileKind::Coordinates => "coordinates",
            crate::GuiPdbFileKind::TwoFoFc => "2fofc",
            crate::GuiPdbFileKind::FoFc => "fofc",
        })
        .map(str::to_string)
        .collect())
}

#[tauri::command]
async fn prefetch_pdb(
    pdb_id: String,
    fetch_coordinates: bool,
    coordinate_source: String,
    fetch_two_fofc: bool,
    fetch_fofc: bool,
    map_source: String,
) -> Result<(), String> {
    let options = pdb_fetch_options(
        pdb_id,
        fetch_coordinates,
        coordinate_source,
        fetch_two_fofc,
        fetch_fofc,
        map_source,
    );
    tauri::async_runtime::spawn_blocking(move || crate::download_pdb_files(options))
        .await
        .map_err(|error| format!("PDB prefetch task failed: {error}"))??;
    Ok(())
}

#[tauri::command]
fn get_pdb_download_progress(
    pdb_id: String,
    fetch_coordinates: bool,
    coordinate_source: String,
    fetch_two_fofc: bool,
    fetch_fofc: bool,
    map_source: String,
) -> Result<u64, String> {
    crate::pdb_downloaded_bytes(&pdb_fetch_options(
        pdb_id,
        fetch_coordinates,
        coordinate_source,
        fetch_two_fofc,
        fetch_fofc,
        map_source,
    ))
}

#[tauri::command]
fn cancel_pdb_download(
    pdb_id: String,
    fetch_coordinates: bool,
    coordinate_source: String,
    fetch_two_fofc: bool,
    fetch_fofc: bool,
    map_source: String,
) -> Result<(), String> {
    crate::cancel_pdb_download(&pdb_fetch_options(
        pdb_id,
        fetch_coordinates,
        coordinate_source,
        fetch_two_fofc,
        fetch_fofc,
        map_source,
    ))?;
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn open_pdb(
    pdb_id: String,
    fetch_coordinates: bool,
    coordinate_source: String,
    fetch_two_fofc: bool,
    fetch_fofc: bool,
    map_source: String,
    renderer_setup: Option<RendererSetupInput>,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    let pdb_id = crate::normalize_pdb_id(&pdb_id)?;
    let renderer_setup = renderer_setup.map(crate::GuiRendererSetup::from);
    if fetch_coordinates != renderer_setup.is_some() {
        return Err(if fetch_coordinates {
            "PDB coordinates require renderer setup".to_string()
        } else {
            "renderer setup is only valid when coordinates are selected".to_string()
        });
    }
    if let Some(setup) = renderer_setup.as_ref() {
        let _ = crate::save_renderer_setup_preferences(&setup.renderer_type, setup.recenter);
    }
    let options = pdb_fetch_options(
        pdb_id.clone(),
        fetch_coordinates,
        coordinate_source,
        fetch_two_fofc,
        fetch_fofc,
        map_source,
    );
    let _ = request_model(&state, ModelRequest::StartRendererWarmup)?;
    let files = tauri::async_runtime::spawn_blocking(move || crate::download_pdb_files(options))
        .await
        .map_err(|error| format!("PDB download task failed: {error}"))??;
    let update = molecule_selection_update_response(request_model(
        &state,
        ModelRequest::OpenPdb {
            pdb_id: pdb_id.clone(),
            files,
            renderer_setup,
            width,
            height,
        },
    )?)?;
    let _ = crate::remember_pdb_id(&pdb_id);
    Ok(update)
}

#[tauri::command]
fn load_trajectory(
    topology_path: String,
    trajectory_paths: Vec<String>,
    nevery: usize,
    renderer_setup: RendererSetupInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::LoadTrajectory {
            topology_path,
            trajectory_paths,
            nevery,
            renderer_setup: renderer_setup.into(),
        },
    )?)
}

#[tauri::command]
fn set_trajectory_frame(
    object_uid: u64,
    frame: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetTrajectoryFrame { object_uid, frame },
    )?)
}

#[tauri::command]
fn set_trajectory_frame_viewport(
    object_uid: u64,
    frame: usize,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<TrajectoryFrameDto, String> {
    trajectory_frame_response(request_model(
        &state,
        ModelRequest::SetTrajectoryFrameViewport {
            object_uid,
            frame,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn append_trajectory_block(
    object_uid: u64,
    path: String,
    nevery: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AppendTrajectoryBlock {
            object_uid,
            path,
            nevery,
        },
    )?)
}

#[tauri::command]
fn remove_trajectory_block(
    object_uid: u64,
    index: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RemoveTrajectoryBlock { object_uid, index },
    )?)
}

#[tauri::command]
fn move_trajectory_block(
    object_uid: u64,
    from: usize,
    to: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MoveTrajectoryBlock {
            object_uid,
            from,
            to,
        },
    )?)
}

#[tauri::command]
fn new_scene(name: Option<String>, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::NewScene(name))?)
}

#[tauri::command]
fn new_tab(
    mode: String,
    name: String,
    inherit_view_props: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::NewTab {
            mode,
            name,
            inherit_view_props,
        },
    )?)
}

#[tauri::command]
fn activate_scene(scene_uid: u64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ActivateScene(scene_uid),
    )?)
}

#[tauri::command]
fn activate_view(view_uid: u64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ActivateView(view_uid))?)
}

#[tauri::command]
fn close_active_scene(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::CloseActiveScene)?)
}

#[tauri::command]
fn set_selection(selection: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetSelection(selection),
    )?)
}

#[tauri::command]
fn molecule_selection_action(
    object_uid: u64,
    selection: String,
    mode: String,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    molecule_selection_update_response(request_model(
        &state,
        ModelRequest::MolStructAction {
            object_uid,
            selection,
            mode,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn set_renderer_visible(
    object_uid: u64,
    renderer_uid: u64,
    visible: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetRendererVisible {
            object_uid,
            renderer_uid,
            visible,
        },
    )?)
}

#[tauri::command]
fn add_renderer(
    object_uid: u64,
    renderer_type: String,
    group_name: Option<String>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AddRenderer {
            object_uid,
            renderer_type,
            group_name,
        },
    )?)
}

#[tauri::command]
fn set_object_coloring(
    object_uid: u64,
    coloring: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetObjectColoring {
            object_uid,
            coloring,
        },
    )?)
}

#[tauri::command]
fn set_coloring(
    update: ColoringUpdateInput,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    molecule_selection_update_response(request_model(
        &state,
        ModelRequest::SetColoring {
            input: update,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn delete_atoms(
    object_uid: u64,
    selection: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::DeleteAtoms {
            object_uid,
            selection,
        },
    )?)
}

#[tauri::command]
fn change_chain(
    object_uid: u64,
    selection: String,
    chain_name: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ChangeChain {
            object_uid,
            selection,
            chain_name,
        },
    )?)
}

#[tauri::command]
fn renumber_residues(
    object_uid: u64,
    selection: String,
    start: i32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RenumberResidues {
            object_uid,
            selection,
            start,
        },
    )?)
}

#[tauri::command]
fn merge_molecules(
    source_object_uid: u64,
    selection: String,
    target_object_uid: u64,
    copy: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MergeMolecules {
            source_object_uid,
            selection,
            target_object_uid,
            copy,
        },
    )?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn superpose_objects(
    algorithm: String,
    reference_object_uid: u64,
    reference_selection: String,
    moving_object_uid: u64,
    moving_selection: String,
    auto_recenter: bool,
    use_property: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SuperposeObjects {
            algorithm,
            reference_object_uid,
            reference_selection,
            moving_object_uid,
            moving_selection,
            auto_recenter,
            use_property,
        },
    )?)
}

#[tauri::command]
fn edit_bond(
    object_uid: u64,
    aid1: i32,
    aid2: i32,
    make: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::EditBond {
            object_uid,
            aid1,
            aid2,
            make,
        },
    )?)
}

#[tauri::command]
fn get_nonstandard_bonds(
    object_uid: u64,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::GetNonstandardBonds { object_uid },
    )?)
}

#[tauri::command]
fn remove_nonstandard_bonds(
    object_uid: u64,
    pairs: Vec<[i32; 2]>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RemoveNonstandardBonds { object_uid, pairs },
    )?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn calculate_contacts(
    object_uid: u64,
    selection1: String,
    object2_uid: Option<u64>,
    selection2: Option<String>,
    r_min: f64,
    r_max: f64,
    hbond: bool,
    max_count: usize,
    renderer_name: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::CalculateContacts {
            object_uid,
            selection1,
            object2_uid,
            selection2,
            r_min,
            r_max,
            hbond,
            max_count,
            renderer_name,
        },
    )?)
}

#[tauri::command]
fn get_space_groups(lattice: String, state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::GetSpaceGroups { lattice },
    )?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn change_crystal_info(
    object_uid: u64,
    a: f64,
    b: f64,
    c: f64,
    alpha: f64,
    beta: f64,
    gamma: f64,
    space_group_number: i32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ChangeCrystalInfo {
            object_uid,
            a,
            b,
            c,
            alpha,
            beta,
            gamma,
            space_group_number,
        },
    )?)
}

#[tauri::command]
fn assign_secondary_structure(
    object_uid: u64,
    ignore_bulge: bool,
    angle: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AssignSecondaryStructure {
            object_uid,
            ignore_bulge,
            angle,
        },
    )?)
}

#[tauri::command]
fn create_molecular_surface(
    object_uid: u64,
    selection: String,
    name: String,
    density: f64,
    probe_radius: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::CreateMolecularSurface {
            object_uid,
            selection,
            name,
            density,
            probe_radius,
        },
    )?)
}

#[tauri::command]
fn write_surface(
    object_uid: u64,
    selection: String,
    density: f64,
    probe_radius: f64,
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::WriteSurface {
            object_uid,
            selection,
            density,
            probe_radius,
            path,
        },
    )?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn write_cut_surface(
    object_uid: u64,
    selection: String,
    density: f64,
    probe_radius: f64,
    normal: [f64; 3],
    position: [f64; 3],
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::WriteCutSurface {
            object_uid,
            selection,
            density,
            probe_radius,
            normal,
            position,
            path,
        },
    )?)
}

#[tauri::command]
fn cut_surface(
    surface_uid: u64,
    density: f64,
    mode: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::CutSurface {
            surface_uid,
            density,
            mode,
        },
    )?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn write_apbs_files(
    object_uid: u64,
    selection: String,
    pqr_path: String,
    input_path: String,
    dx_path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::WriteApbsFiles {
            object_uid,
            selection,
            pqr_path,
            input_path,
            dx_path,
        },
    )?)
}

#[tauri::command]
fn start_apbs_calculation(
    input: ApbsCalculationInput,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::StartApbsCalculation(input),
    )?)
}

#[tauri::command]
fn poll_apbs_calculation(state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(&state, ModelRequest::PollApbsCalculation)?)
}

#[tauri::command]
fn cancel_apbs_calculation(state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(&state, ModelRequest::CancelApbsCalculation)?)
}

#[tauri::command]
fn append_morph_frame(
    target_object_uid: u64,
    source_object_uid: u64,
    frame_name: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AppendMorphFrame {
            target_object_uid,
            source_object_uid,
            frame_name,
        },
    )?)
}

#[tauri::command]
fn remove_morph_frame(
    object_uid: u64,
    index: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RemoveMorphFrame { object_uid, index },
    )?)
}

#[tauri::command]
fn apply_morph_frame(
    object_uid: u64,
    frame: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ApplyMorphFrame { object_uid, frame },
    )?)
}

#[tauri::command]
fn set_display_option(
    option: String,
    value: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetDisplayOption { option, value },
    )?)
}

#[tauri::command]
fn set_density_level(level: f64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SetDensityLevel(level))?)
}

#[tauri::command]
fn set_background(
    background: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetBackground(background),
    )?)
}

#[tauri::command]
fn pick_atom(
    x: f64,
    y: f64,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    molecule_selection_update_response(request_model(
        &state,
        ModelRequest::PickAtom {
            x,
            y,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn pick_atom_identifier(
    x: f64,
    y: f64,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<Option<PickedAtomDto>, String> {
    picked_atom_response(request_model(
        &state,
        ModelRequest::PickAtomIdentifier {
            x,
            y,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn select_screen_rect(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<MolStructUpdateDto, String> {
    molecule_selection_update_response(request_model(
        &state,
        ModelRequest::SelectScreenRect {
            x1,
            y1,
            x2,
            y2,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn add_animation_object(
    kind: String,
    after_index: Option<usize>,
    input: Option<AnimationObjectPropertiesInput>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    let request = match input {
        Some(properties) => ModelRequest::AddConfiguredAnimationObject {
            kind,
            after_index,
            properties,
        },
        None => ModelRequest::AddAnimationObject { kind, after_index },
    };
    snapshot_response(request_model(&state, request)?)
}

#[tauri::command]
fn delete_animation_object(
    index: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::DeleteAnimationObject(index),
    )?)
}

#[tauri::command]
fn delete_animation_objects(
    indices: Vec<usize>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::DeleteAnimationObjects(indices),
    )?)
}

#[tauri::command]
fn move_animation_object(
    index: usize,
    direction: i32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MoveAnimationObject { index, direction },
    )?)
}

#[tauri::command]
fn move_animation_objects(
    indices: Vec<usize>,
    direction: i32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MoveAnimationObjects { indices, direction },
    )?)
}

#[tauri::command]
fn set_animation_duration(
    index: usize,
    duration: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetAnimationDuration { index, duration },
    )?)
}

#[tauri::command]
fn set_animation_object_properties(
    input: AnimationObjectPropertiesInput,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetAnimationObjectProperties(input),
    )?)
}

#[tauri::command]
fn animation_control(
    action: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AnimationControl(action),
    )?)
}

#[tauri::command]
fn set_animation_loop(value: bool, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetAnimationLoop(value),
    )?)
}

#[tauri::command]
fn set_animation_start_camera(
    name: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetAnimationStartCamera(name),
    )?)
}

#[tauri::command]
fn seek_animation(elapsed: f64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SeekAnimation(elapsed))?)
}

#[tauri::command]
fn tick_animation(dt: f64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::TickAnimation(dt))?)
}

#[tauri::command]
fn tick_animation_frame(
    dt: f64,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<AnimationFrameDto, String> {
    animation_frame_response(request_model(
        &state,
        ModelRequest::TickAnimationFrame { dt, width, height },
    )?)
}

#[tauri::command]
fn clear_log(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ClearLog)?)
}

#[tauri::command]
fn execute_script_file(
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ExecuteScriptFile(path),
    )?)
}

#[tauri::command]
fn export_pov(path: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ExportPov { path })?)
}

#[tauri::command]
fn start_pov_render(
    input: PovRenderInput,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(&state, ModelRequest::StartPovRender(input))?)
}

#[tauri::command]
fn poll_pov_render(state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(&state, ModelRequest::PollPovRender)?)
}

#[tauri::command]
fn cancel_pov_render(state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(&state, ModelRequest::CancelPovRender)?)
}

#[tauri::command]
fn save_pov_render_image(path: String, state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::SavePovRenderImage { path },
    )?)
}

#[tauri::command]
fn clear_pov_render_result(state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(&state, ModelRequest::ClearPovRenderResult)?)
}

#[tauri::command]
fn start_animation_render(
    input: AnimationRenderInput,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::StartAnimationRender(input),
    )?)
}

#[tauri::command]
fn start_animation_movie(
    input: AnimationRenderInput,
    total_frames: u32,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::StartAnimationMovie {
            input,
            total_frames,
        },
    )?)
}

#[tauri::command]
fn poll_animation_render(state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(&state, ModelRequest::PollAnimationRender)?)
}

#[tauri::command]
fn cancel_animation_render(state: State<'_, ManagedState>) -> Result<String, String> {
    message_response(request_model(&state, ModelRequest::CancelAnimationRender)?)
}

#[tauri::command]
fn animation_render_preview(
    output_dir: String,
    base_name: String,
    frame_index: u32,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    message_response(request_model(
        &state,
        ModelRequest::AnimationRenderPreview {
            output_dir,
            base_name,
            frame_index,
        },
    )?)
}

#[tauri::command]
fn render_viewport(
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<ViewportDto, String> {
    viewport_response(request_model(
        &state,
        ModelRequest::Render { width, height },
    )?)
}

#[tauri::command]
fn update_camera(
    action: String,
    x: f64,
    y: f64,
    width: u32,
    height: u32,
    render_scale: Option<f64>,
    state: State<'_, ManagedState>,
) -> Result<ViewportDto, String> {
    viewport_response(request_model(
        &state,
        ModelRequest::UpdateCamera {
            action,
            x,
            y,
            width,
            height,
            render_scale: render_scale.unwrap_or(1.0),
        },
    )?)
}

#[tauri::command]
fn save_scene(
    path: String,
    options: Option<QscSaveOptionsDto>,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    let options = options.map(TryInto::try_into).transpose()?;
    snapshot_response(request_model(
        &state,
        ModelRequest::SaveScene { path, options },
    )?)
}

#[tauri::command]
fn save_object(
    object_uid: u64,
    object_kind: Option<String>,
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SaveObject {
            object_uid,
            object_kind: object_kind.unwrap_or_else(|| "object".to_string()),
            path,
        },
    )?)
}

#[tauri::command]
fn save_view(path: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SaveView(path))?)
}

#[tauri::command]
fn load_view(path: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::LoadView(path))?)
}

#[tauri::command]
fn export_png(
    path: String,
    width: u32,
    height: u32,
    resolution_dpi: f64,
    transparent: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ExportPng {
            path,
            width,
            height,
            resolution_dpi,
            transparent,
        },
    )?)
}

fn backup_scene_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let mut backup_name = path.as_os_str().to_os_string();
    backup_name.push(".bak");
    let backup_path = PathBuf::from(backup_name);
    if backup_path.exists() {
        fs::remove_file(&backup_path).map_err(|error| {
            format!(
                "cannot remove old backup file {}: {error}",
                backup_path.display()
            )
        })?;
    }
    fs::rename(path, &backup_path).map_err(|error| {
        format!(
            "cannot create backup file {}: {error}",
            backup_path.display()
        )
    })
}

fn camera_to_view_data(camera: &Camera) -> String {
    let center = camera.center();
    let rotation = camera.rotation();
    let ortho_scale = 1000.0 / camera.zoom().max(0.1);
    format!(
        "cuemol2-rs-view\t3\n\
target\t{:.6}\t{:.6}\t{:.6}\n\
rotation\t{:.6}\t{:.6}\t{:.6}\t{:.6}\n\
distance\t{:.6}\n\
slab\t{:.6}\n\
ortho_scale\t{:.6}\n\
perspective\t{}\n\
stereo_mode\t{}\n\
stereo_distance\t{:.6}\n\
center_mark\t{}\n",
        center.x,
        center.y,
        center.z,
        rotation.x,
        rotation.y,
        rotation.z,
        rotation.w,
        camera.cam_dist(),
        camera.slab_depth(),
        ortho_scale,
        camera.is_perspective(),
        stereo_mode_name(camera.stereo_mode()),
        camera.stereo_distance(),
        center_mark_name(camera.center_mark()),
    )
}

fn camera_from_view_data(data: &str) -> Result<Camera, String> {
    let mut camera = Camera::new();
    let mut saw_header = false;
    for line in data.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.as_slice() {
            [] => {}
            ["cuemol2-rs-view", "1" | "2" | "3"] => saw_header = true,
            ["target", x, y, z] => {
                camera.set_center(Vector4::new3(
                    parse_view_number(x)?,
                    parse_view_number(y)?,
                    parse_view_number(z)?,
                ));
            }
            ["rotation", x, y, z, w] => {
                let rotation = Quat::new(
                    parse_view_number(x)?,
                    parse_view_number(y)?,
                    parse_view_number(z)?,
                    parse_view_number(w)?,
                )
                .normalize()
                .map_err(|error| error.to_string())?;
                camera.set_rotation(rotation);
            }
            ["distance", value] => camera.set_cam_dist(parse_view_number(value)?),
            ["slab", value] => camera.set_slab_depth(parse_view_number(value)?),
            ["ortho_scale", value] => {
                let scale = parse_view_number(value)?;
                if scale <= 0.0 {
                    return Err("orthographic scale must be positive".to_string());
                }
                camera.set_zoom(1000.0 / scale);
            }
            ["perspective", "true"] => camera.set_perspective(true),
            ["perspective", "false"] => camera.set_perspective(false),
            ["stereo_mode", value] => camera.set_stereo_mode(parse_stereo_mode(value)?),
            ["stereo_distance", value] => camera.set_stereo_distance(parse_view_number(value)?),
            ["center_mark", value] => camera.set_center_mark(parse_center_mark(value)?),
            _ => return Err(format!("invalid view line: {line}")),
        }
    }
    if !saw_header {
        return Err("missing view header".to_string());
    }
    Ok(camera)
}

fn parse_view_number(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| format!("invalid view number: {value}"))
}

fn frame_camera(snapshot: &RenderSceneSnapshot) -> Camera {
    let mut camera = Camera::new();
    if let Some((min, max)) = snapshot.bounds {
        let size = ((max.x - min.x).powi(2) + (max.y - min.y).powi(2) + (max.z - min.z).powi(2))
            .sqrt()
            .max(1.0);
        camera.set_center(Vector4::new3(
            (min.x + max.x) / 2.0,
            (min.y + max.y) / 2.0,
            (min.z + max.z) / 2.0,
        ));
        camera.set_zoom(size * 1.2);
        camera.set_cam_dist(size * 3.0);
        camera.set_slab_depth(size * 2.5);
    }
    camera
}

fn pan_center(camera: &Camera, dx_pix: f64, dy_pix: f64, viewport_height: u32) -> Vector4 {
    let scale = camera.zoom() / f64::from(viewport_height.max(1));
    let rotation = camera.rotation().to_matrix();
    let center = camera.center();
    let camera_x = dx_pix * scale;
    let camera_y = -dy_pix * scale;
    Vector4::new3(
        center.x - camera_x * rotation.values[0][0] - camera_y * rotation.values[1][0],
        center.y - camera_x * rotation.values[0][1] - camera_y * rotation.values[1][1],
        center.z - camera_x * rotation.values[0][2] - camera_y * rotation.values[1][2],
    )
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let byte_0 = u32::from(chunk[0]);
        let byte_1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let byte_2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let value = (byte_0 << 16) | (byte_1 << 8) | byte_2;
        output.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

pub fn run() -> Result<(), tauri::Error> {
    let initial_paths = std::env::args_os().skip(1).map(PathBuf::from).collect();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(spawn_model(initial_paths))
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_molecule_details,
            get_initial_state,
            get_application_info,
            get_preferences,
            set_preferences,
            get_view_properties,
            get_camera_properties,
            set_view_properties,
            set_camera_properties,
            set_scene_properties,
            set_object_properties,
            set_renderer_properties,
            set_density_map_properties,
            set_density_renderer_properties,
            set_density_renderer_visible,
            add_density_renderer,
            generate_density_surface,
            set_surface_properties,
            set_surface_renderer_properties,
            set_lw_object_properties,
            set_lw_renderer_properties,
            add_lw_renderer,
            set_surface_renderer_visible,
            add_surface_renderer,
            regenerate_surface,
            create_camera,
            save_camera,
            load_camera,
            save_camera_view,
            load_camera_view,
            load_camera_file,
            reload_camera_file,
            save_camera_file,
            destroy_camera,
            get_camera_visibility,
            set_camera_visibility,
            clear_camera_visibility,
            create_style,
            load_style_file,
            reload_style_file,
            save_style_file,
            create_renderer_style,
            get_style_properties,
            get_renderer_style_options,
            get_workspace_renderer_setup_options,
            get_atom_intr_defs,
            remove_atom_intr_defs,
            set_style_properties,
            destroy_style,
            set_workspace_visible,
            set_workspace_items_visible,
            set_workspace_locked,
            delete_workspace_item,
            delete_workspace_items,
            move_workspace_object,
            move_workspace_renderer,
            move_workspace_renderers,
            set_workspace_object_collapsed,
            set_workspace_renderer_collapsed,
            set_workspace_renderer_selection,
            change_workspace_renderer_type,
            create_workspace_renderer,
            apply_workspace_renderer_style,
            set_workspace_renderer_styles,
            set_workspace_renderer_coloring,
            insert_workspace_renderer_paint,
            insert_workspace_object_paint,
            copy_workspace_renderer,
            copy_workspace_renderers,
            paste_workspace_renderers,
            copy_workspace_object,
            paste_workspace_object,
            copy_workspace_camera,
            paste_workspace_camera,
            rename_workspace_camera,
            copy_workspace_style,
            paste_workspace_style,
            create_workspace_renderer_group,
            rename_workspace_renderer_group,
            rename_workspace_node,
            fit_workspace_item,
            set_config_value,
            undo,
            redo,
            clear_undo,
            open_paths,
            open_scene,
            open_initial_paths,
            reload_scene,
            get_pdb_history,
            get_file_renderer_setup,
            open_path_with_renderer_setup,
            get_pdb_renderer_setup,
            check_pdb_availability,
            prefetch_pdb,
            get_pdb_download_progress,
            cancel_pdb_download,
            open_pdb,
            load_trajectory,
            set_trajectory_frame,
            set_trajectory_frame_viewport,
            append_trajectory_block,
            remove_trajectory_block,
            move_trajectory_block,
            new_scene,
            new_tab,
            activate_scene,
            activate_view,
            close_active_scene,
            set_selection,
            molecule_selection_action,
            set_renderer_visible,
            add_renderer,
            set_object_coloring,
            set_coloring,
            delete_atoms,
            change_chain,
            renumber_residues,
            merge_molecules,
            superpose_objects,
            edit_bond,
            get_nonstandard_bonds,
            remove_nonstandard_bonds,
            calculate_contacts,
            get_space_groups,
            change_crystal_info,
            assign_secondary_structure,
            create_molecular_surface,
            write_surface,
            write_cut_surface,
            cut_surface,
            write_apbs_files,
            start_apbs_calculation,
            poll_apbs_calculation,
            cancel_apbs_calculation,
            append_morph_frame,
            remove_morph_frame,
            apply_morph_frame,
            set_display_option,
            set_density_level,
            set_background,
            pick_atom,
            pick_atom_identifier,
            select_screen_rect,
            add_animation_object,
            delete_animation_object,
            delete_animation_objects,
            move_animation_object,
            move_animation_objects,
            set_animation_duration,
            set_animation_object_properties,
            animation_control,
            set_animation_loop,
            set_animation_start_camera,
            seek_animation,
            tick_animation,
            tick_animation_frame,
            clear_log,
            execute_script_file,
            export_pov,
            start_pov_render,
            poll_pov_render,
            cancel_pov_render,
            save_pov_render_image,
            clear_pov_render_result,
            start_animation_render,
            start_animation_movie,
            poll_animation_render,
            cancel_animation_render,
            animation_render_preview,
            render_viewport,
            update_camera,
            save_scene,
            save_object,
            save_view,
            load_view,
            export_png,
        ])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qsc_save_options_dto_validates_reference_values() {
        let options: crate::GuiSceneSaveOptions = QscSaveOptionsDto {
            embed_all: true,
            version: "QDF1".to_string(),
            compress: "xzip".to_string(),
            base64: true,
        }
        .try_into()
        .unwrap();
        assert_eq!(options.version, crate::GuiQscVersion::Qdf1);
        assert_eq!(options.compression, crate::GuiQscCompression::Xz);
        assert!(options.embed_all);
        assert!(options.base64);
        assert!(crate::GuiSceneSaveOptions::try_from(QscSaveOptionsDto {
            embed_all: false,
            version: "QDF2".to_string(),
            compress: "none".to_string(),
            base64: false,
        })
        .is_err());
    }

    #[test]
    fn qsc_save_request_applies_explicit_scene_options() {
        let path =
            std::env::temp_dir().join(format!("cuemol-qsc-options-{}.qsc", std::process::id()));
        let mut model = model_without_gpu();
        handle_model_request(
            &mut model,
            ModelRequest::SaveScene {
                path: path.to_string_lossy().into_owned(),
                options: Some(crate::GuiSceneSaveOptions {
                    embed_all: true,
                    version: crate::GuiQscVersion::Qdf1,
                    compression: crate::GuiQscCompression::Gzip,
                    base64: true,
                }),
            },
        )
        .unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("base64=\"true\" compress=\"gzip\" version=\"QDF1\""));
        fs::remove_file(path).unwrap();
    }

    fn model_without_gpu() -> AppModel {
        let mut desktop = DesktopAppState::new();
        desktop.ensure_active_scene();
        AppModel {
            desktop,
            config_path: None,
            camera: Camera::new(),
            renderer: None,
            renderer_warmup: None,
            renderer_init_attempted: false,
            mesh: None,
            screen_lines: Vec::new(),
            mesh_refit_pending: false,
            selection: String::new(),
            background: "black".to_string(),
            show_unit_cell: false,
            show_symmetry_mates: false,
            show_density_contour: false,
            color_proofing: false,
            density_level: 1.5,
            animation_elapsed: 0.0,
            animation_playing: false,
            animation_start_camera: None,
            animation_start_desktop: None,
            view_width: 100,
            view_height: 100,
            trans_mms: false,
            rot_mms: false,
            swap_stereo_eyes: false,
            opened_paths: Vec::new(),
            structure_revision: 1,
            modified_scenes: BTreeSet::new(),
            workspace_camera_clipboard: None,
            workspace_object_clipboard: None,
            workspace_renderer_clipboard: None,
            workspace_style_clipboard: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            apbs_job: None,
            pov_render_job: None,
            pov_render_result: None,
            animation_render_job: None,
            log_lines: Vec::new(),
            status: "Ready".to_string(),
        }
    }

    fn add_test_animation(model: &mut AppModel, kind: AnimKind, duration: f64) {
        let mut animation = model.active_animation_manager().unwrap();
        let mut motion = new_animation_motion(
            kind,
            unique_animation_name(&animation, animation_name_prefix(kind)),
            String::new(),
        );
        motion.base_mut().set_rel_end(animation_time(duration));
        animation.append_motion(motion);
        model.set_active_animation_manager(&animation).unwrap();
    }

    fn animation_properties_input(object: &AnimationObjectDto) -> AnimationObjectPropertiesInput {
        AnimationObjectPropertiesInput {
            index: object.index,
            name: object.name.clone(),
            disabled: object.disabled,
            time_ref_name: object.time_ref_name.clone(),
            start: object.relative_start,
            duration: object.duration,
            quadric: object.quadric,
            angle: object.angle,
            axis: object.axis,
            target_camera: object.target_camera.clone(),
            ignore_center: object.ignore_center,
            ignore_rotate: object.ignore_rotate,
            ignore_zoom: object.ignore_zoom,
            ignore_slab: object.ignore_slab,
            keep_quaternion_positive: object.keep_quaternion_positive,
            renderer_names: object.renderer_names.clone(),
            hide: object.hide,
            fade: object.fade,
            target_alpha: object.target_alpha,
            direction: object.direction,
            distance: object.distance,
            molecule_name: object.molecule_name.clone(),
            property_name: object.property_name.clone(),
            start_value: object.start_value,
            end_value: object.end_value,
            start_position: object.start_position,
            end_position: object.end_position,
        }
    }

    fn model_with_renderer() -> (AppModel, Uid, Uid) {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let scene = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap();
        let object_uid =
            scene.add_molecule_with_source_type("molecule", cuemol_molstr::MolCoord::new(), "pdb");
        let renderer_uid = scene
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "simple 1");
        (model, object_uid, renderer_uid)
    }

    fn default_renderer_properties_input(
        object_uid: Uid,
        renderer_uid: Uid,
        name: &str,
    ) -> RendererPropertiesInput {
        RendererPropertiesInput {
            object_uid: object_uid.get(),
            renderer_uid: renderer_uid.get(),
            name: name.to_string(),
            selection: "*".to_string(),
            visible: true,
            locked: false,
            material: String::new(),
            alpha: 1.0,
            edge_type: "none".to_string(),
            edge_width: 0.01,
            edge_color: "#000000".to_string(),
            line_width: 1.2,
            detail: 3,
            bond_width: 0.2,
            sphere_radius: 0.3,
            show_ring: false,
            ring_thickness: 0.2,
            ring_color: "#ffffbf".to_string(),
            specific: BTreeMap::new(),
        }
    }

    #[test]
    fn frame_camera_fits_snapshot_bounds() {
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.bounds = Some((
            Vector4::new3(-2.0, -1.0, -3.0),
            Vector4::new3(2.0, 3.0, 1.0),
        ));
        let camera = frame_camera(&snapshot);
        assert_eq!(camera.center(), Vector4::new3(0.0, 1.0, -1.0));
        assert!(camera.zoom() > 4.0);
    }

    #[test]
    fn current_view_text_roundtrips_the_qsys_camera() {
        let mut camera = Camera::new();
        camera.set_center(Vector4::new3(1.0, 2.0, 3.0));
        camera.set_cam_dist(80.0);
        camera.set_zoom(25.0);
        camera.set_slab_depth(42.0);
        camera.set_perspective(true);
        camera.set_stereo_mode(StereoMode::Cross);
        camera.set_stereo_distance(2.5);
        camera.set_center_mark(CenterMark::Axis);
        camera.set_rotation(Quat::new(0.0, 0.0, 0.0, 1.0));
        let restored = camera_from_view_data(&camera_to_view_data(&camera)).unwrap();
        assert_eq!(restored.center(), camera.center());
        assert_eq!(restored.cam_dist(), camera.cam_dist());
        assert_eq!(restored.zoom(), camera.zoom());
        assert_eq!(restored.slab_depth(), camera.slab_depth());
        assert_eq!(restored.rotation(), camera.rotation());
        assert!(restored.is_perspective());
        assert_eq!(restored.stereo_mode(), StereoMode::Cross);
        assert_eq!(restored.stereo_distance(), 2.5);
        assert_eq!(restored.center_mark(), CenterMark::Axis);
    }

    #[test]
    fn base64_encoder_uses_standard_padding() {
        assert_eq!(base64_encode(b"CueMol"), "Q3VlTW9s");
        assert_eq!(base64_encode(b"rs"), "cnM=");
    }

    #[test]
    fn interactive_viewport_dimensions_reduce_pixels_and_preserve_full_renders() {
        assert_eq!(scaled_viewport_dimensions(1_000, 800, 0.625), (625, 500));
        assert_eq!(
            scaled_viewport_dimensions(3_200, 2_400, 0.625),
            (1_000, 1_000)
        );
        assert_eq!(scaled_viewport_dimensions(80, 80, 0.1), (64, 64));
        assert_eq!(scaled_viewport_dimensions(1_000, 800, 2.0), (1_000, 800));
        assert_eq!(
            scaled_viewport_dimensions(1_000, 800, f64::NAN),
            (1_000, 800)
        );

        let mut model = model_without_gpu();
        let response = handle_model_request(
            &mut model,
            ModelRequest::UpdateCamera {
                action: "pan".to_string(),
                x: 1.0,
                y: 0.0,
                width: 1_000,
                height: 800,
                render_scale: 0.625,
            },
        )
        .unwrap();
        let ModelResponse::Viewport(viewport) = response else {
            panic!("expected interactive viewport response");
        };
        assert_eq!((viewport.width, viewport.height), (625, 500));
    }

    #[test]
    fn startup_defers_mesh_and_gpu_initialization_until_viewport_render() {
        let mut model = AppModel::new(Vec::new());
        assert!(model.renderer.is_none());
        assert!(model.renderer_warmup.is_none());
        assert!(!model.renderer_init_attempted);
        assert!(model.mesh.is_none());
        model.prepare_mesh_if_needed().unwrap();
        assert!(model
            .mesh
            .as_ref()
            .is_some_and(|mesh| mesh.indices.is_empty()));
        assert!(!model.renderer_init_attempted);
    }

    #[test]
    fn renderer_warmup_runs_off_thread_and_is_not_repeated() {
        let mut model = model_without_gpu();
        let (started_sender, started_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        model.start_renderer_warmup_with(move || {
            started_sender.send(()).expect("signal renderer warmup");
            release_receiver.recv().expect("release renderer warmup");
            None
        });
        started_receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("renderer warmup starts in the background");
        assert!(model.renderer_init_attempted);
        assert!(model.renderer_warmup.is_some());

        let repeated = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let repeated_flag = repeated.clone();
        model.start_renderer_warmup_with(move || {
            repeated_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            None
        });
        assert!(!repeated.load(std::sync::atomic::Ordering::SeqCst));

        release_sender.send(()).expect("finish renderer warmup");
        model.finish_renderer_warmup();
        assert!(model.renderer_warmup.is_none());
        assert!(model.renderer.is_none());
    }

    #[test]
    fn initial_state_combines_snapshot_and_viewport_without_starting_the_gpu() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("initial", cuemol_molstr::MolCoord::new());
        model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("deferred", cuemol_molstr::MolCoord::new());
        let response = handle_model_request(
            &mut model,
            ModelRequest::GetInitialState {
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::InitialState(initial) = response else {
            panic!("expected the combined initial state");
        };
        assert_eq!(initial.viewport.width, 800);
        assert_eq!(initial.viewport.height, 600);
        assert!(initial.viewport.image.is_none());
        assert!(!initial.startup_paths_pending);
        assert_eq!(initial.snapshot.scenes.len(), 1);
        assert_eq!(initial.snapshot.views.len(), 1);
        assert_eq!(initial.snapshot.objects.len(), 2);
        assert!(!initial.snapshot.structure_key.is_empty());
        assert!(!initial.snapshot.sequence_key.is_empty());
        let object_uid = initial.snapshot.objects.first().map(|object| object.uid);
        let ModelResponse::MoleculeDetails(details) = handle_model_request(
            &mut model,
            ModelRequest::GetMoleculeDetails {
                structure_key: String::new(),
                structure_object_uid: None,
                sequence_key: String::new(),
                include_sequence: false,
                object_uid,
            },
        )
        .unwrap() else {
            panic!("expected molecule details");
        };
        assert_eq!(details.structure_key, initial.snapshot.structure_key);
        assert_eq!(details.structure_object_uid, object_uid);
        assert_eq!(details.sequence_key, initial.snapshot.sequence_key);
        assert_eq!(details.structure.as_ref().map(Vec::len), Some(1));
        assert!(details.sequence.is_none());
        let second_uid = Some(initial.snapshot.objects[1].uid);
        let ModelResponse::MoleculeDetails(second) = handle_model_request(
            &mut model,
            ModelRequest::GetMoleculeDetails {
                structure_key: initial.snapshot.structure_key.clone(),
                structure_object_uid: object_uid,
                sequence_key: String::new(),
                include_sequence: false,
                object_uid: second_uid,
            },
        )
        .unwrap() else {
            panic!("expected object-scoped molecule details");
        };
        assert_eq!(second.structure_object_uid, second_uid);
        assert_eq!(second.structure.as_ref().map(Vec::len), Some(1));
        assert_eq!(second.structure.unwrap()[0].object_name, "deferred");
        let ModelResponse::MoleculeDetails(cached) = handle_model_request(
            &mut model,
            ModelRequest::GetMoleculeDetails {
                structure_key: initial.snapshot.structure_key.clone(),
                structure_object_uid: object_uid,
                sequence_key: String::new(),
                include_sequence: true,
                object_uid,
            },
        )
        .unwrap() else {
            panic!("expected cached molecule details");
        };
        assert!(cached.structure.is_none());
        assert!(cached.sequence.is_some());
        let ModelResponse::MoleculeDetails(unchanged) = handle_model_request(
            &mut model,
            ModelRequest::GetMoleculeDetails {
                structure_key: initial.snapshot.structure_key.clone(),
                structure_object_uid: object_uid,
                sequence_key: initial.snapshot.sequence_key.clone(),
                include_sequence: true,
                object_uid,
            },
        )
        .unwrap() else {
            panic!("expected molecule details");
        };
        assert!(unchanged.structure.is_none());
        assert!(unchanged.sequence.is_none());
        assert!(model.renderer.is_none());
    }

    #[test]
    fn new_tab_request_creates_inherited_views_and_named_scenes() {
        let mut model = model_without_gpu();
        assert_eq!(
            model.desktop.active_scene_summary().unwrap().name,
            "Untitled 1"
        );
        let first_summary = model.desktop.active_view_summary().unwrap();
        assert_eq!(first_summary.name, "0");
        let first_view = first_summary.uid;
        model.camera.set_zoom(81.0);
        let response = handle_model_request(
            &mut model,
            ModelRequest::NewTab {
                mode: "new-view".to_string(),
                name: "Detail".to_string(),
                inherit_view_props: true,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected snapshot");
        };
        assert_eq!(snapshot.views.len(), 2);
        assert!(snapshot
            .views
            .iter()
            .any(|view| view.name == "Detail" && view.active));
        assert_eq!(model.camera.zoom(), 81.0);

        model.camera.set_zoom(42.0);
        handle_model_request(&mut model, ModelRequest::ActivateView(first_view.get())).unwrap();
        assert_eq!(model.camera.zoom(), 81.0);

        let response = handle_model_request(
            &mut model,
            ModelRequest::NewTab {
                mode: "new-scene".to_string(),
                name: "Second".to_string(),
                inherit_view_props: false,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected snapshot");
        };
        assert_eq!(snapshot.scenes.len(), 2);
        assert_eq!(snapshot.views.len(), 3);
        assert!(snapshot.scenes.iter().any(|scene| scene.name == "Second"));
    }

    #[test]
    fn close_tab_request_closes_a_view_before_its_scene() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        model.desktop.new_view(scene_uid, "1", None).unwrap();

        let response = handle_model_request(&mut model, ModelRequest::CloseActiveScene).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected snapshot");
        };
        assert_eq!(snapshot.scenes.len(), 1);
        assert_eq!(snapshot.views.len(), 1);
        assert_eq!(model.desktop.active_scene(), Some(scene_uid));

        let response = handle_model_request(&mut model, ModelRequest::CloseActiveScene).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected snapshot");
        };
        assert!(snapshot.scenes.is_empty());
        assert!(snapshot.views.is_empty());
        assert_eq!(model.desktop.active_scene(), None);
    }

    #[test]
    fn open_paths_returns_before_gpu_frame_encoding() {
        let path =
            std::env::temp_dir().join(format!("cuemol-deferred-open-{}.pdb", std::process::id()));
        fs::write(
            &path,
            b"ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let mut model = model_without_gpu();
        let response = handle_model_request(
            &mut model,
            ModelRequest::OpenPathsViewport {
                paths: vec![path.to_string_lossy().into_owned()],
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected molecule update");
        };
        assert_eq!(update.snapshot.objects.len(), 1);
        assert!(update.viewport.image.is_none());
        assert!(model.mesh.is_none());
        assert!(model.mesh_refit_pending);
        assert_eq!(update.viewport.width, 800);
        assert_eq!(update.viewport.height, 600);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn open_scene_reuses_an_empty_tab_then_creates_a_new_tab() {
        let first_path = std::env::temp_dir().join(format!(
            "cuemol-open-scene-first-{}.qsc",
            std::process::id()
        ));
        let second_path = std::env::temp_dir().join(format!(
            "cuemol-open-scene-second-{}.qsc",
            std::process::id()
        ));
        fs::write(&first_path, "<scene name=\"first\"/>").unwrap();
        fs::write(&second_path, "<scene name=\"second\"/>").unwrap();

        let mut model = model_without_gpu();
        let empty_scene_uid = model.desktop.active_scene().unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::OpenSceneViewport {
                path: first_path.to_string_lossy().into_owned(),
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected scene update");
        };
        assert_eq!(update.snapshot.scenes.len(), 1);
        assert_eq!(
            update.snapshot.active_scene_uid,
            Some(empty_scene_uid.get())
        );
        assert_eq!(update.snapshot.opened_paths, [first_path.to_string_lossy()]);
        assert!(!update.snapshot.can_undo);
        assert!(!update.snapshot.scenes[0].modified);
        assert_eq!(
            update.snapshot.scenes[0].name,
            first_path.file_name().unwrap().to_string_lossy()
        );
        assert_eq!(
            update.snapshot.scenes[0].source,
            first_path.to_string_lossy()
        );
        assert_eq!(update.snapshot.scenes[0].source_type, "qsc_xml");
        assert!(model.mesh_refit_pending);

        let response = handle_model_request(
            &mut model,
            ModelRequest::OpenSceneViewport {
                path: second_path.to_string_lossy().into_owned(),
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected scene update");
        };
        assert_eq!(update.snapshot.scenes.len(), 2);
        assert_ne!(
            update.snapshot.active_scene_uid,
            Some(empty_scene_uid.get())
        );
        assert_eq!(
            update.snapshot.opened_paths,
            [second_path.to_string_lossy()]
        );

        fs::remove_file(first_path).unwrap();
        fs::remove_file(second_path).unwrap();
    }

    #[test]
    fn failed_open_scene_preserves_the_active_tab_and_opened_path() {
        let directory = std::env::temp_dir().join(format!(
            "cuemol-tauri-open-scene-rollback-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("broken.qsc");
        fs::write(
            &path,
            "<scene><object type=\"DensityMap\" name=\"broken-map\" \
             src=\"broken.dx\" srctype=\"apbs\"/></scene>",
        )
        .unwrap();
        fs::write(directory.join("broken.dx"), "not an OpenDX map").unwrap();

        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        model.opened_paths = vec!["existing.pdb".to_string()];

        let error = model
            .open_scene(path.to_string_lossy().into_owned())
            .unwrap_err();
        assert!(error.contains("OpenDX"));
        assert_eq!(model.desktop.active_scene(), Some(scene_uid));
        assert_eq!(model.desktop.scene_summaries().len(), 1);
        assert!(model.desktop.active_scene_is_just_created());
        assert_eq!(model.opened_paths, ["existing.pdb"]);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn open_pse_uses_the_scene_reader_tab_contract() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cuemol-importers/tests/fixtures/mol3.pse");
        let mut model = model_without_gpu();
        let empty_scene_uid = model.desktop.active_scene().unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::OpenSceneViewport {
                path: path.to_string_lossy().into_owned(),
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.scenes.len(), 1);
        assert_eq!(snapshot.active_scene_uid, Some(empty_scene_uid.get()));
        assert!(!snapshot.objects.is_empty());
        let scene = model
            .desktop
            .scene_manager()
            .scene(empty_scene_uid)
            .unwrap();
        assert_eq!(scene.name(), path.file_name().unwrap().to_string_lossy());
        assert!(scene.source().is_empty());
        assert!(scene.source_type().is_empty());
        assert!(scene.base_path().is_empty());
        assert!(!snapshot.can_undo);
        assert!(snapshot.scenes[0].modified);
    }

    #[test]
    fn open_scene_applies_its_saved_camera_to_a_reused_view() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-open-scene-camera-{}.qsc",
            std::process::id()
        ));
        let mut source = DesktopAppState::new();
        source.new_scene("camera", true);
        let mut camera = Camera::new();
        camera.set_name("__current");
        camera.set_zoom(73.0);
        source.set_active_scene_camera(camera).unwrap();
        source.save_active_scene_qsc(&path).unwrap();

        let mut model = model_without_gpu();
        let empty_scene_uid = model.desktop.active_scene().unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::OpenSceneViewport {
                path: path.to_string_lossy().into_owned(),
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        assert_eq!(model.desktop.active_scene(), Some(empty_scene_uid));
        assert_eq!(model.camera.zoom(), 73.0);
        assert_eq!(model.desktop.active_view_camera().unwrap().zoom(), 73.0);
        assert!(!model.mesh_refit_pending);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn file_open_renderer_setup_precedes_molecule_loading() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-renderer-setup-open-{}.pdb",
            std::process::id()
        ));
        fs::write(
            &path,
            b"ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let path_text = path.to_string_lossy().into_owned();
        let mut model = model_without_gpu();
        let setup_response = handle_model_request(
            &mut model,
            ModelRequest::GetFileRendererSetup {
                path: path_text.clone(),
            },
        )
        .unwrap();
        let ModelResponse::FileRendererSetup(Some(defaults)) = setup_response else {
            panic!("expected file renderer setup");
        };
        assert_eq!(
            defaults.object_name,
            path.file_name().unwrap().to_string_lossy()
        );
        assert_eq!(defaults.target_kind, "molecule");
        assert!(!defaults.options.is_empty());
        assert_eq!(
            defaults.reader_options.as_ref().map(|options| (
                options.kind.as_str(),
                options.ignore_multiple_models,
                options.calculate_secondary_structure,
            )),
            Some(("pdb", true, true))
        );

        let error = match handle_model_request(
            &mut model,
            ModelRequest::OpenPathWithRendererSetup {
                path: path_text.clone(),
                target: crate::GuiFileRendererTarget::Molecule,
                reader_options: defaults
                    .reader_options
                    .clone()
                    .map(crate::GuiMoleculeReaderOptions::try_from)
                    .transpose()
                    .unwrap(),
                namd_coor_options: None,
                density_map_options: None,
                msms_options: None,
                renderer_setup: crate::GuiRendererSetup {
                    object_name: "opened.pdb".to_string(),
                    renderer_type: "invalid".to_string(),
                    renderer_name: "atoms".to_string(),
                    selection: None,
                    recenter: true,
                },
                width: 800,
                height: 600,
            },
        ) {
            Err(error) => error,
            Ok(_) => panic!("invalid renderer setup was accepted"),
        };
        assert!(error.contains("unsupported renderer setup type"));
        assert!(model.snapshot().objects.is_empty());

        let response = handle_model_request(
            &mut model,
            ModelRequest::OpenPathWithRendererSetup {
                path: path_text.clone(),
                target: crate::GuiFileRendererTarget::Molecule,
                reader_options: defaults
                    .reader_options
                    .clone()
                    .map(crate::GuiMoleculeReaderOptions::try_from)
                    .transpose()
                    .unwrap(),
                namd_coor_options: None,
                density_map_options: None,
                msms_options: None,
                renderer_setup: crate::GuiRendererSetup {
                    object_name: "opened.pdb".to_string(),
                    renderer_type: "ballstick".to_string(),
                    renderer_name: "atoms".to_string(),
                    selection: Some("name CA".to_string()),
                    recenter: false,
                },
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected molecule update");
        };
        assert_eq!(update.snapshot.objects[0].name, "opened.pdb");
        assert!(update.snapshot.objects[0]
            .renderers
            .iter()
            .any(|renderer| renderer.name == "atoms" && renderer.renderer_type == "ballstick"));
        assert!(update.snapshot.can_undo);
        assert!(!model.mesh_refit_pending);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn file_open_renderer_setup_loads_density_map_with_selected_renderer() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-renderer-setup-open-{}.map",
            std::process::id()
        ));
        fs::write(
            &path,
            "\n       1\n REMARKS generated by desktop test\n       2       0       1       1       0       0       1       0       0\n 2.00000E+00 1.00000E+00 1.00000E+00 9.00000E+01 9.00000E+01 9.00000E+01\nZYX\n0\n 1.00000E+00 2.00000E+00\n",
        )
        .unwrap();
        let path_text = path.to_string_lossy().into_owned();
        let mut model = model_without_gpu();
        model.camera.set_center(Vector4::new3(4.0, 5.0, 6.0));
        let setup_response = handle_model_request(
            &mut model,
            ModelRequest::GetFileRendererSetup {
                path: path_text.clone(),
            },
        )
        .unwrap();
        let ModelResponse::FileRendererSetup(Some(defaults)) = setup_response else {
            panic!("expected density-map renderer setup");
        };
        assert_eq!(defaults.target_kind, "densityMap");
        assert_eq!(defaults.renderer_type, "contour");
        assert!(!defaults.recenter);
        assert!(defaults.reader_options.is_none());
        assert!(defaults.ccp4_map_options.is_none());
        assert!(defaults.mtz_map_options.is_none());

        let response = handle_model_request(
            &mut model,
            ModelRequest::OpenPathWithRendererSetup {
                path: path_text,
                target: crate::GuiFileRendererTarget::DensityMap,
                reader_options: None,
                namd_coor_options: None,
                density_map_options: None,
                msms_options: None,
                renderer_setup: crate::GuiRendererSetup {
                    object_name: "density".to_string(),
                    renderer_type: "contour".to_string(),
                    renderer_name: "contour1".to_string(),
                    selection: None,
                    recenter: false,
                },
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected density-map update");
        };
        assert_eq!(update.snapshot.density_maps.len(), 1);
        assert_eq!(update.snapshot.density_maps[0].name, "density");
        assert_eq!(update.snapshot.density_maps[0].renderers.len(), 1);
        assert_eq!(
            update.snapshot.density_maps[0].renderers[0].name,
            "contour1"
        );
        assert_eq!(
            update.snapshot.density_maps[0].renderers[0]
                .specific
                .get("center"),
            Some(&Vector4::new3(4.0, 5.0, 6.0).to_string())
        );
        assert!(!model.mesh_refit_pending);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn file_open_renderer_setup_loads_msms_surface_with_vertex_options() {
        let dir =
            std::env::temp_dir().join(format!("cuemol-renderer-setup-msms-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let face_path = dir.join("surface.face");
        let vert_path = dir.join("surface.vert");
        fs::write(&face_path, "# MSMS\n# faces\n1\n1 2 3\n").unwrap();
        fs::write(
            &vert_path,
            "# MSMS\n# vertices\n3\n0 0 0 0 0 1\n1 0 0 0 0 1\n0 1 0 0 0 1\n",
        )
        .unwrap();
        let path_text = face_path.to_string_lossy().into_owned();
        let mut model = model_without_gpu();
        let response = handle_model_request(
            &mut model,
            ModelRequest::GetFileRendererSetup {
                path: path_text.clone(),
            },
        )
        .unwrap();
        let ModelResponse::FileRendererSetup(Some(defaults)) = response else {
            panic!("expected MSMS renderer setup");
        };
        assert_eq!(defaults.target_kind, "surface");
        assert_eq!(defaults.renderer_type, "molsurf");
        assert_eq!(
            defaults
                .msms_options
                .as_ref()
                .map(|options| options.vertex_path.as_str()),
            vert_path.to_str()
        );

        let response = handle_model_request(
            &mut model,
            ModelRequest::OpenPathWithRendererSetup {
                path: path_text,
                target: crate::GuiFileRendererTarget::Surface,
                reader_options: None,
                namd_coor_options: None,
                density_map_options: None,
                msms_options: defaults.msms_options.map(crate::GuiMsmsReaderOptions::from),
                renderer_setup: crate::GuiRendererSetup {
                    object_name: "surface.face".to_string(),
                    renderer_type: "molsurf".to_string(),
                    renderer_name: "surface1".to_string(),
                    selection: None,
                    recenter: true,
                },
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected surface update");
        };
        assert_eq!(update.snapshot.surfaces.len(), 1);
        assert_eq!(update.snapshot.surfaces[0].name, "surface.face");
        assert_eq!(update.snapshot.surfaces[0].renderers[0].name, "surface1");
        assert!(model.mesh_refit_pending);

        for path in [face_path, vert_path] {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn mtz_renderer_setup_dto_roundtrips_reader_columns_and_fft_options() {
        let options = crate::GuiMtzMapReaderOptions {
            columns: vec![
                crate::GuiMtzColumnOption {
                    name: "FWT".to_string(),
                    kind: 'F',
                },
                crate::GuiMtzColumnOption {
                    name: "PHWT".to_string(),
                    kind: 'P',
                },
            ],
            column_f: "FWT".to_string(),
            use_phase: true,
            column_phi: "PHWT".to_string(),
            use_weight: false,
            column_weight: String::new(),
            resolution: 2.0,
            min_resolution: 2.0,
            max_resolution: 10.0,
            grid_size: 0.25,
        };
        let mut model = model_without_gpu();
        let defaults = density_map_renderer_setup_defaults(
            &mut model,
            "sample.mtz",
            Some(crate::GuiDensityMapReaderOptions::Mtz(options.clone())),
        )
        .unwrap();

        assert_eq!(defaults.target_kind, "densityMap");
        assert!(defaults.ccp4_map_options.is_none());
        let roundtrip = defaults
            .mtz_map_options
            .map(crate::GuiMtzMapReaderOptions::from)
            .expect("MTZ options");
        assert_eq!(roundtrip, options);
    }

    #[test]
    fn namd_renderer_setup_dto_roundtrips_psf_path() {
        let options = crate::GuiNamdCoorReaderOptions {
            topology_path: "/tmp/system.psf".to_string(),
        };
        let mut model = model_without_gpu();
        let defaults =
            renderer_setup_defaults(&mut model, "system.coor", None, Some(options.clone()))
                .unwrap();

        assert_eq!(defaults.target_kind, "molecule");
        assert!(defaults.reader_options.is_none());
        let roundtrip = defaults
            .namd_coor_options
            .map(crate::GuiNamdCoorReaderOptions::from)
            .expect("NAMD options");
        assert_eq!(roundtrip, options);
    }

    #[test]
    fn selection_refresh_reuses_the_cached_molecule_hierarchy() {
        let mut model = model_without_gpu();
        let before = model.snapshot();
        handle_model_request(
            &mut model,
            ModelRequest::SetSelection("chain A".to_string()),
        )
        .unwrap();
        let after = model.snapshot();
        assert_eq!(after.structure_key, before.structure_key);
        assert_ne!(after.sequence_key, before.sequence_key);
        let details =
            model.molecule_details(&after.structure_key, None, &before.sequence_key, true, None);
        assert!(details.structure.is_none());
        assert!(details.sequence.is_some());
        assert_eq!(details.sequence_key, after.sequence_key);
    }

    #[test]
    fn help_dialog_info_uses_registered_plugins_and_live_config() {
        let mut model = model_without_gpu();
        let response = handle_model_request(&mut model, ModelRequest::GetApplicationInfo).unwrap();
        let ModelResponse::ApplicationInfo(info) = response else {
            panic!("expected application information");
        };
        assert_eq!(info.product_name, "CueMol2");
        assert!(!info.plugins.is_empty());
        assert!(info
            .plugins
            .iter()
            .any(|plugin| plugin.category == "Object reader"));

        let response = handle_model_request(
            &mut model,
            ModelRequest::SetConfigValue {
                key: "ui:test_value".to_string(),
                value: "enabled".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::ApplicationInfo(info) = response else {
            panic!("expected updated application information");
        };
        assert!(info
            .config
            .iter()
            .any(|entry| entry.key == "ui:test_value" && entry.value == "enabled"));
        assert_eq!(info.status, "Config updated: ui:test_value");
    }

    #[test]
    fn options_roundtrip_through_the_persistent_config_and_runtime_services() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("cuemol-options-{}-{nonce}.tsv", std::process::id()));
        let mut model = AppModel::new_with_config(Vec::new(), Some(path.clone()));
        let mut preferences = model.preferences();
        preferences.atom_label_font = "monospace".to_string();
        preferences.atom_label_size = 18.0;
        preferences.atom_label_color = "#33aa55".to_string();
        preferences.atom_label_italic = true;
        preferences.ui_language = "ja".to_string();
        preferences.mouse_preset = "PymolViewInConf".to_string();
        preferences.trackball_radius = 1.25;
        preferences.hit_precision = 6.0;
        preferences.momentum_scroll = true;
        preferences.shortcuts[0].key = "ArrowUp".to_string();
        preferences.shortcuts[1].key = "ArrowDown".to_string();
        model.set_preferences(preferences).unwrap();
        assert_eq!(model.desktop.view_input_config().tbrad(), 1.25);
        assert_eq!(model.desktop.view_input_config().hitprec(), 6.0);
        assert!(path.is_file());

        let restored = AppModel::new_with_config(Vec::new(), Some(path.clone())).preferences();
        assert_eq!(restored.atom_label_font, "monospace");
        assert_eq!(restored.atom_label_size, 18.0);
        assert_eq!(restored.atom_label_color, "#33aa55");
        assert!(restored.atom_label_italic);
        assert_eq!(restored.ui_language, "ja");
        assert_eq!(restored.mouse_preset, "PymolViewInConf");
        assert_eq!(restored.trackball_radius, 1.25);
        assert_eq!(restored.hit_precision, 6.0);
        assert!(restored.momentum_scroll);
        assert_eq!(restored.shortcuts[0].key, "ArrowUp");
        assert_eq!(restored.shortcuts[1].key, "ArrowDown");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn scene_properties_are_undoable_and_persist_as_qsc_attributes() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetSceneProperties(ScenePropertiesInput {
                scene_uid: scene_uid.get(),
                background: "#123456".to_string(),
                ao_enabled: true,
                ao_radius: 6.5,
                ao_intensity: 1.7,
                ao_slices: 12,
                ao_steps: 5,
                ao_half_res: true,
                aa_method: "smaa".to_string(),
                aa_jitter_level: 2,
                onload: "print('ready');".to_string(),
                use_color_proof: true,
                icc_filename: "GenericCMYK.icm".to_string(),
                icc_intent: "relative_colorimetric".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected scene property snapshot");
        };
        let scene = &snapshot.scenes[0];
        assert_eq!(scene.background, "#123456");
        assert!(scene.ao_enabled);
        assert_eq!(scene.ao_radius, 6.5);
        assert_eq!(scene.aa_method, "smaa");
        assert_eq!(scene.aa_jitter_level, 2);
        assert!(scene.use_color_proof);
        assert_eq!(model.background, "#123456");
        assert!(model.color_proofing);
        assert_eq!(snapshot.undo_descriptions, ["Scene properties updated"]);

        let qsc = model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .write_qsc_string();
        assert!(qsc.contains("bgcolor=\"#123456\""));
        assert!(qsc.contains("aoEnabled=\"true\""));
        assert!(qsc.contains("aa_method=\"smaa\""));
        assert!(qsc.contains("icc_intent=\"relative_colorimetric\""));

        model.undo(0).unwrap();
        let scene = model.desktop.scene_manager().scene(scene_uid).unwrap();
        assert_eq!(scene.scene_attribute("bgcolor"), None);
        assert_eq!(model.background, "black");
        assert!(!model.color_proofing);
    }

    #[test]
    fn scene_post_processing_applies_ao_and_edge_aa() {
        let mut rgba = vec![200_u8; 5 * 5 * 4];
        let mut depth = vec![0_u8; 5 * 5 * 4];
        for channel in 0..4 {
            depth[(2 * 5 + 2) * 4 + channel] = 100;
            depth[(2 * 5 + 3) * 4 + channel] = 220;
        }
        apply_screen_space_ao(&mut rgba, &depth, 5, 5, 1, 2.2, 4, 1);
        assert!(rgba[(2 * 5 + 2) * 4] < 200);

        let mut edge = vec![0_u8; 5 * 5 * 4];
        for y in 0..5 {
            for x in 2..5 {
                let index = (y * 5 + x) * 4;
                edge[index..index + 4].fill(255);
            }
        }
        apply_fast_post_aa(&mut edge, 5, 5, false);
        assert!(edge[(2 * 5 + 1) * 4] > 0);
        assert!(edge[(2 * 5 + 2) * 4] < 255);
        assert_eq!(background_rgba("#123456").unwrap()[0], 18.0 / 255.0);
    }

    #[test]
    fn workspace_object_and_renderer_properties_are_undoable_and_persisted() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::GetRendererStyleOptions {
                scene_uid: scene_uid.get(),
                renderer_type: "simple".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::RendererStyleOptions(style_options) = response else {
            panic!("expected renderer style options");
        };
        assert!(style_options
            .iter()
            .any(|style| style.name == "DefaultSimple"));

        model.prepare_mesh_if_needed().unwrap();
        assert!(model.mesh.is_some());
        handle_model_request(
            &mut model,
            ModelRequest::SetObjectProperties(ObjectPropertiesInput {
                object_uid: object_uid.get(),
                name: "renamed molecule".to_string(),
                visible: false,
                locked: true,
            }),
        )
        .unwrap();
        let object = model.desktop.scene_manager().object(object_uid).unwrap();
        assert_eq!(object.name(), "renamed molecule");
        assert!(!object.is_visible());
        assert!(object.is_ui_locked());
        assert!(model.mesh.is_none());
        model.prepare_mesh_if_needed().unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::SetRendererProperties(RendererPropertiesInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                name: "carbon atoms".to_string(),
                selection: "elem C".to_string(),
                visible: true,
                locked: true,
                material: "matte".to_string(),
                alpha: 0.4,
                edge_type: "silhouette".to_string(),
                edge_width: 0.2,
                edge_color: "#123456".to_string(),
                line_width: 2.4,
                detail: 3,
                bond_width: 0.2,
                sphere_radius: 0.3,
                show_ring: false,
                ring_thickness: 0.2,
                ring_color: "#ffffbf".to_string(),
                specific: BTreeMap::from([(
                    "style".to_string(),
                    "ThickSimple,DefaultCPKColoring".to_string(),
                )]),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let renderer = &snapshot.objects[0].renderers[0];
        assert_eq!(renderer.name, "carbon atoms");
        assert_eq!(renderer.selection, "elem C");
        assert!(renderer.locked);
        assert_eq!(renderer.material, "matte");
        assert_eq!(renderer.alpha, 0.4);
        assert_eq!(renderer.edge_type, "silhouette");
        assert_eq!(renderer.edge_width, 0.2);
        assert_eq!(renderer.edge_color, "#123456");
        assert_eq!(renderer.line_width, 2.4);
        assert_eq!(renderer.style, "ThickSimple,DefaultCPKColoring");
        assert_eq!(snapshot.undo_descriptions.len(), 2);
        assert!(model.mesh.is_none());

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(renderer.name(), "simple 1");
        assert_eq!(renderer.style(), None);
        assert_eq!(renderer.qsc_attr("alpha"), None);
        assert_eq!(renderer.qsc_attr("width"), None);
    }

    #[test]
    fn spline_properties_are_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let renderer_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("spline", "spline 1");
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetRendererProperties(RendererPropertiesInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                name: "smooth backbone".to_string(),
                selection: "*".to_string(),
                visible: true,
                locked: false,
                material: String::new(),
                alpha: 1.0,
                edge_type: "none".to_string(),
                edge_width: 0.01,
                edge_color: "#000000".to_string(),
                line_width: 2.6,
                detail: 3,
                bond_width: 0.2,
                sphere_radius: 0.3,
                show_ring: false,
                ring_thickness: 0.2,
                ring_color: "#ffffbf".to_string(),
                specific: BTreeMap::from([
                    ("axialdetail".to_string(), "12".to_string()),
                    ("smooth".to_string(), "0.25".to_string()),
                    ("smoothcolor".to_string(), "false".to_string()),
                    ("pivotatom".to_string(), "CA".to_string()),
                ]),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let dto = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid.get())
            .unwrap();
        assert_eq!(dto.line_width, 2.6);
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        for (key, value) in [
            ("line_width", "2.6"),
            ("axialdetail", "12"),
            ("smooth", "0.25"),
            ("smoothcolor", "false"),
            ("pivotatom", "CA"),
        ] {
            assert_eq!(renderer.qsc_attr(key), Some(value));
        }

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        for key in [
            "line_width",
            "axialdetail",
            "smooth",
            "smoothcolor",
            "pivotatom",
        ] {
            assert_eq!(renderer.qsc_attr(key), None, "{key} restored");
        }
    }

    #[test]
    fn selection_properties_are_readonly_for_sel_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let renderer_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("*selection", "");
        let mut properties = default_renderer_properties_input(object_uid, renderer_uid, "");
        properties.selection = "bad selector (".to_string();
        properties.specific = BTreeMap::from([
            ("mode".to_string(), "point".to_string()),
            ("linew".to_string(), "5".to_string()),
            ("dispx".to_string(), "6".to_string()),
            ("dispy".to_string(), "-4".to_string()),
            ("color".to_string(), "rgba(18,52,86,0.25)".to_string()),
        ]);
        let response =
            handle_model_request(&mut model, ModelRequest::SetRendererProperties(properties))
                .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let dto = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid.get())
            .unwrap();
        for (key, value) in [
            ("mode", "point"),
            ("linew", "5"),
            ("dispx", "6"),
            ("dispy", "-4"),
            ("color", "rgba(18,52,86,0.25)"),
        ] {
            assert_eq!(dto.specific[key], value, "{key}");
        }
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("sel"), None);
        assert_eq!(renderer.qsc_attr("mode"), Some("point"));

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        for key in ["sel", "mode", "linew", "dispx", "dispy", "color"] {
            assert_eq!(renderer.qsc_attr(key), None, "{key} restored");
        }
    }

    #[test]
    fn name_label_properties_are_selectionless_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let renderer_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("*namelabel", "labels");

        let initial = model.snapshot();
        let label = initial.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid.get())
            .unwrap();
        assert!(label.selection.is_empty());
        assert!(!label.supports_coloring);
        assert_eq!(label.specific["maxlabel"], "50");
        assert_eq!(label.specific["font_name"], "sans-serif");

        let mut properties = default_renderer_properties_input(object_uid, renderer_uid, "labels");
        properties.selection = "invalid selector (".to_string();
        properties.specific = BTreeMap::from([
            ("maxlabel".to_string(), "25".to_string()),
            ("dispx".to_string(), "6".to_string()),
            ("dispy".to_string(), "-4".to_string()),
            ("color".to_string(), "#123456".to_string()),
            ("font_size".to_string(), "18".to_string()),
            ("font_name".to_string(), "monospace".to_string()),
            ("font_style".to_string(), "italic".to_string()),
            ("font_weight".to_string(), "bold".to_string()),
        ]);
        let response =
            handle_model_request(&mut model, ModelRequest::SetRendererProperties(properties))
                .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let label = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid.get())
            .unwrap();
        for (key, expected) in [
            ("maxlabel", "25"),
            ("dispx", "6"),
            ("dispy", "-4"),
            ("color", "#123456"),
            ("font_size", "18"),
            ("font_name", "monospace"),
            ("font_style", "italic"),
            ("font_weight", "bold"),
        ] {
            assert_eq!(label.specific[key], expected, "{key}");
        }
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("sel"), None);

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        for key in [
            "sel",
            "maxlabel",
            "dispx",
            "dispy",
            "color",
            "font_size",
            "font_name",
            "font_style",
            "font_weight",
        ] {
            assert_eq!(renderer.qsc_attr(key), None, "{key} restored");
        }
    }

    #[test]
    fn ballstick_properties_are_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let renderer_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("ballstick", "ballstick 1");
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetRendererProperties(RendererPropertiesInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                name: "rings".to_string(),
                selection: "*".to_string(),
                visible: true,
                locked: false,
                material: String::new(),
                alpha: 1.0,
                edge_type: "none".to_string(),
                edge_width: 0.01,
                edge_color: "#000000".to_string(),
                line_width: 1.2,
                detail: 8,
                bond_width: 0.4,
                sphere_radius: 0.5,
                show_ring: true,
                ring_thickness: 0.25,
                ring_color: "#abcdef".to_string(),
                specific: BTreeMap::new(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let renderer = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid.get())
            .unwrap();
        assert_eq!(renderer.detail, 8);
        assert_eq!(renderer.bond_width, 0.4);
        assert_eq!(renderer.sphere_radius, 0.5);
        assert!(renderer.show_ring);
        assert_eq!(renderer.ring_thickness, 0.25);
        assert_eq!(renderer.ring_color, "#abcdef");

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        for key in ["detail", "bondw", "sphr", "ring", "thickness", "ringcolor"] {
            assert_eq!(renderer.qsc_attr(key), None, "{key} restored");
        }
    }

    #[test]
    fn anisou_properties_are_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let renderer_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("anisou", "anisou 1");
        let specific = BTreeMap::from([
            ("drawdisc".to_string(), "false".to_string()),
            ("discscale".to_string(), "1.4".to_string()),
            ("discthick".to_string(), "0.2".to_string()),
            ("maxverts".to_string(), "250000".to_string()),
        ]);
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetRendererProperties(RendererPropertiesInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                name: "thermal ellipsoids".to_string(),
                selection: "*".to_string(),
                visible: true,
                locked: false,
                material: String::new(),
                alpha: 1.0,
                edge_type: "none".to_string(),
                edge_width: 0.01,
                edge_color: "#000000".to_string(),
                line_width: 1.2,
                detail: 6,
                bond_width: 0.15,
                sphere_radius: 0.45,
                show_ring: false,
                ring_thickness: 0.2,
                ring_color: "#ffffbf".to_string(),
                specific,
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let renderer = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid.get())
            .unwrap();
        assert_eq!(renderer.detail, 6);
        assert_eq!(renderer.sphere_radius, 0.45);
        assert_eq!(renderer.specific["drawdisc"], "false");
        assert_eq!(renderer.specific["discscale"], "1.4");
        assert_eq!(renderer.specific["discthick"], "0.2");
        assert_eq!(renderer.specific["maxverts"], "250000");

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        for key in [
            "detail",
            "bondw",
            "sphr",
            "drawdisc",
            "discscale",
            "discthick",
            "maxverts",
        ] {
            assert_eq!(renderer.qsc_attr(key), None, "{key} restored");
        }
    }

    #[test]
    fn atom_intr_properties_are_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let renderer_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("atomintr", "measure");
        let specific = BTreeMap::from([
            ("mode".to_string(), "fancy".to_string()),
            ("showlabel".to_string(), "true".to_string()),
            ("detail".to_string(), "8".to_string()),
            ("stipple0".to_string(), "0.25".to_string()),
            ("stipple1".to_string(), "0.4".to_string()),
            ("stipple2".to_string(), "-1".to_string()),
            ("stipple3".to_string(), "-1".to_string()),
            ("stipple4".to_string(), "-1".to_string()),
            ("stipple5".to_string(), "-1".to_string()),
            ("width".to_string(), "0.25".to_string()),
            ("color".to_string(), "#12abef".to_string()),
            ("captype_start".to_string(), "arrow".to_string()),
            ("captype_end".to_string(), "flat".to_string()),
        ]);
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetRendererProperties(RendererPropertiesInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                name: "measurement".to_string(),
                selection: "*".to_string(),
                visible: true,
                locked: false,
                material: String::new(),
                alpha: 1.0,
                edge_type: "none".to_string(),
                edge_width: 0.01,
                edge_color: "#000000".to_string(),
                line_width: 1.2,
                detail: 3,
                bond_width: 0.2,
                sphere_radius: 0.3,
                show_ring: false,
                ring_thickness: 0.2,
                ring_color: "#ffffbf".to_string(),
                specific,
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let renderer = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid.get())
            .unwrap();
        for (key, expected) in [
            ("mode", "fancy"),
            ("showlabel", "true"),
            ("detail", "8"),
            ("stipple0", "0.25"),
            ("stipple1", "0.4"),
            ("width", "0.25"),
            ("color", "#12abef"),
            ("captype_start", "arrow"),
            ("captype_end", "flat"),
        ] {
            assert_eq!(renderer.specific[key], expected, "{key}");
        }

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        for key in [
            "mode",
            "showlabel",
            "detail",
            "stipple0",
            "stipple1",
            "width",
            "color",
            "captype_start",
            "captype_end",
        ] {
            assert_eq!(renderer.qsc_attr(key), None, "{key} restored");
        }
    }

    #[test]
    fn atom_intr_definition_list_and_removal_are_undoable() {
        let (mut model, object_uid, _) = model_with_renderer();
        let lines = vec![
            cuemol_molstr::AtomIntrLine::from_positions(
                cuemol_molstr::AtomIntrKind::Distance,
                vec![Vector4::new3(0.0, 0.0, 0.0), Vector4::new3(1.0, 0.0, 0.0)],
            ),
            cuemol_molstr::AtomIntrLine::from_positions(
                cuemol_molstr::AtomIntrKind::Angle,
                vec![
                    Vector4::new3(0.0, 0.0, 0.0),
                    Vector4::new3(1.0, 0.0, 0.0),
                    Vector4::new3(1.0, 1.0, 0.0),
                ],
            ),
        ];
        let renderer_uid = model
            .desktop
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_atom_intr_renderer_defs("measure", &[])
            .unwrap();
        let renderer = model
            .desktop
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap();
        for line in lines {
            renderer.append_atom_intr_line(line).unwrap();
        }

        let response = handle_model_request(
            &mut model,
            ModelRequest::GetAtomIntrDefs {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
            },
        )
        .unwrap();
        let ModelResponse::AtomIntrDefs(entries) = response else {
            panic!("expected atom interaction definitions");
        };
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].mode, 1);
        assert_eq!(entries[1].mode, 2);

        handle_model_request(
            &mut model,
            ModelRequest::RemoveAtomIntrDefs {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                ids: vec![0],
            },
        )
        .unwrap();
        assert_eq!(
            model
                .desktop
                .atom_intr_def_summaries(object_uid, renderer_uid)
                .unwrap()
                .into_iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            vec![1]
        );
        assert!(model
            .modified_scenes
            .contains(&model.desktop.active_scene().unwrap()));

        model.undo(0).unwrap();
        assert_eq!(
            model
                .desktop
                .atom_intr_def_summaries(object_uid, renderer_uid)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn simple_atom_intr_lines_are_projected_into_the_viewport() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let fixture = root.join("tests/test_data/number_chain_atomintr1.qsc");
        if !fixture.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.load_qsc_file(&fixture).unwrap();
        model.rebuild_mesh(true).unwrap();

        let viewport = model.viewport_metadata(800, 600);
        let interaction_lines = viewport
            .lines
            .iter()
            .filter(|line| line.width == 1.0 && line.stipple)
            .collect::<Vec<_>>();
        assert_eq!(interaction_lines.len(), 2);
        assert!(interaction_lines.iter().all(|line| {
            line.x1.is_finite() && line.y1.is_finite() && line.x2.is_finite() && line.y2.is_finite()
        }));
    }

    #[test]
    fn viewport_projects_selection_point_with_pixel_displacement() {
        let mut model = model_without_gpu();
        let position = Vector4::new3(0.0, 0.0, 0.0);
        model.screen_lines = vec![RenderBond::new(position, position, 0.03, None)
            .with_screen_style(5.0, false)
            .with_screen_offset(6.0, -4.0)
            .as_screen_point()];
        let mvp = camera_mvp(&model.camera, 4.0 / 3.0);
        let projected = project_viewport_point(mvp, position, 800, 600).unwrap();
        let viewport = model.viewport_metadata(800, 600);
        assert_eq!(viewport.lines.len(), 1);
        let point = &viewport.lines[0];
        assert!(point.point);
        assert_eq!(point.width, 5.0);
        assert!((point.x1 - projected.0 - 6.0).abs() < 1.0e-5);
        assert!((point.y1 - projected.1 + 4.0).abs() < 1.0e-5);
        assert_eq!((point.x1, point.y1), (point.x2, point.y2));
    }

    #[test]
    fn add_renderer_accepts_the_reference_molecular_catalog() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let object_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("renderer target", cuemol_molstr::MolCoord::new());

        for renderer_type in MOLECULE_RENDERER_TYPES {
            handle_model_request(
                &mut model,
                ModelRequest::AddRenderer {
                    object_uid: object_uid.get(),
                    renderer_type: (*renderer_type).to_string(),
                    group_name: None,
                },
            )
            .unwrap();
        }

        let object = model.desktop.scene_manager().object(object_uid).unwrap();
        for renderer_type in MOLECULE_RENDERER_TYPES {
            let renderer = object
                .renderers()
                .iter()
                .find(|renderer| renderer.renderer_type() == *renderer_type)
                .unwrap();
            assert_eq!(renderer.name(), format!("{renderer_type}1"));
        }
        assert_eq!(
            object
                .renderers()
                .iter()
                .filter(|renderer| renderer.renderer_type() == "*selection")
                .count(),
            1
        );
        assert!(handle_model_request(
            &mut model,
            ModelRequest::AddRenderer {
                object_uid: object_uid.get(),
                renderer_type: "molsurf".to_string(),
                group_name: None,
            },
        )
        .is_err());
    }

    #[test]
    fn renderer_group_requests_sync_members_and_are_undoable() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let object_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("group target", cuemol_molstr::MolCoord::new());
        let (group_uid, child_uid, free_uid) = {
            let object = model
                .desktop
                .scene_manager_mut()
                .object_mut(object_uid)
                .unwrap();
            let group_uid = object.add_renderer("*group", "representations");
            let child_uid = object.add_renderer("simple", "lines");
            object
                .renderer_mut(child_uid)
                .unwrap()
                .set_qsc_attr("group", "representations");
            let free_uid = object.add_renderer("cpk", "spacefill");
            (group_uid, child_uid, free_uid)
        };

        handle_model_request(
            &mut model,
            ModelRequest::SetRendererVisible {
                object_uid: object_uid.get(),
                renderer_uid: group_uid.get(),
                visible: false,
            },
        )
        .unwrap();
        let object = model.desktop.scene_manager().object(object_uid).unwrap();
        assert!(!object.renderer(group_uid).unwrap().is_visible());
        assert!(!object.renderer(child_uid).unwrap().is_visible());
        assert!(object.renderer(free_uid).unwrap().is_visible());
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(child_uid)
            .unwrap()
            .is_visible());

        let mut properties =
            default_renderer_properties_input(object_uid, group_uid, "main representations");
        properties.selection = "invalid selector (".to_string();
        handle_model_request(&mut model, ModelRequest::SetRendererProperties(properties)).unwrap();
        assert_eq!(
            model
                .desktop
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(child_uid)
                .unwrap()
                .group(),
            Some("main representations")
        );
        model.prepare_mesh_if_needed().unwrap();
        assert!(model.mesh.is_some());

        let response = handle_model_request(
            &mut model,
            ModelRequest::MoveWorkspaceRenderer(WorkspaceMoveRendererInput {
                object_uid: object_uid.get(),
                renderer_uid: free_uid.get(),
                target_uid: group_uid.get(),
                position: "inside".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(
            snapshot.objects[0]
                .renderers
                .iter()
                .find(|renderer| renderer.uid == free_uid.get())
                .unwrap()
                .group
                .as_deref(),
            Some("main representations")
        );
        assert_eq!(snapshot.undo_descriptions[0], "Move renderer");
        assert!(model.mesh.is_some());
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(
            model
                .desktop
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(free_uid)
                .unwrap()
                .group(),
            None
        );

        handle_model_request(
            &mut model,
            ModelRequest::AddRenderer {
                object_uid: object_uid.get(),
                renderer_type: "tube".to_string(),
                group_name: Some("main representations".to_string()),
            },
        )
        .unwrap();
        let grouped_count = model.snapshot().objects[0]
            .renderers
            .iter()
            .filter(|renderer| renderer.group.as_deref() == Some("main representations"))
            .count();
        assert_eq!(grouped_count, 2);

        handle_model_request(
            &mut model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "rendererGroup".to_string(),
                object_uid: object_uid.get(),
                renderer_uid: Some(group_uid.get()),
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        let object = model.desktop.scene_manager().object(object_uid).unwrap();
        assert!(object.renderer(group_uid).is_none());
        assert!(object.renderer(child_uid).is_none());
        assert!(object.renderer(free_uid).is_some());
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(group_uid)
            .is_some());
    }

    #[test]
    fn workspace_object_move_is_cross_kind_undoable_and_keeps_mesh() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let map_uid =
            model
                .desktop
                .add_density_map(scene_uid, "map", cuemol_xtal::DensityMap::new());
        model.prepare_mesh_if_needed().unwrap();
        assert!(model.mesh.is_some());

        let response = handle_model_request(
            &mut model,
            ModelRequest::MoveWorkspaceObject(WorkspaceMoveObjectInput {
                scene_uid: scene_uid.get(),
                source_kind: "densityMap".to_string(),
                source_uid: map_uid.get(),
                target_kind: "object".to_string(),
                target_uid: object_uid.get(),
                position: "before".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.density_maps[0].ui_order, 1);
        assert_eq!(snapshot.objects[0].ui_order, 2);
        assert_eq!(snapshot.undo_descriptions[0], "Move object");
        assert!(model.mesh.is_some());

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        let snapshot = model.snapshot();
        assert!(snapshot.objects[0].ui_order < snapshot.density_maps[0].ui_order);
    }

    #[test]
    fn workspace_renderer_selection_preset_is_undoable() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceRendererSelection(WorkspaceRendererSelectionInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                selection: "protein".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.objects[0].renderers[0].selection, "protein");
        assert_eq!(snapshot.undo_descriptions[0], "Change renderer selection");

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().objects[0].renderers[0].selection, "*");
    }

    #[test]
    fn workspace_renderer_type_change_is_undoable_and_invalidates_mesh() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        model.prepare_mesh_if_needed().unwrap();
        assert!(model.mesh.is_some());
        let response = handle_model_request(
            &mut model,
            ModelRequest::ChangeWorkspaceRendererType(WorkspaceRendererTypeInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                renderer_type: "cartoon".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let replacement = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.renderer_type == "cartoon")
            .unwrap();
        assert_ne!(replacement.uid, renderer_uid.get());
        assert_eq!(replacement.name, "simple 1");
        assert_eq!(
            snapshot.undo_descriptions[0],
            "Change renderer type to cartoon"
        );
        assert!(model.mesh.is_none());

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .is_some());

        let scene_uid = model.desktop.active_scene().unwrap();
        let map_uid =
            model
                .desktop
                .add_density_map(scene_uid, "map", cuemol_xtal::DensityMap::new());
        let map_renderer_uid = model
            .desktop
            .add_density_renderer(map_uid, "contour")
            .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::ChangeWorkspaceRendererType(WorkspaceRendererTypeInput {
                object_uid: map_uid.get(),
                renderer_uid: map_renderer_uid.get(),
                renderer_type: "isosurf".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_ne!(
            snapshot.density_maps[0].renderers[0].uid,
            map_renderer_uid.get()
        );
        assert_eq!(
            snapshot.density_maps[0].renderers[0].renderer_type,
            "isosurf"
        );

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(
            model.snapshot().density_maps[0].renderers[0].renderer_type,
            "contour"
        );
        assert_eq!(
            model.snapshot().density_maps[0].renderers[0].uid,
            map_renderer_uid.get()
        );
    }

    #[test]
    fn workspace_renderer_setup_creates_named_selected_renderer_and_rolls_back_errors() {
        let (mut model, object_uid, _) = model_with_renderer();
        let initial_count = model.snapshot().objects[0].renderers.len();
        let invalid = handle_model_request(
            &mut model,
            ModelRequest::CreateWorkspaceRenderer(WorkspaceNewRendererInput {
                kind: "renderer".to_string(),
                object_uid: object_uid.get(),
                renderer_type: "cpk".to_string(),
                renderer_name: "invalid selection".to_string(),
                selection: Some("bad selector (".to_string()),
                group_name: None,
                density_centering: None,
                recenter: Some(true),
            }),
        );
        assert!(invalid.is_err());
        assert_eq!(model.snapshot().objects[0].renderers.len(), initial_count);
        assert!(model.undo_stack.is_empty());

        let response = handle_model_request(
            &mut model,
            ModelRequest::CreateWorkspaceRenderer(WorkspaceNewRendererInput {
                kind: "renderer".to_string(),
                object_uid: object_uid.get(),
                renderer_type: "cpk".to_string(),
                renderer_name: "atoms".to_string(),
                selection: Some("protein".to_string()),
                group_name: None,
                density_centering: None,
                recenter: Some(false),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let renderer = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.name == "atoms")
            .unwrap();
        assert_eq!(renderer.renderer_type, "cpk");
        assert_eq!(renderer.selection, "protein");
        assert_eq!(snapshot.undo_descriptions[0], "Added cpk renderer: atoms");

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().objects[0].renderers.len(), initial_count);

        let scene_uid = model.desktop.active_scene().unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::GetWorkspaceRendererSetupOptions {
                scene_uid: scene_uid.get(),
                kind: "renderer".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::RendererStyleOptions(options) = response else {
            panic!("expected renderer setup options");
        };
        assert!(options
            .iter()
            .any(|option| option.name == "Default1RendPreset"));

        let response = handle_model_request(
            &mut model,
            ModelRequest::CreateWorkspaceRenderer(WorkspaceNewRendererInput {
                kind: "renderer".to_string(),
                object_uid: object_uid.get(),
                renderer_type: "Default1RendPreset".to_string(),
                renderer_name: "preset".to_string(),
                selection: None,
                group_name: None,
                density_centering: None,
                recenter: Some(true),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert!(snapshot.objects[0]
            .renderers
            .iter()
            .any(|renderer| { renderer.renderer_type == "*group" && renderer.name == "preset" }));
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().objects[0].renderers.len(), initial_count);

        let map_uid =
            model
                .desktop
                .add_density_map(scene_uid, "map", cuemol_xtal::DensityMap::new());
        model
            .camera
            .set_center(cuemol_core::Vector4::new3(3.0, 4.0, 5.0));
        handle_model_request(
            &mut model,
            ModelRequest::CreateWorkspaceRenderer(WorkspaceNewRendererInput {
                kind: "densityRenderer".to_string(),
                object_uid: map_uid.get(),
                renderer_type: "isosurf".to_string(),
                renderer_name: "density surface".to_string(),
                selection: None,
                group_name: None,
                density_centering: Some("map".to_string()),
                recenter: None,
            }),
        )
        .unwrap();
        let density_renderer = &model.snapshot().density_maps[0].renderers[0];
        assert_eq!(density_renderer.name, "density surface");
        assert_eq!(density_renderer.renderer_type, "isosurf");
        assert_eq!(density_renderer.specific["center"], "(3,4,5)");
    }

    #[test]
    fn workspace_renderer_style_and_coloring_requests_are_undoable() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        model.prepare_mesh_if_needed().unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::ApplyWorkspaceRendererStyle(WorkspaceRendererStyleInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                style_name: "DefaultSimple".to_string(),
                component: "shape".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.objects[0].renderers[0].style, "DefaultSimple");
        assert_eq!(snapshot.undo_descriptions[0], "Change style");
        assert!(model.mesh.is_none());

        let response = handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceRendererStyles(WorkspaceRendererStylesInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                style_names: vec![
                    "DefaultCPKColoring".to_string(),
                    " ThickSimple ".to_string(),
                ],
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(
            snapshot.objects[0].renderers[0].style,
            "DefaultCPKColoring,ThickSimple"
        );
        assert_eq!(snapshot.undo_descriptions[0], "Change style");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(
            model
                .desktop
                .renderer_summaries(object_uid)
                .first()
                .and_then(|renderer| renderer.style.as_deref()),
            Some("DefaultSimple")
        );

        let response = handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceRendererColoring(WorkspaceRendererColoringInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                preset: "rainbow".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.undo_descriptions[0], "Change coloring");
        assert!(model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap()
            .qsc_children()
            .iter()
            .any(|child| child.type_name() == "RainbowColoring"));

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(!model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap()
            .qsc_children()
            .iter()
            .any(|child| child.type_name() == "RainbowColoring"));

        handle_model_request(
            &mut model,
            ModelRequest::ApplyWorkspaceRendererStyle(WorkspaceRendererStyleInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                style_name: "WoodyHSCPaint".to_string(),
                component: "coloring".to_string(),
            }),
        )
        .unwrap();
        model
            .desktop
            .set_molecule_selection_text(object_uid, "protein")
            .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::InsertWorkspaceRendererPaint(WorkspaceRendererPaintInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                color: "hsb(120, 1.0, 1.0)".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.undo_descriptions[0], "Insert paint entry");
        assert_eq!(
            snapshot.objects[0].renderers[0].paint_entries[0].selection,
            "protein"
        );
        assert_eq!(
            snapshot.objects[0].renderers[0].paint_entries[0].color,
            "hsb(120, 1.0, 1.0)"
        );

        let undo_count = model.undo_stack.len();
        handle_model_request(
            &mut model,
            ModelRequest::InsertWorkspaceObjectPaint(WorkspaceObjectPaintInput {
                object_uid: object_uid.get(),
                color: "red".to_string(),
            }),
        )
        .unwrap();
        assert_eq!(model.undo_stack.len(), undo_count);

        model
            .desktop
            .set_object_coloring_details(
                object_uid,
                Some("PaintColoring".to_string()),
                BTreeMap::new(),
                vec![("*".to_string(), "white".to_string())],
                None,
            )
            .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::InsertWorkspaceObjectPaint(WorkspaceObjectPaintInput {
                object_uid: object_uid.get(),
                color: "hsb(240, 1.0, 1.0)".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.undo_descriptions[0], "Insert paint entry");
        assert_eq!(snapshot.objects[0].paint_entries[0].selection, "protein");
        assert_eq!(
            snapshot.objects[0].paint_entries[0].color,
            "hsb(240, 1.0, 1.0)"
        );
    }

    #[test]
    fn workspace_renderer_copy_uses_small_response_and_paste_is_undoable() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let response = handle_model_request(
            &mut model,
            ModelRequest::CopyWorkspaceRenderer(WorkspaceRendererCopyInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
            }),
        )
        .unwrap();
        let ModelResponse::Message(message) = response else {
            panic!("expected copy status message");
        };
        assert_eq!(message, "Copied 1 renderer(s)");
        assert!(model.undo_stack.is_empty());

        model
            .desktop
            .destroy_renderer_in_object(object_uid, renderer_uid)
            .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceRenderers(WorkspaceRendererPasteInput {
                object_uid: object_uid.get(),
                destination_group_uid: None,
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected paste snapshot");
        };
        assert_eq!(snapshot.objects[0].renderers.len(), 1);
        assert_eq!(snapshot.objects[0].renderers[0].renderer_type, "simple");
        assert_eq!(snapshot.undo_descriptions[0], "Pasted 1 renderer(s)");
        assert!(model.mesh.is_none());

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().objects[0].renderers.is_empty());
        assert!(model.workspace_renderer_clipboard.is_some());
    }

    #[test]
    fn workspace_multiple_commands_use_one_undo_transaction() {
        let (mut model, object_uid, first_uid) = model_with_renderer();
        let second_uid = model
            .desktop
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("cpk", "atoms");

        let response = handle_model_request(
            &mut model,
            ModelRequest::CopyWorkspaceRenderers(vec![
                WorkspaceRendererCopyInput {
                    object_uid: object_uid.get(),
                    renderer_uid: first_uid.get(),
                },
                WorkspaceRendererCopyInput {
                    object_uid: object_uid.get(),
                    renderer_uid: second_uid.get(),
                },
            ]),
        )
        .unwrap();
        let ModelResponse::Message(message) = response else {
            panic!("expected copy status message");
        };
        assert_eq!(message, "Copied 2 renderer(s)");
        assert!(model.undo_stack.is_empty());

        let targets = || {
            vec![
                WorkspaceToggleInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(first_uid.get()),
                },
                WorkspaceToggleInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(second_uid.get()),
                },
            ]
        };
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceItemsVisible {
                targets: targets(),
                visible: false,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert!(snapshot.objects[0]
            .renderers
            .iter()
            .all(|renderer| !renderer.visible));
        assert_eq!(snapshot.undo_descriptions[0], "Hide 2 Workspace items");
        assert_eq!(model.undo_stack.len(), 1);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().objects[0]
            .renderers
            .iter()
            .all(|renderer| renderer.visible));

        let delete_targets = targets()
            .into_iter()
            .map(|target| WorkspaceDeleteInput {
                kind: target.kind,
                object_uid: target.object_uid,
                renderer_uid: target.renderer_uid,
                scene_uid: None,
                name: None,
                local_index: None,
            })
            .collect();
        let response = handle_model_request(
            &mut model,
            ModelRequest::DeleteWorkspaceItems(delete_targets),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert!(snapshot.objects[0].renderers.is_empty());
        assert_eq!(snapshot.undo_descriptions[0], "Destroy 2 Workspace items");
        assert_eq!(model.undo_stack.len(), 1);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().objects[0].renderers.len(), 2);
    }

    #[test]
    fn workspace_parent_child_delete_orders_children_first() {
        let (mut model, object_uid, first_uid) = model_with_renderer();
        let (group_uid, child_uid) = {
            let object = model
                .desktop
                .scene_manager_mut()
                .object_mut(object_uid)
                .unwrap();
            let group_uid = object.add_renderer("*group", "group");
            let child_uid = object.add_renderer("cpk", "child");
            object
                .renderer_mut(child_uid)
                .unwrap()
                .set_qsc_attr("group", "group");
            (group_uid, child_uid)
        };

        let response = handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceItemsVisible {
                targets: vec![
                    WorkspaceToggleInput {
                        kind: "renderer".to_string(),
                        object_uid: object_uid.get(),
                        renderer_uid: Some(first_uid.get()),
                    },
                    WorkspaceToggleInput {
                        kind: "rendererGroup".to_string(),
                        object_uid: object_uid.get(),
                        renderer_uid: Some(group_uid.get()),
                    },
                ],
                visible: false,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let renderers = &snapshot.objects[0].renderers;
        assert!(
            !renderers
                .iter()
                .find(|renderer| renderer.uid == first_uid.get())
                .unwrap()
                .visible
        );
        assert!(
            renderers
                .iter()
                .find(|renderer| renderer.uid == child_uid.get())
                .unwrap()
                .visible
        );
        assert_eq!(snapshot.undo_descriptions[0], "Hide 2 Workspace items");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::DeleteWorkspaceItems(vec![
                WorkspaceDeleteInput {
                    kind: "object".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: None,
                    scene_uid: None,
                    name: None,
                    local_index: None,
                },
                WorkspaceDeleteInput {
                    kind: "rendererGroup".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(group_uid.get()),
                    scene_uid: None,
                    name: None,
                    local_index: None,
                },
                WorkspaceDeleteInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(child_uid.get()),
                    scene_uid: None,
                    name: None,
                    local_index: None,
                },
            ]),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert!(snapshot.objects.is_empty());
        assert_eq!(snapshot.undo_descriptions[0], "Destroy 3 Workspace items");
        assert_eq!(model.undo_stack.len(), 1);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        let restored = model.desktop.scene_manager().object(object_uid).unwrap();
        assert!(restored.renderer(group_uid).is_some());
        assert!(restored.renderer(child_uid).is_some());
    }

    #[test]
    fn workspace_multiple_camera_and_style_delete_is_undoable() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        model.save_named_camera("camera_0", false).unwrap();
        model.create_named_style("first").unwrap();
        model.create_named_style("second").unwrap();
        model.undo_stack.clear();
        model.redo_stack.clear();

        let response = handle_model_request(
            &mut model,
            ModelRequest::DeleteWorkspaceItems(vec![
                WorkspaceDeleteInput {
                    kind: "style".to_string(),
                    object_uid: 0,
                    renderer_uid: None,
                    scene_uid: Some(scene_uid.get()),
                    name: None,
                    local_index: Some(0),
                },
                WorkspaceDeleteInput {
                    kind: "camera".to_string(),
                    object_uid: 0,
                    renderer_uid: None,
                    scene_uid: Some(scene_uid.get()),
                    name: Some("camera_0".to_string()),
                    local_index: None,
                },
                WorkspaceDeleteInput {
                    kind: "style".to_string(),
                    object_uid: 0,
                    renderer_uid: None,
                    scene_uid: Some(scene_uid.get()),
                    name: None,
                    local_index: Some(1),
                },
            ]),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert!(snapshot
            .cameras
            .iter()
            .all(|camera| camera.name != "camera_0"));
        assert!(snapshot
            .styles
            .iter()
            .filter(|style| style.owner_scene_uid == scene_uid.get())
            .all(|style| style.local_index.is_none()));
        assert_eq!(snapshot.undo_descriptions[0], "Destroy 3 Workspace items");
        assert_eq!(model.undo_stack.len(), 1);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        let snapshot = model.snapshot();
        assert!(snapshot
            .cameras
            .iter()
            .any(|camera| camera.name == "camera_0"));
        assert_eq!(
            snapshot
                .styles
                .iter()
                .filter(|style| {
                    style.owner_scene_uid == scene_uid.get() && style.local_index.is_some()
                })
                .map(|style| style.name.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );

        model.undo_stack.clear();
        model.redo_stack.clear();
        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceItemsVisible {
                targets: vec![
                    WorkspaceToggleInput {
                        kind: "camera".to_string(),
                        object_uid: 0,
                        renderer_uid: None,
                    },
                    WorkspaceToggleInput {
                        kind: "style".to_string(),
                        object_uid: 0,
                        renderer_uid: None,
                    },
                ],
                visible: false,
            },
        )
        .unwrap();
        assert_eq!(model.undo_stack.len(), 1);
        assert!(model
            .snapshot()
            .cameras
            .iter()
            .any(|camera| camera.name == "camera_0"));
    }

    #[test]
    fn workspace_multiple_command_rolls_back_on_error() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let result = handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceItemsVisible {
                targets: vec![
                    WorkspaceToggleInput {
                        kind: "renderer".to_string(),
                        object_uid: object_uid.get(),
                        renderer_uid: Some(renderer_uid.get()),
                    },
                    WorkspaceToggleInput {
                        kind: "renderer".to_string(),
                        object_uid: u64::MAX,
                        renderer_uid: Some(renderer_uid.get()),
                    },
                ],
                visible: false,
            },
        );
        let Err(error) = result else {
            panic!("expected multiple visibility command to fail");
        };
        assert!(error.contains("object"));
        assert!(model.snapshot().objects[0].renderers[0].visible);
        assert!(model.undo_stack.is_empty());
    }

    #[test]
    fn workspace_density_renderer_copy_pastes_to_compatible_map() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let source_uid =
            model
                .desktop
                .add_density_map(scene_uid, "source map", cuemol_xtal::DensityMap::new());
        let target_uid =
            model
                .desktop
                .add_density_map(scene_uid, "target map", cuemol_xtal::DensityMap::new());
        let renderer_uid = model
            .desktop
            .add_density_renderer(source_uid, "contour")
            .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::CopyWorkspaceRenderer(WorkspaceRendererCopyInput {
                object_uid: source_uid.get(),
                renderer_uid: renderer_uid.get(),
            }),
        )
        .unwrap();
        assert!(matches!(response, ModelResponse::Message(_)));

        let response = handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceRenderers(WorkspaceRendererPasteInput {
                object_uid: target_uid.get(),
                destination_group_uid: None,
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected paste snapshot");
        };
        let target = snapshot
            .density_maps
            .iter()
            .find(|map| map.uid == target_uid.get())
            .unwrap();
        assert_eq!(target.renderers.len(), 1);
        assert_eq!(target.renderers[0].name, "contour 1");
        assert_eq!(snapshot.undo_descriptions[0], "Pasted 1 renderer(s)");

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model
            .snapshot()
            .density_maps
            .iter()
            .find(|map| map.uid == target_uid.get())
            .unwrap()
            .renderers
            .is_empty());
        assert!(model.workspace_renderer_clipboard.is_some());
    }

    #[test]
    fn workspace_mixed_renderer_copy_pastes_compatible_items_only() {
        let (mut model, object_uid, molecule_renderer_uid) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let map_uid =
            model
                .desktop
                .add_density_map(scene_uid, "map", cuemol_xtal::DensityMap::new());
        let map_renderer_uid = model
            .desktop
            .add_density_renderer(map_uid, "contour")
            .unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::CopyWorkspaceRenderers(vec![
                WorkspaceRendererCopyInput {
                    object_uid: object_uid.get(),
                    renderer_uid: molecule_renderer_uid.get(),
                },
                WorkspaceRendererCopyInput {
                    object_uid: map_uid.get(),
                    renderer_uid: map_renderer_uid.get(),
                },
            ]),
        )
        .unwrap();
        let ModelResponse::Message(message) = response else {
            panic!("expected copy status message");
        };
        assert_eq!(message, "Copied 2 renderer(s)");

        let response = handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceRenderers(WorkspaceRendererPasteInput {
                object_uid: object_uid.get(),
                destination_group_uid: None,
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.objects[0].renderers.len(), 2);
        assert_eq!(snapshot.density_maps[0].renderers.len(), 1);
        assert_eq!(model.undo_stack.len(), 1);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceRenderers(WorkspaceRendererPasteInput {
                object_uid: map_uid.get(),
                destination_group_uid: None,
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.objects[0].renderers.len(), 1);
        assert_eq!(snapshot.density_maps[0].renderers.len(), 2);
        assert_eq!(snapshot.undo_descriptions[0], "Pasted 1 renderer(s)");
    }

    #[test]
    fn workspace_aux_renderer_style_requests_are_undoable() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let style_index = model.desktop.create_scene_style(scene_uid, "aux").unwrap();
        model
            .desktop
            .update_scene_style(
                scene_uid,
                style_index,
                "aux",
                false,
                "<styles><style id=\"Widecontour\" type=\"renderer\" width=\"2\"/></styles>",
            )
            .unwrap();
        let map_uid =
            model
                .desktop
                .add_density_map(scene_uid, "map", cuemol_xtal::DensityMap::new());
        let renderer_uid = model
            .desktop
            .add_density_renderer(map_uid, "contour")
            .unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::ApplyWorkspaceRendererStyle(WorkspaceRendererStyleInput {
                object_uid: map_uid.get(),
                renderer_uid: renderer_uid.get(),
                style_name: "Widecontour".to_string(),
                component: "shape".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.density_maps[0].renderers[0].style, "Widecontour");
        assert_eq!(snapshot.undo_descriptions[0], "Change style");

        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceRendererStyles(WorkspaceRendererStylesInput {
                object_uid: map_uid.get(),
                renderer_uid: renderer_uid.get(),
                style_names: vec!["Widecontour".to_string(), "EgLineAux".to_string()],
            }),
        )
        .unwrap();
        assert_eq!(
            model.snapshot().density_maps[0].renderers[0].style,
            "Widecontour,EgLineAux"
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(
            model.snapshot().density_maps[0].renderers[0].style,
            "Widecontour"
        );
    }

    #[test]
    fn workspace_lightweight_renderer_common_properties_are_undoable() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let object_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_loaded_lw_object(cuemol_molstr::LoadedLWObject {
                uid: Uid::new(0),
                name: "lightweight".to_string(),
                srctype: "qdflwobj".to_string(),
                raw_data: Vec::new(),
                visible: true,
                xform: cuemol_core::Matrix4::IDENTITY,
                renderers: vec![cuemol_molstr::LoadedLWRenderer {
                    name: "draw".to_string(),
                    data_id: 1,
                    visible: true,
                    xform: cuemol_core::Matrix4::IDENTITY,
                }],
                qsc_node: None,
            });
        let renderer_uid = model
            .desktop
            .lw_object_summary(object_uid)
            .unwrap()
            .renderers[0]
            .uid;
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetLwRendererProperties(LwRendererPropertiesInput {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                name: "overlay".to_string(),
                visible: true,
                locked: true,
                material: "matte".to_string(),
                alpha: 0.4,
                edge_type: "silhouette".to_string(),
                edge_width: 0.2,
                edge_color: "#123456".to_string(),
                data_id: 7,
                specific: BTreeMap::from([("style".to_string(), "CustomLw".to_string())]),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected properties snapshot");
        };
        let renderer = &snapshot.lw_objects[0].renderers[0];
        assert_eq!(renderer.name, "overlay");
        assert!(renderer.locked);
        assert_eq!(renderer.style, "CustomLw");
        assert_eq!(renderer.material, "matte");
        assert_eq!(renderer.alpha, 0.4);
        assert_eq!(renderer.edge_type, "silhouette");
        assert_eq!(renderer.edge_width, 0.2);
        assert_eq!(renderer.edge_color, "#123456");
        assert_eq!(renderer.data_id, 7);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        let snapshot = model.snapshot();
        let renderer = &snapshot.lw_objects[0].renderers[0];
        assert_eq!(renderer.name, "draw");
        assert_eq!(renderer.alpha, 1.0);
        assert_eq!(renderer.data_id, 1);
    }

    #[test]
    fn workspace_object_copy_uses_small_response_and_paste_is_undoable() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::CopyWorkspaceObject(WorkspaceObjectCopyInput {
                object_uid: object_uid.get(),
            }),
        )
        .unwrap();
        let ModelResponse::Message(message) = response else {
            panic!("expected copy status message");
        };
        assert_eq!(message, "Copied object molecule");
        assert!(model.undo_stack.is_empty());

        let response = handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceObject(WorkspaceObjectPasteInput {
                scene_uid: scene_uid.get(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected paste snapshot");
        };
        assert_eq!(snapshot.objects.len(), 2);
        let pasted = snapshot
            .objects
            .iter()
            .find(|object| object.uid != object_uid.get())
            .unwrap();
        assert_eq!(pasted.name, "copy1_molecule");
        assert_eq!(pasted.renderers.len(), 1);
        assert_ne!(pasted.renderers[0].uid, renderer_uid.get());
        assert_eq!(snapshot.undo_descriptions[0], "Paste object");
        assert!(model.mesh.is_none());

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().objects.len(), 1);
        assert!(model.workspace_object_clipboard.is_some());
    }

    #[test]
    fn workspace_renderer_group_create_and_rename_are_undoable() {
        let (mut model, object_uid, _) = model_with_renderer();
        let response = handle_model_request(
            &mut model,
            ModelRequest::CreateWorkspaceRendererGroup(WorkspaceRendererGroupInput {
                object_uid: object_uid.get(),
                group_uid: None,
                name: "group1".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let group_uid = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.renderer_type == "*group")
            .unwrap()
            .uid;
        assert_eq!(snapshot.undo_descriptions[0], "Create renderer group");

        let response = handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceRendererGroup(WorkspaceRendererGroupInput {
                object_uid: object_uid.get(),
                group_uid: Some(group_uid),
                name: "renamed".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert!(snapshot.objects[0]
            .renderers
            .iter()
            .any(|renderer| renderer.uid == group_uid && renderer.name == "renamed"));
        assert_eq!(snapshot.undo_descriptions[0], "Rename renderer group");

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().objects[0]
            .renderers
            .iter()
            .any(|renderer| renderer.uid == group_uid && renderer.name == "group1"));
    }

    #[test]
    fn workspace_object_and_renderer_inline_renames_are_undoable() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceNode(WorkspaceRenameInput {
                kind: "scene".to_string(),
                object_uid: scene_uid.get(),
                renderer_uid: None,
                name: "renamed scene".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.scenes[0].name, "renamed scene");
        assert_eq!(snapshot.undo_descriptions[0], "Rename to renamed scene");

        let response = handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceNode(WorkspaceRenameInput {
                kind: "object".to_string(),
                object_uid: object_uid.get(),
                renderer_uid: None,
                name: "renamed molecule".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.objects[0].name, "renamed molecule");
        assert_eq!(snapshot.undo_descriptions[0], "Rename to renamed molecule");

        let response = handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceNode(WorkspaceRenameInput {
                kind: "renderer".to_string(),
                object_uid: object_uid.get(),
                renderer_uid: Some(renderer_uid.get()),
                name: "renamed renderer".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.objects[0].renderers[0].name, "renamed renderer");
        assert_eq!(snapshot.undo_descriptions[0], "Rename to renamed renderer");

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().objects[0].renderers[0].name, "simple 1");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().objects[0].name, "molecule");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().scenes[0].name, "Untitled 1");
    }

    #[test]
    fn direct_surface_properties_are_validated_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let renderer_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("dsurface", "surface");

        let initial = model.snapshot();
        let renderer = initial.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid.get())
            .unwrap();
        assert_eq!(renderer.specific["detail"], "6");
        assert_eq!(renderer.specific["surftype"], "ses");
        assert_eq!(renderer.specific["colormode"], "molecule");
        assert!(renderer.supports_electrostatic_coloring);

        let mut properties =
            default_renderer_properties_input(object_uid, renderer_uid, "wire surface");
        properties.selection = "chain A".to_string();
        properties.specific = BTreeMap::from([
            ("drawmode".to_string(), "line".to_string()),
            ("width".to_string(), "2.5".to_string()),
            ("surftype".to_string(), "sas".to_string()),
            ("detail".to_string(), "9".to_string()),
            ("target".to_string(), "".to_string()),
            ("showsel".to_string(), "name CA".to_string()),
            ("colormode".to_string(), "potential".to_string()),
        ]);
        handle_model_request(&mut model, ModelRequest::SetRendererProperties(properties)).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("sel"), Some("chain A"));
        assert_eq!(renderer.qsc_attr("drawmode"), Some("line"));
        assert_eq!(renderer.qsc_attr("width"), Some("2.5"));
        assert_eq!(renderer.qsc_attr("surftype"), Some("sas"));
        assert_eq!(renderer.qsc_attr("detail"), Some("9"));
        assert_eq!(renderer.qsc_attr("showsel"), Some("name CA"));
        assert_eq!(renderer.qsc_attr("colormode"), Some("potential"));

        let mut invalid =
            default_renderer_properties_input(object_uid, renderer_uid, "wire surface");
        invalid
            .specific
            .insert("detail".to_string(), "22".to_string());
        assert!(
            handle_model_request(&mut model, ModelRequest::SetRendererProperties(invalid),)
                .is_err()
        );
    }

    #[test]
    fn cpk_and_disorder_properties_are_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let object = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap();
        let cpk_uid = object.add_renderer("cpk", "cpk 1");
        object.add_renderer("tube", "backbone");
        let disorder_uid = object.add_renderer("disorder", "disorder 1");

        let initial = model.snapshot();
        let initial_cpk = initial.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == cpk_uid.get())
            .unwrap();
        assert_eq!(initial_cpk.specific["detail"], "10");
        assert_eq!(initial_cpk.specific["glrender_mode"], "default");

        let mut cpk = default_renderer_properties_input(object_uid, cpk_uid, "spacefill");
        cpk.specific = BTreeMap::from([
            ("vdwr_H".to_string(), "0.8".to_string()),
            ("vdwr_C".to_string(), "2.4".to_string()),
            ("vdwr_N".to_string(), "1.6".to_string()),
            ("vdwr_O".to_string(), "1.4".to_string()),
            ("vdwr_S".to_string(), "2".to_string()),
            ("vdwr_P".to_string(), "2.1".to_string()),
            ("vdwr_X".to_string(), "1.9".to_string()),
            ("detail".to_string(), "9".to_string()),
            ("glrender_mode".to_string(), "shader".to_string()),
        ]);
        let response =
            handle_model_request(&mut model, ModelRequest::SetRendererProperties(cpk)).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let cpk = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == cpk_uid.get())
            .unwrap();
        assert_eq!(cpk.specific["vdwr_C"], "2.4");
        assert_eq!(cpk.specific["detail"], "9");
        assert_eq!(cpk.specific["glrender_mode"], "shader");

        let mut disorder =
            default_renderer_properties_input(object_uid, disorder_uid, "missing loop");
        disorder.specific = BTreeMap::from([
            ("target".to_string(), "backbone".to_string()),
            ("detail".to_string(), "8".to_string()),
            ("width".to_string(), "0.6".to_string()),
            ("dotsep".to_string(), "0.5".to_string()),
            ("loopsize".to_string(), "3".to_string()),
            ("loopsize2".to_string(), "4".to_string()),
            ("defaultcolor".to_string(), "#123456".to_string()),
        ]);
        handle_model_request(&mut model, ModelRequest::SetRendererProperties(disorder)).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(disorder_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("target"), Some("backbone"));
        assert_eq!(renderer.qsc_attr("defaultcolor"), Some("#123456"));
        assert_eq!(renderer.qsc_attr("dotsep"), Some("0.5"));

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(disorder_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("target"), None);
        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(cpk_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("vdwr_C"), None);
        assert_eq!(renderer.qsc_attr("detail"), None);
        assert_eq!(renderer.qsc_attr("glrender_mode"), None);
    }

    #[test]
    fn tube_and_nucl_properties_are_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let object = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap();
        let tube_uid = object.add_renderer("tube", "tube 1");
        let nucl_uid = object.add_renderer("nucl", "nucl 1");
        object
            .renderer_mut(nucl_uid)
            .unwrap()
            .set_qsc_attr("style", "DefaultNucl");

        let mut tube = default_renderer_properties_input(object_uid, tube_uid, "putty tube");
        tube.specific = BTreeMap::from([
            ("section.type".to_string(), "roundsquare".to_string()),
            ("section.width".to_string(), "0.4".to_string()),
            ("section.tuber".to_string(), "2.5".to_string()),
            ("section.sharp".to_string(), "0.7".to_string()),
            ("section.detail".to_string(), "9".to_string()),
            ("axialdetail".to_string(), "12".to_string()),
            ("smooth".to_string(), "0.25".to_string()),
            ("smoothcolor".to_string(), "false".to_string()),
            ("pivotatom".to_string(), "N".to_string()),
            ("start_captype".to_string(), "flat".to_string()),
            ("end_captype".to_string(), "none".to_string()),
            ("segend_fade".to_string(), "true".to_string()),
            ("putty_mode".to_string(), "linear1".to_string()),
            ("putty_tgt".to_string(), "occ".to_string()),
            ("putty_hiscl".to_string(), "4".to_string()),
            ("putty_loscl".to_string(), "2".to_string()),
        ]);
        let response =
            handle_model_request(&mut model, ModelRequest::SetRendererProperties(tube)).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let tube = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == tube_uid.get())
            .unwrap();
        assert_eq!(tube.specific["section.type"], "roundsquare");
        assert_eq!(tube.specific["section.width"], "0.4");
        assert_eq!(tube.specific["section.tuber"], "2.5");
        assert_eq!(tube.specific["putty_mode"], "linear1");
        let nucl = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == nucl_uid.get())
            .unwrap();
        assert_eq!(nucl.specific["section.width"], "0.5");
        assert_eq!(nucl.specific["section.tuber"], "2.5");
        assert_eq!(nucl.specific["axialdetail"], "8");
        assert_eq!(nucl.specific["pivotatom"], "P");
        let qsc = model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .write_qsc_string();
        assert!(qsc.contains("<section type=\"roundsquare\""));
        assert!(qsc.contains("width=\"0.4\""));

        let mut nucl = default_renderer_properties_input(object_uid, nucl_uid, "bases");
        nucl.specific = BTreeMap::from([
            ("show_tube".to_string(), "false".to_string()),
            ("show_basepair".to_string(), "false".to_string()),
            ("base_size".to_string(), "0.8".to_string()),
            ("base_detail".to_string(), "7".to_string()),
            ("base_thick".to_string(), "0.3".to_string()),
            ("base_type".to_string(), "detail2".to_string()),
        ]);
        handle_model_request(&mut model, ModelRequest::SetRendererProperties(nucl)).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(nucl_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("show_tube"), Some("false"));
        assert_eq!(renderer.qsc_attr("base_type"), Some("detail2"));

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(nucl_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("show_tube"), None);
        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(tube_uid)
            .unwrap();
        assert_eq!(renderer.qsc_child_attr("section", "width"), None);
        assert_eq!(renderer.qsc_attr("putty_mode"), None);
    }

    #[test]
    fn ribbon_and_cartoon_properties_are_undoable_and_persisted() {
        let (mut model, object_uid, _) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let object = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap();
        let ribbon_uid = object.add_renderer("ribbon", "ribbon 1");
        let cartoon_uid = object.add_renderer("cartoon", "cartoon 1");

        let mut ribbon = default_renderer_properties_input(object_uid, ribbon_uid, "wide ribbon");
        ribbon.specific = BTreeMap::from([
            ("helix.type".to_string(), "roundsquare".to_string()),
            ("helix.width".to_string(), "0.4".to_string()),
            ("helix.tuber".to_string(), "3".to_string()),
            ("helix.sharp".to_string(), "0.7".to_string()),
            ("helix.detail".to_string(), "12".to_string()),
            ("sheethead.type".to_string(), "arrow".to_string()),
            ("sheethead.gamma".to_string(), "3".to_string()),
            ("sheethead.basw".to_string(), "0.2".to_string()),
            ("sheethead.arrow".to_string(), "2.1".to_string()),
            ("helix_smooth".to_string(), "1.5".to_string()),
            ("helix_usebackcol".to_string(), "true".to_string()),
            ("helix_backcol".to_string(), "#123456".to_string()),
        ]);
        let response =
            handle_model_request(&mut model, ModelRequest::SetRendererProperties(ribbon)).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let ribbon = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == ribbon_uid.get())
            .unwrap();
        assert_eq!(ribbon.specific["helix.type"], "roundsquare");
        assert_eq!(ribbon.specific["helix.width"], "0.4");
        assert_eq!(ribbon.specific["sheethead.type"], "arrow");
        assert_eq!(ribbon.specific["helix_backcol"], "#123456");

        let mut cartoon =
            default_renderer_properties_input(object_uid, cartoon_uid, "anchored cartoon");
        cartoon.specific = BTreeMap::from([
            ("anchor_sel".to_string(), "chain A".to_string()),
            ("anchor_weight".to_string(), "12".to_string()),
            ("helix_ribbon".to_string(), "true".to_string()),
            ("helix_width_mode".to_string(), "wavy".to_string()),
            ("helix_width".to_string(), "2.6".to_string()),
            ("helix_wplus".to_string(), "0.4".to_string()),
            ("ribhelix.type".to_string(), "rectangle".to_string()),
            ("ribhelix.width".to_string(), "0.3".to_string()),
            ("ribhelix.tuber".to_string(), "5".to_string()),
            ("ribhelix.detail".to_string(), "13".to_string()),
            ("ribhelix_head.type".to_string(), "flat".to_string()),
        ]);
        let response =
            handle_model_request(&mut model, ModelRequest::SetRendererProperties(cartoon)).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        let cartoon = snapshot.objects[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == cartoon_uid.get())
            .unwrap();
        assert_eq!(cartoon.specific["anchor_sel"], "chain A");
        assert_eq!(cartoon.specific["anchor_weight"], "12");
        assert_eq!(cartoon.specific["helix_ribbon"], "true");
        assert_eq!(cartoon.specific["ribhelix.type"], "rectangle");
        assert_eq!(cartoon.specific["ribhelix_head.type"], "flat");

        let qsc = model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .write_qsc_string();
        assert!(qsc.contains("anchor_sel=\"chain A\""));
        assert!(qsc.contains("<ribhelix type=\"rectangle\""));

        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(cartoon_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("anchor_sel"), None);
        assert_eq!(renderer.qsc_child_attr("ribhelix", "type"), None);
        model.undo(0).unwrap();
        let renderer = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(ribbon_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("helix_backcol"), None);
        assert_eq!(renderer.qsc_child_attr("helix", "width"), None);
    }

    #[test]
    fn slab_mouse_action_scales_the_camera_clipping_depth() {
        let mut model = model_without_gpu();
        model.camera.set_slab_depth(50.0);
        model.update_camera("slab", 0.5, 0.0, 480).unwrap();
        assert_eq!(model.camera.slab_depth(), 25.0);
    }

    #[test]
    fn view_property_editor_roundtrips_the_reference_view_fields() {
        let mut model = model_without_gpu();
        let response = handle_model_request(&mut model, ModelRequest::GetViewProperties).unwrap();
        let ModelResponse::ViewProperties(properties) = response else {
            panic!("expected view properties");
        };
        assert_eq!(properties.zoom, 50.0);
        assert_eq!(properties.stereo_mode, "none");
        assert_eq!(properties.center_mark, "crosshair");
        assert!(!properties.has_hw_stereo);

        let response = handle_model_request(
            &mut model,
            ModelRequest::SetViewProperties {
                properties: ViewPropertiesInput {
                    name: "detail".to_string(),
                    zoom: 75.0,
                    distance: 300.0,
                    slab: 80.0,
                    center: [1.0, 2.0, 3.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    perspective: true,
                    trans_mms: true,
                    rot_mms: true,
                    stereo_mode: "cross".to_string(),
                    stereo_dist: 2.5,
                    swap_stereo_eyes: true,
                    center_mark: "axis".to_string(),
                },
                width: 640,
                height: 480,
            },
        )
        .unwrap();
        let ModelResponse::ViewPropertiesUpdate(update) = response else {
            panic!("expected updated view properties");
        };
        assert_eq!(update.properties.name, "detail");
        assert_eq!(update.properties.center, [1.0, 2.0, 3.0]);
        assert_eq!(update.properties.stereo_mode, "cross");
        assert_eq!(update.properties.center_mark, "axis");
        assert!(update.properties.swap_stereo_eyes);
        assert!(update.viewport.perspective);
        assert!(update.snapshot.can_undo);
        assert!(update.snapshot.scenes[0].modified);

        model.undo(0).unwrap();
        let restored = model.view_properties();
        assert_eq!(restored.zoom, 50.0);
        assert_eq!(restored.center_mark, "crosshair");
        assert!(!restored.swap_stereo_eyes);

        model
            .update_camera("center-mark-none", 0.0, 0.0, 480)
            .unwrap();
        let viewport = model.render(640, 480);
        assert_eq!(viewport.center_mark, "none");
    }

    #[test]
    fn open_pdb_request_loads_downloaded_files_and_supports_undo() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-open-pdb-request-{}.pdb",
            std::process::id()
        ));
        fs::write(
            &path,
            b"ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let mut model = model_without_gpu();
        let setup_response = handle_model_request(
            &mut model,
            ModelRequest::GetPdbRendererSetup {
                pdb_id: "1CRN".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::PdbRendererSetup(setup_defaults) = setup_response else {
            panic!("expected PDB renderer setup");
        };
        assert_eq!(setup_defaults.object_name, "1crn");
        assert!(setup_defaults
            .options
            .iter()
            .any(|option| option.name == "Default1RendPreset" && option.preset));
        let response = handle_model_request(
            &mut model,
            ModelRequest::OpenPdb {
                pdb_id: "1CRN".to_string(),
                files: vec![crate::PdbDownloadedFile {
                    kind: crate::GuiPdbFileKind::Coordinates,
                    path: path.clone(),
                }],
                renderer_setup: Some(crate::GuiRendererSetup {
                    object_name: "downloaded".to_string(),
                    renderer_type: "simple".to_string(),
                    renderer_name: "lines".to_string(),
                    selection: None,
                    recenter: false,
                }),
                width: 640,
                height: 480,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected PDB viewport update");
        };
        assert_eq!(update.snapshot.objects.len(), 1);
        assert_eq!(update.snapshot.objects[0].name, "downloaded");
        assert!(update.snapshot.objects[0]
            .renderers
            .iter()
            .any(|renderer| renderer.name == "lines" && renderer.renderer_type == "simple"));
        assert!(update.snapshot.can_undo);
        assert_eq!(update.snapshot.status, "Opened PDB 1CRN: 1 file(s)");
        model.undo(0).unwrap();
        assert!(model.snapshot().objects.is_empty());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn opening_a_missing_path_fails_without_creating_an_undo_entry() {
        let mut model = model_without_gpu();
        let missing = std::env::temp_dir()
            .join(format!("cuemol-missing-{}.pdb", std::process::id()))
            .to_string_lossy()
            .to_string();
        let error =
            apply_open_paths(&mut model, vec![missing]).expect_err("missing paths must fail");
        assert!(error.contains("Loaded 0; failed:"));
        assert!(model.undo_stack.is_empty());
        assert!(model.desktop.active_object_summaries().is_empty());
    }

    #[test]
    fn reload_scene_replaces_the_active_scene_and_clears_undo_history() {
        let path =
            std::env::temp_dir().join(format!("cuemol-reload-scene-{}.qsc", std::process::id()));
        fs::write(
            &path,
            b"<?xml version=\"1.0\"?><scene marker=\"file\"><qsc_opts version=\"QDF0\"/></scene>",
        )
        .unwrap();
        let mut model = model_without_gpu();
        apply_open_paths(&mut model, vec![path.to_string_lossy().into_owned()]).unwrap();
        let scene_uid = model.desktop.active_scene().unwrap();
        let scene_count = model.desktop.scene_summaries().len();
        model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .set_scene_attribute("marker", "dirty");
        handle_model_request(&mut model, ModelRequest::SetBackground("white".to_string())).unwrap();

        let response = handle_model_request(&mut model, ModelRequest::ReloadScene).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(model.desktop.active_scene(), Some(scene_uid));
        assert_eq!(model.desktop.scene_summaries().len(), scene_count);
        assert_eq!(
            model
                .desktop
                .scene_manager()
                .scene(scene_uid)
                .unwrap()
                .scene_attribute("marker"),
            Some("file")
        );
        assert!(!snapshot.can_undo);
        assert!(!snapshot.can_redo);
        assert!(
            !snapshot
                .scenes
                .iter()
                .find(|scene| scene.active)
                .unwrap()
                .modified
        );
        assert!(snapshot.status.ends_with("] is reloaded."));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn reload_scene_rejects_an_unsaved_scene() {
        let mut model = model_without_gpu();
        let error = handle_model_request(&mut model, ModelRequest::ReloadScene)
            .err()
            .expect("an unsaved scene must not be reloaded");
        assert_eq!(error, "active scene has no source path");
    }

    #[test]
    fn failed_reload_preserves_the_current_scene_and_undo_history() {
        let path =
            std::env::temp_dir().join(format!("cuemol-reload-failure-{}.qsc", std::process::id()));
        fs::write(
            &path,
            b"<?xml version=\"1.0\"?><scene marker=\"file\"><qsc_opts version=\"QDF0\"/></scene>",
        )
        .unwrap();
        let mut model = model_without_gpu();
        apply_open_paths(&mut model, vec![path.to_string_lossy().into_owned()]).unwrap();
        let scene_uid = model.desktop.active_scene().unwrap();
        model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .set_scene_attribute("marker", "dirty");
        handle_model_request(&mut model, ModelRequest::SetBackground("white".to_string())).unwrap();
        let undo_count = model.undo_stack.len();
        assert!(model.modified_scenes.contains(&scene_uid));
        fs::write(&path, b"not a QSC file").unwrap();

        assert!(handle_model_request(&mut model, ModelRequest::ReloadScene).is_err());
        assert_eq!(model.undo_stack.len(), undo_count);
        assert_eq!(
            model
                .desktop
                .scene_manager()
                .scene(scene_uid)
                .unwrap()
                .scene_attribute("marker"),
            Some("dirty")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn saving_to_the_scene_source_creates_a_backup_and_clears_undo() {
        let path =
            std::env::temp_dir().join(format!("cuemol-save-backup-{}.qsc", std::process::id()));
        let backup_path = PathBuf::from(format!("{}.bak", path.display()));
        let mut model = model_without_gpu();
        handle_model_request(
            &mut model,
            ModelRequest::SaveScene {
                path: path.to_string_lossy().into_owned(),
                options: None,
            },
        )
        .unwrap();
        let first_contents = fs::read(&path).unwrap();
        let scene_uid = model.desktop.active_scene().unwrap();
        model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .set_scene_attribute("saved_version", "second");
        handle_model_request(&mut model, ModelRequest::SetBackground("white".to_string())).unwrap();
        assert!(!model.undo_stack.is_empty());

        let response = handle_model_request(
            &mut model,
            ModelRequest::SaveScene {
                path: path.to_string_lossy().into_owned(),
                options: None,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(fs::read(&backup_path).unwrap(), first_contents);
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("saved_version=\"second\""));
        assert!(!snapshot.can_undo);
        assert!(!snapshot.can_redo);
        assert!(
            !snapshot
                .scenes
                .iter()
                .find(|scene| scene.active)
                .unwrap()
                .modified
        );
        assert_eq!(
            snapshot
                .scenes
                .iter()
                .find(|scene| scene.active)
                .unwrap()
                .source,
            path.to_string_lossy()
        );
        fs::remove_file(path).unwrap();
        fs::remove_file(backup_path).unwrap();
    }

    #[test]
    fn panel_display_options_update_the_snapshot() {
        let mut model = model_without_gpu();
        handle_model_request(
            &mut model,
            ModelRequest::SetDisplayOption {
                option: "unit-cell".to_string(),
                value: true,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SetDisplayOption {
                option: "color-proof".to_string(),
                value: true,
            },
        )
        .unwrap();
        handle_model_request(&mut model, ModelRequest::SetDensityLevel(2.4)).unwrap();
        let response =
            handle_model_request(&mut model, ModelRequest::SetBackground("white".to_string()))
                .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert!(snapshot.show_unit_cell);
        assert!(snapshot.color_proofing);
        assert_eq!(snapshot.density_level, 2.4);
        assert_eq!(snapshot.background, "white");
    }

    #[test]
    fn model_undo_and_redo_restore_gui_mutations() {
        let mut model = model_without_gpu();
        handle_model_request(&mut model, ModelRequest::SetBackground("white".to_string())).unwrap();
        handle_model_request(&mut model, ModelRequest::SetDensityLevel(2.4)).unwrap();
        assert_eq!(
            model.snapshot().undo_descriptions,
            ["Density level: 2.40 σ", "Background: white"]
        );
        handle_model_request(&mut model, ModelRequest::Undo(1)).unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.background, "black");
        assert_eq!(snapshot.density_level, 1.5);
        assert!(!snapshot.can_undo);
        assert!(snapshot.can_redo);
        assert_eq!(
            snapshot.redo_descriptions,
            ["Background: white", "Density level: 2.40 σ"]
        );
        assert_eq!(snapshot.status, "Undo: Background: white");
        handle_model_request(&mut model, ModelRequest::Redo(1)).unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.background, "white");
        assert_eq!(snapshot.density_level, 2.4);
        assert_eq!(snapshot.status, "Redo: Density level: 2.40 σ");
    }

    #[test]
    fn density_map_workspace_commands_update_snapshot_and_support_undo() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let mut map = cuemol_xtal::DensityMap::new();
        map.set_map_byte_array(
            &[0, 255, 255, 0, 255, 0, 0, 255],
            [2, 2, 2],
            -1.0,
            1.0,
            0.0,
            0.5,
        );
        map.set_map_params([0, 0, 0], [2, 2, 2]);
        map.set_xtal_params(2.0, 2.0, 2.0, 90.0, 90.0, 90.0);
        let map_uid = model.desktop.add_density_map(scene_uid, "density", map);

        handle_model_request(
            &mut model,
            ModelRequest::AddDensityRenderer {
                map_uid: map_uid.get(),
                renderer_type: "contour".to_string(),
                group_name: None,
            },
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.density_maps.len(), 1);
        assert_eq!(snapshot.density_maps[0].scene_uid, scene_uid.get());
        assert_eq!(snapshot.density_maps[0].class_name, "DensityMap");
        assert_eq!(snapshot.density_maps[0].renderers.len(), 1);
        let renderer_uid = snapshot.density_maps[0].renderers[0].uid;

        let mut properties =
            default_renderer_properties_input(map_uid, Uid::new(renderer_uid), "edited contour");
        properties.selection.clear();
        properties.material = "glass".to_string();
        properties.alpha = 0.5;
        properties.specific = BTreeMap::from([
            ("siglevel".to_string(), "0".to_string()),
            ("width".to_string(), "2.5".to_string()),
            ("bufsize".to_string(), "150".to_string()),
            ("autoupdate".to_string(), "false".to_string()),
        ]);
        handle_model_request(
            &mut model,
            ModelRequest::SetDensityRendererProperties(properties),
        )
        .unwrap();
        let snapshot = model.snapshot();
        let renderer = &snapshot.density_maps[0].renderers[0];
        assert_eq!(renderer.name, "edited contour");
        assert_eq!(renderer.material, "glass");
        assert_eq!(renderer.alpha, 0.5);
        assert_eq!(renderer.specific["width"], "2.5");
        assert_eq!(renderer.specific["bufsize"], "150");
        assert!(snapshot.can_undo);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.density_maps[0].renderers[0].name, "contour 1");
        assert_eq!(snapshot.density_maps[0].renderers[0].alpha, 1.0);
        handle_model_request(&mut model, ModelRequest::Redo(0)).unwrap();
        assert_eq!(
            model.snapshot().density_maps[0].renderers[0].name,
            "edited contour"
        );

        handle_model_request(
            &mut model,
            ModelRequest::SetDensityMapProperties(ObjectPropertiesInput {
                object_uid: map_uid.get(),
                name: "renamed density".to_string(),
                visible: false,
                locked: true,
            }),
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.density_maps[0].name, "renamed density");
        assert!(!snapshot.density_maps[0].visible);
        assert!(snapshot.density_maps[0].locked);

        handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceNode(WorkspaceRenameInput {
                kind: "densityMap".to_string(),
                object_uid: map_uid.get(),
                renderer_uid: None,
                name: "inline density".to_string(),
            }),
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceNode(WorkspaceRenameInput {
                kind: "densityRenderer".to_string(),
                object_uid: map_uid.get(),
                renderer_uid: Some(renderer_uid),
                name: "inline contour".to_string(),
            }),
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.density_maps[0].name, "inline density");
        assert_eq!(snapshot.density_maps[0].renderers[0].name, "inline contour");
        assert_eq!(snapshot.undo_descriptions[0], "Rename to inline contour");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().density_maps[0].name, "renamed density");
        assert_eq!(
            model.snapshot().density_maps[0].renderers[0].name,
            "edited contour"
        );

        handle_model_request(
            &mut model,
            ModelRequest::AddDensityRenderer {
                map_uid: map_uid.get(),
                renderer_type: "isosurf".to_string(),
                group_name: None,
            },
        )
        .unwrap();
        let isosurface_uid = model.snapshot().density_maps[0]
            .renderers
            .iter()
            .find(|renderer| renderer.renderer_type == "isosurf")
            .unwrap()
            .uid;
        handle_model_request(
            &mut model,
            ModelRequest::GenerateDensitySurface {
                map_uid: map_uid.get(),
                renderer_uid: isosurface_uid,
            },
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.surfaces.len(), 1);
        assert_eq!(snapshot.surfaces[0].name, "renamed density_sf1");
        assert_eq!(snapshot.surfaces[0].format, "qdfsurf");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().surfaces.is_empty());
        handle_model_request(&mut model, ModelRequest::Redo(0)).unwrap();
        assert_eq!(model.snapshot().surfaces.len(), 1);
    }

    #[test]
    fn density_renderer_groups_are_exposed_and_editable() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let mut map = cuemol_xtal::DensityMap::new();
        map.set_map_byte_array(
            &[0, 255, 255, 0, 255, 0, 0, 255],
            [2, 2, 2],
            -1.0,
            1.0,
            0.0,
            0.5,
        );
        map.set_map_params([0, 0, 0], [2, 2, 2]);
        map.set_xtal_params(2.0, 2.0, 2.0, 90.0, 90.0, 90.0);
        let map_uid = model.desktop.add_density_map(scene_uid, "density", map);

        handle_model_request(
            &mut model,
            ModelRequest::CreateWorkspaceRendererGroup(WorkspaceRendererGroupInput {
                object_uid: map_uid.get(),
                group_uid: None,
                name: "maps".to_string(),
            }),
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::AddDensityRenderer {
                map_uid: map_uid.get(),
                renderer_type: "contour".to_string(),
                group_name: Some("maps".to_string()),
            },
        )
        .unwrap();
        let snapshot = model.snapshot();
        let group = snapshot.density_maps[0]
            .renderers
            .iter()
            .find(|renderer| renderer.renderer_type == "*group")
            .unwrap();
        let group_uid = group.uid;
        assert_eq!(group.name, "maps");
        assert_eq!(
            snapshot.density_maps[0]
                .renderers
                .iter()
                .find(|renderer| renderer.renderer_type == "contour")
                .unwrap()
                .group
                .as_deref(),
            Some("maps")
        );
        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceRendererCollapsed(WorkspaceRendererCollapsedInput {
                object_uid: map_uid.get(),
                renderer_uid: group_uid,
                collapsed: true,
            }),
        )
        .unwrap();
        assert!(
            model.snapshot().density_maps[0]
                .renderers
                .iter()
                .find(|renderer| renderer.uid == group_uid)
                .unwrap()
                .ui_collapsed
        );

        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceVisible {
                target: WorkspaceToggleInput {
                    kind: "rendererGroup".to_string(),
                    object_uid: map_uid.get(),
                    renderer_uid: Some(group_uid),
                },
                visible: false,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceNode(WorkspaceRenameInput {
                kind: "densityRenderer".to_string(),
                object_uid: map_uid.get(),
                renderer_uid: Some(group_uid),
                name: "renamed maps".to_string(),
            }),
        )
        .unwrap();
        let snapshot = model.snapshot();
        let group = snapshot.density_maps[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == group_uid)
            .unwrap();
        assert_eq!(group.name, "renamed maps");
        assert!(!group.visible);
        let renderer_uid = snapshot.density_maps[0]
            .renderers
            .iter()
            .find(|renderer| renderer.renderer_type == "contour")
            .unwrap()
            .uid;

        handle_model_request(
            &mut model,
            ModelRequest::MoveWorkspaceRenderer(WorkspaceMoveRendererInput {
                object_uid: map_uid.get(),
                renderer_uid,
                target_uid: group_uid,
                position: "before".to_string(),
            }),
        )
        .unwrap();
        assert!(model.snapshot().density_maps[0]
            .renderers
            .iter()
            .find(|renderer| renderer.uid == renderer_uid)
            .unwrap()
            .group
            .is_none());
        handle_model_request(
            &mut model,
            ModelRequest::MoveWorkspaceRenderer(WorkspaceMoveRendererInput {
                object_uid: map_uid.get(),
                renderer_uid,
                target_uid: group_uid,
                position: "inside".to_string(),
            }),
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::AddDensityRenderer {
                map_uid: map_uid.get(),
                renderer_type: "isosurf".to_string(),
                group_name: Some("renamed maps".to_string()),
            },
        )
        .unwrap();
        let second_uid = model.snapshot().density_maps[0]
            .renderers
            .iter()
            .find(|renderer| renderer.renderer_type == "isosurf")
            .unwrap()
            .uid;
        handle_model_request(
            &mut model,
            ModelRequest::MoveWorkspaceRenderer(WorkspaceMoveRendererInput {
                object_uid: map_uid.get(),
                renderer_uid: second_uid,
                target_uid: renderer_uid,
                position: "before".to_string(),
            }),
        )
        .unwrap();
        assert_eq!(
            model.snapshot().density_maps[0]
                .renderers
                .iter()
                .filter(|renderer| renderer.renderer_type != "*group")
                .map(|renderer| renderer.uid)
                .collect::<Vec<_>>(),
            vec![second_uid, renderer_uid]
        );

        let undo_count = model.undo_stack.len();
        handle_model_request(
            &mut model,
            ModelRequest::MoveWorkspaceRenderers(WorkspaceMoveRenderersInput {
                object_uid: map_uid.get(),
                renderer_uids: vec![second_uid, renderer_uid],
                target_uid: group_uid,
                position: "before".to_string(),
            }),
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(model.undo_stack.len(), undo_count + 1);
        assert_eq!(snapshot.undo_descriptions[0], "Move 2 renderers");
        assert!(snapshot.density_maps[0]
            .renderers
            .iter()
            .filter(|renderer| renderer.renderer_type != "*group")
            .all(|renderer| renderer.group.is_none()));
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().density_maps[0]
            .renderers
            .iter()
            .filter(|renderer| renderer.renderer_type != "*group")
            .all(|renderer| renderer.group.as_deref() == Some("renamed maps")));

        handle_model_request(
            &mut model,
            ModelRequest::CopyWorkspaceRenderer(WorkspaceRendererCopyInput {
                object_uid: map_uid.get(),
                renderer_uid: group_uid,
            }),
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceRenderers(WorkspaceRendererPasteInput {
                object_uid: map_uid.get(),
                destination_group_uid: None,
            }),
        )
        .unwrap();
        let pasted_group_uid = model.snapshot().density_maps[0]
            .renderers
            .iter()
            .find(|renderer| renderer.name == "renamed maps(1)")
            .unwrap()
            .uid;
        handle_model_request(
            &mut model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "rendererGroup".to_string(),
                object_uid: map_uid.get(),
                renderer_uid: Some(pasted_group_uid),
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();

        handle_model_request(
            &mut model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "densityRenderer".to_string(),
                object_uid: map_uid.get(),
                renderer_uid: Some(group_uid),
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        assert!(model.snapshot().density_maps[0].renderers.is_empty());
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().density_maps[0].renderers.len(), 3);
    }

    #[test]
    fn surface_workspace_commands_update_snapshot_and_support_undo() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let data = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let surface_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_loaded_surface(cuemol_molstr::LoadedSurface {
                uid: Uid::new(0),
                name: "surface".to_string(),
                object_type: "MolSurfObj".to_string(),
                srctype: "obj".to_string(),
                data: data.to_string(),
                raw_data: data.as_bytes().to_vec(),
                qsc_node: None,
            });
        let snapshot = model.snapshot();
        assert_eq!(snapshot.surfaces.len(), 1);
        assert_eq!(snapshot.surfaces[0].class_name, "MolSurfObj");
        assert_eq!(snapshot.surfaces[0].renderers.len(), 1);
        let renderer_uid = snapshot.surfaces[0].renderers[0].uid;
        assert!(snapshot.surfaces[0].renderers[0].supports_coloring);
        assert_eq!(
            snapshot.surfaces[0].renderers[0]
                .scheme_coloring_type
                .as_deref(),
            Some("PaintColoring")
        );
        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceRendererColoring(WorkspaceRendererColoringInput {
                object_uid: surface_uid.get(),
                renderer_uid,
                preset: "rainbow".to_string(),
            }),
        )
        .unwrap();
        assert_eq!(
            model.snapshot().surfaces[0].renderers[0]
                .scheme_coloring_type
                .as_deref(),
            Some("RainbowColoring")
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();

        let mut properties =
            default_renderer_properties_input(surface_uid, Uid::new(renderer_uid), "surface wire");
        properties.material = "glass".to_string();
        properties.alpha = 0.5;
        properties.specific = BTreeMap::from([
            ("drawmode".to_string(), "line".to_string()),
            ("width".to_string(), "2".to_string()),
            ("defaultcolor".to_string(), "#ff0000".to_string()),
            ("colormode".to_string(), "solid".to_string()),
            ("showsel".to_string(), "*".to_string()),
        ]);
        handle_model_request(
            &mut model,
            ModelRequest::SetSurfaceRendererProperties(properties),
        )
        .unwrap();
        let snapshot = model.snapshot();
        let renderer = &snapshot.surfaces[0].renderers[0];
        assert_eq!(renderer.name, "surface wire");
        assert_eq!(renderer.material, "glass");
        assert_eq!(renderer.alpha, 0.5);
        assert_eq!(renderer.specific["drawmode"], "line");
        assert_eq!(renderer.specific["width"], "2");
        assert!(snapshot.can_undo);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().surfaces[0].renderers[0].name, "molsurf 1");
        handle_model_request(&mut model, ModelRequest::Redo(0)).unwrap();
        assert_eq!(
            model.snapshot().surfaces[0].renderers[0].name,
            "surface wire"
        );

        handle_model_request(
            &mut model,
            ModelRequest::SetSurfaceProperties(ObjectPropertiesInput {
                object_uid: surface_uid.get(),
                name: "renamed surface".to_string(),
                visible: false,
                locked: true,
            }),
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.surfaces[0].name, "renamed surface");
        assert!(!snapshot.surfaces[0].visible);
        assert!(snapshot.surfaces[0].locked);

        handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceNode(WorkspaceRenameInput {
                kind: "surface".to_string(),
                object_uid: surface_uid.get(),
                renderer_uid: None,
                name: "inline surface".to_string(),
            }),
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceNode(WorkspaceRenameInput {
                kind: "surfaceRenderer".to_string(),
                object_uid: surface_uid.get(),
                renderer_uid: Some(renderer_uid),
                name: "inline surface renderer".to_string(),
            }),
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.surfaces[0].name, "inline surface");
        assert_eq!(
            snapshot.surfaces[0].renderers[0].name,
            "inline surface renderer"
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().surfaces[0].name, "renamed surface");
        assert_eq!(
            model.snapshot().surfaces[0].renderers[0].name,
            "surface wire"
        );
    }

    #[test]
    fn workspace_state_commands_toggle_and_undo_object_and_renderer() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceObjectCollapsed(WorkspaceObjectCollapsedInput {
                object_uid: object_uid.get(),
                collapsed: true,
            }),
        )
        .unwrap();
        assert!(model.snapshot().objects[0].ui_collapsed);
        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceVisible {
                target: WorkspaceToggleInput {
                    kind: "object".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: None,
                },
                visible: false,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceLocked {
                target: WorkspaceToggleInput {
                    kind: "object".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: None,
                },
                locked: true,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceVisible {
                target: WorkspaceToggleInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(renderer_uid.get()),
                },
                visible: false,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SetWorkspaceLocked {
                target: WorkspaceToggleInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(renderer_uid.get()),
                },
                locked: true,
            },
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert!(!snapshot.objects[0].visible);
        assert!(snapshot.objects[0].locked);
        assert!(!snapshot.objects[0].renderers[0].visible);
        assert!(snapshot.objects[0].renderers[0].locked);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(!model.snapshot().objects[0].renderers[0].locked);
        assert!(model.snapshot().objects[0].ui_collapsed);
        assert_eq!(model.status, "Undo: Workspace item locked");
    }

    #[test]
    fn workspace_edits_preserve_molecule_detail_keys_when_structure_is_unchanged() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let original_key = model.snapshot().structure_key;
        let original_revision = model.structure_revision;

        handle_model_request(
            &mut model,
            ModelRequest::SetObjectProperties(ObjectPropertiesInput {
                object_uid: object_uid.get(),
                name: "molecule".to_string(),
                visible: false,
                locked: true,
            }),
        )
        .unwrap();
        assert_eq!(model.snapshot().structure_key, original_key);
        assert_eq!(model.structure_revision, original_revision);
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().structure_key, original_key);
        handle_model_request(&mut model, ModelRequest::Redo(0)).unwrap();
        assert_eq!(model.snapshot().structure_key, original_key);

        handle_model_request(
            &mut model,
            ModelRequest::SetObjectProperties(ObjectPropertiesInput {
                object_uid: object_uid.get(),
                name: "renamed molecule".to_string(),
                visible: false,
                locked: true,
            }),
        )
        .unwrap();
        let renamed_key = model.snapshot().structure_key;
        assert_ne!(renamed_key, original_key);
        assert_eq!(model.structure_revision, original_revision);
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().structure_key, original_key);
        handle_model_request(&mut model, ModelRequest::Redo(0)).unwrap();
        assert_eq!(model.snapshot().structure_key, renamed_key);

        handle_model_request(
            &mut model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "renderer".to_string(),
                object_uid: object_uid.get(),
                renderer_uid: Some(renderer_uid.get()),
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        assert_eq!(model.snapshot().structure_key, renamed_key);
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().structure_key, renamed_key);
        handle_model_request(&mut model, ModelRequest::Redo(0)).unwrap();
        assert_eq!(model.snapshot().structure_key, renamed_key);

        handle_model_request(
            &mut model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "object".to_string(),
                object_uid: object_uid.get(),
                renderer_uid: None,
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        let deleted_key = model.snapshot().structure_key;
        assert_ne!(deleted_key, renamed_key);
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().structure_key, renamed_key);
        handle_model_request(&mut model, ModelRequest::Redo(0)).unwrap();
        assert_eq!(model.snapshot().structure_key, deleted_key);
    }

    #[test]
    fn undo_and_redo_restore_molecule_structure_revisions() {
        let mut model = model_without_gpu();
        let original_revision = model.structure_revision;
        let undo = model.capture_undo();
        model.touch_structure_details();
        let changed_revision = model.structure_revision;
        model.push_undo(undo);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.structure_revision, original_revision);
        handle_model_request(&mut model, ModelRequest::Redo(0)).unwrap();
        assert_eq!(model.structure_revision, changed_revision);
    }

    #[test]
    fn camera_workspace_commands_update_snapshot_and_support_undo() {
        let mut model = model_without_gpu();
        model.camera.set_zoom(63.0);
        handle_model_request(
            &mut model,
            ModelRequest::CreateCamera("camera_0".to_string()),
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.cameras.len(), 1);
        assert_eq!(snapshot.cameras[0].name, "camera_0");
        assert_eq!(
            snapshot.cameras[0].scene_uid,
            snapshot.active_scene_uid.unwrap()
        );
        assert!(snapshot.can_undo);
        let duplicate = handle_model_request(
            &mut model,
            ModelRequest::CreateCamera("camera_0".to_string()),
        );
        assert_eq!(
            duplicate.err().as_deref(),
            Some("camera already exists: camera_0")
        );

        model.camera.set_zoom(12.0);
        handle_model_request(&mut model, ModelRequest::LoadCamera("camera_0".to_string())).unwrap();
        assert_eq!(model.camera.zoom(), 63.0);

        let response = handle_model_request(
            &mut model,
            ModelRequest::GetCameraProperties("camera_0".to_string()),
        )
        .unwrap();
        let ModelResponse::ViewProperties(properties) = response else {
            panic!("expected camera properties");
        };
        assert_eq!(properties.zoom, 63.0);
        handle_model_request(
            &mut model,
            ModelRequest::SetCameraProperties {
                old_name: "camera_0".to_string(),
                properties: ViewPropertiesInput {
                    name: "camera_renamed".to_string(),
                    zoom: 91.0,
                    distance: properties.distance,
                    slab: properties.slab,
                    center: properties.center,
                    rotation: properties.rotation,
                    perspective: properties.perspective,
                    trans_mms: properties.trans_mms,
                    rot_mms: properties.rot_mms,
                    stereo_mode: properties.stereo_mode,
                    stereo_dist: properties.stereo_dist,
                    swap_stereo_eyes: properties.swap_stereo_eyes,
                    center_mark: properties.center_mark,
                },
            },
        )
        .unwrap();
        assert_eq!(model.snapshot().cameras[0].name, "camera_renamed");
        assert_eq!(
            model
                .named_camera_properties("camera_renamed")
                .unwrap()
                .zoom,
            91.0
        );

        let scene_uid = model.desktop.active_scene().unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceCamera(WorkspaceCameraRenameInput {
                scene_uid: scene_uid.get(),
                old_name: "camera_renamed".to_string(),
                new_name: " camera_inline ".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected camera rename snapshot");
        };
        assert_eq!(snapshot.cameras[0].name, "camera_inline");
        assert_eq!(snapshot.undo_descriptions[0], "Rename to camera_inline");
        assert_eq!(
            model.named_camera_properties("camera_inline").unwrap().zoom,
            91.0
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().cameras[0].name, "camera_renamed");

        let undo_count = model.undo_stack.len();
        let response = handle_model_request(
            &mut model,
            ModelRequest::CopyWorkspaceCamera(WorkspaceCameraCopyInput {
                scene_uid: scene_uid.get(),
                name: "camera_renamed".to_string(),
            }),
        )
        .unwrap();
        let ModelResponse::Message(message) = response else {
            panic!("expected copy status message");
        };
        assert_eq!(message, "Copied camera camera_renamed");
        assert_eq!(model.undo_stack.len(), undo_count);
        handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceCamera(WorkspaceCameraPasteInput {
                scene_uid: scene_uid.get(),
            }),
        )
        .unwrap();
        assert!(model
            .snapshot()
            .cameras
            .iter()
            .any(|camera| camera.name == "copy1_camera_renamed"));
        let duplicate_rename = handle_model_request(
            &mut model,
            ModelRequest::RenameWorkspaceCamera(WorkspaceCameraRenameInput {
                scene_uid: scene_uid.get(),
                old_name: "camera_renamed".to_string(),
                new_name: "copy1_camera_renamed".to_string(),
            }),
        );
        assert_eq!(
            duplicate_rename.err().as_deref(),
            Some("camera already exists: copy1_camera_renamed")
        );
        assert_eq!(
            model.undo_stack.last().unwrap().description,
            "Pasted camera copy1_camera_renamed"
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().cameras.len(), 1);
        assert!(model.workspace_camera_clipboard.is_some());

        handle_model_request(
            &mut model,
            ModelRequest::DestroyCamera("camera_renamed".to_string()),
        )
        .unwrap();
        assert!(model.snapshot().cameras.is_empty());
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().cameras[0].name, "camera_renamed");
    }

    #[test]
    fn camera_file_commands_load_reload_save_and_undo() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-tauri-camera-file-{}.cam",
            std::process::id()
        ));
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        model.camera.set_zoom(63.0);
        handle_model_request(
            &mut model,
            ModelRequest::SaveCameraView("camera_0".to_string()),
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SaveCameraFile {
                scene_uid: scene_uid.get(),
                name: "camera_0".to_string(),
                path: path.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        assert!(path.is_file());
        assert_eq!(model.snapshot().cameras[0].source, path.to_string_lossy());

        model.camera.set_zoom(91.0);
        handle_model_request(
            &mut model,
            ModelRequest::SaveCameraView("camera_0".to_string()),
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::ReloadCameraFile {
                scene_uid: scene_uid.get(),
                name: "camera_0".to_string(),
            },
        )
        .unwrap();
        assert_eq!(
            model.named_camera_properties("camera_0").unwrap().zoom,
            63.0
        );

        handle_model_request(
            &mut model,
            ModelRequest::LoadCameraFile {
                scene_uid: scene_uid.get(),
                path: path.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        assert_eq!(model.camera.zoom(), 63.0);
        assert!(model
            .snapshot()
            .cameras
            .iter()
            .any(|camera| camera.name == "copy1_camera_0"));
        assert_eq!(
            model.undo_stack.last().unwrap().description,
            "Load camera file copy1_camera_0"
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().cameras.len(), 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn camera_visibility_commands_edit_clear_and_undo_flags() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        handle_model_request(&mut model, ModelRequest::SaveCamera("camera_0".to_string())).unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::GetCameraVisibility {
                scene_uid: scene_uid.get(),
                name: "camera_0".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::CameraVisibility(entries) = response else {
            panic!("expected camera visibility entries");
        };
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| {
            entry.target_uid == object_uid.get() && entry.is_object && entry.included
        }));

        model.camera.set_zoom(64.0);
        handle_model_request(
            &mut model,
            ModelRequest::SaveCameraView("camera_0".to_string()),
        )
        .unwrap();
        assert_eq!(model.snapshot().cameras[0].visibility_count, 2);
        model.desktop.set_object_visible(object_uid, false).unwrap();
        model.camera.set_zoom(10.0);
        let undo_count = model.undo_stack.len();
        handle_model_request(
            &mut model,
            ModelRequest::LoadCameraView("camera_0".to_string()),
        )
        .unwrap();
        assert_eq!(model.camera.zoom(), 64.0);
        assert!(!model.desktop.object_summaries(scene_uid)[0].visible);
        assert_eq!(model.undo_stack.len(), undo_count);

        handle_model_request(
            &mut model,
            ModelRequest::SetCameraVisibility(CameraVisibilityUpdateInput {
                scene_uid: scene_uid.get(),
                name: "camera_0".to_string(),
                entries: vec![CameraVisibilitySettingInput {
                    target_uid: renderer_uid.get(),
                    is_object: false,
                    visible: false,
                }],
            }),
        )
        .unwrap();
        assert_eq!(model.snapshot().cameras[0].visibility_count, 1);
        assert!(model.modified_scenes.contains(&scene_uid));

        handle_model_request(
            &mut model,
            ModelRequest::ClearCameraVisibility {
                scene_uid: scene_uid.get(),
                name: "camera_0".to_string(),
            },
        )
        .unwrap();
        assert_eq!(model.snapshot().cameras[0].visibility_count, 0);
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().cameras[0].visibility_count, 1);
    }

    #[test]
    fn style_workspace_commands_update_snapshot_and_support_undo() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        handle_model_request(&mut model, ModelRequest::CreateStyle("style_0".to_string())).unwrap();
        let snapshot = model.snapshot();
        let local = snapshot
            .styles
            .iter()
            .find(|style| style.local_index.is_some())
            .unwrap();
        assert_eq!(local.name, "style_0");
        assert_eq!(local.scene_uid, scene_uid.get());
        let local_index = local.local_index.unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::GetStyleProperties {
                scene_uid: scene_uid.get(),
                local_index,
            },
        )
        .unwrap();
        let ModelResponse::StyleProperties(properties) = response else {
            panic!("expected style properties");
        };
        assert!(properties.xml.contains("<styles"));
        handle_model_request(
            &mut model,
            ModelRequest::SetStyleProperties(StylePropertiesInput {
                scene_uid: scene_uid.get(),
                local_index,
                name: "custom".to_string(),
                read_only: true,
                xml: "<styles><style id=\"Wide\" width=\"3\"/></styles>".to_string(),
            }),
        )
        .unwrap();
        let local = model
            .snapshot()
            .styles
            .into_iter()
            .find(|style| style.local_index == Some(local_index))
            .unwrap();
        assert_eq!(local.name, "custom");
        assert!(local.read_only);

        let undo_count = model.undo_stack.len();
        let response = handle_model_request(
            &mut model,
            ModelRequest::CopyWorkspaceStyle(WorkspaceStyleCopyInput {
                scene_uid: scene_uid.get(),
                local_index,
            }),
        )
        .unwrap();
        let ModelResponse::Message(message) = response else {
            panic!("expected copy status message");
        };
        assert_eq!(message, "Copied style custom");
        assert_eq!(model.undo_stack.len(), undo_count);
        model
            .desktop
            .update_scene_style(
                scene_uid,
                local_index,
                "custom",
                false,
                "<styles><style id=\"Narrow\" width=\"1\"/></styles>",
            )
            .unwrap();
        assert!(handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceStyle(WorkspaceStylePasteInput {
                scene_uid: scene_uid.get(),
                replace: false,
            }),
        )
        .is_err());
        handle_model_request(
            &mut model,
            ModelRequest::PasteWorkspaceStyle(WorkspaceStylePasteInput {
                scene_uid: scene_uid.get(),
                replace: true,
            }),
        )
        .unwrap();
        assert!(model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index]
            .data
            .contains("Wide"));
        assert_eq!(
            model.undo_stack.last().unwrap().description,
            "Pasted style custom"
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index]
            .data
            .contains("Narrow"));
        assert!(model.workspace_style_clipboard.is_some());

        handle_model_request(
            &mut model,
            ModelRequest::DestroyStyle {
                scene_uid: scene_uid.get(),
                local_index,
            },
        )
        .unwrap();
        assert!(model
            .snapshot()
            .styles
            .iter()
            .all(|style| style.local_index.is_none()));
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model
            .snapshot()
            .styles
            .iter()
            .any(|style| style.name == "custom"));
    }

    #[test]
    fn style_file_commands_load_save_and_undo_linkage() {
        let input_path = std::env::temp_dir().join(format!(
            "cuemol-tauri-style-file-input-{}.xml",
            std::process::id()
        ));
        let output_path = std::env::temp_dir().join(format!(
            "cuemol-tauri-style-file-output-{}.xml",
            std::process::id()
        ));
        std::fs::write(
            &input_path,
            "<styles id=\"external\"><style id=\"Wide\" width=\"3\"/></styles>",
        )
        .unwrap();
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::LoadStyleFile {
                scene_uid: scene_uid.get(),
                path: input_path.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let loaded = model
            .snapshot()
            .styles
            .into_iter()
            .find(|style| style.name == "external")
            .unwrap();
        assert!(loaded.read_only);
        let local_index = loaded.local_index.unwrap();

        handle_model_request(
            &mut model,
            ModelRequest::SaveStyleFile {
                scene_uid: scene_uid.get(),
                local_index,
                path: output_path.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let saved = std::fs::read_to_string(&output_path).unwrap();
        assert!(saved.contains("id=\"Wide\""));
        assert!(!saved.contains("readonly="));
        assert_eq!(
            model
                .snapshot()
                .styles
                .into_iter()
                .find(|style| style.name == "external")
                .unwrap()
                .source,
            output_path.to_string_lossy()
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(
            model
                .snapshot()
                .styles
                .into_iter()
                .find(|style| style.name == "external")
                .unwrap()
                .source,
            input_path.to_string_lossy()
        );

        std::fs::write(
            &input_path,
            "<styles id=\"external\"><style id=\"Narrow\" width=\"1\"/></styles>",
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::ReloadStyleFile {
                scene_uid: scene_uid.get(),
                local_index,
            },
        )
        .unwrap();
        assert_eq!(
            model.status,
            format!("Reloaded style file {}", input_path.to_string_lossy())
        );
        assert!(model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index]
            .data
            .contains("Narrow"));
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index]
            .data
            .contains("Wide"));

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }

    #[test]
    fn renderer_context_style_creation_is_undoable_without_rebuilding_the_mesh() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let scene_uid = model.desktop.active_scene().unwrap();
        let local_index = model.desktop.create_scene_style(scene_uid, "user").unwrap();
        model
            .desktop
            .set_renderer_property(object_uid, renderer_uid, "width", "3.5")
            .unwrap();
        model.prepare_mesh_if_needed().unwrap();
        assert!(model.mesh.is_some());

        let response = handle_model_request(
            &mut model,
            ModelRequest::CreateRendererStyle(RendererStyleCreateInput {
                scene_uid: scene_uid.get(),
                local_index,
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                name_prefix: "Wide".to_string(),
                overwrite: false,
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.status, "Created style Widesimple");
        assert!(snapshot
            .styles
            .iter()
            .any(|style| style.local_index == Some(local_index) && style.modified));
        assert!(model.mesh.is_some());
        let style = &model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index];
        assert!(style.data.contains("id=\"Widesimple\""));
        assert!(style.data.contains("width=\"3.5\""));

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        let style = &model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index];
        assert!(!style.data.contains("id=\"Widesimple\""));

        let map_uid =
            model
                .desktop
                .add_density_map(scene_uid, "map", cuemol_xtal::DensityMap::new());
        let map_renderer_uid = model
            .desktop
            .add_density_renderer(map_uid, "contour")
            .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::CreateRendererStyle(RendererStyleCreateInput {
                scene_uid: scene_uid.get(),
                local_index,
                object_uid: map_uid.get(),
                renderer_uid: map_renderer_uid.get(),
                name_prefix: "Map".to_string(),
                overwrite: false,
            }),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected workspace snapshot");
        };
        assert_eq!(snapshot.status, "Created style Mapcontour");
        assert!(model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index]
            .data
            .contains("id=\"Mapcontour\""));
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(!model
            .desktop
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index]
            .data
            .contains("id=\"Mapcontour\""));
    }

    #[test]
    fn workspace_delete_removes_each_item_kind_and_supports_undo() {
        let (mut molecule_model, object_uid, renderer_uid) = model_with_renderer();
        handle_model_request(
            &mut molecule_model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "renderer".to_string(),
                object_uid: object_uid.get(),
                renderer_uid: Some(renderer_uid.get()),
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        assert!(molecule_model.snapshot().objects[0].renderers.is_empty());
        handle_model_request(&mut molecule_model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(molecule_model.snapshot().objects[0].renderers.len(), 1);
        handle_model_request(
            &mut molecule_model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "object".to_string(),
                object_uid: object_uid.get(),
                renderer_uid: None,
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        assert!(molecule_model.snapshot().objects.is_empty());
        handle_model_request(&mut molecule_model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(molecule_model.snapshot().objects.len(), 1);

        let mut density_model = model_without_gpu();
        let scene_uid = density_model.desktop.active_scene().unwrap();
        let mut map = cuemol_xtal::DensityMap::new();
        map.set_map_byte_array(&[0, 255], [2, 1, 1], 0.0, 1.0, 0.5, 0.5);
        map.set_map_params([0, 0, 0], [2, 1, 1]);
        let map_uid = density_model
            .desktop
            .add_density_map(scene_uid, "density", map);
        let density_renderer_uid = density_model
            .desktop
            .add_density_renderer(map_uid, "contour")
            .unwrap();
        handle_model_request(
            &mut density_model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "densityRenderer".to_string(),
                object_uid: map_uid.get(),
                renderer_uid: Some(density_renderer_uid.get()),
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        assert!(density_model.snapshot().density_maps[0]
            .renderers
            .is_empty());
        handle_model_request(&mut density_model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(density_model.snapshot().density_maps[0].renderers.len(), 1);
        handle_model_request(
            &mut density_model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "densityMap".to_string(),
                object_uid: map_uid.get(),
                renderer_uid: None,
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        assert!(density_model.snapshot().density_maps.is_empty());
        handle_model_request(&mut density_model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(density_model.snapshot().density_maps.len(), 1);

        let mut surface_model = model_without_gpu();
        let scene_uid = surface_model.desktop.active_scene().unwrap();
        let data = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let surface_uid = surface_model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_loaded_surface(cuemol_molstr::LoadedSurface {
                uid: Uid::new(0),
                name: "surface".to_string(),
                object_type: "MolSurfObj".to_string(),
                srctype: "obj".to_string(),
                data: data.to_string(),
                raw_data: data.as_bytes().to_vec(),
                qsc_node: None,
            });
        let surface_renderer_uid = surface_model.snapshot().surfaces[0].renderers[0].uid;
        handle_model_request(
            &mut surface_model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "surfaceRenderer".to_string(),
                object_uid: surface_uid.get(),
                renderer_uid: Some(surface_renderer_uid),
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        assert!(surface_model.snapshot().surfaces[0].renderers.is_empty());
        handle_model_request(&mut surface_model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(surface_model.snapshot().surfaces[0].renderers.len(), 1);
        handle_model_request(
            &mut surface_model,
            ModelRequest::DeleteWorkspaceItem(WorkspaceDeleteInput {
                kind: "surface".to_string(),
                object_uid: surface_uid.get(),
                renderer_uid: None,
                scene_uid: None,
                name: None,
                local_index: None,
            }),
        )
        .unwrap();
        assert!(surface_model.snapshot().surfaces.is_empty());
        handle_model_request(&mut surface_model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(surface_model.snapshot().surfaces.len(), 1);
    }

    #[test]
    fn workspace_zoom_fits_renderer_selection_object_and_density_map() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      2  CA  GLY A   2      10.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      3  CA  GLY B   1     100.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let object_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule_with_source_type("molecule", molecule, "pdb");
        let renderer_uid = {
            let object = model
                .desktop
                .scene_manager_mut()
                .object_mut(object_uid)
                .unwrap();
            let uid = object.add_renderer("simple", "chain A");
            object
                .renderer_mut(uid)
                .unwrap()
                .set_selection(cuemol_molstr::parse_selector("chain A").unwrap());
            uid
        };

        assert!(model
            .fit_workspace_item(
                WorkspaceFitInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(renderer_uid.get()),
                },
                800,
                400,
            )
            .unwrap());
        assert!((model.camera.center().x - 5.0).abs() < 1.0e-6);
        assert!((model.camera.zoom() - 7.0).abs() < 1.0e-6);

        assert!(model
            .fit_workspace_item(
                WorkspaceFitInput {
                    kind: "object".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: None,
                },
                800,
                400,
            )
            .unwrap());
        assert!((model.camera.center().x - 50.0).abs() < 1.0e-6);
        assert!((model.camera.zoom() - 70.0).abs() < 1.0e-6);

        let mut map = cuemol_xtal::DensityMap::new();
        map.set_map_byte_array(&[0, 255], [2, 1, 1], 0.0, 1.0, 0.5, 0.5);
        map.set_map_params([0, 0, 0], [2, 1, 1]);
        map.set_xtal_params(20.0, 10.0, 10.0, 90.0, 90.0, 90.0);
        let map_uid = model.desktop.add_density_map(scene_uid, "density", map);
        assert!(model
            .fit_workspace_item(
                WorkspaceFitInput {
                    kind: "densityMap".to_string(),
                    object_uid: map_uid.get(),
                    renderer_uid: None,
                },
                800,
                400,
            )
            .unwrap());
        assert!((model.camera.center().x - 10.0).abs() < 1.0e-6);
        assert!((model.camera.center().y - 5.0).abs() < 1.0e-6);
        assert!((model.camera.zoom() - 10.0).abs() < 1.0e-6);
    }

    #[test]
    fn molstruct_actions_select_center_and_zoom_selected_atoms() {
        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      2  CA  GLY A   2      10.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      3  CA  GLY B   1     100.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let object_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule_with_source_type("molecule", molecule, "pdb");
        let selection = "'A'.1:2.*";

        model
            .apply_molecule_selection_action(object_uid.get(), selection, "center", 800, 400)
            .unwrap();
        let object = model.desktop.scene_manager().object(object_uid).unwrap();
        assert_eq!(object.mol().selection_text(), selection);
        assert!(object
            .renderers()
            .iter()
            .any(|renderer| renderer.renderer_type() == "*selection"));
        assert_eq!(model.selection, selection);
        assert!((model.camera.center().x - 5.0).abs() < 1.0e-6);

        model
            .apply_molecule_selection_action(object_uid.get(), selection, "zoom", 800, 400)
            .unwrap();
        assert!((model.camera.center().x - 5.0).abs() < 1.0e-6);
        assert!((model.camera.zoom() - 7.0).abs() < 1.0e-6);
        assert!(
            model
                .apply_molecule_selection_action(
                    object_uid.get(),
                    "bad selector (",
                    "select",
                    800,
                    400,
                )
                .is_err()
        );
        assert!(model
            .apply_molecule_selection_action(object_uid.get(), "'B'.1.*", "invalid", 800, 400)
            .is_err());
        assert_eq!(model.selection, selection);
    }

    #[test]
    fn shared_core_populates_structure_sequence_and_coloring_panels() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.selection = "chain A and resid 1".to_string();
        model.rebuild_mesh(true).unwrap();
        let object_uid = model.desktop.active_object_summaries()[0].uid;
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetObjectColoring {
                object_uid: object_uid.get(),
                coloring: "rainbow".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        let details = model.molecule_details("", None, "", true, Some(object_uid.get()));
        assert_eq!(snapshot.structure_key, details.structure_key);
        assert_eq!(snapshot.sequence_key, details.sequence_key);
        let structure = details.structure.as_ref().unwrap();
        assert_eq!(details.structure_object_uid, Some(object_uid.get()));
        assert_eq!(structure[0].object_name, "1CRN");
        assert_eq!(structure[0].chains[0].chain_id, "A");
        let sequence = details.sequence.as_ref().unwrap();
        assert_eq!(sequence[0].residues[0].letter, "T");
        assert!(sequence[0].residues[0].selected);
        assert_eq!(
            snapshot.objects[0].coloring_type.as_deref(),
            Some("RainbowColoring")
        );
    }

    #[test]
    fn detailed_renderer_coloring_is_serialized_and_undoable() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetColoring {
                input: ColoringUpdateInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(renderer_uid.get()),
                    coloring_type: Some("PaintColoring".to_string()),
                    attrs: BTreeMap::new(),
                    paint_entries: vec![
                        PaintEntryDto {
                            selection: "sheet".to_string(),
                            color: "#4682b4".to_string(),
                        },
                        PaintEntryDto {
                            selection: "*".to_string(),
                            color: "#fffaf0".to_string(),
                        },
                    ],
                    gradient_entries: Vec::new(),
                    default_color: None,
                },
                width: 100,
                height: 100,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected a snapshot with viewport");
        };
        assert_eq!(update.viewport.width, 100);
        assert_eq!(update.viewport.height, 100);
        let snapshot = update.snapshot;
        let renderer = &snapshot.objects[0].renderers[0];
        assert_eq!(renderer.coloring_type.as_deref(), Some("PaintColoring"));
        assert_eq!(renderer.paint_entries[0].selection, "sheet");
        assert_eq!(renderer.paint_entries[0].color, "#4682b4");
        let renderer_state = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(
            renderer_state.qsc_children()[0].type_name(),
            "PaintColoring"
        );

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().objects[0].renderers[0]
            .coloring_type
            .is_none());
    }

    #[test]
    fn script_coloring_update_preserves_source_and_is_undoable() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let source = "return cuemol.makeColor(atom.element === \"C\" ? \"red\" : \"blue\");";
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetColoring {
                input: ColoringUpdateInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(renderer_uid.get()),
                    coloring_type: Some("ScriptColoring".to_string()),
                    attrs: BTreeMap::from([("script".to_string(), source.to_string())]),
                    paint_entries: Vec::new(),
                    gradient_entries: Vec::new(),
                    default_color: None,
                },
                width: 100,
                height: 100,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected a snapshot with viewport");
        };
        let renderer = &update.snapshot.objects[0].renderers[0];
        assert_eq!(renderer.coloring_type.as_deref(), Some("ScriptColoring"));
        assert_eq!(
            renderer.coloring_attrs.get("script").map(String::as_str),
            Some(source)
        );
        let renderer_state = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        let coloring = &renderer_state.qsc_children()[0];
        assert_eq!(coloring.type_name(), "ScriptColoring");
        assert_eq!(coloring.find_child("script").unwrap().contents(), source);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().objects[0].renderers[0]
            .coloring_type
            .is_none());
    }

    #[test]
    fn invalid_detailed_coloring_does_not_create_undo_data() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let result = handle_model_request(
            &mut model,
            ModelRequest::SetColoring {
                input: ColoringUpdateInput {
                    kind: "renderer".to_string(),
                    object_uid: object_uid.get(),
                    renderer_uid: Some(renderer_uid.get()),
                    coloring_type: Some("CPKColoring".to_string()),
                    attrs: BTreeMap::from([("col_C".to_string(), "not-a-color".to_string())]),
                    paint_entries: Vec::new(),
                    gradient_entries: Vec::new(),
                    default_color: None,
                },
                width: 100,
                height: 100,
            },
        );
        assert!(result.is_err());
        assert!(model.undo_stack.is_empty());
        assert!(model.snapshot().objects[0].renderers[0]
            .coloring_type
            .is_none());
    }

    #[test]
    fn edit_menu_atom_deletion_is_undoable() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let object_uid = model.desktop.active_object_summaries()[0].uid;
        handle_model_request(
            &mut model,
            ModelRequest::DeleteAtoms {
                object_uid: object_uid.get(),
                selection: "aid 1".to_string(),
            },
        )
        .unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.objects[0].atom_count, 326);
        assert_eq!(snapshot.undo_descriptions[0], "Deleted 1 atom(s)");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().objects[0].atom_count, 327);
    }

    #[test]
    fn viewport_click_selects_the_projected_atom() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let object_uid = model.desktop.active_object_summaries()[0].uid;
        model.selection = "aid 1".to_string();
        let mvp = camera_mvp(&model.camera, 800.0 / 600.0);
        let labels = model
            .desktop
            .active_atom_labels(&model.selection, mvp, 800.0, 600.0, 1);
        assert_eq!(labels.len(), 1);
        model.selection.clear();
        model.rebuild_mesh(false).unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::PickAtomIdentifier {
                x: f64::from(labels[0].x),
                y: f64::from(labels[0].y),
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::PickedAtom(Some(atom)) = response else {
            panic!("expected a lightweight picked-atom response");
        };
        assert_eq!(atom.object_uid, object_uid.get());
        assert_eq!(atom.aid, 1);
        assert!(model.selection.is_empty());
        let response = handle_model_request(
            &mut model,
            ModelRequest::PickAtom {
                x: f64::from(labels[0].x),
                y: f64::from(labels[0].y),
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        let ModelResponse::MolStructUpdate(update) = response else {
            panic!("expected a compact selection and viewport response");
        };
        assert_eq!(update.snapshot.selection, "aid 1");
        assert_eq!(update.viewport.width, 800);
        assert_eq!(update.viewport.height, 600);
        assert_eq!(model.snapshot().selection, "aid 1");
    }

    #[test]
    fn animation_ribbon_controls_the_shared_timeline() {
        let mut model = model_without_gpu();
        let before = model.camera.rotation();
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "simple-spin".to_string(),
                after_index: None,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::AnimationControl("play".to_string()),
        )
        .unwrap();
        handle_model_request(&mut model, ModelRequest::TickAnimation(0.1)).unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.animation_objects.len(), 1);
        assert_eq!(snapshot.animation_duration, 1.0);
        assert!(snapshot.animation_elapsed > 0.0);
        assert!(snapshot.animation_playing);
        assert_ne!(model.camera.rotation(), before);
        let response = handle_model_request(
            &mut model,
            ModelRequest::TickAnimationFrame {
                dt: 0.1,
                width: 320,
                height: 240,
            },
        )
        .unwrap();
        let ModelResponse::AnimationFrame(frame) = response else {
            panic!("expected a compact animation frame");
        };
        assert!(frame.animation_elapsed > snapshot.animation_elapsed);
        assert_eq!(frame.animation_duration, 1.0);
        assert!(frame.animation_playing);
        assert_eq!(frame.viewport.width, 320);
        handle_model_request(
            &mut model,
            ModelRequest::AnimationControl("stop".to_string()),
        )
        .unwrap();
        assert_eq!(model.snapshot().animation_elapsed, 0.0);
        assert_eq!(model.camera.rotation(), before);

        handle_model_request(
            &mut model,
            ModelRequest::AnimationControl("play".to_string()),
        )
        .unwrap();
        for _ in 0..4 {
            handle_model_request(&mut model, ModelRequest::TickAnimation(0.25)).unwrap();
        }
        assert!(!model.animation_playing);
        assert_eq!(model.animation_elapsed, model.animation_duration());
        handle_model_request(
            &mut model,
            ModelRequest::AnimationControl("stop".to_string()),
        )
        .unwrap();
        assert_eq!(model.camera.rotation(), before);
        assert_eq!(
            model
                .desktop
                .active_view_camera()
                .expect("active view camera")
                .rotation(),
            before
        );
    }

    #[test]
    fn animation_seek_applies_typed_properties_and_stop_restores_them() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        let original = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .expect("object exists")
            .renderer(renderer_uid)
            .expect("renderer exists")
            .clone();
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "hide".to_string(),
                after_index: None,
            },
        )
        .unwrap();
        let mut input = animation_properties_input(&model.snapshot().animation_objects[0]);
        input.renderer_names = Some("molecule/simple 1".to_string());
        input.hide = Some(true);
        input.fade = Some(true);
        input.target_alpha = Some(0.8);
        handle_model_request(
            &mut model,
            ModelRequest::SetAnimationObjectProperties(input),
        )
        .unwrap();

        handle_model_request(&mut model, ModelRequest::SeekAnimation(0.5)).unwrap();
        let animated = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .expect("animated object exists")
            .renderer(renderer_uid)
            .expect("animated renderer exists");
        assert!(animated.is_visible());
        assert_eq!(animated.qsc_attr("alpha"), Some("0.4"));
        assert!(model.animation_start_desktop.is_some());
        assert!(!model.animation_playing);

        handle_model_request(
            &mut model,
            ModelRequest::AnimationControl("stop".to_string()),
        )
        .unwrap();
        let restored = model
            .desktop
            .scene_manager()
            .object(object_uid)
            .expect("restored object exists")
            .renderer(renderer_uid)
            .expect("restored renderer exists");
        assert_eq!(restored.is_visible(), original.is_visible());
        assert_eq!(restored.qsc_attr("alpha"), original.qsc_attr("alpha"));
        assert!(model.animation_start_desktop.is_none());
        assert_eq!(model.animation_elapsed, 0.0);
    }

    #[test]
    fn edits_while_animation_is_paused_are_not_lost_on_stop() {
        let (mut model, object_uid, renderer_uid) = model_with_renderer();
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "hide".to_string(),
                after_index: None,
            },
        )
        .unwrap();
        handle_model_request(&mut model, ModelRequest::SeekAnimation(0.5)).unwrap();
        assert!(model.animation_start_desktop.is_some());

        handle_model_request(
            &mut model,
            ModelRequest::SetRendererVisible {
                object_uid: object_uid.get(),
                renderer_uid: renderer_uid.get(),
                visible: false,
            },
        )
        .unwrap();
        assert!(model.animation_start_desktop.is_none());
        assert!(!model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap()
            .is_visible());

        handle_model_request(
            &mut model,
            ModelRequest::AnimationControl("stop".to_string()),
        )
        .unwrap();
        assert!(!model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap()
            .is_visible());

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model
            .desktop
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap()
            .is_visible());
    }

    #[test]
    fn animation_timeline_is_scene_specific_and_qsc_backed() {
        let mut model = model_without_gpu();
        let first_scene = model.desktop.active_scene().unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "simple-spin".to_string(),
                after_index: None,
            },
        )
        .unwrap();
        let first = model.desktop.scene_animation(first_scene).unwrap().unwrap();
        assert_eq!(first.size(), 1);
        assert!(matches!(first.motions()[0], AnimMotion::Spin(_)));
        assert_eq!(first.startcam(), "__current");

        handle_model_request(
            &mut model,
            ModelRequest::NewScene(Some("second".to_string())),
        )
        .unwrap();
        let second_scene = model.desktop.active_scene().unwrap();
        assert_ne!(first_scene, second_scene);
        assert!(model.snapshot().animation_objects.is_empty());
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "hide".to_string(),
                after_index: None,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SetAnimationDuration {
                index: 0,
                duration: 2.5,
            },
        )
        .unwrap();
        handle_model_request(&mut model, ModelRequest::SetAnimationLoop(true)).unwrap();
        let second = model
            .desktop
            .scene_animation(second_scene)
            .unwrap()
            .unwrap();
        assert_eq!(second.size(), 1);
        assert!(matches!(
            second.motions()[0],
            AnimMotion::ShowHide(ref value) if value.hide()
        ));
        assert!(second.loop_enabled());
        assert_eq!(animation_seconds(second.length()), 2.5);

        handle_model_request(&mut model, ModelRequest::ActivateScene(first_scene.get())).unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.animation_objects.len(), 1);
        assert!(snapshot.animation_objects[0].name.contains("SimpleSpin"));
        assert_eq!(snapshot.animation_duration, 1.0);
        assert!(!snapshot.animation_loop);
        assert_eq!(snapshot.animation_start_camera, "__current");
        let path = std::env::temp_dir().join(format!(
            "cuemol-tauri-animation-scene-{}.qsc",
            std::process::id()
        ));
        handle_model_request(
            &mut model,
            ModelRequest::SaveScene {
                path: path.to_string_lossy().into_owned(),
                options: None,
            },
        )
        .unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("<animation"));
        assert!(saved.contains("type=\"SimpleSpin\""));
        assert!(saved.contains("startcam=\"__current\""));
        let mut reloaded = model_without_gpu();
        reloaded
            .open_scene(path.to_string_lossy().into_owned())
            .unwrap();
        let reloaded_snapshot = reloaded.snapshot();
        assert_eq!(reloaded_snapshot.animation_objects.len(), 1);
        assert_eq!(reloaded_snapshot.animation_duration, 1.0);
        assert_eq!(reloaded_snapshot.animation_start_camera, "__current");
        fs::remove_file(path).unwrap();

        let mut first_input = animation_test_input(first_scene.get(), Path::new("."));
        first_input.frame_rate = 10.0;
        let mut second_input = animation_test_input(second_scene.get(), Path::new("."));
        second_input.frame_rate = 10.0;
        assert_eq!(
            model.animation_render_frame_count(&first_input).unwrap(),
            10
        );
        assert_eq!(
            model.animation_render_frame_count(&second_input).unwrap(),
            25
        );
    }

    #[test]
    fn animation_start_camera_and_edits_follow_scene_undo() {
        let mut model = model_without_gpu();
        model.save_named_camera("overview", false).unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "noop".to_string(),
                after_index: None,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SetAnimationStartCamera("overview".to_string()),
        )
        .unwrap();
        assert_eq!(model.snapshot().animation_start_camera, "overview");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().animation_start_camera, "__current");
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().animation_objects.is_empty());
    }

    #[test]
    fn animation_add_inserts_after_the_selected_row() {
        let mut model = model_without_gpu();
        for kind in ["noop", "simple-spin"] {
            handle_model_request(
                &mut model,
                ModelRequest::AddAnimationObject {
                    kind: kind.to_string(),
                    after_index: None,
                },
            )
            .unwrap();
        }
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "hide".to_string(),
                after_index: Some(0),
            },
        )
        .unwrap();

        let objects = model.snapshot().animation_objects;
        assert_eq!(
            objects
                .iter()
                .map(|object| object.type_name.as_str())
                .collect::<Vec<_>>(),
            ["NoopAnimObj", "ShowHideAnim", "SimpleSpin"]
        );
        assert_eq!(objects[1].time_ref_name, objects[0].name);

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(
            model
                .snapshot()
                .animation_objects
                .iter()
                .map(|object| object.type_name.as_str())
                .collect::<Vec<_>>(),
            ["NoopAnimObj", "SimpleSpin"]
        );
    }

    #[test]
    fn configured_animation_add_commits_properties_in_one_undo_step() {
        let mut model = model_without_gpu();
        let motion = new_animation_motion(AnimKind::Spin, "SimpleSpin1".to_string(), String::new());
        let mut properties = animation_properties_input(&animation_object_dto(0, &motion));
        properties.name = "custom-spin".to_string();
        properties.duration = 1.0;
        properties.angle = Some(90.0);

        handle_model_request(
            &mut model,
            ModelRequest::AddConfiguredAnimationObject {
                kind: "simple-spin".to_string(),
                after_index: None,
                properties,
            },
        )
        .unwrap();

        let snapshot = model.snapshot();
        assert_eq!(snapshot.animation_objects.len(), 1);
        assert_eq!(snapshot.animation_objects[0].name, "custom-spin");
        assert_eq!(snapshot.animation_objects[0].angle, Some(90.0));
        assert_eq!(model.undo_stack.len(), 1);
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert!(model.snapshot().animation_objects.is_empty());

        let motion = new_animation_motion(AnimKind::Spin, "SimpleSpin1".to_string(), String::new());
        let properties = animation_properties_input(&animation_object_dto(0, &motion));
        assert!(handle_model_request(
            &mut model,
            ModelRequest::AddConfiguredAnimationObject {
                kind: "simple-spin".to_string(),
                after_index: None,
                properties,
            },
        )
        .is_err());
        assert!(model.snapshot().animation_objects.is_empty());
        assert!(model.undo_stack.is_empty());
    }

    #[test]
    fn animation_multiple_selection_moves_and_deletes_in_one_undo_step() {
        let mut model = model_without_gpu();
        for kind in ["noop", "simple-spin", "hide", "slide-in"] {
            handle_model_request(
                &mut model,
                ModelRequest::AddAnimationObject {
                    kind: kind.to_string(),
                    after_index: None,
                },
            )
            .unwrap();
        }

        handle_model_request(
            &mut model,
            ModelRequest::MoveAnimationObjects {
                indices: vec![1, 2],
                direction: -1,
            },
        )
        .unwrap();
        assert_eq!(
            model
                .snapshot()
                .animation_objects
                .into_iter()
                .map(|object| object.type_name)
                .collect::<Vec<_>>(),
            [
                "SimpleSpin",
                "ShowHideAnim",
                "NoopAnimObj",
                "SlideInOutAnim"
            ]
        );

        handle_model_request(
            &mut model,
            ModelRequest::MoveAnimationObjects {
                indices: vec![0, 1],
                direction: 1,
            },
        )
        .unwrap();
        assert_eq!(
            model
                .snapshot()
                .animation_objects
                .into_iter()
                .map(|object| object.type_name)
                .collect::<Vec<_>>(),
            [
                "NoopAnimObj",
                "SimpleSpin",
                "ShowHideAnim",
                "SlideInOutAnim"
            ]
        );

        handle_model_request(&mut model, ModelRequest::DeleteAnimationObjects(vec![1, 2])).unwrap();
        assert_eq!(
            model
                .snapshot()
                .animation_objects
                .into_iter()
                .map(|object| object.type_name)
                .collect::<Vec<_>>(),
            ["NoopAnimObj", "SlideInOutAnim"]
        );
        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        assert_eq!(model.snapshot().animation_objects.len(), 4);
    }

    #[test]
    fn animation_object_properties_update_typed_values_and_reference_names() {
        let mut model = model_without_gpu();
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "simple-spin".to_string(),
                after_index: None,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject {
                kind: "noop".to_string(),
                after_index: None,
            },
        )
        .unwrap();
        let snapshot = model.snapshot();
        let mut input = animation_properties_input(&snapshot.animation_objects[0]);
        input.name = "turn".to_string();
        input.disabled = true;
        input.start = 0.25;
        input.duration = 2.5;
        input.quadric = 0.2;
        input.angle = Some(180.0);
        input.axis = Some([1.0, 2.0, 3.0]);
        handle_model_request(
            &mut model,
            ModelRequest::SetAnimationObjectProperties(input),
        )
        .unwrap();

        let snapshot = model.snapshot();
        let spin = &snapshot.animation_objects[0];
        assert_eq!(spin.name, "turn");
        assert_eq!(spin.type_name, "SimpleSpin");
        assert!(spin.disabled);
        assert_eq!(spin.relative_start, 0.25);
        assert_eq!(spin.duration, 2.5);
        assert_eq!(spin.quadric, 0.2);
        assert_eq!(spin.angle, Some(180.0));
        assert_eq!(snapshot.animation_objects[1].time_ref_name, "turn");
        assert_eq!(snapshot.animation_duration, 3.75);
        let axis = spin.axis.unwrap();
        assert!((axis[0] - 1.0 / 14.0_f64.sqrt()).abs() < 1.0e-9);

        let undo_count = model.undo_stack.len();
        let mut cyclic = animation_properties_input(&snapshot.animation_objects[0]);
        cyclic.time_ref_name = snapshot.animation_objects[1].name.clone();
        assert!(handle_model_request(
            &mut model,
            ModelRequest::SetAnimationObjectProperties(cyclic),
        )
        .is_err());
        assert_eq!(model.undo_stack.len(), undo_count);
        assert_eq!(model.snapshot().animation_objects[0].time_ref_name, "");

        handle_model_request(&mut model, ModelRequest::Undo(0)).unwrap();
        let restored = model.snapshot();
        assert_eq!(restored.animation_objects[0].name, "SimpleSpin1");
        assert_eq!(restored.animation_objects[1].time_ref_name, "SimpleSpin1");
    }

    #[test]
    fn trajectory_panel_snapshot_and_seek_use_shared_frames() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        let object = model.desktop.active_object_summaries()[0].clone();
        let atom_count = object.atom_count;
        model.desktop.set_object_trajectory(
            object.uid,
            vec![
                vec![Vector4::new3(0.0, 0.0, 0.0); atom_count],
                vec![Vector4::new3(9.0, 8.0, 7.0); atom_count],
            ],
        );

        let snapshot = model.snapshot();
        assert_eq!(snapshot.trajectories.len(), 1);
        assert_eq!(snapshot.trajectories[0].frame_count, 2);
        assert_eq!(snapshot.trajectories[0].current_frame, 0);
        assert_eq!(snapshot.trajectories[0].blocks.len(), 1);
        assert_eq!(snapshot.trajectories[0].blocks[0].start_index, 0);
        handle_model_request(
            &mut model,
            ModelRequest::SetTrajectoryFrame {
                object_uid: object.uid.get(),
                frame: 1,
            },
        )
        .unwrap();
        assert_eq!(model.snapshot().trajectories[0].current_frame, 1);
        assert_eq!(
            model
                .desktop
                .scene_manager()
                .object(object.uid)
                .unwrap()
                .mol()
                .atoms()[0]
                .position,
            Vector4::new3(9.0, 8.0, 7.0)
        );
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetTrajectoryFrameViewport {
                object_uid: object.uid.get(),
                frame: 0,
                width: 320,
                height: 240,
            },
        )
        .unwrap();
        let ModelResponse::TrajectoryFrame(frame) = response else {
            panic!("expected a compact trajectory frame");
        };
        assert_eq!(frame.object_uid, object.uid.get());
        assert_eq!(frame.frame, 0);
        assert_eq!(frame.viewport.height, 240);
        assert_eq!(model.snapshot().trajectories[0].current_frame, 0);
    }

    #[test]
    fn analysis_dialog_requests_call_the_shared_tools() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let objects = model.desktop.active_object_summaries();
        let first = objects[0].uid.get();
        let second = objects[1].uid.get();
        let bond_count = model
            .desktop
            .nonstandard_bond_summaries(Uid::new(first))
            .unwrap()
            .len();

        handle_model_request(
            &mut model,
            ModelRequest::SuperposeObjects {
                algorithm: "lsq".to_string(),
                reference_object_uid: first,
                reference_selection: "name CA".to_string(),
                moving_object_uid: second,
                moving_selection: "name CA".to_string(),
                auto_recenter: true,
                use_property: true,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::EditBond {
                object_uid: first,
                aid1: 1,
                aid2: 327,
                make: true,
            },
        )
        .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::GetNonstandardBonds { object_uid: first },
        )
        .unwrap();
        let ModelResponse::Message(bonds) = response else {
            panic!("expected non-standard bond JSON");
        };
        assert!(bonds.contains("\"aid\":1"));
        assert!(bonds.contains("\"aid\":327"));
        assert_eq!(
            model
                .desktop
                .nonstandard_bond_summaries(Uid::new(first))
                .unwrap()
                .len(),
            bond_count + 1
        );
        let response = handle_model_request(
            &mut model,
            ModelRequest::RemoveNonstandardBonds {
                object_uid: first,
                pairs: vec![[1, 327]],
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected snapshot after bond removal");
        };
        assert_eq!(snapshot.status, "Removed 1 non-standard bond(s)");
        assert_eq!(
            model
                .desktop
                .nonstandard_bond_summaries(Uid::new(first))
                .unwrap()
                .len(),
            bond_count
        );
        let response = handle_model_request(
            &mut model,
            ModelRequest::CalculateContacts {
                object_uid: first,
                selection1: "name CA".to_string(),
                object2_uid: Some(second),
                selection2: Some("name CA".to_string()),
                r_min: 0.0,
                r_max: 0.1,
                hbond: false,
                max_count: 2,
                renderer_name: "measure".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.status, "Defined 2 interaction(s) in measure");
        let renderer = model
            .desktop
            .scene_manager()
            .object(Uid::new(first))
            .unwrap()
            .renderers()
            .iter()
            .find(|renderer| renderer.renderer_type() == "atomintr")
            .expect("interaction renderer");
        assert_eq!(renderer.atom_intr_lines().len(), 2);
        assert_eq!(renderer.atom_intr_lines()[0].positions().len(), 2);
        let response = handle_model_request(
            &mut model,
            ModelRequest::GetSpaceGroups {
                lattice: "CUBIC".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::Message(space_groups) = response else {
            panic!("expected space-group JSON");
        };
        assert!(space_groups.contains("\"id\":195"));
        let response = handle_model_request(
            &mut model,
            ModelRequest::ChangeCrystalInfo {
                object_uid: first,
                a: 20.0,
                b: 20.0,
                c: 20.0,
                alpha: 90.0,
                beta: 90.0,
                gamma: 90.0,
                space_group_number: 195,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.status, "Changed symmetry information");
        assert_eq!(snapshot.crystals[0].space_group_number, Some(195));
        assert_eq!(snapshot.crystals[0].lattice, "CUBIC");
        let response = handle_model_request(
            &mut model,
            ModelRequest::AssignSecondaryStructure {
                object_uid: first,
                ignore_bulge: false,
                angle: 0.0,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(
            snapshot.status,
            "Secondary structure assigned for 327 atom(s)"
        );
        assert!(snapshot.can_undo);
    }

    #[test]
    fn surface_apbs_and_morph_dialog_requests_write_and_update_data() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let output_dir =
            std::env::temp_dir().join(format!("cuemol-tauri-tools-{}", std::process::id()));
        std::fs::create_dir_all(&output_dir).unwrap();
        let surface_path = output_dir.join("surface.obj");
        let cut_path = output_dir.join("cut.obj");
        let pqr_path = output_dir.join("apbs.pqr");
        let input_path = output_dir.join("apbs.in");
        let dx_path = output_dir.join("apbs.dx");

        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let objects = model.desktop.active_object_summaries();
        let first = objects[0].uid.get();
        let second = objects[1].uid.get();

        let response = handle_model_request(
            &mut model,
            ModelRequest::CreateMolecularSurface {
                object_uid: first,
                selection: "aid 1".to_string(),
                name: "surface".to_string(),
                density: 1.0,
                probe_radius: 1.4,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected molecular-surface snapshot");
        };
        assert_eq!(snapshot.status, "Created molecular surface surface");
        assert_eq!(snapshot.surfaces.len(), 1);
        assert_eq!(snapshot.surfaces[0].original_molecule, objects[0].name);
        let surface_uid = Uid::new(snapshot.surfaces[0].uid);
        model.camera.set_center(Vector4::new3(0.0, 0.0, -120.0));
        model.camera.set_slab_depth(240.0);
        let response = handle_model_request(
            &mut model,
            ModelRequest::CutSurface {
                surface_uid: surface_uid.get(),
                density: 1.0,
                mode: "separate".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected surface-cut snapshot");
        };
        assert_eq!(snapshot.surfaces.len(), 2);
        assert!(snapshot
            .surfaces
            .iter()
            .any(|item| item.name == "sect1_surface"));
        assert!(snapshot.status.starts_with("Surface cut:"));

        handle_model_request(
            &mut model,
            ModelRequest::WriteSurface {
                object_uid: first,
                selection: "aid 1".to_string(),
                density: 0.5,
                probe_radius: 1.4,
                path: surface_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(surface_path.exists());
        handle_model_request(
            &mut model,
            ModelRequest::WriteCutSurface {
                object_uid: first,
                selection: "aid 1".to_string(),
                density: 0.5,
                probe_radius: 1.4,
                normal: [0.0, 0.0, 1.0],
                position: [0.0, 0.0, 0.0],
                path: cut_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(cut_path.exists());
        handle_model_request(
            &mut model,
            ModelRequest::WriteApbsFiles {
                object_uid: first,
                selection: "name CA".to_string(),
                pqr_path: pqr_path.to_string_lossy().to_string(),
                input_path: input_path.to_string_lossy().to_string(),
                dx_path: dx_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(pqr_path.exists() && input_path.exists());

        for (source, name) in [(first, "start"), (second, "end")] {
            handle_model_request(
                &mut model,
                ModelRequest::AppendMorphFrame {
                    target_object_uid: first,
                    source_object_uid: source,
                    frame_name: name.to_string(),
                },
            )
            .unwrap();
        }
        handle_model_request(
            &mut model,
            ModelRequest::ApplyMorphFrame {
                object_uid: first,
                frame: 0.5,
            },
        )
        .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::RemoveMorphFrame {
                object_uid: first,
                index: 1,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.morphs.len(), 1);
        assert_eq!(snapshot.morphs[0].frames.len(), 1);

        for path in [surface_path, cut_path, pqr_path, input_path] {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(output_dir);
    }

    #[cfg(unix)]
    #[test]
    fn apbs_job_request_runs_and_loads_electrostatic_map() {
        use std::os::unix::fs::PermissionsExt;

        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      2  CB  ALA A   1       2.000   3.000   4.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let object_uid = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("source", molecule);
        let script =
            std::env::temp_dir().join(format!("cuemol-tauri-apbs-test-{}.sh", std::process::id()));
        fs::write(
            &script,
            "#!/bin/sh\nbase=$(awk '/write pot dx/{print $4}' \"$1\")\n\
             printf 'object 1 class gridpositions counts 1 1 2\\norigin 0 0 0\\ndelta 1 0 0\\ndelta 0 1 0\\ndelta 0 0 1\\nobject 2 class gridconnections counts 1 1 2\\nobject 3 class array type double rank 0 items 2 data follows\\n-1 1\\n' > \"${base}.dx\"\nprintf 'complete\\n'\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::StartApbsCalculation(ApbsCalculationInput {
                object_uid: object_uid.get(),
                selection: String::new(),
                potential_name: "pot_source".to_string(),
                charge_method: "internal".to_string(),
                force_field: "charmm".to_string(),
                use_hydrogen: false,
                use_npbe: false,
                temperature: 298.15,
                grid_spacing: 1.0,
                water_dielectric: 78.54,
                protein_dielectric: 2.0,
                apbs_executable: script.to_string_lossy().into_owned(),
                pdb2pqr_executable: String::new(),
            }),
        )
        .unwrap();
        assert!(matches!(response, ModelResponse::Message(ref value) if value == "running"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let completed = loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let response =
                handle_model_request(&mut model, ModelRequest::PollApbsCalculation).unwrap();
            let ModelResponse::Message(state) = response else {
                panic!("expected APBS job state");
            };
            if let Some(uid) = state.strip_prefix("complete:") {
                break uid.parse::<u64>().unwrap();
            }
            assert_eq!(state, "running");
            assert!(
                std::time::Instant::now() < deadline,
                "APBS worker timed out"
            );
        };
        let map = model
            .desktop
            .density_map_summary(Uid::new(completed))
            .unwrap();
        assert_eq!(map.name, "pot_source");
        assert_eq!(map.class_name, "ElePotMap");
        assert_eq!(map.renderers[0].renderer_type, "*unitcell");
        assert!(model.apbs_job.is_none());
        fs::remove_file(script).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn pov_render_request_runs_previews_and_saves_image() {
        use std::os::unix::fs::PermissionsExt;

        let mut model = model_without_gpu();
        let scene_uid = model.desktop.active_scene().unwrap();
        let view_uid = model.desktop.active_view_summary().unwrap().uid;
        let mut target_camera = Camera::new();
        target_camera.set_center(Vector4::new3(4.0, 5.0, 6.0));
        target_camera.set_cam_dist(12.0);
        model
            .desktop
            .set_active_view_camera(&target_camera)
            .unwrap();
        let active_scene = model.desktop.new_scene("other", true).result_scene;
        model.camera = Camera::new();
        let script =
            std::env::temp_dir().join(format!("cuemol-tauri-pov-test-{}.sh", std::process::id()));
        let saved =
            std::env::temp_dir().join(format!("cuemol-tauri-pov-test-{}.png", std::process::id()));
        fs::write(
            &script,
            "#!/bin/sh\nout=''\nfor arg in \"$@\"; do\n  case \"$arg\" in +O*) out=${arg#\"+O\"};; esac\ndone\nprintf 'PNG' > \"$out\"\nprintf 'render complete\\n'\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::StartPovRender(PovRenderInput {
                scene_uid: scene_uid.get(),
                view_uid: view_uid.get(),
                width: 800,
                height: 600,
                threads: 4,
                perspective: true,
                stereo: -1,
                stereo_depth: 0.05,
                transparent: true,
                clip_plane: true,
                post_blend: true,
                edge_lines: true,
                pixel_labels: false,
                executable: script.to_string_lossy().into_owned(),
                include_dir: String::new(),
                radiosity: 7,
                shadow: true,
                default_lighting: true,
                light_spread: 1,
                light_intensity: 1.3,
                flash_fraction: 0.8 / 1.3,
                ambient_fraction: 0.0,
            }),
        )
        .unwrap();
        assert!(matches!(response, ModelResponse::Message(ref value) if value == "running"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let preview = loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let ModelResponse::Message(state) =
                handle_model_request(&mut model, ModelRequest::PollPovRender).unwrap()
            else {
                panic!("expected POV-Ray job state");
            };
            if let Some(preview) = state.strip_prefix("complete:") {
                break preview.to_string();
            }
            assert_eq!(state, "running");
            assert!(
                std::time::Instant::now() < deadline,
                "POV-Ray worker timed out"
            );
        };
        assert_eq!(preview, "data:image/png;base64,UE5H");
        assert_eq!(model.desktop.active_scene(), Some(active_scene));
        let pov = fs::read_to_string(
            model
                .pov_render_result
                .as_ref()
                .unwrap()
                .work_dir
                .join("render.pov"),
        )
        .unwrap();
        assert!(pov.contains("#declare _camera_pos = <4.000000,5.000000,18.000000>"));
        assert!(model
            .log_lines
            .iter()
            .any(|line| line.contains("POV-Ray> render complete")));
        let response = handle_model_request(
            &mut model,
            ModelRequest::SavePovRenderImage {
                path: saved.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        assert!(matches!(response, ModelResponse::Message(ref value) if value == "saved"));
        assert_eq!(fs::read(&saved).unwrap(), b"PNG");
        let work_dir = model.pov_render_result.as_ref().unwrap().work_dir.clone();
        let response =
            handle_model_request(&mut model, ModelRequest::ClearPovRenderResult).unwrap();
        assert!(matches!(response, ModelResponse::Message(ref value) if value == "cleared"));
        assert!(model.pov_render_result.is_none());
        assert!(!work_dir.exists());
        fs::remove_file(saved).unwrap();
        fs::remove_file(script).unwrap();
    }

    #[test]
    fn pov_render_poll_composites_pixel_labels_before_preview() {
        let mut model = model_without_gpu();
        let work_dir = create_pov_work_dir().unwrap();
        let image_path = work_dir.join("render.png");
        image::RgbaImage::from_pixel(96, 48, image::Rgba([255, 255, 255, 255]))
            .save(&image_path)
            .unwrap();
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Ok(PovProcessOutput {
                output_path: image_path.clone(),
                log: String::new(),
            }))
            .unwrap();
        model.pov_render_job = Some(TauriPovRenderJob {
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            work_dir: work_dir.clone(),
            labels: vec![GuiAtomLabel {
                x: 8.0,
                y: 24.0,
                text: "CA".to_string(),
                color: "#0000ff".to_string(),
                font_size: 18.0,
                font_name: "sans-serif".to_string(),
                font_style: "normal".to_string(),
                font_weight: "normal".to_string(),
            }],
        });
        let response = handle_model_request(&mut model, ModelRequest::PollPovRender).unwrap();
        assert!(matches!(
            response,
            ModelResponse::Message(ref value) if value.starts_with("complete:data:image/png;base64,")
        ));
        let frame = image::open(&image_path).unwrap().to_rgba8();
        assert!(frame.pixels().any(|pixel| pixel.0 != [255, 255, 255, 255]));
        model.pov_render_result.take();
        fs::remove_dir_all(work_dir).unwrap();
    }

    #[test]
    fn execute_script_file_reads_and_runs_internal_javascript() {
        let script_path = std::env::temp_dir().join(format!(
            "cuemol-tauri-script-file-{}.js",
            std::process::id()
        ));
        std::fs::write(&script_path, "print('file-script');").unwrap();
        let mut model = model_without_gpu();
        let response = handle_model_request(
            &mut model,
            ModelRequest::ExecuteScriptFile(script_path.to_string_lossy().into_owned()),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.log_lines[0], "file-script");
        assert_eq!(
            snapshot.log_lines[1],
            format!("Exec internal JS: {} done.", script_path.to_string_lossy())
        );
        assert_eq!(snapshot.status, "Script executed");
        let _ = std::fs::remove_file(script_path);
    }

    #[test]
    fn rendering_and_script_dialog_requests_produce_outputs() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let pov_path =
            std::env::temp_dir().join(format!("cuemol-tauri-pov-{}.pov", std::process::id()));
        let script_path =
            std::env::temp_dir().join(format!("cuemol-tauri-script-{}.js", std::process::id()));
        std::fs::write(&script_path, "print('tauri-script');").unwrap();
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::ExecuteScriptFile(script_path.to_string_lossy().into_owned()),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.log_lines[0], "tauri-script");
        assert_eq!(
            snapshot.log_lines[1],
            format!("Exec internal JS: {} done.", script_path.to_string_lossy())
        );
        handle_model_request(
            &mut model,
            ModelRequest::ExportPov {
                path: pov_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(std::fs::read_to_string(&pov_path)
            .unwrap()
            .contains("CueMol POV-Ray scene"));
        let response = handle_model_request(&mut model, ModelRequest::ClearLog).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert!(snapshot.log_lines.is_empty());
        let _ = std::fs::remove_file(script_path);
        let _ = std::fs::remove_file(pov_path);
    }

    #[test]
    fn file_dialog_requests_merge_save_and_restore_views() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let output_dir =
            std::env::temp_dir().join(format!("cuemol-tauri-files-{}", std::process::id()));
        std::fs::create_dir_all(&output_dir).unwrap();
        let object_path = output_dir.join("merged.pdb");
        let view_path = output_dir.join("current.view");
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let objects = model.desktop.active_object_summaries();
        let source = objects[0].uid.get();
        let target = objects[1].uid.get();
        handle_model_request(
            &mut model,
            ModelRequest::ChangeChain {
                object_uid: source,
                selection: "name CA".to_string(),
                chain_name: "B".to_string(),
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::MergeMolecules {
                source_object_uid: source,
                selection: "chain B".to_string(),
                target_object_uid: target,
                copy: false,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SaveObject {
                object_uid: target,
                object_kind: "object".to_string(),
                path: object_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(std::fs::read_to_string(&object_path)
            .unwrap()
            .contains("ATOM"));

        model.camera.set_center(Vector4::new3(4.0, 5.0, 6.0));
        handle_model_request(
            &mut model,
            ModelRequest::SaveView(view_path.to_string_lossy().to_string()),
        )
        .unwrap();
        model.camera.set_center(Vector4::ZERO);
        handle_model_request(
            &mut model,
            ModelRequest::LoadView(view_path.to_string_lossy().to_string()),
        )
        .unwrap();
        assert_eq!(model.camera.center(), Vector4::new3(4.0, 5.0, 6.0));

        for path in [object_path, view_path] {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(output_dir);
    }

    fn animation_test_input(scene_uid: u64, output_dir: &Path) -> AnimationRenderInput {
        AnimationRenderInput {
            scene_uid,
            output_dir: output_dir.to_string_lossy().into_owned(),
            base_name: "movie".to_string(),
            width: 800,
            height: 600,
            frame_rate: 24.0,
            perspective: false,
            duplicate_last_frame: false,
            concurrency: 3,
            backend: "povray".to_string(),
            povray_executable: "povray".to_string(),
            povray_include_dir: String::new(),
            pov_clip_plane: true,
            pov_post_blend: true,
            pov_shadow: true,
            pov_edge_lines: true,
            pov_labels: true,
            pov_radiosity: 3,
            make_movie: false,
            ffmpeg_executable: "ffmpeg".to_string(),
            movie_format: "mp4_h264".to_string(),
            bitrate_kbps: 2_048,
        }
    }

    #[test]
    fn animation_render_paths_and_pov_options_match_reference() {
        let input = animation_test_input(1, Path::new("frames"));
        assert_eq!(
            tauri_animation_frame_path(&input, 7, "png"),
            PathBuf::from("frames/movie_frm_0007.png")
        );
        let task = tauri_animation_pov_process_task(
            &input,
            Path::new("frames/movie_frm_0007.pov"),
            Path::new("frames/movie_frm_0007.png"),
        );
        let args = crate::pov_job::povray_args(&task);
        for expected in [
            "+Iframes/movie_frm_0007.pov",
            "+Oframes/movie_frm_0007.png",
            "+W800",
            "+H600",
            "+WT1",
            "Declare=_perspective=0",
            "Declare=_shadow=1",
            "Declare=_radiosity=3",
            "-V",
        ] {
            assert!(
                args.iter().any(|argument| argument == expected),
                "{expected}"
            );
        }
    }

    #[test]
    fn animation_render_preview_reads_reference_frame_name() {
        let output_dir = std::env::temp_dir().join(format!(
            "cuemol-tauri-animation-preview-{}",
            std::process::id()
        ));
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("movie_frm_0007.png"), b"PNG").unwrap();
        let mut model = model_without_gpu();
        let response = handle_model_request(
            &mut model,
            ModelRequest::AnimationRenderPreview {
                output_dir: output_dir.to_string_lossy().into_owned(),
                base_name: "movie".to_string(),
                frame_index: 7,
            },
        )
        .unwrap();
        assert!(matches!(
            response,
            ModelResponse::Message(ref value) if value == "data:image/png;base64,UE5H"
        ));
        fs::remove_dir_all(output_dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn animation_movie_encoder_uses_reference_frame_pattern() {
        use std::os::unix::fs::PermissionsExt;

        let output_dir = std::env::temp_dir().join(format!(
            "cuemol-tauri-animation-movie-{}",
            std::process::id()
        ));
        fs::create_dir_all(&output_dir).unwrap();
        for index in 0..2 {
            fs::write(output_dir.join(format!("movie_frm_{index:04}.png")), b"PNG").unwrap();
        }
        let executable = output_dir.join("fake-ffmpeg.sh");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > ffmpeg-args.txt\nlast=''\nfor arg in \"$@\"; do last=\"$arg\"; done\nprintf movie > \"$last\"\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
        let mut input = animation_test_input(1, &output_dir);
        input.make_movie = true;
        input.ffmpeg_executable = executable.to_string_lossy().into_owned();
        let output = encode_tauri_animation_movie(&TauriAnimationMovieTask {
            input,
            total_frames: 2,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(AtomicU32::new(0)),
        })
        .unwrap();
        assert!(matches!(
            output,
            TauriAnimationTaskOutput::Movie { ref path, .. }
                if path == &output_dir.join("movie.mp4")
        ));
        let args = fs::read_to_string(output_dir.join("ffmpeg-args.txt")).unwrap();
        assert!(args.contains("movie_frm_%04d.png"));
        assert!(args.contains("2048k"));
        assert!(args.contains("movie.mp4"));
        fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn animation_render_frame_count_matches_reference_loop_semantics() {
        let mut model = model_without_gpu();
        add_test_animation(&mut model, AnimKind::Spin, 2.0);
        let mut input = AnimationRenderInput {
            scene_uid: model.desktop.active_scene().unwrap().get(),
            output_dir: ".".to_string(),
            base_name: "frame".to_string(),
            width: 640,
            height: 480,
            frame_rate: 30.0,
            perspective: true,
            duplicate_last_frame: false,
            concurrency: 2,
            backend: "umbreon".to_string(),
            povray_executable: "povray".to_string(),
            povray_include_dir: String::new(),
            pov_clip_plane: true,
            pov_post_blend: true,
            pov_shadow: false,
            pov_edge_lines: true,
            pov_labels: false,
            pov_radiosity: -1,
            make_movie: false,
            ffmpeg_executable: "ffmpeg".to_string(),
            movie_format: "mov_h264".to_string(),
            bitrate_kbps: 10_240,
        };
        assert_eq!(model.animation_render_frame_count(&input).unwrap(), 60);
        input.duplicate_last_frame = true;
        assert_eq!(model.animation_render_frame_count(&input).unwrap(), 61);
        input.base_name = "../frame".to_string();
        assert!(model.animation_render_frame_count(&input).is_err());
    }

    #[test]
    fn animation_render_cancel_restores_live_state_before_first_frame() {
        let output_dir = std::env::temp_dir().join(format!(
            "cuemol-tauri-animation-cancel-{}",
            std::process::id()
        ));
        let mut model = model_without_gpu();
        add_test_animation(&mut model, AnimKind::Spin, 1.0);
        model.camera.set_center(Vector4::new3(3.0, 4.0, 5.0));
        model.camera.set_perspective(true);
        model.animation_elapsed = 0.5;
        let scene_uid = model.desktop.active_scene().unwrap().get();
        let input = AnimationRenderInput {
            scene_uid,
            output_dir: output_dir.to_string_lossy().into_owned(),
            base_name: "frame".to_string(),
            width: 640,
            height: 480,
            frame_rate: 30.0,
            perspective: false,
            duplicate_last_frame: false,
            concurrency: 2,
            backend: "umbreon".to_string(),
            povray_executable: "povray".to_string(),
            povray_include_dir: String::new(),
            pov_clip_plane: true,
            pov_post_blend: true,
            pov_shadow: false,
            pov_edge_lines: true,
            pov_labels: false,
            pov_radiosity: -1,
            make_movie: false,
            ffmpeg_executable: "ffmpeg".to_string(),
            movie_format: "mov_h264".to_string(),
            bitrate_kbps: 10_240,
        };
        handle_model_request(&mut model, ModelRequest::StartAnimationRender(input)).unwrap();
        assert_eq!(model.animation_elapsed, 0.0);
        assert!(!model.camera.is_perspective());
        handle_model_request(&mut model, ModelRequest::CancelAnimationRender).unwrap();
        let response = handle_model_request(&mut model, ModelRequest::PollAnimationRender).unwrap();
        assert!(matches!(
            response,
            ModelResponse::Message(ref state)
                if state == "error:Animation rendering cancelled"
        ));
        assert!(model.animation_render_job.is_none());
        assert_eq!(model.animation_elapsed, 0.5);
        assert_eq!(model.camera.center(), Vector4::new3(3.0, 4.0, 5.0));
        assert!(model.camera.is_perspective());
        let _ = std::fs::remove_dir(output_dir);
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn animation_rendering_writes_numbered_png_frames() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let output_dir =
            std::env::temp_dir().join(format!("cuemol-tauri-animation-{}", std::process::id()));
        let mut model = AppModel::new(vec![pdb]);
        assert!(model.mesh.is_none());
        let viewport = model.render(64, 64);
        assert!(model.renderer.is_some());
        assert!(viewport.image.is_some());
        add_test_animation(&mut model, AnimKind::Spin, 1.0);
        let scene_uid = model.desktop.active_scene().unwrap().get();
        let response = handle_model_request(
            &mut model,
            ModelRequest::StartAnimationRender(AnimationRenderInput {
                scene_uid,
                output_dir: output_dir.to_string_lossy().to_string(),
                base_name: "frame".to_string(),
                width: 100,
                height: 100,
                frame_rate: 2.0,
                perspective: true,
                duplicate_last_frame: false,
                concurrency: 2,
                backend: "umbreon".to_string(),
                povray_executable: "povray".to_string(),
                povray_include_dir: String::new(),
                pov_clip_plane: true,
                pov_post_blend: true,
                pov_shadow: false,
                pov_edge_lines: true,
                pov_labels: false,
                pov_radiosity: -1,
                make_movie: false,
                ffmpeg_executable: "ffmpeg".to_string(),
                movie_format: "mov_h264".to_string(),
                bitrate_kbps: 10_240,
            }),
        )
        .unwrap();
        assert!(matches!(
            response,
            ModelResponse::Message(ref state) if state.starts_with("running:0:2:")
        ));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let response =
                handle_model_request(&mut model, ModelRequest::PollAnimationRender).unwrap();
            if matches!(
                response,
                ModelResponse::Message(ref state) if state == "complete:2"
            ) {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let snapshot = model.snapshot();
        assert_eq!(snapshot.status, "Animation rendered: 2 frame(s)");
        let first = std::fs::read(output_dir.join("frame_frm_0000.png")).unwrap();
        assert_eq!(&first[..8], b"\x89PNG\r\n\x1a\n");
        let _ = std::fs::remove_file(output_dir.join("frame_frm_0000.png"));
        let _ = std::fs::remove_file(output_dir.join("frame_frm_0001.png"));
        let _ = std::fs::remove_dir(output_dir);
    }
}
