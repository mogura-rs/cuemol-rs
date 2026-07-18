//! QDF molecule reader/writer — CueMol's native `*.qdf` container for a
//! molecular structure. Ports C++ `molstr::QdfMolReader` / `QdfMolWriter`
//! (file type `"MOL2"`).
//!
//! The stream is hierarchical — chains contain residues contain atoms, linked by
//! `u32` UIDs — whereas the Rust [`MolCoord`] is a flat atom list. The writer
//! reconstructs the chain/residue hierarchy from the flat atoms (grouping by
//! chain id, then by residue index + insertion code) and the reader flattens it
//! back. Data blocks:
//! - `mol`: crystallographic `CrystalInfo` ext-data (cell and space group).
//! - `chai`: `id`(uid), `name`(fixstr).
//! - `resi`: `id`(uid), `pid`(parent chain uid), `name`(fixstr), `idx`(i32), and
//!   `ins`(i8 insertion code) only when some residue has one.
//! - `atom`: `id`, `pid`(parent residue uid), `name`/`cnam`(fixstr), `conf`(i8
//!   alt-loc), `elem`(u8 atomic number), `posx/posy/posz`, `bfac`, `occ`(f32).
//! - `bond`: `id`, `aid1`, `aid2`(atom uids), `type`(u8 valence), `udef`(u8).
//!
//! Per-residue string properties, per-atom integer properties, and anisotropic U
//! are preserved through the C++ `PROP_PFX` and `u00..u22` columns. Atoms are
//! emitted in flat order so an atom's uid equals its index, which keeps the bond
//! references valid after the flatten round-trip. The element string is reduced
//! to an atomic number, so its case is normalised on round-trip (`FE` -> `Fe`);
//! only the atomic number is preserved, matching C++ `ElemID`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cuemol_core::{CellParams, CueMolError, Result, Vector4};
use cuemol_qsys::{
    InOutHandler, ObjReader, ObjWriter, QdfFieldDef, QdfReader, QdfWriter, QDF_TYPE_BOOL,
    QDF_TYPE_FIXSTR16, QDF_TYPE_FIXSTR32, QDF_TYPE_FIXSTR8, QDF_TYPE_FLOAT32, QDF_TYPE_INT32,
    QDF_TYPE_INT8, QDF_TYPE_UID, QDF_TYPE_UINT8,
};

use crate::molecule::{Atom, BondOrder, MolCoord, ResidueKey};

/// Prefix marking a dynamic (user/computed) property column, e.g. the DSSP
/// `secondary` residue prop is stored as `$secondary` (C++ `PROP_PFX`).
const PROP_PFX: char = '$';

/// QdfMol anisotropic-U columns (C++ `QdfMolWriter`/`QdfMolReader`, in matrix
/// `getU(i,j)` order) paired with the Rust ANISOU int-prop names the PDB reader
/// fills (PDB U-naming, stored x10^4): `u00`=U(0,0)=U11, `u01`=U(0,1)=U12,
/// `u02`=U(0,2)=U13, `u11`=U(1,1)=U22, `u12`=U(1,2)=U23, `u22`=U(2,2)=U33. The
/// mapping is anchored to the C++ source, not to the round-trip, so a swapped
/// component cannot pass self-consistently.
const ANISO_COLUMNS: [(&str, &str); 6] = [
    ("u00", "anisou_u11"),
    ("u01", "anisou_u12"),
    ("u02", "anisou_u13"),
    ("u11", "anisou_u22"),
    ("u12", "anisou_u23"),
    ("u22", "anisou_u33"),
];

/// Build the residue-index string (`"100"` or `"100A"`) used by
/// `MolCoord::set_residue_prop_str`.
fn residue_sindex(res_id: i32, ins_code: Option<char>) -> String {
    match ins_code {
        Some(c) => format!("{res_id}{c}"),
        None => res_id.to_string(),
    }
}

/// Element symbols indexed by atomic number (index 0 is the unknown/dummy slot).
const ELEMENTS: [&str; 119] = [
    "Xx", "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S",
    "Cl", "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge",
    "As", "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd",
    "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd",
    "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg",
    "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm",
    "Bk", "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn",
    "Nh", "Fl", "Mc", "Lv", "Ts", "Og",
];

/// Map an element symbol to its atomic number (case-insensitive; unknown -> 0).
fn element_to_number(symbol: &str) -> u8 {
    let sym = symbol.trim();
    ELEMENTS
        .iter()
        .position(|e| e.eq_ignore_ascii_case(sym))
        .unwrap_or(0) as u8
}

/// Map an atomic number back to its canonical element symbol.
fn number_to_element(number: u8) -> &'static str {
    ELEMENTS.get(number as usize).copied().unwrap_or("Xx")
}

fn conf_to_alt_loc(conf: i8) -> Option<char> {
    if conf == 0 {
        None
    } else {
        Some(conf as u8 as char)
    }
}

fn alt_loc_to_conf(alt_loc: Option<char>) -> i8 {
    alt_loc.map(|c| c as i32 as i8).unwrap_or(0)
}

/// Identifies a residue within the flat atom list.
#[derive(Clone, PartialEq, Eq)]
struct ResidKey {
    chain: String,
    res_id: i32,
    ins_code: Option<char>,
}

