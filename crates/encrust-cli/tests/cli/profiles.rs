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
fn a_broken_copy_is_listed_past_and_reset_to_the_shipped_profile() {
    let dir = resin_dir("broken-copy");
    let supports = dir.join("supports");
    fs::create_dir_all(&supports).expect("a writable temporary directory");
    fs::write(supports.join("medium.toml"), "name = ").expect("a writable file");

    let listed = encrust_with_profiles(&dir, &["profiles", "list"]);
    assert!(listed.status.success(), "{}", stdout(&listed));
    assert!(stdout(&listed).contains("medium"), "{}", stdout(&listed));

    let reset = encrust_with_profiles(&dir, &["profiles", "reset-unreadable"]);
    assert!(reset.status.success(), "{}", stdout(&reset));
    assert!(!supports.join("medium.toml").exists(), "{}", stdout(&reset));
    assert!(
        dir.join("resins/my-grey.toml").exists(),
        "a readable file is never touched"
    );
}

#[test]
fn a_broken_profile_of_your_own_goes_only_when_asked() {
    let dir = resin_dir("broken-own");
    let own = dir.join("resins/my-broken.toml");
    fs::write(&own, "name = ").expect("a writable file");

    let reset = encrust_with_profiles(&dir, &["profiles", "reset-unreadable"]);
    assert!(reset.status.success(), "{}", stdout(&reset));
    assert!(own.exists(), "nothing ships in its place, so it stays");

    let reset = encrust_with_profiles(&dir, &["profiles", "reset-unreadable", "--with-own"]);
    assert!(reset.status.success(), "{}", stdout(&reset));
    assert!(!own.exists(), "{}", stdout(&reset));
    assert!(
        dir.join("resins/my-grey.toml").exists(),
        "a readable file is never touched"
    );
}

#[test]
fn a_user_profile_overrides_the_catalogue_by_id() {
    let dir = std::env::temp_dir().join("encrust-cli-user-profiles");
    let printers = dir.join("printers");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&printers).expect("temp dir");
    // The user's file of a shipped machine holds their edits over it (ADR 0373).
    fs::write(
        printers.join("elegoo-mars-4-ultra.toml"),
        "edits_over_shipped = true\nname = \"Test panel\"\n",
    )
    .expect("temp dir");

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

