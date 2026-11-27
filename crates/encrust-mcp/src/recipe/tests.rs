use std::path::PathBuf;

use super::finish::{HollowMode, Infill};
use super::*;

fn plain() -> Recipe {
    Recipe::bare(
        PathBuf::from("/models/bust.stl"),
        "elegoo-mars-4-ultra".into(),
        "my-grey".into(),
    )
}

#[test]
fn a_plain_recipe_stages_the_model_centred_and_checks_drainage() {
    assert_eq!(
        plain().flags(),
        [
            "/models/bust.stl",
            "--printer=elegoo-mars-4-ultra",
            "--resin=my-grey",
            "--center",
            "--check-drainage"
        ]
    );
}

#[test]
fn a_job_with_no_machine_is_checked_on_the_standard_one_but_a_project_on_its_own() {
    let unnamed = Recipe::bare(
        PathBuf::from("/models/bust.stl"),
        String::new(),
        String::new(),
    );
    let project = Recipe::bare(
        PathBuf::from("/plates/a.encrust"),
        String::new(),
        String::new(),
    );

    assert_eq!(
        unnamed.flags()[1..3],
        ["--standard-machine", "--center"],
        "a model with no printer named is placed on the standard printer's plate"
    );
    assert!(
        !project
            .flags()
            .iter()
            .any(|flag| flag == "--standard-machine"),
        "a saved project keeps the machine it names"
    );
}

#[test]
fn every_step_becomes_the_flags_the_command_line_documents() {
    let recipe = Recipe {
        repair: Some(Repair::CloseHoles),
        pose: Some(Pose::search(Goal::Auto, Some(30.0))),
        hollow: Some(Hollow {
            wall_mm: 2.0,
            open_floor: false,
            infill: Some(Infill::Hive),
            infill_cell_mm: Some(6.0),
            infill_density: None,
            ..Default::default()
        }),
        drains: Some(Drains {
            diameter_mm: 4.0,
            auto: false,
            vent: true,
            at_mm: vec![[30.0, 30.0, 0.5], [-5.0, 0.0, 2.0]],
            plugs: true,
            ..Default::default()
        }),
        supports: Some(Supports {
            profile: "auto".into(),
            cavity: true,
            ..Default::default()
        }),
        ..plain()
    };
    let flags = recipe.flags();
    assert_eq!(
        flags[5..],
        [
            "--repair=close-holes",
            "--orient",
            "--orient-goal=auto",
            "--max-tilt=30",
            "--hollow=2",
            "--infill=hive",
            "--infill-cell=6",
            "--drain=4",
            "--drain-at=30,30,0.5",
            "--drain-at=-5,0,2",
            "--drain-plugs",
            "--supports=auto",
            "--support-cavity"
        ]
    );
}

#[test]
fn a_negative_number_rides_on_its_flag() {
    let picked = Recipe {
        pose: Some(Pose::Search {
            keep_clear_at_mm: vec![[10.0, -2.0, 3.0]],
            keep_clear_seen_deg: Some([0.0, -90.0, 0.0]),
            stood_deg: None,
            keep_clear_radius_mm: Some(5.0),
            pick: Some(2),
            goal: Goal::Auto,
            max_tilt_deg: None,
            clean_surface: true,
        }),
        ..plain()
    };
    let flags = picked.flags();
    for flag in [
        "--orient-pick=2",
        "--orient-keep-clear=10,-2,3",
        "--orient-keep-clear-turn=0,-90,0",
        "--orient-keep-clear-radius=5",
        "--clean-surface",
    ] {
        assert!(flags.contains(&flag.to_owned()), "{flag} in {flags:?}");
    }
    let tilted = Recipe {
        pose: Some(Pose::search(Goal::LeastPeel, Some(-5.0))),
        ..plain()
    };
    assert!(tilted.flags().contains(&"--max-tilt=-5".to_owned()));
    let turned = Recipe {
        pose: Some(Pose::Rotate {
            degrees: [-90.0, 0.0, 45.0],
        }),
        ..plain()
    };
    assert_eq!(
        turned.flags().last().map(String::as_str),
        Some("--rotate=-90,0,45")
    );
}

