//! Combining, labelling, cutting and splitting models on the command line before they are
//! sliced, as the window's Relief and Cut tools do.

use std::path::{Path, PathBuf};
use std::process::Output;

use crate::support::*;

/// Slices `models` at 1 mm into a PNG stack with whatever flags follow.
fn shaped(name: &str, models: &[&Path], extra: &[&str]) -> (Output, PathBuf) {
    let out = output_dir(name);
    let mut args: Vec<String> = models
        .iter()
        .map(|path| path.to_str().unwrap().to_owned())
        .collect();
    args.extend(
        [
            "--profile",
            test_panel().to_str().unwrap(),
            "--layer-height",
            "1",
            "-o",
            out.to_str().unwrap(),
        ]
        .map(str::to_owned),
    );
    args.extend(extra.iter().map(|argument| (*argument).to_owned()));
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    (slice(&borrowed), out)
}

fn succeeded(output: &Output) {
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_box_less_a_smaller_one_in_its_corner_leaves_an_l_shaped_section() {
    let big = write_box_stl("model-big", 16.0, 12, 0);
    let small = write_box_stl("model-small", 10.0, 12, 0);
    let (output, out) = shaped("difference", &[&big, &small], &["--combine", "difference"]);
    succeeded(&output);
    assert!(stdout(&output).contains("Combined 2 models"));
    // 2.5 mm up, inside the 10 mm box: 256 mm² less 100, 15 600 pixels at a 0.1 mm pitch.
    let lit = lit_pixels(&out, 2);
    assert!(
        (lit as f32 - 15_600.0).abs() / 15_600.0 < 0.02,
        "a 16 mm square less a 10 mm one is 15 600 pixels, got {lit}"
    );
}

#[test]
fn a_cut_box_joined_by_dowels_is_laid_out_in_pieces() {
    let path = write_box_stl("model-cut", 16.0, 12, 0);
    let (output, _) = shaped(
        "cut",
        &[&path],
        &["--cut", "z:8", "--joint", "dowels", "--arrange"],
    );
    succeeded(&output);
    assert!(stdout(&output).contains("Cut: "), "{}", stdout(&output));
}

#[test]
fn a_cut_without_arrange_is_refused_rather_than_stacked() {
    let path = write_box_stl("model-cut-stacked", 16.0, 12, 0);
    let (output, _) = shaped("cut-stacked", &[&path], &["--cut", "z:8"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--arrange"));
}

#[test]
fn a_label_raised_on_the_top_of_a_box_stands_above_it() {
    let path = write_box_stl("model-label", 16.0, 12, 0);
    let (output, out) = shaped(
        "label",
        &[&path],
        &[
            "--label",
            "E",
            "--label-at",
            "8,8,16",
            "--label-height",
            "8",
        ],
    );
    succeeded(&output);
    assert!(stdout(&output).contains("Labelled"), "{}", stdout(&output));
    assert!(
        lit_pixels(&out, 16) > 0,
        "the letter stands 0.8 mm over the 16 mm top"
    );
}

#[test]
fn splitting_one_box_leaves_one_part() {
    let path = write_box_stl("model-split", 16.0, 12, 0);
    let (output, _) = shaped("split", &[&path], &["--split"]);
    succeeded(&output);
    assert!(
        stdout(&output).contains("Split into 1 parts"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_label_on_several_models_not_combined_is_refused() {
    let first = write_box_stl("model-label-a", 16.0, 12, 0);
    let second = write_box_stl("model-label-b", 10.0, 12, 0);
    let (output, _) = shaped(
        "label-several",
        &[&first, &second],
        &["--arrange", "--label", "E", "--label-at", "8,8,16"],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--combine"));
}
