//! mmCIF map-coefficient reader: parse an `_refln` structure-factor loop
//! (`pdbx_FWT`/`pdbx_PHWT`, or the difference-map `pdbx_DELFWT`/`pdbx_DELPHWT`)
//! plus the unit cell and space group, then synthesize an electron-density map
//! via the shared inverse FFT. Port of C++ `xtal/MmcifMapReader`, which feeds
//! the same `MapFFT` the MTZ reader uses.

use cuemol_core::{parse_cif, CueMolError, Result};

use crate::fft::Reflection;
use crate::map_renderers::DensityMap;
use crate::mtz::density_map_from_reflections_with_cell;

/// Structure-factor coefficients and crystal parameters parsed from a map CIF.
#[derive(Debug, Clone)]
pub struct CifMapCoeffs {
    /// Unit cell `[a, b, c, alpha, beta, gamma]` (lengths in Angstrom, angles in degrees).
    pub cell: [f64; 6],
    /// Space group number (`_symmetry.Int_Tables_number`).
    pub space_group: i32,
    /// Reflections `(h, k, l, F, phase[deg])`.
    pub reflections: Vec<Reflection>,
}

/// Parse the `_cell`, `_symmetry` and `_refln` data of a map-coefficient CIF.
/// Mirrors C++ `MmcifMapReader::read{Cell,Symm,Refln}Line`: the amplitude column
/// is `pdbx_FWT` (falling back to `pdbx_DELFWT`), the phase column `pdbx_PHWT`
/// (falling back to `pdbx_DELPHWT`); the space group is the integer
/// `_symmetry.Int_Tables_number`.
pub fn parse_cif_map_coeffs(input: &str) -> Result<CifMapCoeffs> {
    let document = parse_cif(input)?;

    let cell = [
        cell_item(&document, "_cell.length_a")?,
        cell_item(&document, "_cell.length_b")?,
        cell_item(&document, "_cell.length_c")?,
        cell_item(&document, "_cell.angle_alpha")?,
        cell_item(&document, "_cell.angle_beta")?,
        cell_item(&document, "_cell.angle_gamma")?,
    ];

    let space_group = document
        .item("_symmetry.Int_Tables_number")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| CueMolError::parse("_symmetry.Int_Tables_number not found"))?;

    let refln_loop = document
        .loops()
        .find(|cif_loop| cif_loop.category_is("_refln"))
        .ok_or_else(|| CueMolError::parse("_refln loop not found"))?;

    let col_h = refln_loop
        .column_index("index_h")
        .ok_or_else(|| CueMolError::parse("_refln.index_h not found"))?;
    let col_k = refln_loop
        .column_index("index_k")
        .ok_or_else(|| CueMolError::parse("_refln.index_k not found"))?;
    let col_l = refln_loop
        .column_index("index_l")
        .ok_or_else(|| CueMolError::parse("_refln.index_l not found"))?;
    let col_f = refln_loop
        .column_index("pdbx_FWT")
        .or_else(|| refln_loop.column_index("pdbx_DELFWT"))
        .ok_or_else(|| CueMolError::parse("_refln.pdbx_FWT or pdbx_DELFWT not found"))?;
    let col_phi = refln_loop
        .column_index("pdbx_PHWT")
        .or_else(|| refln_loop.column_index("pdbx_DELPHWT"))
        .ok_or_else(|| CueMolError::parse("_refln.pdbx_PHWT or pdbx_DELPHWT not found"))?;

    let max_col = col_h.max(col_k).max(col_l).max(col_f).max(col_phi);
    let mut reflections = Vec::with_capacity(refln_loop.rows.len());
    for row in &refln_loop.rows {
        if row.len() <= max_col {
            continue;
        }
        // C++ warns and skips rows whose fields do not convert (e.g. `?`/`.`).
        let (Ok(h), Ok(k), Ok(l)) = (
            row[col_h].parse::<i32>(),
            row[col_k].parse::<i32>(),
            row[col_l].parse::<i32>(),
        ) else {
            continue;
        };
        let (Ok(f), Ok(phi)) = (row[col_f].parse::<f64>(), row[col_phi].parse::<f64>()) else {
            continue;
        };
        reflections.push(Reflection {
            h,
            k,
            l,
            f: f as f32,
            phi_deg: phi as f32,
        });
    }

    Ok(CifMapCoeffs {
        cell,
        space_group,
        reflections,
    })
}

/// Read a map-coefficient CIF and synthesize its electron-density `DensityMap`.
/// `resolution <= 0.1` auto-selects the sampling from the maximum Miller indices.
pub fn density_map_from_cif(input: &str, resolution: f64, grid_size: f64) -> Result<DensityMap> {
    let coeffs = parse_cif_map_coeffs(input)?;
    if coeffs.reflections.is_empty() {
        return Err(CueMolError::parse("no reflections read from mmCIF map"));
    }
    density_map_from_reflections_with_cell(
        &coeffs.reflections,
        coeffs.cell,
        coeffs.space_group,
        resolution,
        grid_size,
    )
}

