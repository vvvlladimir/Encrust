//! Supports the window places by hand, given on the command line at points.

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
