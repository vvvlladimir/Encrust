use core_edit::Edit;
use core_geometry::{Scalar, Vec3, cuboid, signed_volume, transform_mesh};

use super::*;
use crate::apply::apply;
use crate::fixtures::{SIDE_MM, cube};

const CUBE: ObjectId = ObjectId(1);
const CUBE_MM3: Scalar = SIDE_MM * SIDE_MM * SIDE_MM;

fn on_top(text: &str, raised: bool) -> LabelAsked {
    LabelAsked {
        text: text.to_owned(),
        at_mm: Vec3::new(SIDE_MM / 2.0, SIDE_MM / 2.0, SIDE_MM),
        height_mm: 6.0,
        depth_mm: 1.0,
        raised,
        follow_surface: false,
        angle_deg: 0.0,
        font: None,
    }
}

fn the_cube() -> Mesh {
    cuboid(Vec3::ZERO, Vec3::splat(SIDE_MM))
}

fn written(asked: &LabelAsked) -> Result<Mesh, ApplyError> {
    labelled(&the_cube(), asked, Cancel::never(), CUBE)
}

#[test]
fn a_raised_letter_adds_volume_and_a_sunk_one_takes_it_away() {
    let raised = written(&on_top("I", true)).expect("the letter stands on the top face");
    let sunk = written(&on_top("I", false)).expect("the letter sinks into the top face");

    let (raised, sunk) = (signed_volume(&raised), signed_volume(&sunk));
    assert!(
        raised > CUBE_MM3 + 1.0,
        "a raised letter adds to the {CUBE_MM3} mm³ cube; got {raised}"
    );
    assert!(
        sunk < CUBE_MM3 - 1.0,
        "a sunk letter takes from the {CUBE_MM3} mm³ cube; got {sunk}"
    );
}

#[test]
fn a_label_edit_replaces_the_object_where_it_stands_with_the_lettered_one() {
    let mut document = Document::from_project(cube());
    let edit = Edit::Label {
        object: CUBE,
        text: "I".to_owned(),
        at_mm: Vec3::new(SIDE_MM / 2.0, SIDE_MM / 2.0, SIDE_MM),
        height_mm: Some(6.0),
        depth_mm: Some(1.0),
        raised: true,
        follow_surface: false,
        angle_deg: 0.0,
        font: None,
    };

    let applied = apply(&mut document, &edit, Cancel::never()).expect("the letter fits the top");

    assert_eq!(applied.report, Report::Labelled);
    let [(id, _)] = applied.added[..] else {
        panic!("one object is made, got {:?}", applied.added);
    };
    let (state, meshes) = document.object(id).expect("the result is on the plate");
    let volume = signed_volume(&transform_mesh(&meshes.source, state.transform));
    assert!(
        volume > CUBE_MM3 + 1.0,
        "a raised letter adds to the {CUBE_MM3} mm³ cube; got {volume}"
    );
}

#[test]
fn text_of_spaces_is_blank() {
    assert_eq!(written(&on_top("  ", true)), Err(ApplyError::BlankLabel));
}

#[test]
fn a_model_with_no_faces_has_no_surface_to_write_on() {
    let asked = on_top("I", true);
    let result = labelled(&Mesh::default(), &asked, Cancel::never(), CUBE);
    assert_eq!(result, Err(ApplyError::NoSurface(CUBE)));
}

#[test]
fn a_font_file_that_is_not_there_does_not_read() {
    let mut asked = on_top("I", true);
    asked.font = Some(PathBuf::from("/nowhere/font.ttf"));
    assert!(matches!(
        written(&asked),
        Err(ApplyError::BadFont { path: Some(_), .. })
    ));
}

#[test]
fn bytes_that_are_not_a_font_do_not_read() {
    let mut asked = on_top("I", true);
    asked.font = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"));
    assert!(matches!(
        written(&asked),
        Err(ApplyError::BadFont { path: Some(_), .. })
    ));
}

#[test]
fn a_font_the_text_cannot_be_drawn_in_names_the_file_asked_for() {
    let path = PathBuf::from("/fonts/mine.ttf");
    let error = TextError::BadFont("no outline".to_owned());

    let refused = text_error(error, CUBE, Some(&path));

    assert_eq!(
        refused,
        ApplyError::BadFont {
            path: Some(path),
            reason: "no outline".to_owned()
        }
    );
}
