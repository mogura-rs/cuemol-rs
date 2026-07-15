use cuemol_core::{CueMolError, Result, Uid};
use cuemol_gfx::SolidColor;
use cuemol_molstr::{Atom, Selector};
use cuemol_qsys::{InOutHandler, ObjReader, ObjWriter};
use std::fs;
use std::path::Path;
use std::process::Command;

pub mod pickle;
pub use pickle::{parse_pickle, PickleValue};
pub mod pse_extract;
pub use pse_extract::{extract_pse_molecules, extract_pse_named_molecules};

pub use cuemol_molstr::{
    read_mmcif_file, read_mmcif_file_with_options, read_mmcif_str, read_mmcif_str_with_options,
    read_mol2_file, read_mol2_file_with_options, read_mol2_str, read_mol2_str_with_options,
    read_opendx_file, read_opendx_str, read_pdb_file, read_pdb_str, read_sdf_file,
    read_sdf_file_with_options, read_sdf_str, read_sdf_str_with_options, write_pdb_file,
    write_pdb_string, write_sdf_string, CompressionMode, ContentVerdict, HandlerCategory,
    HandlerSniffer, MmcifMapStatus, MmcifReadOptions, MmcifReader, Mol2ReadOptions, Mol2Reader,
    PdbReader, PdbWriter, SdfReadOptions, SdfReader, SdfWriter, StreamHandlerInfo, StreamManager,
};

pub fn default_stream_manager() -> StreamManager {
    StreamManager::new()
}

pub type MOL2MolReader = Mol2MolReader;
pub type PSEFileReader = PseFileReader;
pub type SDFMolReader = SdfMolReader;
pub type SDFMolWriter = SdfMolWriter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mol2MolReader {
    reader: ObjReader,
    load_compound_id: i32,
    load_as_chain: bool,
    chain_name: String,
    residue_index: i32,
}

impl Mol2MolReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("mol2", "SYBYL Mol2 Coordinates (*.mol2)", "*.mol2"),
            load_compound_id: -1,
            load_as_chain: true,
            chain_name: "_".to_string(),
            residue_index: 1,
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        &mut self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn load_compound_id(&self) -> i32 {
        self.load_compound_id
    }

    pub fn load_cmpd_id(&self) -> i32 {
        self.load_compound_id()
    }

    pub fn set_load_compound_id(&mut self, value: i32) {
        self.load_compound_id = value;
    }

    pub fn set_load_cmpd_id(&mut self, value: i32) {
        self.set_load_compound_id(value);
    }

    pub fn load_as_chain(&self) -> bool {
        self.load_as_chain
    }

    pub fn set_load_as_chain(&mut self, value: bool) {
        self.load_as_chain = value;
    }

    pub fn chain_name(&self) -> &str {
        &self.chain_name
    }

    pub fn set_chain_name(&mut self, value: impl Into<String>) {
        self.chain_name = value.into();
    }

    pub fn residue_index(&self) -> i32 {
        self.residue_index
    }

    pub fn resid_index(&self) -> i32 {
        self.residue_index()
    }

    pub fn set_residue_index(&mut self, value: i32) {
        self.residue_index = value;
    }

    pub fn set_resid_index(&mut self, value: i32) {
        self.set_residue_index(value);
    }

    pub fn options(&self) -> Mol2ReadOptions {
        Mol2ReadOptions {
            load_compound_id: self.load_compound_id,
            load_as_chain: self.load_as_chain,
            chain_name: self.chain_name.clone(),
            residue_index: self.residue_index,
        }
    }

    pub fn read_str_configured(&self, input: &str) -> cuemol_core::Result<cuemol_molstr::MolCoord> {
        read_mol2_str_with_options(input, &self.options())
    }

    pub fn read_file_configured(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<cuemol_molstr::MolCoord> {
        read_mol2_file_with_options(path, &self.options())
    }
}