/// Serialize a `MolCoord` to a QDF `MOL2` byte stream.
pub fn write_qdf_mol_bytes(mol: &MolCoord) -> Vec<u8> {
    // Reconstruct the chain/residue hierarchy from the flat atoms, preserving
    // first-seen order and assigning sequential UIDs.
    let mut chain_uid: Vec<String> = Vec::new();
    let mut resid_keys: Vec<ResidKey> = Vec::new();
    let mut resid_names: Vec<String> = Vec::new();
    let mut resid_chain: Vec<u32> = Vec::new();
    let mut atom_resid: Vec<u32> = Vec::with_capacity(mol.atom_count());

    for atom in mol.atoms() {
        let chain_uid_idx = match chain_uid.iter().position(|c| c == &atom.chain_id) {
            Some(i) => i as u32,
            None => {
                chain_uid.push(atom.chain_id.clone());
                (chain_uid.len() - 1) as u32
            }
        };
        let key = ResidKey {
            chain: atom.chain_id.clone(),
            res_id: atom.res_id,
            ins_code: atom.ins_code,
        };
        let resid_idx = match resid_keys.iter().position(|k| k == &key) {
            Some(i) => i as u32,
            None => {
                resid_keys.push(key);
                resid_names.push(atom.res_name.clone());
                resid_chain.push(chain_uid_idx);
                (resid_keys.len() - 1) as u32
            }
        };
        atom_resid.push(resid_idx);
    }

    let has_ins = resid_keys.iter().any(|k| k.ins_code.is_some());

    // Fixed-string column widths (max byte length, at least 1).
    let chain_name_len = chain_uid.iter().map(|c| c.len()).max().unwrap_or(0).max(1);
    let resn_len = resid_names
        .iter()
        .map(|n| n.len())
        .max()
        .unwrap_or(0)
        .max(1);
    let aname_len = mol
        .atoms()
        .iter()
        .map(|a| a.name.len())
        .max()
        .unwrap_or(0)
        .max(1);

    let mut w = QdfWriter::new("MOL2");

    write_mol_ext_data(&mut w, mol.cell_params());

    // chai block
    w.data_def("chai", chain_uid.len());
    w.record_def_ext(&[
        ("id", QDF_TYPE_UID, 0),
        ("name", QDF_TYPE_FIXSTR8, chain_name_len),
    ]);
    for (uid, name) in chain_uid.iter().enumerate() {
        w.u32(uid as u32);
        w.fixed_str(name, chain_name_len);
    }

    // Collect per-residue string props (e.g. DSSP `secondary`) as `$`-prefixed
    // FIXSTR columns, sorted for a stable schema (mirrors C++ PROP_PFX).
    let resid_props: Vec<Option<&BTreeMap<String, String>>> = resid_keys
        .iter()
        .map(|k| {
            mol.residue_props()
                .get(&ResidueKey::new(&k.chain, k.res_id, k.ins_code))
        })
        .collect();
    let prop_names: Vec<String> = {
        let mut set: BTreeSet<String> = BTreeSet::new();
        for props in resid_props.iter().flatten() {
            set.extend(props.keys().cloned());
        }
        set.into_iter().collect()
    };
    let prop_lens: Vec<usize> = prop_names
        .iter()
        .map(|name| {
            resid_props
                .iter()
                .flatten()
                .filter_map(|p| p.get(name))
                .map(|v| v.len())
                .max()
                .unwrap_or(0)
                .max(1)
        })
        .collect();
    let prop_cols: Vec<String> = prop_names
        .iter()
        .map(|n| format!("{PROP_PFX}{n}"))
        .collect();

    // resi block
    w.data_def("resi", resid_keys.len());
    let mut resi_schema = vec![
        ("id", QDF_TYPE_UID, 0),
        ("pid", QDF_TYPE_UID, 0),
        ("name", QDF_TYPE_FIXSTR8, resn_len),
        ("idx", QDF_TYPE_INT32, 0),
    ];
    if has_ins {
        resi_schema.push(("ins", QDF_TYPE_INT8, 0));
    }
    for (col, &len) in prop_cols.iter().zip(prop_lens.iter()) {
        resi_schema.push((col.as_str(), QDF_TYPE_FIXSTR8, len));
    }
    w.record_def_ext(&resi_schema);
    for (uid, key) in resid_keys.iter().enumerate() {
        w.u32(uid as u32);
        w.u32(resid_chain[uid]);
        w.fixed_str(&resid_names[uid], resn_len);
        w.i32(key.res_id);
        if has_ins {
            w.i8(key.ins_code.map(|c| c as i32 as i8).unwrap_or(0));
        }
        for (name, &len) in prop_names.iter().zip(prop_lens.iter()) {
            let val = resid_props[uid]
                .and_then(|p| p.get(name))
                .map(String::as_str)
                .unwrap_or("");
            w.fixed_str(val, len);
        }
    }

    // atom block (flat order -> uid == index). ANISOU columns are written only
    // when at least one atom carries them (C++ `QdfMolWriter`'s `bHasAnisou`).
    let has_anisou = mol
        .atoms()
        .iter()
        .any(|atom| atom.int_prop("anisou_u11").is_some());
    let anisou_props: BTreeSet<_> = ANISO_COLUMNS.iter().map(|(_, name)| *name).collect();
    let mut atom_prop_types = BTreeMap::new();
    for atom in mol.atoms() {
        for name in atom
            .int_props
            .keys()
            .filter(|name| !anisou_props.contains(name.as_str()))
        {
            atom_prop_types.entry(name.clone()).or_insert(false);
        }
        for name in atom.real_props.keys() {
            atom_prop_types.insert(name.clone(), true);
        }
    }
    let atom_prop_names = atom_prop_types.keys().cloned().collect::<Vec<_>>();
    let atom_prop_columns: Vec<_> = atom_prop_names
        .iter()
        .map(|name| format!("{PROP_PFX}{name}"))
        .collect();
    let mut atom_schema: Vec<(&str, i8, usize)> = vec![
        ("id", QDF_TYPE_UID, 0),
        ("pid", QDF_TYPE_UID, 0),
        ("name", QDF_TYPE_FIXSTR8, aname_len),
        ("cnam", QDF_TYPE_FIXSTR8, aname_len),
        ("conf", QDF_TYPE_INT8, 0),
        ("elem", QDF_TYPE_UINT8, 0),
        ("posx", QDF_TYPE_FLOAT32, 0),
        ("posy", QDF_TYPE_FLOAT32, 0),
        ("posz", QDF_TYPE_FLOAT32, 0),
        ("bfac", QDF_TYPE_FLOAT32, 0),
        ("occ", QDF_TYPE_FLOAT32, 0),
    ];
    if has_anisou {
        for (col, _) in ANISO_COLUMNS {
            atom_schema.push((col, QDF_TYPE_FLOAT32, 0));
        }
    }
    for (name, column) in atom_prop_names.iter().zip(&atom_prop_columns) {
        atom_schema.push((
            column,
            if atom_prop_types[name] {
                QDF_TYPE_FLOAT32
            } else {
                QDF_TYPE_INT32
            },
            0,
        ));
    }
    w.data_def("atom", mol.atom_count());
    w.record_def_ext(&atom_schema);
    for (i, atom) in mol.atoms().iter().enumerate() {
        w.u32(i as u32);
        w.u32(atom_resid[i]);
        w.fixed_str(&atom.name, aname_len);
        w.fixed_str(&atom.name, aname_len); // cnam: flat model has no separate canonical name
        w.i8(alt_loc_to_conf(atom.alt_loc));
        w.u8(element_to_number(&atom.element));
        w.f32(atom.position.x as f32);
        w.f32(atom.position.y as f32);
        w.f32(atom.position.z as f32);
        w.f32(atom.b_factor);
        w.f32(atom.occupancy);
        // ANISOU: the PDB reader stores U components x10^4 as int props; C++
        // QdfMol stores the raw f32 in Angstrom^2 (getU). Write the f32, 0.0 for
        // an atom that lacks aniso (matching C++'s per-atom 0.0 fallback).
        if has_anisou {
            for (_, prop) in ANISO_COLUMNS {
                w.f32(atom.int_prop(prop).map(|v| v as f32 / 1.0e4).unwrap_or(0.0));
            }
        }
        for name in &atom_prop_names {
            if atom_prop_types[name] {
                w.f32(atom.real_prop(name).unwrap_or(0.0) as f32);
            } else {
                w.i32(atom.int_prop(name).unwrap_or(0));
            }
        }
    }

    // Only user-defined/persistent bonds are serialized. Topology bonds are
    // regenerated after reading, matching QdfMolWriter/QdfMolReader.
    let bonds = mol
        .bonds()
        .iter()
        .filter(|bond| bond.persist)
        .collect::<Vec<_>>();
    w.data_def("bond", bonds.len());
    if !bonds.is_empty() {
        w.record_def_ext(&[
            ("id", QDF_TYPE_UID, 0),
            ("aid1", QDF_TYPE_UID, 0),
            ("aid2", QDF_TYPE_UID, 0),
            ("type", QDF_TYPE_UINT8, 0),
            ("udef", QDF_TYPE_UINT8, 0),
        ]);
        for (i, bond) in bonds.iter().enumerate() {
            w.u32(i as u32);
            w.u32(bond.atom1 as u32);
            w.u32(bond.atom2 as u32);
            w.u8(bond.order.sdf_code().clamp(0, 255) as u8);
            w.u8(u8::from(bond.persist));
        }
    }

    w.finish()
}

