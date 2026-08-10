use crate::qsys::{AtomIntrKind, MolRenderer};
use cuemol_core::{
    parse_ldom2_xml, write_ldom2_xml, ByteArray, CueMolError, LDom2Node, LDom2Tree, Result,
};
use cuemol_qsys::{
    Camera, IOThread, InOutHandler, ObjReader, Renderer as QsysRenderer, SceneXMLReader,
    SceneXMLWriter,
};
use std::any::Any;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerCategory {
    ObjReader,
    ObjWriter,
    RendToFile,
    SceneReader,
    SceneWriter,
}

impl HandlerCategory {
    pub const OBJ_READER: i32 = 0;
    pub const OBJ_WRITER: i32 = 1;
    pub const REND_TO_FILE: i32 = 2;
    pub const SCENE_READER: i32 = 3;
    pub const SCENE_WRITER: i32 = 4;

    pub fn code(self) -> i32 {
        match self {
            Self::ObjReader => Self::OBJ_READER,
            Self::ObjWriter => Self::OBJ_WRITER,
            Self::RendToFile => Self::REND_TO_FILE,
            Self::SceneReader => Self::SCENE_READER,
            Self::SceneWriter => Self::SCENE_WRITER,
        }
    }

    pub fn from_code(code: i32) -> Result<Self> {
        match code {
            Self::OBJ_READER => Ok(Self::ObjReader),
            Self::OBJ_WRITER => Ok(Self::ObjWriter),
            Self::REND_TO_FILE => Ok(Self::RendToFile),
            Self::SCENE_READER => Ok(Self::SceneReader),
            Self::SCENE_WRITER => Ok(Self::SceneWriter),
            _ => Err(CueMolError::invalid_argument(format!(
                "unknown stream handler category {code}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    None,
    Gzip,
    Bzip2,
    Xzip,
}

impl CompressionMode {
    pub const NONE: i32 = 0;
    pub const GZIP: i32 = 1;
    pub const BZIP2: i32 = 2;
    pub const XZIP: i32 = 3;

    pub fn code(self) -> i32 {
        match self {
            Self::None => Self::NONE,
            Self::Gzip => Self::GZIP,
            Self::Bzip2 => Self::BZIP2,
            Self::Xzip => Self::XZIP,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentVerdict {
    No = -1,
    Unknown = 0,
    Yes = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerSniffer {
    Pdb,
    MmcifMol,
    Sdf,
    Mol2,
    Gro,
    Psf,
    AmberPrmtop,
    NamdCoor,
    Dcd,
    Trr,
    Xtc,
    AmberNetcdf,
    OpenDx,
    MmcifMap,
    Ccp4Map,
    XplorMap,
    BrixMap,
    MtzMap,
    MsmsSurface,
    PlySurface,
    QdfMol,
    QdfMap,
    QdfSurf,
    QdfPot,
    QdfLwObj,
    QscXml,
    Pse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamHandlerInfo {
    pub abi_name: String,
    pub nickname: String,
    pub descr: String,
    pub fext: String,
    pub category: HandlerCategory,
    pub sniffer: Option<HandlerSniffer>,
}

impl StreamHandlerInfo {
    pub fn new(
        abi_name: impl Into<String>,
        nickname: impl Into<String>,
        descr: impl Into<String>,
        fext: impl Into<String>,
        category: HandlerCategory,
        sniffer: Option<HandlerSniffer>,
    ) -> Self {
        Self {
            abi_name: abi_name.into(),
            nickname: nickname.into(),
            descr: descr.into(),
            fext: fext.into(),
            category,
            sniffer,
        }
    }
}

#[derive(Debug)]
pub struct AsyncLoadTask {
    reader_type: String,
    worker: Option<IOThread<AsyncLoadResult>>,
    byte_len: usize,
    eos: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AsyncLoadedObject {
    Molecule(crate::MolCoord),
    GridMap(crate::GridMap),
    /// Fully buffered input for readers implemented by a higher-level crate.
    /// This preserves asynchronous transport without introducing dependency
    /// cycles from molstr to xtal, surface, mdtools, importers, or lwview.
    Buffered {
        reader_type: String,
        data: ByteArray,
    },
}

#[derive(Debug)]
struct AsyncLoadResult {
    data: Vec<u8>,
    object: Result<AsyncLoadedObject>,
}

impl AsyncLoadTask {
    fn new(reader_type: impl Into<String>) -> Self {
        let reader_type = reader_type.into();
        let worker_reader_type = reader_type.clone();
        Self {
            reader_type,
            worker: Some(IOThread::spawn(move |input| {
                let mut data = Vec::new();
                input.read_to_end(&mut data)?;
                let object = parse_async_object(&worker_reader_type, &data);
                Ok(AsyncLoadResult { data, object })
            })),
            byte_len: 0,
            eos: false,
        }
    }

    pub fn reader_type(&self) -> &str {
        &self.reader_type
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub fn is_eos(&self) -> bool {
        self.eos
    }

    fn supply_data(&mut self, data: &[u8]) -> bool {
        if self.eos {
            return false;
        }
        let supplied = self
            .worker
            .as_mut()
            .is_some_and(|worker| worker.supply_data(data));
        if supplied {
            self.byte_len = self.byte_len.saturating_add(data.len());
        }
        supplied
    }

    fn wait(&mut self) -> Result<AsyncLoadResult> {
        self.eos = true;
        self.worker
            .take()
            .ok_or_else(|| CueMolError::invalid_argument("asynchronous load already completed"))?
            .wait()
    }
}

#[derive(Debug)]
pub struct StreamManager {
    handlers: Vec<StreamHandlerInfo>,
    next_async_load_id: i32,
    async_loads: BTreeMap<i32, AsyncLoadTask>,
}

impl Clone for StreamManager {
    fn clone(&self) -> Self {
        Self {
            handlers: self.handlers.clone(),
            next_async_load_id: self.next_async_load_id,
            async_loads: BTreeMap::new(),
        }
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamManager {
    pub fn new() -> Self {
        let mut manager = Self::empty();
        manager.register_default_handlers().unwrap();
        manager
    }

    pub fn empty() -> Self {
        Self {
            handlers: Vec::new(),
            next_async_load_id: 1,
            async_loads: BTreeMap::new(),
        }
    }

    pub fn get_cat_objreader(&self) -> i32 {
        HandlerCategory::OBJ_READER
    }

    pub fn get_cat_objwriter(&self) -> i32 {
        HandlerCategory::OBJ_WRITER
    }

    pub fn get_cat_scereader(&self) -> i32 {
        HandlerCategory::SCENE_READER
    }

    pub fn get_cat_scewriter(&self) -> i32 {
        HandlerCategory::SCENE_WRITER
    }

    pub fn handlers(&self) -> &[StreamHandlerInfo] {
        &self.handlers
    }

    pub fn register_handler(&mut self, info: StreamHandlerInfo) -> Result<()> {
        if self
            .handlers
            .iter()
            .any(|handler| handler.abi_name == info.abi_name)
        {
            return Err(CueMolError::invalid_argument(format!(
                "stream handler already exists: {}",
                info.abi_name
            )));
        }
        self.handlers.push(info);
        Ok(())
    }

    pub fn unregister_reader(&mut self, abi_name: &str, writer: bool) -> bool {
        let category = if writer {
            HandlerCategory::ObjWriter
        } else {
            HandlerCategory::ObjReader
        };
        let before = self.handlers.len();
        self.handlers
            .retain(|handler| handler.abi_name != abi_name || handler.category != category);
        before != self.handlers.len()
    }

    pub fn is_reader_registered(&self, abi_name: &str) -> bool {
        self.handlers
            .iter()
            .any(|handler| handler.abi_name == abi_name)
    }

    pub fn handler_by_nickname(
        &self,
        nickname: &str,
        category: HandlerCategory,
    ) -> Option<&StreamHandlerInfo> {
        self.handlers
            .iter()
            .find(|handler| handler.nickname == nickname && handler.category == category)
    }

    pub fn create_handler(&self, nickname: &str, category: i32) -> Option<InOutHandler> {
        let category = HandlerCategory::from_code(category).ok()?;
        let info = self.handler_by_nickname(nickname, category)?;
        Some(InOutHandler::new(
            info.nickname.clone(),
            info.descr.clone(),
            info.fext.clone(),
            category.code(),
        ))
    }

    pub fn get_reader_info_json(&self) -> String {
        self.get_handler_info_pseudo_json(HandlerCategory::ObjReader)
    }

    pub fn get_writer_info_json(&self) -> String {
        self.get_handler_info_pseudo_json(HandlerCategory::ObjWriter)
    }

    pub fn get_info_json2(&self) -> String {
        let entries = self
            .handlers
            .iter()
            .map(|handler| {
                format!(
                    "{{\"descr\":\"{}\",\"fext\":\"{}\",\"name\":\"{}\",\"category\":{}}}",
                    escape_json(&handler.descr),
                    escape_json(&handler.fext),
                    escape_json(&handler.nickname),
                    handler.category.code()
                )
            })
            .collect::<Vec<_>>();
        format!("[{}]", entries.join(","))
    }

    pub fn get_info_json_2(&self) -> String {
        self.get_info_json2()
    }

    pub fn find_compatible_writer_names_for_obj(&self, objid: u64) -> String {
        if objid == 0 {
            return String::new();
        }
        self.handlers
            .iter()
            .filter(|handler| handler.category == HandlerCategory::ObjWriter)
            .map(|handler| handler.nickname.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn load_object_async<R: 'static>(&mut self, reader: &R) -> i32 {
        let id = self.next_async_load_id;
        self.next_async_load_id = self.next_async_load_id.saturating_add(1).max(1);
        self.async_loads.insert(
            id,
            AsyncLoadTask::new(
                async_reader_name(reader).unwrap_or_else(|| std::any::type_name::<R>().to_string()),
            ),
        );
        id
    }

    pub fn supply_data_async(&mut self, load_id: i32, data: &[u8], data_len: i32) -> bool {
        let Some(task) = self.async_loads.get_mut(&load_id) else {
            return false;
        };
        if task.eos {
            return false;
        }
        let Ok(data_len) = usize::try_from(data_len) else {
            return false;
        };
        let Some(data) = data.get(..data_len) else {
            return false;
        };
        task.supply_data(data)
    }

    pub fn supply_all_data_async(&mut self, load_id: i32, data: &[u8]) -> bool {
        let Ok(data_len) = i32::try_from(data.len()) else {
            return false;
        };
        self.supply_data_async(load_id, data, data_len)
    }

    pub fn wait_load_bytes_async(&mut self, load_id: i32) -> Option<ByteArray> {
        let mut task = self.async_loads.remove(&load_id)?;
        let result = task.wait().ok()?;
        Some(ByteArray::copy_from_u8(&result.data))
    }

    pub fn wait_load_async(&mut self, load_id: i32) -> Option<AsyncLoadedObject> {
        self.wait_load_object_async(load_id).ok().flatten()
    }

    pub fn wait_load_object_async(&mut self, load_id: i32) -> Result<Option<AsyncLoadedObject>> {
        let Some(mut task) = self.async_loads.remove(&load_id) else {
            return Ok(None);
        };
        task.wait()?.object.map(Some)
    }

    pub fn async_load(&self, load_id: i32) -> Option<&AsyncLoadTask> {
        self.async_loads.get(&load_id)
    }

    pub fn to_xml<T: 'static>(&self, object: &T) -> ByteArray {
        if let Some(camera) = (object as &dyn Any).downcast_ref::<Camera>() {
            return SceneXMLWriter::new().camera_to_byte_array(camera);
        }
        if let Some(renderer) = (object as &dyn Any).downcast_ref::<MolRenderer>() {
            return node_to_byte_array(renderer_node_from_mol(renderer, None));
        }
        if let Some(renderer) = (object as &dyn Any).downcast_ref::<QsysRenderer>() {
            return node_to_byte_array(renderer_node_from_qsys(renderer, None));
        }
        if let Some(node) = (object as &dyn Any).downcast_ref::<LDom2Node>() {
            return node_to_byte_array(node.clone());
        }
        ByteArray::copy_from_u8(
            format!(
                "<object type=\"{}\"/>",
                escape_xml(std::any::type_name::<T>())
            )
            .as_bytes(),
        )
    }

    pub fn to_xml_2<T: 'static>(&self, object: &T, type_override: &str) -> ByteArray {
        if type_override.trim().is_empty() {
            return self.to_xml(object);
        }
        if let Some(renderer) = (object as &dyn Any).downcast_ref::<MolRenderer>() {
            return node_to_byte_array(renderer_node_from_mol(renderer, Some(type_override)));
        }
        if let Some(renderer) = (object as &dyn Any).downcast_ref::<QsysRenderer>() {
            return node_to_byte_array(renderer_node_from_qsys(renderer, Some(type_override)));
        }
        if (object as &dyn Any).is::<Camera>() {
            return self.to_xml(object);
        }
        ByteArray::copy_from_u8(
            format!("<object type=\"{}\"/>", escape_xml(type_override)).as_bytes(),
        )
    }

    pub fn to_xml2<T: 'static>(&self, object: &T, type_override: &str) -> ByteArray {
        self.to_xml_2(object, type_override)
    }

    pub fn array_to_xml<T: 'static>(&self, array: &[T]) -> ByteArray {
        if let Some(nodes) = renderer_nodes(array, "") {
            return renderer_nodes_to_byte_array(nodes, "");
        }
        ByteArray::copy_from_u8(
            format!(
                "<array type=\"{}\" size=\"{}\"/>",
                escape_xml(std::any::type_name::<T>()),
                array.len()
            )
            .as_bytes(),
        )
    }

    pub fn rend_grp_to_xml<T: 'static>(&self, array: &[T], group_name: &str) -> ByteArray {
        if let Some(nodes) = renderer_nodes(array, group_name) {
            return renderer_nodes_to_byte_array(nodes, group_name);
        }
        ByteArray::copy_from_u8(
            format!(
                "<rendgrp name=\"{}\" size=\"{}\"/>",
                escape_xml(group_name),
                array.len()
            )
            .as_bytes(),
        )
    }

    pub fn from_xml(&self, data: &ByteArray, _flags: i32) -> Option<String> {
        String::from_utf8(data.as_bytes().to_vec()).ok()
    }

    pub fn camera_from_xml(&self, data: &ByteArray, _scene_id: i32) -> Result<Camera> {
        SceneXMLReader::new().camera_from_byte_array(data)
    }

    pub fn renderer_from_xml(&self, data: &ByteArray, _scene_id: i32) -> Result<QsysRenderer> {
        let tree = parse_xml_byte_array(data)?;
        renderer_from_node(tree.top())
    }

    pub fn renderer_array_from_xml(
        &self,
        data: &ByteArray,
        _scene_id: i32,
    ) -> Result<(String, Vec<QsysRenderer>)> {
        let tree = parse_xml_byte_array(data)?;
        if tree.top().tag_name() != "renderers" {
            return Err(CueMolError::parse("renderer array root is not renderers"));
        }
        let group = tree.top().str_attr("group").to_string();
        let renderers = tree
            .top()
            .children()
            .iter()
            .filter(|node| !node.is_attr() && node.tag_name() == "renderer")
            .map(renderer_from_node)
            .collect::<Result<Vec<_>>>()?;
        Ok((group, renderers))
    }

    pub fn rend_array_from_xml(&self, data: &ByteArray, _flags: i32) -> Vec<String> {
        if let Ok((group, renderers)) = self.renderer_array_from_xml(data, _flags) {
            return std::iter::once(group)
                .chain(
                    renderers
                        .into_iter()
                        .map(|renderer| renderer.name().to_string()),
                )
                .collect();
        }
        let Ok(xml) = String::from_utf8(data.as_bytes().to_vec()) else {
            return Vec::new();
        };
        let mut values = Vec::new();
        if let Some(group_name) = extract_xml_attr(&xml, "rendgrp", "name") {
            values.push(group_name);
        }
        for tag in xml.match_indices("<renderer") {
            let fragment = &xml[tag.0..];
            if let Some(name) = extract_xml_attr(fragment, "renderer", "name") {
                values.push(name);
            }
        }
        values
    }

    pub fn search_reader_by_content(
        &self,
        path: impl AsRef<Path>,
        nicknames_csv: &str,
        category: HandlerCategory,
        support_compression: bool,
        max_bytes: u64,
    ) -> String {
        self.search_by_content(
            path,
            nicknames_csv,
            category,
            support_compression,
            max_bytes,
            true,
        )
    }

    pub fn search_readers_by_content(
        &self,
        path: impl AsRef<Path>,
        nicknames_csv: &str,
        category: HandlerCategory,
        support_compression: bool,
        max_bytes: u64,
    ) -> String {
        self.search_by_content(
            path,
            nicknames_csv,
            category,
            support_compression,
            max_bytes,
            false,
        )
    }

    pub fn sniff_content(
        &self,
        path: impl AsRef<Path>,
        sniffer: HandlerSniffer,
        max_bytes: u64,
    ) -> ContentVerdict {
        let Ok(input) = read_bytes(path, max_bytes) else {
            return ContentVerdict::Unknown;
        };
        sniff_bytes(&input, sniffer)
    }

    pub fn cat_objreader(&self) -> i32 {
        HandlerCategory::OBJ_READER
    }

    pub fn object_reader(&self) -> i32 {
        self.cat_objreader()
    }

    pub fn cat_objwriter(&self) -> i32 {
        HandlerCategory::OBJ_WRITER
    }

    pub fn object_writer(&self) -> i32 {
        self.cat_objwriter()
    }

    pub fn cat_rendtofile(&self) -> i32 {
        HandlerCategory::REND_TO_FILE
    }

    pub fn cat_scereader(&self) -> i32 {
        HandlerCategory::SCENE_READER
    }

    pub fn scene_reader(&self) -> i32 {
        self.cat_scereader()
    }

    pub fn cat_scewriter(&self) -> i32 {
        HandlerCategory::SCENE_WRITER
    }

    pub fn scene_writer(&self) -> i32 {
        self.cat_scewriter()
    }

    pub fn comp_none(&self) -> i32 {
        CompressionMode::NONE
    }

    pub fn comp_gzip(&self) -> i32 {
        CompressionMode::GZIP
    }

    pub fn comp_bzip2(&self) -> i32 {
        CompressionMode::BZIP2
    }

    pub fn comp_xzip(&self) -> i32 {
        CompressionMode::XZIP
    }

    fn register_default_handlers(&mut self) -> Result<()> {
        for handler in default_handlers() {
            self.register_handler(handler)?;
        }
        Ok(())
    }

    fn get_handler_info_pseudo_json(&self, category: HandlerCategory) -> String {
        let handlers = self
            .handlers
            .iter()
            .filter(|handler| handler.category == category)
            .collect::<Vec<_>>();
        let mut output = format!("({{ size: {}", handlers.len());
        if handlers.is_empty() {
            output.push_str(" })\n");
            return output;
        }

        output.push_str(",\n");
        output.push_str(&format!(
            "descrs:[ {} ],\n",
            quoted_list(handlers.iter().map(|handler| handler.descr.as_str()))
        ));
        output.push_str(&format!(
            "fexts:[ {} ],\n",
            quoted_list(handlers.iter().map(|handler| handler.fext.as_str()))
        ));
        output.push_str(&format!(
            "nicknames:[ {} ] }})\n",
            quoted_list(handlers.iter().map(|handler| handler.nickname.as_str()))
        ));
        output
    }

    fn search_by_content(
        &self,
        path: impl AsRef<Path>,
        nicknames_csv: &str,
        category: HandlerCategory,
        support_compression: bool,
        max_bytes: u64,
        first_only: bool,
    ) -> String {
        let wanted = parse_nickname_csv(nicknames_csv);
        let filtered = !wanted.is_empty();
        let candidates = self
            .handlers
            .iter()
            .filter(|handler| handler.category == category)
            .filter(|handler| !filtered || wanted.iter().any(|name| name == &handler.nickname))
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return String::new();
        }

        if !support_compression && has_compressed_extension(path.as_ref()) {
            return String::new();
        }

        let Ok(input) = read_search_bytes(path, max_bytes, support_compression) else {
            return String::new();
        };

        let mut hits = Vec::new();
        for handler in candidates {
            let Some(sniffer) = handler.sniffer else {
                continue;
            };
            if sniff_bytes(&input, sniffer) == ContentVerdict::Yes {
                if first_only {
                    return handler.nickname.clone();
                }
                hits.push(handler.nickname.clone());
            }
        }
        hits.join(",")
    }
}

fn default_handlers() -> Vec<StreamHandlerInfo> {
    vec![
        StreamHandlerInfo::new(
            "cuemol_rs::PDBFileReader",
            "pdb",
            "PDB Coordinates (*.pdb;*.ent;*.pdb.gz)",
            "*.pdb; *.ent; *.pdb.gz",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Pdb),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfPdbReader",
            "qdfpdb",
            "QDF-PDB (*.qdfpdb)",
            "*.qdfpdb",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Pdb),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfMolReader",
            "qdfmol",
            "CueMol coordinates file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::QdfMol),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::MmcifMolReader",
            "mmcif",
            "mmCIF Coordinates (*.cif;*.cif.gz)",
            "*.cif; *.cif.gz",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::MmcifMol),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::SDFMolReader",
            "sdf",
            "MOL/SDF Coordinates (*.mol;*.sdf)",
            "*.mol; *.sdf",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Sdf),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::MOL2MolReader",
            "mol2",
            "SYBYL Mol2 Coordinates (*.mol2)",
            "*.mol2",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Mol2),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::GROFileReader",
            "gro",
            "GROMACS Coordinates (*.gro)",
            "*.gro",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Gro),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::PsfReader",
            "psf",
            "CHARMM/NAMD PSF topology (*.psf)",
            "*.psf",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Psf),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::AmberPrmtopReader",
            "amberprm",
            "AMBER topology file (*.parm7;*.prmtop)",
            "*.parm7; *.prmtop",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::AmberPrmtop),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::NAMDCoorReader",
            "namdcoor",
            "NAMD binary coordinates (*.coor)",
            "*.coor",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::NamdCoor),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::DCDTrajReader",
            "dcdtraj",
            "DCD binary trajectory (*.dcd)",
            "*.dcd",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Dcd),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::TrrTrajReader",
            "trrtraj",
            "GROMACS TRR trajectory (*.trr)",
            "*.trr",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Trr),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::XtcTrajReader",
            "xtctraj",
            "GROMACS XTC trajectory (*.xtc)",
            "*.xtc",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Xtc),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::AmberNetCDFReader",
            "ambnetcdftraj",
            "AMBER NetCDF trajectory (*.nc)",
            "*.nc",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::AmberNetcdf),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::OpenDXPotReader",
            "apbs",
            "APBS Potential Map(*.dx)",
            "*.dx",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::OpenDx),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::MmcifMapReader",
            "mmcifmap",
            "mmCIF Map Coeff (*.cif;*.cif.gz)",
            "*.cif; *.cif.gz",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::MmcifMap),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::CCP4MapReader",
            "ccp4map",
            "CCP4 Density Map(*.map;*.ccp4;*.mrc;*.ccp4.gz)",
            "*.map; *.ccp4; *.mrc; *.ccp4.gz",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Ccp4Map),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::XplorMapReader",
            "xplormap",
            "XPLOR/CNS Density Map(*.map;*.cns)",
            "*.map;*.cns",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::XplorMap),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::BrixMapReader",
            "brix",
            "BRIX Density Map(*.brix;*.omap)",
            "*.brix; *.omap; *.omap.gz",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::BrixMap),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::MTZ2MapReader",
            "mtzmap",
            "MTZ Structure Factor (*.mtz)",
            "*.mtz",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::MtzMap),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfDenMapReader",
            "qdfmap",
            "CueMol density map file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::QdfMap),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::MSMSFileReader",
            "msms",
            "MSMS Surface (*.vert;*.face)",
            "*.vert; *.face",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::MsmsSurface),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::PLYFileReader",
            "ply",
            "PLY polygon file (*.ply)",
            "*.ply",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::PlySurface),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfSurfReader",
            "qdfsurf",
            "CueMol molsurf file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::QdfSurf),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfPotReader",
            "qdfpot",
            "CueMol elepot file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::QdfPot),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfLWObjReader",
            "qdflwobj",
            "CueMol light-weight obj file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::QdfLwObj),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::SceneXMLReader",
            "qsc_xml",
            "CueMol Scene (*.qsc;*.qsl)",
            "*.qsc; *.qsl",
            HandlerCategory::SceneReader,
            Some(HandlerSniffer::QscXml),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::PSEFileReader",
            "psefile",
            "PyMOL Session (*.pse)",
            "*.pse",
            HandlerCategory::SceneReader,
            Some(HandlerSniffer::Pse),
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::PDBFileWriter",
            "pdb",
            "PDB coordinates (*.pdb)",
            "*.pdb",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::SDFMolWriter",
            "sdf",
            "MOL/SDF Coordinates (*.mol;*.sdf)",
            "*.mol; *.sdf",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfMolWriter",
            "qdfmol",
            "CueMol coordinates file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::XYZRFileWriter",
            "xyzr",
            "XYZR coordinates(*.xyzr)",
            "*.xyzr",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::PQRFileWriter",
            "pqr",
            "PQR coordinates(*.pqr)",
            "*.pqr",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfSurfWriter",
            "qdfsurf",
            "CueMol molsurf file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfPotWriter",
            "qdfpot",
            "CueMol elepot file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfDenMapWriter",
            "qdfmap",
            "CueMol density map file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::QdfLWObjWriter",
            "qdflwobj",
            "CueMol light-weight obj file (*.qdf)",
            "*.qdf",
            HandlerCategory::ObjWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::SceneXMLWriter",
            "qsc_xml",
            "CueMol Scene (*.qsc)",
            "*.qsc",
            HandlerCategory::SceneWriter,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::MqoSceneExporter",
            "mqo",
            "Metasequoia object file (*.mqo)",
            "*.mqo",
            HandlerCategory::RendToFile,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::PngSceneExporter",
            "png",
            "Portable Network Graphics image (*.png)",
            "*.png",
            HandlerCategory::RendToFile,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::UmbreonSceneExporter",
            "umbreon",
            "Umbreon ray-traced image (*.png)",
            "*.png",
            HandlerCategory::RendToFile,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::PovSceneExporter",
            "pov",
            "POV-Ray SDL (*.pov)",
            "*.pov",
            HandlerCategory::RendToFile,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::WbpSceneExporter",
            "wbp",
            "Warabi project file (*.wbp)",
            "*.wbp",
            HandlerCategory::RendToFile,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::StlSceneExporter",
            "stl",
            "StereoLithography (*.stl)",
            "*.stl",
            HandlerCategory::RendToFile,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::LuxRendSceneExporter",
            "luxrend",
            "LuxRender Scene (*.lxs)",
            "*.lxs",
            HandlerCategory::RendToFile,
            None,
        ),
        StreamHandlerInfo::new(
            "cuemol_rs::LuxCoreSceneExporter",
            "luxcore",
            "LuxCore Scene CFG (*.cfg)",
            "*.cfg",
            HandlerCategory::RendToFile,
            None,
        ),
    ]
}

