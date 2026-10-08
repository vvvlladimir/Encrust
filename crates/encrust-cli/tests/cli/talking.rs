//! What a run says and with which exit code: the JSON document, the quiet flag, the
//! progress bar and a wrong invocation.

use std::fs;
use std::process::Command;

use crate::support::*;

#[test]
fn json_puts_one_document_on_stdout_and_the_logs_on_stderr() {
    let path = write_box_stl("json-slice", 10.0, 12, 0);
    let out = output_file("json-slice.goo");
    let output = encrust(&[
        "--json",
        "slice",
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));

    let report = json(&output);
    assert_eq!(report["schema"], 1);
    assert_eq!(report["status"], "ok");
    assert_eq!(report["slicing"]["layers"], 10, "a 10 mm cube at 1 mm");
    assert!(report["cured"].is_object(), "a written file was measured");
}

#[test]
fn json_reports_a_failure_as_a_document_too() {
    let output = encrust(&["slice", "model.gcode", "--json"]);

    assert_eq!(output.status.code(), Some(1));
    let failure = json(&output);
    assert_eq!(failure["schema"], 1);
    assert!(
        failure["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("gcode")),
        "{failure}"
    );
    assert!(
        stderr(&output).contains("error:"),
        "the text still goes to stderr"
    );
}

#[test]
fn json_reports_wrong_arguments_as_a_document_too() {
    let output = encrust(&["slice", "model.stl", "--json", "--no-such-flag"]);

    assert_eq!(output.status.code(), Some(2));
    let failure = json(&output);
    assert_eq!(failure["schema"], 1);
    assert!(
        failure["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("--no-such-flag")),
        "{failure}"
    );
    assert!(
        stderr(&output).contains("Usage"),
        "the usage still goes to stderr"
    );
}

#[test]
fn quiet_info_and_profiles_list_print_nothing_but_still_check_the_file() {
    let path = write_box_stl("quiet-info", 10.0, 12, 0);
    let out = output_file("quiet-info.goo");
    let sliced = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "2",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(sliced.status.success(), "{}", stderr(&sliced));

    let info = encrust(&["-q", "info", out.to_str().unwrap()]);
    assert!(info.status.success(), "{}", stderr(&info));
    assert_eq!(stdout(&info), "", "--quiet prints nothing but errors");

    let listing = encrust(&["-q", "profiles", "list"]);
    assert!(listing.status.success(), "{}", stderr(&listing));
    assert_eq!(stdout(&listing), "");
}

#[test]
fn inspect_info_and_profiles_answer_json_as_well() {
    let path = write_box_stl("json-inspect", 10.0, 12, 0);
    let inspected = json(&encrust(&["inspect", path.to_str().unwrap(), "--json"]));
    assert_eq!(inspected["model"]["faces"], 12);
    assert!(
        inspected.get("output").is_none(),
        "nothing is written by inspect"
    );

    let out = output_file("json-info.goo");
    let sliced = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "2",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(sliced.status.success(), "{}", stderr(&sliced));
    let info = json(&encrust(&["info", out.to_str().unwrap(), "--json"]));
    assert_eq!(info["file"]["format"], "goo");
    assert_eq!(info["stack"]["layers_decoded"], 5);

    let listing = json(&encrust(&["profiles", "list", "--json"]));
    let printers = listing["printers"].as_array().expect("a list of printers");
    assert!(
        printers
            .iter()
            .any(|printer| printer["id"] == "elegoo-mars-4-ultra")
    );
}

#[test]
fn a_flag_no_subcommand_takes_is_exit_code_2() {
    assert_eq!(encrust(&["slice", "--no-such-flag"]).status.code(), Some(2));
    assert_eq!(
        encrust(&[]).status.code(),
        Some(2),
        "a subcommand is required"
    );
}

#[test]
fn slice_refuses_a_directory_and_points_at_batch() {
    let input = batch_input("not-a-slice");
    let output = slice(&[input.to_str().expect("ascii path")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("encrust batch"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn piped_output_draws_no_progress_bar() {
    let path = write_box_stl("no-bar", 10.0, 12, 0);
    let out = output_file("no-bar.goo");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        !stderr(&output).contains("layers,"),
        "stderr is a pipe here, not a terminal: {}",
        stderr(&output)
    );
}

#[test]
fn a_printer_that_does_not_answer_fails_with_one_json_document() {
    let file = output_dir("printer-send").with_extension("goo");
    fs::write(&file, b"not a real stack").expect("the temporary directory is writable");
    let output = Command::new(env!("CARGO_BIN_EXE_encrust"))
        .env("ENCRUST_PRUSALINK_KEY", "k3y")
        .args(["printer", "send", file.to_str().unwrap(), "127.0.0.1:1"])
        .args(["--prusalink", "--json"])
        .output()
        .expect("the encrust binary runs");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let document = json(&output);
    assert!(
        document["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("127.0.0.1:1")),
        "{document}"
    );
    fs::remove_file(file).ok();
}