/// Parse a QDF `MOL2` byte stream into a flat `MolCoord`.
pub fn read_qdf_mol_bytes(input: &[u8]) -> Result<MolCoord> {
    let mut r = QdfReader::from_bytes(input)?;
    r.expect_file_type("MOL2")?;

    let nmol = r.read_data_def("mol")?;
    let cell_params = read_mol_ext_data(&mut r, nmol)?;

    // chai block -> chain uid -> name
    let nchain = r.read_data_def("chai")?;
    let chai_defs = r.read_record_def()?;
    let chain_name_len = QdfFieldDef::fixed_len_of(&chai_defs, "name")
        .ok_or_else(|| CueMolError::parse("QDF mol: chai block missing 'name'"))?;
    let mut chain_names: BTreeMap<u32, String> = BTreeMap::new();
    for _ in 0..nchain {
        let id = r.read_u32()?;
        let name = r.read_fixed_str(chain_name_len)?;
        chain_names.insert(id, name);
    }

    // resi block -> residue uid -> (chain name, res name, idx, ins)
    let nresid = r.read_data_def("resi")?;
    let resi_defs = r.read_record_def()?;
    let resn_len = QdfFieldDef::fixed_len_of(&resi_defs, "name")
        .ok_or_else(|| CueMolError::parse("QDF mol: resi block missing 'name'"))?;
    let has_ins = QdfFieldDef::is_defined(&resi_defs, "ins");
    // `$`-prefixed columns are dynamic residue string props (in schema order,
    // after the standard fields).
    let resi_prop_fields: Vec<(String, usize)> = resi_defs
        .iter()
        .filter(|d| d.name.starts_with(PROP_PFX))
        .map(|d| (d.name[1..].to_string(), d.fixed_len))
        .collect();
    struct ResInfo {
        chain: String,
        name: String,
        res_id: i32,
        ins_code: Option<char>,
        props: Vec<(String, String)>,
    }
    let mut residues: BTreeMap<u32, ResInfo> = BTreeMap::new();
    for _ in 0..nresid {
        let id = r.read_u32()?;
        let pid = r.read_u32()?;
        let name = r.read_fixed_str(resn_len)?;
        let res_id = r.read_i32()?;
        let ins = if has_ins { r.read_i8()? } else { 0 };
        let mut props = Vec::new();
        for (pname, plen) in &resi_prop_fields {
            let value = r.read_fixed_str(*plen)?;
            if !value.is_empty() {
                props.push((pname.clone(), value));
            }
        }
        let chain = chain_names.get(&pid).cloned().unwrap_or_default();
        residues.insert(
            id,
            ResInfo {
                chain,
                name,
                res_id,
                ins_code: conf_to_alt_loc(ins),
                props,
            },
        );
    }

    // atom block -> flat atoms (written in uid order, so read order == index)
    let natom = r.read_data_def("atom")?;
    let atom_defs = r.read_record_def()?;
    let aname_len = QdfFieldDef::fixed_len_of(&atom_defs, "name")
        .ok_or_else(|| CueMolError::parse("QDF mol: atom block missing 'name'"))?;
    let cnam_len = QdfFieldDef::fixed_len_of(&atom_defs, "cnam").unwrap_or(aname_len);
    // ANISOU columns are present only when the writer block had any (C++
    // `QdfMolReader` `in.isDefined("u00")`).
    let has_anisou = QdfFieldDef::is_defined(&atom_defs, "u00");
    let atom_prop_fields: Vec<_> = atom_defs
        .iter()
        .filter(|definition| definition.name.starts_with(PROP_PFX))
        .collect();

    let mut mol = MolCoord::new();
    if let Some(cell) = cell_params {
        mol.set_cell_params(cell);
    }
    // Atom UID -> flat index, so bonds stay correct even if ids are not the
    // sequential order this writer emits (cross-compat robustness).
    let mut atom_index: BTreeMap<u32, usize> = BTreeMap::new();
    for _ in 0..natom {
        let id = r.read_u32()?;
        let pid = r.read_u32()?;
        let name = r.read_fixed_str(aname_len)?;
        let _cnam = r.read_fixed_str(cnam_len)?;
        let conf = r.read_i8()?;
        let elem = r.read_u8()?;
        let posx = r.read_f32()? as f64;
        let posy = r.read_f32()? as f64;
        let posz = r.read_f32()? as f64;
        let bfac = r.read_f32()?;
        let occ = r.read_f32()?;
        // ANISOU: read the 6 f32 U components, restore the PDB int props (x10^4).
        // C++ `QdfMolReader` drops an all-zero set (the no-aniso fallback), so a
        // bare atom does not gain spurious props.
        let aniso = if has_anisou {
            let mut vals = [0.0f32; 6];
            for value in &mut vals {
                *value = r.read_f32()?;
            }
            vals.iter()
                .any(|v| v.abs() > 1.0e-6)
                .then(|| vals.map(|v| (v * 1.0e4).round() as i32))
        } else {
            None
        };

        let res = residues.get(&pid);
        let (chain, res_name, res_id, ins_code) = match res {
            Some(ri) => (ri.chain.clone(), ri.name.clone(), ri.res_id, ri.ins_code),
            None => (String::new(), String::new(), 0, None),
        };

        let mut atom = Atom {
            serial: 0,
            name,
            alt_loc: conf_to_alt_loc(conf),
            res_name: res_name.clone(),
            chain_id: chain.clone(),
            res_id,
            ins_code,
            position: Vector4::new3(posx, posy, posz),
            occupancy: occ,
            b_factor: bfac,
            element: number_to_element(elem).to_string(),
            formal_charge: None,
            int_props: BTreeMap::new(),
            real_props: BTreeMap::new(),
        };
        if let Some(vals) = aniso {
            for ((_, prop), value) in ANISO_COLUMNS.iter().zip(vals) {
                atom.set_int_prop(*prop, value);
            }
        }
        for definition in &atom_prop_fields {
            let name = &definition.name[1..];
            match definition.type_tag {
                QDF_TYPE_INT32 => atom.set_int_prop(name, r.read_i32()?),
                QDF_TYPE_BOOL => {
                    r.read_bool()?;
                }
                QDF_TYPE_FLOAT32 => {
                    atom.set_real_prop(name, f64::from(r.read_f32()?));
                }
                QDF_TYPE_FIXSTR8 | QDF_TYPE_FIXSTR16 | QDF_TYPE_FIXSTR32 => {
                    r.read_fixed_str(definition.fixed_len)?;
                }
                _ => {
                    return Err(CueMolError::parse(
                        "QDF mol: unsupported atom property type",
                    ))
                }
            }
        }
        mol.append_atom1(atom, chain, res_id, res_name);
        atom_index.insert(id, mol.atom_count() - 1);
    }

    // Re-apply per-residue string props (e.g. DSSP `secondary`) now that the
    // residues have atoms (set_residue_prop_str requires an existing residue).
    for ri in residues.values() {
        if ri.props.is_empty() {
            continue;
        }
        let sindex = residue_sindex(ri.res_id, ri.ins_code);
        for (name, value) in &ri.props {
            let _ = mol.set_residue_prop_str(&ri.chain, &sindex, name.clone(), value.clone());
        }
    }

    // bond block (schema present only when there are bonds)
    let nbond = r.read_data_def("bond")?;
    if nbond > 0 {
        let _bond_defs = r.read_record_def()?;
        for _ in 0..nbond {
            let _id = r.read_u32()?;
            let aid1 = r.read_u32()?;
            let aid2 = r.read_u32()?;
            let btype = r.read_u8()?;
            let user_defined = r.read_u8()? == 1;
            if user_defined {
                let (Some(&i1), Some(&i2)) = (atom_index.get(&aid1), atom_index.get(&aid2)) else {
                    continue;
                };
                mol.add_bond(i1, i2, BondOrder::from_sdf(btype as i32));
                mol.mark_bond_persistent(i1, i2);
            }
        }
    }

    mol.apply_topology();
    Ok(mol)
}

