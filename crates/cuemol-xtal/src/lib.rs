pub mod cif_map;
pub mod fft;
pub mod map_renderers;
pub mod mtz;
pub mod qdf_map;

use cuemol_qsys::{InOutHandler, ObjReader};

pub use cif_map::{density_map_from_cif, parse_cif_map_coeffs, CifMapCoeffs};
pub use cuemol_core::{CifBlock, CifDocument, CifItem, CifLoop, CifParser};
pub use cuemol_molstr::{
    read_brix_map_bytes, read_brix_map_file, read_brix_map_with_header_bytes,
    read_brix_map_with_header_file, read_ccp4_map_bytes, read_ccp4_map_file,
    read_ccp4_map_with_header_bytes, read_ccp4_map_with_header_file, read_opendx_file,
    read_opendx_str, read_xplor_map_file, read_xplor_map_str, read_xplor_map_with_header_file,
    read_xplor_map_with_header_str, BrixHeader, BrixMap, Ccp4Header, Ccp4Map, GridMap, MapStats,
    XplorHeader, XplorMap,
};
pub use cuemol_symm::CrystalInfo;
pub use fft::{MapFFT, Reflection, SymOp};
pub use map_renderers::{
    contour_lines_to_obj, DensityMap, GlslMapMeshRenderer, GlslMapMeshRenderer2,
    GlslMapVolRenderer, MapColorMode, MapMeshRenderer, MapRenderer, MapSurfRenderer,
    MapSurfaceDrawMode,
};
pub use mtz::{
    density_map_from_mtz, density_map_from_mtz_weighted, density_map_from_reflections,
    density_map_from_reflections_with_cell, read_mtz_bytes, read_mtz_file, read_mtz_metadata_file,
    Mtz2MapReader, MtzColumn, MtzFile, MtzReflection, MtzResolution,
};
pub use qdf_map::{
    read_qdf_denmap_bytes, write_qdf_denmap_bytes, QdfDenMapReader, QdfDenMapWriter,
};

pub type CCP4MapReader = Ccp4MapReader;
pub type GLSLMapMeshRenderer = GlslMapMeshRenderer;
pub type GLSLMapMeshRenderer2 = GlslMapMeshRenderer2;
pub type GLSLMapVolRenderer = GlslMapVolRenderer;
pub type MTZ2MapReader = Mtz2MapReader;

