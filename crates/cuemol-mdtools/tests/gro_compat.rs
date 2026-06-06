use cuemol_mdtools::{read_gro_frame_str, read_gro_str};

#[test]
fn gro_reader_converts_nm_to_angstrom_and_reads_cell() {
    let input = "\
water
    3
    1SOL     OW    1   0.100   0.200   0.300
    1SOL    HW1    2   0.110   0.200   0.300
    1SOL    HW2    3   0.100   0.210   0.300
   2.00000   2.00000   2.00000
";
    let frame = read_gro_frame_str(input).unwrap();
    let mol = frame.molecule;
    assert_eq!(mol.atom_count(), 3);
    assert_eq!(mol.atoms()[0].chain_id, "A");
    assert_eq!(mol.atoms()[0].res_name, "SOL");
    assert_eq!(mol.atoms()[0].element, "O");
    assert!((mol.atoms()[0].position.x - 1.0).abs() < 1.0e-6);
    assert!((mol.atoms()[0].position.y - 2.0).abs() < 1.0e-6);
    assert!((mol.atoms()[0].position.z - 3.0).abs() < 1.0e-6);
    assert_eq!(frame.cell.unwrap().a, 20.0);
}

#[test]
fn gro_reader_handles_velocity_and_high_precision_fields() {
    let with_velocity = "\
water with velocity
    1
    1SOL     OW    1   0.100   0.200   0.300  0.0100  0.0200  0.0300
   2.00000   2.00000   2.00000
";
    let mol = read_gro_str(with_velocity).unwrap();
    assert!((mol.atoms()[0].position.x - 1.0).abs() < 1.0e-6);

    let high_precision = "\
high precision
    1
    1SOL     OW    1   0.1234567   0.2345678   0.3456789
   2.0000000   2.0000000   2.0000000
";
    let mol = read_gro_str(high_precision).unwrap();
    assert!((mol.atoms()[0].position.x - 1.234567).abs() < 1.0e-6);
}