#[test]
fn the_autopilot_rides_before_every_step_given_outright() {
    let recipe = Recipe {
        prepare: true,
        supports: Some(Supports {
            profile: "light".into(),
            cavity: false,
            ..Default::default()
        }),
        ..plain()
    };
    assert_eq!(recipe.flags()[5..], ["--prepare", "--supports=light"]);
}

#[test]
fn the_whole_repair_is_the_bare_flag() {
    let recipe = Recipe {
        repair: Some(Repair::Full),
        ..plain()
    };
    assert_eq!(recipe.flags().last().map(String::as_str), Some("--repair"));
}

#[test]
fn a_profile_given_as_a_toml_file_takes_the_path_flags() {
    let recipe = Recipe {
        printer: "/profiles/my-printer.toml".into(),
        resin: "/profiles/grey.TOML".into(),
        ..plain()
    };
    assert_eq!(
        recipe.flags()[1..3],
        [
            "--profile=/profiles/my-printer.toml",
            "--material=/profiles/grey.TOML"
        ]
    );
}

#[test]
fn automatic_drains_ask_the_engine_and_may_leave_out_the_vent() {
    let auto = |vent| Recipe {
        drains: Some(Drains {
            diameter_mm: 3.0,
            auto: true,
            vent,
            at_mm: Vec::new(),
            plugs: false,
            ..Default::default()
        }),
        ..plain()
    };
    assert_eq!(auto(true).flags()[5..], ["--drain=3", "--drain-at=auto"]);
    assert_eq!(
        auto(false).flags()[5..],
        ["--drain=3", "--drain-at=auto", "--no-drain-vent"]
    );
}

#[test]
fn drains_with_no_point_add_no_flag() {
    let recipe = Recipe {
        drains: Some(Drains {
            diameter_mm: 4.0,
            auto: false,
            vent: true,
            at_mm: Vec::new(),
            plugs: true,
            ..Default::default()
        }),
        ..plain()
    };
    assert_eq!(recipe.flags(), plain().flags());
}

#[test]
fn a_recipe_reads_back_from_the_toml_it_is_kept_in() {
    let recipe = Recipe {
        repair: Some(Repair::Wrap),
        pose: Some(Pose::search(Goal::FewestSupports, None)),
        supports: Some(Supports {
            profile: "medium".into(),
            cavity: false,
            ..Default::default()
        }),
        ..plain()
    };
    let kept = toml::to_string(&recipe).expect("a recipe is plain data");
    let read: Recipe = toml::from_str(&kept).expect("it was just written");
    assert_eq!(read, recipe);
}

#[test]
fn the_modelling_and_a_relief_become_the_command_lines_flags_with_the_others_after_the_model() {
    let recipe = Recipe {
        modelling: Some(Modelling {
            scale: Some([2.0, 2.0, 2.0]),
            combine: Some(shape::Combine {
                op: shape::Op::Difference,
                with: vec![PathBuf::from("/models/cutter.stl")],
            }),
            label: Some(shape::Label {
                text: "-A1".into(),
                at_mm: [1.0, 2.0, 3.0],
                height_mm: Some(5.0),
                depth_mm: None,
                sunk: true,
                follow: false,
                angle_deg: None,
                font: Some(PathBuf::from("/fonts/serif.ttf")),
            }),
            cut: Some(shape::Cut {
                axis: shape::Axis::Z,
                at_mm: 10.0,
                keep: Some(shape::Keep::Lower),
                joint: Some(shape::Joint::Dowels),
                joint_count: Some(3),
                clearance_mm: Some(0.2),
                peg_mm: Some([5.0, 8.0]),
                magnet_mm: None,
                key_mm: None,
                unkeyed: true,
                upright: true,
            }),
            split_smallest_mm3: Some(5.0),
            part_off_at_mm: Some([-1.0, 0.0, 4.0]),
            arrange: true,
            mirror: Some(shape::Axis::Y),
            move_mm: Some([0.0, -3.0, 0.0]),
            copies: Some(2),
            objects: Vec::new(),
        }),
        relief: Some(Relief {
            depth_mm: -0.4,
            image: Some(PathBuf::from("/pictures/bark.png")),
            tile_mm: Some(20.0),
            tile_angle_deg: None,
            invert: true,
            symmetric: false,
            smooth_px: None,
            hold_bottom_deg: Some(30.0),
            hold_top_deg: None,
        }),
        ..plain()
    };
    let flags = recipe.flags();
    assert_eq!(flags[..2], ["/models/bust.stl", "/models/cutter.stl"]);
    for flag in [
        "--scale=2,2,2",
        "--combine=difference",
        "--label=-A1",
        "--label-at=1,2,3",
        "--label-height=5",
        "--label-sunk",
        "--cut=z:10",
        "--keep=lower",
        "--joint=dowels",
        "--joint-count=3",
        "--joint-clearance=0.2",
        "--peg-size=5:8",
        "--unkeyed",
        "--cut-upright",
        "--label-font=/fonts/serif.ttf",
        "--split",
        "--split-smallest=5",
        "--part-off=-1,0,4",
        "--arrange",
        "--mirror=y",
        "--move=0,-3,0",
        "--copies=2",
        "--relief=-0.4",
        "--relief-image=/pictures/bark.png",
        "--relief-tile=20",
        "--relief-hold-bottom=30",
        "--relief-invert",
    ] {
        assert!(
            flags.iter().any(|given| given == flag),
            "{flag} is missing from {flags:?}"
        );
    }
}

