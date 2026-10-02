use std::path::Path;

use core_geometry::{Mapping, Vec2, Vec3};
use core_mesh_io::{MeshIoError, MeshLoader, ObjLoader, Texture, loader_for_extension};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn tetrahedron_loads_with_known_bounds() {
    let mesh = ObjLoader
        .load(&fixture("tetrahedron.obj"))
        .expect("fixture loads")
        .mesh;

    assert_eq!(mesh.faces.len(), 4);
    assert_eq!(
        mesh.vertices.len(),
        4,
        "OBJ shares vertices between faces, so no weld is needed to see it"
    );
    let aabb = mesh.aabb().expect("non-empty mesh");
    assert_eq!(aabb.mins, Vec3::ZERO);
    assert_eq!(aabb.maxs, Vec3::ONE);

    // Unit corner tetrahedron: three legs of area 1/2 plus a face of area sqrt(3)/2.
    let expected = 1.5 + 3f32.sqrt() / 2.0;
    assert!(
        (mesh.surface_area() - expected).abs() < 1e-5,
        "area {} is not the tetrahedron's {expected}",
        mesh.surface_area()
    );
}

#[test]
fn quads_are_triangulated_and_the_material_file_is_ignored() {
    let mesh = ObjLoader
        .load(&fixture("cube_quads.obj"))
        .expect("an absent mtllib does not stop the mesh loading")
        .mesh;

    assert_eq!(mesh.faces.len(), 12, "six quads become twelve triangles");
    assert!(
        (mesh.surface_area() - 6.0).abs() < 1e-5,
        "a unit cube has six faces of unit area"
    );
    let aabb = mesh.aabb().expect("non-empty mesh");
    assert_eq!(aabb.mins, Vec3::ZERO);
    assert_eq!(aabb.maxs, Vec3::ONE);
}

#[test]
fn relative_face_indices_count_back_from_the_last_vertex() {
    let mesh = ObjLoader
        .load(&fixture("relative_indices.obj"))
        .expect("fixture loads")
        .mesh;

    assert_eq!(mesh.faces, vec![[0, 1, 2]]);
    assert!((mesh.surface_area() - 0.5).abs() < 1e-6);
}

#[test]
fn dispatching_by_extension_reaches_the_obj_loader() {
    let loader = loader_for_extension("OBJ").expect("obj is supported");
    assert_eq!(loader.extensions(), &["obj"]);
}

#[test]
fn an_unparsable_vertex_names_the_format() {
    let err = ObjLoader
        .load(&fixture("broken.obj"))
        .expect_err("a vertex of words is not a position");
    assert!(matches!(err, MeshIoError::Malformed { format: "OBJ", .. }));
}

#[test]
fn missing_file_reports_its_path() {
    let err = ObjLoader
        .load(&fixture("absent.obj"))
        .expect_err("the fixture is not there");
    assert!(matches!(err, MeshIoError::Io { .. }));
}

#[test]
fn texture_coordinates_come_out_one_pair_per_corner() {
    let loaded = ObjLoader
        .load(&fixture("textured_quad.obj"))
        .expect("fixture loads");
    let uvs = loaded.uvs.expect("the quad carries vt lines");

    assert_eq!(uvs.len(), 2, "one entry per triangle of the quad");
    assert_eq!(
        uvs.of_face(0),
        Some(Mapping::sole([Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::ONE,])),
        "the first triangle takes vt 1, 2 and 3"
    );

    // The quad spans 2 mm and the whole texture, so its middle is the middle of both.
    let (_, middle) = uvs
        .at(&loaded.mesh, 0, Vec3::new(1.0, 1.0, 0.0))
        .expect("the point is on the face");
    assert!((middle - Vec2::splat(0.5)).length() < 1e-6, "got {middle}");
}

#[test]
fn the_diffuse_map_is_read_from_beside_the_file() {
    let loaded = ObjLoader
        .load(&fixture("textured_quad.obj"))
        .expect("fixture loads");
    let [texture] = loaded.textures.as_slice() else {
        panic!("the material names one map_Kd");
    };

    assert_eq!(texture.name, "relief.png");
    assert_eq!(
        &texture.bytes[..8],
        b"\x89PNG\r\n\x1a\n",
        "the bytes are the image as stored, undecoded"
    );
}

