use cuemol_gfx::parse_color;
use cuemol_molstr::{parse_selector, read_pdb_file, HandlerCategory, StreamManager};

pub fn compile_selection_dump(expression: &str) -> Result<String, String> {
    parse_selector(expression)
        .map(|selector| selector.dump())
        .map_err(|err| err.to_string())
}

pub fn color_syntax_is_valid(expression: &str) -> bool {
    parse_color(expression).is_ok()
}

pub fn pdb_array_metadata(path: &str, array_name: &str) -> Result<String, String> {
    let mol = read_pdb_file(path).map_err(|err| err.to_string())?;
    let array = match array_name {
        "crd" => mol.crd_array(),
        "bfac" | "occ" => mol.prop_array(array_name),
        other => return Err(format!("unknown molecule array: {other}")),
    };
    Ok(format!(
        "{{\"type\":{},\"length\":{},\"bytes\":{}}}",
        array.elem_type_code(),
        array.length(),
        array.byte_len()
    ))
}

pub fn pdb_summary_json(path: &str) -> Result<String, String> {
    let mol = read_pdb_file(path).map_err(|err| err.to_string())?;
    Ok(format!(
        "{{\"atoms\":{},\"residues\":{},\"chains\":{},\"chainNames\":{}}}",
        mol.atom_count(),
        mol.residue_count(),
        mol.chain_ids().len(),
        mol.chains_json()
    ))
}

pub fn pdb_selection_count(path: &str, expression: &str) -> Result<usize, String> {
    let mol = read_pdb_file(path).map_err(|err| err.to_string())?;
    let selector = parse_selector(expression).map_err(|err| err.to_string())?;
    Ok(mol.atom_sel_count(&selector))
}

pub fn search_reader_by_content(
    path: &str,
    nicknames: &str,
    category: i32,
    support_compression: bool,
    max_bytes: u64,
) -> Result<String, String> {
    let category = HandlerCategory::from_code(category).map_err(|err| err.to_string())?;
    Ok(StreamManager::new().search_reader_by_content(
        path,
        nicknames,
        category,
        support_compression,
        max_bytes,
    ))
}

pub fn search_readers_by_content(
    path: &str,
    nicknames: &str,
    category: i32,
    support_compression: bool,
    max_bytes: u64,
) -> Result<String, String> {
    let category = HandlerCategory::from_code(category).map_err(|err| err.to_string())?;
    Ok(StreamManager::new().search_readers_by_content(
        path,
        nicknames,
        category,
        support_compression,
        max_bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_binding_ready_core_functions() {
        assert_eq!(compile_selection_dump("chain A,B").unwrap(), "chain B,A");
        assert!(color_syntax_is_valid("rgb(255, 0, 0)"));
    }

    #[test]
    fn exposes_pdb_array_metadata_for_bindings() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let path = root.join("1CRN.pdb");
        assert_eq!(
            pdb_array_metadata(path.to_str().unwrap(), "bfac").unwrap(),
            "{\"type\":21,\"length\":327,\"bytes\":1308}"
        );
    }

    #[test]
    fn exposes_pdb_summary_selection_and_stream_search_for_bindings() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let path = root.join("1CRN.pdb");
        assert_eq!(
            pdb_summary_json(path.to_str().unwrap()).unwrap(),
            "{\"atoms\":327,\"residues\":46,\"chains\":1,\"chainNames\":[\"A\"]}"
        );
        assert_eq!(
            pdb_selection_count(path.to_str().unwrap(), "chain A and name CA").unwrap(),
            46
        );
        assert_eq!(
            search_reader_by_content(
                path.to_str().unwrap(),
                "",
                HandlerCategory::OBJ_READER,
                false,
                0
            )
            .unwrap(),
            "pdb"
        );
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
}
