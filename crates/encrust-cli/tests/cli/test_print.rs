//! `encrust test-print`: the window's test prints written as STL files.

use crate::support::*;

#[test]
fn the_exposure_check_is_written_into_a_directory_under_its_own_name() {
    let dir = output_dir("test-print");
    std::fs::create_dir_all(&dir).expect("the temp dir is writable");
    let output = encrust(&[
        "--json",
        "test-print",
        "exposure",
        "-o",
        dir.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let path = dir.join("exposure-check.stl");
    assert_eq!(json(&output)["path"], path.to_str().unwrap());
    let read = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
    ]);
    assert!(
        read.status.success(),
        "the STL reads back: {}",
        stderr(&read)
    );
}

#[test]
fn a_test_print_that_is_not_one_is_a_usage_error() {
    let output = encrust(&["test-print", "benchy"]);
    assert_eq!(output.status.code(), Some(2));
}
