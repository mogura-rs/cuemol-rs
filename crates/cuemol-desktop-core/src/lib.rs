use cuemol_anim::{MorphFrame, MorphMol};
use cuemol_core::{CueMolError, Matrix4, Result, Uid};
use cuemol_mdtools::{read_amber_prmtop_file, read_gro_file, read_psf_file};
use cuemol_molanl::MolAnlManager;
use cuemol_molstr::{
    parse_selector, read_mol2_file, write_pdb_string, write_sdf_string, LoadObjectResult,
    MolCommandManager, MolCoord, MolObject, MolRenderer, MolSceneManager, NewRendererResult,
    NewSceneResult, QscLoadStatus, Selector, StreamManager,
};
use cuemol_molvis::{
    anisou_mesh_snapshot, apply_renderer_styles, default_style_manager, disorder_lines_snapshot,
    molecule_render_snapshot, na_base_sticks_snapshot, ribbon_mesh_snapshot, MoleculeRenderOptions,
    DEFAULT_STYLE_SCOPE,
};
use cuemol_qsys::{ScalarObject, StyleManager};
use cuemol_render::PovSceneExporter;
use cuemol_render::RenderSceneSnapshot;
use cuemol_surface::{ApbsInputConfig, MolSurfObj, PqrFileWriter};
use cuemol_xtal::{
    read_brix_map_file, read_ccp4_map_file, read_opendx_file, read_xplor_map_file, MapMeshRenderer,
    MapSurfRenderer,
};
// Re-exported so consumers (and tests) can name the scalar-field types the surface
// potential API takes — e.g. `set_surface_potential_map(DensityMap)`.
pub use cuemol_xtal::{DensityMap, GridMap};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

// Offscreen wgpu mesh renderer (verifiable headless; the wgpu 3D component every
// front end composites). Behind the `offscreen` feature so the default build is
// GPU-free; GUI crates that show a 3D view enable it.
#[cfg(feature = "offscreen")]
pub mod offscreen;

#[derive(Debug, Clone)]
pub struct DesktopAppState {
    command_manager: MolCommandManager,
    stream_manager: StreamManager,
    config: cuemol_qsys::SysConfig,
    active_scene: Option<Uid>,
    last_qsc_status: Option<QscLoadStatus>,
    morphs: BTreeMap<Uid, MorphMol>,
    density_maps: BTreeMap<Uid, GuiDensityMapObject>,
    next_density_map_uid: u64,
    /// Per-object MD trajectory frames (each frame = atom positions in molecule order),
    /// for the MolAnim animation type. Empty until a trajectory is loaded.
    trajectories: BTreeMap<Uid, Vec<Vec<cuemol_core::Vector4>>>,
    /// Electrostatic-potential scalar field for potential-coloured surfaces (an
    /// APBS/OpenDX `.dx` grid, held as a `DensityMap` scalar carrier). `None` until
    /// a potential is loaded. Kept separate from `density_maps` so the density
    /// contour/isosurface features never pick it up. See
    /// [`build_surface_potential_mesh`](DesktopAppState::build_surface_potential_mesh).
    surface_potential: Option<DensityMap>,
    /// The ± half-width (in potential units) mapping to full red/blue for the SCAPOT
    /// surface (the C++ `m_dParLow/High` span around the neutral mid). `None` = the
    /// robust auto default (2·rmsd), which — unlike the field min/max — is not washed
    /// out by near-atom potential spikes. `Some(s)` clamps the gradient to `mid ± s`.
    potential_scale: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiSceneSummary {
    pub uid: Uid,
    pub name: String,
    pub object_count: usize,
    pub map_count: usize,
    pub camera_count: usize,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiObjectSummary {
    pub uid: Uid,
    pub name: String,
    pub atom_count: usize,
    pub residue_count: usize,
    pub bond_count: usize,
    pub renderer_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiRendererSummary {
    pub uid: Uid,
    pub name: String,
    pub renderer_type: String,
    pub visible: bool,
    pub group: Option<String>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiAtomSummary {
    pub object_uid: Uid,
    pub atom_index: usize,
    pub serial: i32,
    pub name: String,
    pub residue_name: String,
    pub chain_id: String,
    pub residue_id: i32,
    pub selection_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiSequenceSummary {
    pub object_uid: Uid,
    pub object_name: String,
    pub chain_id: String,
    pub residue_count: usize,
    pub atom_count: usize,
    pub sequence: String,
}

/// One atom leaf in the MolStruct hierarchy tree (CueMol2 `molStructTree`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiStructAtom {
    pub atom_index: usize,
    pub serial: i32,
    pub name: String,
    pub element: String,
}

/// One residue node (holds its atoms) in the MolStruct hierarchy tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiStructResidue {
    pub res_name: String,
    pub res_id: i32,
    pub ins_code: Option<char>,
    pub atoms: Vec<GuiStructAtom>,
}

/// One chain node (holds its residues) in the MolStruct hierarchy tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiStructChain {
    pub chain_id: String,
    pub residues: Vec<GuiStructResidue>,
}

/// A molecule's full chain → residue → atom hierarchy for the MolStruct side panel
/// (CueMol2 `molstruct-panel.xul` `molStructTree`). Built from the molecule's own
/// `chain_summaries()` + `residue_summaries()`; shared by all three front ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiStructObject {
    pub object_uid: Uid,
    pub object_name: String,
    pub chains: Vec<GuiStructChain>,
}

/// Animation-object kind (a desktop-side subset of CueMol2's anim objects): a
/// `SimpleSpin`, a back-and-forth rock, or a hold. Shared across all three front ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimKind {
    Spin,
    Rock,
    Pause,
    SlideIn,
    SlideOut,
    ShowAnim,
    HideAnim,
    MolAnim,
}

impl AnimKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Spin => "Spin",
            Self::Rock => "Rock",
            Self::Pause => "Pause",
            Self::SlideIn => "SlideIn",
            Self::SlideOut => "SlideOut",
            Self::ShowAnim => "Show",
            Self::HideAnim => "Hide",
            Self::MolAnim => "MolAnim",
        }
    }
}

/// One animation object in the timeline (CueMol2 `anim-panel` row): a kind + duration.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimObject {
    pub kind: AnimKind,
    pub duration: f64,
}

/// The per-frame camera action for the active animation object: a rotation direction
/// (`+1`/`-1`/`0` for Spin / Rock-by-phase / hold) and a slide direction (`-1` zoom in
/// for SlideIn, `+1` zoom out for SlideOut, `0` otherwise). Each front end multiplies
/// these by its own per-frame angle / zoom step, so the timeline stays shared while the
/// camera API stays front-end-specific.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimFrame {
    pub elapsed: f64,
    pub rotate_dir: f64,
    pub slide_dir: f64,
    /// Target object visibility for Show/Hide objects (`None` = leave unchanged). The
    /// front end applies it only when the current visibility differs (so it re-renders
    /// once on entry, not every frame).
    pub set_visible: Option<bool>,
    /// Trajectory playback progress 0..1 for a MolAnim object (`None` otherwise). The
    /// front end maps it to a trajectory frame via `apply_trajectory_fraction`.
    pub frame_fraction: Option<f64>,
}

/// Advance an animation timeline by `dt` seconds (looping at the total duration) and
/// return the active object's per-frame camera action. An empty list spins forever.
pub fn anim_step(objects: &[AnimObject], elapsed: f64, dt: f64) -> AnimFrame {
    if objects.is_empty() {
        return AnimFrame {
            elapsed: 0.0,
            rotate_dir: 1.0,
            slide_dir: 0.0,
            set_visible: None,
            frame_fraction: None,
        };
    }
    let total: f64 = objects.iter().map(|object| object.duration.max(0.01)).sum();
    let mut next = elapsed + dt;
    if next >= total {
        next %= total;
    }
    let mut acc = 0.0;
    for object in objects {
        let d = object.duration.max(0.01);
        if next < acc + d {
            let progress = (next - acc) / d; // 0..1 within this segment
            let (rotate_dir, slide_dir, set_visible, frame_fraction) = match object.kind {
                AnimKind::Spin => (1.0, 0.0, None, None),
                AnimKind::Rock => (if progress < 0.5 { 1.0 } else { -1.0 }, 0.0, None, None),
                AnimKind::Pause => (0.0, 0.0, None, None),
                AnimKind::SlideIn => (0.0, -1.0, None, None),
                AnimKind::SlideOut => (0.0, 1.0, None, None),
                AnimKind::ShowAnim => (0.0, 0.0, Some(true), None),
                AnimKind::HideAnim => (0.0, 0.0, Some(false), None),
                AnimKind::MolAnim => (0.0, 0.0, None, Some(progress)),
            };
            return AnimFrame {
                elapsed: next,
                rotate_dir,
                slide_dir,
                set_visible,
                frame_fraction,
            };
        }
        acc += d;
    }
    AnimFrame {
        elapsed: next,
        rotate_dir: 0.0,
        slide_dir: 0.0,
        set_visible: None,
        frame_fraction: None,
    }
}

/// A projected atom label for the 3D view (CueMol2 atom labels): the `text` to draw at
/// image-pixel `(x, y)`. Front ends overlay these over the composited viewport image.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiAtomLabel {
    pub x: f32,
    pub y: f32,
    pub text: String,
}

/// One residue cell in the interactive sequence viewer (CueMol2 `seqpanel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiSeqResidue {
    /// One-letter code (20 AA + nucleotides; `X` otherwise).
    pub letter: String,
    pub res_id: i32,
    pub res_name: String,
    /// Selection expression that picks just this residue (for a click handler).
    pub selection: String,
    /// Whether any of this residue's atoms is in the current selection.
    pub selected: bool,
}

/// One chain row in the interactive sequence viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiSeqChain {
    pub object_uid: Uid,
    pub object_name: String,
    pub chain_id: String,
    pub residues: Vec<GuiSeqResidue>,
}

/// Crystal cell / space-group readout for the Symmetry side panel
/// (CueMol2 `symmetry-panel.xul`).
#[derive(Debug, Clone, PartialEq)]
pub struct GuiCrystalSummary {
    pub object_uid: Uid,
    pub object_name: String,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub space_group_name: String,
    pub space_group_number: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiSurfaceSummary {
    pub object_uid: Uid,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub cut_plane_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiApbsSummary {
    pub object_uid: Uid,
    pub atom_count: usize,
    pub pqr_path: String,
    pub input_path: String,
    pub dx_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiMorphSummary {
    pub object_uid: Uid,
    pub frame_count: usize,
    pub atom_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiDensityMapSummary {
    pub uid: Uid,
    pub scene_uid: Uid,
    pub name: String,
    pub format: String,
    pub counts: [usize; 3],
    pub value_count: usize,
    pub min_density: f64,
    pub max_density: f64,
    pub mean_density: f64,
    pub rmsd_density: f64,
    pub center: [f64; 3],
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct GuiDensityMapObject {
    uid: Uid,
    scene_uid: Uid,
    name: String,
    format: String,
    map: DensityMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiConfigEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiLoadedPath {
    Scene { scene_uid: Uid },
    Object { scene_uid: Uid, object_uid: Uid },
    Map { scene_uid: Uid, map_uid: Uid },
}

impl Default for DesktopAppState {
    fn default() -> Self {
        Self {
            command_manager: MolCommandManager::new(),
            stream_manager: StreamManager::new(),
            config: default_desktop_config(),
            active_scene: None,
            last_qsc_status: None,
            morphs: BTreeMap::new(),
            density_maps: BTreeMap::new(),
            next_density_map_uid: 1,
            trajectories: BTreeMap::new(),
            surface_potential: None,
            potential_scale: None,
        }
    }
}

impl DesktopAppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scene_manager(&self) -> &MolSceneManager {
        self.command_manager.scene_manager()
    }

    pub fn scene_manager_mut(&mut self) -> &mut MolSceneManager {
        self.command_manager.scene_manager_mut()
    }

    pub fn command_manager(&self) -> &MolCommandManager {
        &self.command_manager
    }

    pub fn command_manager_mut(&mut self) -> &mut MolCommandManager {
        &mut self.command_manager
    }

    pub fn stream_manager(&self) -> &StreamManager {
        &self.stream_manager
    }

    pub fn config_value(&self, key: &str) -> String {
        self.config.get(key).to_string()
    }

    pub fn put_config_value(&mut self, key: &str, value: impl Into<String>) {
        self.config.put(key, value);
    }

    pub fn config_entries(&self) -> Vec<GuiConfigEntry> {
        let mut entries = Vec::new();
        collect_config_entries("", self.config.root(), &mut entries);
        entries
    }

    pub fn save_config_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let output = self
            .config_entries()
            .into_iter()
            .map(|entry| format!("{}\t{}", entry.key, entry.value.replace('\n', "\\n")))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, output)?;
        Ok(())
    }

    pub fn load_config_file(&mut self, path: impl AsRef<Path>) -> Result<usize> {
        let input = fs::read_to_string(path)?;
        let mut count = 0;
        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, value)) = trimmed.split_once('\t') else {
                return Err(CueMolError::parse(format!(
                    "invalid config line: {trimmed}"
                )));
            };
            self.put_config_value(key.trim(), value.replace("\\n", "\n"));
            count += 1;
        }
        Ok(count)
    }

    pub fn active_scene(&self) -> Option<Uid> {
        self.active_scene
    }

    pub fn set_active_scene(&mut self, value: Option<Uid>) -> bool {
        if value.is_some_and(|uid| self.scene_manager().scene(uid).is_none()) {
            return false;
        }
        self.active_scene = value;
        true
    }

    pub fn last_qsc_status(&self) -> Option<&QscLoadStatus> {
        self.last_qsc_status.as_ref()
    }

    pub fn scene_summaries(&self) -> Vec<GuiSceneSummary> {
        self.scene_manager()
            .scenes()
            .iter()
            .map(|scene| GuiSceneSummary {
                uid: scene.uid(),
                name: scene.name().to_string(),
                object_count: scene.objects().len(),
                map_count: self
                    .density_maps
                    .values()
                    .filter(|map| map.scene_uid == scene.uid())
                    .count(),
                camera_count: scene.cameras().len(),
                active: Some(scene.uid()) == self.active_scene,
            })
            .collect()
    }

    pub fn active_scene_summary(&self) -> Option<GuiSceneSummary> {
        self.scene_summaries()
            .into_iter()
            .find(|scene| Some(scene.uid) == self.active_scene)
    }

