//! `encrust batch`: one file and one report per model, and what a failure among them
//! does to the run.

use std::fs;

use crate::support::*;

#[test]
fn a_directory_of_models_is_sliced_one_file_and_one_report_each() {
    let input = batch_input("stack");
    let out = output_dir("batch-stack");

    let output = encrust(&[
        "batch",
        input.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "-o",
        out.to_str().expect("ascii path"),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));

    for model in ["alpha", "beta"] {
        let sliced = out.join(format!("{model}.goo"));
        assert!(sliced.is_file(), "no {}", sliced.display());
        let report = out.join(format!("{model}.json"));
        let text =
            fs::read_to_string(&report).unwrap_or_else(|_| panic!("no {}", report.display()));
        assert!(text.contains("\"status\": \"ok\""), "{text}");
        assert!(text.contains("\"layers\""), "{text}");
    }
    assert!(
        !out.join("notes.goo").exists(),
        "a text file in the directory is not a model"
    );

    let summary = fs::read_to_string(out.join("batch.json")).expect("the batch report is written");
    assert!(summary.contains("\"models\": 2"), "{summary}");
    assert!(summary.contains("\"ok\": 2"), "{summary}");
    assert!(summary.contains("\"failed\": 0"), "{summary}");
}

#[test]
fn a_model_that_cannot_be_loaded_is_reported_and_the_rest_still_run() {
    let input = batch_input("broken");
    fs::write(input.join("broken.stl"), b"not an stl at all").expect("write the bad model");
    let out = output_dir("batch-broken");

    let output = encrust(&[
        "batch",
        input.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "-o",
        out.to_str().expect("ascii path"),
    ]);
    assert_eq!(
        output.status.code(),
        Some(4),
        "a batch in which a model failed says so in its exit code: {}",
        stderr(&output)
    );

    assert!(out.join("alpha.goo").is_file(), "the good ones still wrote");
    let summary = fs::read_to_string(out.join("batch.json")).expect("the batch report is written");
    assert!(summary.contains("\"failed\": 1"), "{summary}");
    assert!(summary.contains("\"ok\": 2"), "{summary}");

    let broken = fs::read_to_string(out.join("broken.json")).expect("the bad model reports too");
    assert!(broken.contains("\"status\": \"failed\""), "{broken}");
    assert!(broken.contains("\"error\""), "{broken}");
}

#[test]
fn an_unclean_model_fails_a_strict_batch_with_3() {
    let input = batch_input("strict");
    let open = write_box_stl("strict-batch-open", 10.0, 10, 0);
    fs::copy(&open, input.join("open.stl")).expect("copy the open box in");
    let out = output_dir("batch-strict");

    let output = encrust(&[
        "batch",
        input.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "-o",
        out.to_str().expect("ascii path"),
        "--strict",
    ]);
    assert_eq!(
        output.status.code(),
        Some(3),
        "an open box has to fail --strict: {}",
        stderr(&output)
    );
}

/// Whether a cube needs holding is settled in `supports.rs`; what this pins down is that
/// the flag reaches the pipeline and the run says what it stood.
#[test]
fn asking_for_supports_reports_what_was_stood() {
    let model = write_box_stl("supported", 10.0, 12, 0);
    let held = output_dir("supports-held");

    let with = slice(&[
        model.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "--supports",
        "medium",
        "-o",
        held.to_str().expect("ascii path"),
    ]);
    assert!(with.status.success(), "{}", stderr(&with));
    assert!(
        stdout(&with).contains("Supports"),
        "the run says what it stood: {}",
        stdout(&with)
    );
}

#[test]
fn a_batch_answers_json_with_its_summary() {
    let input = batch_input("json");
    let out = output_dir("batch-json");
    let output = encrust(&[
        "batch",
        input.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "-o",
        out.to_str().expect("ascii path"),
        "--json",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let summary = json(&output);
    assert_eq!(summary["schema"], 1);
    assert_eq!(summary["models"], 2);
}