#[test]
fn a_texture_decodes_into_one_height_per_pixel() {
    let loaded = ObjLoader
        .load(&fixture("textured_quad.obj"))
        .expect("fixture loads");
    let heights = loaded.textures[0].decode().expect("the fixture is a PNG");

    assert_eq!((heights.width(), heights.height()), (2, 2));
    // The fixture's top row is black then white; V runs up, so the top row is V near one.
    let at = |u, v| heights.sample(Vec2::new(u, v));
    assert!(at(0.25, 0.75) < 1e-6, "the top left pixel is black");
    assert!(
        (at(0.75, 0.75) - 1.0).abs() < 1e-6,
        "the top right is white"
    );
    let grey = at(0.25, 0.25);
    assert!(
        (grey - 128.0 / 255.0).abs() < 1e-6,
        "got {grey}, expected the 128 of the bottom left pixel"
    );
}

#[test]
fn a_texture_that_is_not_an_image_says_so() {
    let broken = Texture {
        name: "relief.png".to_owned(),
        bytes: b"not an image".to_vec(),
    };
    let Err(error) = broken.decode() else {
        panic!("twelve bytes of text are not an image");
    };
    assert!(matches!(
        error,
        MeshIoError::UndecodableTexture { name, .. } if name == "relief.png"
    ));
}

#[test]
fn a_map_kd_naming_another_machine_is_found_beside_the_model() {
    let loaded = ObjLoader
        .load(&fixture("windows_path.obj"))
        .expect("fixture loads");
    let [texture] = loaded.textures.as_slice() else {
        panic!("the image travels beside the model whatever the material says");
    };

    assert_eq!(texture.name, "relief.png");
    assert_eq!(texture.decode().expect("a PNG").width(), 2);
}

#[test]
fn an_object_without_vt_lines_leaves_its_own_faces_unmapped() {
    let loaded = ObjLoader
        .load(&fixture("partly_textured.obj"))
        .expect("fixture loads");

    let uvs = loaded.uvs.expect("half the file carries vt lines");
    assert_eq!(uvs.len(), loaded.mesh.faces.len(), "one entry per face");
    assert_eq!(uvs.mapped(), 1, "only the first object is textured");
    assert!(uvs.of_face(0).is_some());
    assert!(uvs.of_face(1).is_none());
}

#[test]
fn a_material_each_gives_a_face_an_image_of_its_own() {
    let loaded = ObjLoader
        .load(&fixture("two_materials.obj"))
        .expect("fixture loads");
    let uvs = loaded.uvs.expect("both objects carry vt lines");

    let names: Vec<&str> = loaded
        .textures
        .iter()
        .map(|texture| texture.name.as_str())
        .collect();
    assert_eq!(names, ["relief.png", "white.png"], "one image per material");
    assert_eq!(uvs.textures(), 2);
    assert_eq!(uvs.of_face(0).map(|mapping| mapping.texture), Some(0));
    assert_eq!(uvs.of_face(1).map(|mapping| mapping.texture), Some(1));
}

#[test]
fn a_bitmap_is_decoded_like_any_other_image() {
    let bmp = Texture {
        name: "relief.bmp".to_owned(),
        bytes: std::fs::read(fixture("relief.bmp")).expect("the fixture is there"),
    };
    let heights = bmp.decode().expect("BMP is one of the four formats read");
    assert_eq!((heights.width(), heights.height()), (2, 2));
}

#[test]
fn a_mtllib_line_the_parser_cannot_split_falls_back_to_the_models_own_name() {
    let loaded = ObjLoader
        .load(&fixture("tab_mtllib.obj"))
        .expect("fixture loads");
    assert!(
        !loaded.textures.is_empty(),
        "a tab between the keyword and the name still reaches the material file"
    );
}

#[test]
fn a_file_without_vt_lines_is_not_mapped() {
    let loaded = ObjLoader
        .load(&fixture("tetrahedron.obj"))
        .expect("fixture loads");
    assert!(loaded.uvs.is_none());
    assert!(loaded.textures.is_empty());
}

#[test]
fn a_material_file_that_is_not_there_is_not_an_error() {
    let loaded = ObjLoader
        .load(&fixture("cube_quads.obj"))
        .expect("an absent mtllib does not stop the mesh loading");
    assert_eq!(loaded.mesh.faces.len(), 12);
    assert!(
        loaded.textures.is_empty(),
        "there is no material file to read"
    );
}