fn write_mol_ext_data(writer: &mut QdfWriter, cell: Option<&CellParams>) {
    let Some(cell) = cell else {
        writer.data_def("mol", 0);
        return;
    };
    let mut values = vec![
        ("CrystalInfo.lena", cell.a.to_string()),
        ("CrystalInfo.lenb", cell.b.to_string()),
        ("CrystalInfo.lenc", cell.c.to_string()),
        ("CrystalInfo.anga", cell.alpha.to_string()),
        ("CrystalInfo.angb", cell.beta.to_string()),
        ("CrystalInfo.angg", cell.gamma.to_string()),
    ];
    if let Some(space_group) = cell.space_group_number {
        values.push(("CrystalInfo.sgid", space_group.to_string()));
    }
    if !cell.space_group_name.is_empty() {
        // Retain the original H-M name when no numeric space-group table is
        // available in this dependency layer. C++ safely ignores this field.
        values.push(("CrystalInfo.sgname", cell.space_group_name.clone()));
    }
    writer.data_def("mol", 1);
    let schema: Vec<_> = values
        .iter()
        .map(|(name, value)| (*name, QDF_TYPE_FIXSTR8, value.len().max(1)))
        .collect();
    writer.record_def_ext(&schema);
    for (_, value) in values {
        writer.fixed_str(&value, value.len().max(1));
    }
}

