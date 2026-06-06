pub mod amber;
mod fileio;
pub mod gro;
pub mod namd;
pub mod psf;

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
pub use psf::{read_psf_file, read_psf_str, sniff_psf_content, PsfReader};

pub type GROFileReader = GroReader;
pub type NAMDCoorReader = NamdCoorReader;
pub type PSFReader = PsfReader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdCoordinateFormat {
    Gro,
    NamdCoor,
    AmberCrd,
    AmberMdcrd,
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
    }
}