/// Read a required numeric `_cell.*` item, erroring if it is absent.
fn cell_item(document: &cuemol_core::CifDocument, tag: &str) -> Result<f64> {
    document
        .item(tag)
        .and_then(|value| value.parse::<f64>().ok())
        .ok_or_else(|| CueMolError::parse(format!("{tag} not found")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal P1 map CIF carrying a single reflection F(1,0,0), phase 0.
    fn single_reflection_cif(phase_deg: f64) -> String {
        format!(
            "data_map\n\
             _cell.length_a 10.0\n\
             _cell.length_b 12.0\n\
             _cell.length_c 14.0\n\
             _cell.angle_alpha 90.0\n\
             _cell.angle_beta 90.0\n\
             _cell.angle_gamma 90.0\n\
             _symmetry.Int_Tables_number 1\n\
             loop_\n\
             _refln.index_h\n\
             _refln.index_k\n\
             _refln.index_l\n\
             _refln.pdbx_FWT\n\
             _refln.pdbx_PHWT\n\
             1 0 0 1000.0 {phase_deg}\n\
             #\n"
        )
    }

    #[test]
    fn parses_cell_space_group_and_reflections() {
        let coeffs = parse_cif_map_coeffs(&single_reflection_cif(0.0)).unwrap();
        assert_eq!(coeffs.cell, [10.0, 12.0, 14.0, 90.0, 90.0, 90.0]);
        assert_eq!(coeffs.space_group, 1);
        assert_eq!(coeffs.reflections.len(), 1);
        let r = &coeffs.reflections[0];
        assert_eq!((r.h, r.k, r.l), (1, 0, 0));
        assert_eq!(r.f, 1000.0);
    }

    #[test]
    fn falls_back_to_difference_map_columns() {
        // A CIF with only pdbx_DELFWT/pdbx_DELPHWT must still parse.
        let cif = "data_map\n\
             _cell.length_a 10.0\n\
             _cell.length_b 10.0\n\
             _cell.length_c 10.0\n\
             _cell.angle_alpha 90.0\n\
             _cell.angle_beta 90.0\n\
             _cell.angle_gamma 90.0\n\
             _symmetry.Int_Tables_number 1\n\
             loop_\n\
             _refln.index_h\n\
             _refln.index_k\n\
             _refln.index_l\n\
             _refln.pdbx_DELFWT\n\
             _refln.pdbx_DELPHWT\n\
             0 1 0 500.0 90.0\n";
        let coeffs = parse_cif_map_coeffs(cif).unwrap();
        assert_eq!(coeffs.reflections.len(), 1);
        assert_eq!(coeffs.reflections[0].f, 500.0);
    }

    #[test]
    fn density_from_cif_matches_analytic_single_reflection() {
        // Same analytic oracle as the MTZ path: one reflection F(1,0,0), phase 0
        // gives rho = (2F/V) cos(2*pi*a/na); no F000 -> mean 0, max = +2F/V.
        let map = density_map_from_cif(&single_reflection_cif(0.0), -1.0, 0.33).unwrap();
        let amp = 2.0 * 1000.0 / (10.0 * 12.0 * 14.0);
        assert!(map.den_mean().abs() < 1.0e-3, "mean {}", map.den_mean());
        assert!(
            (map.den_max() - amp).abs() < 1.0e-3,
            "max {} vs {amp}",
            map.den_max()
        );
        assert!(map.den_min() < 0.0, "min {}", map.den_min());
    }

    #[test]
    fn density_from_cif_preserves_non_orthogonal_cell() {
        let cif =
            single_reflection_cif(0.0).replace("_cell.angle_gamma 90.0", "_cell.angle_gamma 120.0");
        let map = density_map_from_cif(&cif, -1.0, 0.33).unwrap();
        let cell = map.cell();
        assert!((cell.a - 10.0).abs() < 1.0e-9);
        assert!((cell.b - 12.0).abs() < 1.0e-9);
        assert!((cell.c - 14.0).abs() < 1.0e-9);
        assert!((cell.alpha - 90.0).abs() < 1.0e-9);
        assert!((cell.beta - 90.0).abs() < 1.0e-9);
        assert!((cell.gamma - 120.0).abs() < 1.0e-9);
    }

    #[test]
    fn missing_cell_or_space_group_is_an_error() {
        let no_sg = "data_map\n\
             _cell.length_a 10.0\n_cell.length_b 10.0\n_cell.length_c 10.0\n\
             _cell.angle_alpha 90.0\n_cell.angle_beta 90.0\n_cell.angle_gamma 90.0\n\
             loop_\n_refln.index_h\n_refln.index_k\n_refln.index_l\n\
             _refln.pdbx_FWT\n_refln.pdbx_PHWT\n1 0 0 1.0 0.0\n";
        assert!(parse_cif_map_coeffs(no_sg).is_err());
    }

    #[test]
    fn reflection_rows_can_span_lines_and_use_quoted_numbers() {
        let cif = "data_map\n\
             _cell.length_a 10.0\n\
             _cell.length_b 10.0\n\
             _cell.length_c 10.0\n\
             _cell.angle_alpha 90.0\n\
             _cell.angle_beta 90.0\n\
             _cell.angle_gamma 90.0\n\
             _symmetry.Int_Tables_number 1\n\
             loop_\n\
             _refln.index_h\n\
             _refln.index_k\n\
             _refln.index_l\n\
             _refln.pdbx_FWT\n\
             _refln.pdbx_PHWT\n\
             1 0\n\
             0 '250.0' 45.0\n";
        let coeffs = parse_cif_map_coeffs(cif).unwrap();
        assert_eq!(coeffs.reflections.len(), 1);
        assert_eq!(coeffs.reflections[0].f, 250.0);
        assert_eq!(coeffs.reflections[0].phi_deg, 45.0);
    }
}