fn read_mol_ext_data(reader: &mut QdfReader, count: usize) -> Result<Option<CellParams>> {
    if count == 0 {
        return Ok(None);
    }
    if count > 1 {
        return Err(CueMolError::parse("QDF mol: invalid moldata size"));
    }
    let defs = reader.read_record_def()?;
    let mut values = BTreeMap::new();
    for definition in &defs {
        match definition.type_tag {
            QDF_TYPE_FIXSTR8 | QDF_TYPE_FIXSTR16 | QDF_TYPE_FIXSTR32 => {
                values.insert(
                    definition.name.clone(),
                    reader.read_fixed_str(definition.fixed_len)?,
                );
            }
            _ => return Err(CueMolError::parse("QDF mol: invalid moldata value type")),
        }
    }
    if !values.keys().any(|name| name.starts_with("CrystalInfo.")) {
        return Ok(None);
    }
    let number = |name: &str, default: f64| {
        values
            .get(name)
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(default)
    };
    let mut cell = CellParams::new(
        number("CrystalInfo.lena", 100.0),
        number("CrystalInfo.lenb", 100.0),
        number("CrystalInfo.lenc", 100.0),
        number("CrystalInfo.anga", 90.0),
        number("CrystalInfo.angb", 90.0),
        number("CrystalInfo.angg", 90.0),
    );
    cell.space_group_number = values
        .get("CrystalInfo.sgid")
        .and_then(|value| value.parse().ok());
    cell.space_group_name = values
        .get("CrystalInfo.sgname")
        .cloned()
        .unwrap_or_default();
    Ok(Some(cell))
}

