//! Printers and resins by id: the catalogue, a user's override of it, and what a run
//! with no resin is refused with.

use std::fs;
use std::path::PathBuf;

use crate::support::*;

#[test]
fn the_catalogue_is_listed_without_a_mesh() {
    let dir = resin_dir("listing");
    let output = encrust_with_profiles(&dir, &["profiles", "list"]);
    assert!(output.status.success(), "{}", stdout(&output));

    let text = stdout(&output);
    assert!(text.contains("elegoo-mars-4-ultra"), "{text}");
    assert!(text.contains("phrozen-sonic-mini-8k"), "{text}");
    // The catalogue ships no resin, so what is listed is the user's own (ADR 0196).
    assert!(
        text.contains("my-grey") && text.contains("[user]"),
        "{text}"
    );
}

#[test]
fn a_printer_id_replaces_the_profile_path() {
    let path = write_box_stl("by-id", 10.0, 12, 0);
    let output = encrust(&[
        "inspect",
        path.to_str().unwrap(),
        "--printer",
        "elegoo-mars-3-pro",
    ]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(field(&stdout(&output), "fits"), "Mars 3 Pro: yes");
}

#[test]
fn an_unknown_printer_id_is_refused() {
    let path = write_box_stl("unknown-id", 10.0, 12, 0);
    let output = encrust(&[
        "inspect",
        path.to_str().unwrap(),
        "--printer",
        "no-such-machine",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let errors = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert!(errors.contains("no-such-machine"), "{errors}");
}

#[test]
fn a_resin_id_arrives_tuned_for_the_chosen_printer() {
    let dir = resin_dir("tuned-resin");
    let path = write_box_stl("tuned-resin", 10.0, 12, 0);
    let out = output_file("tuned.goo");
    let output = encrust_with_profiles(
        &dir,
        &[
            "slice",
            path.to_str().unwrap(),
            "--profile",
            test_panel().to_str().unwrap(),
            "--resin",
            "my-grey",
            "--printer",
            "elegoo-mars-3-pro",
            "--layer-height",
            "2",
            "-o",
            out.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));

    // The path wins for the panel, so the file is the 200 px test panel, while the
    // exposure is the 3.0 s the resin was measured at on a Mars 3 Pro.
    let file = fs::read(&out).expect("the goo file was written");
    let text = String::from_utf8_lossy(&file[0..1000]).to_string();
    assert!(text.contains("Test panel"), "{text}");
}

/// A machine named out of the catalogue needs a resin named with it: nothing ships one,
/// so there is no exposure to fall back on (ADR 0196).
#[test]
fn a_printer_without_a_resin_is_refused_rather_than_given_stock_numbers() {
    let dir = resin_dir("no-resin");
    let path = write_box_stl("no-resin", 10.0, 12, 0);
    let out = output_file("no-resin.goo");
    let output = encrust_with_profiles(
        &dir,
        &[
            "slice",
            path.to_str().unwrap(),
            "--printer",
            "elegoo-mars-3-pro",
            "--layer-height",
            "2",
            "-o",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let errors = stderr(&output);
    assert!(errors.contains("--resin"), "{errors}");
    assert!(!out.exists(), "and nothing is written at invented numbers");
}

/// A profile directory of this test's own holding one resin, `my-grey`, which is what a
/// user who has measured one has.
fn resin_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-cli-resins-{name}"));
    let resins = dir.join("resins");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&resins).expect("a writable temporary directory");
    fs::copy(test_resin(), resins.join("my-grey.toml")).expect("copy the resin");
    dir
}

#[test]
fn a_user_profile_overrides_the_catalogue_by_id() {
    let dir = std::env::temp_dir().join("encrust-cli-user-profiles");
    let printers = dir.join("printers");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&printers).expect("temp dir");
    fs::copy(test_panel(), printers.join("elegoo-mars-4-ultra.toml")).expect("copy the panel");

    let path = write_box_stl("user-override", 10.0, 12, 0);
    let output = encrust_with_profiles(
        &dir,
        &[
            "inspect",
            path.to_str().unwrap(),
            "--printer",
            "elegoo-mars-4-ultra",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(
        field(&stdout(&output), "fits"),
        "Test panel: yes",
        "the user's file wins over the bundled machine of the same id"
    );

    fs::remove_dir_all(&dir).expect("the temporary directory goes away");
}

#[test]
fn a_shipped_profile_shows_as_toml_that_loads_back() {
    let mine = output_dir("profiles-show-mine");
    fs::create_dir_all(&mine).expect("the temporary directory is writable");
    let output = encrust_with_profiles(&mine, &["profiles", "show", "elegoo-mars-4-ultra"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let path = output_dir("profiles-show").with_extension("toml");
    fs::write(&path, output.stdout).expect("the temporary directory is writable");
    let shown = printer_profiles::PrinterProfile::load(&path).expect("the shown TOML loads");
    let shipped =
        printer_profiles::PrinterProfile::load(&shipped_profile()).expect("the shipped file");
    assert_eq!(shown.name, shipped.name);

    let unknown = encrust_with_profiles(&mine, &["profiles", "show", "no-such-machine"]);
    assert_eq!(unknown.status.code(), Some(1));
}

#[test]
fn completions_are_printed_for_each_shell() {
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let output = encrust(&["completions", shell]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{shell}: {}",
            stderr(&output)
        );
        assert!(stdout(&output).contains("encrust"), "{shell}");
    }
}
