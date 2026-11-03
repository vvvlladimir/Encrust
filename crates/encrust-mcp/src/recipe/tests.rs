use std::path::PathBuf;

use super::*;

fn plain() -> Recipe {
    Recipe {
        model: PathBuf::from("/models/bust.stl"),
        printer: "elegoo-mars-4-ultra".into(),
        resin: "my-grey".into(),
        prepare: false,
        repair: None,
        pose: None,
        hollow: None,
        drains: None,
        supports: None,
        mend: None,
        modelling: None,
        relief: None,
        slicing: None,
    }
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
            keep_clear_radius_mm: Some(5.0),
            pick: Some(2),
            goal: Goal::Auto,
            max_tilt_deg: None,
        }),
        ..plain()
    };
    let flags = picked.flags();
    for flag in [
        "--orient-pick=2",
        "--orient-keep-clear=10,-2,3",
        "--orient-keep-clear-turn=0,-90,0",
        "--orient-keep-clear-radius=5",
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
            }),
            cut: Some(shape::Cut {
                axis: shape::Axis::Z,
                at_mm: 10.0,
                keep: Some(shape::Keep::Lower),
                joint: Some(shape::Joint::Dowels),
            }),
            split_smallest_mm3: Some(5.0),
            part_off_at_mm: Some([-1.0, 0.0, 4.0]),
            arrange: true,
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
        "--split",
        "--split-smallest=5",
        "--part-off=-1,0,4",
        "--arrange",
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
            keep_solid_radius_mm: Some(3.0),
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
        "--keep-solid=5,5,5",
        "--keep-solid-radius=3",
        "--channel=0,0,1:10,0,1",
        "--drain-sides=6",
        "--drain-walled",
        "--support-profile=/profiles/thin.toml",
        "--support-at=1,1,0",
        "--support-only-at",
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
