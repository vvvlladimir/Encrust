//! Changing a model by hand from the command line: listing its shells, dropping and turning
//! what is numbered, bridging two open edges.

use crate::support::*;

fn two_boxes_apart(name: &str) -> std::path::PathBuf {
    write_boxes_stl(name, &[([0.0; 3], [1.0; 3]), ([2.0; 3], [3.0; 3])])
}

fn inspect(args: &[&str]) -> std::process::Output {
    encrust(&[&["inspect"], args].concat())
}

#[test]
fn shells_are_listed_from_one_with_their_faces_area_and_whether_they_close() {
    let path = two_boxes_apart("listed");
    let text = stdout(&inspect(&[path.to_str().unwrap(), "--shells"]));
    assert_eq!(field(&text, "shell 1"), "12 faces, 6.00 mm^2, closed");
    assert_eq!(field(&text, "shell 2"), "12 faces, 6.00 mm^2, closed");
}

#[test]
fn a_shell_dropped_by_its_number_leaves_the_other() {
    let path = two_boxes_apart("dropped");
    let output = inspect(&[path.to_str().unwrap(), "--drop-shells", "2"]);
    let text = stdout(&output);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(field(&text, "closed"), "yes (1 shell, euler 2)");
    assert_eq!(field(&text, "by hand"), "dropped 12 faces, dropped 1 shell");
}

#[test]
fn a_shell_named_twice_is_said_dropped_once() {
    let path = two_boxes_apart("dropped-twice");
    let text = stdout(&inspect(&[path.to_str().unwrap(), "--drop-shells", "2,2"]));
    assert_eq!(field(&text, "by hand"), "dropped 12 faces, dropped 1 shell");
}

#[test]
fn faces_dropped_by_number_open_the_model_and_are_said() {
    let path = write_box_stl("faces-dropped", 10.0, 12, 0);
    let text = stdout(&inspect(&[path.to_str().unwrap(), "--drop-faces", "0,1"]));
    assert_eq!(field(&text, "by hand"), "dropped 2 faces");
    assert_eq!(field(&text, "triangles"), "10");
}

#[test]
fn a_face_turned_by_hand_is_said_and_kept_turned() {
    let path = write_box_stl("turned", 10.0, 12, 0);
    let output = encrust(&[
        "--json",
        "inspect",
        path.to_str().unwrap(),
        "--flip-faces",
        "4",
    ]);
    let report = json(&output);
    assert_eq!(report["repair"]["by_hand"]["faces_turned"], 1, "{report}");
    assert_eq!(
        report["repair"]["closed"], false,
        "a turned face leaves its edges unpaired"
    );
}

#[test]
fn a_face_or_shell_past_the_last_is_refused_by_name() {
    let path = write_box_stl("past", 10.0, 12, 0);
    let faces = inspect(&[path.to_str().unwrap(), "--drop-faces", "12"]);
    assert!(!faces.status.success());
    assert!(
        stderr(&faces).contains("there is no face 12"),
        "{}",
        stderr(&faces)
    );
    let shells = inspect(&[path.to_str().unwrap(), "--drop-shells", "2"]);
    assert!(
        stderr(&shells).contains("there is no shell 2"),
        "{}",
        stderr(&shells)
    );
}

#[test]
fn bridging_an_edge_two_faces_share_is_refused() {
    let path = write_box_stl("shared", 10.0, 12, 0);
    let output = inspect(&[path.to_str().unwrap(), "--bridge", "0-1:2-3"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--bridge 0-1:2-3"),
        "{}",
        stderr(&output)
    );
}