fn sniff_text(input: &str, sniffer: HandlerSniffer) -> ContentVerdict {
    match sniffer {
        HandlerSniffer::Pdb => sniff_pdb(input),
        HandlerSniffer::MmcifMol => sniff_mmcif_mol(input),
        HandlerSniffer::Sdf => sniff_sdf(input),
        HandlerSniffer::Mol2 => sniff_mol2(input),
        HandlerSniffer::Gro => sniff_gro(input),
        HandlerSniffer::Psf => sniff_psf(input),
        HandlerSniffer::AmberPrmtop => sniff_amber_prmtop(input.as_bytes()),
        HandlerSniffer::NamdCoor => sniff_namd_coor(input.as_bytes()),
        HandlerSniffer::Dcd => sniff_dcd(input.as_bytes()),
        HandlerSniffer::Trr => sniff_trr(input.as_bytes()),
        HandlerSniffer::Xtc => sniff_xtc(input.as_bytes()),
        HandlerSniffer::AmberNetcdf => sniff_amber_netcdf(input.as_bytes()),
        HandlerSniffer::OpenDx => sniff_opendx(input),
        HandlerSniffer::MmcifMap => sniff_mmcif_map(input),
        HandlerSniffer::Ccp4Map => sniff_ccp4_map(input.as_bytes()),
        HandlerSniffer::XplorMap => sniff_xplor_map(input),
        HandlerSniffer::BrixMap => sniff_brix_map(input),
        HandlerSniffer::MtzMap => sniff_mtz_map(input.as_bytes()),
        HandlerSniffer::MsmsSurface => sniff_msms_surface(input),
        HandlerSniffer::PlySurface => sniff_ply_surface(input),
        HandlerSniffer::QdfMol => sniff_qdf_file_types(input.as_bytes(), &[b"MOL2"]),
        HandlerSniffer::QdfMap => sniff_qdf_file_types(input.as_bytes(), &[b"MAP1"]),
        HandlerSniffer::QdfSurf => sniff_qdf_file_types(input.as_bytes(), &[b"SRF1"]),
        HandlerSniffer::QdfPot => sniff_qdf_file_types(input.as_bytes(), &[b"POT1"]),
        HandlerSniffer::QdfLwObj => {
            sniff_qdf_file_types(input.as_bytes(), &[b"LWO1", b"LWO2", b"LWO3"])
        }
        HandlerSniffer::QscXml => sniff_qsc_xml(input),
        HandlerSniffer::Pse => sniff_pse(input.as_bytes()),
    }
}

