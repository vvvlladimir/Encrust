//! `recipes list` and `recipes show`, and `--recipe` finding a built-in one or one of the
//! user's by id (ADR 0403).

use std::fs;
use std::path::{Path, PathBuf};

use crate::support::*;

const OWN: &str = "name = \"Mine\"\nformat = 1\n\n[[step]]\ndo = \"check\"\n";

/// A profile directory of its own whose `recipes/` holds `files`, as (file name, text).
fn profile_dir(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-cli-recipes-{name}"));
    let _ = fs::remove_dir_all(&dir);
    let recipes = dir.join("recipes");
    fs::create_dir_all(&recipes).expect("a writable temporary directory");
    for (file, text) in files {
        fs::write(recipes.join(file), text).expect("a writable file");
    }
    dir
}

#[test]
fn the_built_in_recipes_are_listed_before_the_users_with_an_unreadable_one_marked() {
    let dir = profile_dir("list", &[("mine.toml", OWN), ("broken.toml", "name = ")]);
    let output = encrust_with_profiles(&dir, &["recipes", "list", "--json"]);
    assert!(output.status.success(), "{}", stderr(&output));

    let listed = json(&output);
    let recipes = listed["recipes"].as_array().expect("a list of recipes");
    let ids: Vec<&str> = recipes.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["prepare", "miniatures", "broken", "mine"]);
    assert_eq!(recipes[1]["name"], "Miniatures");
    assert_eq!(recipes[1]["source"], "built-in");
    assert_eq!(recipes[3]["source"], "user");
    assert_eq!(recipes[3]["name"], "Mine");
    assert!(recipes[2]["error"].is_string(), "{}", recipes[2]);

    let text = stdout(&encrust_with_profiles(&dir, &["recipes", "list"]));
    assert!(
        text.contains("miniatures") && text.contains("Mine"),
        "{text}"
    );
    assert!(text.contains("does not read"), "{text}");
}

#[test]
fn a_recipe_is_shown_as_the_toml_it_reads_back() {
    let dir = profile_dir("show", &[]);
    let output = encrust_with_profiles(&dir, &["recipes", "show", "miniatures"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let shown = stdout(&output);
    assert!(shown.contains("name = \"Miniatures\""), "{shown}");
    assert!(shown.contains("least-peel"), "{shown}");

    let path = dir.join("copy.toml");
    fs::write(&path, &shown).expect("a writable file");
    let again = encrust_with_profiles(&dir, &["recipes", "show", path.to_str().unwrap()]);
    assert_eq!(stdout(&again), shown, "what is shown reads back as itself");

    let as_json = json(&encrust_with_profiles(
        &dir,
        &["recipes", "show", "miniatures", "--json"],
    ));
    assert_eq!(as_json["toml"], shown);
    assert_eq!(as_json["source"], "built-in");
}

#[test]
fn an_unknown_recipe_is_refused_with_the_ids_there_are() {
    let dir = profile_dir("unknown", &[]);
    let output = encrust_with_profiles(&dir, &["recipes", "show", "no-such"]);
    assert_eq!(output.status.code(), Some(1));
    let errors = stderr(&output);
    assert!(
        errors.contains("no-such") && errors.contains("miniatures"),
        "{errors}"
    );
}

/// A 16 mm cube sliced through `--recipe recipe` with the profile directory `dir`.
fn slice_by(dir: &Path, name: &str, recipe: &str) -> std::process::Output {
    let model = write_box_stl(name, 16.0, 12, 0);
    let panel = test_panel();
    let out = output_dir(name);
    encrust_with_profiles(
        dir,
        &[
            "slice",
            model.to_str().unwrap(),
            "--profile",
            panel.to_str().unwrap(),
            "--layer-height",
            "0.5",
            "--recipe",
            recipe,
            "-o",
            out.to_str().unwrap(),
            "--json",
        ],
    )
}

#[test]
fn a_built_in_recipe_slices_a_model_by_its_id() {
    let dir = profile_dir("slice-built-in", &[]);
    let output = slice_by(&dir, "recipes-miniatures", "miniatures");
    assert!(output.status.success(), "{}", stderr(&output));
    let report = json(&output);
    let steps = report["recipe"]["steps"].as_array().expect("the steps run");
    assert_eq!(
        steps.len(),
        5,
        "repair, orient, supports, check and the gate"
    );
}

#[test]
fn a_users_recipe_is_found_by_its_stem_and_a_built_in_one_wins_a_clash() {
    let dir = profile_dir("by-stem", &[("mine.toml", OWN), ("miniatures.toml", OWN)]);
    let output = slice_by(&dir, "recipes-by-stem", "mine");
    assert!(output.status.success(), "{}", stderr(&output));
    let steps = json(&output)["recipe"]["steps"].as_array().unwrap().len();
    assert_eq!(steps, 1, "the user's one-step recipe ran");

    let output = slice_by(&dir, "recipes-clash", "miniatures");
    assert!(output.status.success(), "{}", stderr(&output));
    let steps = json(&output)["recipe"]["steps"].as_array().unwrap().len();
    assert_eq!(steps, 5, "the built-in miniatures ran, not the user's copy");
}
