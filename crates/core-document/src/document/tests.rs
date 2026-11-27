use super::*;
use crate::fixtures::cube;

#[test]
fn a_plate_read_from_before_ids_finds_its_object_by_the_id_it_was_given() {
    let document = Document::from_project(cube());

    let (object, _) = document
        .object(ObjectId(1))
        .expect("the one object is given the first id");
    assert_eq!(object.name, "cube");
    assert!(document.object(ObjectId(2)).is_none());
}

#[test]
fn models_become_one_object_each_with_ids_from_one() {
    let mesh = |side_mm| {
        core_geometry::cuboid(
            core_geometry::Vec3::ZERO,
            core_geometry::Vec3::splat(side_mm),
        )
    };
    let template = cube().manifest.objects[0].summary.clone();
    let model = |name: &str, side_mm| ImportedModel {
        name: name.to_owned(),
        mesh: mesh(side_mm),
        summary: template.clone(),
    };

    let document = Document::of_models(
        Manifest::new(None, None),
        vec![model("small", 10.0), model("large", 20.0)],
    );

    let (small, _) = document.object(ObjectId(1)).expect("the first model is 1");
    assert_eq!(small.name, "small");
    let (large, meshes) = document.object(ObjectId(2)).expect("the second model is 2");
    assert_eq!(large.name, "large");
    let volume = core_geometry::signed_volume(&meshes.source);
    assert!(
        (volume - 8000.0).abs() < 0.01,
        "the 20 mm cube is {volume} mm3, expected 8000"
    );
}

#[test]
fn an_id_deleted_is_not_given_again_after_the_plate_is_saved_and_read() {
    use core_edit::Edit;
    use core_engine::project::{read_from, write_to};
    use core_volume::Cancel;

    let mut document = Document::from_project(cube());
    let copy = |document: &mut Document| {
        let edit = Edit::Duplicate {
            object: ObjectId(1),
            offset_mm: core_geometry::Vec3::X * 20.0,
        };
        let applied = crate::apply(document, &edit, Cancel::never()).expect("the cube copies");
        applied.added[0].0
    };
    assert_eq!(copy(&mut document), ObjectId(2));
    let delete = Edit::Delete {
        object: ObjectId(2),
    };
    crate::apply(&mut document, &delete, Cancel::never()).expect("the copy is on the plate");

    let mut file = std::io::Cursor::new(Vec::new());
    write_to(&mut file, &document.into_project()).expect("the plate writes");
    file.set_position(0);
    let mut reopened = Document::from_project(read_from(file).expect("the plate reads back"));

    assert_eq!(
        copy(&mut reopened),
        ObjectId(3),
        "id 2 was taken once already"
    );
}
