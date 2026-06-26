use crate::fft::{auto_resolution, synthesize_density_map_with_cell, Reflection, SymOp};
use crate::map_renderers::DensityMap;
use cuemol_core::{CueMolError, Result};
use cuemol_qsys::{InOutHandler, ObjReader};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct MtzFile {
    pub header_start_word: usize,
    pub column_count: usize,
    pub reflection_count: usize,
    pub columns: Vec<MtzColumn>,
    pub cell: Option<[f64; 6]>,
    pub space_group: Option<i32>,
    pub resolution: Option<MtzResolution>,
    pub byte_swapped: bool,
    pub reflections: Vec<MtzReflection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtzColumn {
    pub name: String,
    pub kind: char,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MtzResolution {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MtzReflection {
    pub values: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mtz2MapReader {
    reader: ObjReader,
    resolution: f64,
    min_res: f64,
    max_res: f64,
    grid_size: f64,
    column_f: String,
    column_phi: String,
    column_weight: String,
    selected_f: Option<usize>,
    selected_phi: Option<usize>,
    selected_weight: Option<usize>,
    mtz: Option<MtzFile>,
}

impl Default for Mtz2MapReader {
    fn default() -> Self {
        Self {
            reader: ObjReader::new("mtzmap", "MTZ Structure Factor (*.mtz)", "*.mtz"),
            resolution: -1.0,
            min_res: 0.0,
            max_res: 0.0,
            grid_size: 0.33,
            column_f: String::new(),
            column_phi: String::new(),
            column_weight: String::new(),
            selected_f: None,
            selected_phi: None,
            selected_weight: None,
            mtz: None,
        }
    }
}

impl MtzFile {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .find(|column| column.name.eq_ignore_ascii_case(name))
            .map(|column| column.index)
    }

    pub fn reflection_value(&self, reflection_index: usize, column_name: &str) -> Option<f32> {
        let column_index = self.column_index(column_name)?;
        self.reflections
            .get(reflection_index)?
            .values
            .get(column_index)
            .copied()
    }

    pub fn reflection_hkl(&self, reflection_index: usize) -> Option<(i32, i32, i32)> {
        Some((
            self.reflection_value(reflection_index, "H")?.round() as i32,
            self.reflection_value(reflection_index, "K")?.round() as i32,
            self.reflection_value(reflection_index, "L")?.round() as i32,
        ))
    }

    pub fn column_values(&self, column_name: &str) -> Option<Vec<f32>> {
        let column_index = self.column_index(column_name)?;
        Some(
            self.reflections
                .iter()
                .filter_map(|reflection| reflection.values.get(column_index).copied())
                .collect(),
        )
    }
}

impl Mtz2MapReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&self) -> &'static str {
        "mtzmap"
    }

    pub fn type_descr(&self) -> &'static str {
        "MTZ Structure Factor (*.mtz)"
    }

    pub fn file_ext(&self) -> &'static str {
        "*.mtz"
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

    pub fn resolution(&self) -> f64 {
        self.resolution
    }

    pub fn set_resolution(&mut self, value: f64) {
        self.resolution = value;
    }

    pub fn min_res(&self) -> f64 {
        self.min_res
    }

    pub fn max_res(&self) -> f64 {
        self.max_res
    }

    pub fn grid_size(&self) -> f64 {
        self.grid_size
    }

    pub fn gridsize(&self) -> f64 {
        self.grid_size()
    }

    pub fn set_grid_size(&mut self, value: f64) {
        self.grid_size = value;
    }

    pub fn set_gridsize(&mut self, value: f64) {
        self.set_grid_size(value);
    }

    pub fn column_f(&self) -> &str {
        &self.column_f
    }

    pub fn clmn_f(&self) -> &str {
        self.column_f()
    }

    pub fn set_column_f(&mut self, value: impl Into<String>) {
        self.column_f = value.into();
    }

    pub fn set_clmn_f(&mut self, value: impl Into<String>) {
        self.set_column_f(value);
    }

    pub fn column_phi(&self) -> &str {
        &self.column_phi
    }

    pub fn clmn_phi(&self) -> &str {
        self.column_phi()
    }

    pub fn set_column_phi(&mut self, value: impl Into<String>) {
        self.column_phi = value.into();
    }

    pub fn set_clmn_phi(&mut self, value: impl Into<String>) {
        self.set_column_phi(value);
    }

    pub fn column_weight(&self) -> &str {
        &self.column_weight
    }

    pub fn clmn_wt(&self) -> &str {
        self.column_weight()
    }

    pub fn set_column_weight(&mut self, value: impl Into<String>) {
        self.column_weight = value.into();
    }

    pub fn set_clmn_wt(&mut self, value: impl Into<String>) {
        self.set_column_weight(value);
    }

    pub fn selected_f(&self) -> Option<usize> {
        self.selected_f
    }

    pub fn selected_phi(&self) -> Option<usize> {
        self.selected_phi
    }

    pub fn selected_weight(&self) -> Option<usize> {
        self.selected_weight
    }

    pub fn mtz(&self) -> Option<&MtzFile> {
        self.mtz.as_ref()
    }

    pub fn read_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        let mtz = read_mtz_bytes(bytes)?;
        if let Some(resolution) = mtz.resolution {
            self.min_res = resolution.min;
            self.max_res = resolution.max;
            if self.resolution < 0.0 {
                self.resolution = resolution.max;
            }
        }
        self.select_fft_columns_from(&mtz)?;
        self.mtz = Some(mtz);
        Ok(())
    }

    pub fn column_info_json(&self) -> String {
        self.mtz
            .as_ref()
            .map(column_info_json)
            .unwrap_or_else(|| "[]".to_string())
    }

    pub fn get_column_info_json(&self) -> String {
        self.column_info_json()
    }

    fn select_fft_columns_from(&mut self, mtz: &MtzFile) -> Result<()> {
        self.selected_f = select_requested_column(mtz, &self.column_f, 'F');
        self.selected_phi = select_requested_column(mtz, &self.column_phi, 'P');
        self.selected_weight = select_requested_column(mtz, &self.column_weight, 'W');

        if self.selected_f.is_none() {
            let (f, phi) = guess_fft_columns(mtz)?;
            self.selected_f = Some(f);
            self.selected_phi = phi;
        }
        Ok(())
    }

    /// Synthesize the electron-density map from the loaded MTZ via inverse FFT,
    /// using the selected amplitude/phase columns, the MTZ cell and space group,
    /// and the configured (or auto) resolution. Mirrors C++ `MTZ2MapReader`.
    pub fn build_density_map(&self) -> Result<DensityMap> {
        let mtz = self
            .mtz
            .as_ref()
            .ok_or_else(|| CueMolError::invalid_argument("no MTZ data loaded"))?;
        let f_col = self
            .selected_f
            .ok_or_else(|| CueMolError::parse("no amplitude (F) column selected"))?;
        density_map_from_mtz(
            mtz,
            f_col,
            self.selected_phi,
            self.resolution,
            self.grid_size,
        )
    }
}

