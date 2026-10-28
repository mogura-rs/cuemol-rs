use cuemol_core::{CueMolError, Matrix4, Result};
use cuemol_molstr::{MolCoord, Selector};

#[derive(Debug, Clone)]
pub struct UsAlignResult {
    pub matrix: Matrix4,
    pub tm_reference: f64,
    pub tm_moving: f64,
    pub rmsd: f64,
    pub aligned_residues: usize,
}

pub fn superpose_usalign(
    reference: &MolCoord,
    reference_selection: &Selector,
    moving: &MolCoord,
    moving_selection: &Selector,
    pivot: &str,
) -> Result<UsAlignResult> {
    let pivot = if pivot.is_empty() || pivot == "auto" {
        if moving
            .atoms()
            .iter()
            .any(|a| a.name == "CA" && moving_selection.matches_in_molecule(a, moving))
        {
            "CA"
        } else {
            "C3'"
        }
    } else {
        pivot
    };
    let extract = |mol: &MolCoord, sel: &Selector| -> Result<(Vec<f64>, Vec<u8>)> {
        let mut xyz = Vec::new();
        let mut names = Vec::new();
        for atom in mol
            .atoms()
            .iter()
            .filter(|a| a.name == pivot && sel.matches_in_molecule(a, mol))
        {
            if ![atom.position.x, atom.position.y, atom.position.z]
                .iter()
                .all(|v| v.is_finite())
            {
                return Err(CueMolError::invalid_argument("non-finite atom position"));
            }
            xyz.extend([atom.position.x, atom.position.y, atom.position.z]);
            let mut name = [b' '; 3];
            let bytes = atom.res_name.as_bytes();
            let count = bytes.len().min(3);
            name[3 - count..].copy_from_slice(&bytes[..count]);
            names.extend(name);
        }
        if xyz.len() < 9 || xyz.len() / 3 > i32::MAX as usize {
            return Err(CueMolError::invalid_argument(
                "US-align needs at least three selected pivot atoms",
            ));
        }
        Ok((xyz, names))
    };
    let (rx, rn) = extract(reference, reference_selection)?;
    let (mx, mn) = extract(moving, moving_selection)?;
    let (matrix, scores) = align_arrays(&mx, &mn, &rx, &rn, pivot)?;
    let mut transform = Matrix4::IDENTITY;
    for row in 0..3 {
        for col in 0..4 {
            transform.set_at(row + 1, col + 1, matrix[row * 4 + col])?;
        }
    }
    Ok(UsAlignResult {
        matrix: transform,
        tm_reference: scores[0],
        tm_moving: scores[1],
        rmsd: scores[2],
        aligned_residues: scores[3] as usize,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn align_arrays(
    mx: &[f64],
    mn: &[u8],
    rx: &[f64],
    rn: &[u8],
    pivot: &str,
) -> Result<([f64; 12], [f64; 4])> {
    let pivot = std::ffi::CString::new(pivot)
        .map_err(|_| CueMolError::invalid_argument("invalid pivot atom"))?;
    let mut matrix = [0.; 12];
    let mut scores = [0.; 4];
    let mut error = [0i8; 512];
    // The buffers have one residue name and three coordinates per atom; outputs are fixed size.
    let status = unsafe {
        cuemol_usalign(
            mx.as_ptr(),
            mn.as_ptr().cast(),
            (mx.len() / 3) as u32,
            rx.as_ptr(),
            rn.as_ptr().cast(),
            (rx.len() / 3) as u32,
            pivot.as_ptr(),
            matrix.as_mut_ptr(),
            scores.as_mut_ptr(),
            error.as_mut_ptr(),
            error.len() as u32,
        )
    };
    if status != 0 {
        let message = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }.to_string_lossy();
        return Err(CueMolError::invalid_argument(format!(
            "US-align failed: {message}"
        )));
    }
    Ok((matrix, scores))
}

#[cfg(target_arch = "wasm32")]
fn align_arrays(
    mx: &[f64],
    mn: &[u8],
    rx: &[f64],
    rn: &[u8],
    pivot: &str,
) -> Result<([f64; 12], [f64; 4])> {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = cuemolKernels, js_name = usalign, catch)]
        fn align(
            moving: &[f64],
            moving_names: &[u8],
            reference: &[f64],
            reference_names: &[u8],
            pivot: &str,
        ) -> std::result::Result<Vec<f64>, JsValue>;
    }
    if pivot.contains('\0') {
        return Err(CueMolError::invalid_argument("invalid pivot atom"));
    }
    let values = align(mx, mn, rx, rn, pivot)
        .map_err(|error| CueMolError::invalid_argument(format!("US-align failed: {error:?}")))?;
    if values.len() != 16 || values.iter().any(|v| !v.is_finite()) {
        return Err(CueMolError::invalid_argument("invalid US-align result"));
    }
    Ok((
        values[..12].try_into().unwrap(),
        values[12..].try_into().unwrap(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
unsafe extern "C" {
    fn cuemol_usalign(
        moving: *const f64,
        moving_names: *const std::ffi::c_char,
        moving_len: u32,
        reference: *const f64,
        reference_names: *const std::ffi::c_char,
        reference_len: u32,
        pivot: *const std::ffi::c_char,
        matrix: *mut f64,
        scores: *mut f64,
        error: *mut std::ffi::c_char,
        error_len: u32,
    ) -> i32;
}
