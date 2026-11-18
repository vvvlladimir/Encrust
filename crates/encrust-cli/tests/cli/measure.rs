//! Measuring a model: straight between two points, and over its surface when asked.

use crate::support::*;

#[test]
fn the_top_of_a_cube_to_its_bottom_is_its_side_straight_and_twice_that_over_the_surface() {
    let path = write_box_stl("measured", 10.0, 12, 0);
    let output = encrust(&[
        "measure",
        path.to_str().unwrap(),
        "--from",
        "5,5,10",
        "--to",
        "5,5,0",
        "--along-surface",
    ]);
    let text = stdout(&output);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(field(&text, "distance"), "10.000 mm");
    assert_eq!(field(&text, "dZ"), "-10.000 mm");
    assert_eq!(
        field(&text, "along surface"),
        "20.000 mm",
        "5 to the edge, 10 down a side, 5 in"
    );
}

#[test]
fn an_end_off_the_model_is_taken_to_its_nearest_point_of_the_surface() {
    let path = write_box_stl("measured-json", 10.0, 12, 0);
    let output = encrust(&[
        "--json",
        "measure",
        path.to_str().unwrap(),
        "--from",
        "5,5,30",
        "--to",
        "5,-4,5",
    ]);
    let document = json(&output);
    assert_eq!(document["from"], serde_json::json!([5.0, 5.0, 10.0]));
    assert_eq!(document["to"], serde_json::json!([5.0, 0.0, 5.0]));
    assert!(
        document.get("along_surface_mm").is_none(),
        "not asked for: {document}"
    );
}

#[test]
fn the_walk_over_the_surface_is_printed_end_to_end_as_json() {
    let path = write_box_stl("measured-walk", 10.0, 12, 0);
    let output = encrust(&[
        "--json",
        "measure",
        path.to_str().unwrap(),
        "--from",
        "5,5,10",
        "--to",
        "5,0,5",
        "--along-surface",
    ]);
    let document = json(&output);
    let points = document["along_surface_points"]
        .as_array()
        .expect("asked for along the surface");
    assert_eq!(points.first(), Some(&serde_json::json!([5.0, 5.0, 10.0])));
    assert_eq!(points.last(), Some(&serde_json::json!([5.0, 0.0, 5.0])));
}

#[test]
fn ends_on_two_separate_boxes_fail_with_nothing_printed() {
    let path = write_boxes_stl(
        "measured-apart",
        &[([0.0; 3], [10.0; 3]), ([20.0; 3], [30.0; 3])],
    );
    let output = encrust(&[
        "measure",
        path.to_str().unwrap(),
        "--from",
        "5,5,10",
        "--to",
        "25,25,30",
        "--along-surface",
    ]);
    assert!(!output.status.success(), "no surface joins two boxes");
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
}
