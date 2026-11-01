use cuemol_mdtools::{read_amber_prmtop_str, read_amber_restart_str, read_gro_str, read_psf_str};

#[test]
fn gro_rejects_overflowing_atom_count_without_panicking() {
    for count in [usize::MAX, usize::MAX - 1, 1024] {
        assert!(read_gro_str(&format!("invalid\n{count}\n")).is_err());
    }
}

#[test]
fn psf_rejects_overflowing_bond_count_without_panicking() {
    for count in [usize::MAX, usize::MAX / 2, 1024] {
        assert!(read_psf_str(&format!("PSF\n0 !NATOM\n{count} !NBOND\n")).is_err());
    }
}

#[test]
fn amber_rejects_reversed_format_delimiters_without_panicking() {
    assert!(read_amber_prmtop_str("%FLAG POINTERS\n%FORMAT)10I8(\n0\n").is_err());
}

#[test]
fn amber_restart_rejects_overflowing_atom_count_without_panicking() {
    for count in [usize::MAX, usize::MAX / 3, 1024] {
        assert!(read_amber_restart_str(&format!("invalid\n{count}\n")).is_err());
    }
}

#[test]
fn amber_rejects_overflowing_residue_pointer_without_panicking() {
    let mut pointers = [0; 31];
    pointers[0] = 1;
    pointers[11] = 1;
    let mut input = String::from("%FLAG POINTERS\n%FORMAT(1I12)\n");
    for value in pointers {
        input.push_str(&format!("{value:12}\n"));
    }
    input.push_str("%FLAG ATOM_NAME\n%FORMAT(20a4)\nX   \n%FLAG RESIDUE_LABEL\n%FORMAT(20a4)\nUNK \n%FLAG RESIDUE_POINTER\n%FORMAT(1I12)\n");
    input.push_str(&format!("{:12}\n", i32::MIN));
    assert!(read_amber_prmtop_str(&input).is_err());
}

#[test]
fn psf_rejects_invalid_optional_bond_count() {
    for count in ["-1", "invalid", "184467440737095516160"] {
        assert!(read_psf_str(&format!("PSF\n0 !NATOM\n{count} !NBOND\n")).is_err());
    }
}