impl Default for Mol2MolReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmcifMolReader {
    reader: ObjReader,
    load_model: bool,
    load_anisou: bool,
    load_alt_conf: bool,
    load_secstr: bool,
    auto_topo_gen: bool,
}

impl MmcifMolReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new(
                "mmcif",
                "mmCIF Coordinates (*.cif;*.cif.gz)",
                "*.cif; *.cif.gz",
            ),
            load_model: false,
            load_anisou: true,
            load_alt_conf: true,
            load_secstr: false,
            auto_topo_gen: true,
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn load_model(&self) -> bool {
        self.load_model
    }

    pub fn loadmodel(&self) -> bool {
        self.load_model()
    }

    pub fn set_load_model(&mut self, value: bool) {
        self.load_model = value;
    }

    pub fn set_loadmodel(&mut self, value: bool) {
        self.set_load_model(value);
    }

    pub fn load_anisou(&self) -> bool {
        self.load_anisou
    }

    pub fn loadanisou(&self) -> bool {
        self.load_anisou()
    }

    pub fn set_load_anisou(&mut self, value: bool) {
        self.load_anisou = value;
    }

    pub fn set_loadanisou(&mut self, value: bool) {
        self.set_load_anisou(value);
    }

    pub fn load_alt_conf(&self) -> bool {
        self.load_alt_conf
    }

    pub fn loadaltconf(&self) -> bool {
        self.load_alt_conf()
    }

    pub fn set_load_alt_conf(&mut self, value: bool) {
        self.load_alt_conf = value;
    }

    pub fn set_loadaltconf(&mut self, value: bool) {
        self.set_load_alt_conf(value);
    }

    pub fn load_secstr(&self) -> bool {
        self.load_secstr
    }

    pub fn loadsecstr(&self) -> bool {
        self.load_secstr()
    }

    pub fn set_load_secstr(&mut self, value: bool) {
        self.load_secstr = value;
    }

    pub fn set_loadsecstr(&mut self, value: bool) {
        self.set_load_secstr(value);
    }

    pub fn auto_topo_gen(&self) -> bool {
        self.auto_topo_gen
    }

    pub fn set_auto_topo_gen(&mut self, value: bool) {
        self.auto_topo_gen = value;
    }

    pub fn options(&self) -> MmcifReadOptions {
        MmcifReadOptions {
            load_model: self.load_model,
            load_anisou: self.load_anisou,
            load_alt_conf: self.load_alt_conf,
            load_secstr: self.load_secstr,
            auto_topo_gen: self.auto_topo_gen,
        }
    }

    pub fn read_str_configured(&self, input: &str) -> cuemol_core::Result<cuemol_molstr::MolCoord> {
        read_mmcif_str_with_options(input, self.options())
    }

    pub fn read_file_configured(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<cuemol_molstr::MolCoord> {
        read_mmcif_file_with_options(path, self.options())
    }
}

impl Default for MmcifMolReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdfMolReader {
    reader: ObjReader,
    load_compound_id: i32,
    load_as_chain: bool,
    chain_name: String,
    residue_index: i32,
}

