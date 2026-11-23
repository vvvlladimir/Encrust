use super::*;
use crate::recipe::{Cut, Drains, Hollow, Supports};

const MODEL: ObjectId = ObjectId(3);

fn asked_edits(recipe: &Recipe, earlier: &Recipe, object: ObjectId) -> Option<Vec<Edit>> {
    asked_after_check(recipe, earlier, object, (None, None, None))
}

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
fn a_drain_search_with_channels_and_plugs_is_a_drain_a_channel_each_and_plugs() {
    let recipe = Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            auto: true,
            vent: true,
            plugs: true,
            channels: vec![vec![[0.0, 0.0, 1.0], [5.0, 0.0, 1.0]]],
            ..Drains::default()
        }),
        ..bare()
    };
    let asked = asked_after_check(&recipe, &bare(), MODEL, (None, Some(0.0), None))
        .expect("a search, a channel and plugs are edits");
    assert!(
        matches!(
            asked.as_slice(),
            [
                Edit::Drain { auto: true, vent: true, at_mm, .. },
                Edit::Channel { points_mm, .. },
                Edit::Plugs { .. },
            ] if at_mm.is_empty() && points_mm.len() == 2
        ),
        "{asked:?}"
    );
}

#[test]
fn drains_into_the_supports_or_a_search_taken_back_are_no_edit_yet() {
    let into_supports = Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            support_at_mm: vec![[1.0, 1.0, 0.0]],
            ..Drains::default()
        }),
        ..bare()
    };
    assert_eq!(asked_edits(&into_supports, &bare(), MODEL), None);
    let searched = Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            auto: true,
            ..Drains::default()
        }),
        ..bare()
    };
    let not_searched = Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            ..Drains::default()
        }),
        ..bare()
    };
    assert_eq!(asked_edits(&not_searched, &searched, MODEL), None);
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
fn supports_by_a_preset_or_sized_automatically_are_an_edit() {
    let by = |profile: &str| Recipe {
        supports: Some(Supports {
            profile: profile.into(),
            ..Supports::default()
        }),
        ..bare()
    };
    for profile in ["medium", "auto"] {
        let asked = asked_edits(&by(profile), &bare(), MODEL).expect("supports are an edit");
        assert!(
            matches!(asked.as_slice(), [Edit::Supports { .. }]),
            "{asked:?}"
        );
    }
}

fn searching(pick: Option<u16>, keep_clear_at_mm: Vec<[f32; 3]>) -> Pose {
    Pose::Search {
        goal: crate::recipe::Goal::LeastPeel,
        max_tilt_deg: Some(30.0),
        pick,
        keep_clear_at_mm,
        keep_clear_radius_mm: None,
        keep_clear_seen_deg: None,
    }
}

#[test]
fn a_pose_the_search_picked_is_an_orient_proved_on_the_jobs_printer_and_resin() {
    let recipe = Recipe {
        pose: Some(searching(Some(2), Vec::new())),
        ..bare()
    };

    let area = Some(([5.0, 3.0], [200.0, 120.0]));
    let asked = asked_after_check(&recipe, &bare(), MODEL, (None, None, area))
        .expect("a search is an edit");

    assert_eq!(
        asked,
        vec![Edit::Orient {
            object: MODEL,
            goal: OrientAim::LeastPeel,
            max_tilt_deg: Some(30.0),
            clean_surface: false,
            pick: 1,
            plate_mm: Some([200.0, 120.0]),
            low_mm: [5.0, 3.0],
            printer: Some("p".to_owned()),
            resin: Some("r".to_owned()),
            keep_clear_mm: Vec::new(),
            keep_clear_radius_mm: None,
            keep_clear_seen_deg: None,
        }]
    );
}

#[test]
fn a_search_on_a_printer_file_is_the_turn_its_check_chose() {
    let by_file = Recipe {
        printer: "/m/printer.toml".into(),
        pose: Some(searching(None, Vec::new())),
        ..bare()
    };
    assert_eq!(asked_edits(&by_file, &bare(), MODEL), None);
    let asked = asked_after_check(
        &by_file,
        &bare(),
        MODEL,
        (Some([90.0, 0.0, 0.0]), None, None),
    )
    .expect("the turn chosen is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Rotate { degrees, .. }] if (degrees.x - 90.0).abs() < 1e-6),
        "{asked:?}"
    );
}

#[test]
fn a_search_keeping_a_place_clear_is_an_orient_keeping_it_clear_as_the_check_saw_it() {
    let keeping = Recipe {
        pose: Some(searching(None, vec![[1.0, 2.0, 3.0]])),
        ..bare()
    };

    let asked = asked_after_check(&keeping, &bare(), MODEL, (None, None, None))
        .expect("a search is an edit");

    assert!(
        matches!(
            asked.as_slice(),
            [Edit::Orient { keep_clear_mm, keep_clear_seen_deg: Some(seen), .. }]
                if keep_clear_mm == &[core_geometry::Vec3::new(1.0, 2.0, 3.0)]
                    && *seen == core_geometry::Vec3::ZERO
        ),
        "{asked:?}"
    );
}

#[test]
fn a_plug_gap_is_the_typed_gap_less_the_resins_fit_drift() {
    let drains = |plug_gap_mm| Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            at_mm: vec![[1.0, 2.0, 3.0]],
            plugs: true,
            plug_gap_mm,
            ..Drains::default()
        }),
        ..bare()
    };
    let gap_of = |recipe: &Recipe| {
        let asked = asked_after_check(recipe, &bare(), MODEL, (None, Some(0.05), None))
            .expect("holes and plugs are edits");
        match asked.last() {
            Some(Edit::Plugs { gap_mm, .. }) => *gap_mm,
            other => panic!("plugs last, got {other:?}"),
        }
    };

    assert!(
        (gap_of(&drains(Some(0.2))) - 0.15).abs() < 1e-6,
        "0.2 less 0.05"
    );
    assert!(
        (gap_of(&drains(None)) - 0.10).abs() < 1e-6,
        "0.15 less 0.05"
    );
    assert_eq!(
        asked_after_check(&drains(None), &bare(), MODEL, (None, None, None)),
        None,
        "without the resin's drift a plug is no edit"
    );
}