#[test]
fn what_is_placed_by_hand_and_how_it_is_sliced_become_flags() {
    let recipe = Recipe {
        hollow: Some(Hollow {
            wall_mm: 2.0,
            mode: Some(HollowMode::External),
            keep_solid_at_mm: vec![[5.0, 5.0, 5.0]],
            keep_solid_reach_mm: Some(3.0),
            ..Default::default()
        }),
        drains: Some(Drains {
            diameter_mm: 3.0,
            channels: vec![vec![[0.0, 0.0, 1.0], [10.0, 0.0, 1.0]]],
            sides: Some(6),
            kind: Some(HoleKind::Walled),
            ..Default::default()
        }),
        supports: Some(Supports {
            profile: "/profiles/thin.toml".into(),
            at_mm: vec![[1.0, 1.0, 0.0]],
            only_at: true,
            shown_at_mm: vec![[2.0, 3.0, 4.0]],
            shown_radius_mm: Some(6.0),
            merge_at_mm: vec![vec![[1.0, 1.0, 0.0], [2.0, 1.0, 0.0]]],
            ..Default::default()
        }),
        slicing: Some(Slicing {
            layer_height_mm: Some(0.03),
            anti_alias: Some(false),
            remove_islands_passes: Some(2),
            exposure_bands: vec![slicing::Band {
                from_mm: 0.0,
                to_mm: 10.0,
                exposure_s: 4.5,
            }],
            ..Default::default()
        }),
        mend: Some(Mend {
            flip_faces: vec![3, 7],
            ..Default::default()
        }),
        ..plain()
    };
    let flags = recipe.flags();
    for flag in [
        "--hollow-mode=external",
        "--shown-at=2,3,4:6",
        "--keep-solid=5,5,5",
        "--keep-solid-size=3",
        "--channel=0,0,1:10,0,1",
        "--drain-sides=6",
        "--drain-walled",
        "--support-profile=/profiles/thin.toml",
        "--support-at=1,1,0",
        "--support-only-at",
        "--merge-supports=1,1,0;2,1,0",
        "--layer-height=0.03",
        "--no-anti-alias",
        "--remove-islands",
        "--hanging-passes=2",
        "--exposure-at=0:10:4.5",
        "--flip-faces=3,7",
    ] {
        assert!(
            flags.iter().any(|given| given == flag),
            "{flag} is missing from {flags:?}"
        );
    }
}

#[test]
fn a_relative_path_from_the_client_is_refused() {
    let recipe = Recipe {
        relief: Some(Relief {
            depth_mm: 0.5,
            image: Some(PathBuf::from("bark.png")),
            tile_mm: None,
            tile_angle_deg: None,
            invert: false,
            symmetric: false,
            smooth_px: None,
            hold_bottom_deg: None,
            hold_top_deg: None,
        }),
        ..plain()
    };
    assert!(recipe.check_paths().is_err());
}

#[test]
fn a_recipe_named_runs_in_place_of_the_autopilot() {
    let recipe = Recipe {
        prepare: true,
        steps_by: Some("miniatures".into()),
        ..plain()
    };
    let flags = recipe.flags();
    assert!(
        flags.contains(&"--recipe=miniatures".to_owned()),
        "{flags:?}"
    );
    assert!(!flags.contains(&"--prepare".to_owned()), "{flags:?}");
}