impl SdfMolReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("sdf", "MOL/SDF Coordinates (*.mol;*.sdf)", "*.mol; *.sdf"),
            load_compound_id: -1,
            load_as_chain: false,
            chain_name: "_".to_string(),
            residue_index: 1,
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn load_compound_id(&self) -> i32 {
        self.load_compound_id
    }

    pub fn load_cmpd_id(&self) -> i32 {
        self.load_compound_id()
    }

    pub fn set_load_compound_id(&mut self, value: i32) {
        self.load_compound_id = value;
    }

    pub fn set_load_cmpd_id(&mut self, value: i32) {
        self.set_load_compound_id(value);
    }

    pub fn load_as_chain(&self) -> bool {
        self.load_as_chain
    }

    pub fn set_load_as_chain(&mut self, value: bool) {
        self.load_as_chain = value;
    }

    pub fn chain_name(&self) -> &str {
        &self.chain_name
    }

    pub fn set_chain_name(&mut self, value: impl Into<String>) {
        self.chain_name = value.into();
    }

    pub fn residue_index(&self) -> i32 {
        self.residue_index
    }

    pub fn resid_index(&self) -> i32 {
        self.residue_index()
    }

    pub fn set_residue_index(&mut self, value: i32) {
        self.residue_index = value;
    }

    pub fn set_resid_index(&mut self, value: i32) {
        self.set_residue_index(value);
    }

    pub fn options(&self) -> SdfReadOptions {
        SdfReadOptions {
            load_compound_id: self.load_compound_id,
            load_as_chain: self.load_as_chain,
            chain_name: self.chain_name.clone(),
            residue_index: self.residue_index,
        }
    }

    pub fn read_str_configured(&self, input: &str) -> cuemol_core::Result<cuemol_molstr::MolCoord> {
        read_sdf_str_with_options(input, &self.options())
    }

    pub fn read_file_configured(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<cuemol_molstr::MolCoord> {
        read_sdf_file_with_options(path, &self.options())
    }
}

impl Default for SdfMolReader {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! impl_obj_reader_deref {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl std::ops::Deref for $ty {
                type Target = ObjReader;

                fn deref(&self) -> &Self::Target {
                    &self.reader
                }
            }

            impl std::ops::DerefMut for $ty {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.reader
                }
            }
        )+
    };
}

impl_obj_reader_deref!(Mol2MolReader, MmcifMolReader, SdfMolReader);

#[derive(Debug, Clone, PartialEq)]
pub struct SdfMolWriter {
    writer: ObjWriter,
    selection: Selector,
}

impl SdfMolWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("sdf", "MOL/SDF Coordinates (*.mol;*.sdf)", "*.mol; *.sdf"),
            selection: Selector::All,
        }
    }

    pub fn writer(&self) -> &ObjWriter {
        &self.writer
    }

    pub fn writer_mut(&mut self) -> &mut ObjWriter {
        &mut self.writer
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn selection(&self) -> &Selector {
        &self.selection
    }

    pub fn get_selection(&self) -> &Selector {
        self.selection()
    }

    pub fn set_selection(&mut self, value: Selector) {
        self.selection = value;
    }
}

