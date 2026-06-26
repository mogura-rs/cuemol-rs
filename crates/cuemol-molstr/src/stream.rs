use cuemol_core::{ByteArray, CueMolError, Result};
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
    OpenDx,
    MmcifMap,
    Ccp4Map,
    XplorMap,
    BrixMap,
    MtzMap,
    MsmsSurface,
    QdfSurf,
    QdfPot,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncLoadTask {
    reader_type: String,
    data: Vec<u8>,
    eos: bool,
}

impl AsyncLoadTask {
    fn new(reader_type: impl Into<String>) -> Self {
        Self {
            reader_type: reader_type.into(),
            data: Vec::new(),
            eos: false,
        }
    }

    pub fn reader_type(&self) -> &str {
        &self.reader_type
    }

    pub fn byte_len(&self) -> usize {
        self.data.len()
    }

    pub fn is_eos(&self) -> bool {
        self.eos
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamManager {
    handlers: Vec<StreamHandlerInfo>,
    next_async_load_id: i32,
    async_loads: BTreeMap<i32, AsyncLoadTask>,
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

    pub fn load_object_async<R>(&mut self, _reader: &R) -> i32 {
        let id = self.next_async_load_id;
        self.next_async_load_id = self.next_async_load_id.saturating_add(1).max(1);
        self.async_loads
            .insert(id, AsyncLoadTask::new(std::any::type_name::<R>()));
        id
    }

    pub fn supply_data_async(&mut self, load_id: i32, data: &[u8]) -> bool {
        let Some(task) = self.async_loads.get_mut(&load_id) else {
            return false;
        };
        if task.eos {
            return false;
        }
        task.data.extend_from_slice(data);
        true
    }

    pub fn wait_load_async(&mut self, load_id: i32) -> Option<ByteArray> {
        let mut task = self.async_loads.remove(&load_id)?;
        task.eos = true;
        Some(ByteArray::copy_from_u8(&task.data))
    }

    pub fn async_load(&self, load_id: i32) -> Option<&AsyncLoadTask> {
        self.async_loads.get(&load_id)
    }

    pub fn to_xml<T>(&self, _object: &T) -> ByteArray {
        ByteArray::copy_from_u8(
            format!(
                "<object type=\"{}\"/>",
                escape_xml(std::any::type_name::<T>())
            )
            .as_bytes(),
        )
    }

    pub fn to_xml_2<T>(&self, object: &T, type_override: &str) -> ByteArray {
        if type_override.trim().is_empty() {
            return self.to_xml(object);
        }
        ByteArray::copy_from_u8(
            format!("<object type=\"{}\"/>", escape_xml(type_override)).as_bytes(),
        )
    }

    pub fn array_to_xml<T>(&self, array: &[T]) -> ByteArray {
        ByteArray::copy_from_u8(
            format!(
                "<array type=\"{}\" size=\"{}\"/>",
                escape_xml(std::any::type_name::<T>()),
                array.len()
            )
            .as_bytes(),
        )
    }

    pub fn rend_grp_to_xml<T>(&self, array: &[T], group_name: &str) -> ByteArray {
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

    pub fn rend_array_from_xml(&self, data: &ByteArray, _flags: i32) -> Vec<String> {
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
            "cuemol_rs::MSMSFileReader",
            "msms",
            "MSMS Surface (*.vert;*.face)",
            "*.vert; *.face",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::MsmsSurface),
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
            "cuemol_rs::SceneXMLWriter",
            "qsc_xml",
            "CueMol Scene (*.qsc)",
            "*.qsc",
            HandlerCategory::SceneWriter,
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
        HandlerSniffer::OpenDx => sniff_opendx(input),
        HandlerSniffer::MmcifMap => sniff_mmcif_map(input),
        HandlerSniffer::Ccp4Map => sniff_ccp4_map(input.as_bytes()),
        HandlerSniffer::XplorMap => sniff_xplor_map(input),
        HandlerSniffer::BrixMap => sniff_brix_map(input),
        HandlerSniffer::MtzMap => sniff_mtz_map(input.as_bytes()),
        HandlerSniffer::MsmsSurface => sniff_msms_surface(input),
        HandlerSniffer::QdfSurf => sniff_qdf_file_type(input.as_bytes(), b"SRF1"),
        HandlerSniffer::QdfPot => sniff_qdf_file_type(input.as_bytes(), b"POT1"),
        HandlerSniffer::QscXml => sniff_qsc_xml(input),
        HandlerSniffer::Pse => sniff_pse(input.as_bytes()),
    }
}

fn sniff_bytes(bytes: &[u8], sniffer: HandlerSniffer) -> ContentVerdict {
    match sniffer {
        HandlerSniffer::AmberPrmtop => sniff_amber_prmtop(bytes),
        HandlerSniffer::NamdCoor => sniff_namd_coor(bytes),
        HandlerSniffer::Ccp4Map => sniff_ccp4_map(bytes),
        HandlerSniffer::MtzMap => sniff_mtz_map(bytes),
        HandlerSniffer::QdfSurf => sniff_qdf_file_type(bytes, b"SRF1"),
        HandlerSniffer::QdfPot => sniff_qdf_file_type(bytes, b"POT1"),
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
    if is_valid_namd_coor_header(bytes, false) || is_valid_namd_coor_header(bytes, true) {
        ContentVerdict::Yes
    } else {
        ContentVerdict::Unknown
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

fn sniff_qdf_file_type(bytes: &[u8], expected_type: &[u8; 4]) -> ContentVerdict {
    if bytes.len() < 26 {
        return ContentVerdict::Unknown;
    }
    let version = match parse_qdf_version(bytes.get(0..4).unwrap_or_default()) {
        Some(version) => version,
        None => return ContentVerdict::Unknown,
    };
    let mut offset = if version == 0 {
        4
    } else {
        if bytes.len() < 28 {
            return ContentVerdict::Unknown;
        }
        if bytes.get(4..6) != Some(b"00".as_slice()) {
            return ContentVerdict::Unknown;
        }
        6
    };

    let Some(&int_bo) = bytes.get(offset) else {
        return ContentVerdict::Unknown;
    };
    offset += 1;
    let Some(marker) = read_qdf_i32(bytes, offset, int_bo) else {
        return ContentVerdict::Unknown;
    };
    if marker != 12_345_678 {
        return ContentVerdict::Unknown;
    }
    offset += 4;
    let Some(&float_bo) = bytes.get(offset) else {
        return ContentVerdict::Unknown;
    };
    if float_bo != int_bo {
        return ContentVerdict::Unknown;
    }
    offset += 1;
    offset += 4; // float byte-order marker
    offset += 4; // dummy marker

    let Some(type_len) = read_qdf_i32(bytes, offset, int_bo) else {
        return ContentVerdict::Unknown;
    };
    offset += 4;
    if type_len != 4 {
        return ContentVerdict::Unknown;
    }
    match bytes.get(offset..offset + 4) {
        Some(found) if found == expected_type => ContentVerdict::Yes,
        Some(b"SRF1") | Some(b"POT1") => ContentVerdict::No,
        _ => ContentVerdict::Unknown,
    }
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
        CompressionEncoding::Gzip => decompress_with_command("gzip", path)?,
        CompressionEncoding::Bzip2 => decompress_with_command("bzip2", path)?,
        CompressionEncoding::Xz => decompress_with_command("xz", path)?,
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
