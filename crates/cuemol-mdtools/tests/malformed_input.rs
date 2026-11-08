use cuemol_mdtools::{read_amber_prmtop_str, read_amber_restart_str, read_gro_str, read_psf_str};

#[test]
fn xdr_float_array_rejects_overflowing_count_without_panicking() {
    let mut input = cuemol_mdtools::XdrInStream::new(&[0; 4]);
    assert!(input.read_f32_array(usize::MAX).is_err());
}

#[test]
fn xdr_double_array_rejects_overflowing_count_without_panicking() {
    let mut input = cuemol_mdtools::XdrInStream::new(&[0; 8]);
    assert!(input.read_f64_array_as_f32(usize::MAX).is_err());
}

#[test]
fn xtc_coordinates_reject_overflowing_count_without_panicking() {
    let mut bytes = Vec::new();
    bytes.extend(1.0_f32.to_be_bytes());
    for value in [0_i32, 0, 0, 0, 0, 0, 9, 1] {
        bytes.extend(value.to_be_bytes());
    }
    bytes.extend([0; 4]);
    let mut input = cuemol_mdtools::XdrInStream::new(&bytes);
    assert!(input
        .read_compressed_coordinates(usize::MAX, false)
        .is_err());
}

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