impl Default for SdfMolWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for SdfMolWriter {
    type Target = ObjWriter;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl std::ops::DerefMut for SdfMolWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseFileReader {
    handler: InOutHandler,
    client_scene: Option<Uid>,
    read_count: usize,
    last_status: Option<PseReadStatus>,
    error_msg: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseEncoding {
    Raw,
    Gzip,
    Bzip2,
    Xz,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseReadStatus {
    byte_len: usize,
    encoding: PseEncoding,
    pickle_protocol: Option<u8>,
}

impl PseReadStatus {
    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub fn encoding(&self) -> PseEncoding {
        self.encoding
    }

    pub fn pickle_protocol(&self) -> Option<u8> {
        self.pickle_protocol
    }
}

impl PseFileReader {
    pub fn new() -> Self {
        Self {
            handler: InOutHandler::new(
                "psefile",
                "PyMOL Session (*.pse)",
                "*.pse",
                InOutHandler::IOH_CAT_SCEREADER,
            ),
            client_scene: None,
            read_count: 0,
            last_status: None,
            error_msg: String::new(),
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        &self.handler
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        &mut self.handler
    }

    pub fn set_path(&mut self, value: impl Into<String>) {
        self.handler.set_path(value);
    }

    pub fn path(&self) -> &str {
        self.handler.path()
    }

    pub fn attach_scene(&mut self, scene_uid: Uid) {
        self.client_scene = Some(scene_uid);
    }

    pub fn attach(&mut self, scene_uid: Uid) {
        self.attach_scene(scene_uid);
    }

    pub fn detach_scene(&mut self) -> Option<Uid> {
        self.client_scene.take()
    }

    pub fn detach(&mut self) -> Option<Uid> {
        self.detach_scene()
    }

    pub fn client_scene(&self) -> Option<Uid> {
        self.client_scene
    }

    pub fn read(&mut self) {
        let _ = self.read_checked();
    }

    pub fn read_checked(&mut self) -> Result<PseReadStatus> {
        self.read_count += 1;
        self.error_msg.clear();
        match self.inspect_path() {
            Ok(status) => {
                self.last_status = Some(status.clone());
                Ok(status)
            }
            Err(error) => {
                self.last_status = None;
                self.error_msg = error.to_string();
                Err(error)
            }
        }
    }

    pub fn read_count(&self) -> usize {
        self.read_count
    }

    pub fn last_status(&self) -> Option<&PseReadStatus> {
        self.last_status.as_ref()
    }

    pub fn error_msg(&self) -> &str {
        &self.error_msg
    }

    fn inspect_path(&self) -> Result<PseReadStatus> {
        if self.path().trim().is_empty() {
            return Err(CueMolError::invalid_argument("PSE path is empty"));
        }
        let bytes = fs::read(self.path())?;
        if bytes.is_empty() {
            return Err(CueMolError::parse("PSE file is empty"));
        }
        let encoding = detect_pse_encoding(&bytes);
        let decoded = match encoding {
            PseEncoding::Raw => None,
            PseEncoding::Gzip | PseEncoding::Bzip2 | PseEncoding::Xz => {
                let decoded = decompress_with_command(
                    pse_encoding_command(encoding),
                    Path::new(self.path()),
                )?;
                if decoded.is_empty() {
                    return Err(CueMolError::parse(format!(
                        "PSE {} payload is empty",
                        pse_encoding_label(encoding)
                    )));
                }
                Some(decoded)
            }
        };
        let payload = decoded.as_deref().unwrap_or(bytes.as_slice());
        let pickle_protocol = pickle_protocol(payload);
        Ok(PseReadStatus {
            byte_len: bytes.len(),
            encoding,
            pickle_protocol,
        })
    }

    /// Read the file and return the decompressed (or raw) pickle payload.
    fn decoded_payload(&self) -> Result<Vec<u8>> {
        if self.path().trim().is_empty() {
            return Err(CueMolError::invalid_argument("PSE path is empty"));
        }
        let bytes = fs::read(self.path())?;
        if bytes.is_empty() {
            return Err(CueMolError::parse("PSE file is empty"));
        }
        let encoding = detect_pse_encoding(&bytes);
        if encoding == PseEncoding::Raw {
            return Ok(bytes);
        }
        let decoded =
            decompress_with_command(pse_encoding_command(encoding), Path::new(self.path()))?;
        if decoded.is_empty() {
            return Err(CueMolError::parse(format!(
                "PSE {} payload is empty",
                pse_encoding_label(encoding)
            )));
        }
        Ok(decoded)
    }

    /// Decode the session pickle into a [`PickleValue`] tree (the raw session).
    pub fn parse_pickle_tree(&self) -> Result<PickleValue> {
        parse_pickle(&self.decoded_payload()?)
    }

    /// Read the PyMOL session's molecule objects as [`MolCoord`]s — the pickle VM
    /// parses the session tree and `extract_pse_molecules` reconstructs the
    /// ChemPy molecules (atoms, residues, chains, coordinates, bonds).
    pub fn read_molecules(&self) -> Result<Vec<cuemol_molstr::MolCoord>> {
        let tree = self.parse_pickle_tree()?;
        Ok(extract_pse_molecules(&tree))
    }

    pub fn read_named_molecules(&self) -> Result<Vec<(String, cuemol_molstr::MolCoord)>> {
        let tree = self.parse_pickle_tree()?;
        Ok(extract_pse_named_molecules(&tree))
    }
}

fn detect_pse_encoding(bytes: &[u8]) -> PseEncoding {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return PseEncoding::Gzip;
    }
    if bytes.starts_with(b"BZh") {
        return PseEncoding::Bzip2;
    }
    if bytes.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]) {
        return PseEncoding::Xz;
    }
    PseEncoding::Raw
}

fn pse_encoding_command(encoding: PseEncoding) -> &'static str {
    match encoding {
        PseEncoding::Raw => "",
        PseEncoding::Gzip => "gzip",
        PseEncoding::Bzip2 => "bzip2",
        PseEncoding::Xz => "xz",
    }
}

