use std::path::PathBuf;

use super::*;

fn plain() -> Recipe {
    Recipe {
        model: PathBuf::from("/models/bust.stl"),
        printer: "elegoo-mars-4-ultra".into(),
        resin: "my-grey".into(),
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
            "--printer",
            "elegoo-mars-4-ultra",
            "--resin",
            "my-grey",
            "--center",
            "--check-drainage"
        ]
    );
}

#[test]
fn every_step_becomes_the_flags_the_command_line_documents() {
    let recipe = Recipe {
        repair: Some("close-holes".into()),
        pose: Some(Pose::Search {
            goal: "auto".into(),
            max_tilt_deg: Some(30.0),
        }),
        hollow: Some(Hollow {
            wall_mm: 2.0,
            open_floor: false,
            infill: Some("hive".into()),
            infill_cell_mm: Some(6.0),
            infill_density: None,
        }),
        drains: Some(Drains {
            diameter_mm: 4.0,
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
    let tail: Vec<&str> = flags[7..].iter().map(String::as_str).collect();
    assert_eq!(
        tail,
        [
            "--repair=close-holes",
            "--orient",
            "--orient-goal",
            "auto",
            "--max-tilt",
            "30",
            "--hollow",
            "2",
            "--infill",
            "hive",
            "--infill-cell",
            "6",
            "--drain",
            "4",
            "--drain-at=30,30,0.5",
            "--drain-at=-5,0,2",
            "--drain-plugs",
            "--supports",
            "auto",
            "--support-cavity"
        ]
    );
}

#[test]
fn a_turn_given_outright_is_one_rotate_flag_that_takes_negative_degrees() {
    let recipe = Recipe {
        pose: Some(Pose::Rotate {
            degrees: [-90.0, 0.0, 45.0],
        }),
        ..plain()
    };
    assert_eq!(
        recipe.flags().last().map(String::as_str),
        Some("--rotate=-90,0,45")
    );
}

#[test]
fn a_profile_given_as_a_toml_file_takes_the_path_flags() {
    let recipe = Recipe {
        printer: "/profiles/my-printer.toml".into(),
        resin: "/profiles/grey.toml".into(),
        ..plain()
    };
    let flags = recipe.flags();
    assert_eq!(
        flags[1..5],
        [
            "--profile",
            "/profiles/my-printer.toml",
            "--material",
            "/profiles/grey.toml"
        ]
    );
}

#[test]
fn drains_with_no_point_add_no_flag() {
    let recipe = Recipe {
        drains: Some(Drains {
            diameter_mm: 4.0,
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
        pose: Some(Pose::Rotate {
            degrees: [0.0, 35.0, 0.0],
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