#[test]
fn the_windows_finer_settings_ride_on_their_flags() {
    let recipe = Recipe {
        mend: Some(Mend {
            weld_tolerance_mm: Some(0.01),
            ..Mend::default()
        }),
        hollow: Some(Hollow {
            wall_mm: 2.0,
            keep_solid_under_mm: Some(4.0),
            ..Hollow::default()
        }),
        supports: Some(Supports {
            profile: "auto".into(),
            lift_mm: Some(5.0),
            ..Supports::default()
        }),
        slicing: Some(Slicing {
            print: Some(slicing::PrintSettings {
                exposure_s: Some(2.5),
                bottom_layers: Some(6),
                wait_after_lift_s: Some(1.0),
                light_power: Some(200),
                ..slicing::PrintSettings::default()
            }),
            ..Slicing::default()
        }),
        ..plain()
    };
    let flags = recipe.flags();
    for flag in [
        "--weld-tolerance=0.01",
        "--keep-solid-under=4",
        "--lift=5",
        "--exposure=2.5",
        "--bottom-layers=6",
        "--wait-after-lift=1",
        "--light-power=200",
    ] {
        assert!(flags.contains(&flag.to_owned()), "{flag} in {flags:?}");
    }
}

#[test]
fn checks_answers_ride_on_their_flags() {
    let recipe = Recipe {
        fixes: Some(Fixes {
            support_islands: true,
            drill_cups: false,
            hold_necks: true,
            objects: Vec::new(),
        }),
        ..plain()
    };
    let flags = recipe.flags();
    assert!(flags.contains(&"--support-islands".to_owned()), "{flags:?}");
    assert!(flags.contains(&"--hold-necks".to_owned()), "{flags:?}");
    assert!(!flags.contains(&"--drill-cups".to_owned()), "{flags:?}");
}

#[test]
fn steps_naming_objects_run_on_the_plate_before_them_by_only() {
    let recipe = Recipe {
        modelling: Some(Modelling {
            cut: Some(
                serde_json::from_value(serde_json::json!({"axis": "z", "at_mm": 10.0}))
                    .expect("a cut reads"),
            ),
            arrange: true,
            ..Modelling::default()
        }),
        hollow: Some(Hollow {
            wall_mm: 2.0,
            objects: vec![2],
            ..Hollow::default()
        }),
        supports: Some(Supports {
            profile: "auto".into(),
            ..Supports::default()
        }),
        ..plain()
    };
    let runs = recipe.runs().expect("runs");
    assert_eq!(
        runs.len(),
        3,
        "the model and its cut, then each object group: {runs:?}"
    );
    assert!(runs[0].contains(&"--cut=z:10".to_owned()), "{:?}", runs[0]);
    assert_eq!(runs[1][0], PREVIOUS);
    assert!(runs[1].contains(&"--only=2".to_owned()) && runs[1].contains(&"--hollow=2".to_owned()));
    assert!(
        !runs[2].iter().any(|flag| flag.starts_with("--only")),
        "every object: {:?}",
        runs[2]
    );
    assert!(
        runs[2].contains(&"--arrange".to_owned()),
        "spread last: {:?}",
        runs[2]
    );
}

#[test]
fn checks_answers_run_last_on_the_objects_they_name() {
    let recipe = Recipe {
        hollow: Some(Hollow {
            wall_mm: 2.0,
            objects: vec![2],
            ..Hollow::default()
        }),
        fixes: Some(Fixes {
            support_islands: true,
            objects: vec![3],
            ..Fixes::default()
        }),
        ..plain()
    };
    let runs = recipe
        .runs()
        .expect("answers run on a plate whose steps name objects");
    let last = runs.last().expect("a run");
    assert_eq!(
        last[0], PREVIOUS,
        "on the plate the run before wrote: {runs:?}"
    );
    for flag in ["--only=3", "--support-islands"] {
        assert!(last.contains(&flag.to_owned()), "{flag} in {last:?}");
    }
    assert!(
        !runs[..runs.len() - 1]
            .concat()
            .contains(&"--support-islands".to_owned()),
        "answered once, last: {runs:?}"
    );
}