/// Build an electron-density `DensityMap` from a parsed MTZ and chosen column
/// indices. Reflections whose amplitude is missing (non-finite) are skipped; an
/// absent phase column is treated as phase 0. Resolution <= 0.1 triggers the
/// auto resolution from the maximum Miller indices.
pub fn density_map_from_mtz(
    mtz: &MtzFile,
    f_col: usize,
    phi_col: Option<usize>,
    resolution: f64,
    grid_size: f64,
) -> Result<DensityMap> {
    let cell6 = mtz
        .cell
        .ok_or_else(|| CueMolError::parse("MTZ has no unit cell"))?;

    let mut reflections = Vec::new();
    for index in 0..mtz.reflections.len() {
        let Some((h, k, l)) = mtz.reflection_hkl(index) else {
            continue;
        };
        let values = &mtz.reflections[index].values;
        let Some(&f) = values.get(f_col) else {
            continue;
        };
        if !f.is_finite() {
            continue; // missing reflection
        }
        let phi_deg = phi_col
            .and_then(|c| values.get(c).copied())
            .filter(|p| p.is_finite())
            .unwrap_or(0.0);
        reflections.push(Reflection {
            h,
            k,
            l,
            f,
            phi_deg,
        });
    }

    let sg = mtz.space_group.unwrap_or(1);
    density_map_from_reflections_with_cell(&reflections, cell6, sg, resolution, grid_size)
}

