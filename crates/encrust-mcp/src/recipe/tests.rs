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
        pose: Some(Pose::Search {
            goal: Goal::Auto,
            max_tilt_deg: Some(30.0),
        }),
        hollow: Some(Hollow {
            wall_mm: 2.0,
            open_floor: false,
            infill: Some(Infill::Hive),
            infill_cell_mm: Some(6.0),
            infill_density: None,
        }),
        drains: Some(Drains {
            diameter_mm: 4.0,
            auto: false,
            vent: true,
            at_mm: vec![[30.0, 30.0, 0.5], [-5.0, 0.0, 2.0]],
            plugs: true,
        }),
        supports: Some(Supports {
            profile: "auto".into(),
            cavity: true,
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
    let tilted = Recipe {
        pose: Some(Pose::Search {
            goal: Goal::LeastPeel,
            max_tilt_deg: Some(-5.0),
        }),
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
        }),
        ..plain()
    };
    assert_eq!(recipe.flags(), plain().flags());
}

#[test]
fn a_recipe_reads_back_from_the_toml_it_is_kept_in() {
    let recipe = Recipe {
        repair: Some(Repair::Wrap),
        pose: Some(Pose::Search {
            goal: Goal::FewestSupports,
            max_tilt_deg: None,
        }),
        supports: Some(Supports {
            profile: "medium".into(),
            cavity: false,
        }),
        ..plain()
    };
    let kept = toml::to_string(&recipe).expect("a recipe is plain data");
    let read: Recipe = toml::from_str(&kept).expect("it was just written");
    assert_eq!(read, recipe);
}
