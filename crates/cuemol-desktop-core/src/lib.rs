use cuemol_anim::{MorphFrame, MorphMol};
use cuemol_core::{CueMolError, Result, Uid};
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
    read_brix_map_file, read_ccp4_map_file, read_opendx_file, read_xplor_map_file, DensityMap,
};
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
    // Object-level rainbow coloring (`<coloring type="RainbowColoring">`).
    options.rainbow = cuemol_molvis::rainbow_coloring_from_attrs(object.coloring_type(), |name| {
        object.coloring_attr(name)
    });
    let snapshot = match (renderer_type, renderer) {
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
    match renderer.map(MolRenderer::renderer_type).unwrap_or("simple") {
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

#[cfg(test)]
mod tests {
    use super::*;

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
