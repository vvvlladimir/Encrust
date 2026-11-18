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