/// `ObjReader` facade for QDF molecules (handler name `qdfmol`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfMolFileReader {
    reader: ObjReader,
    build2ndry: bool,
}

impl QdfMolFileReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("qdfmol", "CueMol molecule file (*.qdf)", "*.qdf"),
            build2ndry: false,
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn create_default_obj(&self) -> MolCoord {
        MolCoord::new()
    }

    pub fn build2ndry(&self) -> bool {
        self.build2ndry
    }

    pub fn set_build2ndry(&mut self, value: bool) {
        self.build2ndry = value;
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<MolCoord> {
        read_qdf_mol_bytes(input)
    }

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<MolCoord> {
        self.read_bytes(&std::fs::read(path)?)
    }
}

impl Default for QdfMolFileReader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for QdfMolFileReader {
    type Target = ObjReader;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl std::ops::DerefMut for QdfMolFileReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
    }
}

/// `ObjWriter` facade for QDF molecules (handler name `qdfmol`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfMolFileWriter {
    writer: ObjWriter,
}

impl QdfMolFileWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("qdfmol", "CueMol molecule file (*.qdf)", "*.qdf"),
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn can_handle(&self, _object: &MolCoord) -> bool {
        true
    }

    pub fn write_bytes(&self, object: &MolCoord) -> Vec<u8> {
        write_qdf_mol_bytes(object)
    }

    pub fn write_file(&self, object: &MolCoord, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, self.write_bytes(object))?;
        Ok(())
    }
}

impl Default for QdfMolFileWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for QdfMolFileWriter {
    type Target = ObjWriter;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl std::ops::DerefMut for QdfMolFileWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_qsys::qdf_file_type;

    fn atom(
        name: &str,
        chain: &str,
        res_id: i32,
        res_name: &str,
        elem: &str,
        pos: [f64; 3],
    ) -> Atom {
        Atom {
            serial: 0,
            name: name.to_string(),
            alt_loc: None,
            res_name: res_name.to_string(),
            chain_id: chain.to_string(),
            res_id,
            ins_code: None,
            position: Vector4::new3(pos[0], pos[1], pos[2]),
            occupancy: 1.0,
            b_factor: 20.0,
            element: elem.to_string(),
            formal_charge: None,
            int_props: BTreeMap::new(),
            real_props: BTreeMap::new(),
        }
    }

    fn sample_mol() -> MolCoord {
        let mut mol = MolCoord::new();
        // Chain A, residue 1 (ALA): N, CA, C; residue 2 (GLY): N, CA.
        mol.append_atom1(
            atom("N", "A", 1, "ALA", "N", [0.0, 0.0, 0.0]),
            "A",
            1,
            "ALA",
        );
        mol.append_atom1(
            atom("CA", "A", 1, "ALA", "C", [1.5, 0.0, 0.0]),
            "A",
            1,
            "ALA",
        );
        mol.append_atom1(
            atom("C", "A", 1, "ALA", "C", [2.0, 1.4, 0.0]),
            "A",
            1,
            "ALA",
        );
        mol.append_atom1(
            atom("N", "A", 2, "GLY", "N", [3.3, 1.5, 0.0]),
            "A",
            2,
            "GLY",
        );
        mol.append_atom1(
            atom("CA", "A", 2, "GLY", "C", [4.1, 2.7, 0.0]),
            "A",
            2,
            "GLY",
        );
        // Chain B, residue 10 (HOH): O (with a Fe to exercise 2-letter elements).
        mol.append_atom1(
            atom("O", "B", 10, "HOH", "O", [10.0, 0.0, 0.0]),
            "B",
            10,
            "HOH",
        );
        mol.append_atom1(
            atom("FE", "B", 11, "FE2", "Fe", [11.0, 0.0, 0.0]),
            "B",
            11,
            "FE2",
        );
        mol.add_bond(0, 1, BondOrder::Single);
        mol.add_bond(1, 2, BondOrder::Single);
        mol.add_bond(2, 3, BondOrder::Single); // peptide bond
        mol.add_bond(3, 4, BondOrder::Double);
        for (atom1, atom2) in [(0, 1), (1, 2), (2, 3), (3, 4)] {
            mol.mark_bond_persistent(atom1, atom2);
        }
        mol
    }

