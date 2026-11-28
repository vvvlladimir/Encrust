//! `profiles new-resin`: a resin from the numbers on its bottle, set up on one printer, held
//! to what that printer takes.

use std::fs;
use std::path::{Path, PathBuf};

use crate::support::*;

/// A profile directory of its own holding `limited`, the tests' panel with an exposure from
/// 0.5 to 20 s, advised at most 8.
fn profile_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-cli-new-resin-{name}"));
    let _ = fs::remove_dir_all(&dir);
    let printers = dir.join("printers");
    fs::create_dir_all(&printers).expect("a writable temporary directory");
    let panel = fs::read_to_string(test_panel()).expect("the fixture reads");
    let limited =
        format!("{panel}\n[limits]\nexposure_s = {{ min = 0.5, max = 20, warn_max = 8 }}\n");
    fs::write(printers.join("limited.toml"), limited).expect("a writable file");
    dir
}

fn new_resin(dir: &Path, exposure: &str) -> std::process::Output {
    encrust_with_profiles(
        dir,
        &[
            "profiles",
            "new-resin",
            "--printer",
            "limited",
            "--name",
            "Bottle grey",
            "--exposure",
            exposure,
            "--bottom-exposure",
            "30",
            "--bottom-layers",
            "5",
            "--json",
        ],
    )
}

#[test]
fn a_resin_from_its_bottles_numbers_is_kept_on_that_printer() {
    let dir = profile_dir("kept");
    let output = new_resin(&dir, "2.5");
    assert!(output.status.success(), "{}", stderr(&output));
    let made = json(&output);
    assert_eq!(made["advised_against"], serde_json::json!([]));
    let id = made["id"].as_str().expect("an id");
    let shown = stdout(&encrust_with_profiles(
        &dir,
        &["profiles", "show", id, "--kind", "resin"],
    ));
    assert!(shown.contains("exposure_s = 2.5"), "{shown}");
    assert!(shown.contains("bottom_layers = 5"), "{shown}");
    assert!(shown.contains("[printers.limited]"), "{shown}");
}

#[test]
fn a_number_the_printer_advises_against_is_kept_and_said() {
    let dir = profile_dir("advised");
    let output = new_resin(&dir, "12");
    assert!(output.status.success(), "{}", stderr(&output));
    let advised = json(&output)["advised_against"].to_string();
    assert!(advised.contains("exposure_s is 12"), "{advised}");
}

#[test]
fn a_number_past_what_the_printer_takes_is_refused_and_nothing_kept() {
    let dir = profile_dir("refused");
    let output = new_resin(&dir, "60");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("exposure_s is 60"),
        "{}",
        stderr(&output)
    );
    assert!(!dir.join("resins").exists(), "no resin was kept");

    let zero = new_resin(&dir, "0");
    assert_eq!(
        zero.status.code(),
        Some(2),
        "an exposure of zero is no exposure"
    );
}