fn sniff_bytes(bytes: &[u8], sniffer: HandlerSniffer) -> ContentVerdict {
    match sniffer {
        HandlerSniffer::AmberPrmtop => sniff_amber_prmtop(bytes),
        HandlerSniffer::NamdCoor => sniff_namd_coor(bytes),
        HandlerSniffer::Dcd => sniff_dcd(bytes),
        HandlerSniffer::Trr => sniff_trr(bytes),
        HandlerSniffer::Xtc => sniff_xtc(bytes),
        HandlerSniffer::AmberNetcdf => sniff_amber_netcdf(bytes),
        HandlerSniffer::Ccp4Map => sniff_ccp4_map(bytes),
        HandlerSniffer::MtzMap => sniff_mtz_map(bytes),
        HandlerSniffer::QdfMol => sniff_qdf_file_types(bytes, &[b"MOL2"]),
        HandlerSniffer::QdfMap => sniff_qdf_file_types(bytes, &[b"MAP1"]),
        HandlerSniffer::QdfSurf => sniff_qdf_file_types(bytes, &[b"SRF1"]),
        HandlerSniffer::QdfPot => sniff_qdf_file_types(bytes, &[b"POT1"]),
        HandlerSniffer::QdfLwObj => sniff_qdf_file_types(bytes, &[b"LWO1", b"LWO2", b"LWO3"]),
        HandlerSniffer::Pse => sniff_pse(bytes),
        _ => {
            let input = String::from_utf8_lossy(bytes);
            sniff_text(input.as_ref(), sniffer)
        }
    }
}

