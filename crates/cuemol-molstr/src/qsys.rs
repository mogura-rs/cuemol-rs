use crate::{
    read_mmcif_file, read_mol2_file, read_pdb_file, read_pdb_str, read_qdfpdb_file, read_sdf_file,
    write_pdb_string, Atom, MolCoord,
};
use cuemol_core::{CueMolError, Quat, Result, Uid, Vector4};
use cuemol_qsys::{Camera, CenterMark, StereoMode, StyleManager, View};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct MolRenderer {
    uid: Uid,
    name: String,
    renderer_type: String,
    qsc_attrs: BTreeMap<String, String>,
    atom_intr_lines: Vec<AtomIntrLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomIntrLine {
    pub a0: String,
    pub a1: String,
    aid1: String,
    aid2: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QscLoadStatus {
    Loaded { scene_uid: Uid },
    GitLfsPointer { oid: String, size: u64 },
}

pub type MolView = View;

impl MolRenderer {
    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn renderer_type(&self) -> &str {
        &self.renderer_type
    }

    pub fn qsc_attr(&self, name: &str) -> Option<&str> {
        self.qsc_attrs.get(name).map(String::as_str)
    }

    pub fn group(&self) -> Option<&str> {
        self.qsc_attr("group")
    }

    pub fn style(&self) -> Option<&str> {
        self.qsc_attr("style")
    }

    /// Resolve a renderer property through the C++ value cascade (`applyStyles`):
    /// an inline `.qsc` attribute on this renderer overrides any preset; failing
    /// that, the renderer's `style="A,B"` list is resolved against `styles` in
    /// `scope_id` (a later style in the list wins). Returns `None` when neither
    /// the renderer nor any referenced style defines `attr`.
    ///
    /// This is what wires a scene's `<renderer style="DefaultBallStick">` to the
    /// preset's `bondw`/`sphr`/… so renderers inherit styled geometry instead of
    /// hardcoded defaults.
    pub fn resolved_attr(
        &self,
        attr: &str,
        styles: &StyleManager,
        scope_id: u64,
    ) -> Option<String> {
        if let Some(inline) = self.qsc_attr(attr) {
            if !inline.is_empty() {
                return Some(inline.to_string());
            }
        }
        let style_list = self.style()?;
        styles.resolve_style_attr(scope_id, style_list, attr)
    }

    pub fn is_visible(&self) -> bool {
        !matches!(self.qsc_attr("visible"), Some("false" | "0" | "no" | "off"))
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.qsc_attrs
            .insert("visible".to_string(), visible.to_string());
    }

    pub fn set_qsc_attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.qsc_attrs.insert(name.into(), value.into());
    }

    pub fn remove_qsc_attr(&mut self, name: &str) -> bool {
        self.qsc_attrs.remove(name).is_some()
    }

    pub fn get_defs_json(&self) -> String {
        let entries = self
            .atom_intr_lines
            .iter()
            .map(|line| {
                format!(
                    "{{\"a0\":\"{}\",\"a1\":\"{}\"}}",
                    escape_json(&line.a0),
                    escape_json(&line.a1)
                )
            })
            .collect::<Vec<_>>();
        format!("[{}]", entries.join(","))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolObject {
    uid: Uid,
    name: String,
    mol: MolCoord,
    renderers: Vec<MolRenderer>,
    next_renderer_uid: u64,
}

impl MolObject {
    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mol(&self) -> &MolCoord {
        &self.mol
    }

    pub fn mol_mut(&mut self) -> &mut MolCoord {
        &mut self.mol
    }

    pub fn renderers(&self) -> &[MolRenderer] {
        &self.renderers
    }

    pub fn renderer_by_name(&self, name: &str) -> Option<&MolRenderer> {
        self.renderers.iter().find(|renderer| renderer.name == name)
    }

    pub fn renderer(&self, uid: Uid) -> Option<&MolRenderer> {
        self.renderers.iter().find(|renderer| renderer.uid == uid)
    }

    pub fn renderer_mut(&mut self, uid: Uid) -> Option<&mut MolRenderer> {
        self.renderers
            .iter_mut()
            .find(|renderer| renderer.uid == uid)
    }

    pub fn destroy_renderer(&mut self, uid: Uid) -> bool {
        let before = self.renderers.len();
        self.renderers.retain(|renderer| renderer.uid != uid);
        self.renderers.len() != before
    }

    pub fn add_renderer(
        &mut self,
        renderer_type: impl Into<String>,
        name: impl Into<String>,
    ) -> Uid {
        self.add_renderer_with_attrs(renderer_type, name, BTreeMap::new())
    }

    fn add_renderer_with_attrs(
        &mut self,
        renderer_type: impl Into<String>,
        name: impl Into<String>,
        qsc_attrs: BTreeMap<String, String>,
    ) -> Uid {
        let uid = Uid::new(self.next_renderer_uid);
        self.next_renderer_uid += 1;
        self.renderers.push(MolRenderer {
            uid,
            name: name.into(),
            renderer_type: renderer_type.into(),
            qsc_attrs,
            atom_intr_lines: Vec::new(),
        });
        uid
    }

    pub fn add_atom_intr_renderer(
        &mut self,
        name: impl Into<String>,
        aids: &[(String, String)],
    ) -> Result<Uid> {
        self.add_atom_intr_renderer_with_attrs(name, aids, BTreeMap::new())
    }

    fn add_atom_intr_renderer_with_attrs(
        &mut self,
        name: impl Into<String>,
        aids: &[(String, String)],
        qsc_attrs: BTreeMap<String, String>,
    ) -> Result<Uid> {
        let mut lines = Vec::new();
        for (aid1, aid2) in aids {
            lines.push(AtomIntrLine {
                a0: self.resolve_atom_label(aid1)?,
                a1: self.resolve_atom_label(aid2)?,
                aid1: aid1.clone(),
                aid2: aid2.clone(),
            });
        }
        let uid = Uid::new(self.next_renderer_uid);
        self.next_renderer_uid += 1;
        self.renderers.push(MolRenderer {
            uid,
            name: name.into(),
            renderer_type: "atomintr".to_string(),
            qsc_attrs,
            atom_intr_lines: lines,
        });
        Ok(uid)
    }

    /// Resolve an atom-interaction reference (`aid`) to an atom. The reference is
    /// either a numeric index into the atom array or a `chain.resid.name` triple.
    fn resolve_atom(&self, aid: &str) -> Option<&Atom> {
        if let Ok(index) = aid.parse::<usize>() {
            self.mol.atoms().get(index)
        } else {
            let parts = aid.split('.').collect::<Vec<_>>();
            if parts.len() != 3 {
                return None;
            }
            let chain = parts[0];
            let resid = parts[1].parse::<i32>().ok();
            let name = parts[2];
            self.mol.atoms().iter().find(|atom| {
                atom.chain_id == chain
                    && Some(atom.res_id) == resid
                    && atom.name.trim().eq_ignore_ascii_case(name)
            })
        }
    }

    fn resolve_atom_label(&self, aid: &str) -> Result<String> {
        self.resolve_atom(aid)
            .map(atom_display_name)
            .ok_or_else(|| CueMolError::NotFound(format!("atom id {aid}")))
    }

    /// Distance (Å) measured by each line of an atom-interaction `renderer`, in
    /// line order; `None` for a line whose atoms cannot be resolved. The `.qsc`
    /// scene facade models only 2-atom distance measurements (angle/torsion live in
    /// the molvis `AtomIntrRenderer`); this is the value C++ `AtomIntrRenderer`
    /// shows for a distance interaction.
    pub fn atom_intr_distances(&self, renderer: &MolRenderer) -> Vec<Option<f64>> {
        renderer
            .atom_intr_lines
            .iter()
            .map(|line| {
                let p0 = self.resolve_atom(&line.aid1)?.position;
                let p1 = self.resolve_atom(&line.aid2)?.position;
                Some((p0 - p1).length3d())
            })
            .collect()
    }

    /// JSON array of `{a0, a1, distance}` for an atom-interaction renderer, where
    /// `distance` is the measured value (or `null` when unresolved).
    pub fn atom_intr_measurements_json(&self, renderer: &MolRenderer) -> String {
        let distances = self.atom_intr_distances(renderer);
        let entries = renderer
            .atom_intr_lines
            .iter()
            .zip(distances)
            .map(|(line, dist)| {
                let dist = match dist {
                    Some(value) => format!("{value:.3}"),
                    None => "null".to_string(),
                };
                format!(
                    "{{\"a0\":\"{}\",\"a1\":\"{}\",\"distance\":{}}}",
                    escape_json(&line.a0),
                    escape_json(&line.a1),
                    dist
                )
            })
            .collect::<Vec<_>>();
        format!("[{}]", entries.join(","))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolScene {
    uid: Uid,
    name: String,
    objects: Vec<MolObject>,
    cameras: Vec<Camera>,
    next_object_uid: u64,
}

impl MolScene {
    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn objects(&self) -> &[MolObject] {
        &self.objects
    }

    pub fn cameras(&self) -> &[Camera] {
        &self.cameras
    }

    pub fn camera_by_name(&self, name: &str) -> Option<&Camera> {
        self.cameras.iter().find(|camera| camera.name() == name)
    }

    pub fn set_camera(&mut self, camera: Camera) {
        if let Some(existing) = self
            .cameras
            .iter_mut()
            .find(|existing| existing.name() == camera.name())
        {
            *existing = camera;
        } else {
            self.cameras.push(camera);
        }
    }

    pub fn object_by_name(&self, name: &str) -> Option<&MolObject> {
        self.objects.iter().find(|object| object.name == name)
    }

    pub fn object(&self, uid: Uid) -> Option<&MolObject> {
        self.objects.iter().find(|object| object.uid == uid)
    }

    pub fn object_mut(&mut self, uid: Uid) -> Option<&mut MolObject> {
        self.objects.iter_mut().find(|object| object.uid == uid)
    }

    pub fn load_object(
        &mut self,
        file_path: impl AsRef<Path>,
        object_name: impl Into<String>,
        file_format: &str,
    ) -> Result<Uid> {
        let mol = match file_format {
            "pdb" => read_pdb_file(file_path)?,
            "qdfpdb" => read_qdfpdb_file(file_path)?,
            "mmcif" | "cif" => read_mmcif_file(file_path)?,
            "sdf" | "mol" => read_sdf_file(file_path)?,
            "mol2" => read_mol2_file(file_path)?,
            other => {
                return Err(CueMolError::invalid_argument(format!(
                    "unknown format {other}"
                )))
            }
        };
        Ok(self.add_molecule(object_name, mol))
    }

    pub fn add_molecule(&mut self, name: impl Into<String>, mol: MolCoord) -> Uid {
        let uid = Uid::new(self.next_object_uid);
        self.next_object_uid += 1;
        self.objects.push(MolObject {
            uid,
            name: name.into(),
            mol,
            renderers: Vec::new(),
            next_renderer_uid: 1,
        });
        uid
    }

    pub fn load_qsc_str(&mut self, input: &str) -> Result<()> {
        let xml = input
            .split("========== End of XML ==========")
            .next()
            .unwrap_or(input);
        let object_blocks = extract_qsc_object_blocks(xml);
        if object_blocks.is_empty() && extract_qsc_camera_lines(xml).is_empty() {
            return Err(CueMolError::parse("QSC scene content not found"));
        }

        if !object_blocks.is_empty() {
            let chunks = extract_datachunks(input)?;
            for block in object_blocks {
                self.load_qsc_object_block(&block, &chunks)?;
            }
        }
        for line in extract_qsc_camera_lines(xml) {
            self.set_camera(parse_qsc_camera(&line)?);
        }
        Ok(())
    }

    fn load_qsc_object_block(
        &mut self,
        block: &[String],
        chunks: &BTreeMap<String, String>,
    ) -> Result<()> {
        let object_line = block
            .first()
            .ok_or_else(|| CueMolError::parse("QSC object element not found"))?;
        let object_name = attr_value(object_line, "name")
            .ok_or_else(|| CueMolError::parse("QSC object name not found"))?;
        let data = attr_value(object_line, "src")
            .and_then(|src| src.strip_prefix("datachunk:").map(str::to_string))
            .and_then(|chunk_id| chunks.get(&chunk_id).cloned())
            .or_else(|| chunks.values().next().cloned())
            .ok_or_else(|| CueMolError::parse("QSC datachunk not found"))?;
        let object_uid = self.add_molecule(object_name, read_pdb_str(&data)?);
        let object = self
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound("loaded QSC object".to_string()))?;

        let mut current_atomintr: Option<(String, Vec<(String, String)>)> = None;
        let mut current_atomintr_attrs = BTreeMap::new();
        for line in block.iter().map(|line| line.trim()) {
            if line.starts_with("<renderer ") {
                let attrs = attr_map(line);
                let renderer_type = attr_value(line, "type").unwrap_or_default();
                let renderer_name =
                    attr_value(line, "name").unwrap_or_else(|| renderer_type.clone());
                if renderer_type == "atomintr" && !line.ends_with("/>") {
                    current_atomintr_attrs = attrs;
                    current_atomintr = Some((renderer_name, Vec::new()));
                } else if renderer_type == "atomintr" {
                    object.add_atom_intr_renderer_with_attrs(renderer_name, &[], attrs)?;
                } else if !renderer_name.is_empty() {
                    object.add_renderer_with_attrs(renderer_type, renderer_name, attrs);
                }
            } else if line.starts_with("<line ") {
                if let Some((_, lines)) = current_atomintr.as_mut() {
                    let aid1 = attr_value(line, "aid1")
                        .ok_or_else(|| CueMolError::parse("AtomIntr aid1 not found"))?;
                    let aid2 = attr_value(line, "aid2")
                        .ok_or_else(|| CueMolError::parse("AtomIntr aid2 not found"))?;
                    lines.push((aid1, aid2));
                }
            } else if line.starts_with("</renderer>") {
                if let Some((name, lines)) = current_atomintr.take() {
                    object.add_atom_intr_renderer_with_attrs(
                        name,
                        &lines,
                        std::mem::take(&mut current_atomintr_attrs),
                    )?;
                }
            }
        }

        if let Some((name, lines)) = current_atomintr.take() {
            object.add_atom_intr_renderer_with_attrs(name, &lines, current_atomintr_attrs)?;
        }
        Ok(())
    }

    pub fn write_qsc_string(&self) -> String {
        let mut output = String::new();
        output.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\n");
        output.push_str("<scene>\n");
        output.push_str("\t<qsc_opts base64=\"false\" compress=\"none\" version=\"QDF0\"/>\n");
        for (index, object) in self.objects.iter().enumerate() {
            let chunk_id = format!("{index:05}");
            output.push_str(&format!(
                "\t<object type=\"MolCoord\" name=\"{}\" src=\"datachunk:{}\" srctype=\"qdfpdb\">\n",
                escape_xml(object.name()),
                chunk_id
            ));
            for renderer in object.renderers() {
                if renderer.renderer_type() == "atomintr" {
                    output.push_str(&format!(
                        "\t\t<renderer{}>\n",
                        format_renderer_attrs(renderer, Some("DefaultLabel,DefaultAtomIntr"))
                    ));
                    for line in &renderer.atom_intr_lines {
                        output.push_str(&format!(
                            "\t\t\t<line aid1=\"{}\" aid2=\"{}\"/>\n",
                            escape_xml(&line.aid1),
                            escape_xml(&line.aid2)
                        ));
                    }
                    output.push_str("\t\t</renderer>\n");
                } else {
                    output.push_str(&format!(
                        "\t\t<renderer{}/>\n",
                        format_renderer_attrs(renderer, None)
                    ));
                }
            }
            output.push_str("\t</object>\n");
        }
        for camera in &self.cameras {
            output.push_str(&format!(
                "\t<camera center=\"{}\" centerMark=\"{}\" distance=\"{}\" name=\"{}\" perspec=\"{}\" rotation=\"{}\" slab=\"{}\" stereoDist=\"{}\" stereoMode=\"{}\" zoom=\"{}\"/>\n",
                format_vector3(camera.center()),
                center_mark_to_qsc(camera.center_mark()),
                format_float(camera.cam_dist()),
                escape_xml(camera.name()),
                camera.is_perspective(),
                format_quat(camera.rotation()),
                format_float(camera.slab_depth()),
                format_float(camera.stereo_distance()),
                stereo_mode_to_qsc(camera.stereo_mode()),
                format_float(camera.zoom()),
            ));
        }
        output.push_str("</scene>\n\n");
        output.push_str("========== End of XML ==========\n\n");
        for (index, object) in self.objects.iter().enumerate() {
            let chunk_id = format!("{index:05}");
            output.push_str(&format!("==========START datachunk:{chunk_id}==========\n"));
            output.push_str(&write_pdb_string(object.mol()));
            output.push_str(&format!("\n==========END datachunk:{chunk_id}==========\n"));
        }
        output
    }
}

#[derive(Debug, Default, Clone)]
pub struct MolSceneManager {
    scenes: Vec<MolScene>,
    next_scene_uid: u64,
}

impl MolSceneManager {
    pub fn new() -> Self {
        Self {
            scenes: Vec::new(),
            next_scene_uid: 1,
        }
    }

    pub fn new_scene(&mut self, name: impl Into<String>) -> Uid {
        let uid = Uid::new(self.next_scene_uid);
        self.next_scene_uid += 1;
        self.scenes.push(MolScene {
            uid,
            name: name.into(),
            objects: Vec::new(),
            cameras: Vec::new(),
            next_object_uid: 1,
        });
        uid
    }

    pub fn scene(&self, uid: Uid) -> Option<&MolScene> {
        self.scenes.iter().find(|scene| scene.uid == uid)
    }

    pub fn scenes(&self) -> &[MolScene] {
        &self.scenes
    }

    pub fn scene_uids(&self) -> Vec<Uid> {
        self.scenes.iter().map(MolScene::uid).collect()
    }

    pub fn scene_mut(&mut self, uid: Uid) -> Option<&mut MolScene> {
        self.scenes.iter_mut().find(|scene| scene.uid == uid)
    }

    pub fn destroy_scene(&mut self, uid: Uid) -> bool {
        let before = self.scenes.len();
        self.scenes.retain(|scene| scene.uid != uid);
        before != self.scenes.len()
    }

    pub fn object(&self, uid: Uid) -> Option<&MolObject> {
        self.scenes.iter().find_map(|scene| scene.object(uid))
    }

    pub fn object_mut(&mut self, uid: Uid) -> Option<&mut MolObject> {
        self.scenes
            .iter_mut()
            .find_map(|scene| scene.object_mut(uid))
    }

    pub fn scene_uid_for_object(&self, uid: Uid) -> Option<Uid> {
        self.scenes
            .iter()
            .find(|scene| scene.object(uid).is_some())
            .map(MolScene::uid)
    }

    pub fn destroy_all_scenes(&mut self) {
        self.scenes.clear();
    }

    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    pub fn load_qsc_file(&mut self, path: impl AsRef<Path>) -> Result<Uid> {
        match self.load_qsc_file_checked(path)? {
            QscLoadStatus::Loaded { scene_uid } => Ok(scene_uid),
            QscLoadStatus::GitLfsPointer { oid, size } => Err(CueMolError::parse(format!(
                "QSC file is a Git LFS pointer: oid={oid}, size={size}"
            ))),
        }
    }

    pub fn load_qsc_file_checked(&mut self, path: impl AsRef<Path>) -> Result<QscLoadStatus> {
        let input = fs::read_to_string(path)?;
        if let Some((oid, size)) = parse_git_lfs_pointer(&input) {
            return Ok(QscLoadStatus::GitLfsPointer { oid, size });
        }
        let uid = self.new_scene("loaded scene");
        self.scene_mut(uid)
            .ok_or_else(|| CueMolError::NotFound("loaded scene".to_string()))?
            .load_qsc_str(&input)?;
        Ok(QscLoadStatus::Loaded { scene_uid: uid })
    }

    pub fn write_qsc_file(&self, scene_uid: Uid, path: impl AsRef<Path>) -> Result<()> {
        let scene = self
            .scene(scene_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", scene_uid.get())))?;
        fs::write(path, scene.write_qsc_string())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewSceneResult {
    pub result_scene: Uid,
    pub result_view: Option<Uid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadObjectResult {
    pub result_object: Uid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadSceneResult {
    pub result_scene: Option<Uid>,
    pub status: QscLoadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewRendererResult {
    pub result_renderer: Uid,
}

#[derive(Debug, Clone)]
pub struct MolCommandManager {
    scene_manager: MolSceneManager,
    views: Vec<MolView>,
    next_view_uid: u64,
}

impl Default for MolCommandManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MolCommandManager {
    pub fn new() -> Self {
        Self {
            scene_manager: MolSceneManager::new(),
            views: Vec::new(),
            next_view_uid: 1,
        }
    }

    pub fn scene_manager(&self) -> &MolSceneManager {
        &self.scene_manager
    }

    pub fn scene_manager_mut(&mut self) -> &mut MolSceneManager {
        &mut self.scene_manager
    }

    pub fn view(&self, uid: Uid) -> Option<&MolView> {
        self.views.iter().find(|view| view.uid() == uid)
    }

    pub fn new_scene_command(
        &mut self,
        scene_name: impl Into<String>,
        create_view: bool,
    ) -> NewSceneResult {
        let scene_uid = self.scene_manager.new_scene(scene_name);
        let result_view = create_view.then(|| {
            let uid = Uid::new(self.next_view_uid);
            self.next_view_uid += 1;
            self.views.push(MolView::new(uid, scene_uid));
            uid
        });
        NewSceneResult {
            result_scene: scene_uid,
            result_view,
        }
    }

    pub fn load_object_command(
        &mut self,
        target_scene: Uid,
        file_path: impl AsRef<Path>,
        object_name: impl Into<String>,
        file_format: &str,
    ) -> Result<LoadObjectResult> {
        let scene = self
            .scene_manager
            .scene_mut(target_scene)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", target_scene.get())))?;
        let object_uid = scene.load_object(file_path, object_name, file_format)?;
        Ok(LoadObjectResult {
            result_object: object_uid,
        })
    }

    pub fn load_scene_command(
        &mut self,
        file_path: impl AsRef<Path>,
        target_scene: Option<Uid>,
    ) -> Result<LoadSceneResult> {
        let input = fs::read_to_string(file_path)?;
        if let Some((oid, size)) = parse_git_lfs_pointer(&input) {
            return Ok(LoadSceneResult {
                result_scene: None,
                status: QscLoadStatus::GitLfsPointer { oid, size },
            });
        }

        let scene_uid = match target_scene {
            Some(uid) => uid,
            None => self.scene_manager.new_scene("loaded scene"),
        };
        self.scene_manager
            .scene_mut(scene_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", scene_uid.get())))?
            .load_qsc_str(&input)?;
        Ok(LoadSceneResult {
            result_scene: Some(scene_uid),
            status: QscLoadStatus::Loaded { scene_uid },
        })
    }

    pub fn new_renderer_command(
        &mut self,
        target_scene: Uid,
        target_object: Uid,
        renderer_type: impl Into<String>,
        renderer_name: impl Into<String>,
    ) -> Result<NewRendererResult> {
        let object = self
            .scene_manager
            .scene_mut(target_scene)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", target_scene.get())))?
            .object_mut(target_object)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", target_object.get())))?;
        let renderer_uid = object.add_renderer(renderer_type, renderer_name);
        Ok(NewRendererResult {
            result_renderer: renderer_uid,
        })
    }
}

fn atom_display_name(atom: &Atom) -> String {
    format!(
        "{} {} {} {}",
        atom.chain_id,
        atom.res_name,
        atom.res_id,
        atom.name.trim()
    )
}

fn attr_value(line: &str, name: &str) -> Option<String> {
    let marker = format!("{name}=\"");
    let start = line.find(&marker)? + marker.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

fn attr_map(line: &str) -> BTreeMap<String, String> {
    let Some(mut rest) = line.trim().strip_prefix('<') else {
        return BTreeMap::new();
    };
    rest = rest.trim_start_matches(|ch: char| !ch.is_whitespace());
    let mut attrs = BTreeMap::new();
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with('/') || rest.starts_with('>') {
            return attrs;
        }
        let Some(eq_pos) = rest.find('=') else {
            return attrs;
        };
        let name = rest[..eq_pos].trim();
        let after_eq = rest[eq_pos + 1..].trim_start();
        let Some(after_quote) = after_eq.strip_prefix('"') else {
            return attrs;
        };
        let Some(end_quote) = after_quote.find('"') else {
            return attrs;
        };
        attrs.insert(name.to_string(), after_quote[..end_quote].to_string());
        rest = &after_quote[end_quote + 1..];
    }
}

fn format_renderer_attrs(renderer: &MolRenderer, default_style: Option<&str>) -> String {
    let mut attrs = renderer.qsc_attrs.clone();
    attrs.insert("type".to_string(), renderer.renderer_type().to_string());
    attrs.insert("name".to_string(), renderer.name().to_string());
    attrs.entry("group".to_string()).or_default();
    if let Some(default_style) = default_style {
        attrs
            .entry("style".to_string())
            .or_insert_with(|| default_style.to_string());
    }
    let ordered = ["type", "group", "name", "style"];
    let mut parts = Vec::new();
    for name in ordered {
        if let Some(value) = attrs.remove(name) {
            parts.push(format!("{name}=\"{}\"", escape_xml(&value)));
        }
    }
    for (name, value) in attrs {
        parts.push(format!("{name}=\"{}\"", escape_xml(&value)));
    }
    format!(" {}", parts.join(" "))
}

fn extract_datachunks(input: &str) -> Result<BTreeMap<String, String>> {
    let mut chunks = BTreeMap::new();
    let start_marker = "==========START datachunk:";
    let end_marker = "==========END datachunk:";
    let mut search_from = 0;
    while let Some(relative_start) = input[search_from..].find(start_marker) {
        let start = search_from + relative_start;
        let header_start = start + start_marker.len();
        let data_start = input[start..]
            .find('\n')
            .map(|offset| start + offset + 1)
            .ok_or_else(|| CueMolError::parse("QSC datachunk body not found"))?;
        let chunk_id = input[header_start..data_start]
            .trim()
            .trim_end_matches('=')
            .to_string();
        let data_end = input[data_start..]
            .find(end_marker)
            .map(|offset| data_start + offset)
            .ok_or_else(|| CueMolError::parse("QSC datachunk end not found"))?;
        chunks.insert(chunk_id, input[data_start..data_end].to_string());
        search_from = data_end + end_marker.len();
    }
    if chunks.is_empty() {
        return Err(CueMolError::parse("QSC datachunk not found"));
    }
    Ok(chunks)
}

fn extract_qsc_object_blocks(xml: &str) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut current: Option<Vec<String>> = None;
    for line in xml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<object ") {
            current = Some(vec![trimmed.to_string()]);
            if trimmed.ends_with("/>") {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
            }
        } else if let Some(block) = current.as_mut() {
            block.push(trimmed.to_string());
            if trimmed.starts_with("</object>") {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
            }
        }
    }
    blocks
}

fn extract_qsc_camera_lines(xml: &str) -> Vec<String> {
    xml.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("<camera "))
        .map(str::to_string)
        .collect()
}

fn parse_qsc_camera(line: &str) -> Result<Camera> {
    let mut camera = Camera::default();
    if let Some(name) = attr_value(line, "name") {
        camera.set_name(name);
    }
    if let Some(center) = attr_value(line, "center") {
        camera.set_center(parse_vector3(&center)?);
    }
    if let Some(rotation) = attr_value(line, "rotation") {
        camera.set_rotation(parse_quat(&rotation)?);
    }
    if let Some(distance) = attr_value(line, "distance").and_then(|value| value.parse().ok()) {
        camera.set_cam_dist(distance);
    }
    if let Some(slab) = attr_value(line, "slab").and_then(|value| value.parse().ok()) {
        camera.set_slab_depth(slab);
    }
    if let Some(zoom) = attr_value(line, "zoom").and_then(|value| value.parse().ok()) {
        camera.set_zoom(zoom);
    }
    if let Some(stereo_distance) =
        attr_value(line, "stereoDist").and_then(|value| value.parse().ok())
    {
        camera.set_stereo_distance(stereo_distance);
    }
    if let Some(perspective) = attr_value(line, "perspec") {
        camera.set_perspective(perspective.eq_ignore_ascii_case("true"));
    }
    if let Some(center_mark) = attr_value(line, "centerMark") {
        camera.set_center_mark(center_mark_from_qsc(&center_mark));
    }
    if let Some(stereo_mode) = attr_value(line, "stereoMode") {
        camera.set_stereo_mode(stereo_mode_from_qsc(&stereo_mode));
    }
    Ok(camera)
}

fn parse_vector3(input: &str) -> Result<Vector4> {
    let values = parse_tuple(input)?;
    if values.len() != 3 {
        return Err(CueMolError::parse(format!(
            "invalid vector3 tuple: {input}"
        )));
    }
    Ok(Vector4::new3(values[0], values[1], values[2]))
}

fn parse_quat(input: &str) -> Result<Quat> {
    let values = parse_tuple(input)?;
    if values.len() != 4 {
        return Err(CueMolError::parse(format!(
            "invalid quaternion tuple: {input}"
        )));
    }
    Ok(Quat::new(values[0], values[1], values[2], values[3]))
}

fn parse_tuple(input: &str) -> Result<Vec<f64>> {
    let trimmed = input.trim().trim_start_matches('(').trim_end_matches(')');
    trimmed
        .split(',')
        .map(|value| {
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| CueMolError::parse(format!("invalid tuple number: {value}")))
        })
        .collect()
}

fn format_vector3(value: Vector4) -> String {
    format!(
        "({},{},{})",
        format_float(value.x),
        format_float(value.y),
        format_float(value.z)
    )
}

fn format_quat(value: Quat) -> String {
    format!(
        "({},{},{},{})",
        format_float(value.x),
        format_float(value.y),
        format_float(value.z),
        format_float(value.w)
    )
}

fn format_float(value: f64) -> String {
    let mut formatted = format!("{value:.6}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    if formatted == "-0" {
        "0".to_string()
    } else {
        formatted
    }
}

fn center_mark_from_qsc(value: &str) -> CenterMark {
    match value {
        "none" => CenterMark::None,
        _ => CenterMark::Cross,
    }
}

fn center_mark_to_qsc(value: CenterMark) -> &'static str {
    match value {
        CenterMark::None => "none",
        CenterMark::Cross => "crosshair",
    }
}

fn stereo_mode_from_qsc(value: &str) -> StereoMode {
    match value {
        "parallel" | "para" => StereoMode::Parallel,
        "cross" => StereoMode::Cross,
        _ => StereoMode::None,
    }
}

fn stereo_mode_to_qsc(value: StereoMode) -> &'static str {
    match value {
        StereoMode::None => "none",
        StereoMode::Parallel => "parallel",
        StereoMode::Cross => "cross",
    }
}

fn parse_git_lfs_pointer(input: &str) -> Option<(String, u64)> {
    let mut lines = input.lines();
    if lines.next()? != "version https://git-lfs.github.com/spec/v1" {
        return None;
    }
    let mut oid = None;
    let mut size = None;
    for line in lines {
        if let Some(value) = line.strip_prefix("oid sha256:") {
            oid = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("size ") {
            size = value.parse::<u64>().ok();
        }
    }
    Some((oid?, size?))
}

fn escape_json(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