/// Read map coefficients from an mmCIF file and synthesize the density map.
/// Compressed inputs use the same gzip/bzip2/xz path as molecule readers.
pub fn read_mmcif_map_file(path: impl AsRef<std::path::Path>) -> cuemol_core::Result<DensityMap> {
    let input = cuemol_molstr::read_text_maybe_compressed(path)?;
    if input.starts_with("version https://git-lfs.github.com/spec/v1") {
        return Err(cuemol_core::CueMolError::parse(
            "mmCIF map is a Git LFS pointer",
        ));
    }
    density_map_from_cif(&input, -1.0, 0.33)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensityMapFormat {
    OpenDx,
    Mmcif,
    Ccp4,
    Xplor,
    Brix,
    Mtz,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrixMapReader {
    reader: ObjReader,
}

impl BrixMapReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new(
                "brix",
                "BRIX Density Map(*.brix;*.omap)",
                "*.brix; *.omap; *.omap.gz",
            ),
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

    pub fn create_default_obj(&self) -> DensityMap {
        DensityMap::new()
    }

    pub fn can_handle_content(&self, input: &[u8]) -> cuemol_molstr::ContentVerdict {
        let first_token = String::from_utf8_lossy(input)
            .split(|ch: char| ch == ' ' || ch == ',' || ch.is_ascii_whitespace())
            .find(|token| !token.is_empty())
            .map(str::to_string);
        match first_token.as_deref() {
            Some(":-)") => cuemol_molstr::ContentVerdict::Yes,
            _ => cuemol_molstr::ContentVerdict::Unknown,
        }
    }

    pub fn read_file(&self, path: impl AsRef<std::path::Path>) -> cuemol_core::Result<BrixMap> {
        read_brix_map_with_header_file(path)
    }

    pub fn read_bytes(&self, input: &[u8]) -> cuemol_core::Result<BrixMap> {
        read_brix_map_with_header_bytes(input)
    }

    pub fn read_density_map_file(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<DensityMap> {
        Ok(density_map_from_brix(self.read_file(path)?))
    }

    pub fn read_density_map_bytes(&self, input: &[u8]) -> cuemol_core::Result<DensityMap> {
        Ok(density_map_from_brix(self.read_bytes(input)?))
    }
}

impl Default for BrixMapReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ccp4MapReader {
    reader: ObjReader,
    normalize: bool,
    truncate_min: bool,
    min: f64,
    truncate_max: bool,
    max: f64,
}

impl Ccp4MapReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new(
                "ccp4map",
                "CCP4 Density Map(*.map;*.ccp4;*.mrc;*.ccp4.gz)",
                "*.map; *.ccp4; *.mrc; *.ccp4.gz",
            ),
            normalize: false,
            truncate_min: false,
            min: 0.0,
            truncate_max: false,
            max: 5.0,
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

    pub fn normalize(&self) -> bool {
        self.normalize
    }

    pub fn set_normalize(&mut self, value: bool) {
        self.normalize = value;
    }

    pub fn truncate_min(&self) -> bool {
        self.truncate_min
    }

    pub fn set_truncate_min(&mut self, value: bool) {
        self.truncate_min = value;
    }

    pub fn min(&self) -> f64 {
        self.min
    }

    pub fn set_min(&mut self, value: f64) {
        self.min = value;
    }

    pub fn truncate_max(&self) -> bool {
        self.truncate_max
    }

    pub fn set_truncate_max(&mut self, value: bool) {
        self.truncate_max = value;
    }

    pub fn max(&self) -> f64 {
        self.max
    }

    pub fn set_max(&mut self, value: f64) {
        self.max = value;
    }

    pub fn create_default_obj(&self) -> DensityMap {
        DensityMap::new()
    }

    pub fn can_handle_content(&self, input: &[u8]) -> cuemol_molstr::ContentVerdict {
        if input.get(208..212) == Some(b"MAP ".as_slice()) {
            cuemol_molstr::ContentVerdict::Yes
        } else {
            cuemol_molstr::ContentVerdict::Unknown
        }
    }

    pub fn read_file(&self, path: impl AsRef<std::path::Path>) -> cuemol_core::Result<Ccp4Map> {
        let mut map = read_ccp4_map_with_header_file(path)?;
        self.apply_options(&mut map)?;
        Ok(map)
    }

    pub fn read_bytes(&self, input: &[u8]) -> cuemol_core::Result<Ccp4Map> {
        let mut map = read_ccp4_map_with_header_bytes(input)?;
        self.apply_options(&mut map)?;
        Ok(map)
    }

    pub fn read_density_map_file(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<DensityMap> {
        Ok(density_map_from_ccp4(self.read_file(path)?))
    }

    pub fn read_density_map_bytes(&self, input: &[u8]) -> cuemol_core::Result<DensityMap> {
        Ok(density_map_from_ccp4(self.read_bytes(input)?))
    }

    fn apply_options(&self, map: &mut Ccp4Map) -> cuemol_core::Result<()> {
        if map.header.mode != 2 {
            return Ok(());
        }

        let sigma = map.header.stats.rms;
        if self.truncate_min {
            let lower = self.min * sigma;
            for value in &mut map.grid.values {
                *value = value.max(lower);
            }
        }
        if self.truncate_max {
            let upper = self.max * sigma;
            for value in &mut map.grid.values {
                *value = value.min(upper);
            }
        }
        if self.normalize {
            if !sigma.is_finite() || sigma.abs() <= f64::EPSILON {
                return Err(cuemol_core::CueMolError::parse(
                    "cannot normalize CCP4 map with zero RMS",
                ));
            }
            let mean = map.header.stats.mean;
            for value in &mut map.grid.values {
                *value = (*value - mean) / sigma;
            }
        }
        Ok(())
    }
}

impl Default for Ccp4MapReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XplorMapReader {
    reader: ObjReader,
}

impl XplorMapReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new(
                "xplormap",
                "XPLOR/CNS Density Map(*.map;*.cns)",
                "*.map;*.cns",
            ),
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

    pub fn create_default_obj(&self) -> DensityMap {
        DensityMap::new()
    }

    pub fn can_handle_content(&self, input: &str) -> cuemol_molstr::ContentVerdict {
        if input.lines().any(|line| line.trim().starts_with("ZYX")) {
            cuemol_molstr::ContentVerdict::Yes
        } else {
            cuemol_molstr::ContentVerdict::Unknown
        }
    }

    pub fn read_file(&self, path: impl AsRef<std::path::Path>) -> cuemol_core::Result<XplorMap> {
        read_xplor_map_with_header_file(path)
    }

    pub fn read_str(&self, input: &str) -> cuemol_core::Result<XplorMap> {
        read_xplor_map_with_header_str(input)
    }

    pub fn read_density_map_file(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<DensityMap> {
        Ok(density_map_from_xplor(self.read_file(path)?))
    }

    pub fn read_density_map_str(&self, input: &str) -> cuemol_core::Result<DensityMap> {
        Ok(density_map_from_xplor(self.read_str(input)?))
    }
}

impl Default for XplorMapReader {
    fn default() -> Self {
        Self::new()
    }
}

fn density_map_from_brix(map: BrixMap) -> DensityMap {
    let header = map.header;
    DensityMap::from_crystallographic_grid(
        map.grid,
        header.start.map(|value| value as isize),
        header.grid_intervals,
        header.cell,
        1,
    )
}

fn density_map_from_ccp4(map: Ccp4Map) -> DensityMap {
    let header = map.header;
    DensityMap::from_crystallographic_grid(
        map.grid,
        header.start.map(|value| value as isize),
        header.grid_intervals,
        header.cell,
        header.space_group,
    )
}

fn density_map_from_xplor(map: XplorMap) -> DensityMap {
    let header = map.header;
    DensityMap::from_crystallographic_grid(
        map.grid,
        header.start.map(|value| value as isize),
        header.grid_intervals,
        header.cell,
        1,
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct MmcifMapReader {
    reader: ObjReader,
    resolution: f64,
    grid_size: f64,
}

impl MmcifMapReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new(
                "mmcifmap",
                "mmCIF Map Coeff (*.cif;*.cif.gz)",
                "*.cif; *.cif.gz",
            ),
            resolution: -1.0,
            grid_size: 0.33,
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

    pub fn create_default_obj(&self) -> DensityMap {
        DensityMap::new()
    }

    pub fn can_handle_content(&self, input: &str) -> cuemol_molstr::ContentVerdict {
        for line in input.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with("_refln.") {
                return cuemol_molstr::ContentVerdict::Yes;
            }
            if line.starts_with("_atom_site.") {
                return cuemol_molstr::ContentVerdict::No;
            }
        }
        cuemol_molstr::ContentVerdict::Unknown
    }

    pub fn resolution(&self) -> f64 {
        self.resolution
    }

    pub fn resoln(&self) -> f64 {
        self.resolution()
    }

    pub fn get_resoln(&self) -> f64 {
        self.resolution()
    }

    pub fn set_resolution(&mut self, value: f64) {
        self.resolution = value;
    }

    pub fn set_resoln(&mut self, value: f64) {
        self.set_resolution(value);
    }

    pub fn grid_size(&self) -> f64 {
        self.grid_size
    }

    pub fn get_grid_size(&self) -> f64 {
        self.grid_size()
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

    /// Synthesize the electron-density map from mmCIF map-coefficient text
    /// (the `_refln` FWT/PHWT loop + cell + space group), using the configured
    /// (or auto) resolution and grid. Faithful equivalent of C++
    /// `MmcifMapReader::read`, which consumes the already-decompressed stream.
    pub fn build_density_map_from_str(&self, input: &str) -> cuemol_core::Result<DensityMap> {
        density_map_from_cif(input, self.resolution, self.grid_size)
    }

    /// Convenience file entry point. The CIF is read as plain text; compressed
    /// inputs are decompressed by the stream/handler layer before reaching here
    /// (mirrors the C++ InStream contract).
    pub fn build_density_map_from_file(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<DensityMap> {
        let input = cuemol_molstr::read_text_maybe_compressed(path)?;
        self.build_density_map_from_str(&input)
    }
}

impl Default for MmcifMapReader {
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

impl_obj_reader_deref!(BrixMapReader, Ccp4MapReader, MmcifMapReader, XplorMapReader);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_reader_metadata_and_defaults_match_qif() {
        let brix = BrixMapReader::new();
        assert_eq!(brix.handler().name(), "brix");
        assert_eq!(
            brix.handler().type_descr(),
            "BRIX Density Map(*.brix;*.omap)"
        );
        assert_eq!(brix.handler().file_ext(), "*.brix; *.omap; *.omap.gz");
        assert_eq!(brix.handler().category(), InOutHandler::IOH_CAT_OBJREADER);

        let ccp4 = Ccp4MapReader::new();
        assert_eq!(ccp4.handler().name(), "ccp4map");
        assert_eq!(
            ccp4.handler().type_descr(),
            "CCP4 Density Map(*.map;*.ccp4;*.mrc;*.ccp4.gz)"
        );
        assert!(!ccp4.normalize());
        assert!(!ccp4.truncate_min());
        assert_eq!(ccp4.min(), 0.0);
        assert!(!ccp4.truncate_max());
        assert_eq!(ccp4.max(), 5.0);

        let mmcif = MmcifMapReader::new();
        assert_eq!(mmcif.handler().name(), "mmcifmap");
        assert_eq!(
            mmcif.handler().type_descr(),
            "mmCIF Map Coeff (*.cif;*.cif.gz)"
        );
        assert_eq!(mmcif.resolution(), -1.0);
        assert_eq!(mmcif.grid_size(), 0.33);

        let xplor = XplorMapReader::new();
        assert_eq!(xplor.handler().name(), "xplormap");
        assert_eq!(
            xplor.handler().type_descr(),
            "XPLOR/CNS Density Map(*.map;*.cns)"
        );
        assert_eq!(xplor.handler().file_ext(), "*.map;*.cns");

        let mtz = Mtz2MapReader::new();
        assert_eq!(mtz.handler().name(), "mtzmap");
        assert_eq!(mtz.handler().type_descr(), "MTZ Structure Factor (*.mtz)");
        assert_eq!(mtz.resolution(), -1.0);
        assert_eq!(mtz.grid_size(), 0.33);
    }

    #[test]
    fn mmcif_map_reader_builds_density_from_coefficients() {
        // Exercise the public reader entry point end to end (parse -> FFT).
        let cif = "data_map\n\
             _cell.length_a 16.0\n_cell.length_b 16.0\n_cell.length_c 16.0\n\
             _cell.angle_alpha 90.0\n_cell.angle_beta 90.0\n_cell.angle_gamma 90.0\n\
             _symmetry.Int_Tables_number 1\n\
             loop_\n_refln.index_h\n_refln.index_k\n_refln.index_l\n\
             _refln.pdbx_FWT\n_refln.pdbx_PHWT\n\
             1 0 0 800.0 0.0\n0 1 1 400.0 45.0\n#\n";
        let reader = MmcifMapReader::new();
        let map = reader.build_density_map_from_str(cif).unwrap();
        assert!(map.den_min().is_finite() && map.den_max().is_finite());
        assert!(map.den_max() > map.den_min());
        assert!(map.den_sigma() > 0.0);
    }

    #[test]
    fn map_readers_sniff_reference_magic_and_xplor_axis_marker() {
        let brix = BrixMapReader::new();
        assert_eq!(
            brix.can_handle_content(b" \n :-) origin 0 0 0"),
            cuemol_molstr::ContentVerdict::Yes
        );
        assert_eq!(
            brix.can_handle_content(b"not a BRIX map"),
            cuemol_molstr::ContentVerdict::Unknown
        );

        let ccp4 = Ccp4MapReader::new();
        let mut header = vec![0_u8; 212];
        header[208..212].copy_from_slice(b"MAP ");
        assert_eq!(
            ccp4.can_handle_content(&header),
            cuemol_molstr::ContentVerdict::Yes
        );
        assert_eq!(
            ccp4.can_handle_content(b"MAP "),
            cuemol_molstr::ContentVerdict::Unknown
        );

        let xplor = XplorMapReader::new();
        assert_eq!(
            xplor.can_handle_content("REMARKS verbose\n  ZYX\n"),
            cuemol_molstr::ContentVerdict::Yes
        );
        assert_eq!(
            xplor.can_handle_content("XYZ\n"),
            cuemol_molstr::ContentVerdict::Unknown
        );

        let mmcif = MmcifMapReader::new();
        assert_eq!(
            mmcif.can_handle_content("data_map\n# comment\n_refln.index_h 1\n"),
            cuemol_molstr::ContentVerdict::Yes
        );
        assert_eq!(
            mmcif.can_handle_content("data_model\n_atom_site.id 1\n"),
            cuemol_molstr::ContentVerdict::No
        );
        assert_eq!(
            mmcif.can_handle_content("data_empty\n_cell.length_a 10\n"),
            cuemol_molstr::ContentVerdict::Unknown
        );

        let mtz = Mtz2MapReader::new();
        assert_eq!(
            mtz.can_handle_content(b"MTZ \0\0\0\0"),
            cuemol_molstr::ContentVerdict::Yes
        );
        assert_eq!(
            mtz.can_handle_content(b"MTZ"),
            cuemol_molstr::ContentVerdict::Unknown
        );
    }

    #[test]
    fn ccp4_reader_applies_truncation_then_normalization() {
        let input = ccp4_test_bytes(&[-5.0, 7.0], 1.0, 2.0);
        let mut reader = Ccp4MapReader::new();
        reader.set_truncate_min(true);
        reader.set_min(-1.0);
        reader.set_truncate_max(true);
        reader.set_max(2.0);
        reader.set_normalize(true);

        let map = reader.read_bytes(&input).unwrap();
        assert_eq!(map.grid.values, vec![-1.5, 1.5]);

        let zero_rms = ccp4_test_bytes(&[1.0], 1.0, 0.0);
        assert!(reader.read_bytes(&zero_rms).is_err());
    }

    #[test]
    fn ccp4_density_reader_preserves_crystallographic_header() {
        let mut input = ccp4_test_bytes(&[1.0, 2.0], 1.5, 0.5);
        write_i32_word(&mut input, 4, 3);
        write_i32_word(&mut input, 5, -2);
        write_i32_word(&mut input, 6, 5);
        write_i32_word(&mut input, 22, 19);

        let density = Ccp4MapReader::new().read_density_map_bytes(&input).unwrap();
        assert_eq!(
            [
                cuemol_qsys::ScalarObject::start_col(&density),
                cuemol_qsys::ScalarObject::start_row(&density),
                cuemol_qsys::ScalarObject::start_sec(&density),
            ],
            [3, -2, 5]
        );
        assert_eq!(density.intervals(), [2, 1, 1]);
        assert_eq!(density.cell().space_group(), 19);
        let first = cuemol_qsys::ScalarObject::conv_to_orth(&density, cuemol_core::Vector4::ZERO);
        assert!((first - cuemol_core::Vector4::new3(3.0, -2.0, 5.0)).length3d() < 1.0e-12);
        assert!(cuemol_qsys::ScalarObject::is_in_range(&density, first));
    }

    #[test]
    fn xplor_reader_builds_map_and_density_object() {
        let mut input = String::from("\n       1\n REMARKS generated\n");
        input.push_str(&format!(
            "{:8}{:8}{:8}{:8}{:8}{:8}{:8}{:8}{:8}\n",
            2, 0, 1, 1, 0, 0, 1, 0, 0
        ));
        input.push_str(&format!(
            "{:12.5E}{:12.5E}{:12.5E}{:12.5E}{:12.5E}{:12.5E}\n",
            2.0, 1.0, 1.0, 90.0, 90.0, 90.0
        ));
        input.push_str("ZYX\n0\n 1.00000E+00 2.00000E+00\n");
        let reader = XplorMapReader::new();
        let map = reader.read_str(&input).unwrap();
        assert_eq!(map.grid.counts, [2, 1, 1]);
        assert_eq!(map.grid.values, vec![1.0, 2.0]);

        let density = reader.read_density_map_str(&input).unwrap();
        assert!(density.den_min() <= 1.0);
        assert!(density.den_max() >= 2.0);
    }

    fn ccp4_test_bytes(values: &[f32], mean: f32, rms: f32) -> Vec<u8> {
        let mut bytes = vec![0_u8; 1024];
        write_i32_word(&mut bytes, 0, values.len() as i32);
        write_i32_word(&mut bytes, 1, 1);
        write_i32_word(&mut bytes, 2, 1);
        write_i32_word(&mut bytes, 3, 2);
        write_i32_word(&mut bytes, 7, values.len() as i32);
        write_i32_word(&mut bytes, 8, 1);
        write_i32_word(&mut bytes, 9, 1);
        write_f32_word(&mut bytes, 10, values.len() as f32);
        write_f32_word(&mut bytes, 11, 1.0);
        write_f32_word(&mut bytes, 12, 1.0);
        write_f32_word(&mut bytes, 13, 90.0);
        write_f32_word(&mut bytes, 14, 90.0);
        write_f32_word(&mut bytes, 15, 90.0);
        write_i32_word(&mut bytes, 16, 1);
        write_i32_word(&mut bytes, 17, 2);
        write_i32_word(&mut bytes, 18, 3);
        write_f32_word(&mut bytes, 19, -5.0);
        write_f32_word(&mut bytes, 20, 7.0);
        write_f32_word(&mut bytes, 21, mean);
        write_i32_word(&mut bytes, 22, 1);
        bytes[208..212].copy_from_slice(b"MAP ");
        write_f32_word(&mut bytes, 54, rms);
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn write_i32_word(bytes: &mut [u8], word: usize, value: i32) {
        bytes[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_f32_word(bytes: &mut [u8], word: usize, value: f32) {
        bytes[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}
