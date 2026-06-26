pub mod cif_map;
pub mod fft;
pub mod map_renderers;
pub mod mtz;
pub mod qdf_map;

use cuemol_qsys::{InOutHandler, ObjReader};

pub use cif_map::{density_map_from_cif, parse_cif_map_coeffs, CifMapCoeffs};
pub use cuemol_molstr::{
    read_brix_map_bytes, read_brix_map_file, read_brix_map_with_header_bytes,
    read_brix_map_with_header_file, read_ccp4_map_bytes, read_ccp4_map_file,
    read_ccp4_map_with_header_bytes, read_ccp4_map_with_header_file, read_mmcif_map_file,
    read_opendx_file, read_opendx_str, read_xplor_map_file, read_xplor_map_str,
    read_xplor_map_with_header_file, read_xplor_map_with_header_str, BrixHeader, BrixMap,
    Ccp4Header, Ccp4Map, GridMap, MapStats, XplorHeader, XplorMap,
};
pub use cuemol_symm::CrystalInfo;
pub use map_renderers::{
    contour_lines_to_obj, DensityMap, GlslMapMeshRenderer, GlslMapMeshRenderer2,
    GlslMapVolRenderer, MapColorMode, MapMeshRenderer, MapRenderer, MapSurfRenderer,
    MapSurfaceDrawMode,
};
pub use mtz::{
    density_map_from_reflections, density_map_from_reflections_with_cell, read_mtz_bytes,
    read_mtz_file, Mtz2MapReader, MtzColumn, MtzFile, MtzReflection, MtzResolution,
};
pub use qdf_map::{
    read_qdf_denmap_bytes, write_qdf_denmap_bytes, QdfDenMapReader, QdfDenMapWriter,
};

pub type CCP4MapReader = Ccp4MapReader;
pub type GLSLMapMeshRenderer = GlslMapMeshRenderer;
pub type GLSLMapMeshRenderer2 = GlslMapMeshRenderer2;
pub type GLSLMapVolRenderer = GlslMapVolRenderer;
pub type MTZ2MapReader = Mtz2MapReader;

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
}

impl Default for Ccp4MapReader {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn resolution(&self) -> f64 {
        self.resolution
    }

    pub fn resoln(&self) -> f64 {
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
        let input = std::fs::read_to_string(path)?;
        self.build_density_map_from_str(&input)
    }
}

impl Default for MmcifMapReader {
    fn default() -> Self {
        Self::new()
    }
}

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
}
