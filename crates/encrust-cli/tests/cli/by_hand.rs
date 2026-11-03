//! What the window places by hand, given on the command line: supports at points and walls
//! kept solid round them.

use serde_json::Value;

use crate::support::*;

/// `estimate --json` of a 16 mm cube standing on the plate's corner, with `extra`.
fn estimated(name: &str, extra: &[&str]) -> Value {
    let path = write_box_stl(name, 16.0, 12, 0);
    let panel = test_panel();
    let mut args = vec![
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--json",
    ];
    args.extend_from_slice(extra);
    let output = estimate(&args);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        stderr(&output)
    );
    json(&output)
}

fn cavity_mm3(report: &Value) -> f64 {
    report["models"][0]["hollow"]["cavity_mm3"]
        .as_f64()
        .expect("a hollowed model reports its cavity")
}

#[test]
fn a_wall_kept_solid_round_a_point_leaves_a_smaller_cavity() {
    let open = estimated("by-hand-open", &["--hollow", "2"]);
    let kept = estimated(
        "by-hand-kept",
        &[
            "--hollow",
            "2",
            "--keep-solid",
            "2,8,8",
            "--keep-solid-radius",
            "4",
        ],
    );
    let (open, kept) = (cavity_mm3(&open), cavity_mm3(&kept));
    // A 12 mm cavity is 1728 mm³; a 4 mm ball reaching 6 mm past the wall's 2 mm takes some.
    assert!(
        open > 1000.0,
        "a 2 mm wall in a 16 mm cube leaves near 1728 mm³, read {open}"
    );
    assert!(
        kept < open - 20.0,
        "the ball kept solid takes from the cavity: {kept} of {open}"
    );
}

#[test]
fn keeping_a_wall_solid_with_no_hollow_is_refused() {
    let path = write_box_stl("by-hand-no-hollow", 16.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--keep-solid",
        "8,8,8",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--hollow"), "{}", stderr(&output));
}

#[test]
fn only_the_supports_placed_by_hand_stand_under_the_model() {
    let report = estimated(
        "by-hand-supports",
        &[
            "--supports",
            "medium",
            "--support-at",
            "8,8,0",
            "--support-only-at",
        ],
    );
    let supports = &report["models"][0]["supports"];
    assert_eq!(
        supports["contacts"], 1,
        "one contact, the one placed: {supports}"
    );
    assert_eq!(supports["standing"], 1, "and a column under it: {supports}");
}

#[test]
fn a_support_placed_by_hand_with_no_profile_is_refused() {
    let path = write_box_stl("by-hand-no-profile", 16.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--support-at",
        "8,8,0",
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--supports"),
        "{}",
        stderr(&output)
    );
}
