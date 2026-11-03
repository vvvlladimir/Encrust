use std::path::Path;

use core_geometry::{Mapping, Vec2, Vec3, signed_volume};
use core_mesh_io::{MeshIoError, MeshLoader, ModelFile, ThreeMfLoader, loader_for_extension};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn a_3mf_in_memory_reads_as_it_does_from_disk() {
    let bytes = std::fs::read(fixture("tetrahedron.3mf")).expect("the fixture is there");
    let mesh = ThreeMfLoader
        .read(ModelFile {
            path: Path::new("tetrahedron.3mf"),
            source: &mut std::io::Cursor::new(bytes),
            beside: &|_| None,
        })
        .expect("the bytes are the fixture")
        .mesh;

    // The same unit corner tetrahedron the file on disk is.
    assert_eq!(mesh.faces.len(), 4);
    assert_eq!(mesh.aabb().map(|aabb| aabb.maxs), Some(Vec3::ONE));
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

/// The fuzzer's finds: the second carries a data descriptor on another entry, which makes
/// the archive's total size unknowable while each part still states its own; the third
/// claims exactly two gigabytes, the most a part may hold but far more than a kilobyte inflates to.
#[test]
fn a_part_claiming_more_bytes_than_fit_in_memory_is_refused_rather_than_reserved_for() {
    for name in [
        "crashes/oversized_part.3mf",
        "crashes/oversized_part_behind_a_data_descriptor.3mf",
        "crashes/part_claiming_two_gigabytes.3mf",
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

#[test]
fn a_build_item_reaches_an_object_in_another_model_part() {
    let mesh = ThreeMfLoader
        .load(&fixture("production.3mf"))
        .expect("the root is the part the relationships name")
        .mesh;

    // A unit cube kept in /3D/parts/cube.model, named /3D/Parts/Cube.model: part names
    // compare without case. The item moves it five millimetres along x.
    assert_eq!(mesh.faces.len(), 12);
    let aabb = mesh.aabb().expect("non-empty mesh");
    assert_eq!(aabb.mins, Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(aabb.maxs, Vec3::new(6.0, 1.0, 1.0));
}

#[test]
fn a_component_outside_the_root_naming_another_part_is_malformed() {
    let err = ThreeMfLoader
        .load(&fixture("component_path_outside_root.3mf"))
        .expect_err("only the root model may name another part");
    let MeshIoError::Malformed { format, reason, .. } = err else {
        panic!("a second level of parts is malformed, not something else");
    };
    assert_eq!(format, "3MF");
    assert!(reason.contains("outside the root"), "got {reason}");
}

#[test]
fn a_triangle_naming_only_p1_maps_every_corner_to_it() {
    let loaded = ThreeMfLoader
        .load(&fixture("one_property_per_triangle.3mf"))
        .expect("fixture loads");

    let uvs = loaded.uvs.expect("the triangle names the texture group");
    let p1 = Vec2::new(0.25, 0.75);
    assert_eq!(uvs.of_face(0), Some(Mapping::sole([p1, p1, p1])));
    assert_eq!(loaded.textures.len(), 1);
}

#[test]
fn a_boolean_shape_is_refused_rather_than_dropped() {
    let err = ThreeMfLoader
        .load(&fixture("boolean_shape.3mf"))
        .expect_err("the build places a boolean shape");
    assert!(
        matches!(err, MeshIoError::Unimplemented(_)),
        "an empty mesh would be a silent loss, got {err:?}"
    );
}

#[test]
fn a_vertex_at_nan_is_malformed() {
    let err = ThreeMfLoader
        .load(&fixture("vertex_not_a_number.3mf"))
        .expect_err("NaN is no coordinate");
    let MeshIoError::Malformed { reason, .. } = err else {
        panic!("a number that is none is malformed, not something else");
    };
    assert!(reason.contains("vertex"), "got {reason}");
}

#[test]
fn an_object_inside_an_unknown_namespace_is_skipped() {
    let mesh = ThreeMfLoader
        .load(&fixture("foreign_namespace_object.3mf"))
        .expect("fixture loads")
        .mesh;

    // The core tetrahedron, not the one-triangle object an unknown extension wraps.
    assert_eq!(mesh.faces.len(), 4);
}

#[test]
fn the_first_of_two_objects_sharing_an_id_is_placed() {
    let mesh = ThreeMfLoader
        .load(&fixture("duplicate_object_id.3mf"))
        .expect("fixture loads")
        .mesh;

    assert_eq!(mesh.faces.len(), 4, "the tetrahedron comes first");
}

#[test]
fn a_component_path_with_escapes_reaches_its_part() {
    let mesh = ThreeMfLoader
        .load(&fixture("percent_escaped_part.3mf"))
        .expect("/3D/My%20Part.model is the entry 3D/My Part.model")
        .mesh;

    assert_eq!(mesh.faces.len(), 4);
}

#[test]
fn a_triangle_naming_no_property_takes_its_objects_pindex() {
    let loaded = ThreeMfLoader
        .load(&fixture("object_pindex.3mf"))
        .expect("fixture loads");

    let uvs = loaded.uvs.expect("the object names the texture group");
    let pindex = Vec2::new(0.5, 0.25);
    assert_eq!(uvs.of_face(3), Some(Mapping::sole([pindex; 3])));
}

#[test]
fn a_texture_failing_its_checksum_is_malformed() {
    let err = ThreeMfLoader
        .load(&fixture("corrupt_texture.3mf"))
        .expect_err("the image bytes do not match their CRC");
    let MeshIoError::Malformed { reason, .. } = err else {
        panic!("a damaged part is malformed, not something else, got {err:?}");
    };
    assert!(reason.contains("white.png"), "got {reason}");
}