/// Shared structure-factors -> electron-density core used by every reflection
/// source (MTZ, mmCIF map coefficients, ...). Picks the FFT grid resolution
/// (auto from the max Miller indices when `resolution <= 0.1`), expands the
/// reflections by the space group's real symmetry operators (full `symop.dat`
/// table; P1 fallback only if the group is unknown), runs the inverse FFT, and
/// wraps the grid as a `DensityMap`.
pub fn density_map_from_reflections(
    reflections: &[Reflection],
    cell_abc: [f64; 3],
    space_group: i32,
    resolution: f64,
    grid_size: f64,
) -> DensityMap {
    density_map_from_reflections_with_cell(
        reflections,
        [cell_abc[0], cell_abc[1], cell_abc[2], 90.0, 90.0, 90.0],
        space_group,
        resolution,
        grid_size,
    )
    .expect("orthogonal cell parameters are valid")
}

pub fn density_map_from_reflections_with_cell(
    reflections: &[Reflection],
    cell: [f64; 6],
    space_group: i32,
    resolution: f64,
    grid_size: f64,
) -> Result<DensityMap> {
    let cell_abc = [cell[0], cell[1], cell[2]];
    let max_hkl = reflections.iter().fold([1i32; 3], |mut max, r| {
        max[0] = max[0].max(r.h.abs());
        max[1] = max[1].max(r.k.abs());
        max[2] = max[2].max(r.l.abs());
        max
    });

    let grid_spacing = if grid_size > 0.0 {
        grid_size
    } else {
        1.0 / 3.0
    };
    let resolution = if resolution > 0.1 {
        resolution
    } else {
        auto_resolution(cell_abc, max_hkl, grid_spacing)
    };

    let manager = cuemol_symm::SymmOpManager::new();
    let symops: Vec<SymOp> = manager
        .symmetry_operators(space_group)
        .map(|ops| {
            ops.iter()
                .map(|op| SymOp::from_matrix(&op.matrix))
                .collect()
        })
        .unwrap_or_else(|| vec![SymOp::identity()]);

    let grid =
        synthesize_density_map_with_cell(reflections, &symops, cell, resolution, grid_spacing)?;
    Ok(DensityMap::from_grid_map(grid))
}

pub fn read_mtz_file(path: impl AsRef<Path>) -> Result<MtzFile> {
    let bytes = fs::read(path)?;
    read_mtz_bytes(&bytes)
}

pub fn read_mtz_bytes(bytes: &[u8]) -> Result<MtzFile> {
    if bytes.len() < 12 {
        return Err(CueMolError::parse("MTZ file is too short"));
    }
    if bytes.get(0..4) != Some(b"MTZ ".as_slice()) {
        return Err(CueMolError::parse("missing MTZ magic"));
    }

    let (header_start_word, byte_swapped) = detect_mtz_header_start(bytes)?;
    let header_offset = (header_start_word - 1)
        .checked_mul(4)
        .ok_or_else(|| CueMolError::parse("MTZ header offset overflow"))?;
    if header_offset >= bytes.len() {
        return Err(CueMolError::parse("MTZ header offset is outside file"));
    }

    let mut mtz = MtzFile {
        header_start_word,
        column_count: 0,
        reflection_count: 0,
        columns: Vec::new(),
        cell: None,
        space_group: None,
        resolution: None,
        byte_swapped,
        reflections: Vec::new(),
    };

    let mut offset = header_offset;
    while offset + 80 <= bytes.len() {
        let record = std::str::from_utf8(&bytes[offset..offset + 80])
            .map_err(|_| CueMolError::parse("invalid MTZ header text"))?
            .trim_matches(['\0', ' ', '\r', '\n']);
        offset += 80;
        if record.is_empty() {
            continue;
        }
        if record.starts_with("END") {
            break;
        }
        read_mtz_record(record, &mut mtz)?;
    }

    if mtz.column_count == 0 && !mtz.columns.is_empty() {
        mtz.column_count = mtz.columns.len();
    }
    mtz.reflections = read_reflection_body(bytes, header_offset, &mtz)?;
    Ok(mtz)
}

