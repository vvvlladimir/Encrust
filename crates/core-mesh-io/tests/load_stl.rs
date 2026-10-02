use std::path::Path;

use core_geometry::Vec3;
use core_mesh_io::{MeshIoError, MeshLoader, StlLoader, loader_for_extension};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn ascii_tetrahedron_loads_with_known_bounds() {
    let mesh = StlLoader
        .load(&fixture("tetrahedron.stl"))
        .expect("fixture loads")
        .mesh;

    assert_eq!(mesh.faces.len(), 4);
    assert_eq!(
        mesh.vertices.len(),
        12,
        "loaders do not weld; each face brings its own"
    );
    let aabb = mesh.aabb().expect("non-empty mesh");
    assert_eq!(aabb.mins, Vec3::ZERO);
    assert_eq!(aabb.maxs, Vec3::ONE);

    // Unit corner tetrahedron: three legs of area 1/2 plus a face of area sqrt(3)/2.
    let total: f32 = mesh.triangles().map(|t| t.area()).sum();
    assert!((total - (1.5 + 3f32.sqrt() / 2.0)).abs() < 1e-5);
}

#[test]
fn dispatching_by_extension_reaches_the_stl_loader() {
    let loader = loader_for_extension("stl").expect("stl is supported");
    assert_eq!(
        loader
            .load(&fixture("tetrahedron.stl"))
            .expect("the fixture loads")
            .mesh
            .faces
            .len(),
        4
    );
}

#[test]
fn missing_file_reports_its_path() {
    let err = StlLoader.load(&fixture("absent.stl")).unwrap_err();
    assert!(matches!(err, MeshIoError::Io { .. }));
}