    pub fn object_summaries(&self, scene_uid: Uid) -> Vec<GuiObjectSummary> {
        self.scene_manager()
            .scene(scene_uid)
            .map(|scene| {
                scene
                    .objects()
                    .iter()
                    .map(|object| GuiObjectSummary {
                        uid: object.uid(),
                        name: object.name().to_string(),
                        atom_count: object.mol().atom_count(),
                        residue_count: object.mol().residue_count(),
                        bond_count: object.mol().bond_count(),
                        renderer_count: object.renderers().len(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn density_map_summary(&self, map_uid: Uid) -> Option<GuiDensityMapSummary> {
        self.density_maps.get(&map_uid).map(density_map_summary)
    }

    pub fn density_map_summaries(&self, scene_uid: Uid) -> Vec<GuiDensityMapSummary> {
        self.density_maps
            .values()
            .filter(|map| map.scene_uid == scene_uid)
            .map(density_map_summary)
            .collect()
    }

    pub fn active_density_map_summaries(&self) -> Vec<GuiDensityMapSummary> {
        self.active_scene
            .map(|scene_uid| self.density_map_summaries(scene_uid))
            .unwrap_or_default()
    }

    pub fn active_object_summaries(&self) -> Vec<GuiObjectSummary> {
        self.active_scene
            .map(|scene_uid| self.object_summaries(scene_uid))
            .unwrap_or_default()
    }

    pub fn renderer_summaries(&self, object_uid: Uid) -> Vec<GuiRendererSummary> {
        self.scene_manager()
            .object(object_uid)
            .map(|object| {
                object
                    .renderers()
                    .iter()
                    .map(|renderer| GuiRendererSummary {
                        uid: renderer.uid(),
                        name: renderer.name().to_string(),
                        renderer_type: renderer.renderer_type().to_string(),
                        visible: renderer.is_visible(),
                        group: renderer.group().map(str::to_string),
                        style: renderer.style().map(str::to_string),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn atom_summary(&self, object_uid: Uid, atom_index: usize) -> Result<GuiAtomSummary> {
        let object = self
            .scene_manager()
            .object(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let atom = object.mol().atoms().get(atom_index).ok_or_else(|| {
            CueMolError::NotFound(format!("atom {atom_index} in object {}", object_uid.get()))
        })?;
        Ok(GuiAtomSummary {
            object_uid,
            atom_index,
            serial: atom.serial,
            name: atom.name.clone(),
            residue_name: atom.res_name.clone(),
            chain_id: atom.chain_id.clone(),
            residue_id: atom.res_id,
            selection_text: format!("aid {}", atom.serial),
        })
    }

    /// Crystal cell / space group for a molecule object, if the file carried one
    /// (CueMol2 Symmetry panel). Returns `None` for non-crystallographic molecules.
    pub fn crystal_summary(&self, object_uid: Uid) -> Option<GuiCrystalSummary> {
        let object = self.scene_manager().object(object_uid)?;
        let cell = object.mol().cell_params()?;
        Some(GuiCrystalSummary {
            object_uid,
            object_name: object.name().to_string(),
            a: cell.a,
            b: cell.b,
            c: cell.c,
            alpha: cell.alpha,
            beta: cell.beta,
            gamma: cell.gamma,
            space_group_name: cell.space_group_name.clone(),
            space_group_number: cell.space_group_number,
        })
    }

    /// Crystal summaries for every molecule object in the active scene that has a cell.
    pub fn active_crystal_summaries(&self) -> Vec<GuiCrystalSummary> {
        self.active_object_summaries()
            .into_iter()
            .filter_map(|object| self.crystal_summary(object.uid))
            .collect()
    }

    /// The Cartesian unit-cell box edges for every crystallographic object in the
    /// active scene (CueMol2 Symmetry panel "Unit cell"). A front end draws these as
    /// line segments around the molecule; empty for non-crystallographic scenes.
    pub fn active_unit_cell_edges(&self) -> Vec<(cuemol_core::Vector4, cuemol_core::Vector4)> {
        let Some(scene_uid) = self.active_scene else {
            return Vec::new();
        };
        let Some(scene) = self.scene_manager().scene(scene_uid) else {
            return Vec::new();
        };
        let mut edges = Vec::new();
        for object in scene.objects() {
            if let Some(cell) = object.mol().cell_params() {
                if let Some(crystal) = cuemol_symm::CrystalInfo::from_cell_params(cell) {
                    edges.extend(cuemol_symm::unit_cell_edges(&crystal));
                }
            }
        }
        edges
    }

    /// Append the crystallographic unit-cell box and/or home-cell symmetry-mate copies
    /// to an already-built render snapshot (CueMol2 Symmetry "Unit cell" / "Symm mol").
    /// Shared by all three front ends so crystal packing renders identically. Symmetry
    /// mates are added first (they clone the bare molecule); then the yellow cell box.
    pub fn append_crystal_geometry(
        &self,
        snapshot: &mut RenderSceneSnapshot,
        show_unit_cell: bool,
        show_symm_mates: bool,
    ) {
        if show_symm_mates {
            let transforms = self.active_symmetry_mate_transforms();
            if !transforms.is_empty() {
                let base = snapshot.clone();
                for matrix in transforms {
                    let mut mate = base.clone();
                    mate.apply_xform(&matrix);
                    merge_bounds(&mut snapshot.bounds, mate.bounds);
                    snapshot.atoms.extend(mate.atoms);
                    snapshot.bonds.extend(mate.bonds);
                    snapshot.meshes.extend(mate.meshes);
                }
            }
        }
        if show_unit_cell {
            let edges = self.active_unit_cell_edges();
            if !edges.is_empty() {
                let cell_color = cuemol_gfx::ColorSpec {
                    base: cuemol_gfx::ColorBase::Html {
                        r: 210,
                        g: 210,
                        b: 110,
                        a: 255,
                    },
                    modifiers: Vec::new(),
                };
                for (from, to) in edges {
                    // Each endpoint as a degenerate point-box so min/max grow correctly
                    // regardless of the edge's component ordering.
                    merge_bounds(&mut snapshot.bounds, Some((from, from)));
                    merge_bounds(&mut snapshot.bounds, Some((to, to)));
                    snapshot.bonds.push(cuemol_render::RenderBond {
                        from,
                        to,
                        radius: 0.12,
                        color: Some(cell_color.clone()),
                        object_uid: None,
                        atom_indices: None,
                        selected: false,
                    });
                }
            }
        }
    }

    /// Append electron-density "chickenwire" contour lines to a render snapshot
    /// (CueMol2 `MapMeshRenderer`): for every density map in the active scene, contour
    /// `density == siglevel·σ` into wireframe segments and push them as thin cyan
    /// line-bonds. Wires the otherwise-orphaned `generate_contour_lines` into the GUI
    /// render path (the unit-cell-edge pattern). No-op unless `show`.
    pub fn append_density_geometry(
        &self,
        snapshot: &mut RenderSceneSnapshot,
        siglevel: f64,
        show: bool,
    ) {
        if !show {
            return;
        }
        let Some(scene_uid) = self.active_scene else {
            return;
        };
        let contour_color = cuemol_gfx::ColorSpec {
            base: cuemol_gfx::ColorBase::Html {
                r: 90,
                g: 200,
                b: 255,
                a: 255,
            },
            modifiers: Vec::new(),
        };
        for map_object in self
            .density_maps
            .values()
            .filter(|object| object.scene_uid == scene_uid)
        {
            let mut renderer = MapMeshRenderer::new();
            renderer.map_mut().set_siglevel(siglevel);
            for segment in renderer.generate_contour_lines(&map_object.map) {
                merge_bounds(&mut snapshot.bounds, Some((segment[0], segment[0])));
                merge_bounds(&mut snapshot.bounds, Some((segment[1], segment[1])));
                snapshot.bonds.push(cuemol_render::RenderBond {
                    from: segment[0],
                    to: segment[1],
                    radius: 0.04,
                    color: Some(contour_color.clone()),
                    object_uid: None,
                    atom_indices: None,
                    selected: false,
                });
            }
        }
    }

    /// Build the FILLED density isosurface mesh for every map in the active scene at the
    /// given sigma level (CueMol2 `MapSurfRenderer` — the solid-surface density display,
    /// the second mode alongside the chickenwire contour). Wires the orphaned
    /// `generate_surf_obj` into the GUI: marching the map's own grid → a `RenderMesh`
    /// (the same MolSurfObj→mesh conversion as the molecular surface). `None` when no
    /// map yields geometry.
    pub fn build_density_surface_mesh(&self, siglevel: f64) -> Option<cuemol_render::RenderMesh> {
        let scene_uid = self.active_scene?;
        let color = [0.45, 0.65, 0.95, 1.0];
        let mut vertices: Vec<cuemol_render::MeshVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for map_object in self
            .density_maps
            .values()
            .filter(|object| object.scene_uid == scene_uid)
        {
            let mut renderer = MapSurfRenderer::new();
            renderer.map_mut().set_siglevel(siglevel);
            let surface = renderer.generate_surf_obj(&map_object.map);
            let mesh = surface.mesh();
            let base = vertices.len() as u32;
            vertices.extend(mesh.vertices().iter().map(|vertex| cuemol_render::MeshVertex {
                position: vertex.position,
                normal: vertex.normal,
                color,
            }));
            indices.extend(
                mesh.triangles()
                    .iter()
                    .flat_map(|triangle| triangle.iter().map(|index| base + index)),
            );
        }
        if indices.is_empty() {
            return None;
        }
        Some(cuemol_render::RenderMesh { vertices, indices })
    }

    /// Insert a pre-built density map (a computed/synthetic `DensityMap`, not read from
    /// a file) into the active scene's map set, returning its uid. Companion to the
    /// file-based [`load_density_map`](Self::load_density_map).
    pub fn add_density_map(
        &mut self,
        scene_uid: Uid,
        name: impl Into<String>,
        map: DensityMap,
    ) -> Uid {
        let uid = Uid::new(self.next_density_map_uid);
        self.next_density_map_uid = self.next_density_map_uid.saturating_add(1).max(1);
        self.density_maps.insert(
            uid,
            GuiDensityMapObject {
                uid,
                scene_uid,
                name: name.into(),
                format: "synthetic".to_string(),
                map,
            },
        );
        uid
    }

    /// Store an MD trajectory (frames of atom positions, in molecule order) for an
    /// object, enabling the MolAnim animation type. Frames whose length doesn't match
    /// the molecule's atom count are kept but applied position-by-position up to the
    /// shorter length.
    pub fn set_object_trajectory(
        &mut self,
        object_uid: Uid,
        frames: Vec<Vec<cuemol_core::Vector4>>,
    ) {
        self.trajectories.insert(object_uid, frames);
    }

    /// Load an AMBER mdcrd trajectory file and attach it to an object (MolAnim source).
    /// The atom count is taken from the object's molecule (mdcrd is headerless).
    pub fn load_amber_trajectory(&mut self, object_uid: Uid, path: &Path) -> Result<usize> {
        let natom = self
            .scene_manager()
            .object(object_uid)
            .map(|object| object.mol().atoms().len())
            .unwrap_or(0);
        let trajectory = cuemol_mdtools::read_amber_mdcrd_file(path, natom, false)?;
        let frames: Vec<Vec<cuemol_core::Vector4>> = trajectory
            .frames
            .into_iter()
            .map(|frame| frame.positions)
            .collect();
        let count = frames.len();
        self.trajectories.insert(object_uid, frames);
        Ok(count)
    }

    /// Whether an object has a loaded trajectory (so the front end can offer MolAnim).
    pub fn object_has_trajectory(&self, object_uid: Uid) -> bool {
        self.trajectories
            .get(&object_uid)
            .is_some_and(|frames| !frames.is_empty())
    }

    /// The frame count of an object's trajectory (0 if none).
    pub fn trajectory_len(&self, object_uid: Uid) -> usize {
        self.trajectories
            .get(&object_uid)
            .map_or(0, |frames| frames.len())
    }

    /// Apply the trajectory frame at `fraction` (0..1) to an object's atoms — overwrites
    /// the molecule's positions in order (MolAnim playback). No-op without a trajectory.
    /// The caller re-tessellates afterwards.
    pub fn apply_trajectory_fraction(&mut self, object_uid: Uid, fraction: f64) {
        let Some(frames) = self.trajectories.get(&object_uid) else {
            return;
        };
        if frames.is_empty() {
            return;
        }
        let last = frames.len() - 1;
        let index = ((fraction.clamp(0.0, 1.0)) * last as f64).round() as usize;
        let frame = frames[index.min(last)].clone();
        let Some(object) = self.scene_manager_mut().object_mut(object_uid) else {
            return;
        };
        let atoms = object.mol_mut().atoms_mut();
        for (atom, position) in atoms.iter_mut().zip(frame.iter()) {
            atom.position = *position;
        }
    }

    /// Home-unit-cell symmetry-mate transforms (CueMol2 Symmetry panel "Symm mol" →
    /// Unit cell): the Cartesian matrices that map each crystallographic object onto
    /// its symmetry copies within the home cell, EXCLUDING the identity (the molecule
    /// itself). A front end bakes each into a copy of the rendered molecule. Empty for
    /// non-crystallographic scenes. Port of C++ `SymmRenderer::genByCell`.
    pub fn active_symmetry_mate_transforms(&self) -> Vec<cuemol_core::Matrix4> {
        let Some(scene_uid) = self.active_scene else {
            return Vec::new();
        };
        let Some(scene) = self.scene_manager().scene(scene_uid) else {
            return Vec::new();
        };
        let manager = cuemol_symm::SymmOpManager::new();
        let mut out = Vec::new();
        for object in scene.objects() {
            let mol = object.mol();
            let Some(cell) = mol.cell_params() else {
                continue;
            };
            let Some(crystal) = cuemol_symm::CrystalInfo::from_cell_params(cell) else {
                continue;
            };
            let Some(operators) = manager.symmetry_operators(crystal.space_group()) else {
                continue;
            };
            let atoms = mol.atoms();
            if atoms.is_empty() {
                continue;
            }
            // Molecule centroid (Cartesian) — drives the home-cell shift of each mate.
            let (mut sx, mut sy, mut sz) = (0.0, 0.0, 0.0);
            for atom in atoms {
                sx += atom.position.x;
                sy += atom.position.y;
                sz += atom.position.z;
            }
            let n = atoms.len() as f64;
            let center = cuemol_core::Vector4::new3(sx / n, sy / n, sz / n);
            for mate in cuemol_symm::symm_mates_by_cell(center, &crystal, operators) {
                // Skip the identity mate (the molecule at its home position): it leaves
                // the centroid fixed, while a genuine symmetry copy moves it.
                let mut moved = center;
                mate.matrix.xform3d(&mut moved);
                let dx = moved.x - center.x;
                let dy = moved.y - center.y;
                let dz = moved.z - center.z;
                if dx * dx + dy * dy + dz * dz > 1.0e-6 {
                    out.push(mate.matrix);
                }
            }
        }
        out
    }

    pub fn new_scene(&mut self, name: impl Into<String>, create_view: bool) -> NewSceneResult {
        let result = self
            .command_manager
            .new_scene_command(name.into(), create_view);
        self.active_scene = Some(result.result_scene);
        result
    }

    pub fn load_object(
        &mut self,
        target_scene: Uid,
        file_path: impl AsRef<Path>,
        object_name: impl Into<String>,
        file_format: &str,
    ) -> Result<LoadObjectResult> {
        self.command_manager
            .load_object_command(target_scene, file_path, object_name, file_format)
    }

    pub fn load_object_into_active_scene(
        &mut self,
        file_path: impl AsRef<Path>,
        object_name: impl Into<String>,
        file_format: &str,
    ) -> Result<LoadObjectResult> {
        let scene_uid = self
            .active_scene
            .ok_or_else(|| CueMolError::invalid_argument("no active scene"))?;
        self.load_object(scene_uid, file_path, object_name, file_format)
    }

    pub fn new_renderer(
        &mut self,
        target_scene: Uid,
        target_object: Uid,
        renderer_type: impl Into<String>,
        renderer_name: impl Into<String>,
    ) -> Result<NewRendererResult> {
        self.command_manager.new_renderer_command(
            target_scene,
            target_object,
            renderer_type,
            renderer_name,
        )
    }

    pub fn new_renderer_in_active_scene(
        &mut self,
        target_object: Uid,
        renderer_type: impl Into<String>,
        renderer_name: impl Into<String>,
    ) -> Result<NewRendererResult> {
        let scene_uid = self
            .active_scene
            .ok_or_else(|| CueMolError::invalid_argument("no active scene"))?;
        self.new_renderer(scene_uid, target_object, renderer_type, renderer_name)
    }

    pub fn set_renderer_visible(
        &mut self,
        object_uid: Uid,
        renderer_uid: Uid,
        visible: bool,
    ) -> Result<()> {
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let renderer = object
            .renderer_mut(renderer_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("renderer {}", renderer_uid.get())))?;
        renderer.set_visible(visible);
        Ok(())
    }

    /// The object's current object-level coloring scheme class name (e.g.
    /// `"RainbowColoring"`), or `None` when it uses the default element/CPK coloring.
    /// Lets the Coloring panel reflect the live state instead of guessing.
    pub fn object_coloring_type(&self, object_uid: Uid) -> Option<String> {
        self.scene_manager()
            .object(object_uid)?
            .coloring_type()
            .map(str::to_string)
    }

    /// Set an object's coloring scheme (CueMol2 Coloring panel). `coloring_type` is a
    /// C++ scheme class name (e.g. `"RainbowColoring"`, `"BfacColoring"`); `None`
    /// clears it so the renderers fall back to their default (element/CPK) coloring.
    /// `attrs` are the scheme parameters (`start_hue`, `low_color`, …) read back by
    /// the snapshot's `*_coloring_from_attrs` builders.
    pub fn set_object_coloring(
        &mut self,
        object_uid: Uid,
        coloring_type: Option<String>,
        attrs: std::collections::BTreeMap<String, String>,
    ) -> Result<()> {
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        object.set_coloring(coloring_type, attrs);
        Ok(())
    }

    /// Set an inline `.qsc` property attribute on a renderer (CueMol2 renderer property
    /// dialogs): e.g. `bondw` (bond cylinder radius), `sphr` (atom sphere radius),
    /// `detail` (tessellation). The snapshot's `apply_renderer_styles` resolves the
    /// inline attribute ahead of the style default, so the geometry updates on re-render.
    pub fn set_renderer_property(
        &mut self,
        object_uid: Uid,
        renderer_uid: Uid,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<()> {
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let renderer = object
            .renderer_mut(renderer_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("renderer {}", renderer_uid.get())))?;
        renderer.set_qsc_attr(key, value);
        Ok(())
    }

    /// The inline value of a renderer property attribute (`None` if unset, so the front
    /// end can fall back to the style/default). Companion to
    /// [`set_renderer_property`](Self::set_renderer_property).
    pub fn renderer_property(
        &self,
        object_uid: Uid,
        renderer_uid: Uid,
        key: &str,
    ) -> Option<String> {
        self.scene_manager()
            .object(object_uid)?
            .renderer(renderer_uid)?
            .qsc_attr(key)
            .map(str::to_string)
    }

    pub fn destroy_renderer_in_object(
        &mut self,
        object_uid: Uid,
        renderer_uid: Uid,
    ) -> Result<bool> {
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        Ok(object.destroy_renderer(renderer_uid))
    }

    pub fn load_qsc_file(&mut self, path: impl AsRef<Path>) -> Result<Option<Uid>> {
        let result = self.command_manager.load_scene_command(path, None)?;
        self.last_qsc_status = Some(result.status.clone());
        self.active_scene = result.result_scene;
        Ok(result.result_scene)
    }

    pub fn open_path(&mut self, path: impl AsRef<Path>) -> Result<GuiLoadedPath> {
        let path = path.as_ref();
        let format = guess_path_format(path, &self.stream_manager)?;
        if is_scene_format(format) {
            let scene_uid = self
                .load_qsc_file(path)?
                .ok_or_else(|| CueMolError::parse("scene was not loaded"))?;
            return Ok(GuiLoadedPath::Scene { scene_uid });
        }

        let scene_uid = self.ensure_active_scene();
        let object_name = default_object_name(path);
        if is_density_map_format(format) {
            let map_uid = self.load_density_map(scene_uid, path, object_name, format)?;
            return Ok(GuiLoadedPath::Map { scene_uid, map_uid });
        }
        let object_uid = if is_desktop_extension_format(format) {
            self.load_extension_object(scene_uid, path, object_name, format)?
        } else {
            self.load_object(scene_uid, path, object_name, format)?
                .result_object
        };
        if self
            .scene_manager()
            .object(object_uid)
            .is_some_and(|object| object.renderers().is_empty())
        {
            let _ = self.new_renderer(scene_uid, object_uid, "simple", "simple");
        }
        Ok(GuiLoadedPath::Object {
            scene_uid,
            object_uid,
        })
    }

    fn load_density_map(
        &mut self,
        scene_uid: Uid,
        path: &Path,
        map_name: String,
        format: &str,
    ) -> Result<Uid> {
        if self.scene_manager().scene(scene_uid).is_none() {
            return Err(CueMolError::NotFound(format!("scene {}", scene_uid.get())));
        }
        let map = read_density_map(path, format)?;
        let uid = Uid::new(self.next_density_map_uid);
        self.next_density_map_uid = self.next_density_map_uid.saturating_add(1).max(1);
        self.density_maps.insert(
            uid,
            GuiDensityMapObject {
                uid,
                scene_uid,
                name: map_name,
                format: format.to_string(),
                map,
            },
        );
        Ok(uid)
    }

    fn load_extension_object(
        &mut self,
        scene_uid: Uid,
        path: &Path,
        object_name: String,
        format: &str,
    ) -> Result<Uid> {
        let mol = read_extension_molecule(path, format)?;
        self.scene_manager_mut()
            .scene_mut(scene_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", scene_uid.get())))
            .map(|scene| scene.add_molecule(object_name, mol))
    }

    pub fn save_active_scene_qsc(&self, path: impl AsRef<Path>) -> Result<()> {
        let scene_uid = self
            .active_scene
            .ok_or_else(|| CueMolError::invalid_argument("no active scene"))?;
        self.scene_manager().write_qsc_file(scene_uid, path)
    }

    pub fn set_active_scene_camera(&mut self, camera: cuemol_qsys::Camera) -> Result<()> {
        let scene_uid = self
            .active_scene
            .ok_or_else(|| CueMolError::invalid_argument("no active scene"))?;
        let scene = self
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", scene_uid.get())))?;
        scene.set_camera(camera);
        Ok(())
    }

    pub fn save_object_as(&self, object_uid: Uid, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let object = self
            .scene_manager()
            .object(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let output = match ext.as_str() {
            "pdb" | "ent" | "" => write_pdb_string(object.mol()),
            "sdf" | "mol" => write_sdf_string(object.mol()),
            other => {
                return Err(CueMolError::invalid_argument(format!(
                    "unsupported object save extension: {other}"
                )))
            }
        };
        fs::write(path, output)?;
        Ok(())
    }

    /// Whether a file path ends in the given extension (case-insensitive). Front ends
    /// use it to dispatch "Export scene" between the PNG image path and POV-Ray.
    pub fn path_has_extension(path: &str, ext: &str) -> bool {
        Path::new(path.trim())
            .extension()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(ext))
    }

    pub fn export_active_scene_pov(
        &self,
        path: impl AsRef<Path>,
        selection_text: &str,
    ) -> Result<usize> {
        let snapshot = self.active_render_snapshot(selection_text)?;
        let output = PovSceneExporter::new().write_atoms_string(&snapshot.atoms);
        fs::write(path, output)?;
        Ok(snapshot.atom_count())
    }

    pub fn close_active_scene(&mut self) -> Result<Option<Uid>> {
        let scene_uid = self
            .active_scene
            .ok_or_else(|| CueMolError::invalid_argument("no active scene"))?;
        if !self.scene_manager_mut().destroy_scene(scene_uid) {
            return Err(CueMolError::NotFound(format!("scene {}", scene_uid.get())));
        }
        self.density_maps
            .retain(|_, map| map.scene_uid != scene_uid);
        self.active_scene = self
            .scene_manager()
            .scenes()
            .first()
            .map(|scene| scene.uid());
        Ok(self.active_scene)
    }

    pub fn active_sequence_summaries(&self) -> Vec<GuiSequenceSummary> {
        let Some(scene_uid) = self.active_scene else {
            return Vec::new();
        };
        let Some(scene) = self.scene_manager().scene(scene_uid) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for object in scene.objects() {
            let residues = object.mol().residue_summaries();
            for chain in object.mol().chain_summaries() {
                let mut sequence = String::new();
                let mut atom_count = 0;
                for key in &chain.residue_keys {
                    if let Some(residue) = residues.iter().find(|residue| &residue.key == key) {
                        sequence.push(residue_one_letter(&residue.res_name));
                        atom_count += residue.atom_indices.len();
                    }
                }
                rows.push(GuiSequenceSummary {
                    object_uid: object.uid(),
                    object_name: object.name().to_string(),
                    chain_id: chain.chain_id,
                    residue_count: chain.residue_keys.len(),
                    atom_count,
                    sequence,
                });
            }
        }
        rows
    }

    /// The active scene's molecules as a chain → residue → atom hierarchy for the
    /// MolStruct side panel (CueMol2 `molStructTree`). Mirrors the per-chain grouping
    /// of [`active_sequence_summaries`](Self::active_sequence_summaries) but descends
    /// to individual atoms, so a front end can render the full collapsible tree.
    pub fn active_structure_tree(&self) -> Vec<GuiStructObject> {
        let Some(scene_uid) = self.active_scene else {
            return Vec::new();
        };
        let Some(scene) = self.scene_manager().scene(scene_uid) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for object in scene.objects() {
            let mol = object.mol();
            let atoms = mol.atoms();
            let residues = mol.residue_summaries();
            let mut chains = Vec::new();
            for chain in mol.chain_summaries() {
                let mut chain_residues = Vec::new();
                for key in &chain.residue_keys {
                    let Some(residue) = residues.iter().find(|residue| &residue.key == key) else {
                        continue;
                    };
                    let mut res_atoms = Vec::new();
                    for &index in &residue.atom_indices {
                        if let Some(atom) = atoms.get(index) {
                            res_atoms.push(GuiStructAtom {
                                atom_index: index,
                                serial: atom.serial,
                                name: atom.name.trim().to_string(),
                                element: atom.element.clone(),
                            });
                        }
                    }
                    chain_residues.push(GuiStructResidue {
                        res_name: residue.res_name.clone(),
                        res_id: key.res_id,
                        ins_code: key.ins_code,
                        atoms: res_atoms,
                    });
                }
                chains.push(GuiStructChain {
                    chain_id: chain.chain_id,
                    residues: chain_residues,
                });
            }
            out.push(GuiStructObject {
                object_uid: object.uid(),
                object_name: object.name().to_string(),
                chains,
            });
        }
        out
    }

    /// The active scene's molecules as an interactive sequence view (CueMol2 `seqpanel`):
    /// per-chain 1-letter residue cells, each with the selection expression that picks
    /// it and whether it is currently selected (any atom in `selection_text`). Shared by
    /// all three front ends so the sequence ↔ 3D selection sync is computed once.
    pub fn active_sequence_view(&self, selection_text: &str) -> Vec<GuiSeqChain> {
        let Some(scene_uid) = self.active_scene else {
            return Vec::new();
        };
        let Some(scene) = self.scene_manager().scene(scene_uid) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for object in scene.objects() {
            let mol = object.mol();
            let selected: std::collections::HashSet<usize> = parse_gui_selector(selection_text)
                .ok()
                .flatten()
                .map(|selector| mol.select_indices(&selector).into_iter().collect())
                .unwrap_or_default();
            let residues = mol.residue_summaries();
            for chain in mol.chain_summaries() {
                let mut chain_residues = Vec::new();
                for key in &chain.residue_keys {
                    let Some(residue) = residues.iter().find(|residue| &residue.key == key) else {
                        continue;
                    };
                    let is_selected = residue
                        .atom_indices
                        .iter()
                        .any(|index| selected.contains(index));
                    let selection = if chain.chain_id.is_empty() {
                        format!("resid {}", key.res_id)
                    } else {
                        format!("chain {} and resid {}", chain.chain_id, key.res_id)
                    };
                    chain_residues.push(GuiSeqResidue {
                        letter: residue_one_letter(&residue.res_name).to_string(),
                        res_id: key.res_id,
                        res_name: residue.res_name.clone(),
                        selection,
                        selected: is_selected,
                    });
                }
                out.push(GuiSeqChain {
                    object_uid: object.uid(),
                    object_name: object.name().to_string(),
                    chain_id: chain.chain_id,
                    residues: chain_residues,
                });
            }
        }
        out
    }

    /// Project the SELECTED atoms to image-pixel positions + their `RESid:ATOM` text,
    /// for overlaying atom labels over the composited viewport image (CueMol2 atom
    /// labels) in the gpui/dioxus front ends. `mvp` is the column-major view-projection
    /// (`camera_mvp`); `width`/`height` are the image pixel size. Atoms behind the camera
    /// or off-screen are dropped; capped at `max`.
    pub fn active_atom_labels(
        &self,
        selection_text: &str,
        mvp: [[f32; 4]; 4],
        width: f32,
        height: f32,
        max: usize,
    ) -> Vec<GuiAtomLabel> {
        let Ok(snapshot) = self.active_render_snapshot(selection_text) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for atom in snapshot.atoms.iter().filter(|atom| atom.selected) {
            if out.len() >= max {
                break;
            }
            let (Some(object_uid), Some(atom_index)) = (atom.object_uid, atom.atom_index) else {
                continue;
            };
            let p = atom.center;
            let Some((sx, sy)) =
                project_to_image(mvp, [p.x as f32, p.y as f32, p.z as f32], width, height)
            else {
                continue;
            };
            if let Ok(info) = self.atom_summary(Uid::new(object_uid), atom_index) {
                out.push(GuiAtomLabel {
                    x: sx,
                    y: sy,
                    text: format!("{}{}:{}", info.residue_name.trim(), info.residue_id, info.name),
                });
            }
        }
        out
    }

    /// Distance / angle / torsion of the selected atoms (CueMol2 AtomIntr): 2 selected
    /// atoms → distance, 3 → angle, 4 → torsion, in atom-index order. Shared by all
    /// three front ends. `None` for any other count.
    pub fn active_measure(&self, selection_text: &str) -> Option<String> {
        let scene_uid = self.active_scene?;
        let scene = self.scene_manager().scene(scene_uid)?;
        let selector = parse_gui_selector(selection_text).ok().flatten()?;
        let mut points: Vec<cuemol_core::Vector4> = Vec::new();
        for object in scene.objects() {
            let mol = object.mol();
            let atoms = mol.atoms();
            for index in mol.select_indices(&selector) {
                if let Some(atom) = atoms.get(index) {
                    points.push(atom.position);
                }
            }
        }
        measure_points(&points)
    }

    pub fn active_render_snapshot(&self, selection_text: &str) -> Result<RenderSceneSnapshot> {
        let scene_uid = self
            .active_scene
            .ok_or_else(|| CueMolError::invalid_argument("no active scene"))?;
        self.render_snapshot(scene_uid, selection_text)
    }

    /// The active scene's saved view camera — the `__current` camera restored from
    /// a loaded `.qsc` (else the first defined camera), or `None` when the scene
    /// has none (e.g. a plain molecule loaded from a PDB). Front ends and the PNG
    /// export restore this saved view instead of auto-framing.
    pub fn active_scene_saved_camera(&self) -> Option<cuemol_qsys::Camera> {
        let scene = self.scene_manager().scene(self.active_scene?)?;
        scene
            .camera_by_name("__current")
            .or_else(|| scene.cameras().first())
            .cloned()
    }

    /// Render the active scene to a PNG file via the offscreen wgpu renderer.
    /// Restores the scene's saved camera (a loaded `.qsc` view) when present, else
    /// auto-frames the molecule from its bounding box. Headless molecular image
    /// export (no window). Errors if no GPU adapter is available.
    #[cfg(feature = "offscreen")]
    pub fn export_active_scene_png(
        &self,
        path: impl AsRef<Path>,
        width: u32,
        height: u32,
        selection_text: &str,
    ) -> Result<()> {
        let snapshot = self.active_render_snapshot(selection_text)?;
        let camera = match self.active_scene_saved_camera() {
            Some(saved) => saved,
            None => {
                let mut camera = cuemol_qsys::Camera::new();
                if let Some((min, max)) = snapshot.bounds {
                    let dx = max.x - min.x;
                    let dy = max.y - min.y;
                    let dz = max.z - min.z;
                    let size = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0);
                    camera.set_center(cuemol_core::Vector4::new3(
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
        };
        let aspect = width.max(1) as f64 / height.max(1) as f64;
        let mvp = crate::offscreen::camera_mvp(&camera, aspect);
        let png = crate::offscreen::render_snapshot_png(
            &snapshot,
            mvp,
            width,
            height,
            [0.1, 0.1, 0.12, 1.0],
        )
        .ok_or_else(|| CueMolError::invalid_argument("no GPU adapter for offscreen render"))?;
        fs::write(path, png)?;
        Ok(())
    }

    pub fn render_snapshot(
        &self,
        scene_uid: Uid,
        selection_text: &str,
    ) -> Result<RenderSceneSnapshot> {
        let scene = self
            .scene_manager()
            .scene(scene_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", scene_uid.get())))?;
        let selector = parse_gui_selector(selection_text)?;
        // Resolve renderer `style="..."` presets against the bundled system style
        // sheet (the same source the CLI `render-qsc`/`qsc-info` consume), built
        // once per snapshot. Without this the GUI viewer ignored every scene
        // style and drew the hardcoded per-type look — the gap this closes.
        let styles = default_style_manager();
        let mut snapshot = RenderSceneSnapshot::empty();
        for object in scene.objects() {
            // C++ `Scene::displayRendImpl` skips an invisible object before its
            // renderers (`Scene.cpp:635`): an object-level `visible="false"`
            // hides the whole object regardless of per-renderer visibility.
            if !object.is_visible() {
                continue;
            }
            append_object_snapshot(&mut snapshot, object, selector.as_ref(), &styles);
        }
        Ok(snapshot)
    }

    pub fn ensure_active_scene(&mut self) -> Uid {
        if let Some(scene_uid) = self.active_scene {
            if self.scene_manager().scene(scene_uid).is_some() {
                return scene_uid;
            }
        }
        self.new_scene("Untitled", true).result_scene
    }

    pub fn delete_atoms_in_object(
        &mut self,
        object_uid: Uid,
        selection_text: &str,
    ) -> Result<usize> {
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let before = object.mol().atom_count();
        let changed = MolAnlManager::new().delete_atoms(object.mol_mut(), &selector);
        Ok(if changed {
            before - object.mol().atom_count()
        } else {
            0
        })
    }

    pub fn change_chain_in_object(
        &mut self,
        object_uid: Uid,
        selection_text: &str,
        chain_name: &str,
    ) -> Result<usize> {
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let count = object.mol().atom_sel_count(&selector);
        let changed =
            MolAnlManager::new().change_chain_name(object.mol_mut(), &selector, chain_name.trim());
        Ok(if changed { count } else { 0 })
    }

    pub fn renumber_residues_in_object(
        &mut self,
        object_uid: Uid,
        selection_text: &str,
        start: i32,
    ) -> Result<usize> {
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let count = object.mol().atom_sel_count(&selector);
        let changed =
            MolAnlManager::new().renum_res_index(object.mol_mut(), &selector, true, start);
        Ok(if changed { count } else { 0 })
    }

    pub fn make_bond_in_object(&mut self, object_uid: Uid, aid1: i32, aid2: i32) -> Result<bool> {
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        Ok(MolAnlManager::new().make_bond(object.mol_mut(), aid1, aid2))
    }

    pub fn remove_bond_in_object(&mut self, object_uid: Uid, aid1: i32, aid2: i32) -> Result<bool> {
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        Ok(MolAnlManager::new().remove_bond(object.mol_mut(), aid1, aid2))
    }

    pub fn assign_secondary_structure_in_object(
        &mut self,
        object_uid: Uid,
        ignore_breaks: bool,
        angle: f64,
    ) -> Result<usize> {
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        MolAnlManager::new().calc_prot_2ndry2(object.mol_mut(), ignore_breaks, angle);
        Ok(object.mol().atom_count())
    }

    pub fn calc_atom_contacts_in_object(
        &self,
        object_uid: Uid,
        selection_text: &str,
        r_min: f64,
        r_max: f64,
        hbond: bool,
        max_count: usize,
    ) -> Result<String> {
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager()
            .object(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        Ok(MolAnlManager::new().calc_atom_contact_json(
            object.mol(),
            &selector,
            r_min,
            r_max,
            hbond,
            max_count,
        ))
    }

    pub fn superpose_objects(
        &mut self,
        reference_object: Uid,
        reference_selection: &str,
        moving_object: Uid,
        moving_selection: &str,
    ) -> Result<String> {
        let reference_selection = nonempty_selection(reference_selection);
        let moving_selection = nonempty_selection(moving_selection);
        let matrix = MolAnlManager::new().superpose_ssm2_in_manager(
            self.scene_manager_mut(),
            reference_object.get(),
            reference_selection,
            moving_object.get(),
            moving_selection,
        )?;
        Ok(matrix.to_script_string())
    }

    pub fn write_surface_obj(
        &self,
        object_uid: Uid,
        selection_text: &str,
        density: f64,
        probe_radius: f64,
        path: impl AsRef<Path>,
    ) -> Result<GuiSurfaceSummary> {
        let surface = self.build_surface(object_uid, selection_text, density, probe_radius)?;
        fs::write(path, surface.mesh().to_obj_string("cuemol_surface"))?;
        Ok(GuiSurfaceSummary {
            object_uid,
            vertex_count: surface.mesh().vertex_count(),
            triangle_count: surface.mesh().triangle_count(),
            cut_plane_count: surface.cut_planes().len(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn write_cut_surface_obj(
        &self,
        object_uid: Uid,
        selection_text: &str,
        density: f64,
        probe_radius: f64,
        normal: cuemol_core::Vector4,
        position: cuemol_core::Vector4,
        path: impl AsRef<Path>,
    ) -> Result<GuiSurfaceSummary> {
        let mut surface = self.build_surface(object_uid, selection_text, density, probe_radius)?;
        surface.cut_by_plane(1.0, normal, position, false);
        fs::write(path, surface.mesh().to_obj_string("cuemol_cut_surface"))?;
        Ok(GuiSurfaceSummary {
            object_uid,
            vertex_count: surface.mesh().vertex_count(),
            triangle_count: surface.mesh().triangle_count(),
            cut_plane_count: surface.cut_planes().len(),
        })
    }

    pub fn write_apbs_files(
        &self,
        object_uid: Uid,
        selection_text: &str,
        pqr_path: impl AsRef<Path>,
        input_path: impl AsRef<Path>,
        dx_path: impl AsRef<Path>,
    ) -> Result<GuiApbsSummary> {
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager()
            .object(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let mut writer = PqrFileWriter::new();
        writer.set_selection(Some(selector.clone()));
        writer.set_use_h(true);
        fs::write(&pqr_path, writer.write_pqr_string(object.mol()))?;

        let (center, length) = selection_center_and_length(object.mol(), &selector)
            .unwrap_or((cuemol_core::Vector4::ZERO, 60.0));
        let mut config = ApbsInputConfig::new(
            object.name(),
            pqr_path.as_ref().to_string_lossy(),
            dx_path.as_ref().to_string_lossy(),
        );
        config.center = center;
        config.coarse_length = [length * 1.4, length * 1.4, length * 1.4];
        config.fine_length = [length, length, length];
        fs::write(&input_path, config.to_input_string())?;

        Ok(GuiApbsSummary {
            object_uid,
            atom_count: object.mol().atom_sel_count(&selector),
            pqr_path: pqr_path.as_ref().to_string_lossy().to_string(),
            input_path: input_path.as_ref().to_string_lossy().to_string(),
            dx_path: dx_path.as_ref().to_string_lossy().to_string(),
        })
    }

    pub fn append_morph_frame_from_object(
        &mut self,
        target_object: Uid,
        source_object: Uid,
        frame_name: impl Into<String>,
    ) -> Result<GuiMorphSummary> {
        let source = self.scene_manager().object(source_object).ok_or_else(|| {
            CueMolError::NotFound(format!("source object {}", source_object.get()))
        })?;
        let positions = source
            .mol()
            .atoms()
            .iter()
            .map(|atom| atom.position)
            .collect::<Vec<_>>();
        let source_atom_count = positions.len();
        let target_atom_count = self
            .scene_manager()
            .object(target_object)
            .ok_or_else(|| CueMolError::NotFound(format!("target object {}", target_object.get())))?
            .mol()
            .atom_count();
        if source_atom_count != target_atom_count {
            return Err(CueMolError::invalid_argument(format!(
                "morph frame atom count mismatch: target={target_atom_count}, source={source_atom_count}"
            )));
        }
        let morph = self.morphs.entry(target_object).or_default();
        morph.append_frame(MorphFrame::new(
            frame_name,
            format!("object:{}", source_object.get()),
            "molecule",
            positions,
        ));
        Ok(GuiMorphSummary {
            object_uid: target_object,
            frame_count: morph.frame_size(),
            atom_count: target_atom_count,
        })
    }

    pub fn apply_morph_frame(&mut self, object_uid: Uid, frame: f64) -> Result<GuiMorphSummary> {
        let positions = self
            .morphs
            .get(&object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("morph object {}", object_uid.get())))?
            .interpolated_positions(frame);
        let frame_count = self
            .morphs
            .get(&object_uid)
            .map(MorphMol::frame_size)
            .unwrap_or_default();
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        if positions.len() != object.mol().atom_count() {
            return Err(CueMolError::invalid_argument(format!(
                "morph frame atom count mismatch: object={}, frame={}",
                object.mol().atom_count(),
                positions.len()
            )));
        }
        for (atom, position) in object.mol_mut().atoms_mut().iter_mut().zip(positions) {
            atom.position = position;
        }
        Ok(GuiMorphSummary {
            object_uid,
            frame_count,
            atom_count: object.mol().atom_count(),
        })
    }

    fn build_surface(
        &self,
        object_uid: Uid,
        selection_text: &str,
        density: f64,
        probe_radius: f64,
    ) -> Result<MolSurfObj> {
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager()
            .object(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let mut surface = MolSurfObj::new();
        surface.create_ses_from_mol(object.mol(), selector, density, probe_radius);
        Ok(surface)
    }

    /// Append hydrogen bonds as dashed cyan lines to the snapshot (CueMol2 H-bond
    /// display). Heavy-atom heuristic (deposited structures usually lack H): a donor N
    /// and acceptor O in different residues within 2.4–3.5 Å. No-op unless `show`.
    pub fn append_hbond_geometry(&self, snapshot: &mut RenderSceneSnapshot, show: bool) {
        if !show {
            return;
        }
        let Some(scene_uid) = self.active_scene else {
            return;
        };
        let Some(scene) = self.scene_manager().scene(scene_uid) else {
            return;
        };
        let color = cuemol_gfx::ColorSpec {
            base: cuemol_gfx::ColorBase::Html {
                r: 120,
                g: 220,
                b: 235,
                a: 255,
            },
            modifiers: Vec::new(),
        };
        for object in scene.objects() {
            let atoms = object.mol().atoms();
            for donor in atoms.iter().filter(|atom| atom.element.eq_ignore_ascii_case("N")) {
                for acceptor in atoms.iter().filter(|atom| atom.element.eq_ignore_ascii_case("O")) {
                    if donor.chain_id == acceptor.chain_id
                        && donor.res_id == acceptor.res_id
                        && donor.ins_code == acceptor.ins_code
                    {
                        continue;
                    }
                    let distance = donor.position.sub(acceptor.position).length3d();
                    if !(2.4..=3.5).contains(&distance) {
                        continue;
                    }
                    // 5 dashes (half dash, half gap) along the donor→acceptor line.
                    for k in 0..5 {
                        let t0 = k as f64 / 5.0;
                        let from = lerp_point(donor.position, acceptor.position, t0);
                        let to = lerp_point(donor.position, acceptor.position, t0 + 0.1);
                        snapshot.bonds.push(cuemol_render::RenderBond {
                            from,
                            to,
                            radius: 0.05,
                            color: Some(color.clone()),
                            object_uid: None,
                            atom_indices: None,
                            selected: false,
                        });
                    }
                }
            }
        }
    }

    /// Build a molecular surface (SES) for an object and convert it to a renderable
    /// triangle mesh (CueMol2 `molsurf` renderer DISPLAY — until now the desktop only
    /// EXPORTED the surface to OBJ, never drew it). A front end caches the mesh and adds
    /// it to the snapshot. Light-grey, lit by the per-vertex SES normals.
    pub fn build_surface_mesh(
        &self,
        object_uid: Uid,
        selection_text: &str,
        density: f64,
        probe_radius: f64,
    ) -> Result<cuemol_render::RenderMesh> {
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager()
            .object(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let mut surface = MolSurfObj::new();
        surface.create_ses_from_mol(object.mol(), selector.clone(), density, probe_radius);
        Ok(surface_render_mesh(object, &selector, &surface))
    }

    /// Build the molecular surface and CLIP it by a plane, returning the cross-section
    /// as a renderable mesh for live in-viewport display (CueMol2 surface cutter, live
    /// mode). Same build+cut path as `write_cut_surface_obj` but converted to a
    /// `RenderMesh` instead of written to OBJ.
    #[allow(clippy::too_many_arguments)]
    pub fn build_cut_surface_mesh(
        &self,
        object_uid: Uid,
        selection_text: &str,
        density: f64,
        probe_radius: f64,
        normal: cuemol_core::Vector4,
        position: cuemol_core::Vector4,
    ) -> Result<cuemol_render::RenderMesh> {
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager()
            .object(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let mut surface = MolSurfObj::new();
        surface.create_ses_from_mol(object.mol(), selector.clone(), density, probe_radius);
        surface.cut_by_plane(1.0, normal, position, false);
        Ok(surface_render_mesh(object, &selector, &surface))
    }

    /// Load an electrostatic-potential map (APBS/OpenDX `.dx`) for potential-coloured
    /// surfaces. Reuses the existing OpenDX grid reader (the density-map `apbs` path);
    /// the potential is stored as a `DensityMap` scalar carrier — a functional port of
    /// C++ `OpenDXPotReader` + `ElePotMap` (256-level byte-quantised, not float-exact).
    /// Returns the `(min, max)` potential range so a front end can label the scale.
    pub fn load_surface_potential(&mut self, path: impl AsRef<Path>) -> Result<(f64, f64)> {
        let map = read_density_map(path.as_ref(), "apbs")?;
        let range = (map.min_density(), map.max_density());
        self.surface_potential = Some(map);
        Ok(range)
    }

    /// Inject a pre-built potential scalar field (used by tests and synthetic maps).
    pub fn set_surface_potential_map(&mut self, map: DensityMap) {
        self.surface_potential = Some(map);
    }

    /// Whether a potential map is loaded (a front end enables its "Potential surface"
    /// action only when this is true).
    pub fn has_surface_potential(&self) -> bool {
        self.surface_potential.is_some()
    }

    /// The `(min, max)` of the loaded potential, or `None` if none is loaded.
    pub fn surface_potential_range(&self) -> Option<(f64, f64)> {
        self.surface_potential
            .as_ref()
            .map(|map| (map.min_density(), map.max_density()))
    }

    /// Set the SCAPOT gradient ± half-width (potential units): `Some(s)` clamps the
    /// red/blue ramp to `mid ± s` (higher contrast on the molecular boundary than the
    /// spike-dominated field range); `None` restores the robust 2·rmsd auto default.
    /// A non-positive value is treated as auto.
    pub fn set_potential_scale(&mut self, scale: Option<f64>) {
        self.potential_scale = scale.filter(|s| *s > 0.0);
    }

    /// The user-set SCAPOT ± scale, or `None` when using the auto (2·rmsd) default.
    pub fn potential_scale(&self) -> Option<f64> {
        self.potential_scale
    }

    /// The effective SCAPOT thresholds `(lo, mid, hi)` for a potential map: the neutral
    /// isopotential `mid` (0 when the map straddles it, else its mean) ± the half-width
    /// (the user scale, else a robust 2·rmsd — NOT the field min/max, whose near-atom
    /// spikes wash the boundary surface to near-white). C++ `getColorSca` thresholds.
    fn potential_thresholds(&self, map: &DensityMap) -> (f64, f64, f64) {
        let mid = if map.min_density() < 0.0 && map.max_density() > 0.0 {
            0.0
        } else {
            map.mean_density()
        };
        let half = self.potential_scale.unwrap_or_else(|| {
            let rmsd = map.rmsd_density();
            if rmsd > 1.0e-9 {
                2.0 * rmsd
            } else {
                ((map.max_density() - map.min_density()).abs() * 0.5).max(1.0e-6)
            }
        });
        (mid - half, mid, mid + half)
    }

    /// The effective SCAPOT `(lo, mid, hi)` for the loaded potential, or `None` if none
    /// is loaded — for a front end to label the colour scale.
    pub fn surface_potential_thresholds(&self) -> Option<(f64, f64, f64)> {
        self.surface_potential
            .as_ref()
            .map(|map| self.potential_thresholds(map))
    }

    /// Build a molecular surface and colour every vertex by the loaded electrostatic
    /// potential at that point — the C++ `MolSurfRenderer` SCAPOT mode (`getColorSca`:
    /// sample the scalar field, map through a low→mid→high gradient). The canonical
    /// red(−)/white(0)/blue(+) electrostatic surface. Errors if no potential is loaded.
    pub fn build_surface_potential_mesh(
        &self,
        object_uid: Uid,
        selection_text: &str,
        density: f64,
        probe_radius: f64,
    ) -> Result<cuemol_render::RenderMesh> {
        let potential = self.surface_potential.as_ref().ok_or_else(|| {
            CueMolError::invalid_argument(
                "no electrostatic potential loaded (load a .dx map first)",
            )
        })?;
        let selector = parse_gui_selector(selection_text)?.unwrap_or(Selector::All);
        let object = self
            .scene_manager()
            .object(object_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object_uid.get())))?;
        let mut surface = MolSurfObj::new();
        surface.create_ses_from_mol(object.mol(), selector, density, probe_radius);
        let (lo, mid, hi) = self.potential_thresholds(potential);
        Ok(surface_scalar_mesh(&surface, potential, lo, mid, hi))
    }

    /// Execute a JavaScript source string with the embedded boa engine (CueMol2 Tools →
    /// Execute script). The full JS LANGUAGE runs (arithmetic / loops / strings), and a
    /// `cuemol` global exposes a subset of the real CueMol2 script API:
    ///
    /// - Logging (real names + legacy alias): `putLogMsg(msg)` / `println(msg)` /
    ///   `print(msg)`.
    /// - Selection: `makeSel(expr)` VALIDATES a selector and returns its string (or
    ///   `null`); `select(expr)` applies a selector to the current selection (returned to
    ///   the front end).
    /// - Colour: `makeColor(spec)` resolves a colour spec/name to `{r,g,b,a}` bytes;
    ///   `color(scheme)` applies rainbow/bfactor/element colouring to every object.
    /// - Scene mutations (buffered by name, resolved to Uids at apply time):
    ///   `setColoring(objName, scheme)`, `setRendererVisible(objName, rendName, bool)`,
    ///   `setRendererProp(objName, rendName, key, value)` — flat calls onto the existing
    ///   desktop-core setters (no live proxies).
    /// - Live computed query: `measure(sel)` returns the distance/angle/torsion of a
    ///   selection (2/3/4 atoms) computed against the scene's molecules DURING eval —
    ///   the same math as the Measure ribbon.
    /// - Structural queries over the active scene: `getScene()`, `getObjectNames()`,
    ///   `getObject(name)`, `getClassName(name)`, `getRenderer(objName, rendName)`,
    ///   `getRendererNames(objName)`. These read a read-only snapshot built before eval
    ///   (boa's native closures can't borrow `&mut self`), so they return FROZEN data
    ///   objects (`{uid, name, type, rendererNames}`), NOT live objects — method chaining
    ///   like `getObject(n).setProp(...)` is deliberately out of scope (see below).
    ///
    /// Returns the printed log + the last `select(...)` expression. A real engine (not the
    /// load+preview fallback). SCOPE: this is the top-frequency subset of CueMol2's script
    /// API — structural queries, a live computed `measure`, and flat by-name mutations.
    /// The deeper live-object model (chained property setters via JS proxies reflected in
    /// the real scene, `createObj`) is not yet bridged.
    #[cfg(feature = "scripting")]
    pub fn run_script_js(&mut self, source: &str) -> Result<(String, Option<String>)> {
        use boa_engine::object::builtins::JsArray;
        use boa_engine::{js_string, object::ObjectInitializer, property::Attribute};
        use boa_engine::{Context, JsResult, JsString, JsValue, NativeFunction, Source};

        fn argn(args: &[JsValue], index: usize, context: &mut Context) -> String {
            args.get(index)
                .cloned()
                .unwrap_or(JsValue::undefined())
                .to_string(context)
                .map(|value| value.to_std_string_escaped())
                .unwrap_or_default()
        }
        fn arg0(args: &[JsValue], context: &mut Context) -> String {
            argn(args, 0, context)
        }

        // --- log/print commands (buffered, drained after eval) ---
        fn js_print(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            script_cmds::push(ScriptCmd::Print(arg0(args, ctx)));
            Ok(JsValue::undefined())
        }
        fn js_select(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            script_cmds::push(ScriptCmd::Select(arg0(args, ctx)));
            Ok(JsValue::undefined())
        }
        fn js_color(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            script_cmds::push(ScriptCmd::Color(arg0(args, ctx)));
            Ok(JsValue::undefined())
        }
        // --- scene mutations (buffered by name; resolved to Uids at apply time) ---
        fn js_set_coloring(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            script_cmds::push(ScriptCmd::SetColoring {
                object: arg0(args, ctx),
                scheme: argn(args, 1, ctx),
            });
            Ok(JsValue::undefined())
        }
        fn js_set_renderer_visible(
            _t: &JsValue,
            args: &[JsValue],
            ctx: &mut Context,
        ) -> JsResult<JsValue> {
            let visible = args.get(2).map(JsValue::to_boolean).unwrap_or(true);
            script_cmds::push(ScriptCmd::SetRendererVisible {
                object: arg0(args, ctx),
                renderer: argn(args, 1, ctx),
                visible,
            });
            Ok(JsValue::undefined())
        }
        fn js_set_renderer_prop(
            _t: &JsValue,
            args: &[JsValue],
            ctx: &mut Context,
        ) -> JsResult<JsValue> {
            script_cmds::push(ScriptCmd::SetRendererProp {
                object: arg0(args, ctx),
                renderer: argn(args, 1, ctx),
                key: argn(args, 2, ctx),
                value: argn(args, 3, ctx),
            });
            Ok(JsValue::undefined())
        }
        // --- pure builders (validate + return, no host mutation) ---
        fn js_make_sel(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            let expr = arg0(args, ctx);
            match cuemol_molstr::parse_selector(&expr) {
                Ok(_) => Ok(JsString::from(expr.as_str()).into()),
                Err(_) => Ok(JsValue::null()),
            }
        }
        fn js_make_color(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            let spec = arg0(args, ctx);
            match resolve_script_color(&spec) {
                Some(rgba) => {
                    let obj = ObjectInitializer::new(ctx)
                        .property(js_string!("r"), f64::from(rgba.r), Attribute::all())
                        .property(js_string!("g"), f64::from(rgba.g), Attribute::all())
                        .property(js_string!("b"), f64::from(rgba.b), Attribute::all())
                        .property(js_string!("a"), f64::from(rgba.a), Attribute::all())
                        .build();
                    Ok(obj.into())
                }
                None => Ok(JsValue::null()),
            }
        }
        // --- structural queries (read the pre-built read-only snapshot) ---
        fn js_get_object_names(
            _t: &JsValue,
            _args: &[JsValue],
            ctx: &mut Context,
        ) -> JsResult<JsValue> {
            let names: Vec<JsValue> = script_env::with(|env| {
                env.objects
                    .iter()
                    .map(|object| JsString::from(object.name.as_str()).into())
                    .collect()
            });
            Ok(JsArray::from_iter(names, ctx).into())
        }
        fn js_get_class_name(
            _t: &JsValue,
            args: &[JsValue],
            ctx: &mut Context,
        ) -> JsResult<JsValue> {
            let name = arg0(args, ctx);
            match script_env::with(|env| {
                env.objects
                    .iter()
                    .find(|object| object.name == name)
                    .map(|object| object.class.clone())
            }) {
                Some(class) => Ok(JsString::from(class.as_str()).into()),
                None => Ok(JsValue::null()),
            }
        }
        fn js_get_object(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            let name = arg0(args, ctx);
            let found = script_env::with(|env| {
                env.objects.iter().find(|object| object.name == name).cloned()
            });
            let Some(object) = found else {
                return Ok(JsValue::null());
            };
            let renderer_names: Vec<JsValue> = object
                .renderers
                .iter()
                .map(|renderer| JsString::from(renderer.name.as_str()).into())
                .collect();
            let renderer_array = JsArray::from_iter(renderer_names, ctx);
            let obj = ObjectInitializer::new(ctx)
                .property(js_string!("uid"), object.uid as f64, Attribute::all())
                .property(
                    js_string!("name"),
                    JsString::from(object.name.as_str()),
                    Attribute::all(),
                )
                .property(
                    js_string!("type"),
                    JsString::from(object.class.as_str()),
                    Attribute::all(),
                )
                .property(js_string!("rendererNames"), renderer_array, Attribute::all())
                .build();
            Ok(obj.into())
        }
        fn js_get_renderer_names(
            _t: &JsValue,
            args: &[JsValue],
            ctx: &mut Context,
        ) -> JsResult<JsValue> {
            let name = arg0(args, ctx);
            let names: Vec<JsValue> = script_env::with(|env| {
                env.objects
                    .iter()
                    .find(|object| object.name == name)
                    .map(|object| {
                        object
                            .renderers
                            .iter()
                            .map(|renderer| JsString::from(renderer.name.as_str()).into())
                            .collect()
                    })
                    .unwrap_or_default()
            });
            Ok(JsArray::from_iter(names, ctx).into())
        }
        fn js_get_renderer(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            let object_name = arg0(args, ctx);
            let renderer_name = argn(args, 1, ctx);
            let found = script_env::with(|env| {
                env.objects
                    .iter()
                    .find(|object| object.name == object_name)
                    .and_then(|object| {
                        object
                            .renderers
                            .iter()
                            .find(|renderer| renderer.name == renderer_name)
                            .cloned()
                    })
            });
            let Some(renderer) = found else {
                return Ok(JsValue::null());
            };
            let obj = ObjectInitializer::new(ctx)
                .property(
                    js_string!("name"),
                    JsString::from(renderer.name.as_str()),
                    Attribute::all(),
                )
                .property(
                    js_string!("type"),
                    JsString::from(renderer.rtype.as_str()),
                    Attribute::all(),
                )
                .build();
            Ok(obj.into())
        }
        fn js_get_scene(_t: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            let (uid, name, object_names) = script_env::with(|env| {
                (
                    env.scene_uid,
                    env.scene_name.clone(),
                    env.objects
                        .iter()
                        .map(|object| JsString::from(object.name.as_str()).into())
                        .collect::<Vec<JsValue>>(),
                )
            });
            let names_array = JsArray::from_iter(object_names, ctx);
            let obj = ObjectInitializer::new(ctx)
                .property(js_string!("uid"), uid as f64, Attribute::all())
                .property(
                    js_string!("name"),
                    JsString::from(name.as_str()),
                    Attribute::all(),
                )
                .property(js_string!("objectNames"), names_array, Attribute::all())
                .build();
            Ok(obj.into())
        }
        // Live computed query: measure a selection (2 atoms→distance, 3→angle,
        // 4→torsion) against the snapshot molecules DURING eval — the same math as the
        // Measure ribbon, so a script can read a measurement programmatically.
        fn js_measure(_t: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            let sel = arg0(args, ctx);
            let result = script_env::with(|env| {
                let selector = parse_gui_selector(&sel).ok().flatten()?;
                let mut points = Vec::new();
                for mol in &env.molecules {
                    let atoms = mol.atoms();
                    for index in mol.select_indices(&selector) {
                        if let Some(atom) = atoms.get(index) {
                            points.push(atom.position);
                        }
                    }
                }
                measure_points(&points)
            });
            match result {
                Some(text) => Ok(JsString::from(text.as_str()).into()),
                None => Ok(JsValue::null()),
            }
        }

        let _ = script_cmds::drain(); // clear any stale buffer
        script_env::set(self.build_script_env());
        let mut context = Context::default();
        let cuemol = ObjectInitializer::new(&mut context)
            .function(NativeFunction::from_fn_ptr(js_print), js_string!("print"), 1)
            .function(NativeFunction::from_fn_ptr(js_print), js_string!("putLogMsg"), 1)
            .function(NativeFunction::from_fn_ptr(js_print), js_string!("println"), 1)
            .function(NativeFunction::from_fn_ptr(js_select), js_string!("select"), 1)
            .function(NativeFunction::from_fn_ptr(js_color), js_string!("color"), 1)
            .function(
                NativeFunction::from_fn_ptr(js_set_coloring),
                js_string!("setColoring"),
                2,
            )
            .function(
                NativeFunction::from_fn_ptr(js_set_renderer_visible),
                js_string!("setRendererVisible"),
                3,
            )
            .function(
                NativeFunction::from_fn_ptr(js_set_renderer_prop),
                js_string!("setRendererProp"),
                4,
            )
            .function(NativeFunction::from_fn_ptr(js_make_sel), js_string!("makeSel"), 1)
            .function(NativeFunction::from_fn_ptr(js_make_color), js_string!("makeColor"), 1)
            .function(NativeFunction::from_fn_ptr(js_get_scene), js_string!("getScene"), 0)
            .function(NativeFunction::from_fn_ptr(js_get_object), js_string!("getObject"), 1)
            .function(
                NativeFunction::from_fn_ptr(js_get_object_names),
                js_string!("getObjectNames"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(js_get_class_name),
                js_string!("getClassName"),
                1,
            )
            .function(
                NativeFunction::from_fn_ptr(js_get_renderer),
                js_string!("getRenderer"),
                2,
            )
            .function(
                NativeFunction::from_fn_ptr(js_get_renderer_names),
                js_string!("getRendererNames"),
                1,
            )
            .function(NativeFunction::from_fn_ptr(js_measure), js_string!("measure"), 1)
            .build();
        let outcome = context
            .register_global_property(js_string!("cuemol"), cuemol, Attribute::all())
            .and_then(|()| context.eval(Source::from_bytes(source)));
        script_env::clear();
        outcome.map_err(|error| CueMolError::invalid_argument(format!("script error: {error}")))?;

        let mut log = String::new();
        let mut selection = None;
        for cmd in script_cmds::drain() {
            match cmd {
                ScriptCmd::Print(message) => {
                    log.push_str(&message);
                    log.push('\n');
                }
                ScriptCmd::Select(expr) => selection = Some(expr),
                ScriptCmd::Color(scheme) => self.apply_script_color(&scheme),
                ScriptCmd::SetColoring { object, scheme } => {
                    if let Some(uid) = self.object_uid_by_name(&object) {
                        let (coloring_type, attrs) = coloring_for_scheme(&scheme);
                        let _ = self.set_object_coloring(uid, coloring_type, attrs);
                    }
                }
                ScriptCmd::SetRendererVisible {
                    object,
                    renderer,
                    visible,
                } => {
                    if let Some((obj_uid, rend_uid)) = self.renderer_uid_by_name(&object, &renderer)
                    {
                        let _ = self.set_renderer_visible(obj_uid, rend_uid, visible);
                    }
                }
                ScriptCmd::SetRendererProp {
                    object,
                    renderer,
                    key,
                    value,
                } => {
                    if let Some((obj_uid, rend_uid)) = self.renderer_uid_by_name(&object, &renderer)
                    {
                        let _ = self.set_renderer_property(obj_uid, rend_uid, key, value);
                    }
                }
            }
        }
        Ok((log, selection))
    }

    /// Build the read-only [`ScriptEnv`] snapshot the `cuemol` query functions read
    /// during `eval` (molecule objects are class `MolCoord`, density maps `DensityMap`;
    /// each object carries its renderer name+type list). Frozen structural data only —
    /// no live handles.
    #[cfg(feature = "scripting")]
    fn build_script_env(&self) -> ScriptEnv {
        let mut objects = Vec::new();
        for object in self.active_object_summaries() {
            let renderers = self
                .renderer_summaries(object.uid)
                .into_iter()
                .map(|renderer| ScriptRend {
                    name: renderer.name,
                    rtype: renderer.renderer_type,
                })
                .collect();
            objects.push(ScriptObj {
                uid: object.uid.get(),
                name: object.name,
                class: "MolCoord".to_string(),
                renderers,
            });
        }
        if let Some(scene_uid) = self.active_scene() {
            for map in self.density_map_summaries(scene_uid) {
                objects.push(ScriptObj {
                    uid: map.uid.get(),
                    name: map.name,
                    class: "DensityMap".to_string(),
                    renderers: Vec::new(),
                });
            }
        }
        let (scene_uid, scene_name) = match self.active_scene_summary() {
            Some(scene) => (scene.uid.get(), scene.name),
            None => (0, String::new()),
        };
        let molecules = self
            .active_scene()
            .and_then(|uid| self.scene_manager().scene(uid))
            .map(|scene| scene.objects().iter().map(|object| object.mol().clone()).collect())
            .unwrap_or_default();
        ScriptEnv {
            scene_uid,
            scene_name,
            objects,
            molecules,
        }
    }

    /// Apply a `cuemol.color(scheme)` call (element / rainbow / bfactor) to every
    /// molecule, via the same object-coloring path the Coloring panel uses.
    #[cfg(feature = "scripting")]
    fn apply_script_color(&mut self, scheme: &str) {
        let (coloring_type, attrs) = coloring_for_scheme(scheme);
        let uids: Vec<Uid> = self
            .active_object_summaries()
            .iter()
            .map(|object| object.uid)
            .collect();
        for uid in uids {
            let _ = self.set_object_coloring(uid, coloring_type.clone(), attrs.clone());
        }
    }

    /// Resolve an object's Uid from its scene name (script mutations carry names, not
    /// Uids, and resolve at apply time against the live scene).
    #[cfg(feature = "scripting")]
    fn object_uid_by_name(&self, name: &str) -> Option<Uid> {
        self.active_object_summaries()
            .into_iter()
            .find(|object| object.name == name)
            .map(|object| object.uid)
    }

    /// Resolve an (object Uid, renderer Uid) pair from their names.
    #[cfg(feature = "scripting")]
    fn renderer_uid_by_name(&self, object: &str, renderer: &str) -> Option<(Uid, Uid)> {
        let object_uid = self.object_uid_by_name(object)?;
        let renderer_uid = self
            .renderer_summaries(object_uid)
            .into_iter()
            .find(|summary| summary.name == renderer)
            .map(|summary| summary.uid)?;
        Some((object_uid, renderer_uid))
    }
}

/// Map a `color`/`setColoring` scheme keyword to a (coloring-type, attrs) pair — the
/// shared translation used by both `cuemol.color(scheme)` (all objects) and
/// `cuemol.setColoring(objName, scheme)` (one object). Unknown → element/CPK default.
#[cfg(feature = "scripting")]
fn coloring_for_scheme(
    scheme: &str,
) -> (Option<String>, std::collections::BTreeMap<String, String>) {
    match scheme.to_ascii_lowercase().as_str() {
        "rainbow" => {
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("mode".to_string(), "mol".to_string());
            attrs.insert("start_hue".to_string(), "240".to_string());
            attrs.insert("end_hue".to_string(), "0".to_string());
            (Some("RainbowColoring".to_string()), attrs)
        }
        "bfactor" | "bfac" => (
            Some("BfacColoring".to_string()),
            std::collections::BTreeMap::new(),
        ),
        _ => (None, std::collections::BTreeMap::new()),
    }
}

/// Script-command buffer bridging boa's GC-managed native closures to the host state
/// (single-threaded GUI, so a thread-local is the simplest safe channel).
#[cfg(feature = "scripting")]
enum ScriptCmd {
    Print(String),
    Select(String),
    Color(String),
    SetColoring {
        object: String,
        scheme: String,
    },
    SetRendererVisible {
        object: String,
        renderer: String,
        visible: bool,
    },
    SetRendererProp {
        object: String,
        renderer: String,
        key: String,
        value: String,
    },
}

#[cfg(feature = "scripting")]
mod script_cmds {
    use super::ScriptCmd;
    use std::cell::RefCell;
    thread_local! {
        static CMDS: RefCell<Vec<ScriptCmd>> = const { RefCell::new(Vec::new()) };
    }
    pub(super) fn push(cmd: ScriptCmd) {
        CMDS.with(|cells| cells.borrow_mut().push(cmd));
    }
    pub(super) fn drain() -> Vec<ScriptCmd> {
        CMDS.with(|cells| cells.borrow_mut().drain(..).collect())
    }
}

/// Resolve a `cuemol.makeColor(spec)` argument to RGBA bytes. Handles `rgb()`/`hsb()`/
/// `#hex` forms directly and X11 colour names via `NamedColor`; an unknown name falls
/// back to grey (the C++ default), so a non-empty spec always yields a colour.
#[cfg(feature = "scripting")]
fn resolve_script_color(spec: &str) -> Option<cuemol_gfx::Rgba8> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = cuemol_gfx::parse_color(trimmed) {
        if let Some(rgba) = parsed.rgba8() {
            return Some(rgba);
        }
    }
    Some(cuemol_gfx::NamedColor::from_name(trimmed).resolved_color().rgba8())
}

/// Read-only snapshot of the active scene handed to the `cuemol` script queries
/// (`getScene`/`getObject`/...). Populated once before `eval`; boa's native closures
/// read it (they can't borrow `&self`), mirroring the [`ScriptCmd`] mutation bridge.
#[cfg(feature = "scripting")]
#[derive(Clone, Default)]
struct ScriptEnv {
    scene_uid: u64,
    scene_name: String,
    objects: Vec<ScriptObj>,
    /// Clones of the active scene's molecules — enough for `cuemol.measure(sel)` to
    /// evaluate an arbitrary selector and compute a distance/angle/torsion LIVE during
    /// eval (the frozen structural snapshot alone can't). Read-only; cloned once per run.
    molecules: Vec<cuemol_molstr::MolCoord>,
}

#[cfg(feature = "scripting")]
#[derive(Clone, Default)]
struct ScriptObj {
    uid: u64,
    name: String,
    class: String,
    renderers: Vec<ScriptRend>,
}

#[cfg(feature = "scripting")]
#[derive(Clone, Default)]
struct ScriptRend {
    name: String,
    rtype: String,
}

#[cfg(feature = "scripting")]
mod script_env {
    use super::ScriptEnv;
    use std::cell::RefCell;
    thread_local! {
        static ENV: RefCell<ScriptEnv> = RefCell::new(ScriptEnv::default());
    }
    pub(super) fn set(env: ScriptEnv) {
        ENV.with(|cell| *cell.borrow_mut() = env);
    }
    pub(super) fn clear() {
        ENV.with(|cell| *cell.borrow_mut() = ScriptEnv::default());
    }
    pub(super) fn with<R>(f: impl FnOnce(&ScriptEnv) -> R) -> R {
        ENV.with(|cell| f(&cell.borrow()))
    }
}

/// Convert a built molecular surface to a renderable mesh, colouring each vertex
/// by its nearest surface-generating atom under the object's coloring cascade —
/// the C++ `MolSurfRenderer` MOLFANC mode (`getColorMol`: nearest atom via an
/// atom-position map, then that atom's colour-scheme colour). Reuses the CPK
/// renderer's per-atom resolver ([`cuemol_molvis::resolve_atom_colors`]) so the
/// surface matches the atoms the same viewer draws (element/CPK by default;
/// rainbow/bfac/paint follow the object's scheme on the same carbon cascade the
/// atoms use). Nearest search is a linear scan over the SELECTED atom subset (the
/// atoms the surface was built from): O(vertices × selected_atoms), fine for
/// typical selections; no spatial index until a large-structure test needs one.
fn surface_render_mesh(
    object: &MolObject,
    selector: &Selector,
    surface: &MolSurfObj,
) -> cuemol_render::RenderMesh {
    // Fallback when no atom is available to colour a vertex (the former flat grey).
    const GREY: [f32; 4] = [0.76, 0.79, 0.84, 1.0];
    let options = MoleculeRenderOptions {
        rainbow: cuemol_molvis::rainbow_coloring_from_attrs(object.coloring_type(), |name| {
            object.coloring_attr(name)
        }),
        bfac: cuemol_molvis::bfac_coloring_from_attrs(object.coloring_type(), |name| {
            object.coloring_attr(name)
        }),
        object_paint: object.paint_coloring().to_vec(),
        ..Default::default()
    };
    let atom_colors = cuemol_molvis::resolve_atom_colors(object.mol(), &options);
    // The surface-generating atom subset: position + resolved [f32;4] colour.
    let palette: Vec<(cuemol_core::Vector4, [f32; 4])> = object
        .mol()
        .select_indices(selector)
        .into_iter()
        .filter_map(|index| {
            let atom = object.mol().atoms().get(index)?;
            let rgba = atom_colors
                .get(index)
                .and_then(|color| color.rgba8())
                .map(|c| {
                    [
                        f32::from(c.r) / 255.0,
                        f32::from(c.g) / 255.0,
                        f32::from(c.b) / 255.0,
                        f32::from(c.a) / 255.0,
                    ]
                })
                .unwrap_or(GREY);
            Some((atom.position, rgba))
        })
        .collect();
    let mesh = surface.mesh();
    let vertices = mesh
        .vertices()
        .iter()
        .map(|vertex| {
            let color = palette
                .iter()
                .min_by(|(a, _), (b, _)| {
                    let da = a.sub(vertex.position).sqlen3d();
                    let db = b.sub(vertex.position).sqlen3d();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(_, color)| *color)
                .unwrap_or(GREY);
            cuemol_render::MeshVertex {
                position: vertex.position,
                normal: vertex.normal,
                color,
            }
        })
        .collect();
    let indices = mesh
        .triangles()
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
        .collect();
    cuemol_render::RenderMesh { vertices, indices }
}

/// Convert a built molecular surface to a renderable mesh, colouring each vertex by
/// the value of a scalar field (electrostatic potential) at that vertex — the C++
/// `MolSurfRenderer` SCAPOT mode (`getColorSca`). The value is mapped through a
/// red(`lo`) → white(`mid`) → blue(`hi`) gradient at the given explicit thresholds
/// (C++ `m_dParLow/Mid/High`), NOT the raw field min/max — see
/// [`DesktopAppState::potential_thresholds`].
fn surface_scalar_mesh(
    surface: &MolSurfObj,
    scalar: &DensityMap,
    lo: f64,
    mid: f64,
    hi: f64,
) -> cuemol_render::RenderMesh {
    let mesh = surface.mesh();
    let vertices = mesh
        .vertices()
        .iter()
        .map(|vertex| cuemol_render::MeshVertex {
            position: vertex.position,
            normal: vertex.normal,
            color: potential_color(scalar.value_at(vertex.position), lo, mid, hi),
        })
        .collect();
    let indices = mesh
        .triangles()
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
        .collect();
    cuemol_render::RenderMesh { vertices, indices }
}

/// Electrostatic-potential colour (C++ `getColorSca` 3-stop gradient): `value <= lo`
/// → red, `value >= hi` → blue, `mid` → white, linearly interpolated between. Guards
/// against a degenerate (zero-width) low or high half.
fn potential_color(value: f64, lo: f64, mid: f64, hi: f64) -> [f32; 4] {
    const RED: [f32; 3] = [1.0, 0.0, 0.0];
    const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
    const BLUE: [f32; 3] = [0.0, 0.0, 1.0];
    let lerp = |a: [f32; 3], b: [f32; 3], t: f32| -> [f32; 4] {
        let t = t.clamp(0.0, 1.0);
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
            1.0,
        ]
    };
    if value <= lo {
        return [RED[0], RED[1], RED[2], 1.0];
    }
    if value >= hi {
        return [BLUE[0], BLUE[1], BLUE[2], 1.0];
    }
    if value <= mid {
        let t = if mid > lo { (value - lo) / (mid - lo) } else { 1.0 };
        lerp(RED, WHITE, t as f32)
    } else {
        let t = if hi > mid { (value - mid) / (hi - mid) } else { 1.0 };
        lerp(WHITE, BLUE, t as f32)
    }
}

fn append_object_snapshot(
    target: &mut RenderSceneSnapshot,
    object: &MolObject,
    selector: Option<&cuemol_molstr::Selector>,
    styles: &StyleManager,
) {
    let selected_indices = selector
        .map(|selector| object.mol().select_indices(selector))
        .unwrap_or_default();
    // Skip internal/overlay renderers (e.g. `*selection`): like the CLI
    // render-qsc path and C++, these system renderers carry no standalone scene
    // geometry — the selection overlay is drawn via `selected_indices`
    // highlighting on the real renderers, so rendering `*selection` itself would
    // spuriously dump every atom.
    let visible_renderers = object
        .renderers()
        .iter()
        .filter(|renderer| renderer.is_visible() && !renderer.renderer_type().starts_with('*'))
        .collect::<Vec<_>>();
    if visible_renderers.is_empty() && !object.renderers().is_empty() {
        return;
    }
    if visible_renderers.is_empty() {
        append_object_renderer_snapshot(target, object, None, &selected_indices, styles);
        return;
    }
    for renderer in visible_renderers {
        append_object_renderer_snapshot(target, object, Some(renderer), &selected_indices, styles);
    }
}

fn append_object_renderer_snapshot(
    target: &mut RenderSceneSnapshot,
    object: &MolObject,
    renderer: Option<&MolRenderer>,
    selected_indices: &[usize],
    styles: &StyleManager,
) {
    let renderer_type = renderer.map(MolRenderer::renderer_type).unwrap_or("simple");
    let mut options = renderer_render_options(object.uid(), renderer, selected_indices, styles);
    // Per-renderer `sel="..."`: restrict the atom/bond geometry to the renderer's
    // own selection (e.g. a ballstick renderer scoped to a ligand). `*`/empty/all
    // means no restriction; a parse failure falls back to drawing everything.
    if let Some(sel) = renderer
        .and_then(|renderer| renderer.qsc_attr("sel"))
        .map(str::trim)
        .filter(|sel| !sel.is_empty() && *sel != "*")
    {
        if let Ok(selector) = parse_selector(sel) {
            options.draw_indices = Some(object.mol().select_indices(&selector));
        }
    }
    // Object-level paint coloring (the scene's `<coloring type="PaintColoring">`):
    // colours the backbone where the renderer defers to $molcol.
    if !object.paint_coloring().is_empty() {
        options.object_paint = object.paint_coloring().to_vec();
    }
    // Object-level rainbow/bfac coloring — applied only when the renderer set no own
    // coloring scheme (renderer concrete coloring, set in apply_renderer_styles,
    // takes precedence over the object's, per the C++ cascade).
    if options.rainbow.is_none() {
        options.rainbow = cuemol_molvis::rainbow_coloring_from_attrs(object.coloring_type(), |name| {
            object.coloring_attr(name)
        });
    }
    if options.bfac.is_none() {
        options.bfac = cuemol_molvis::bfac_coloring_from_attrs(object.coloring_type(), |name| {
            object.coloring_attr(name)
        });
    }
    let mut snapshot = match (renderer_type, renderer) {
        // An atom-interaction renderer draws measurement lines between its atom
        // pairs, NOT the whole molecule — route it to the measurement geometry so
        // it stops dumping every atom (the gap this closes).
        ("atomintr", Some(renderer)) => cuemol_render::atom_intr_snapshot(
            &object.atom_intr_segments(renderer),
            Some(object.uid().get()),
        ),
        // Anisotropic thermal ellipsoids (ANISOU atoms), not generic spheres.
        ("anisou", _) => anisou_mesh_snapshot(object.mol(), &options),
        // Disorder = dotted Bézier lines bridging unbonded chain gaps, drawn only
        // where the backbone breaks — NOT a sphere per atom. Params are the C++
        // DisoRenderer defaults (dotsep 1.0 / loopsize 2.0 / loopsize2 -1.0; a thin
        // dot radius). On a fully-bonded molecule this correctly draws nothing.
        ("disorder", _) => disorder_lines_snapshot(object.mol(), 1.0, 0.15, 2.0, -1.0),
        // Nucleic-acid renderer = the backbone ribbon PLUS each nucleotide's base
        // stick (and base-pair rungs), matching C++ NARenderer (which the plain
        // mainchain ribbon alone omits).
        ("nucl", _) => {
            let mut snapshot = ribbon_mesh_snapshot(object.mol(), &options);
            let bases = na_base_sticks_snapshot(object.mol(), 0.25, true);
            merge_bounds(&mut snapshot.bounds, bases.bounds);
            snapshot.atoms.extend(bases.atoms);
            snapshot.bonds.extend(bases.bonds);
            snapshot.meshes.extend(bases.meshes);
            snapshot
        }
        (rtype, _) if is_mainchain_trace_renderer(rtype) => {
            ribbon_mesh_snapshot(object.mol(), &options)
        }
        _ => molecule_render_snapshot(object.mol(), &options),
    };
    // Apply the object + renderer `xformMat` (C++ `Object`/`Renderer` xformMat,
    // multiplied onto the modelview at display; the object xform is propagated onto
    // atom positions). The effective transform is `renderer · object`, baked into
    // this renderer's geometry so a superposed/positioned object renders where the
    // scene saved it. Skipped when both are identity (the common case).
    let object_xform = object.xform_matrix();
    let renderer_xform = renderer.map(MolRenderer::xform_matrix).unwrap_or(Matrix4::IDENTITY);
    let xform = renderer_xform.mul_mat(object_xform);
    if xform != Matrix4::IDENTITY {
        snapshot.apply_xform(&xform);
    }
    merge_bounds(&mut target.bounds, snapshot.bounds);
    target.atoms.extend(snapshot.atoms);
    target.bonds.extend(snapshot.bonds);
    target.meshes.extend(snapshot.meshes);
}

/// Renderer types drawn as a smoothed main-chain trace instead of all-atom geometry.
///
/// Kept in sync with the matching arm in [`renderer_render_options`].
fn is_mainchain_trace_renderer(renderer_type: &str) -> bool {
    matches!(
        renderer_type,
        "trace" | "ribbon" | "cartoon" | "tube" | "spline" | "nucl"
    )
}

fn renderer_render_options(
    object_uid: Uid,
    renderer: Option<&MolRenderer>,
    selected_indices: &[usize],
    styles: &StyleManager,
) -> MoleculeRenderOptions {
    let mut options = MoleculeRenderOptions {
        object_uid: Some(object_uid.get()),
        selected_indices: selected_indices.to_vec(),
        ..MoleculeRenderOptions::default()
    };
    let renderer_type = renderer.map(MolRenderer::renderer_type).unwrap_or("simple");
    match renderer_type {
        "cpk" => {
            options.vdw_scale = 0.55;
            options.bond_radius = 0.0;
            options.draw_bonds = false;
        }
        "ballstick" => {
            options.vdw_scale = 0.16;
            options.bond_radius = 0.08;
        }
        "trace" | "ribbon" | "cartoon" | "tube" | "spline" | "nucl" => {
            options.vdw_scale = 0.12;
            options.bond_radius = 0.12;
        }
        "molsurf" | "directsurf" => {
            options.vdw_scale = 0.32;
            options.bond_radius = 0.04;
        }
        _ => {}
    }
    // `trace` is the angular Cα line strip (C++ TraceRenderer): straight between
    // pivots, no spline. tube/spline/ribbon/cartoon/nucl keep the smooth spline.
    options.linear_backbone = renderer_type == "trace";
    // Layer the scene renderer's resolved `style="..."` geometry (bond/atom radii,
    // ribbon cross-section dims/shape, axial detail) on top of the per-type
    // defaults — the same cascade the CLI `render-qsc` applies. A renderer with no
    // style attr resolves every attr to None and keeps the defaults above, so only
    // styled scenes change. `sphere_detail` is resolved here too but the GUI's
    // `tessellate_snapshot` step does not yet honour it (cosmetic smoothness only).
    if let Some(renderer) = renderer {
        apply_renderer_styles(&mut options, renderer, styles, DEFAULT_STYLE_SCOPE);
    }
    options
}

/// Project a world point through a column-major view-projection `mvp` to image-pixel
/// coordinates `(x, y)` (origin top-left), or `None` if it is behind the camera or
/// outside the `width`×`height` image. The screen-space mapping used by atom labels.
fn project_to_image(
    mvp: [[f32; 4]; 4],
    point: [f32; 3],
    width: f32,
    height: f32,
) -> Option<(f32, f32)> {
    let v = [point[0], point[1], point[2], 1.0];
    let mut clip = [0.0f32; 4];
    for (row, clip_component) in clip.iter_mut().enumerate() {
        for (col, vc) in v.iter().enumerate() {
            *clip_component += mvp[col][row] * vc;
        }
    }
    if clip[3] <= 0.0 {
        return None;
    }
    let sx = (clip[0] / clip[3] + 1.0) * 0.5 * width;
    let sy = (1.0 - clip[1] / clip[3]) * 0.5 * height;
    if sx < 0.0 || sx > width || sy < 0.0 || sy > height {
        return None;
    }
    Some((sx, sy))
}

/// Linear interpolation between two points (used to dash the H-bond lines).
fn lerp_point(a: cuemol_core::Vector4, b: cuemol_core::Vector4, t: f64) -> cuemol_core::Vector4 {
    cuemol_core::Vector4::new3(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.z + (b.z - a.z) * t,
    )
}

fn merge_bounds(
    target: &mut Option<(cuemol_core::Vector4, cuemol_core::Vector4)>,
    next: Option<(cuemol_core::Vector4, cuemol_core::Vector4)>,
) {
    let Some((next_min, next_max)) = next else {
        return;
    };
    match target {
        Some((min, max)) => {
            min.x = min.x.min(next_min.x);
            min.y = min.y.min(next_min.y);
            min.z = min.z.min(next_min.z);
            max.x = max.x.max(next_max.x);
            max.y = max.y.max(next_max.y);
            max.z = max.z.max(next_max.z);
        }
        None => *target = Some((next_min, next_max)),
    }
}

fn parse_gui_selector(selection_text: &str) -> Result<Option<cuemol_molstr::Selector>> {
    let trimmed = selection_text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(parse_selector(trimmed)?))
}

/// CueMol2 AtomIntr measurement from atom positions: 2 → distance, 3 → angle, 4 →
/// torsion, else `None`. Shared by [`DesktopAppState::active_measure`] and the script
/// `cuemol.measure(sel)` query, so both yield identical results.
fn measure_points(points: &[cuemol_core::Vector4]) -> Option<String> {
    match points {
        [a, b] => Some(format!("Distance: {:.3} Å", b.sub(*a).length3d())),
        [a, b, c] => {
            let u = a.sub(*b);
            let v = c.sub(*b);
            let denom = u.length3d() * v.length3d();
            if denom < 1.0e-9 {
                return None;
            }
            let angle = (u.dot3d(v) / denom).clamp(-1.0, 1.0).acos().to_degrees();
            Some(format!("Angle: {angle:.2}°"))
        }
        [a, b, c, d] => {
            let b1 = b.sub(*a);
            let b2 = c.sub(*b);
            let b3 = d.sub(*c);
            let len = b2.length3d();
            if len < 1.0e-9 {
                return None;
            }
            let b2n = cuemol_core::Vector4::new3(b2.x / len, b2.y / len, b2.z / len);
            let n1 = b1.cross(b2);
            let n2 = b2.cross(b3);
            let m = n1.cross(b2n);
            let torsion = m.dot3d(n2).atan2(n1.dot3d(n2)).to_degrees();
            Some(format!("Torsion: {torsion:.2}°"))
        }
        _ => None,
    }
}

fn nonempty_selection(selection_text: &str) -> &str {
    let trimmed = selection_text.trim();
    if trimmed.is_empty() {
        "all"
    } else {
        trimmed
    }
}

fn selection_center_and_length(
    molecule: &cuemol_molstr::MolCoord,
    selector: &Selector,
) -> Option<(cuemol_core::Vector4, f64)> {
    let mut iter = molecule
        .atoms()
        .iter()
        .filter(|atom| selector.matches_in_molecule(atom, molecule));
    let first = iter.next()?.position;
    let mut min = first;
    let mut max = first;
    for atom in iter {
        let pos = atom.position;
        min.x = min.x.min(pos.x);
        min.y = min.y.min(pos.y);
        min.z = min.z.min(pos.z);
        max.x = max.x.max(pos.x);
        max.y = max.y.max(pos.y);
        max.z = max.z.max(pos.z);
    }
    let center = (min + max).scale(0.5);
    let span = (max - min).length3d().max(20.0);
    Some((center, span + 20.0))
}

fn density_map_summary(object: &GuiDensityMapObject) -> GuiDensityMapSummary {
    let (center, radius) = object.map.fit_view_center_and_radius();
    GuiDensityMapSummary {
        uid: object.uid,
        scene_uid: object.scene_uid,
        name: object.name.clone(),
        format: object.format.clone(),
        counts: [
            object.map.col_no(),
            object.map.row_no(),
            object.map.sec_no(),
        ],
        value_count: object.map.byte_values().len(),
        min_density: object.map.den_min(),
        max_density: object.map.den_max(),
        mean_density: object.map.den_mean(),
        rmsd_density: object.map.den_sigma(),
        center: [center.x, center.y, center.z],
        radius,
    }
}

fn guess_path_format(path: &Path, stream_manager: &StreamManager) -> Result<&'static str> {
    let ext = effective_extension(path);
    match ext.as_str() {
        "qsc" | "qsl" => Ok("qsc_xml"),
        "pdb" | "ent" => Ok("pdb"),
        "qdfpdb" => Ok("qdfpdb"),
        "cif" | "mmcif" => Ok("mmcif"),
        "sdf" | "mol" => Ok("sdf"),
        "mol2" => Ok("mol2"),
        "gro" => Ok("gro"),
        "psf" => Ok("psf"),
        "prmtop" | "parm7" | "top" => Ok("amberprm"),
        "dx" => Ok("apbs"),
        "ccp4" | "mrc" => Ok("ccp4map"),
        "cns" => Ok("xplormap"),
        "brix" | "omap" => Ok("brix"),
        "map" => guess_ambiguous_map_format(path, stream_manager),
        other => Err(CueMolError::invalid_argument(format!(
            "unsupported file extension: {other}"
        ))),
    }
}

fn guess_ambiguous_map_format(path: &Path, stream_manager: &StreamManager) -> Result<&'static str> {
    let hit = stream_manager.search_reader_by_content(
        path,
        "ccp4map,xplormap",
        cuemol_molstr::HandlerCategory::ObjReader,
        true,
        0,
    );
    match hit.as_str() {
        "ccp4map" => Ok("ccp4map"),
        "xplormap" => Ok("xplormap"),
        _ => Ok("ccp4map"),
    }
}

fn effective_extension(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "gz" | "bz2" | "xz") {
        return path
            .file_stem()
            .and_then(|value| Path::new(value).extension())
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
    }
    ext
}

fn is_scene_format(format: &str) -> bool {
    matches!(format, "qsc_xml" | "qsc" | "qsl")
}

fn is_desktop_extension_format(format: &str) -> bool {
    matches!(format, "mol2" | "gro" | "psf" | "amberprm")
}

fn is_density_map_format(format: &str) -> bool {
    matches!(format, "apbs" | "ccp4map" | "xplormap" | "brix")
}

fn read_density_map(path: &Path, format: &str) -> Result<DensityMap> {
    let grid = match format {
        "apbs" => read_opendx_file(path)?,
        "ccp4map" => read_ccp4_map_file(path)?,
        "xplormap" => read_xplor_map_file(path)?,
        "brix" => read_brix_map_file(path)?,
        other => {
            return Err(CueMolError::invalid_argument(format!(
                "unknown density map format {other}"
            )))
        }
    };
    Ok(DensityMap::from_grid_map(grid))
}

fn read_extension_molecule(path: &Path, format: &str) -> Result<MolCoord> {
    match format {
        "mol2" => read_mol2_file(path),
        "gro" => read_gro_file(path),
        "psf" => read_psf_file(path),
        "amberprm" => read_amber_prmtop_file(path),
        other => Err(CueMolError::invalid_argument(format!(
            "unknown desktop extension format {other}"
        ))),
    }
}

fn default_object_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let stem = if matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("gz" | "bz2" | "xz")
    ) {
        Path::new(stem)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(stem)
    } else {
        stem
    };
    if stem.trim().is_empty() {
        "object"
    } else {
        stem
    }
    .to_string()
}

fn default_desktop_config() -> cuemol_qsys::SysConfig {
    let mut config = cuemol_qsys::SysConfig::new();
    config.put("config_dir", ".");
    config.put("gui:background", "black");
    config.put("gui:projection", "perspective");
    config
}

fn collect_config_entries(
    prefix: &str,
    section: &cuemol_qsys::SysConfigSection,
    entries: &mut Vec<GuiConfigEntry>,
) {
    let key = if section.name().is_empty() {
        prefix.to_string()
    } else if prefix.is_empty() {
        section.name().to_string()
    } else {
        format!("{prefix}:{}", section.name())
    };
    if section.has_data() && !key.is_empty() {
        entries.push(GuiConfigEntry {
            key: key.clone(),
            value: section.string_data().to_string(),
        });
    }
    for index in 0..section.size() {
        if let Some(child) = section.child(index) {
            collect_config_entries(&key, child, entries);
        }
    }
}

fn residue_one_letter(name: &str) -> char {
    match name.trim().to_ascii_uppercase().as_str() {
        "ALA" => 'A',
        "ARG" => 'R',
        "ASN" => 'N',
        "ASP" => 'D',
        "CYS" => 'C',
        "GLN" => 'Q',
        "GLU" => 'E',
        "GLY" => 'G',
        "HIS" => 'H',
        "ILE" => 'I',
        "LEU" => 'L',
        "LYS" => 'K',
        "MET" => 'M',
        "PHE" => 'F',
        "PRO" => 'P',
        "SER" => 'S',
        "THR" => 'T',
        "TRP" => 'W',
        "TYR" => 'Y',
        "VAL" => 'V',
        "A" | "DA" => 'A',
        "C" | "DC" => 'C',
        "G" | "DG" => 'G',
        "U" => 'U',
        "T" | "DT" => 'T',
        _ => '*',
    }
}

/// Find the atom whose projected screen position is nearest `cursor_px` (within
/// `radius_px`), or `None`. The shared screen-space atom pick used by the GUI
/// front ends: each atom centre is projected with the same column-major `mvp` the
/// offscreen renderer uses, so the projection matches what is drawn. The readback
/// does not flip vertically (image row 0 = NDC y = +1 = top), hence the `1 - y`.
/// Returns the atom's `(object_uid, atom_index)` for `atom_summary` lookup.
///
/// `cursor_px` and `vp_px` must share one pixel basis (both CSS px, or both
/// physical px) — see the front ends' DPI handling. Pure geometry (no GPU): its
/// own oracle is "project an atom, hand its pixel back, get that atom".
pub fn pick_atom_screen(
    atoms: &[cuemol_render::RenderAtom],
    mvp: [[f32; 4]; 4],
    cursor_px: (f64, f64),
    vp_px: (f64, f64),
    radius_px: f64,
) -> Option<(u64, usize)> {
    let mut best: Option<(f64, u64, usize)> = None;
    for atom in atoms {
        let (Some(object_uid), Some(atom_index)) = (atom.object_uid, atom.atom_index) else {
            continue;
        };
        let c = &atom.center;
        let v = [c.x as f32, c.y as f32, c.z as f32, 1.0];
        // column-major mvp * column vector
        let mut clip = [0.0f32; 4];
        for i in 0..4 {
            for col in 0..4 {
                clip[i] += mvp[col][i] * v[col];
            }
        }
        if clip[3].abs() < 1.0e-6 {
            continue;
        }
        let ndc_x = (clip[0] / clip[3]) as f64;
        let ndc_y = (clip[1] / clip[3]) as f64;
        let ndc_z = (clip[2] / clip[3]) as f64;
        if !(-1.0..=1.0).contains(&ndc_z) {
            continue; // outside the depth slab (behind camera / clipped)
        }
        let sx = (ndc_x + 1.0) / 2.0 * vp_px.0;
        let sy = (1.0 - ndc_y) / 2.0 * vp_px.1;
        let dx = sx - cursor_px.0;
        let dy = sy - cursor_px.1;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= radius_px && best.is_none_or(|(b, _, _)| dist < b) {
            best = Some((dist, object_uid, atom_index));
        }
    }
    best.map(|(_, object_uid, atom_index)| (object_uid, atom_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "scripting")]
    #[test]
    fn run_script_js_executes_real_javascript() {
        // The embedded boa engine runs the full JS language (var / arithmetic / string
        // concat), and the `cuemol` binding records print + select. This is real
        // execution, not load+preview.
        let mut app = DesktopAppState::new();
        app.ensure_active_scene();
        let (log, selection) = app
            .run_script_js("var n = 1 + 2; cuemol.print('n=' + n); cuemol.select('resid ' + n);")
            .expect("script runs");
        assert!(log.contains("n=3"), "JS arithmetic + print ran: {log:?}");
        assert_eq!(
            selection.as_deref(),
            Some("resid 3"),
            "the selection was built programmatically in JS"
        );
        // A syntax error surfaces as an Err, not a panic.
        assert!(app.run_script_js("this is not js (((").is_err());
    }

    #[cfg(feature = "scripting")]
    #[test]
    fn run_script_js_makecolor_and_makesel_are_faithful() {
        // Pure builders — no molecule needed. Oracles anchored to C++-true facts:
        // named "red" resolves to (255,0,0,255) in the shared colour table, and a valid
        // selector round-trips while a malformed one yields JS null. Also exercises the
        // real API names putLogMsg / println (aliases of the legacy print).
        let mut app = DesktopAppState::new();
        app.ensure_active_scene();
        let (log, _selection) = app
            .run_script_js(
                "var c = cuemol.makeColor('red'); \
                 cuemol.putLogMsg('red=' + c.r + ',' + c.g + ',' + c.b + ',' + c.a); \
                 var g = cuemol.makeColor('#00ff00'); \
                 cuemol.println('green=' + g.r + ',' + g.g + ',' + g.b); \
                 cuemol.print('sel=' + cuemol.makeSel('resid 3')); \
                 cuemol.print('bad=' + cuemol.makeSel('('));",
            )
            .expect("script runs");
        assert!(log.contains("red=255,0,0,255"), "named red → RGBA: {log:?}");
        assert!(log.contains("green=0,255,0"), "hex #00ff00 → RGB: {log:?}");
        assert!(log.contains("sel=resid 3"), "valid selector round-trips: {log:?}");
        assert!(log.contains("bad=null"), "malformed selector → null: {log:?}");
    }

    #[cfg(feature = "scripting")]
    #[test]
    fn run_script_js_queries_scene_objects() {
        // Structural queries over the read-only snapshot. A loaded PDB is class
        // `MolCoord` (the C++-true class name, 28 refs in ../cuemol2), the object is
        // findable by its load name, its renderer is enumerated, and a missing name → null.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        let scene = app.new_scene("work", false).result_scene;
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        app.new_renderer_in_active_scene(object, "simple", "r1")
            .unwrap();
        assert_eq!(app.active_scene(), Some(scene));

        let (log, _selection) = app
            .run_script_js(
                "cuemol.print('cls=' + cuemol.getClassName('crambin')); \
                 cuemol.print('names=' + cuemol.getObjectNames().join('|')); \
                 var o = cuemol.getObject('crambin'); \
                 cuemol.print('objtype=' + o.type + ' rends=' + o.rendererNames.join(',')); \
                 var r = cuemol.getRenderer('crambin', 'r1'); \
                 cuemol.print('rend=' + r.name + ':' + r.type); \
                 cuemol.print('missing=' + cuemol.getObject('nope')); \
                 var sc = cuemol.getScene(); \
                 cuemol.print('scene=' + sc.name + ' nobj=' + sc.objectNames.length);",
            )
            .expect("script runs");
        assert!(log.contains("cls=MolCoord"), "loaded PDB class: {log:?}");
        assert!(log.contains("crambin"), "object enumerated by name: {log:?}");
        assert!(log.contains("objtype=MolCoord"), "getObject type: {log:?}");
        assert!(log.contains("rends="), "renderer list present: {log:?}");
        assert!(log.contains("r1"), "renderer r1 enumerated: {log:?}");
        assert!(log.contains("rend=r1:"), "getRenderer name: {log:?}");
        assert!(log.contains("missing=null"), "unknown object → null: {log:?}");
        assert!(log.contains("scene=work nobj=1"), "scene name + object count: {log:?}");
    }

    #[cfg(feature = "scripting")]
    #[test]
    fn run_script_js_mutates_scene_by_name() {
        // Flat by-name mutations resolve to Uids at apply time and drive the existing
        // desktop-core setters. Oracles read the effect back through the query APIs:
        // object coloring becomes the C++ RainbowColoring scheme, the renderer is hidden,
        // and its `bondw` inline property (a C++-true attr name) is set.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        app.new_renderer_in_active_scene(object, "ballstick", "r1")
            .unwrap();

        app.run_script_js(
            "cuemol.setColoring('crambin', 'rainbow'); \
             cuemol.setRendererVisible('crambin', 'r1', false); \
             cuemol.setRendererProp('crambin', 'r1', 'bondw', '0.5');",
        )
        .expect("script runs");

        assert_eq!(
            app.object_coloring_type(object).as_deref(),
            Some("RainbowColoring"),
            "setColoring applied the rainbow scheme"
        );
        let renderer = app
            .renderer_summaries(object)
            .into_iter()
            .find(|summary| summary.name == "r1")
            .expect("r1 exists");
        assert!(!renderer.visible, "setRendererVisible hid r1");
        assert_eq!(
            app.renderer_property(object, renderer.uid, "bondw").as_deref(),
            Some("0.5"),
            "setRendererProp set bondw"
        );

        // An unknown object name is a silent no-op — not a panic, not an Err.
        app.run_script_js("cuemol.setColoring('nope', 'bfactor');")
            .expect("no-op on unknown object");
    }

    #[cfg(feature = "scripting")]
    #[test]
    fn run_script_js_measure_matches_active_measure() {
        // cuemol.measure(sel) computes a distance LIVE during eval against the scene's
        // molecules. Oracle: it must equal DesktopAppState::active_measure(sel) for the
        // same selection (the Measure-ribbon value) — a two-atom distance on 1CRN.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        app.load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap();

        let sel = "aid 1 or aid 2"; // exactly two atoms → a distance
        let oracle = app
            .active_measure(sel)
            .expect("active_measure yields a distance for two atoms");
        assert!(oracle.starts_with("Distance:"), "two atoms → distance: {oracle}");

        let (log, _selection) = app
            .run_script_js(&format!("cuemol.print(cuemol.measure('{sel}'));"))
            .expect("script runs");
        assert!(
            log.trim() == oracle,
            "script measure() ({}) equals active_measure ({oracle})",
            log.trim()
        );

        // A selection that isn't 2/3/4 atoms → null (not a panic).
        let (log_all, _) = app
            .run_script_js("cuemol.print('m=' + cuemol.measure('all'));")
            .expect("script runs");
        assert!(log_all.contains("m=null"), "non-measurable selection → null: {log_all:?}");
    }

    #[test]
    fn path_has_extension_routes_png_vs_pov_export() {
        // The predicate that routes "Export scene" between the PNG image path and
        // POV-Ray in all three front ends: case-insensitive, trims, and only true for
        // the actual trailing extension.
        assert!(DesktopAppState::path_has_extension("scene.png", "png"));
        assert!(DesktopAppState::path_has_extension("scene.PNG", "png"));
        assert!(DesktopAppState::path_has_extension("  out/a.Png  ", "png"));
        assert!(!DesktopAppState::path_has_extension("scene.pov", "png"));
        assert!(!DesktopAppState::path_has_extension("scene", "png"));
        assert!(!DesktopAppState::path_has_extension("png", "png"));
    }

    /// Build a pickable atom at a world position (object_uid/atom_index set).
    fn pick_atom_fixture(
        uid: u64,
        index: usize,
        x: f64,
        y: f64,
        z: f64,
    ) -> cuemol_render::RenderAtom {
        cuemol_render::RenderAtom {
            center: cuemol_core::Vector4::new3(x, y, z),
            radius: 1.0,
            color: None,
            object_uid: Some(uid),
            atom_index: Some(index),
            selected: false,
            detail: None,
        }
    }

    #[test]
    fn pick_atom_screen_returns_the_nearest_projected_atom() {
        // Identity column-major mvp -> clip == [x, y, z, 1], so projection is
        // trivial and self-evidently the oracle: ndc == world xyz, screen pixel
        // = ((x+1)/2*w, (1-y)/2*h). No camera_mvp (offscreen feature) needed, so
        // this runs in the default `cargo test --workspace`.
        let mvp = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let vp = (100.0, 100.0);
        let atoms = vec![
            pick_atom_fixture(7, 0, 0.0, 0.0, 0.0), // -> pixel (50, 50)
            pick_atom_fixture(7, 1, 0.4, 0.0, 0.0), // -> pixel (70, 50)
            pick_atom_fixture(9, 9, 0.0, 0.0, 2.0), // ndc_z=2 -> outside slab, skipped
        ];
        // Click near the centre atom.
        assert_eq!(
            pick_atom_screen(&atoms, mvp, (52.0, 50.0), vp, 10.0),
            Some((7, 0))
        );
        // Click near the +x atom.
        assert_eq!(
            pick_atom_screen(&atoms, mvp, (69.0, 50.0), vp, 10.0),
            Some((7, 1))
        );
        // Click far from any projected atom -> nothing.
        assert_eq!(pick_atom_screen(&atoms, mvp, (50.0, 95.0), vp, 10.0), None);
        // The depth-clipped atom is never returned even with a click on its pixel.
        assert_eq!(
            pick_atom_screen(&atoms[2..], mvp, (50.0, 50.0), vp, 10.0),
            None
        );
    }

    #[test]
    fn pick_atom_screen_skips_atoms_without_object_identity() {
        let mvp = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut atom = pick_atom_fixture(0, 0, 0.0, 0.0, 0.0);
        atom.object_uid = None; // overlay/decoration geometry, not a real atom
        assert_eq!(
            pick_atom_screen(&[atom], mvp, (50.0, 50.0), (100.0, 100.0), 10.0),
            None
        );
    }

    #[test]
    fn project_to_image_maps_clip_space_to_pixels() {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        // Origin maps to the image centre.
        assert_eq!(
            project_to_image(id, [0.0, 0.0, 0.0], 800.0, 600.0),
            Some((400.0, 300.0))
        );
        // (0.5, -0.5) → (0.75·w, 0.75·h) — y flips (image origin top-left).
        let (x, y) = project_to_image(id, [0.5, -0.5, 0.0], 800.0, 600.0).unwrap();
        assert!((x - 600.0).abs() < 0.1 && (y - 450.0).abs() < 0.1, "got ({x}, {y})");
        // Off-screen → None.
        assert!(project_to_image(id, [5.0, 0.0, 0.0], 800.0, 600.0).is_none());
    }

    #[test]
    fn anim_spin_direction_sequences_objects() {
        let objects = vec![
            AnimObject { kind: AnimKind::Spin, duration: 1.0 },
            AnimObject { kind: AnimKind::Pause, duration: 1.0 },
        ];
        // In the Spin segment (0..1) → rotate +1.
        let frame = anim_step(&objects, 0.0, 0.1);
        assert!((frame.elapsed - 0.1).abs() < 1e-6 && frame.rotate_dir == 1.0);
        // In the Pause segment (1..2) → no rotation.
        assert_eq!(anim_step(&objects, 1.0, 0.1).rotate_dir, 0.0);
        // Empty list → fallback spin (+1).
        assert_eq!(anim_step(&[], 0.0, 0.1).rotate_dir, 1.0);
        // SlideIn → slide_dir -1 (zoom in); SlideOut → +1.
        let slide = vec![AnimObject { kind: AnimKind::SlideIn, duration: 1.0 }];
        assert_eq!(anim_step(&slide, 0.0, 0.1).slide_dir, -1.0);
    }

    #[test]
    fn trajectory_playback_overwrites_atom_positions() {
        // MolAnim source: a stored trajectory's frame coordinates overwrite the
        // molecule's atom positions when applied by fraction.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.ensure_active_scene();
        if app.open_path(root.join("1CRN.pdb")).is_err() {
            return;
        }
        let object_uid = app.active_object_summaries().first().map(|o| o.uid).unwrap();
        let natom = app
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .mol()
            .atoms()
            .len();
        let frame0 = vec![cuemol_core::Vector4::new3(0.0, 0.0, 0.0); natom];
        let frame1 = vec![cuemol_core::Vector4::new3(5.0, 0.0, 0.0); natom];
        app.set_object_trajectory(object_uid, vec![frame0, frame1]);
        assert!(app.object_has_trajectory(object_uid));
        assert_eq!(app.trajectory_len(object_uid), 2);

        app.apply_trajectory_fraction(object_uid, 1.0); // last frame
        let pos = app
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .mol()
            .atoms()[0]
            .position;
        assert!((pos.x - 5.0).abs() < 1.0e-6, "frame 1 moved atoms to x=5, got {}", pos.x);
    }

    #[test]
    fn active_measure_reports_distance_angle_torsion_by_count() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.ensure_active_scene();
        if app.open_path(root.join("1CRN.pdb")).is_err() {
            return;
        }
        assert!(app.active_measure("aid 1 or aid 5").unwrap().starts_with("Distance:"));
        assert!(app
            .active_measure("aid 1 or aid 5 or aid 9")
            .unwrap()
            .starts_with("Angle:"));
        assert!(app
            .active_measure("aid 1 or aid 5 or aid 9 or aid 13")
            .unwrap()
            .starts_with("Torsion:"));
        assert!(app.active_measure("aid 1").is_none(), "one atom → no measure");
    }

    #[test]
    fn active_sequence_view_marks_selected_residues() {
        // The sequence view returns per-residue 1-letter cells with the selection state
        // computed from the selection expression, so the front ends can sync the
        // sequence highlight to the 3D selection.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.ensure_active_scene();
        if app.open_path(root.join("1CRN.pdb")).is_err() {
            return;
        }
        let chains = app.active_sequence_view("resid 1:3");
        let chain = chains.first().expect("1CRN has one chain");
        assert_eq!(chain.residues.len(), 46, "crambin is 46 residues");
        assert_eq!(chain.residues[0].letter, "T", "1CRN starts with THR (T)");
        assert!(chain.residues[0].selected, "resid 1 is selected");
        assert!(chain.residues[2].selected, "resid 3 is selected");
        assert!(!chain.residues[3].selected, "resid 4 is not selected");
        assert_eq!(chain.residues[0].selection, "chain A and resid 1");
    }

    #[cfg(feature = "offscreen")]
    #[test]
    fn renders_1crn_molecule_to_a_png_for_visual_check() {
        // Produce the ACTUAL molecular view the GUI viewport renders, as a PNG, so
        // it can be eyeballed (self-skips without the reference PDB or a GPU).
        let pdb = std::path::Path::new("/Users/thmini/works/tools/cuemol2/tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut app = DesktopAppState::new();
        app.ensure_active_scene();
        if app.open_path(pdb).is_err() {
            return;
        }
        let out = std::path::Path::new(concat!(
            "/private/tmp/claude-501/-Users-thmini-works-tools-cuemol2-rs/",
            "3e9e9ca1-ce59-4a29-94a8-969339aa0fa7/scratchpad/crn_render.png"
        ));
        match app.export_active_scene_png(out, 600, 600, "") {
            Ok(()) => assert!(out.exists(), "png not written"),
            Err(_) => { /* no GPU adapter; skip */ }
        }
    }

    #[cfg(feature = "offscreen")]
    #[test]
    fn exports_active_scene_to_a_png_file() {
        // Exercises the offscreen render -> PNG -> file path. Self-skips when no
        // GPU adapter is available (the method returns an error).
        let mut app = DesktopAppState::new();
        app.ensure_active_scene();
        let path =
            std::env::temp_dir().join(format!("cuemol2-rs-export-{}.png", std::process::id()));
        match app.export_active_scene_png(&path, 32, 32, "") {
            Ok(()) => {
                let bytes = std::fs::read(&path).expect("read exported png");
                assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']), "not a PNG");
                let _ = std::fs::remove_file(&path);
            }
            Err(_) => { /* no GPU adapter in this environment; skip */ }
        }
    }

    #[test]
    fn desktop_app_state_exposes_command_and_stream_facades() {
        let mut app = DesktopAppState::new();
        assert_eq!(app.scene_manager().scene_count(), 0);
        assert!(app
            .stream_manager()
            .handler_by_nickname("pdb", cuemol_molstr::HandlerCategory::ObjReader)
            .is_some());

        let result = app.new_scene("work", true);
        assert_eq!(app.active_scene(), Some(result.result_scene));
        assert_eq!(app.scene_manager().scene_count(), 1);
        assert!(result.result_view.is_some());
        assert!(app.set_active_scene(None));
        assert_eq!(app.active_scene(), None);
        assert!(!app.set_active_scene(Some(Uid::new(999))));

        app.put_config_value("gui:test", "ok");
        assert_eq!(app.config_value("gui:test"), "ok");
        assert!(app
            .config_entries()
            .iter()
            .any(|entry| entry.key == "gui:test" && entry.value == "ok"));
    }

    #[test]
    fn desktop_app_state_saves_and_loads_config_file() {
        let dir = std::env::temp_dir().join(format!("cuemol-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cuemol.conf");
        let mut app = DesktopAppState::new();
        app.put_config_value("gui:test", "line1\nline2");
        app.save_config_file(&path).unwrap();

        let mut loaded = DesktopAppState::new();
        assert_eq!(loaded.load_config_file(&path).unwrap(), 4);
        assert_eq!(loaded.config_value("gui:test"), "line1\nline2");
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn desktop_app_state_loads_object_and_renderer_into_active_scene() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        let scene = app.new_scene("work", false).result_scene;
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        assert_eq!(app.active_scene(), Some(scene));
        assert_eq!(
            app.scene_manager()
                .scene(scene)
                .unwrap()
                .object(object)
                .unwrap()
                .mol()
                .atom_count(),
            327
        );

        let renderer = app
            .new_renderer_in_active_scene(object, "simple", "r1")
            .unwrap()
            .result_renderer;
        assert!(app
            .scene_manager()
            .scene(scene)
            .unwrap()
            .object(object)
            .unwrap()
            .renderer(renderer)
            .is_some());
        assert_eq!(app.active_render_snapshot("").unwrap().atom_count(), 327);
        assert!(
            app.renderer_summaries(object)
                .iter()
                .find(|summary| summary.uid == renderer)
                .unwrap()
                .visible
        );
        app.set_renderer_visible(object, renderer, false).unwrap();
        assert_eq!(app.active_render_snapshot("").unwrap().atom_count(), 0);
        app.set_renderer_visible(object, renderer, true).unwrap();
        assert_eq!(app.active_render_snapshot("").unwrap().atom_count(), 327);
        assert!(app.destroy_renderer_in_object(object, renderer).unwrap());
        assert_eq!(app.active_render_snapshot("").unwrap().atom_count(), 327);

        let atom = app.atom_summary(object, 0).unwrap();
        assert_eq!(atom.object_uid, object);
        assert_eq!(atom.atom_index, 0);
        assert_eq!(atom.serial, 1);
        assert_eq!(atom.selection_text, "aid 1");
    }

    #[test]
    fn surface_mesh_is_coloured_by_nearest_atom_not_flat_grey() {
        // CueMol2 MolSurfRenderer MOLFANC: each SES vertex takes its nearest atom's
        // colour-scheme colour (getColorMol). The Rust surface was a flat-grey blob
        // in all 3 GUIs; this pins the fix — a real triangle mesh whose vertices
        // carry the per-element CPK colours of the residues under them.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        // A small selection (residues 1-3 = THR/THR/CYS, exposing C/N/O/S) keeps the
        // SES build cheap while still surfacing several element colours.
        let mesh = app.build_surface_mesh(object, "resid 1:3", 1.0, 1.4).unwrap();
        assert!(
            !mesh.indices.is_empty() && !mesh.vertices.is_empty(),
            "the SES must be a real triangle mesh"
        );
        // Quantise to bytes so we can count distinct colours — the flat-grey
        // regression would collapse this to a single colour.
        let quant = |c: f32| (c * 255.0).round() as u8;
        let distinct: std::collections::HashSet<[u8; 3]> = mesh
            .vertices
            .iter()
            .map(|v| [quant(v.color[0]), quant(v.color[1]), quant(v.color[2])])
            .collect();
        assert!(
            distinct.len() > 1,
            "surface must be multi-coloured (nearest-atom), not flat grey; got {distinct:?}"
        );
        // The specific CPK colours of exposed oxygen and carbon must appear, and the
        // former uniform grey (0.76,0.79,0.84) must be gone.
        let has = |target: [f32; 3]| {
            mesh.vertices.iter().any(|v| {
                (0..3).all(|k| (v.color[k] - target[k]).abs() < 0.02)
            })
        };
        assert!(has([0.92, 0.12, 0.12]), "oxygen-red vertices expected on the SES");
        assert!(has([0.38, 0.38, 0.38]), "carbon-grey vertices expected on the SES");
        assert!(
            !has([0.76, 0.79, 0.84]),
            "no vertex should keep the old flat-grey surface colour"
        );
    }

    #[test]
    fn potential_color_is_a_red_white_blue_gradient() {
        // C++ getColorSca 3-stop gradient: low → red, mid → white, high → blue.
        let approx = |a: [f32; 4], b: [f32; 4]| {
            (0..4).all(|k| (a[k] - b[k]).abs() < 1.0e-6)
        };
        assert!(approx(potential_color(-1.0, -1.0, 0.0, 1.0), [1.0, 0.0, 0.0, 1.0]));
        assert!(approx(potential_color(1.0, -1.0, 0.0, 1.0), [0.0, 0.0, 1.0, 1.0]));
        assert!(approx(potential_color(0.0, -1.0, 0.0, 1.0), [1.0, 1.0, 1.0, 1.0]));
        // Halfway on each half.
        assert!(approx(potential_color(-0.5, -1.0, 0.0, 1.0), [1.0, 0.5, 0.5, 1.0]));
        assert!(approx(potential_color(0.5, -1.0, 0.0, 1.0), [0.5, 0.5, 1.0, 1.0]));
        // Beyond the ends clamps to the endpoint colour.
        assert!(approx(potential_color(-9.0, -1.0, 0.0, 1.0), [1.0, 0.0, 0.0, 1.0]));
        assert!(approx(potential_color(9.0, -1.0, 0.0, 1.0), [0.0, 0.0, 1.0, 1.0]));
    }

    #[test]
    fn surface_potential_thresholds_are_robust_and_user_settable() {
        // The SCAPOT gradient must NOT auto-scale to the field min/max: real APBS maps
        // have near-atom potential SPIKES that set min/max and wash the boundary surface
        // to near-white. Default to a robust mid±2·rmsd; let the user clamp it tighter.
        let mut app = DesktopAppState::new();
        // Bulk potential ±1 with two extreme spikes (±100): min/max = ±100, rmsd ≪ 100.
        let n = 10usize;
        let mut values = vec![0.0_f64; n * n * n];
        for (i, v) in values.iter_mut().enumerate() {
            *v = if i % 2 == 0 { 1.0 } else { -1.0 };
        }
        values[0] = 100.0;
        values[1] = -100.0;
        app.set_surface_potential_map(DensityMap::from_grid_map(GridMap {
            counts: [n, n, n],
            origin: cuemol_core::Vector4::ZERO,
            delta: [
                cuemol_core::Vector4::new3(1.0, 0.0, 0.0),
                cuemol_core::Vector4::new3(0.0, 1.0, 0.0),
                cuemol_core::Vector4::new3(0.0, 0.0, 1.0),
            ],
            values,
        }));
        let (min, max) = app.surface_potential_range().unwrap();
        assert!(min <= -99.0 && max >= 99.0, "field range is spike-dominated (±100)");
        // Auto thresholds (2·rmsd around 0) ignore the spikes — FAR inside ±100.
        let (lo, mid, hi) = app.surface_potential_thresholds().unwrap();
        assert_eq!(mid, 0.0, "neutral mid at 0 for a straddling map");
        assert!(hi < 90.0 && lo > -90.0, "auto thresholds ignore the ±100 spikes: ({lo},{hi})");
        assert!((hi + lo).abs() < 1.0e-6, "symmetric around the neutral mid");
        // A user scale clamps the gradient explicitly (C++ m_dParLow/High).
        app.set_potential_scale(Some(1.0));
        let (lo2, _m, hi2) = app.surface_potential_thresholds().unwrap();
        assert!((hi2 - 1.0).abs() < 1.0e-9 && (lo2 + 1.0).abs() < 1.0e-9, "user scale ±1");
        // At ±1 scale a potential of 1.0 is now FULLY blue; under the field range it
        // would wash to near-white — the low-contrast bug this fix closes.
        assert_eq!(potential_color(1.0, lo2, 0.0, hi2), [0.0, 0.0, 1.0, 1.0]);
        let washed = potential_color(1.0, min, 0.0, max);
        assert!(washed[0] > 0.9 && washed[2] > 0.9, "field-range scaling washes to near-white");
        app.set_potential_scale(Some(-5.0));
        assert!(app.potential_scale().is_none(), "a non-positive scale restores auto");
    }

    #[test]
    fn surface_potential_mesh_colours_by_electrostatic_potential() {
        // SCAPOT: a molecular surface coloured by an electrostatic potential field.
        // Synthesise a potential that ramps -x→+x through 0 at the selected residues'
        // centroid, so the SES over them spans red (negative) → blue (positive).
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        // Centroid of residues 1-3 (the surface selection).
        let centroid = {
            let mol = app
                .scene_manager()
                .object(object)
                .unwrap()
                .mol()
                .atoms()
                .iter()
                .filter(|a| (1..=3).contains(&a.res_id))
                .map(|a| a.position)
                .collect::<Vec<_>>();
            let n = mol.len() as f64;
            let sum = mol
                .iter()
                .fold(cuemol_core::Vector4::ZERO, |acc, p| acc + *p);
            cuemol_core::Vector4::new3(sum.x / n, sum.y / n, sum.z / n)
        };
        // An 80 Å cube of 1 Å voxels around the centroid; value = x − centroid.x, so
        // the field straddles 0 (min ≈ −40, max ≈ +40) → mid pins to 0.
        let n = 80usize;
        let origin = cuemol_core::Vector4::new3(
            centroid.x - 40.0,
            centroid.y - 40.0,
            centroid.z - 40.0,
        );
        let mut values = vec![0.0_f64; n * n * n];
        for sec in 0..n {
            for row in 0..n {
                for col in 0..n {
                    let world_x = origin.x + col as f64;
                    values[col + (row + sec * n) * n] = world_x - centroid.x;
                }
            }
        }
        let grid = cuemol_xtal::GridMap {
            counts: [n, n, n],
            origin,
            delta: [
                cuemol_core::Vector4::new3(1.0, 0.0, 0.0),
                cuemol_core::Vector4::new3(0.0, 1.0, 0.0),
                cuemol_core::Vector4::new3(0.0, 0.0, 1.0),
            ],
            values,
        };
        app.set_surface_potential_map(DensityMap::from_grid_map(grid));
        assert!(app.has_surface_potential());
        let (lo, hi) = app.surface_potential_range().unwrap();
        assert!(lo < 0.0 && hi > 0.0, "potential range must straddle 0: ({lo},{hi})");

        let mesh = app
            .build_surface_potential_mesh(object, "resid 1:3", 1.0, 1.4)
            .unwrap();
        assert!(!mesh.indices.is_empty(), "the SES must be a real mesh");
        // The surface must span the neutral point: some vertices lean red (negative
        // side, more red than blue) and some lean blue (positive side). Tested by the
        // colour's SIGN tint, not saturation — the auto-scaled range is dominated by
        // the grid corners, so a small local surface is tinted, not saturated.
        let red = mesh
            .vertices
            .iter()
            .any(|v| v.color[0] > v.color[2] + 0.02);
        let blue = mesh
            .vertices
            .iter()
            .any(|v| v.color[2] > v.color[0] + 0.02);
        assert!(red, "surface must have red-leaning (negative-potential) vertices");
        assert!(blue, "surface must have blue-leaning (positive-potential) vertices");

        // With no potential loaded the call is a clean error, not a panic.
        let mut empty = DesktopAppState::new();
        empty.new_scene("w", false);
        let obj2 = empty
            .load_object_into_active_scene(root.join("1CRN.pdb"), "c", "pdb")
            .unwrap()
            .result_object;
        assert!(empty
            .build_surface_potential_mesh(obj2, "resid 1:3", 1.0, 1.4)
            .is_err());
    }

    #[test]
    fn render_snapshot_applies_a_renderers_resolved_style_geometry() {
        // The GUI render snapshot must honour a renderer's `style="..."` preset,
        // not just its per-type defaults (the gap this closes — egui/gpui/dioxus all
        // route geometry through `active_render_snapshot`). Discriminator: a
        // ballstick renderer draws bonds at the 0.08 type default until tagged with
        // `DefaultBallStick`, whose bundled preset is bondw=0.2 / sphr=0.3.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        let renderer = app
            .new_renderer_in_active_scene(object, "ballstick", "bs")
            .unwrap()
            .result_renderer;

        // Before styling: the hardcoded per-type ballstick look (bond radius 0.08).
        let before = app.active_render_snapshot("").unwrap();
        assert!(before.bond_count() > 0, "ballstick should draw bonds");
        assert!(
            (before.bonds[0].radius - 0.08).abs() < 1e-6,
            "unstyled ballstick bond radius should be the 0.08 default, got {}",
            before.bonds[0].radius
        );

        // Tag the renderer with the bundled DefaultBallStick preset.
        app.scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .renderer_mut(renderer)
            .unwrap()
            .set_qsc_attr("style", "DefaultBallStick");

        // After styling: every bond at the preset bondw=0.2 and every atom forced
        // to the absolute sphr=0.3 (atom_radius_abs overrides the vdW-scaled radius).
        let after = app.active_render_snapshot("").unwrap();
        assert!(
            after.bonds.iter().all(|bond| (bond.radius - 0.2).abs() < 1e-6),
            "styled bonds should all be bondw=0.2, first was {}",
            after.bonds[0].radius
        );
        assert!(
            after.atoms.iter().all(|atom| (atom.radius - 0.3).abs() < 1e-6),
            "styled atoms should all be the absolute sphr=0.3, first was {}",
            after.atoms[0].radius
        );
    }

    #[test]
    fn atomintr_renderer_draws_measurement_lines_not_the_whole_molecule() {
        // An atom-interaction ("measure") renderer must draw a thin line between
        // each of its atom pairs, NOT a sphere per atom of the whole molecule.
        // Regression for the bug where "atomintr" (and the `*selection` overlay)
        // fell through to molecule_render_snapshot and dumped every atom (286
        // spheres for this fixture's two measure renderers).
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let fixture = root.join("number_chain_atomintr1.qsc");
        if !fixture.exists() {
            return;
        }
        let mut app = DesktopAppState::new();
        let Ok(Some(scene_uid)) = app.load_qsc_file(&fixture) else {
            return;
        };

        let snapshot = app.render_snapshot(scene_uid, "").unwrap();
        // Scene = a trace ribbon (mesh) + `*selection` (skipped) + two atomintr
        // renderers (one <line> each). After the fix: zero atom spheres, exactly
        // two measurement-line bonds, plus the trace ribbon mesh.
        assert_eq!(
            snapshot.atom_count(),
            0,
            "atomintr/`*selection` must not dump atom spheres"
        );
        assert_eq!(
            snapshot.bond_count(),
            2,
            "one measurement line per atomintr renderer"
        );
        assert!(
            snapshot.mesh_count() >= 1,
            "the trace renderer still contributes a ribbon mesh"
        );

        // Endpoint oracle: `measure2` is <line aid1="43" aid2="88"/>, so one
        // measurement bond must connect the resolved positions of atoms 43 and 88.
        let scene = app.scene_manager().scene(scene_uid).unwrap();
        let object = scene.objects().first().expect("scene has a molecule object");
        let atoms = object.mol().atoms();
        let p43 = atoms[43].position;
        let p88 = atoms[88].position;
        let close = |u: cuemol_core::Vector4, v: cuemol_core::Vector4| (u - v).length3d() < 1e-6;
        assert!(
            snapshot.bonds.iter().any(|bond| {
                (close(bond.from, p43) && close(bond.to, p88))
                    || (close(bond.from, p88) && close(bond.to, p43))
            }),
            "a measurement line must connect the resolved atoms 43 and 88"
        );
    }

    #[test]
    fn disorder_renderer_draws_gap_lines_not_the_whole_molecule() {
        // A "disorder" renderer draws dotted Bézier lines bridging unbonded chain
        // gaps, NOT a sphere per atom. On 1CRN (a single fully-bonded chain) it
        // should therefore draw (almost) nothing — far fewer than the 327 spheres
        // a generic atom dump produced before the dispatch fix.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        app.new_renderer_in_active_scene(object, "disorder", "diso")
            .unwrap();

        let snapshot = app.active_render_snapshot("").unwrap();
        // Routes to the disorder geometry (matches the molvis function), which is
        // far below the 327-atom generic dump.
        let direct = disorder_lines_snapshot(
            app.scene_manager()
                .object(object)
                .unwrap()
                .mol(),
            1.0,
            0.15,
            2.0,
            -1.0,
        );
        assert_eq!(
            snapshot.atom_count(),
            direct.atom_count(),
            "disorder renderer must route to the disorder geometry"
        );
        assert!(
            snapshot.atom_count() < 100,
            "disorder on a fully-bonded chain must not dump all 327 atoms, got {}",
            snapshot.atom_count()
        );
    }

    #[test]
    fn anisou_renderer_draws_thermal_ellipsoids() {
        // An "anisou" renderer draws an anisotropic displacement ellipsoid mesh per
        // ANISOU atom, not a plain sphere primitive. Build a tiny two-atom molecule
        // with ANISOU records (U values placed at the exact PDB columns the reader
        // reads: serial in 7-11, U11.. from column 29) and confirm the snapshot is
        // two ellipsoid meshes, zero atom spheres.
        let mut pdb = String::new();
        pdb.push_str(
            "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C\n",
        );
        pdb.push_str(
            "ATOM      2  CB  ALA A   1       1.500   0.000   0.000  1.00 10.00           C\n",
        );
        for serial in [1, 2] {
            let mut line = format!("ANISOU{serial:>5}  CA  ALA A   1");
            while line.len() < 28 {
                line.push(' ');
            }
            for u in [1000, 1000, 1000, 0, 0, 0] {
                line.push_str(&format!("{u:>7}"));
            }
            pdb.push_str(&line);
            pdb.push('\n');
        }
        pdb.push_str("END\n");
        let path =
            std::env::temp_dir().join(format!("cuemol2-rs-anisou-{}.pdb", std::process::id()));
        std::fs::write(&path, &pdb).unwrap();

        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(&path, "adp", "pdb")
            .unwrap()
            .result_object;
        // Sanity: the ANISOU records actually parsed onto the atoms.
        assert!(
            app.scene_manager()
                .object(object)
                .unwrap()
                .mol()
                .atoms()[0]
                .int_prop("anisou_u11")
                .is_some(),
            "ANISOU U values must have parsed (else the test verifies nothing)"
        );
        app.new_renderer_in_active_scene(object, "anisou", "adp")
            .unwrap();

        let snapshot = app.active_render_snapshot("").unwrap();
        assert_eq!(
            snapshot.mesh_count(),
            2,
            "one thermal-ellipsoid mesh per ANISOU atom"
        );
        assert_eq!(
            snapshot.atom_count(),
            0,
            "ANISOU atoms render as ellipsoid meshes, not sphere primitives"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn renderer_sel_restricts_drawn_atoms_to_its_selection() {
        // A renderer's `sel="..."` restricts what it draws (e.g. ballstick scoped
        // to a ligand). Without it the GUI drew the whole molecule for every
        // renderer; with it, only the selected atoms.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        let renderer = app
            .new_renderer_in_active_scene(object, "ballstick", "bs")
            .unwrap()
            .result_renderer;

        // No sel: the whole 327-atom molecule.
        assert_eq!(app.active_render_snapshot("").unwrap().atom_count(), 327);

        // sel="resid 1": only the first residue's atoms — a strict, non-empty subset.
        app.scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .renderer_mut(renderer)
            .unwrap()
            .set_qsc_attr("sel", "resid 1");
        let expected = app
            .scene_manager()
            .object(object)
            .unwrap()
            .mol()
            .select_indices(&parse_selector("resid 1").unwrap())
            .len();
        assert!(
            expected > 0 && expected < 327,
            "resid 1 should be a strict non-empty subset, got {expected}"
        );
        assert_eq!(
            app.active_render_snapshot("").unwrap().atom_count(),
            expected,
            "ballstick must draw only its sel'd atoms"
        );
    }

    #[test]
    fn renderer_sel_restricts_the_ribbon_trace_to_its_residues() {
        // The mainchain/ribbon path also honours a renderer's `sel`: a sel'd trace
        // splines only its residues' backbone, not the whole chain.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        let renderer = app
            .new_renderer_in_active_scene(object, "trace", "tr")
            .unwrap()
            .result_renderer;

        let full: usize = app
            .active_render_snapshot("")
            .unwrap()
            .meshes
            .iter()
            .map(|mesh| mesh.vertices.len())
            .sum();
        assert!(full > 0, "the full backbone trace should produce a ribbon mesh");

        app.scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .renderer_mut(renderer)
            .unwrap()
            .set_qsc_attr("sel", "resid 1:10");
        let restricted: usize = app
            .active_render_snapshot("")
            .unwrap()
            .meshes
            .iter()
            .map(|mesh| mesh.vertices.len())
            .sum();
        assert!(
            restricted > 0 && restricted < full,
            "a sel'd trace should cover fewer residues: full={full} restricted={restricted}"
        );
    }

    #[test]
    fn putty_modulates_ribbon_width_by_bfactor() {
        // A trace renderer with putty_mode="scale1" modulates the backbone tube
        // width per residue by the B-factor (TubeRenderer putty). 1CRN has varying
        // B-factors, so the puttied ribbon mesh differs from the unmodulated one
        // (same topology, moved cross-section vertices).
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        let renderer = app
            .new_renderer_in_active_scene(object, "trace", "tr")
            .unwrap()
            .result_renderer;

        let vertices = |app: &DesktopAppState| -> Vec<cuemol_core::Vector4> {
            app.active_render_snapshot("")
                .unwrap()
                .meshes
                .iter()
                .flat_map(|mesh| mesh.vertices.iter().map(|vertex| vertex.position))
                .collect()
        };
        let base = vertices(&app);
        assert!(!base.is_empty(), "the trace should produce a ribbon mesh");

        app.scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .renderer_mut(renderer)
            .unwrap()
            .set_qsc_attr("putty_mode", "scale1");
        let puttied = vertices(&app);
        assert_eq!(
            base.len(),
            puttied.len(),
            "putty changes widths, not topology"
        );
        assert_ne!(
            base, puttied,
            "putty must modulate the ribbon width (B-factor varies in 1CRN)"
        );
    }

    #[test]
    fn styled_sphere_detail_refines_the_gui_tessellation() {
        // A renderer style's sphere `detail` now reaches the GUI tessellation: the
        // snapshot's atoms carry the detail and `tessellate_snapshot` honours it,
        // so DefaultBallStick (detail=10) produces finer spheres (more vertices)
        // than the unstyled default.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap()
            .result_object;
        let renderer = app
            .new_renderer_in_active_scene(object, "ballstick", "bs")
            .unwrap()
            .result_renderer;

        let tessellated_verts = |app: &DesktopAppState| {
            cuemol_render::tessellate_snapshot(&app.active_render_snapshot("").unwrap())
                .vertices
                .len()
        };
        let base = tessellated_verts(&app);

        app.scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .renderer_mut(renderer)
            .unwrap()
            .set_qsc_attr("style", "DefaultBallStick");
        let detailed = tessellated_verts(&app);
        assert!(
            detailed > base,
            "styled detail=10 should refine the spheres: base={base} detailed={detailed}"
        );
    }

    #[test]
    fn nucl_renderer_adds_base_sticks_to_the_backbone_ribbon() {
        // A nucleic-acid renderer draws the backbone ribbon PLUS each nucleotide's
        // base stick (C++ NARenderer), which the plain mainchain ribbon omits.
        // Build two purine nucleotides (C1'/N9/N1) by exact PDB columns.
        fn pdb_atom(serial: i32, name: &str, res: &str, resid: i32, x: f64, y: f64, z: f64) -> String {
            let mut line = vec![b' '; 80];
            let put = |line: &mut [u8], text: &str, start: usize| {
                for (i, byte) in text.bytes().enumerate() {
                    line[start + i] = byte;
                }
            };
            put(&mut line, "ATOM", 0);
            put(&mut line, &format!("{serial:>5}"), 6);
            put(&mut line, &format!("{name:<4}"), 12);
            put(&mut line, &format!("{res:>3}"), 17);
            put(&mut line, "A", 21);
            put(&mut line, &format!("{resid:>4}"), 22);
            put(&mut line, &format!("{x:>8.3}"), 30);
            put(&mut line, &format!("{y:>8.3}"), 38);
            put(&mut line, &format!("{z:>8.3}"), 46);
            String::from_utf8(line).unwrap()
        }
        let mut pdb = String::new();
        for (resid, base_x) in [(1, 0.0), (2, 5.0)] {
            let serial = (resid - 1) * 3;
            pdb.push_str(&pdb_atom(serial + 1, "C1'", "G", resid, base_x, 0.0, 0.0));
            pdb.push('\n');
            pdb.push_str(&pdb_atom(serial + 2, "N9", "G", resid, base_x + 1.0, 0.0, 0.0));
            pdb.push('\n');
            pdb.push_str(&pdb_atom(serial + 3, "N1", "G", resid, base_x + 2.0, 0.0, 0.0));
            pdb.push('\n');
        }
        pdb.push_str("END\n");
        let path = std::env::temp_dir().join(format!("cuemol2-rs-nucl-{}.pdb", std::process::id()));
        std::fs::write(&path, &pdb).unwrap();

        let mut app = DesktopAppState::new();
        app.new_scene("work", false);
        let object = app
            .load_object_into_active_scene(&path, "na", "pdb")
            .unwrap()
            .result_object;
        // Sanity: the nucleotide atoms parsed (C1' present).
        assert!(
            app.scene_manager()
                .object(object)
                .unwrap()
                .mol()
                .atoms()
                .iter()
                .any(|atom| atom.name.trim() == "C1'"),
            "the C1' atoms must have parsed (else the test verifies nothing)"
        );

        // A plain trace draws the ribbon backbone only (no base-stick bonds).
        app.new_renderer_in_active_scene(object, "trace", "tr").unwrap();
        let trace = app.active_render_snapshot("").unwrap();
        assert_eq!(trace.bond_count(), 0, "a plain trace has no base sticks");

        // Replace it with a nucl renderer: ribbon mesh + base-stick bonds.
        app.new_scene("work2", false);
        let object = app
            .load_object_into_active_scene(&path, "na", "pdb")
            .unwrap()
            .result_object;
        app.new_renderer_in_active_scene(object, "nucl", "na").unwrap();
        let nucl = app.active_render_snapshot("").unwrap();
        assert!(
            nucl.mesh_count() >= 1,
            "nucl still draws the backbone ribbon mesh"
        );
        assert!(
            nucl.bond_count() >= 4,
            "nucl adds two base sticks per nucleotide, got {}",
            nucl.bond_count()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn paint_coloring_colors_the_ribbon_by_object_paint() {
        // The scene's object-level `<coloring type="PaintColoring">` colours the
        // backbone ribbon (the trace's CPK carbon CA defers to $molcol = the object
        // paint). FloralWhite (#FFFAF0) is the paint "*" fallback — a colour the
        // default element/secondary-structure colouring never produces, so its
        // appearance proves paint is wired; clearing the paint removes it.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let fixture = root.join("number_chain_atomintr1.qsc");
        if !fixture.exists() {
            return;
        }
        let mut app = DesktopAppState::new();
        let Ok(Some(scene_uid)) = app.load_qsc_file(&fixture) else {
            return;
        };
        let object_uid = app
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .objects()
            .first()
            .expect("scene has a molecule object")
            .uid();
        // Sanity: the PaintColoring actually parsed (else the test verifies nothing).
        assert!(
            !app.scene_manager()
                .object(object_uid)
                .unwrap()
                .paint_coloring()
                .is_empty(),
            "the object's PaintColoring must have parsed"
        );

        let has_floral = |snapshot: &RenderSceneSnapshot| {
            snapshot.meshes.iter().flat_map(|mesh| mesh.vertices.iter()).any(|vertex| {
                (vertex.color[0] - 1.0).abs() < 0.005
                    && (vertex.color[1] - 250.0 / 255.0).abs() < 0.005
                    && (vertex.color[2] - 240.0 / 255.0).abs() < 0.005
            })
        };
        assert!(
            has_floral(&app.render_snapshot(scene_uid, "").unwrap()),
            "the ribbon should show the FloralWhite paint fallback colour"
        );

        // Clear the paint coloring: the ribbon falls back to section colours.
        app.scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .set_paint_coloring(Vec::new());
        assert!(
            !has_floral(&app.render_snapshot(scene_uid, "").unwrap()),
            "without paint coloring the FloralWhite must disappear"
        );
    }

    #[test]
    fn loading_a_qsc_restores_its_saved_view_camera() {
        // A loaded .qsc carries a saved `__current` camera; the viewer/export must
        // restore it (the scene's view) instead of auto-framing. A plain PDB has
        // no saved camera, so those still auto-frame.
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let fixture = root.join("number_chain_atomintr1.qsc");
        if !fixture.exists() {
            return;
        }
        let mut app = DesktopAppState::new();
        if app.load_qsc_file(&fixture).ok().flatten().is_none() {
            return;
        }
        let camera = app
            .active_scene_saved_camera()
            .expect("the .qsc's saved camera must be restored");
        // Exact values from the fixture's <camera center=(7.2782,10.1857,6.8747)
        // zoom=50 perspec=false>.
        let center = camera.center();
        assert!((center.x - 7.2782).abs() < 1e-3, "center.x = {}", center.x);
        assert!((center.y - 10.1857).abs() < 1e-3, "center.y = {}", center.y);
        assert!((center.z - 6.8747).abs() < 1e-3, "center.z = {}", center.z);
        assert!((camera.zoom() - 50.0).abs() < 1e-6, "zoom = {}", camera.zoom());
        assert!(!camera.is_perspective(), "fixture camera is orthographic");

        // A plain PDB load has no saved camera (export auto-frames it).
        let mut pdb_app = DesktopAppState::new();
        pdb_app.new_scene("work", false);
        pdb_app
            .load_object_into_active_scene(root.join("1CRN.pdb"), "crambin", "pdb")
            .unwrap();
        assert!(
            pdb_app.active_scene_saved_camera().is_none(),
            "a PDB load defines no saved camera"
        );
    }

    #[test]
    fn object_visible_false_hides_the_whole_object() {
        // C++ `Object::visible` (Object.qif:33, default true) is a higher-level
        // gate above per-renderer visibility: `Scene::displayRendImpl` skips an
        // invisible object entirely (Scene.cpp:635). The render snapshot must
        // honour the .qsc `<object visible="false">` attribute (the gap this
        // closes — the path was renderer-visible-only). Oracle is INDEPENDENT of
        // the fixture's attribute form: the discriminator is the atom COUNT
        // (6 -> 0), not the parsed flag.
        fn pdb_ca(serial: i32, resid: i32, x: f64) -> String {
            let mut line = vec![b' '; 80];
            let put = |line: &mut [u8], text: &str, start: usize| {
                for (i, byte) in text.bytes().enumerate() {
                    line[start + i] = byte;
                }
            };
            put(&mut line, "ATOM", 0);
            put(&mut line, &format!("{serial:>5}"), 6);
            put(&mut line, "CA", 12);
            put(&mut line, "ALA", 17);
            put(&mut line, "A", 21);
            put(&mut line, &format!("{resid:>4}"), 22);
            put(&mut line, &format!("{x:>8.3}"), 30);
            put(&mut line, &format!("{:>8.3}", 0.0), 38);
            put(&mut line, &format!("{:>8.3}", 0.0), 46);
            String::from_utf8(line).unwrap()
        }
        let mut pdb = String::new();
        for resid in 1..=6 {
            pdb.push_str(&pdb_ca(resid, resid, (resid - 1) as f64 * 3.8));
            pdb.push('\n');
        }
        // `visible_attr` toggles only the one attribute the gap is about.
        let build = |visible_attr: &str| {
            format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<scene>\n\
                 \t<qsc_opts base64=\"false\" compress=\"none\" version=\"QDF0\"/>\n\
                 \t<object type=\"MolCoord\" name=\"mol\" src=\"datachunk:00000\" srctype=\"qdfpdb\"{visible_attr}>\n\
                 \t\t<renderer type=\"cpk\" name=\"cpk1\"/>\n\
                 \t</object>\n</scene>\n\n========== End of XML ==========\n\n\
                 ==========START datachunk:00000==========\n{pdb}\n==========END datachunk:00000==========\n"
            )
        };
        let render_atom_count = |visible_attr: &str| {
            let qsc = build(visible_attr);
            let path = std::env::temp_dir()
                .join(format!("cuemol2-rs-objvis-{}-{visible_attr:?}.qsc", std::process::id()));
            std::fs::write(&path, &qsc).unwrap();
            let mut app = DesktopAppState::new();
            let scene_uid = app.load_qsc_file(&path).ok().flatten().expect("synthetic .qsc loads");
            let _ = std::fs::remove_file(&path);
            let object = app.scene_manager().scene(scene_uid).unwrap().objects().first().unwrap();
            let parsed_visible = object.is_visible();
            let atoms = app.render_snapshot(scene_uid, "").unwrap().atom_count();
            (parsed_visible, atoms)
        };

        // Control: a visible object (no attribute = C++ default true) draws its atoms.
        let (vis_flag, vis_atoms) = render_atom_count("");
        assert!(vis_flag, "default (no attr) object is visible");
        assert_eq!(vis_atoms, 6, "a visible object renders all 6 CA atoms");

        // The gap: `visible="false"` hides the whole object -> no geometry.
        let (hidden_flag, hidden_atoms) = render_atom_count(" visible=\"false\"");
        assert!(!hidden_flag, "visible=\"false\" parses as not visible");
        assert_eq!(hidden_atoms, 0, "a hidden object contributes no atoms (Scene.cpp:635)");
    }

    #[test]
    fn rainbow_object_coloring_gradient_colors_the_ribbon() {
        // A synthetic .qsc whose object declares <coloring type="RainbowColoring"
        // start_hue=240 end_hue=0> colours the backbone ribbon by a blue->red hue
        // gradient (C++ RainbowColoring). Hand-built fixture (no human .qsc needed),
        // same technique as the ANISOU/NA synthetic fixtures.
        fn pdb_ca(serial: i32, resid: i32, x: f64) -> String {
            let mut line = vec![b' '; 80];
            let put = |line: &mut [u8], text: &str, start: usize| {
                for (i, byte) in text.bytes().enumerate() {
                    line[start + i] = byte;
                }
            };
            put(&mut line, "ATOM", 0);
            put(&mut line, &format!("{serial:>5}"), 6);
            put(&mut line, "CA", 12);
            put(&mut line, "ALA", 17);
            put(&mut line, "A", 21);
            put(&mut line, &format!("{resid:>4}"), 22);
            put(&mut line, &format!("{x:>8.3}"), 30);
            put(&mut line, &format!("{:>8.3}", 0.0), 38);
            put(&mut line, &format!("{:>8.3}", 0.0), 46);
            String::from_utf8(line).unwrap()
        }
        let mut pdb = String::new();
        for resid in 1..=6 {
            pdb.push_str(&pdb_ca(resid, resid, (resid - 1) as f64 * 3.8));
            pdb.push('\n');
        }
        let qsc = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<scene>\n\
             \t<qsc_opts base64=\"false\" compress=\"none\" version=\"QDF0\"/>\n\
             \t<object type=\"MolCoord\" name=\"rb\" src=\"datachunk:00000\" srctype=\"qdfpdb\">\n\
             \t\t<coloring type=\"RainbowColoring\" start_hue=\"240\" end_hue=\"0\" sat=\"1\" bri=\"1\"/>\n\
             \t\t<renderer type=\"trace\" name=\"trace1\"/>\n\
             \t</object>\n</scene>\n\n========== End of XML ==========\n\n\
             ==========START datachunk:00000==========\n{pdb}\n==========END datachunk:00000==========\n"
        );
        let path = std::env::temp_dir().join(format!("cuemol2-rs-rainbow-{}.qsc", std::process::id()));
        std::fs::write(&path, &qsc).unwrap();

        let mut app = DesktopAppState::new();
        let loaded = app.load_qsc_file(&path).ok().flatten();
        let _ = std::fs::remove_file(&path);
        let Some(scene_uid) = loaded else {
            panic!("synthetic rainbow .qsc failed to load");
        };
        // Sanity: the object's RainbowColoring parsed.
        assert_eq!(
            app.scene_manager()
                .scene(scene_uid)
                .unwrap()
                .objects()
                .first()
                .unwrap()
                .coloring_type(),
            Some("RainbowColoring"),
            "the object RainbowColoring must have parsed"
        );

        let snapshot = app.render_snapshot(scene_uid, "").unwrap();
        let colors: Vec<[f32; 4]> = snapshot
            .meshes
            .iter()
            .flat_map(|mesh| mesh.vertices.iter().map(|vertex| vertex.color))
            .collect();
        assert!(!colors.is_empty(), "the trace ribbon should produce mesh");
        // The gradient spans start_hue=240 (blue: B high, R low) to end_hue=0 (red:
        // R high, B low) — neither is producible by the default section colouring.
        let has_blue = colors.iter().any(|c| c[2] > 0.8 && c[0] < 0.2);
        let has_red = colors.iter().any(|c| c[0] > 0.8 && c[2] < 0.2);
        assert!(
            has_blue && has_red,
            "rainbow ribbon must span blue->red (has_blue={has_blue} has_red={has_red})"
        );
    }

    #[test]
    fn object_xform_mat_transforms_rendered_geometry() {
        // C++ `Object::xformMat` (e.g. a superposition fit persisted by
        // `superposeSSM1`'s by-property mode) places the rendered structure at its
        // transformed position (`MolAtom::getPos` applies it). Discriminating oracle
        // (advisor-steered): a 90deg-about-Z rotation + translation in the column-
        // major LScrMatrix4D layout — a pure translation would pass even with a
        // column-major<->row-major transpose bug.
        fn pdb_atom(x: f64, y: f64, z: f64) -> String {
            let mut line = vec![b' '; 80];
            let put = |line: &mut [u8], text: &str, start: usize| {
                for (i, byte) in text.bytes().enumerate() {
                    line[start + i] = byte;
                }
            };
            put(&mut line, "ATOM", 0);
            put(&mut line, "    1", 6);
            put(&mut line, "CA", 12);
            put(&mut line, "ALA", 17);
            put(&mut line, "A", 21);
            put(&mut line, "   1", 22);
            put(&mut line, &format!("{x:>8.3}"), 30);
            put(&mut line, &format!("{y:>8.3}"), 38);
            put(&mut line, &format!("{z:>8.3}"), 46);
            String::from_utf8(line).unwrap()
        }
        let pdb = pdb_atom(1.0, 0.0, 0.0);
        // A `cpk` renderer draws the lone atom as a single sphere (no bonds), so the
        // atom centre is the geometry to read back.
        let render_atom_center = |xform_attr: &str, tag: &str| -> cuemol_core::Vector4 {
            let qsc = format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<scene>\n\
                 \t<qsc_opts base64=\"false\" compress=\"none\" version=\"QDF0\"/>\n\
                 \t<object type=\"MolCoord\" name=\"xf\" src=\"datachunk:00000\" srctype=\"qdfpdb\"{xform_attr}>\n\
                 \t\t<renderer type=\"cpk\" name=\"cpk1\"/>\n\
                 \t</object>\n</scene>\n\n========== End of XML ==========\n\n\
                 ==========START datachunk:00000==========\n{pdb}\n==========END datachunk:00000==========\n"
            );
            let path = std::env::temp_dir()
                .join(format!("cuemol2-rs-xform-{tag}-{}.qsc", std::process::id()));
            std::fs::write(&path, &qsc).unwrap();
            let mut app = DesktopAppState::new();
            let loaded = app.load_qsc_file(&path).ok().flatten();
            let _ = std::fs::remove_file(&path);
            let scene_uid = loaded.expect("synthetic xform .qsc failed to load");
            let snapshot = app.render_snapshot(scene_uid, "").unwrap();
            assert_eq!(snapshot.atoms.len(), 1, "cpk lone atom -> exactly one sphere");
            snapshot.atoms[0].center
        };

        // Before: no xformMat -> the atom sits at its raw (1,0,0) (non-vacuous).
        let before = render_atom_center("", "id");
        assert!(
            (before.x - 1.0).abs() < 1e-6 && before.y.abs() < 1e-6 && before.z.abs() < 1e-6,
            "without xform the atom is at its raw position, got ({},{},{})",
            before.x,
            before.y,
            before.z
        );

        // After: R_z(+90) [ (x,y,z)->(-y,x,z) ] + translate(0,0,5) -> (0,1,5).
        let after =
            render_atom_center(" xformMat=\"(0,1,0,0,-1,0,0,0,0,0,1,0,0,0,5,1)\"", "rot");
        assert!(
            after.x.abs() < 1e-6 && (after.y - 1.0).abs() < 1e-6 && (after.z - 5.0).abs() < 1e-6,
            "object xformMat should rotate+translate the atom to (0,1,5), got ({},{},{})",
            after.x,
            after.y,
            after.z
        );
    }

    #[test]
    fn bfac_object_coloring_gradient_colors_the_ribbon() {
        // A synthetic .qsc whose object declares <coloring type="BfacColoring">
        // colours the ribbon by B-factor, lerped low_color(blue)→high_color(red),
        // auto-scaled to the molecule's B range. Low-B residues -> blue, high-B ->
        // red. Hand-built fixture with a B-factor ramp.
        fn pdb_ca(serial: i32, resid: i32, x: f64, bfac: f64) -> String {
            let mut line = vec![b' '; 80];
            let put = |line: &mut [u8], text: &str, start: usize| {
                for (i, byte) in text.bytes().enumerate() {
                    line[start + i] = byte;
                }
            };
            put(&mut line, "ATOM", 0);
            put(&mut line, &format!("{serial:>5}"), 6);
            put(&mut line, "CA", 12);
            put(&mut line, "ALA", 17);
            put(&mut line, "A", 21);
            put(&mut line, &format!("{resid:>4}"), 22);
            put(&mut line, &format!("{x:>8.3}"), 30);
            put(&mut line, &format!("{:>8.3}", 0.0), 38);
            put(&mut line, &format!("{:>8.3}", 0.0), 46);
            put(&mut line, &format!("{:>6.2}", 1.0), 54); // occupancy
            put(&mut line, &format!("{bfac:>6.2}"), 60); // B-factor (cols 61-66)
            String::from_utf8(line).unwrap()
        }
        let mut pdb = String::new();
        for resid in 1..=6 {
            let bfac = (resid - 1) as f64 * 20.0; // 0,20,40,60,80,100
            pdb.push_str(&pdb_ca(resid, resid, (resid - 1) as f64 * 3.8, bfac));
            pdb.push('\n');
        }
        let qsc = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<scene>\n\
             \t<qsc_opts base64=\"false\" compress=\"none\" version=\"QDF0\"/>\n\
             \t<object type=\"MolCoord\" name=\"bf\" src=\"datachunk:00000\" srctype=\"qdfpdb\">\n\
             \t\t<coloring type=\"BfacColoring\"/>\n\
             \t\t<renderer type=\"trace\" name=\"trace1\"/>\n\
             \t</object>\n</scene>\n\n========== End of XML ==========\n\n\
             ==========START datachunk:00000==========\n{pdb}\n==========END datachunk:00000==========\n"
        );
        let path = std::env::temp_dir().join(format!("cuemol2-rs-bfac-{}.qsc", std::process::id()));
        std::fs::write(&path, &qsc).unwrap();

        let mut app = DesktopAppState::new();
        let loaded = app.load_qsc_file(&path).ok().flatten();
        let _ = std::fs::remove_file(&path);
        let Some(scene_uid) = loaded else {
            panic!("synthetic bfac .qsc failed to load");
        };
        assert_eq!(
            app.scene_manager()
                .scene(scene_uid)
                .unwrap()
                .objects()
                .first()
                .unwrap()
                .coloring_type(),
            Some("BfacColoring"),
            "the object BfacColoring must have parsed"
        );

        let colors: Vec<[f32; 4]> = app
            .render_snapshot(scene_uid, "")
            .unwrap()
            .meshes
            .iter()
            .flat_map(|mesh| mesh.vertices.iter().map(|vertex| vertex.color))
            .collect();
        assert!(!colors.is_empty(), "the trace ribbon should produce mesh");
        // Low-B end -> blue (B>0.8,R<0.2), high-B end -> red (R>0.8,B<0.2); the
        // default element/section colouring produces neither.
        let has_blue = colors.iter().any(|c| c[2] > 0.8 && c[0] < 0.2);
        let has_red = colors.iter().any(|c| c[0] > 0.8 && c[2] < 0.2);
        assert!(
            has_blue && has_red,
            "B-factor ramp must span blue->red (has_blue={has_blue} has_red={has_red})"
        );
    }

    #[test]
    fn atom_renderer_carbons_defer_to_object_coloring() {
        // For an atom renderer (cpk), CPKColoring carbon is $molcol, so carbon atoms
        // defer to the object's PaintColoring (here `*`=FloralWhite) while N/O keep
        // their CPK element colours. Self-built .qsc with C/N/O atoms.
        fn pdb_atom(serial: i32, name: &str) -> String {
            let mut line = vec![b' '; 80];
            let put = |line: &mut [u8], text: &str, start: usize| {
                for (i, byte) in text.bytes().enumerate() {
                    line[start + i] = byte;
                }
            };
            put(&mut line, "ATOM", 0);
            put(&mut line, &format!("{serial:>5}"), 6);
            put(&mut line, &format!("{name:<4}"), 12);
            put(&mut line, "ALA", 17);
            put(&mut line, "A", 21);
            put(&mut line, "   1", 22);
            put(&mut line, &format!("{:>8.3}", serial as f64), 30);
            put(&mut line, &format!("{:>8.3}", 0.0), 38);
            put(&mut line, &format!("{:>8.3}", 0.0), 46);
            String::from_utf8(line).unwrap()
        }
        let pdb = format!(
            "{}\n{}\n{}\n",
            pdb_atom(1, "C"),
            pdb_atom(2, "N"),
            pdb_atom(3, "O")
        );
        let qsc = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<scene>\n\
             \t<qsc_opts base64=\"false\" compress=\"none\" version=\"QDF0\"/>\n\
             \t<object type=\"MolCoord\" name=\"a\" src=\"datachunk:00000\" srctype=\"qdfpdb\">\n\
             \t\t<coloring type=\"PaintColoring\">\n\t\t\t<paint sel=\"*\" color=\"FloralWhite\"/>\n\t\t</coloring>\n\
             \t\t<renderer type=\"cpk\" name=\"cpk1\"/>\n\
             \t</object>\n</scene>\n\n========== End of XML ==========\n\n\
             ==========START datachunk:00000==========\n{pdb}\n==========END datachunk:00000==========\n"
        );
        let path = std::env::temp_dir().join(format!("cuemol2-rs-atompaint-{}.qsc", std::process::id()));
        std::fs::write(&path, &qsc).unwrap();
        let mut app = DesktopAppState::new();
        let loaded = app.load_qsc_file(&path).ok().flatten();
        let _ = std::fs::remove_file(&path);
        let Some(scene_uid) = loaded else {
            panic!("synthetic atom-paint .qsc failed to load");
        };

        let snapshot = app.render_snapshot(scene_uid, "").unwrap();
        let mol = app.scene_manager().scene(scene_uid).unwrap().objects()[0].mol();
        let color_of = |element: &str| -> [u8; 3] {
            let atom = snapshot
                .atoms
                .iter()
                .find(|a| {
                    a.atom_index
                        .and_then(|i| mol.atoms().get(i))
                        .is_some_and(|at| at.element.eq_ignore_ascii_case(element))
                })
                .unwrap_or_else(|| panic!("no {element} atom in snapshot"));
            let c = atom.color.as_ref().unwrap().rgba8().unwrap();
            [c.r, c.g, c.b]
        };
        // Carbon defers to the paint `*` fallback FloralWhite (#FFFAF0).
        assert_eq!(color_of("C"), [255, 250, 240], "carbon should be paint FloralWhite");
        // Nitrogen keeps its CPK element colour (not FloralWhite).
        assert_ne!(color_of("N"), [255, 250, 240], "nitrogen keeps its CPK colour");
    }

    #[test]
    fn rainbow_object_coloring_colors_atom_renderer_carbons() {
        // RainbowColoring on an atom renderer: carbon atoms (CPK $molcol) span the
        // blue->red gradient across residues, like the ribbon. Self-built .qsc.
        fn pdb_ca(serial: i32, resid: i32) -> String {
            let mut line = vec![b' '; 80];
            let put = |line: &mut [u8], text: &str, start: usize| {
                for (i, byte) in text.bytes().enumerate() {
                    line[start + i] = byte;
                }
            };
            put(&mut line, "ATOM", 0);
            put(&mut line, &format!("{serial:>5}"), 6);
            put(&mut line, "CA", 12);
            put(&mut line, "ALA", 17);
            put(&mut line, "A", 21);
            put(&mut line, &format!("{resid:>4}"), 22);
            put(&mut line, &format!("{:>8.3}", resid as f64 * 3.8), 30);
            put(&mut line, &format!("{:>8.3}", 0.0), 38);
            put(&mut line, &format!("{:>8.3}", 0.0), 46);
            String::from_utf8(line).unwrap()
        }
        let mut pdb = String::new();
        for resid in 1..=6 {
            pdb.push_str(&pdb_ca(resid, resid));
            pdb.push('\n');
        }
        let qsc = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<scene>\n\
             \t<qsc_opts base64=\"false\" compress=\"none\" version=\"QDF0\"/>\n\
             \t<object type=\"MolCoord\" name=\"r\" src=\"datachunk:00000\" srctype=\"qdfpdb\">\n\
             \t\t<coloring type=\"RainbowColoring\" start_hue=\"240\" end_hue=\"0\" sat=\"1\" bri=\"1\"/>\n\
             \t\t<renderer type=\"ballstick\" name=\"bs1\"/>\n\
             \t</object>\n</scene>\n\n========== End of XML ==========\n\n\
             ==========START datachunk:00000==========\n{pdb}\n==========END datachunk:00000==========\n"
        );
        let path = std::env::temp_dir().join(format!("cuemol2-rs-rbatom-{}.qsc", std::process::id()));
        std::fs::write(&path, &qsc).unwrap();
        let mut app = DesktopAppState::new();
        let loaded = app.load_qsc_file(&path).ok().flatten();
        let _ = std::fs::remove_file(&path);
        let Some(scene_uid) = loaded else {
            panic!("synthetic rainbow-atom .qsc failed to load");
        };
        let snapshot = app.render_snapshot(scene_uid, "").unwrap();
        let colors: Vec<[f32; 4]> = snapshot.atoms.iter().map(|a| a.color.as_ref().unwrap().rgba8().map(|c| [c.r as f32/255.0, c.g as f32/255.0, c.b as f32/255.0, 1.0]).unwrap()).collect();
        assert!(!colors.is_empty(), "ballstick should draw atom spheres");
        let has_blue = colors.iter().any(|c| c[2] > 0.8 && c[0] < 0.2);
        let has_red = colors.iter().any(|c| c[0] > 0.8 && c[2] < 0.2);
        assert!(
            has_blue && has_red,
            "rainbow atom carbons must span blue->red (has_blue={has_blue} has_red={has_red})"
        );
    }

    #[test]
    fn desktop_app_state_opens_path_and_builds_render_snapshot() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        let loaded = app.open_path(root.join("1CRN.pdb")).unwrap();
        let GuiLoadedPath::Object { object_uid, .. } = loaded else {
            panic!("expected object load");
        };
        assert_eq!(app.active_object_summaries()[0].uid, object_uid);

        let snapshot = app.active_render_snapshot("name CA").unwrap();
        assert_eq!(snapshot.atom_count(), 327);
        assert!(snapshot.bond_count() > 0);
        assert_eq!(
            snapshot.atoms.iter().filter(|atom| atom.selected).count(),
            46
        );
        assert!(snapshot.bounds.is_some());
    }

    #[test]
    fn crystal_summary_reports_the_pdb_unit_cell() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        let GuiLoadedPath::Object { object_uid, .. } =
            app.open_path(root.join("1CRN.pdb")).unwrap()
        else {
            panic!("expected object load");
        };
        let cell = app
            .crystal_summary(object_uid)
            .expect("1CRN carries a CRYST1 cell");
        // Independent oracle: 1CRN's deposited CRYST1 record.
        assert!((cell.a - 40.96).abs() < 1e-2, "a = {}", cell.a);
        assert!((cell.b - 18.65).abs() < 1e-2, "b = {}", cell.b);
        assert!((cell.c - 22.52).abs() < 1e-2, "c = {}", cell.c);
        assert!((cell.alpha - 90.0).abs() < 1e-2);
        assert!((cell.beta - 90.77).abs() < 1e-2, "beta = {}", cell.beta);
        assert!((cell.gamma - 90.0).abs() < 1e-2);
        assert_eq!(cell.space_group_name.trim(), "P 1 21 1");
        // The active-scene aggregate finds the same crystal.
        assert_eq!(app.active_crystal_summaries().len(), 1);
    }

    #[test]
    fn desktop_app_state_opens_gzip_pdb_path() {
        let dir =
            std::env::temp_dir().join(format!("cuemol-desktop-gzip-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let raw_path = dir.join("one_atom.pdb");
        let gz_path = dir.join("one_atom.pdb.gz");
        std::fs::write(
            &raw_path,
            "ATOM      1  N   GLY A   1       1.000   2.000   3.000  1.00 10.00           N\nEND\n",
        )
        .unwrap();
        let output = std::process::Command::new("gzip")
            .arg("-c")
            .arg(&raw_path)
            .output();
        let Ok(output) = output else {
            let _ = std::fs::remove_file(raw_path);
            let _ = std::fs::remove_dir(dir);
            return;
        };
        if !output.status.success() {
            let _ = std::fs::remove_file(raw_path);
            let _ = std::fs::remove_dir(dir);
            return;
        }
        std::fs::write(&gz_path, output.stdout).unwrap();

        let mut app = DesktopAppState::new();
        let GuiLoadedPath::Object { object_uid, .. } = app.open_path(&gz_path).unwrap() else {
            panic!("expected object load");
        };
        let summary = app
            .active_object_summaries()
            .into_iter()
            .find(|summary| summary.uid == object_uid)
            .unwrap();
        assert_eq!(summary.name, "one_atom");
        assert_eq!(summary.atom_count, 1);

        let _ = std::fs::remove_file(raw_path);
        let _ = std::fs::remove_file(gz_path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn desktop_app_state_opens_bzip2_pdb_path() {
        let dir =
            std::env::temp_dir().join(format!("cuemol-desktop-bzip2-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let raw_path = dir.join("one_atom.pdb");
        let bz2_path = dir.join("one_atom.pdb.bz2");
        std::fs::write(
            &raw_path,
            "ATOM      1  N   GLY A   1       1.000   2.000   3.000  1.00 10.00           N\nEND\n",
        )
        .unwrap();
        let output = std::process::Command::new("bzip2")
            .arg("-c")
            .arg(&raw_path)
            .output();
        let Ok(output) = output else {
            let _ = std::fs::remove_file(raw_path);
            let _ = std::fs::remove_dir(dir);
            return;
        };
        if !output.status.success() {
            let _ = std::fs::remove_file(raw_path);
            let _ = std::fs::remove_dir(dir);
            return;
        }
        std::fs::write(&bz2_path, output.stdout).unwrap();

        let mut app = DesktopAppState::new();
        let GuiLoadedPath::Object { object_uid, .. } = app.open_path(&bz2_path).unwrap() else {
            panic!("expected object load");
        };
        let summary = app
            .active_object_summaries()
            .into_iter()
            .find(|summary| summary.uid == object_uid)
            .unwrap();
        assert_eq!(summary.name, "one_atom");
        assert_eq!(summary.atom_count, 1);

        let _ = std::fs::remove_file(raw_path);
        let _ = std::fs::remove_file(bz2_path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn desktop_app_state_opens_extension_molecule_formats() {
        let dir =
            std::env::temp_dir().join(format!("cuemol-desktop-ext-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gro_path = dir.join("water.gro");
        let psf_path = dir.join("water.psf");
        let mol2_path = dir.join("ligand.mol2");
        let amber_path = dir.join("water.prmtop");
        let qdfpdb_path = dir.join("one_atom.qdfpdb");
        std::fs::write(
            &gro_path,
            "water\n    1\n    1SOL     OW    1   1.000   2.000   3.000\n   2.00000   2.00000   2.00000\n",
        )
        .unwrap();
        std::fs::write(
            &psf_path,
            "PSF\n\n       1 !NATOM\n       1 SYS      1        WAT      OH2    OT     -0.834000       15.9994           0\n",
        )
        .unwrap();
        std::fs::write(
            &mol2_path,
            "@<TRIPOS>MOLECULE\nligand\n2 1 1 0 0\nSMALL\nUSER_CHARGES\n@<TRIPOS>ATOM\n1 C1 0.0 0.0 0.0 C.3 1 LIG 0.0\n2 O1 1.2 0.0 0.0 O.2 1 LIG 0.0\n@<TRIPOS>BOND\n1 1 2 1\n",
        )
        .unwrap();
        std::fs::write(
            &amber_path,
            "%VERSION  VERSION_STAMP = V0001.000  DATE = 01/01/24  00:00:00\n%FLAG POINTERS\n%FORMAT(10I8)\n       3       2       2       0       0       0       0       0       0       0\n       0       1       0       0       0       0       0       0       0       0\n       0       0       0       0       0       0       0       0       3       0\n       0\n%FLAG ATOM_NAME\n%FORMAT(20a4)\nOW  HW1 HW2 \n%FLAG ATOMIC_NUMBER\n%FORMAT(10I8)\n       8       1       1\n%FLAG MASS\n%FORMAT(5E16.8)\n  1.60000000E+01  1.00800000E+00  1.00800000E+00\n%FLAG RESIDUE_LABEL\n%FORMAT(20a4)\nWAT \n%FLAG RESIDUE_POINTER\n%FORMAT(10I8)\n       1\n%FLAG BONDS_INC_HYDROGEN\n%FORMAT(10I8)\n       0       3       1       0       6       1\n%FLAG BONDS_WITHOUT_HYDROGEN\n%FORMAT(10I8)\n%FLAG AMBER_ATOM_TYPE\n%FORMAT(20a4)\nOW  HW  HW  \n",
        )
        .unwrap();
        std::fs::write(
            &qdfpdb_path,
            "ATOM      1  N   GLY A   1       1.000   2.000   3.000  1.00 10.00           N\nEND\n",
        )
        .unwrap();

        let mut app = DesktopAppState::new();
        for (path, atoms, bonds) in [
            (&gro_path, 1_usize, 0_usize),
            (&psf_path, 1_usize, 0_usize),
            (&mol2_path, 2_usize, 1_usize),
            (&amber_path, 3_usize, 2_usize),
            (&qdfpdb_path, 1_usize, 0_usize),
        ] {
            let GuiLoadedPath::Object { object_uid, .. } = app.open_path(path).unwrap() else {
                panic!("expected object load");
            };
            let summary = app
                .active_object_summaries()
                .into_iter()
                .find(|summary| summary.uid == object_uid)
                .unwrap();
            assert_eq!(summary.atom_count, atoms);
            assert_eq!(summary.bond_count, bonds);
            assert_eq!(summary.renderer_count, 1);
        }

        for path in [gro_path, psf_path, mol2_path, amber_path, qdfpdb_path] {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn desktop_app_state_opens_density_map_formats() {
        let dir =
            std::env::temp_dir().join(format!("cuemol-desktop-map-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dx_path = dir.join("small.dx");
        let xplor_path = dir.join("xplor.map");
        std::fs::write(
            &dx_path,
            "\
object 1 class gridpositions counts  1 1 2
origin 0.0 1.0 2.0
delta  1 0 0
delta  0 1 0
delta  0 0 1
object 2 class gridconnections counts  1 1 2
object 3 class array type \"double\" rank 0 items 2 data follows
1.0 2.0
attribute \"dep\" string \"positions\"
",
        )
        .unwrap();
        std::fs::write(&xplor_path, xplor_map([2, 1, 1], [0, 0, 0], [1.0, 2.0])).unwrap();

        let mut app = DesktopAppState::new();
        let GuiLoadedPath::Map { scene_uid, map_uid } = app.open_path(&dx_path).unwrap() else {
            panic!("expected density map load");
        };
        let summary = app.density_map_summary(map_uid).unwrap();
        assert_eq!(summary.scene_uid, scene_uid);
        assert_eq!(summary.name, "small");
        assert_eq!(summary.format, "apbs");
        assert_eq!(summary.counts, [1, 1, 2]);
        assert_eq!(summary.value_count, 2);
        assert_eq!(app.scene_summaries()[0].map_count, 1);
        assert_eq!(app.scene_summaries()[0].object_count, 0);

        let GuiLoadedPath::Map { map_uid, .. } = app.open_path(&xplor_path).unwrap() else {
            panic!("expected density map load");
        };
        let summary = app.density_map_summary(map_uid).unwrap();
        assert_eq!(summary.format, "xplormap");
        assert_eq!(summary.counts, [2, 1, 1]);
        assert_eq!(app.active_density_map_summaries().len(), 2);

        app.close_active_scene().unwrap();
        assert!(app.density_map_summary(map_uid).is_none());

        let _ = std::fs::remove_file(dx_path);
        let _ = std::fs::remove_file(xplor_path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn desktop_app_state_saves_active_scene_qsc() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("cuemol-desktop-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scene.qsc");

        let mut app = DesktopAppState::new();
        app.open_path(root.join("1CRN.pdb")).unwrap();
        let mut camera = cuemol_qsys::Camera::new();
        camera.set_name("__current");
        camera.set_cam_dist(123.0);
        app.set_active_scene_camera(camera).unwrap();
        app.save_active_scene_qsc(&path).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("<scene>"));
        assert!(saved.contains("name=\"__current\""));
        assert!(saved.contains("==========START datachunk:"));
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn desktop_app_state_saves_object_exports_pov_and_sequences() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let dir =
            std::env::temp_dir().join(format!("cuemol-desktop-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdb_path = dir.join("object.pdb");
        let pov_path = dir.join("scene.pov");

        let mut app = DesktopAppState::new();
        let GuiLoadedPath::Object { object_uid, .. } =
            app.open_path(root.join("1CRN.pdb")).unwrap()
        else {
            panic!("expected object load");
        };
        app.save_object_as(object_uid, &pdb_path).unwrap();
        assert!(std::fs::read_to_string(&pdb_path).unwrap().contains("ATOM"));

        let exported_atoms = app.export_active_scene_pov(&pov_path, "name CA").unwrap();
        assert_eq!(exported_atoms, 327);
        assert!(std::fs::read_to_string(&pov_path)
            .unwrap()
            .contains("sphere"));

        let rows = app.active_sequence_summaries();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chain_id, "A");
        assert_eq!(rows[0].residue_count, 46);
        assert_eq!(rows[0].sequence.len(), 46);

        let _ = std::fs::remove_file(pdb_path);
        let _ = std::fs::remove_file(pov_path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn desktop_app_state_closes_active_scene() {
        let mut app = DesktopAppState::new();
        let first = app.new_scene("first", true).result_scene;
        let second = app.new_scene("second", true).result_scene;
        assert_eq!(app.active_scene(), Some(second));
        assert_eq!(app.close_active_scene().unwrap(), Some(first));
        assert_eq!(app.active_scene(), Some(first));
        assert_eq!(app.close_active_scene().unwrap(), None);
        assert_eq!(app.active_scene(), None);
    }

    #[test]
    fn desktop_app_state_applies_edit_menu_molecule_ops() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        let loaded = app.open_path(root.join("1CRN.pdb")).unwrap();
        let GuiLoadedPath::Object { object_uid, .. } = loaded else {
            panic!("expected object load");
        };

        let changed = app
            .change_chain_in_object(object_uid, "name CA", "Z")
            .unwrap();
        assert_eq!(changed, 46);
        let renumbered = app
            .renumber_residues_in_object(object_uid, "chain Z", 10)
            .unwrap();
        assert_eq!(renumbered, 46);
        let deleted = app.delete_atoms_in_object(object_uid, "chain Z").unwrap();
        assert_eq!(deleted, 46);

        let snapshot = app.active_render_snapshot("").unwrap();
        assert_eq!(snapshot.atom_count(), 281);
    }

    #[test]
    fn desktop_app_state_applies_analysis_tools() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let mut app = DesktopAppState::new();
        let first = match app.open_path(root.join("1CRN.pdb")).unwrap() {
            GuiLoadedPath::Object { object_uid, .. } => object_uid,
            _ => panic!("expected object load"),
        };
        let second = match app.open_path(root.join("1CRN.pdb")).unwrap() {
            GuiLoadedPath::Object { object_uid, .. } => object_uid,
            _ => panic!("expected object load"),
        };

        let matrix = app
            .superpose_objects(first, "name CA", second, "name CA")
            .unwrap();
        assert!(matrix.starts_with('('));

        assert!(app.make_bond_in_object(first, 1, 327).unwrap());
        assert!(app.remove_bond_in_object(first, 1, 327).unwrap());
        let contacts = app
            .calc_atom_contacts_in_object(first, "name CA", 1.0, 8.0, false, 4)
            .unwrap();
        assert!(contacts.starts_with('['));
        assert_eq!(
            app.assign_secondary_structure_in_object(first, false, 60.0)
                .unwrap(),
            327
        );
    }

    #[test]
    fn desktop_app_state_writes_surface_apbs_and_morph_frames() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let dir =
            std::env::temp_dir().join(format!("cuemol-desktop-core-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let surface_path = dir.join("surface.obj");
        let cut_path = dir.join("cut.obj");
        let pqr_path = dir.join("apbs.pqr");
        let input_path = dir.join("apbs.in");
        let dx_path = dir.join("apbs.dx");

        let mut app = DesktopAppState::new();
        let first = match app.open_path(root.join("1CRN.pdb")).unwrap() {
            GuiLoadedPath::Object { object_uid, .. } => object_uid,
            _ => panic!("expected object load"),
        };
        let second = match app.open_path(root.join("1CRN.pdb")).unwrap() {
            GuiLoadedPath::Object { object_uid, .. } => object_uid,
            _ => panic!("expected object load"),
        };

        let surface = app
            .write_surface_obj(first, "name CA", 1.0, 1.4, &surface_path)
            .unwrap();
        // Grid marching-tetrahedra surface of the 46 CA spheres: a substantial
        // closed mesh (the former octahedron stub gave a fixed 276 verts/368 tris).
        assert!(surface.vertex_count > 1000);
        assert!(surface.triangle_count > 1000);
        assert!(std::fs::read_to_string(&surface_path)
            .unwrap()
            .contains("o cuemol_surface"));

        let cut = app
            .write_cut_surface_obj(
                first,
                "name CA",
                1.0,
                1.4,
                cuemol_core::Vector4::new3(0.0, 0.0, 1.0),
                cuemol_core::Vector4::ZERO,
                &cut_path,
            )
            .unwrap();
        assert!(cut.triangle_count < surface.triangle_count);

        let apbs = app
            .write_apbs_files(first, "name CA", &pqr_path, &input_path, &dx_path)
            .unwrap();
        assert_eq!(apbs.atom_count, 46);
        assert!(std::fs::read_to_string(&pqr_path).unwrap().contains("ATOM"));
        assert!(std::fs::read_to_string(&input_path)
            .unwrap()
            .contains("write pot dx"));

        let morph = app
            .append_morph_frame_from_object(first, first, "start")
            .unwrap();
        assert_eq!(morph.frame_count, 1);
        let morph = app
            .append_morph_frame_from_object(first, second, "second")
            .unwrap();
        assert_eq!(morph.frame_count, 2);
        let applied = app.apply_morph_frame(first, 0.5).unwrap();
        assert_eq!(applied.atom_count, 327);

        for path in [surface_path, cut_path, pqr_path, input_path] {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(dir);
    }

    fn reference_test_data_root() -> Option<std::path::PathBuf> {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2")
            });
        let path = root.join("tests/test_data");
        path.exists().then_some(path)
    }

    fn xplor_map(
        counts: [i32; 3],
        start: [i32; 3],
        values: impl IntoIterator<Item = f64>,
    ) -> String {
        let end = [
            start[0] + counts[0] - 1,
            start[1] + counts[1] - 1,
            start[2] + counts[2] - 1,
        ];
        let section_number = start[2];
        let mut output = String::new();
        output.push('\n');
        output.push_str("       1\n");
        output.push_str(" REMARKS generated by desktop test\n");
        output.push_str(&format!(
            "{:8}{:8}{:8}{:8}{:8}{:8}{:8}{:8}{:8}\n",
            counts[0], start[0], end[0], counts[1], start[1], end[1], counts[2], start[2], end[2]
        ));
        output.push_str(&format!(
            "{:12.5E}{:12.5E}{:12.5E}{:12.5E}{:12.5E}{:12.5E}\n",
            2.0, 1.0, 1.0, 90.0, 90.0, 90.0
        ));
        output.push_str("ZYX\n");
        output.push_str(&format!("{section_number}\n"));
        for (index, value) in values.into_iter().enumerate() {
            output.push_str(&format!("{value:12.5E}"));
            if index % 6 == 5 {
                output.push('\n');
            }
        }
        output.push('\n');
        output
    }
}
