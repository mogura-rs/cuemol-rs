use cuemol_gfx::CmsXform;
use js_sys::{Array, Float64Array, Uint32Array};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn surface(xyzr: &[f64], density: f64, probe: f64, backend: u32) -> Result<Array, JsError> {
    let (vertices, triangles, used, owners) =
        cuemol_surface::backends::kernel(xyzr, density, probe, backend)
            .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Array::of4(
        &Float64Array::from(vertices.as_slice()),
        &Uint32Array::from(triangles.as_slice()),
        &used.into(),
        &Uint32Array::from(owners.as_slice()),
    ))
}

#[wasm_bindgen]
pub fn usalign(
    moving: &[f64],
    names: &[u8],
    reference: &[f64],
    reference_names: &[u8],
    pivot: &str,
) -> Result<Vec<f64>, JsError> {
    let (matrix, scores) =
        cuemol_molanl::align_arrays(moving, names, reference, reference_names, pivot)
            .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(matrix.into_iter().chain(scores).collect())
}

#[wasm_bindgen]
pub struct ColorProof(CmsXform);

#[wasm_bindgen]
impl ColorProof {
    #[wasm_bindgen(constructor)]
    pub fn new(profile: &[u8], intent: i32) -> Result<ColorProof, JsError> {
        let mut transform = CmsXform::new();
        transform.set_icc_intent(intent);
        transform
            .load_icc_bytes(profile)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self(transform))
    }

    pub fn info(&self) -> String {
        self.0.info().to_owned()
    }

    pub fn color(&self, rgb: u32, gamut: bool) -> u32 {
        if gamut {
            u32::from(self.0.is_in_gamut(rgb))
        } else {
            self.0.do_xform(rgb) & 0xffffff
        }
    }
}