#[test]
fn checks_answers_for_one_object_stage_the_rest_first() {
    let recipe = Recipe {
        fixes: Some(Fixes {
            drill_cups: true,
            objects: vec![1],
            ..Fixes::default()
        }),
        ..plain()
    };
    let runs = recipe.runs().expect("runs");
    assert_eq!(runs.len(), 2, "the model, then its answers: {runs:?}");
    assert!(runs[1].contains(&"--only=1".to_owned()), "{:?}", runs[1]);
}

#[test]
fn a_recipe_with_no_objects_named_is_one_run() {
    assert_eq!(plain().runs().expect("runs"), [plain().flags()]);
}

#[test]
fn supports_asked_for_one_piece_keep_those_asked_for_every_one() {
    let auto = Supports {
        profile: "auto".into(),
        ..Default::default()
    };
    let one = Supports {
        profile: "auto".into(),
        at_mm: vec![[10.0, 20.0, 3.0]],
        objects: vec![2],
        ..Default::default()
    };
    let mut recipe = plain();
    recipe.ask_supports(Some(auto.clone()));
    recipe.ask_supports(Some(one.clone()));

    let runs = recipe.runs().expect("the steps run in turn");
    assert!(
        runs[0].iter().any(|flag| flag == "--supports=auto"),
        "every piece keeps its supports: {runs:?}"
    );
    assert!(
        runs[1].iter().any(|flag| flag == "--only=2"),
        "then piece 2 alone takes its points: {runs:?}"
    );

    recipe.ask_supports(Some(auto));
    assert!(
        recipe.more_supports.is_empty(),
        "supports for every piece stand in for all before"
    );
}

#[test]
fn supports_asked_again_for_one_piece_keep_the_other_pieces() {
    let piece = |id| Supports {
        profile: "auto".into(),
        objects: vec![id],
        ..Default::default()
    };
    let mut recipe = plain();
    recipe.ask_supports(Some(piece(1)));
    recipe.ask_supports(Some(piece(2)));
    recipe.ask_supports(Some(piece(1)));

    assert_eq!(
        recipe.more_supports,
        [piece(2)],
        "piece 2 keeps its supports when piece 1 is asked again"
    );
}

#[test]
fn a_pose_runs_on_the_pieces_after_a_cut_naming_some_objects() {
    let recipe = Recipe {
        modelling: Some(Modelling {
            cut: Some(
                serde_json::from_value(serde_json::json!({"axis": "z", "at_mm": 10.0}))
                    .expect("a cut reads"),
            ),
            objects: vec![1],
            ..Modelling::default()
        }),
        pose: Some(Pose::search(Goal::Auto, None)),
        ..plain()
    };
    let runs = recipe.runs().expect("runs");
    let at = |flag: &str| runs.iter().position(|run| run.iter().any(|f| f == flag));
    assert!(
        at("--cut=z:10") < at("--orient"),
        "the search runs after the cut: {runs:?}"
    );
    assert!(
        !runs
            .last()
            .expect("a run")
            .iter()
            .any(|f| f.starts_with("--only")),
        "on every piece: {runs:?}"
    );
}

#[test]
fn a_search_keeps_the_turn_its_check_chose_and_stands_by_it_after() {
    let mut recipe = Recipe {
        pose: Some(Pose::search(Goal::Auto, None)),
        ..plain()
    };
    let report = serde_json::json!({"models": [{"oriented": {"rotate_deg": [90.0, 0.0, 45.0]}}]});
    recipe.pin_pose(&report);
    let flags = recipe.flags();
    assert!(flags.contains(&"--rotate=90,0,45".to_owned()), "{flags:?}");
    assert!(!flags.contains(&"--orient".to_owned()), "{flags:?}");
    assert!(
        Pose::same_ask(recipe.pose.as_ref(), Some(&Pose::search(Goal::Auto, None))),
        "the turn kept is no new ask"
    );
}

#[test]
fn a_pose_over_several_models_keeps_no_turn() {
    let mut recipe = Recipe {
        pose: Some(Pose::search(Goal::Auto, None)),
        ..plain()
    };
    let turned = serde_json::json!({"oriented": {"rotate_deg": [90.0, 0.0, 0.0]}});
    recipe.pin_pose(&serde_json::json!({"models": [turned.clone(), turned]}));
    assert!(recipe.flags().contains(&"--orient".to_owned()));
}