#[test]
fn an_exported_drain_preset_is_imported_listed_and_shown() {
    let dir = resin_dir("import-preset");
    let file = dir.join("wide.toml");
    fs::write(
        &file,
        "name = \"Wide\"\n[settings]\ndiameter_mm = 3.5\ndepth_mm = 6.0\ntaper = 0.0\n",
    )
    .expect("the temp dir is writable");

    let output = encrust_with_profiles(
        &dir,
        &[
            "profiles",
            "import",
            file.to_str().unwrap(),
            "--kind",
            "drain",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("imported as wide"),
        "{}",
        stdout(&output)
    );

    let listing = stdout(&encrust_with_profiles(&dir, &["profiles", "list"]));
    assert!(listing.contains("Drain presets:"), "{listing}");
    let shown = encrust_with_profiles(&dir, &["profiles", "show", "wide", "--kind", "drain"]);
    assert!(shown.status.success(), "{}", stderr(&shown));
    assert!(
        stdout(&shown).contains("diameter_mm = 3.5"),
        "{}",
        stdout(&shown)
    );

    let refused = encrust_with_profiles(
        &dir,
        &[
            "profiles",
            "import",
            file.to_str().unwrap(),
            "--kind",
            "hollow",
        ],
    );
    assert_eq!(
        refused.status.code(),
        Some(1),
        "a drain is not a hollow preset"
    );
}

#[test]
fn a_printer_named_for_anything_but_a_resin_is_refused() {
    let dir = resin_dir("import-printer-flag");
    let file = dir.join("wide.toml");
    fs::write(
        &file,
        "name = \"Wide\"\n[settings]\ndiameter_mm = 3.5\ndepth_mm = 6.0\ntaper = 0.0\n",
    )
    .expect("the temp dir is writable");

    let refused = encrust_with_profiles(
        &dir,
        &[
            "profiles",
            "import",
            file.to_str().unwrap(),
            "--kind",
            "drain",
            "--printer",
            "elegoo-mars-4-ultra",
        ],
    );
    assert_eq!(
        refused.status.code(),
        Some(1),
        "a drain preset is set up on no machine"
    );
}

/// A stack sliced for `printer`, or for the 200 px test panel where none is named, with
/// the resin `my-grey` of `dir`.
fn sliced_with_my_grey(dir: &std::path::Path, name: &str, printer: Option<&str>) -> PathBuf {
    let path = write_box_stl(name, 10.0, 12, 0);
    let out = output_file(&format!("{name}.goo"));
    let mut args = vec![
        "slice".to_owned(),
        path.to_str().unwrap().to_owned(),
        "--resin".to_owned(),
        "my-grey".to_owned(),
        "--layer-height".to_owned(),
        "2".to_owned(),
        "-o".to_owned(),
        out.to_str().unwrap().to_owned(),
    ];
    match printer {
        Some(id) => args.extend(["--printer".to_owned(), id.to_owned()]),
        None => args.extend([
            "--profile".to_owned(),
            test_panel().to_str().unwrap().to_owned(),
        ]),
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = encrust_with_profiles(dir, &args);
    assert!(output.status.success(), "{}", stderr(&output));
    out
}

#[test]
fn a_sliced_files_resin_is_imported_onto_the_machine_it_was_cut_for() {
    let dir = resin_dir("import-sliced");
    let file = sliced_with_my_grey(&dir, "import-sliced", Some("elegoo-saturn-4-ultra"));

    let output = encrust_with_profiles(&dir, &["profiles", "import", file.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    let said = stdout(&output);
    assert!(said.contains("set up on elegoo-saturn-4-ultra"), "{said}");

    let id = said
        .split("imported as ")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .expect("the new id is said");
    let shown = stdout(&encrust_with_profiles(
        &dir,
        &["profiles", "show", id, "--kind", "resin"],
    ));
    // The file's own layer, not the 0.05 mm the resin was measured at.
    assert!(shown.contains("layer_height_mm = 2.0"), "{shown}");
    assert!(
        shown.contains("[printers.elegoo-saturn-4-ultra]"),
        "{shown}"
    );
}

#[test]
fn a_sliced_file_for_no_machine_we_know_needs_one_named() {
    let dir = resin_dir("import-sliced-panel");
    let file = sliced_with_my_grey(&dir, "import-sliced-panel", None);

    let refused = encrust_with_profiles(&dir, &["profiles", "import", file.to_str().unwrap()]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        stderr(&refused).contains("--printer"),
        "{}",
        stderr(&refused)
    );

    let named = encrust_with_profiles(
        &dir,
        &[
            "profiles",
            "import",
            file.to_str().unwrap(),
            "--printer",
            "elegoo-mars-3-pro",
        ],
    );
    assert!(named.status.success(), "{}", stderr(&named));
}

#[test]
fn a_profile_file_without_a_kind_is_told_to_say_what_it_holds() {
    let dir = resin_dir("import-no-kind");
    let file = dir.join("resins").join("my-grey.toml");
    let refused = encrust_with_profiles(&dir, &["profiles", "import", file.to_str().unwrap()]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(stderr(&refused).contains("--kind"), "{}", stderr(&refused));
}

fn foreign_resin(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../printer-profiles/tests/fixtures")
        .join(name)
}

#[test]
fn a_cfgx_resin_is_imported_onto_the_machine_it_names() {
    let dir = resin_dir("import-cfgx");
    let file = foreign_resin("saturn-4-ultra.cfgx");

    let output = encrust_with_profiles(&dir, &["profiles", "import", file.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    let said = stdout(&output);
    assert!(said.contains("set up on elegoo-saturn-4-ultra"), "{said}");
    let id = said
        .split("imported as ")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .expect("the new id is said");
    let shown = stdout(&encrust_with_profiles(
        &dir,
        &["profiles", "show", id, "--kind", "resin"],
    ));
    assert!(shown.contains("light_pwm = 211"), "{shown}");
}

#[test]
fn a_material_ini_naming_no_machine_needs_one_named() {
    let dir = resin_dir("import-ini");
    let file = foreign_resin("saved-configuration.ini");

    let refused = encrust_with_profiles(&dir, &["profiles", "import", file.to_str().unwrap()]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        stderr(&refused).contains("--printer"),
        "{}",
        stderr(&refused)
    );

    let named = encrust_with_profiles(
        &dir,
        &[
            "profiles",
            "import",
            file.to_str().unwrap(),
            "--printer",
            "elegoo-saturn-4-ultra",
        ],
    );
    assert!(named.status.success(), "{}", stderr(&named));
}
