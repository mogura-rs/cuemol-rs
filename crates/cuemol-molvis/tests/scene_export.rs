//! End-to-end oracle for the headless scene-export pipeline:
//! molecule -> `molecule_render_snapshot` -> `tessellate_snapshot` -> mesh/OBJ.
//!
//! Verification uses INDEPENDENT invariants (not a writer/reader round-trip):
//! the per-primitive triangle counts, and a world-space bounding-box match
//! between the molecule's atom coordinates and the exported mesh vertices —
//! which would fail if any camera/model transform (or the `w=0` point-transform
//! trap) leaked into the export path.
//!
//! Self-skips when the C++ reference fixture tree is absent (compat-test style).

use cuemol_molstr::read_pdb_file;
use cuemol_molvis::{molecule_render_snapshot, MoleculeRenderOptions};
use cuemol_render::tessellate_snapshot;

const SPHERE_TRIS: usize = 8 * 12 * 2; // RINGS * SEGS * 2
const CYL_TRIS: usize = 10 * 2; // SEGS * 2

fn ref_data_path(name: &str) -> std::path::PathBuf {
    let base = std::env::var("CUEMOL2_REF_DIR").unwrap_or_else(|_| "../cuemol2".to_string());
    std::path::Path::new(&base)
        .join("tests/test_data")
        .join(name)
}

#[test]
fn exports_1crn_snapshot_to_a_world_space_triangle_mesh() {
    let path = ref_data_path("1CRN.pdb");
    if !path.exists() {
        eprintln!("skip: {} absent", path.display());
        return;
    }

    let mol = read_pdb_file(&path).expect("read 1CRN.pdb");
    let snapshot = molecule_render_snapshot(&mol, &MoleculeRenderOptions::default());

    // Default options draw every atom and bond, no ribbon/surface meshes.
    assert_eq!(snapshot.atom_count(), mol.atom_count());
    assert!(snapshot.meshes.is_empty());
    assert!(snapshot.atom_count() > 300, "1CRN has 327 atoms");
    assert!(snapshot.bond_count() > 300, "1CRN is fully bonded");

    let mesh = tessellate_snapshot(&snapshot);

    // The tessellation scales linearly: one sphere per atom, one cylinder per
    // bond (no extra meshes here).
    assert_eq!(
        mesh.triangle_count(),
        snapshot.atom_count() * SPHERE_TRIS + snapshot.bond_count() * CYL_TRIS
    );

    // OBJ serialization is 1:1 with the mesh.
    let obj = mesh.to_obj_string("1crn");
    assert_eq!(
        obj.lines().filter(|l| l.starts_with("v ")).count(),
        mesh.vertices.len()
    );
    assert_eq!(
        obj.lines().filter(|l| l.starts_with("f ")).count(),
        mesh.triangle_count()
    );

    // World-space oracle: the mesh occupies the molecule's coordinate range,
    // expanded by at most the largest drawn radius. (Atom bbox is contained in
    // the mesh bbox; the mesh bbox does not exceed the atom bbox by more than
    // the max sphere/cylinder radius.) This pins that no transform was applied.
    let (amin, amax) = atom_bounds(&mol);
    let (mmin, mmax) = mesh_bounds(&mesh);
    // Derive the bound from the ACTUAL max drawn sphere radius (atom radii may
    // be element-scaled), so the oracle is correct by construction, not by
    // coincidence on this molecule's element set.
    let max_radius = snapshot
        .atoms
        .iter()
        .map(|a| a.radius as f64)
        .fold(0.0_f64, f64::max)
        + 1.0e-3;
    for axis in 0..3 {
        // Atom centres are inside the mesh extent.
        assert!(mmin[axis] <= amin[axis] + 1.0e-6, "axis {axis} min");
        assert!(mmax[axis] >= amax[axis] - 1.0e-6, "axis {axis} max");
        // The mesh does not balloon beyond the atoms by more than a radius.
        assert!(
            amin[axis] - mmin[axis] <= max_radius + 1.0e-6,
            "axis {axis} under"
        );
        assert!(
            mmax[axis] - amax[axis] <= max_radius + 1.0e-6,
            "axis {axis} over"
        );
    }
}

fn atom_bounds(mol: &cuemol_molstr::MolCoord) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for atom in mol.atoms() {
        let p = [atom.position.x, atom.position.y, atom.position.z];
        for axis in 0..3 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
    (min, max)
}

fn mesh_bounds(mesh: &cuemol_render::RenderMesh) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for v in &mesh.vertices {
        let p = [v.position.x, v.position.y, v.position.z];
        for axis in 0..3 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
    (min, max)
}
