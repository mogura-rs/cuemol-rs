pub mod amber;
mod fileio;
pub mod gro;
pub mod namd;
pub mod netcdf;
pub mod psf;
pub mod trajectory;
pub mod trr;
pub mod xdr;
pub mod xtc;

pub use amber::{
    apply_amber_mdcrd_frame, apply_amber_restart, apply_amber_restart_str, read_amber_mdcrd_file,
    read_amber_mdcrd_str, read_amber_prmtop_file, read_amber_prmtop_str,
    read_amber_prmtop_with_restart_files, read_amber_prmtop_with_restart_str,
    read_amber_restart_file, read_amber_restart_str, AmberCrdReader, AmberMdcrdFrame,
    AmberMdcrdTrajectory, AmberPrmtopLoad, AmberPrmtopReader, AmberRestart,
};
pub use gro::{
    read_gro_file, read_gro_frame_file, read_gro_frame_str, read_gro_str, GroFrame, GroReader,
};
pub use namd::{
    apply_namd_coor_bytes, apply_namd_coor_file, apply_namd_coor_frame, read_namd_coor_bytes,
    read_namd_coor_file, read_namd_coor_with_psf_bytes, read_namd_coor_with_psf_files,
    NamdCoorFrame, NamdCoorLoad, NamdCoorReader,
};
pub use netcdf::{
    read_amber_netcdf_bytes, read_amber_netcdf_bytes_with_options, read_amber_netcdf_file,
    read_amber_netcdf_file_with_options, sniff_amber_netcdf_content, AmberNetCDFReadOptions,
    AmberNetCDFReader, NcDim, NcValue, NcVar, Netcdf3Frame, Netcdf3InStream,
};
pub use psf::{read_psf_file, read_psf_str, sniff_psf_content, PsfReader};
pub use trajectory::{
    read_dcd_bytes, read_dcd_bytes_with_options, read_dcd_file, read_dcd_file_with_options,
    sniff_dcd_content, DcdReadOptions, DcdTrajReader, TrajBlock, Trajectory,
};
pub use trr::{
    read_trr_bytes, read_trr_bytes_with_options, read_trr_file, read_trr_file_with_options,
    sniff_trr_content, TrrReadOptions, TrrTrajReader,
};
pub use xdr::XdrInStream;
pub use xtc::{
    read_xtc_bytes, read_xtc_bytes_with_options, read_xtc_file, read_xtc_file_with_options,
    sniff_xtc_content, XtcReadOptions, XtcTrajReader,
};

pub type GROFileReader = GroReader;
pub type NAMDCoorReader = NamdCoorReader;
pub type PSFReader = PsfReader;
pub type DCDTrajReader = DcdTrajReader;
pub type TRRTrajReader = TrrTrajReader;
pub type XTCTrajReader = XtcTrajReader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdCoordinateFormat {
    Gro,
    NamdCoor,
    AmberCrd,
    AmberMdcrd,
    AmberNetcdf,
    Dcd,
    Trr,
    Xtc,
    Psf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_qsys::InOutHandler;

    #[test]
    fn reader_metadata_matches_qif_reference() {
        let amber = AmberPrmtopReader::new();
        assert_eq!(amber.handler().name(), "amberprm");
        assert_eq!(
            amber.handler().type_descr(),
            "AMBER topology (*.prmtop;*.parm7;*.top)"
        );
        assert_eq!(amber.handler().file_ext(), "*.prmtop;*.parm7;*.top");
        assert_eq!(amber.handler().category(), InOutHandler::IOH_CAT_OBJREADER);

        let gro = GroReader::new();
        assert_eq!(gro.handler().name(), "gro");
        assert_eq!(gro.handler().type_descr(), "GROMACS Coordinates (*.gro)");
        assert_eq!(gro.handler().file_ext(), "*.gro");
        assert_eq!(gro.handler().category(), InOutHandler::IOH_CAT_OBJREADER);

        let namd = NamdCoorReader::new();
        assert_eq!(namd.handler().name(), "namdcoor");
        assert_eq!(namd.handler().type_descr(), "NAMD Coordinates (*.coor)");
        assert_eq!(namd.handler().file_ext(), "*.coor");
        assert_eq!(namd.handler().category(), InOutHandler::IOH_CAT_OBJREADER);

        let psf = PsfReader::new();
        assert_eq!(psf.handler().name(), "psf");
        assert_eq!(
            psf.handler().type_descr(),
            "CHARMM/NAMD PSF topology (*.psf)"
        );
        assert_eq!(psf.handler().file_ext(), "*.psf");
        assert_eq!(psf.handler().category(), InOutHandler::IOH_CAT_OBJREADER);

        let dcd = DcdTrajReader::new();
        assert_eq!(dcd.handler().name(), "dcdtraj");
        assert_eq!(dcd.handler().type_descr(), "DCD binary trajectory (*.dcd)");
        assert_eq!(dcd.handler().file_ext(), "*.dcd");
        assert_eq!(dcd.handler().category(), InOutHandler::IOH_CAT_OBJREADER);

        let trr = TrrTrajReader::new();
        assert_eq!(trr.handler().name(), "trrtraj");
        assert_eq!(trr.handler().type_descr(), "GROMACS TRR trajectory (*.trr)");
        assert_eq!(trr.handler().file_ext(), "*.trr");
        assert_eq!(trr.handler().category(), InOutHandler::IOH_CAT_OBJREADER);

        let xtc = XtcTrajReader::new();
        assert_eq!(xtc.handler().name(), "xtctraj");
        assert_eq!(xtc.handler().type_descr(), "GROMACS XTC trajectory (*.xtc)");
        assert_eq!(xtc.handler().file_ext(), "*.xtc");
        assert_eq!(xtc.handler().category(), InOutHandler::IOH_CAT_OBJREADER);

        let netcdf = AmberNetCDFReader::new();
        assert_eq!(netcdf.handler().name(), "ambnetcdftraj");
        assert_eq!(
            netcdf.handler().type_descr(),
            "AMBER NetCDF trajectory (*.nc)"
        );
        assert_eq!(netcdf.handler().file_ext(), "*.nc");
        assert_eq!(netcdf.handler().category(), InOutHandler::IOH_CAT_OBJREADER);
    }
}