fn pse_encoding_label(encoding: PseEncoding) -> &'static str {
    match encoding {
        PseEncoding::Raw => "raw",
        PseEncoding::Gzip => "gzip",
        PseEncoding::Bzip2 => "bzip2",
        PseEncoding::Xz => "xz",
    }
}

fn pickle_protocol(bytes: &[u8]) -> Option<u8> {
    bytes
        .starts_with(&[0x80])
        .then(|| bytes.get(1).copied())
        .flatten()
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

impl Default for PseFileReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomPropColoring {
    prop_name: String,
    fallback_color: SolidColor,
}

impl Default for AtomPropColoring {
    fn default() -> Self {
        Self {
            prop_name: "col".to_string(),
            fallback_color: SolidColor::from_rgb(0.5, 0.5, 0.5),
        }
    }
}

impl AtomPropColoring {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "AtomPropColoring"
    }

    pub fn prop_name(&self) -> &str {
        &self.prop_name
    }

    pub fn set_prop_name(&mut self, value: impl Into<String>) {
        self.prop_name = value.into();
    }

    pub fn fallback_color(&self) -> &SolidColor {
        &self.fallback_color
    }

    pub fn set_fallback_color(&mut self, value: SolidColor) {
        self.fallback_color = value;
    }

    pub fn color_for_atom(&self, atom: &Atom) -> SolidColor {
        atom.int_prop(&self.prop_name)
            .map(|code| SolidColor::from_code(code as u32))
            .unwrap_or_else(|| self.fallback_color.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::Vector4;
    use std::collections::BTreeMap;

    fn atom_with_props(props: BTreeMap<String, i32>) -> Atom {
        Atom {
            serial: 1,
            name: "C".to_string(),
            alt_loc: None,
            res_name: "RES".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(0.0, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: props,
        }
    }

    #[test]
    fn atom_prop_coloring_matches_reference_defaults_and_fallback() {
        let coloring = AtomPropColoring::new();
        assert_eq!(coloring.type_name(), "AtomPropColoring");
        assert_eq!(coloring.prop_name(), "col");
        assert_eq!(
            coloring.fallback_color().rgba8(),
            SolidColor::from_rgb(0.5, 0.5, 0.5).rgba8()
        );

        let empty = atom_with_props(BTreeMap::new());
        assert_eq!(
            coloring.color_for_atom(&empty).rgba8(),
            coloring.fallback_color().rgba8()
        );
    }

    #[test]
    fn atom_prop_coloring_reads_integer_color_code() {
        let mut props = BTreeMap::new();
        props.insert("col".to_string(), 0xff336699_u32 as i32);
        let atom = atom_with_props(props);

        let mut coloring = AtomPropColoring::new();
        assert_eq!(
            coloring.color_for_atom(&atom).rgba8(),
            SolidColor::from_code(0xff336699).rgba8()
        );

        coloring.set_prop_name("missing");
        assert_eq!(
            coloring.color_for_atom(&atom).rgba8(),
            coloring.fallback_color().rgba8()
        );
    }

    #[test]
    fn importers_reader_defaults_match_qif() {
        let mol2 = Mol2MolReader::new();
        assert_eq!(mol2.handler().name(), "mol2");
        assert_eq!(
            mol2.handler().type_descr(),
            "SYBYL Mol2 Coordinates (*.mol2)"
        );
        assert_eq!(mol2.handler().file_ext(), "*.mol2");
        assert_eq!(mol2.handler().category(), InOutHandler::IOH_CAT_OBJREADER);
        assert_eq!(mol2.load_compound_id(), -1);
        assert!(mol2.load_as_chain());
        assert_eq!(mol2.chain_name(), "_");
        assert_eq!(mol2.residue_index(), 1);
        assert_eq!(mol2.options().chain_name, "_");

        let mmcif = MmcifMolReader::new();
        assert_eq!(mmcif.handler().name(), "mmcif");
        assert!(!mmcif.load_model());
        assert!(mmcif.load_anisou());
        assert!(mmcif.load_alt_conf());
        assert!(!mmcif.load_secstr());
        assert!(mmcif.auto_topo_gen());

        let sdf = SdfMolReader::new();
        assert_eq!(sdf.handler().name(), "sdf");
        assert_eq!(sdf.load_compound_id(), -1);
        assert!(!sdf.load_as_chain());
        assert_eq!(sdf.chain_name(), "_");
        assert_eq!(sdf.residue_index(), 1);
        assert_eq!(sdf.options().chain_name, "_");
    }

    #[test]
    fn importers_writer_and_pse_reader_defaults_match_qif() {
        let writer = SdfMolWriter::new();
        assert_eq!(writer.handler().name(), "sdf");
        assert_eq!(writer.handler().category(), InOutHandler::IOH_CAT_OBJWRITER);
        assert_eq!(writer.selection(), &Selector::All);

        let mut pse = PseFileReader::new();
        assert_eq!(pse.handler().name(), "psefile");
        assert_eq!(pse.handler().type_descr(), "PyMOL Session (*.pse)");
        assert_eq!(pse.handler().file_ext(), "*.pse");
        assert_eq!(pse.handler().category(), InOutHandler::IOH_CAT_SCEREADER);
        pse.set_path("/tmp/test.pse");
        assert_eq!(pse.path(), "/tmp/test.pse");
        pse.attach_scene(Uid::new(42));
        assert_eq!(pse.client_scene(), Some(Uid::new(42)));
        assert_eq!(pse.detach_scene(), Some(Uid::new(42)));
        pse.read();
        assert_eq!(pse.read_count(), 1);
        assert!(pse.last_status().is_none());
        assert!(!pse.error_msg().is_empty());
    }

    #[test]
    fn pse_reader_inspects_real_file_status_and_errors() {
        let path =
            std::env::temp_dir().join(format!("cuemol2-rs-pse-{}-raw.pse", std::process::id()));
        std::fs::write(&path, [0x80_u8, 0x04, b'.']).unwrap();

        let mut pse = PseFileReader::new();
        pse.set_path(path.to_str().unwrap());
        let status = pse.read_checked().unwrap();
        assert_eq!(status.byte_len(), 3);
        assert_eq!(status.encoding(), PseEncoding::Raw);
        assert_eq!(status.pickle_protocol(), Some(4));
        assert_eq!(pse.last_status(), Some(&status));
        assert_eq!(pse.error_msg(), "");

        let missing = path.with_extension("missing");
        pse.set_path(missing.to_str().unwrap());
        assert!(pse.read_checked().is_err());
        assert!(pse.last_status().is_none());
        assert!(!pse.error_msg().is_empty());
        assert_eq!(pse.read_count(), 2);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pse_reader_parses_session_pickle_tree() {
        // A raw (uncompressed) CPython-pickled session header; the reader runs the
        // real pickle VM and exposes the value tree.
        let proto2: &[u8] = &[
            128, 2, 125, 113, 0, 40, 88, 7, 0, 0, 0, 118, 101, 114, 115, 105, 111, 110, 113, 1, 71,
            63, 251, 215, 10, 61, 112, 163, 215, 88, 5, 0, 0, 0, 110, 97, 109, 101, 115, 113, 2,
            93, 113, 3, 40, 88, 4, 0, 0, 0, 109, 111, 108, 49, 113, 4, 88, 4, 0, 0, 0, 109, 111,
            108, 50, 113, 5, 101, 88, 5, 0, 0, 0, 99, 111, 117, 110, 116, 113, 6, 75, 2, 88, 5, 0,
            0, 0, 102, 108, 97, 103, 115, 113, 7, 136, 137, 78, 135, 113, 8, 88, 4, 0, 0, 0, 109,
            101, 116, 97, 113, 9, 125, 113, 10, 40, 88, 5, 0, 0, 0, 116, 105, 116, 108, 101, 113,
            11, 88, 4, 0, 0, 0, 116, 101, 115, 116, 113, 12, 88, 3, 0, 0, 0, 110, 97, 116, 113, 13,
            77, 71, 1, 117, 117, 46,
        ];
        let path =
            std::env::temp_dir().join(format!("cuemol2-rs-pse-{}-tree.pse", std::process::id()));
        std::fs::write(&path, proto2).unwrap();

        let mut pse = PseFileReader::new();
        pse.set_path(path.to_str().unwrap());
        let tree = pse.parse_pickle_tree().expect("parse pickle tree");
        assert_eq!(tree.get("count").unwrap().as_i64(), Some(2));
        assert_eq!(
            tree.get("names"),
            Some(&PickleValue::List(vec![
                PickleValue::Str("mol1".into()),
                PickleValue::Str("mol2".into()),
            ]))
        );
        assert_eq!(
            tree.get("meta").unwrap().get("nat").unwrap().as_i64(),
            Some(327)
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pse_reader_detects_gzip_payload_protocol() {
        let raw_path =
            std::env::temp_dir().join(format!("cuemol2-rs-pse-{}-raw2.pse", std::process::id()));
        let gz_path = raw_path.with_extension("pse.gz");
        std::fs::write(&raw_path, [0x80_u8, 0x05, b'.']).unwrap();
        let output = std::process::Command::new("gzip")
            .arg("-c")
            .arg(&raw_path)
            .output();
        let Ok(output) = output else {
            let _ = std::fs::remove_file(raw_path);
            return;
        };
        if !output.status.success() {
            let _ = std::fs::remove_file(raw_path);
            return;
        }
        std::fs::write(&gz_path, output.stdout).unwrap();

        let mut pse = PseFileReader::new();
        pse.set_path(gz_path.to_str().unwrap());
        let status = pse.read_checked().unwrap();
        assert_eq!(status.encoding(), PseEncoding::Gzip);
        assert_eq!(status.pickle_protocol(), Some(5));

        let _ = std::fs::remove_file(raw_path);
        let _ = std::fs::remove_file(gz_path);
    }

    #[test]
    fn pse_reader_detects_bzip2_payload_protocol() {
        let raw_path =
            std::env::temp_dir().join(format!("cuemol2-rs-pse-{}-raw3.pse", std::process::id()));
        let bz2_path = raw_path.with_extension("pse.bz2");
        std::fs::write(&raw_path, [0x80_u8, 0x05, b'.']).unwrap();
        let output = std::process::Command::new("bzip2")
            .arg("-c")
            .arg(&raw_path)
            .output();
        let Ok(output) = output else {
            let _ = std::fs::remove_file(raw_path);
            return;
        };
        if !output.status.success() {
            let _ = std::fs::remove_file(raw_path);
            return;
        }
        std::fs::write(&bz2_path, output.stdout).unwrap();

        let mut pse = PseFileReader::new();
        pse.set_path(bz2_path.to_str().unwrap());
        let status = pse.read_checked().unwrap();
        assert_eq!(status.encoding(), PseEncoding::Bzip2);
        assert_eq!(status.pickle_protocol(), Some(5));

        let _ = std::fs::remove_file(raw_path);
        let _ = std::fs::remove_file(bz2_path);
    }
}
