use super::*;
use crate::recipe::{Cut, Drains, HoleKind, Hollow, Mend, Relief, Supports};

const MODEL: ObjectId = ObjectId(3);

fn asked_edits(recipe: &Recipe, earlier: &Recipe, object: ObjectId) -> Option<Vec<Edit>> {
    asked_after_check(recipe, earlier, object, (None, None, None, (false, false)))
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
fn a_repairs_limits_given_as_a_mend_reach_the_repair() {
    let recipe = Recipe {
        repair: Some(Repair::Full),
        mend: Some(Mend {
            seam_tolerance_mm: Some(0.2),
            smallest_shell_mm2: Some(3.0),
            ..Mend::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a repair is an edit");
    assert!(
        matches!(
            asked.as_slice(),
            [Edit::Repair { seam_tolerance_mm: Some(seam), smallest_shell_mm2: Some(shell), .. }]
                if (seam - 0.2).abs() < 1e-6 && (shell - 3.0).abs() < 1e-6
        ),
        "the limits go with the repair and no empty mend is asked: {asked:?}"
    );
}

#[test]
fn a_repairs_limits_changed_alone_ask_the_repair_again() {
    let earlier = Recipe {
        repair: Some(Repair::Full),
        ..bare()
    };
    let recipe = Recipe {
        mend: Some(Mend {
            seam_tolerance_mm: Some(0.2),
            ..Mend::default()
        }),
        ..earlier.clone()
    };
    let asked = asked_edits(&recipe, &earlier, MODEL).expect("a repair is an edit");
    assert!(
        matches!(
            asked.as_slice(),
            [Edit::Repair {
                seam_tolerance_mm: Some(_),
                ..
            }]
        ),
        "{asked:?}"
    );
}

#[test]
fn a_bridge_asked_is_mended_before_the_repair() {
    let recipe = Recipe {
        repair: Some(Repair::Full),
        mend: Some(Mend {
            bridge: vec!["1-2:5-6".into()],
            ..Mend::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a bridge is an edit");
    assert!(
        matches!(
            asked.as_slice(),
            [Edit::Mend { bridge, .. }, Edit::Repair { .. }] if *bridge == [[[1, 2], [5, 6]]]
        ),
        "the faces are numbered as imported, so the mend goes first: {asked:?}"
    );
}

#[test]
fn a_bridge_not_written_as_two_edges_is_no_edit() {
    let recipe = Recipe {
        mend: Some(Mend {
            bridge: vec!["1-2".into()],
            ..Mend::default()
        }),
        ..bare()
    };
    assert_eq!(asked_edits(&recipe, &bare(), MODEL), None);
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
fn walls_kept_solid_at_points_are_the_hollows_blockers() {
    let recipe = Recipe {
        hollow: Some(Hollow {
            wall_mm: 2.0,
            keep_solid_at_mm: vec![[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]],
            keep_solid_reach_mm: Some(3.0),
            ..Hollow::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a wall kept solid is an edit");
    assert!(
        matches!(
            asked.as_slice(),
            [Edit::Hollow { keep_solid, keep_solid_reach_mm: Some(reach), .. }]
                if keep_solid.len() == 2 && keep_solid[0].len() == 1 && (reach - 3.0).abs() < 1e-6
        ),
        "each point is a blocker of its own, as `--keep-solid` takes one: {asked:?}"
    );
}

#[test]
fn walled_holes_are_drilled_before_the_hollow_they_shape() {
    let earlier = Recipe {
        hollow: Some(Hollow {
            wall_mm: 2.0,
            ..Hollow::default()
        }),
        ..bare()
    };
    let recipe = Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            at_mm: vec![[1.0, 1.0, 1.0]],
            kind: Some(HoleKind::Walled),
            ..Drains::default()
        }),
        ..earlier.clone()
    };
    let asked = asked_edits(&recipe, &earlier, MODEL).expect("a walled hole is an edit");
    assert!(
        matches!(
            asked.as_slice(),
            [
                Edit::Drain {
                    kind: core_edit::HoleKind::Walled,
                    ..
                },
                Edit::Hollow { .. }
            ]
        ),
        "the hollow is grown again round the hole: {asked:?}"
    );
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
    let asked = asked_after_check(
        &recipe,
        &bare(),
        MODEL,
        (None, Some(0.0), None, (false, false)),
    )
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
fn holes_into_the_raft_are_drilled_once_the_supports_stand() {
    let recipe = Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            support_at_mm: vec![[1.0, 1.0, 0.0]],
            ..Drains::default()
        }),
        supports: Some(Supports {
            profile: "medium".into(),
            ..Supports::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a hole into the raft is an edit");
    assert!(
        matches!(
            asked.as_slice(),
            [
                Edit::Supports { .. },
                Edit::Drain {
                    kind: core_edit::HoleKind::Support,
                    ..
                }
            ]
        ),
        "{asked:?}"
    );
}

#[test]
fn a_search_taken_back_is_no_edit_yet() {
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
fn a_merge_added_joins_those_supports_alone() {
    let earlier = Recipe {
        supports: Some(Supports {
            profile: "medium".into(),
            ..Supports::default()
        }),
        ..bare()
    };
    let group = vec![[1.0, 1.0, 0.0], [3.0, 1.0, 0.0]];
    let recipe = Recipe {
        supports: Some(Supports {
            profile: "medium".into(),
            merge_at_mm: vec![group],
            ..Supports::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &earlier, MODEL).expect("a merge is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::MergeSupports { near_mm, .. }] if near_mm.len() == 2),
        "the supports standing are not grown again: {asked:?}"
    );
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
        clean_surface: false,
        keep_clear_at_mm,
        keep_clear_radius_mm: None,
        keep_clear_seen_deg: None,
        stood_deg: None,
    }
}

#[test]
fn a_pose_the_search_picked_is_an_orient_proved_on_the_jobs_printer_and_resin() {
    let recipe = Recipe {
        pose: Some(searching(Some(2), Vec::new())),
        ..bare()
    };

    let area = Some(([5.0, 3.0], [200.0, 120.0]));
    let asked = asked_after_check(&recipe, &bare(), MODEL, (None, None, area, (false, false)))
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
            lift_mm: None,
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
        (Some([90.0, 0.0, 0.0]), None, None, (false, false)),
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

    let asked = asked_after_check(&keeping, &bare(), MODEL, (None, None, None, (false, false)))
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
        let asked = asked_after_check(
            recipe,
            &bare(),
            MODEL,
            (None, Some(0.05), None, (false, false)),
        )
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
        asked_after_check(
            &drains(None),
            &bare(),
            MODEL,
            (None, None, None, (false, false))
        ),
        None,
        "without the resin's drift a plug is no edit"
    );
}

fn pressed() -> Relief {
    Relief {
        depth_mm: 0.5,
        image: Some("/pictures/bark.png".into()),
        tile_mm: None,
        tile_angle_deg: None,
        invert: false,
        symmetric: false,
        smooth_px: None,
        hold_bottom_deg: None,
        hold_top_deg: None,
    }
}

#[test]
fn a_relief_and_a_turn_asked_at_once_are_pressed_on_the_turned_model() {
    let recipe = Recipe {
        relief: Some(pressed()),
        pose: Some(Pose::Rotate {
            degrees: [90.0, 0.0, 0.0],
        }),
        modelling: Some(Modelling {
            scale: Some([2.0, 2.0, 2.0]),
            ..Modelling::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("each is an edit");
    assert!(
        matches!(
            asked.as_slice(),
            [Edit::Scale { .. }, Edit::Rotate { .. }, Edit::Relief { .. }]
        ),
        "scaled, turned, then pressed, as the command line does: {asked:?}"
    );
}

#[test]
fn a_relief_is_pressed_on_the_lattice_the_hollow_asks() {
    let recipe = Recipe {
        relief: Some(pressed()),
        hollow: Some(Hollow {
            wall_mm: 2.0,
            precision: Some(0.8),
            ..Hollow::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("each is an edit");
    assert!(
        asked.iter().any(
            |edit| matches!(edit, Edit::Relief { precision, .. } if (precision - 0.8).abs() < 1e-6)
        ),
        "--precision 0.8 is the relief's lattice too: {asked:?}"
    );
}

fn cut_x() -> Cut {
    serde_json::from_value(serde_json::json!({"axis": "x", "at_mm": 10.0})).expect("a cut reads")
}

#[test]
fn a_cut_asked_with_arrange_is_spread_over_the_usable_area_last() {
    let recipe = with_modelling(Modelling {
        cut: Some(cut_x()),
        arrange: true,
        ..Modelling::default()
    });
    let area = ([5.0, 5.0], [200.0, 120.0]);
    let asked = asked_after_check(
        &recipe,
        &bare(),
        MODEL,
        (None, None, Some(area), (false, false)),
    )
    .expect("a cut and an arrange are edits");
    assert!(
        matches!(asked.as_slice(), [Edit::Cut { .. }, _]),
        "{asked:?}"
    );
    let spread = Edit::Arrange {
        low_mm: [5.0, 5.0],
        plate_mm: [200.0, 120.0],
        clearance_mm: core_edit::defaults::arrange_clearance_mm(),
    };
    assert_eq!(asked[1], spread, "spread last over the usable area");
    assert_eq!(
        asked_edits(&recipe, &bare(), MODEL),
        None,
        "with no area known the revision is no edit, and the command line arranges it"
    );
}

#[test]
fn an_arrange_kept_from_before_is_not_asked_again_without_new_pieces() {
    let cut = with_modelling(Modelling {
        cut: Some(cut_x()),
        arrange: true,
        ..Modelling::default()
    });
    let scaled = Recipe {
        modelling: Some(Modelling {
            scale: Some([2.0, 2.0, 2.0]),
            ..cut.modelling.clone().expect("modelling")
        }),
        ..bare()
    };
    let asked = asked_edits(&scaled, &cut, MODEL).expect("a scale is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Scale { .. }]),
        "{asked:?}"
    );
}

#[test]
fn a_cut_upright_with_sized_dowels_is_that_edit() {
    let recipe = with_modelling(Modelling {
        cut: Some(
            serde_json::from_value(serde_json::json!({"axis": "z", "at_mm": 10.0,
                "joint": "dowels", "peg_mm": [5.0, 8.0], "upright": true}))
            .expect("a cut reads from JSON"),
        ),
        ..Modelling::default()
    });
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a cut is an edit");
    let [
        Edit::Cut {
            joints, lay_flat, ..
        },
    ] = asked.as_slice()
    else {
        panic!("one cut, got {asked:?}");
    };
    assert!(!lay_flat, "the halves stand as cut");
    assert!(!joints.pegs.auto_size, "the pegs keep the size given");
}

#[test]
fn a_weld_or_a_lift_changed_is_no_edit_but_a_check_by_flags() {
    let welded = Recipe {
        mend: Some(Mend {
            weld_tolerance_mm: Some(0.01),
            ..Mend::default()
        }),
        ..bare()
    };
    assert_eq!(asked_edits(&welded, &bare(), MODEL), None);
    let lifted = Recipe {
        supports: Some(Supports {
            profile: "auto".into(),
            lift_mm: Some(5.0),
            ..Supports::default()
        }),
        ..bare()
    };
    assert_eq!(asked_edits(&lifted, &bare(), MODEL), None);
}

#[test]
fn a_hollow_keeping_thin_parts_solid_carries_that_thickness() {
    let recipe = Recipe {
        hollow: Some(Hollow {
            wall_mm: 2.0,
            keep_solid_under_mm: Some(4.0),
            ..Hollow::default()
        }),
        ..bare()
    };
    let asked = asked_edits(&recipe, &bare(), MODEL).expect("a hollow is an edit");
    let [Edit::Hollow { settings, .. }] = asked.as_slice() else {
        panic!("one hollow, got {asked:?}");
    };
    assert_eq!(settings.solid_under_mm, Some(4.0));
}

#[test]
fn a_mirror_and_a_move_are_edits_and_copies_a_check_by_flags() {
    let moved = with_modelling(
        serde_json::from_value(serde_json::json!({"mirror": "x", "move_mm": [5.0, 0.0, 0.0]}))
            .expect("modelling reads from JSON"),
    );
    let asked = asked_edits(&moved, &bare(), MODEL).expect("both are edits");
    assert!(
        matches!(asked.as_slice(), [Edit::Mirror { .. }, Edit::Move { by_mm, .. }] if (by_mm.x - 5.0).abs() < 1e-6),
        "{asked:?}"
    );
    let further = with_modelling(Modelling {
        move_mm: Some([2.0, 0.0, 0.0]),
        ..moved.modelling.clone().expect("modelling")
    });
    let asked = asked_edits(&further, &moved, MODEL).expect("a move is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Move { by_mm, .. }] if (by_mm.x + 3.0).abs() < 1e-6),
        "only the change of the move: {asked:?}"
    );
    let copied = with_modelling(Modelling {
        copies: Some(2),
        ..Modelling::default()
    });
    assert_eq!(asked_edits(&copied, &bare(), MODEL), None);
}

mod anew;