fn sniff_pdb(input: &str) -> ContentVerdict {
    for line in input.lines() {
        if line.starts_with("data_") {
            return ContentVerdict::No;
        }
        if line.len() < 6 {
            continue;
        }
        let record = &line[..6];
        if matches!(
            record,
            "HEADER"
                | "REMARK"
                | "CRYST1"
                | "HETATM"
                | "ANISOU"
                | "SSBOND"
                | "HELIX "
                | "SHEET "
                | "TITLE "
                | "MODEL "
                | "ATOM  "
        ) {
            return ContentVerdict::Yes;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_sdf(input: &str) -> ContentVerdict {
    let mut lines = input.lines();
    for _ in 0..3 {
        if lines.next().is_none() {
            return ContentVerdict::Unknown;
        }
    }
    let Some(counts) = lines.next() else {
        return ContentVerdict::Unknown;
    };
    match counts.get(33..39).unwrap_or("") {
        " V2000" => ContentVerdict::Yes,
        " V3000" => ContentVerdict::No,
        _ => ContentVerdict::Unknown,
    }
}

fn sniff_mol2(input: &str) -> ContentVerdict {
    for line in input.lines().map(str::trim_end) {
        if line == "@<TRIPOS>MOLECULE" {
            return ContentVerdict::Yes;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_gro(input: &str) -> ContentVerdict {
    let mut lines = input.lines();
    if lines.next().is_none() {
        return ContentVerdict::Unknown;
    }
    let Some(count_line) = lines.next() else {
        return ContentVerdict::Unknown;
    };
    let Ok(atom_count) = count_line.trim().parse::<isize>() else {
        return ContentVerdict::No;
    };
    if atom_count < 0 {
        return ContentVerdict::No;
    }
    if atom_count == 0 {
        return ContentVerdict::Unknown;
    }
    let Some(atom_line) = lines.next() else {
        return ContentVerdict::No;
    };
    if atom_line.len() < 44 {
        return ContentVerdict::No;
    }
    for start in [20, 28, 36] {
        if atom_line
            .get(start..start + 8)
            .unwrap_or("")
            .trim()
            .parse::<f64>()
            .is_err()
        {
            return ContentVerdict::No;
        }
    }
    ContentVerdict::Yes
}

fn sniff_psf(input: &str) -> ContentVerdict {
    let Some(first) = input.lines().find(|line| !line.trim().is_empty()) else {
        return ContentVerdict::Unknown;
    };
    if !first.trim_start().starts_with("PSF") {
        return ContentVerdict::No;
    }
    for line in input.lines() {
        if line
            .split_once('!')
            .map(|(_, marker)| {
                marker
                    .trim_start()
                    .to_ascii_uppercase()
                    .starts_with("NATOM")
            })
            .unwrap_or(false)
        {
            return ContentVerdict::Yes;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_amber_prmtop(bytes: &[u8]) -> ContentVerdict {
    if bytes.is_empty() {
        return ContentVerdict::Unknown;
    }
    if bytes.contains(&0) {
        return ContentVerdict::No;
    }

    let input = String::from_utf8_lossy(bytes);
    for line in input.lines().map(str::trim_start) {
        if line.starts_with("%VERSION") || line.starts_with("%FLAG") {
            return ContentVerdict::Yes;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_namd_coor(bytes: &[u8]) -> ContentVerdict {
    if bytes.len() < 28 {
        return ContentVerdict::Unknown;
    }
    if sniff_dcd(bytes) == ContentVerdict::Yes {
        return ContentVerdict::No;
    }
    if is_valid_namd_coor_header(bytes, false) || is_valid_namd_coor_header(bytes, true) {
        ContentVerdict::Yes
    } else {
        ContentVerdict::Unknown
    }
}

fn sniff_dcd(bytes: &[u8]) -> ContentVerdict {
    if bytes.len() < 8 {
        return ContentVerdict::Unknown;
    }
    let marker = bytes[..4].try_into().unwrap();
    if (u32::from_le_bytes(marker) == 84 || u32::from_be_bytes(marker) == 84)
        && bytes.get(4..8) == Some(b"CORD".as_slice())
    {
        ContentVerdict::Yes
    } else {
        ContentVerdict::Unknown
    }
}

fn sniff_trr(bytes: &[u8]) -> ContentVerdict {
    match bytes.get(..4).and_then(|value| value.try_into().ok()) {
        Some(value) if i32::from_be_bytes(value) == 1993 => ContentVerdict::Yes,
        _ => ContentVerdict::Unknown,
    }
}

fn sniff_xtc(bytes: &[u8]) -> ContentVerdict {
    match bytes
        .get(..4)
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map(i32::from_be_bytes)
    {
        Some(1995 | 2023) => ContentVerdict::Yes,
        _ => ContentVerdict::Unknown,
    }
}

fn sniff_amber_netcdf(bytes: &[u8]) -> ContentVerdict {
    match bytes.get(..4) {
        Some([b'C', b'D', b'F', 1 | 2 | 5]) => ContentVerdict::Yes,
        _ => ContentVerdict::Unknown,
    }
}

fn is_valid_namd_coor_header(bytes: &[u8], big_endian: bool) -> bool {
    let natoms = read_i32(bytes, 0, big_endian);
    if !(1..=10_000_000).contains(&natoms) {
        return false;
    }

    [4, 12, 20]
        .into_iter()
        .map(|offset| read_f64(bytes, offset, big_endian))
        .all(|value| value.is_finite() && value.abs() <= 10_000.0)
}

fn read_i32(bytes: &[u8], offset: usize, big_endian: bool) -> i32 {
    let field = bytes[offset..offset + 4].try_into().unwrap();
    if big_endian {
        i32::from_be_bytes(field)
    } else {
        i32::from_le_bytes(field)
    }
}

fn read_f64(bytes: &[u8], offset: usize, big_endian: bool) -> f64 {
    let field = bytes[offset..offset + 8].try_into().unwrap();
    if big_endian {
        f64::from_be_bytes(field)
    } else {
        f64::from_le_bytes(field)
    }
}

fn sniff_mmcif_mol(input: &str) -> ContentVerdict {
    for line in input.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("_atom_site.") {
            return ContentVerdict::Yes;
        }
        if line.starts_with("_refln.") {
            return ContentVerdict::No;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_mmcif_map(input: &str) -> ContentVerdict {
    for line in input.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("_refln.") {
            return ContentVerdict::Yes;
        }
        if line.starts_with("_atom_site.") {
            return ContentVerdict::No;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_ccp4_map(bytes: &[u8]) -> ContentVerdict {
    if bytes.get(208..212) == Some(b"MAP ".as_slice()) {
        ContentVerdict::Yes
    } else {
        ContentVerdict::Unknown
    }
}

fn sniff_xplor_map(input: &str) -> ContentVerdict {
    for line in input.lines().map(str::trim) {
        if line == "ZYX" {
            return ContentVerdict::Yes;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_brix_map(input: &str) -> ContentVerdict {
    match input
        .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
        .find(|part| !part.is_empty())
    {
        Some(":-)") => ContentVerdict::Yes,
        _ => ContentVerdict::Unknown,
    }
}

fn sniff_mtz_map(bytes: &[u8]) -> ContentVerdict {
    if bytes.get(0..4) == Some(b"MTZ ".as_slice()) {
        ContentVerdict::Yes
    } else {
        ContentVerdict::Unknown
    }
}

fn sniff_msms_surface(input: &str) -> ContentVerdict {
    if input.is_empty() {
        return ContentVerdict::Unknown;
    }

    for line in input.lines() {
        let line = line.trim_start();
        if line.starts_with('#') {
            if line.starts_with("# MSMS") {
                return ContentVerdict::Yes;
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        return ContentVerdict::Unknown;
    }
    ContentVerdict::Unknown
}

fn sniff_ply_surface(input: &str) -> ContentVerdict {
    match input.lines().map(str::trim).find(|line| !line.is_empty()) {
        Some("ply") => ContentVerdict::Yes,
        _ => ContentVerdict::Unknown,
    }
}

fn sniff_qdf_file_types(bytes: &[u8], expected_types: &[&[u8; 4]]) -> ContentVerdict {
    let Some(found) = qdf_file_type_bytes(bytes) else {
        return ContentVerdict::Unknown;
    };
    if expected_types
        .iter()
        .any(|expected| found == expected.as_slice())
    {
        ContentVerdict::Yes
    } else {
        ContentVerdict::No
    }
}

fn qdf_file_type_bytes(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.len() < 26 {
        return None;
    }
    let version = parse_qdf_version(bytes.get(0..4).unwrap_or_default())?;
    let mut offset = if version == 0 {
        4
    } else {
        if bytes.len() < 28 {
            return None;
        }
        if bytes.get(4..6) != Some(b"00".as_slice()) {
            return None;
        }
        6
    };

    let &int_bo = bytes.get(offset)?;
    offset += 1;
    let marker = read_qdf_i32(bytes, offset, int_bo)?;
    if marker != 12_345_678 {
        return None;
    }
    offset += 4;
    let &float_bo = bytes.get(offset)?;
    if float_bo != int_bo {
        return None;
    }
    offset += 1;
    offset += 4; // float byte-order marker
    offset += 4; // dummy marker

    let type_len = read_qdf_i32(bytes, offset, int_bo)?;
    offset += 4;
    if type_len != 4 {
        return None;
    }
    bytes.get(offset..offset + 4)
}

fn parse_qdf_version(header: &[u8]) -> Option<u16> {
    let text = std::str::from_utf8(header).ok()?;
    let digits = text
        .strip_prefix("QDF")
        .or_else(|| text.strip_prefix("QD"))
        .or_else(|| text.strip_prefix('Q'))?;
    u16::from_str_radix(digits, 16).ok()
}

fn read_qdf_i32(bytes: &[u8], offset: usize, int_bo: u8) -> Option<i32> {
    let field: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    match int_bo {
        0 => Some(i32::from_le_bytes(field)),
        1 => Some(i32::from_be_bytes(field)),
        _ => None,
    }
}

fn sniff_opendx(input: &str) -> ContentVerdict {
    for line in input
        .lines()
        .map(|line| line.trim_end_matches(['\r', '\n']))
    {
        if line.starts_with("object 1 class gridpositions") {
            return ContentVerdict::Yes;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_qsc_xml(input: &str) -> ContentVerdict {
    for line in input.lines().map(str::trim) {
        if line.starts_with("<scene") || line.starts_with("<?xml") {
            return ContentVerdict::Yes;
        }
        if line.starts_with("version https://git-lfs.github.com/spec/v1") {
            return ContentVerdict::No;
        }
    }
    ContentVerdict::Unknown
}

fn sniff_pse(bytes: &[u8]) -> ContentVerdict {
    if bytes.is_empty() {
        return ContentVerdict::Unknown;
    }
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return ContentVerdict::Yes;
    }
    if bytes.first() == Some(&0x80) {
        return match bytes.get(1).copied() {
            Some(0..=5) => ContentVerdict::Yes,
            Some(_) => ContentVerdict::No,
            None => ContentVerdict::Unknown,
        };
    }
    ContentVerdict::Unknown
}

fn read_bytes(path: impl AsRef<Path>, max_bytes: u64) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    if max_bytes == 0 {
        file.read_to_end(&mut bytes)?;
    } else {
        file.take(max_bytes).read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

fn read_search_bytes(
    path: impl AsRef<Path>,
    max_bytes: u64,
    support_compression: bool,
) -> Result<Vec<u8>> {
    let path = path.as_ref();
    if !support_compression {
        return read_bytes(path, max_bytes);
    }

    let encoding = detect_compression(path)?;
    let mut bytes = match encoding {
        CompressionEncoding::Raw => read_bytes(path, max_bytes)?,
        CompressionEncoding::Gzip => cuemol_core::decode_gzip_bytes(&read_bytes(path, 0)?)?,
        CompressionEncoding::Bzip2 => decompress_with_command("bzip2", path)?,
        CompressionEncoding::Xz => cuemol_core::decode_xz_bytes(&read_bytes(path, 0)?)?,
    };
    if max_bytes > 0 && bytes.len() > max_bytes as usize {
        bytes.truncate(max_bytes as usize);
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionEncoding {
    Raw,
    Gzip,
    Bzip2,
    Xz,
}

fn detect_compression(path: &Path) -> Result<CompressionEncoding> {
    let mut file = File::open(path)?;
    let mut magic = [0_u8; 6];
    let nread = file.read(&mut magic)?;
    if nread >= 2 && magic[0] == 0x1f && magic[1] == 0x8b {
        return Ok(CompressionEncoding::Gzip);
    }
    if nread >= 3 && magic[..3] == *b"BZh" {
        return Ok(CompressionEncoding::Bzip2);
    }
    if nread >= 6 && magic == [0xfd, b'7', b'z', b'X', b'Z', 0x00] {
        return Ok(CompressionEncoding::Xz);
    }
    Ok(CompressionEncoding::Raw)
}

fn decompress_with_command(program: &str, path: &Path) -> Result<Vec<u8>> {
    let output = Command::new(program).arg("-cd").arg(path).output()?;
    if !output.status.success() {
        return Err(CueMolError::invalid_argument(format!(
            "failed to decompress {} with {program}",
            path.display()
        )));
    }
    Ok(output.stdout)
}

fn has_compressed_extension(path: &Path) -> bool {
    let path = path.to_string_lossy().to_ascii_lowercase();
    path.ends_with(".gz") || path.ends_with(".bz2") || path.ends_with(".xz")
}

fn parse_nickname_csv(input: &str) -> Vec<String> {
    input
        .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn quoted_list<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values
        .map(|value| format!("\"{}\"", escape_json(value)))
        .collect::<Vec<_>>()
        .join(",\n")
}

fn escape_json(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn node_to_byte_array(node: LDom2Node) -> ByteArray {
    ByteArray::copy_from_u8(write_ldom2_xml(&LDom2Tree::from(node)).as_bytes())
}

fn async_reader_name<R: 'static>(reader: &R) -> Option<String> {
    let any = reader as &dyn Any;
    if let Some(reader) = any.downcast_ref::<ObjReader>() {
        return Some(reader.handler().name().to_string());
    }
    if let Some(value) = any.downcast_ref::<String>() {
        return Some(value.clone());
    }
    if let Some(value) = any.downcast_ref::<&str>() {
        return Some((*value).to_string());
    }
    None
}

fn parse_async_object(reader_name: &str, data: &[u8]) -> Result<AsyncLoadedObject> {
    let name = reader_name.to_ascii_lowercase();
    let utf8 = || {
        std::str::from_utf8(data).map_err(|error| {
            CueMolError::parse(format!("asynchronous input is not UTF-8: {error}"))
        })
    };
    if name == "qdfmol" || name.contains("qdfmol") {
        return Ok(AsyncLoadedObject::Molecule(crate::read_qdf_mol_bytes(
            data,
        )?));
    }
    if name == "qdfpdb" || name.contains("qdfpdb") {
        return Ok(AsyncLoadedObject::Molecule(
            crate::read_qdfpdb_str(utf8()?)?,
        ));
    }
    if name == "pdb" || name.contains("pdbreader") {
        return Ok(AsyncLoadedObject::Molecule(crate::read_pdb_str(utf8()?)?));
    }
    if matches!(name.as_str(), "mmcif" | "cif") || name.contains("mmcifmol") {
        return Ok(AsyncLoadedObject::Molecule(crate::read_mmcif_str(utf8()?)?));
    }
    if matches!(name.as_str(), "sdf" | "mol") || name.contains("sdfreader") {
        return Ok(AsyncLoadedObject::Molecule(crate::read_sdf_str(utf8()?)?));
    }
    if name == "mol2" || name.contains("mol2reader") {
        return Ok(AsyncLoadedObject::Molecule(crate::read_mol2_str(utf8()?)?));
    }
    if name == "apbs" || name.contains("opendx") {
        return Ok(AsyncLoadedObject::GridMap(crate::read_opendx_str(utf8()?)?));
    }
    if name == "ccp4map" || name.contains("ccp4map") {
        return Ok(AsyncLoadedObject::GridMap(crate::read_ccp4_map_bytes(
            data,
        )?));
    }
    if name == "xplormap" || name.contains("xplormap") {
        return Ok(AsyncLoadedObject::GridMap(crate::read_xplor_map_str(
            utf8()?,
        )?));
    }
    if name == "brix" || name.contains("brixmap") {
        return Ok(AsyncLoadedObject::GridMap(crate::read_brix_map_bytes(
            data,
        )?));
    }
    Ok(AsyncLoadedObject::Buffered {
        reader_type: reader_name.to_string(),
        data: ByteArray::copy_from_u8(data),
    })
}

fn renderer_node_from_mol(renderer: &MolRenderer, type_override: Option<&str>) -> LDom2Node {
    let mut node = LDom2Node::new();
    node.set_tag_name("renderer");
    node.set_type_name(type_override.unwrap_or_else(|| renderer.renderer_type()));
    for (name, value) in renderer.qsc_attrs() {
        if name != "type" {
            node.set_str_attr(name, value);
        }
    }
    node.set_str_attr("name", renderer.name());
    if renderer.renderer_type() == "atomintr" {
        for definition in renderer.atom_intr_lines() {
            if !definition.is_active() {
                continue;
            }
            let tag = match definition.kind() {
                AtomIntrKind::Distance => "line",
                AtomIntrKind::Angle => "angle",
                AtomIntrKind::Torsion => "torsion",
            };
            let child = node.append_child(tag);
            if !definition.selectors().is_empty() {
                for (index, selection) in definition.selectors().iter().enumerate() {
                    child.set_str_attr(format!("sel{}", index + 1), selection);
                }
            } else if definition.positions().is_empty() {
                for (index, aid) in definition.aids().iter().enumerate() {
                    child.set_str_attr(format!("aid{}", index + 1), aid);
                }
            } else {
                for (index, position) in definition.positions().iter().enumerate() {
                    child.set_str_attr(format!("pos{}", index + 1), position.to_string());
                }
            }
        }
    }
    node
}

fn renderer_node_from_qsys(renderer: &QsysRenderer, type_override: Option<&str>) -> LDom2Node {
    let mut node = LDom2Node::new();
    node.set_tag_name("renderer");
    node.set_type_name(type_override.unwrap_or_else(|| renderer.type_name()));
    node.set_str_attr("name", renderer.name());
    node.set_str_attr("group", renderer.group_name());
    node.set_str_attr("style", renderer.style_names());
    node.set_str_attr("visible", renderer.visible().to_string());
    node.set_str_attr("locked", renderer.is_ui_locked().to_string());
    node.set_str_attr("default_material", renderer.default_material());
    node.set_str_attr("default_alpha", renderer.default_alpha().to_string());
    node.set_str_attr("xformMat", renderer.xform_matrix().to_script_string());
    node.set_str_attr("ui_order", renderer.ui_order().to_string());
    for (name, value) in renderer.properties() {
        node.set_str_attr(name, value);
    }
    node
}

fn renderer_nodes<T: 'static>(array: &[T], group_name: &str) -> Option<Vec<LDom2Node>> {
    let mut nodes = Vec::with_capacity(array.len());
    for value in array {
        let any = value as &dyn Any;
        let mut node = if let Some(renderer) = any.downcast_ref::<MolRenderer>() {
            renderer_node_from_mol(renderer, None)
        } else if let Some(renderer) = any.downcast_ref::<QsysRenderer>() {
            renderer_node_from_qsys(renderer, None)
        } else {
            return None;
        };
        if group_name.is_empty() {
            node.remove_child("group");
        } else {
            node.set_str_attr("group", group_name);
        }
        nodes.push(node);
    }
    Some(nodes)
}

fn renderer_nodes_to_byte_array(nodes: Vec<LDom2Node>, group_name: &str) -> ByteArray {
    let mut root = LDom2Node::new();
    root.set_tag_name("renderers");
    if !group_name.is_empty() {
        root.set_str_attr("group", group_name);
    }
    for node in nodes {
        root.append_child_node(node);
    }
    node_to_byte_array(root)
}

fn parse_xml_byte_array(data: &ByteArray) -> Result<LDom2Tree> {
    let xml = std::str::from_utf8(data.as_bytes())
        .map_err(|error| CueMolError::parse(format!("XML is not UTF-8: {error}")))?;
    parse_ldom2_xml(xml)
}

fn renderer_from_node(node: &LDom2Node) -> Result<QsysRenderer> {
    if node.tag_name() != "renderer" || node.type_name().is_empty() {
        return Err(CueMolError::parse("renderer XML has no renderer type"));
    }
    let mut renderer = QsysRenderer::new(node.type_name());
    renderer.read_properties_from_node(node);
    Ok(renderer)
}

fn unescape_xml(input: &str) -> String {
    input
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

fn extract_xml_attr(input: &str, tag: &str, attr: &str) -> Option<String> {
    let tag_start = format!("<{tag}");
    let start = input.find(&tag_start)?;
    let end = input[start..].find('>')? + start;
    let tag_text = &input[start..end];
    let attr_start = format!("{attr}=\"");
    let value_start = tag_text.find(&attr_start)? + attr_start.len();
    let value_end = tag_text[value_start..].find('"')? + value_start;
    Some(unescape_xml(&tag_text[value_start..value_end]))
}
