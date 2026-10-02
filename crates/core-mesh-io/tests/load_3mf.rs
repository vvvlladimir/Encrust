use std::path::Path;

use core_geometry::{Mapping, Vec2, Vec3, signed_volume};
use core_mesh_io::{MeshIoError, MeshLoader, ThreeMfLoader, loader_for_extension};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn tetrahedron_loads_with_known_bounds() {
    let mesh = ThreeMfLoader
        .load(&fixture("tetrahedron.3mf"))
        .expect("fixture loads")
        .mesh;

    assert_eq!(mesh.faces.len(), 4);
    assert_eq!(mesh.vertices.len(), 4, "3MF shares vertices between faces");
    let aabb = mesh.aabb().expect("non-empty mesh");
    assert_eq!(aabb.mins, Vec3::ZERO);
    assert_eq!(aabb.maxs, Vec3::ONE);

    // Unit corner tetrahedron: three legs of area 1/2 plus a face of area sqrt(3)/2.
    let expected = 1.5 + 3f32.sqrt() / 2.0;
    assert!((mesh.surface_area() - expected).abs() < 1e-5);
}

#[test]
fn two_build_items_land_where_the_file_puts_them() {
    let mesh = ThreeMfLoader
        .load(&fixture("two_items.3mf"))
        .expect("fixture loads")
        .mesh;

    assert_eq!(mesh.faces.len(), 24, "one cube per build item");
    let aabb = mesh.aabb().expect("non-empty mesh");
    assert_eq!(aabb.mins, Vec3::ZERO);
    assert_eq!(
        aabb.maxs,
        Vec3::new(11.0, 1.0, 1.0),
        "the second item is translated ten millimetres along x"
    );
}

#[test]
fn nested_components_compose_outermost_last() {
    let mesh = ThreeMfLoader
        .load(&fixture("nested.3mf"))
        .expect("fixture loads")
        .mesh;

    // The cube is turned a quarter about Z into x in -1..0, then translated by x=10 and
    // by z=5. Composing the other way round would put it somewhere else entirely.
    let aabb = mesh.aabb().expect("non-empty mesh");
    assert!(
        (aabb.mins - Vec3::new(9.0, 0.0, 5.0)).length() < 1e-5,
        "mins {} are not the composed placement",
        aabb.mins
    );
    assert!((aabb.maxs - Vec3::new(10.0, 1.0, 6.0)).length() < 1e-5);
    assert!(
        (mesh.surface_area() - 6.0).abs() < 1e-5,
        "a turn keeps area"
    );
}

#[test]
fn inches_are_scaled_into_millimetres() {
    let mesh = ThreeMfLoader
        .load(&fixture("inches.3mf"))
        .expect("fixture loads")
        .mesh;

    let aabb = mesh.aabb().expect("non-empty mesh");
    assert!(
        (aabb.maxs - Vec3::splat(25.4)).length() < 1e-4,
        "an inch is 25.4 mm, got {}",
        aabb.maxs
    );
}

#[test]
fn a_mirroring_placement_keeps_the_solid_the_right_way_out() {
    let mesh = ThreeMfLoader
        .load(&fixture("mirrored.3mf"))
        .expect("fixture loads")
        .mesh;

    let aabb = mesh.aabb().expect("non-empty mesh");
    assert_eq!(aabb.mins, Vec3::new(-1.0, 0.0, 0.0));
    assert!(
        (signed_volume(&mesh) - 1.0).abs() < 1e-5,
        "a unit cube mirrored once still encloses +1 mm^3, got {}",
        signed_volume(&mesh)
    );
}

#[test]
fn a_build_item_naming_no_object_is_malformed() {
    let err = ThreeMfLoader
        .load(&fixture("missing_object.3mf"))
        .expect_err("object 99 is not in the file");
    let MeshIoError::Malformed { format, reason, .. } = err else {
        panic!("a dangling objectid is malformed, not something else");
    };
    assert_eq!(format, "3MF");
    assert!(reason.contains("99"), "got {reason}");
}

#[test]
fn dispatching_by_extension_reaches_the_3mf_loader() {
    let loader = loader_for_extension("3MF").expect("3mf is supported");
    assert_eq!(loader.extensions(), &["3mf"]);
}

#[test]
fn missing_file_reports_its_path() {
    let err = ThreeMfLoader
        .load(&fixture("absent.3mf"))
        .expect_err("the fixture is not there");
    assert!(matches!(err, MeshIoError::Io { .. }));
}

#[test]
fn a_texture2dgroup_becomes_the_map_and_the_image_beside_it() {
    let loaded = ThreeMfLoader
        .load(&fixture("textured.3mf"))
        .expect("fixture loads");

    let uvs = loaded.uvs.expect("every triangle names the texture group");
    assert_eq!(uvs.len(), loaded.mesh.faces.len());
    assert_eq!(
        uvs.of_face(0),
        Some(Mapping::sole([
            Vec2::ZERO,
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 0.0),
        ])),
        "the corners follow the triangle's own p1, p2 and p3"
    );

    let [texture] = loaded.textures.as_slice() else {
        panic!("the file names one image and the package carries it");
    };
    assert_eq!(texture.name, "/3D/Textures/relief.png");
    let heights = texture.decode().expect("the part is a PNG");
    assert_eq!((heights.width(), heights.height()), (2, 2));
}

#[test]
fn a_file_without_the_materials_extension_carries_no_map() {
    let loaded = ThreeMfLoader
        .load(&fixture("tetrahedron.3mf"))
        .expect("fixture loads");
    assert!(loaded.uvs.is_none());
    assert!(loaded.textures.is_empty());
}

/// Both fuzzer finds: the second carries a data descriptor on another entry, which makes
/// the archive's total size unknowable while each part still states its own.
#[test]
fn a_part_claiming_more_bytes_than_fit_in_memory_is_refused_rather_than_reserved_for() {
    for name in [
        "crashes/oversized_part.3mf",
        "crashes/oversized_part_behind_a_data_descriptor.3mf",
    ] {
        let err = ThreeMfLoader
            .load(&fixture(name))
            .expect_err("a kilobyte of zip claiming four gigabytes is not a model");
        let MeshIoError::Malformed { format, reason, .. } = err else {
            panic!("{name}: a bomb is a malformed file, not an I/O fault");
        };
        assert_eq!(format, "3MF");
        assert!(reason.contains("claims"), "{name}: {reason}");
    }
}
