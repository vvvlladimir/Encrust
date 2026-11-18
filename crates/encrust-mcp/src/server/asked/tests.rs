use super::*;
use crate::recipe::{Cut, Hollow};

const MODEL: ObjectId = ObjectId(3);

fn bare() -> Recipe {
    Recipe::bare("/m/cube.stl".into(), "p".into(), "r".into())
}

fn with_modelling(modelling: Modelling) -> Recipe {
    Recipe {
        modelling: Some(modelling),
        ..bare()
    }
}

#[test]
fn a_revision_changing_nothing_on_the_plate_asks_no_edit() {
    let earlier = bare();
    let recipe = Recipe {
        printer: "another".into(),
        ..bare()
    };
    assert_eq!(asked_edits(&recipe, &earlier, MODEL), Some(Vec::new()));
}

#[test]
fn a_turn_typed_is_a_rotate_of_the_jobs_model() {
    let recipe = Recipe {
        pose: Some(Pose::Rotate {
            degrees: [0.0, 0.0, 90.0],
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a turn is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Rotate { object, degrees }] if *object == MODEL && (degrees.z - 90.0).abs() < 1e-6),
        "{asked:?}"
    );
}

#[test]
fn a_scale_and_a_cut_asked_at_once_are_made_scale_first() {
    let recipe = with_modelling(Modelling {
        scale: Some([2.0, 2.0, 2.0]),
        cut: Some(
            serde_json::from_value::<Cut>(serde_json::json!({"axis": "z", "at_mm": 10.0}))
                .expect("a cut reads from JSON"),
        ),
        ..Modelling::default()
    });
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("both are edits");
    assert!(
        matches!(asked.as_slice(), [Edit::Scale { .. }, Edit::Cut { .. }]),
        "{asked:?}"
    );
}

#[test]
fn a_full_repair_reads_as_the_windows_repair() {
    let recipe = Recipe {
        repair: Some(Repair::Full),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a repair is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Repair { .. }]),
        "{asked:?}"
    );
}

#[test]
fn a_hollow_asked_is_no_edit_yet_and_the_revision_is_shown_as_before() {
    let recipe = Recipe {
        hollow: Some(Hollow {
            wall_mm: 2.0,
            ..Hollow::default()
        }),
        ..bare()
    };
    assert_eq!(asked_edits(&recipe, &bare(), MODEL), None);
}

#[test]
fn a_scale_taken_back_is_no_edit_and_leaves_the_revision_to_be_shown() {
    let earlier = with_modelling(Modelling {
        scale: Some([2.0, 2.0, 2.0]),
        ..Modelling::default()
    });
    assert_eq!(asked_edits(&bare(), &earlier, MODEL), None);
}
