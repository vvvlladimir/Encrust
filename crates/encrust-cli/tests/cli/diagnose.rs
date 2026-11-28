//! `diagnose`: a failed print's symptoms read against the `--json` report of its file.

use std::fs;

use crate::support::*;

#[test]
fn a_cracked_hollow_cube_with_no_drain_is_put_on_its_trapped_resin() {
    let model = write_box_stl("diagnose-crack", 16.0, 12, 0);
    let panel = test_panel();
    let checked = estimate(&[
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "--check-drainage",
        "--json",
    ]);
    assert!(checked.status.success() || checked.status.code() == Some(3));
    let report = output_file("diagnose-crack.json");
    fs::write(&report, &checked.stdout).expect("a writable file");

    let output = encrust(&[
        "diagnose",
        report.to_str().unwrap(),
        "--symptom",
        "resin-crack",
        "--symptom",
        "off-plate",
        "--json",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let diagnosis = json(&output);
    let crack = &diagnosis["symptoms"][0];
    assert_eq!(crack["symptom"], "resin-crack");
    assert_eq!(crack["causes"][0]["step"], "drains");
    assert!(
        crack["causes"][0]["seen"]
            .as_str()
            .is_some_and(|seen| seen.contains("no way out")),
        "{crack}"
    );
    assert_eq!(diagnosis["symptoms"][1]["symptom"], "off-plate");

    let told = stdout(&encrust(&[
        "diagnose",
        report.to_str().unwrap(),
        "--symptom",
        "resin-crack",
    ]));
    assert!(told.contains("try: set_drains"), "{told}");
}

#[test]
fn a_report_that_is_not_json_is_refused() {
    let report = output_file("diagnose-not-json.json");
    fs::write(&report, "layers: 12").expect("a writable file");
    let output = encrust(&["diagnose", report.to_str().unwrap(), "--symptom", "hole"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("not a --json report"),
        "{}",
        stderr(&output)
    );
}
