use cuemol_mdtools::{TrajBlock, Trajectory};
use cuemol_molstr::read_pdb_str;

#[test]
fn oversized_average_window_covers_available_frames_without_overflow() {
    let molecule = read_pdb_str(
        "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n",
    )
    .unwrap();
    let mut block = TrajBlock::new(1);
    for x in [0., 3., 6.] {
        block.append_frame(vec![x, 0., 0.], None).unwrap();
    }
    let mut trajectory = Trajectory::new(molecule);
    trajectory.append(block).unwrap();
    trajectory.set_frame_average_size(usize::MAX);
    for index in 0..3 {
        trajectory.set_frame(index).unwrap();
        assert_eq!(trajectory.frame(), index);
        assert_eq!(trajectory.atoms()[0].position.x, 3.);
    }
}