fn read_reflection_body(
    bytes: &[u8],
    header_offset: usize,
    mtz: &MtzFile,
) -> Result<Vec<MtzReflection>> {
    const FIXED_HEADER_BYTES: usize = 80;
    if header_offset <= FIXED_HEADER_BYTES || mtz.column_count == 0 || mtz.reflection_count == 0 {
        return Ok(Vec::new());
    }

    let body = &bytes[FIXED_HEADER_BYTES..header_offset];
    let expected_values = mtz
        .column_count
        .checked_mul(mtz.reflection_count)
        .ok_or_else(|| CueMolError::parse("MTZ reflection table size overflow"))?;
    let expected_bytes = expected_values
        .checked_mul(4)
        .ok_or_else(|| CueMolError::parse("MTZ reflection byte size overflow"))?;
    if body.len() < expected_bytes {
        return Err(CueMolError::parse("MTZ reflection table is truncated"));
    }

    let mut reflections = Vec::with_capacity(mtz.reflection_count);
    for reflection_index in 0..mtz.reflection_count {
        let mut values = Vec::with_capacity(mtz.column_count);
        for column_index in 0..mtz.column_count {
            let value_index = reflection_index * mtz.column_count + column_index;
            values.push(read_f32(body, value_index * 4, mtz.byte_swapped));
        }
        reflections.push(MtzReflection { values });
    }
    Ok(reflections)
}

fn select_requested_column(mtz: &MtzFile, name: &str, kind: char) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    mtz.columns
        .iter()
        .find(|column| column.name.eq_ignore_ascii_case(name) && column.kind == kind)
        .map(|column| column.index)
}

fn guess_fft_columns(mtz: &MtzFile) -> Result<(usize, Option<usize>)> {
    for (f_name, phi_name) in [("2FOFCWT", "PH2FOFCWT"), ("FWT", "PHWT"), ("FWT", "PHIC")] {
        if let Some(f_index) = select_requested_column(mtz, f_name, 'F') {
            let phi_index = select_requested_column(mtz, phi_name, 'P');
            if phi_index.is_some() || f_name == "FWT" && phi_name == "PHIC" {
                return Ok((f_index, phi_index));
            }
        }
    }
    mtz.columns
        .iter()
        .find(|column| column.kind == 'F')
        .map(|column| (column.index, None))
        .ok_or_else(|| CueMolError::parse("FFT target column not found"))
}

