pub mod atom_pos_map;
pub mod chem;
pub mod cns_par;
pub mod evaluator;
mod fileio;
pub mod iterators;
pub mod map;
pub mod mmcif;
pub mod mol2;
pub mod mol_undo;
pub mod molecule;
pub mod pdb;
pub mod prot2ndry;
pub mod qdf_mol;
pub mod qsys;
pub mod renderers;
pub mod sdf;
pub mod selection;
pub mod stream;
pub mod toppar;

/// C++ `molstr::param` compatibility namespace.
pub mod param {
    pub use crate::cns_par::{AnglTag, AnglVal, BondTag, BondVal, DiheTag, DiheVal, NonbVal};
    pub type AtomVal = crate::toppar::AtomParam;
}

pub use atom_pos_map::{AtomPosMap, AtomPosMap2};
pub use chem::{ElemSym, MolAtom, MolBond, MolChain, MolResidue, ResidIndex, INVALID_UID};
pub use cns_par::{
    builtin_param_db, read_cns_par_file, read_cns_par_str, read_cns_toppar_file,
    read_cns_toppar_str, AnglParam, BondParam, CnsParFile, DiheParam, NonbParam,
    ParamDB as CnsParamDB,
};
pub use evaluator::{IntNumEvaluator, RealNumEvaluator};
pub use fileio::{read_bytes_maybe_compressed, read_text_maybe_compressed};
pub use iterators::{
    AtomIterator, BondIterator, ChainIterator, ChainView, ResidIterator, ResidRangeSet, ResidueView,
};
pub use map::{
    read_brix_map_bytes, read_brix_map_file, read_brix_map_with_header_bytes,
    read_brix_map_with_header_file, read_ccp4_map_bytes, read_ccp4_map_file,
    read_ccp4_map_with_header_bytes, read_ccp4_map_with_header_file, read_mmcif_map_file,
    read_opendx_file, read_opendx_str, read_xplor_map_file, read_xplor_map_str,
    read_xplor_map_with_header_file, read_xplor_map_with_header_str, BrixHeader, BrixMap,
    Ccp4Header, Ccp4Map, GridMap, MapStats, MmcifMapStatus, XplorHeader, XplorMap,
};
pub use mmcif::{
    read_mmcif_file, read_mmcif_file_with_options, read_mmcif_str, read_mmcif_str_with_options,
    MmcifReadOptions, MmcifReader,
};
pub use mol2::{
    read_mol2_file, read_mol2_file_with_options, read_mol2_str, read_mol2_str_with_options,
    Mol2ReadOptions, Mol2Reader,
};
pub use mol_undo::{MolEditInfo, MolUndoManager, MolXformEditInfo, ResiduePropsEdit};
pub use molecule::{Atom, Bond, BondOrder, ChainSummary, MolCoord, ResidueKey, ResidueSummary};
pub use pdb::{
    read_pdb_file, read_pdb_file_with_options, read_pdb_str, read_pdb_str_with_options,
    read_qdfpdb_file, read_qdfpdb_str, write_pdb_file, write_pdb_string,
    write_pdb_string_with_selection, PdbReadOptions, PdbReader, PdbWriter, QdfPdbReader,
};
pub use prot2ndry::{calc_prot_2ndry, calc_prot_2ndry2, Prot2ndrySet};
pub use qdf_mol::{read_qdf_mol_bytes, write_qdf_mol_bytes, QdfMolFileReader, QdfMolFileWriter};
pub use qsys::{
    atom_intr_defs_json, AtomIntrKind, AtomIntrLine, LoadObjectResult, LoadSceneResult,
    LoadedDensityMap, LoadedLWObject, LoadedLWRenderer, LoadedQscObject, LoadedStyle,
    LoadedSurface, MolCommandManager, MolObject, MolRenderer, MolScene, MolSceneManager, MolView,
    NewRendererResult, NewSceneResult, QscLoadStatus,
};
pub use renderers::{
    BfacColoring, CpkColoring, MolAtomRenderer, NameLabel, NameLabelRenderer, RectSelDrawObj,
    SelectionRenderMode, SelectionRenderer, SimpleRenderer, SolidColoring, TraceRenderer,
};
pub use sdf::{
    read_sdf_file, read_sdf_file_with_options, read_sdf_str, read_sdf_str_with_options,
    write_sdf_file, write_sdf_string, write_sdf_string_with_selection, SdfReadOptions, SdfReader,
    SdfWriter,
};
pub use selection::{
    parse_named_selector, parse_selector, SelCacheData, SelCacheMgr, SelCommand, SelCompiler,
    Selector,
};
pub use stream::{
    AsyncLoadedObject, CompressionMode, ContentVerdict, HandlerCategory, HandlerSniffer,
    StreamHandlerInfo, StreamManager,
};
pub use toppar::{
    build_topology_bonds_with_db, build_topology_bonds_with_db_options, charge_for_atom_mode,
    read_xml_toppar_file, read_xml_toppar_str, AtomParam, LinkDef, LinkTarget, ParamDB, ResiPatch,
    ResiToppar, TopAtom, TopBond, TopoDB, TopparManager, XmlTopparFile, XmlTopparReadStats,
};

pub type ColoringScheme = SolidColoring;
pub type CPKColoring = CpkColoring;
pub type MainChainRenderer = TraceRenderer;
pub type MolSelection = SelCommand;
pub type PDBFileReader = PdbReader;
pub type PDBFileWriter = PdbWriter;
// C++ dual names: the real QDF molecule reader/writer (was a fake alias to
// PdbReader, which cannot parse the QDF binary format).
pub type QdfMolReader = QdfMolFileReader;
pub type QdfMolWriter = QdfMolFileWriter;
