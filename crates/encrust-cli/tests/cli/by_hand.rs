//! Supports the window places and the side it paints, by hand, given on the command line at points.

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

#[test]
fn an_underside_painted_to_be_seen_stands_on_fewer_contacts() {
    let contacts = |name: &str, extra: &[&str]| {
        let mut args = vec!["--supports", "medium"];
        args.extend_from_slice(extra);
        estimated(name, &args)["models"][0]["supports"]["contacts"]
            .as_u64()
            .expect("supports are reported")
    };
    let plain = contacts("shown-plain", &[]);
    let painted = contacts("shown-painted", &["--shown-at", "8,8,0:30"]);
    assert!(
        painted < plain,
        "the painted side stands sparser: {painted} against {plain}"
    );
}

#[test]
fn a_side_painted_with_no_supports_or_no_radius_is_refused() {
    let path = write_box_stl("shown-refused", 16.0, 12, 0);
    let panel = test_panel();
    for extra in [
        &["--shown-at", "8,8,0"][..],
        &["--supports", "medium", "--shown-at", "8,8,0:-1"],
    ] {
        let mut args = vec![path.to_str().unwrap(), "--profile", panel.to_str().unwrap()];
        args.extend_from_slice(extra);
        let output = estimate(&args);
        assert!(!output.status.success(), "{extra:?} was taken");
    }
}