    #[test]
    fn qdf_mol_roundtrip_preserves_atoms_and_bonds() {
        let src = sample_mol();
        let bytes = write_qdf_mol_bytes(&src);
        assert_eq!(qdf_file_type(&bytes).as_deref(), Some("MOL2"));

        let dst = read_qdf_mol_bytes(&bytes).expect("read back QDF molecule");

        assert_eq!(dst.atom_count(), src.atom_count());
        assert_eq!(dst.bonds().len(), src.bonds().len());

        for (s, d) in src.atoms().iter().zip(dst.atoms().iter()) {
            assert_eq!(d.name, s.name);
            assert_eq!(d.chain_id, s.chain_id);
            assert_eq!(d.res_id, s.res_id);
            assert_eq!(d.res_name, s.res_name);
            // Element is normalised to canonical case via the atomic number.
            assert_eq!(element_to_number(&d.element), element_to_number(&s.element));
            assert!((d.position.x - s.position.x).abs() < 1.0e-4);
            assert!((d.position.y - s.position.y).abs() < 1.0e-4);
            assert!((d.position.z - s.position.z).abs() < 1.0e-4);
            assert!((d.b_factor - s.b_factor).abs() < 1.0e-4);
            assert!((d.occupancy - s.occupancy).abs() < 1.0e-4);
        }

        // Bonds preserve endpoints and order.
        for (s, d) in src.bonds().iter().zip(dst.bonds().iter()) {
            assert_eq!(d.atom1, s.atom1);
            assert_eq!(d.atom2, s.atom2);
            assert_eq!(d.order, s.order);
        }
        // The 2-letter element survived as iron (Z=26).
        assert_eq!(dst.atoms()[6].element, "Fe");
    }

    #[test]
    fn qdf_mol_regenerates_nonpersistent_topology_bonds() {
        let mut src = MolCoord::new();
        src.append_atom1(
            atom("N", "A", 1, "ALA", "N", [0.0, 0.0, 0.0]),
            "A",
            1,
            "ALA",
        );
        src.append_atom1(
            atom("CA", "A", 1, "ALA", "C", [1.5, 0.0, 0.0]),
            "A",
            1,
            "ALA",
        );
        src.apply_topology();
        assert_eq!(src.bond_count(), 1);
        assert!(!src.bonds()[0].persist);

        let dst = read_qdf_mol_bytes(&write_qdf_mol_bytes(&src)).unwrap();
        assert_eq!(dst.bond_count(), 1);
        assert!(!dst.bonds()[0].persist);
    }

    #[test]
    fn qdf_mol_roundtrip_preserves_crystal_info_ext_data() {
        let mut src = sample_mol();
        let mut cell = CellParams::new(40.1, 51.2, 62.3, 90.0, 101.5, 90.0);
        cell.space_group_name = "P 1 21 1".to_string();
        cell.space_group_number = Some(4);
        src.set_cell_params(cell.clone());

        let bytes = write_qdf_mol_bytes(&src);
        let dst = read_qdf_mol_bytes(&bytes).expect("read back QDF crystal info");
        assert_eq!(dst.cell_params(), Some(&cell));
    }

    #[test]
    fn qdf_mol_roundtrip_preserves_atom_integer_properties() {
        let mut src = sample_mol();
        src.atoms_mut()[0].set_int_prop("custom_id", 1234);
        src.atoms_mut()[1].set_int_prop("custom_id", -7);
        src.atoms_mut()[2].set_int_prop("secondary_code", 8);

        let bytes = write_qdf_mol_bytes(&src);
        let dst = read_qdf_mol_bytes(&bytes).expect("read back QDF atom properties");
        assert_eq!(dst.atoms()[0].int_prop("custom_id"), Some(1234));
        assert_eq!(dst.atoms()[1].int_prop("custom_id"), Some(-7));
        assert_eq!(dst.atoms()[2].int_prop("custom_id"), Some(0));
        assert_eq!(dst.atoms()[2].int_prop("secondary_code"), Some(8));
    }

    #[test]
    fn qdf_mol_roundtrip_preserves_atom_real_properties() {
        let mut src = sample_mol();
        src.atoms_mut()[0].set_real_prop("score", 1.25);
        src.atoms_mut()[1].set_real_prop("score", -0.5);
        src.atoms_mut()[2].set_int_prop("replace_me", 4);
        src.atoms_mut()[2].set_real_prop("replace_me", 3.75);
        assert_eq!(src.atoms()[2].int_prop("replace_me"), None);

        let dst = read_qdf_mol_bytes(&write_qdf_mol_bytes(&src))
            .expect("read back QDF real atom properties");
        assert_eq!(dst.atoms()[0].real_prop("score"), Some(1.25));
        assert_eq!(dst.atoms()[1].real_prop("score"), Some(-0.5));
        assert_eq!(dst.atoms()[2].real_prop("score"), Some(0.0));
        assert_eq!(dst.atoms()[2].real_prop("replace_me"), Some(3.75));
        assert_eq!(dst.atoms()[2].int_prop("replace_me"), None);
    }