fn column_info_json(mtz: &MtzFile) -> String {
    let entries = mtz
        .columns
        .iter()
        .map(|column| {
            format!(
                "{{\"nid\":{}, \"name\": \"{}\", \"type\": \"{}\"}}",
                column.index,
                escape_json(&column.name),
                column.kind
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", entries.join(","))
}

fn escape_json(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn detect_mtz_header_start(bytes: &[u8]) -> Result<(usize, bool)> {
    let little = read_i32(bytes, 4, false);
    if let Some(value) = plausible_header_start(bytes, little) {
        return Ok((value, false));
    }
    let big = read_i32(bytes, 4, true);
    if let Some(value) = plausible_header_start(bytes, big) {
        return Ok((value, true));
    }
    Err(CueMolError::parse("invalid MTZ header start"))
}

fn plausible_header_start(bytes: &[u8], value: i32) -> Option<usize> {
    if value <= 0 {
        return None;
    }
    let value = value as usize;
    let offset = value.checked_sub(1)?.checked_mul(4)?;
    (offset < bytes.len()).then_some(value)
}

fn read_mtz_record(record: &str, mtz: &mut MtzFile) -> Result<()> {
    let tokens = record.split_whitespace().collect::<Vec<_>>();
    let Some(keyword) = tokens.first().copied() else {
        return Ok(());
    };
    match keyword {
        "NCOL" => {
            if tokens.len() < 3 {
                return Err(CueMolError::parse("invalid MTZ NCOL record"));
            }
            mtz.column_count = parse_usize_token(tokens[1], "invalid MTZ NCOL")?;
            mtz.reflection_count = parse_usize_token(tokens[2], "invalid MTZ NREFL")?;
        }
        "COL" | "COLUMN" => {
            if tokens.len() < 3 {
                return Err(CueMolError::parse("invalid MTZ COL record"));
            }
            let kind = tokens[2]
                .chars()
                .next()
                .ok_or_else(|| CueMolError::parse("invalid MTZ column type"))?;
            mtz.columns.push(MtzColumn {
                name: tokens[1].to_string(),
                kind,
                index: mtz.columns.len(),
            });
        }
        "CELL" => {
            if tokens.len() < 7 {
                return Err(CueMolError::parse("invalid MTZ CELL record"));
            }
            mtz.cell = Some(parse_cell_tokens(&tokens[1..7])?);
        }
        "DCELL" => {
            if tokens.len() < 8 {
                return Err(CueMolError::parse("invalid MTZ DCELL record"));
            }
            mtz.cell = Some(parse_cell_tokens(&tokens[2..8])?);
        }
        "SYMINF" => {
            if tokens.len() >= 5 {
                mtz.space_group = Some(parse_i32_token(tokens[4], "invalid MTZ space group")?);
            }
        }
        "RESO" | "RESOL" | "RESOLUTION" => {
            if tokens.len() < 3 {
                return Err(CueMolError::parse("invalid MTZ RESO record"));
            }
            let low = parse_f64_token(tokens[1], "invalid MTZ low resolution")?;
            let high = parse_f64_token(tokens[2], "invalid MTZ high resolution")?;
            if low <= 0.0 || high <= 0.0 {
                return Err(CueMolError::parse("invalid MTZ resolution values"));
            }
            mtz.resolution = Some(MtzResolution {
                min: (1.0 / low).sqrt(),
                max: (1.0 / high).sqrt(),
            });
        }
        _ => {}
    }
    Ok(())
}

fn parse_cell_tokens(tokens: &[&str]) -> Result<[f64; 6]> {
    Ok([
        parse_f64_token(tokens[0], "invalid MTZ cell a")?,
        parse_f64_token(tokens[1], "invalid MTZ cell b")?,
        parse_f64_token(tokens[2], "invalid MTZ cell c")?,
        parse_f64_token(tokens[3], "invalid MTZ cell alpha")?,
        parse_f64_token(tokens[4], "invalid MTZ cell beta")?,
        parse_f64_token(tokens[5], "invalid MTZ cell gamma")?,
    ])
}

fn parse_usize_token(token: &str, message: &'static str) -> Result<usize> {
    token
        .parse::<usize>()
        .map_err(|_| CueMolError::parse(message))
}

fn parse_i32_token(token: &str, message: &'static str) -> Result<i32> {
    token
        .parse::<i32>()
        .map_err(|_| CueMolError::parse(message))
}

fn parse_f64_token(token: &str, message: &'static str) -> Result<f64> {
    token
        .parse::<f64>()
        .map_err(|_| CueMolError::parse(message))
}

fn read_i32(bytes: &[u8], offset: usize, byte_swapped: bool) -> i32 {
    let field = bytes[offset..offset + 4].try_into().unwrap();
    if byte_swapped {
        i32::from_be_bytes(field)
    } else {
        i32::from_le_bytes(field)
    }
}

fn read_f32(bytes: &[u8], offset: usize, byte_swapped: bool) -> f32 {
    let field = bytes[offset..offset + 4].try_into().unwrap();
    if byte_swapped {
        f32::from_be_bytes(field)
    } else {
        f32::from_le_bytes(field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, kind: char, index: usize) -> MtzColumn {
        MtzColumn {
            name: name.to_string(),
            kind,
            index,
        }
    }

    #[test]
    fn density_map_from_mtz_single_reflection_stats() {
        // Columns H,K,L,F,PHI with one reflection F(1,0,0)=1000, phase 0.
        let mtz = MtzFile {
            header_start_word: 0,
            column_count: 5,
            reflection_count: 1,
            columns: vec![
                column("H", 'H', 0),
                column("K", 'H', 1),
                column("L", 'H', 2),
                column("F", 'F', 3),
                column("PHI", 'P', 4),
            ],
            cell: Some([10.0, 12.0, 14.0, 90.0, 90.0, 90.0]),
            space_group: Some(1),
            resolution: None,
            byte_swapped: false,
            reflections: vec![MtzReflection {
                values: vec![1.0, 0.0, 0.0, 1000.0, 0.0],
            }],
        };

        let map = density_map_from_mtz(&mtz, 3, Some(4), -1.0, 0.33).unwrap();
        // rho = (2A/V) cos(2*pi*a/na): no F000 -> mean 0; max at a=0 is +2A/V.
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
    fn density_map_from_mtz_skips_missing_amplitude() {
        // A NaN amplitude reflection must be skipped (no panic; finite map).
        let mtz = MtzFile {
            header_start_word: 0,
            column_count: 4,
            reflection_count: 2,
            columns: vec![
                column("H", 'H', 0),
                column("K", 'H', 1),
                column("L", 'H', 2),
                column("F", 'F', 3),
            ],
            cell: Some([10.0, 10.0, 10.0, 90.0, 90.0, 90.0]),
            space_group: Some(1),
            resolution: None,
            byte_swapped: false,
            reflections: vec![
                MtzReflection {
                    values: vec![1.0, 0.0, 0.0, f32::NAN],
                },
                MtzReflection {
                    values: vec![0.0, 1.0, 0.0, 500.0],
                },
            ],
        };
        let map = density_map_from_mtz(&mtz, 3, None, -1.0, 0.33).unwrap();
        assert!(map.den_mean().abs() < 1.0e-3);
        assert!(map.den_max() > 0.0);
    }

    fn general_reflection_mtz(space_group: i32) -> MtzFile {
        // One general reflection F(1,2,3); same data, different space group.
        MtzFile {
            header_start_word: 0,
            column_count: 5,
            reflection_count: 1,
            columns: vec![
                column("H", 'H', 0),
                column("K", 'H', 1),
                column("L", 'H', 2),
                column("F", 'F', 3),
                column("PHI", 'P', 4),
            ],
            cell: Some([20.0, 24.0, 28.0, 90.0, 90.0, 90.0]),
            space_group: Some(space_group),
            resolution: None,
            byte_swapped: false,
            reflections: vec![MtzReflection {
                values: vec![1.0, 2.0, 3.0, 1000.0, 30.0],
            }],
        }
    }

    #[test]
    fn density_synthesis_expands_by_real_space_group_symmetry() {
        // With the full symop table, a non-triclinic group now contributes its
        // real operators instead of falling back to P1. P212121 (#19) has 4
        // operators, so its density must differ from the P1 synthesis of the
        // same single reflection. (Before the table was wired this would have
        // produced an identical, P1-only map.)
        let p1 = density_map_from_mtz(&general_reflection_mtz(1), 3, Some(4), 2.0, 0.33).unwrap();
        let p212121 =
            density_map_from_mtz(&general_reflection_mtz(19), 3, Some(4), 2.0, 0.33).unwrap();

        // Same grid, but symmetry mates change the density distribution.
        assert_eq!(p1.byte_values().len(), p212121.byte_values().len());
        assert!(
            (p1.den_sigma() - p212121.den_sigma()).abs() > 1.0e-6
                || (p1.den_max() - p212121.den_max()).abs() > 1.0e-6,
            "P212121 synthesis did not differ from P1 (symmetry not applied)"
        );
        assert!(p212121.den_sigma() > 0.0);
    }
}
