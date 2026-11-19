use super::*;
use crate::recipe::{Cut, Drains, Hollow, Supports};

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
fn a_hollow_asked_is_the_windows_hollow_by_its_numbers() {
    let recipe = Recipe {
        hollow: Some(Hollow {
            wall_mm: 2.0,
            ..Hollow::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a hollow is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Hollow { settings, .. }] if (settings.thickness_mm - 2.0).abs() < 1e-6),
        "{asked:?}"
    );
}

#[test]
fn walls_kept_solid_at_points_are_no_edit_yet_and_the_revision_is_shown_as_before() {
    let recipe = Recipe {
        hollow: Some(Hollow {
            wall_mm: 2.0,
            keep_solid_at_mm: vec![[1.0, 1.0, 1.0]],
            ..Hollow::default()
        }),
        ..bare()
    };
    assert_eq!(asked_edits(&recipe, &bare(), MODEL), None);
}

#[test]
fn a_second_drain_point_drills_that_hole_alone() {
    let drains = |at_mm: Vec<[f32; 3]>| Drains {
        diameter_mm: 3.0,
        vent: true,
        at_mm,
        ..Drains::default()
    };
    let earlier = Recipe {
        drains: Some(drains(vec![[1.0, 1.0, 1.0]])),
        ..bare()
    };
    let recipe = Recipe {
        drains: Some(drains(vec![[1.0, 1.0, 1.0], [2.0, 2.0, 2.0]])),
        ..bare()
    };
    let asked = asked_edits(&recipe, &earlier, MODEL).expect("a point added is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Drain { at_mm, .. }] if at_mm.len() == 1 && (at_mm[0].x - 2.0).abs() < 1e-6),
        "{asked:?}"
    );
}

#[test]
fn drains_the_engine_searches_for_are_no_edit_yet() {
    let recipe = Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            auto: true,
            ..Drains::default()
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

#[test]
fn supports_by_a_preset_are_an_edit_and_the_automatic_sizing_is_not_yet() {
    let by = |profile: &str| Recipe {
        supports: Some(Supports {
            profile: profile.into(),
            ..Supports::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&by("medium"), &bare(), MODEL).expect("a preset is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Supports { .. }]),
        "{asked:?}"
    );
    assert_eq!(asked_edits(&by("auto"), &bare(), MODEL), None);
}