    #[test]
    fn qdf_mol_roundtrip_preserves_anisou() {
        // C++ QdfMolWriter stores 6 ANISOU f32 columns (u00..u22) when any atom
        // has aniso (bHasAnisou); the Rust port had deferred them. The PDB reader
        // stores the U components as int props (x10^4) named `anisou_u11..u23`;
        // the QdfMol writer maps them to u00..u22 / 1e4 (f32) and the reader maps
        // back. ASYMMETRIC values catch a scaling or component-order bug; the C++
        // component correspondence itself is anchored in ANISO_COLUMNS, not here.
        let mut src = MolCoord::new();
        let mut a = atom("CA", "A", 1, "ALA", "C", [0.0, 0.0, 0.0]);
        // Distinct values so a swapped/mis-scaled component cannot pass.
        a.set_int_prop("anisou_u11", 1100);
        a.set_int_prop("anisou_u22", 2200);
        a.set_int_prop("anisou_u33", 3300);
        a.set_int_prop("anisou_u12", 1200);
        a.set_int_prop("anisou_u13", 1300);
        a.set_int_prop("anisou_u23", 2300);
        src.append_atom1(a, "A", 1, "ALA");
        // A second atom WITHOUT aniso: it must not gain spurious props (C++ skips
        // an all-zero set on read).
        src.append_atom1(
            atom("CB", "A", 1, "ALA", "C", [1.5, 0.0, 0.0]),
            "A",
            1,
            "ALA",
        );

        let bytes = write_qdf_mol_bytes(&src);
        let dst = read_qdf_mol_bytes(&bytes).expect("read back QDF molecule");

        let restored = &dst.atoms()[0];
        for (_, prop) in ANISO_COLUMNS {
            assert_eq!(
                restored.int_prop(prop),
                src.atoms()[0].int_prop(prop),
                "ANISOU {prop} must survive the round-trip"
            );
        }
        // The bare atom carries no aniso props.
        for (_, prop) in ANISO_COLUMNS {
            assert_eq!(
                dst.atoms()[1].int_prop(prop),
                None,
                "{prop} must stay unset"
            );
        }
    }

    #[test]
    fn qdf_mol_omits_anisou_columns_without_aniso() {
        // No atom has aniso -> the writer must NOT add the u00..u22 columns, and
        // a round-trip stays clean (the schema matches the pre-aniso format).
        let src = sample_mol();
        let bytes = write_qdf_mol_bytes(&src);
        let dst = read_qdf_mol_bytes(&bytes).expect("read back QDF molecule");
        assert_eq!(dst.atom_count(), src.atom_count());
        for atom in dst.atoms() {
            assert_eq!(atom.int_prop("anisou_u11"), None);
        }
    }

    #[test]
    fn qdf_mol_roundtrip_preserves_insertion_codes() {
        let mut src = MolCoord::new();
        let mut a = atom("CA", "A", 100, "SER", "C", [0.0, 0.0, 0.0]);
        a.ins_code = Some('A');
        src.append_atom1(a, "A", 100, "SER");
        let mut b = atom("CA", "A", 100, "GLY", "C", [1.0, 0.0, 0.0]);
        b.ins_code = Some('B');
        src.append_atom1(b, "A", 100, "GLY");

        let dst = read_qdf_mol_bytes(&write_qdf_mol_bytes(&src)).unwrap();
        assert_eq!(dst.atom_count(), 2);
        assert_eq!(dst.atoms()[0].ins_code, Some('A'));
        assert_eq!(dst.atoms()[1].ins_code, Some('B'));
        // Distinct insertion codes => two distinct residues round-tripped.
        assert_eq!(dst.atoms()[0].res_name, "SER");
        assert_eq!(dst.atoms()[1].res_name, "GLY");
    }

    #[test]
    fn qdf_mol_roundtrip_preserves_residue_props() {
        // Residue string props (e.g. DSSP `secondary` from Increment 1) are
        // carried as `$`-prefixed columns; without this the native save would
        // silently drop secondary structure.
        let mut src = sample_mol();
        src.set_residue_prop_str("A", "1", "secondary", "helix")
            .unwrap();
        src.set_residue_prop_str("A", "1", "secondary2", "Hs")
            .unwrap();
        src.set_residue_prop_str("A", "2", "secondary", "sheet")
            .unwrap();

        let dst = read_qdf_mol_bytes(&write_qdf_mol_bytes(&src)).unwrap();
        assert_eq!(
            dst.get_residue_prop_str("A", "1", "secondary").unwrap(),
            Some("helix")
        );
        assert_eq!(
            dst.get_residue_prop_str("A", "1", "secondary2").unwrap(),
            Some("Hs")
        );
        assert_eq!(
            dst.get_residue_prop_str("A", "2", "secondary").unwrap(),
            Some("sheet")
        );
        // A residue without the prop stays empty (not the padded NUL string).
        assert_eq!(
            dst.get_residue_prop_str("B", "10", "secondary").unwrap(),
            None
        );
    }

    #[test]
    fn read_rejects_non_mol_qdf_stream() {
        let bytes = QdfWriter::new("MAP1").finish();
        assert!(read_qdf_mol_bytes(&bytes).is_err());
    }

    #[test]
    fn element_table_roundtrips_common_symbols() {
        for sym in ["H", "C", "N", "O", "S", "P", "Fe", "Ca", "Zn", "Mg", "Cl"] {
            let n = element_to_number(sym);
            assert_ne!(n, 0, "unknown element {sym}");
            assert_eq!(number_to_element(n), sym);
        }
    }

    #[test]
    fn handler_metadata_matches_reference() {
        let mut reader = QdfMolFileReader::new();
        assert_eq!(reader.handler().name(), "qdfmol");
        assert_eq!(reader.handler().file_ext(), "*.qdf");
        assert!(!reader.build2ndry());
        reader.set_build2ndry(true);
        assert!(reader.build2ndry());
        let writer = QdfMolFileWriter::new();
        assert_eq!(writer.handler().name(), "qdfmol");
        assert!(writer.can_handle(&MolCoord::new()));
    }
}
