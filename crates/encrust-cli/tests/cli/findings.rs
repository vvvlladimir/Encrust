//! The findings of `--json` located in plate millimetres, the frame `--drain-at` takes, so
//! whatever reads the report can act on each one (ADR 0394).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::support::*;

/// A box 4 x 4 mm, standing on nothing from 5 mm up and centred at (14, 15), beside a
/// block on the plate that holds the stack up.
fn island_beside_a_block(name: &str) -> PathBuf {
    write_boxes_stl(
        name,
        &[
            ([2.0, 2.0, 0.0], [8.0, 8.0, 10.0]),
            ([12.0, 13.0, 5.0], [16.0, 17.0, 8.0]),
        ],
    )
}

/// The tests' panel as a panel that takes its masks mirrored along X.
fn mirrored_panel() -> PathBuf {
    let plain = fs::read_to_string(test_panel()).expect("the test panel is readable");
    let mirrored = plain.replace("mirror = \"normal\"", "mirror = \"lcd-mirror\"");
    assert_ne!(plain, mirrored, "the test panel states its mirroring");
    let path = std::env::temp_dir().join("encrust-cli-tests/mirrored-panel.toml");
    fs::create_dir_all(path.parent().expect("a temp file has a directory")).expect("temp dir");
    fs::write(&path, mirrored).expect("write the mirrored panel");
    path
}

fn json_of(args: &[&str]) -> Value {
    let output = encrust(args);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        stderr(&output)
    );
    json(&output)
}

fn point(value: &Value) -> [f64; 3] {
    let at: Vec<f64> = value
        .as_array()
        .unwrap_or_else(|| panic!("not a point: {value}"))
        .iter()
        .map(|axis| axis.as_f64().expect("a coordinate is a number"))
        .collect();
    at.try_into()
        .unwrap_or_else(|_| panic!("not three coordinates: {value}"))
}

fn near(found: [f64; 3], expected: [f64; 3], tolerance: f64) -> bool {
    found
        .iter()
        .zip(expected)
        .all(|(found, expected)| (found - expected).abs() <= tolerance)
}

/// The one island `cured` names.
fn island_of(cured: &Value) -> [f64; 3] {
    let risks = cured["risks"].as_array().expect("cured lists its risks");
    assert_eq!(
        risks.len() as u64,
        cured["islands"].as_u64().expect("cured counts islands"),
        "one entry per island counted: {cured}"
    );
    let island = risks
        .iter()
        .find(|risk| risk["kind"] == "island")
        .unwrap_or_else(|| panic!("no island in {cured}"));
    assert_eq!(island["area_mm2"].as_f64().map(f64::round), Some(16.0));
    point(&island["at_mm"])
}

fn slice_island(name: &str, panel: &Path) -> Value {
    let model = island_beside_a_block(name);
    let out = output_dir(name);
    json_of(&[
        "slice",
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.5",
        "-o",
        out.to_str().unwrap(),
        "--json",
    ])
}

#[test]
fn a_sealed_cavity_is_located_inside_the_hollowed_cube() {
    let model = write_box_stl("located-cavity", 16.0, 12, 0);
    let out = output_dir("located-cavity");
    let report = json_of(&[
        "slice",
        model.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "--hollow",
        "2",
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]);

    let trapped = report["slicing"]["trapped"]
        .as_array()
        .expect("slicing lists the trapped pockets");
    assert_eq!(trapped.len(), 1, "one sealed cavity: {trapped:?}");
    let pocket = &trapped[0];
    let at = point(&pocket["at_mm"]);
    assert!(
        at.iter().all(|axis| (2.0..=14.0).contains(axis)),
        "the hole goes inside the cavity, 2..14 mm on every axis, found {at:?}"
    );
    let volume = pocket["volume_mm3"]
        .as_f64()
        .expect("a pocket holds a volume");
    assert!(
        (volume - 1728.0).abs() / 1728.0 < 0.03,
        "a 12 mm cavity holds 1728 mm^3, found {volume}"
    );
    let (min, max) = (
        point(&pocket["bounds_mm"]["min"]),
        point(&pocket["bounds_mm"]["max"]),
    );
    assert!(
        near(min, [2.0; 3], 0.2) && near(max, [14.0; 3], 0.2),
        "the pocket's box is the cavity's, 2..14 mm, found {min:?}..{max:?}"
    );
}

#[test]
fn an_island_is_located_at_its_plate_point_whichever_way_the_panel_is_mirrored() {
    for (name, panel) in [
        ("located-island-plain", test_panel()),
        ("located-island-mirrored", mirrored_panel()),
    ] {
        let island = island_of(&slice_island(name, &panel)["cured"]);
        assert!(
            near(island, [14.0, 15.0, 5.0], 0.1),
            "{name}: the island cures over (14, 15) mm from the floor at 5 mm, found {island:?}"
        );
    }
}

#[test]
fn an_estimate_locates_the_risks_it_counts() {
    let model = island_beside_a_block("located-estimate");
    let report = json_of(&[
        "estimate",
        model.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--json",
    ]);

    let island = island_of(&report["print"]["cured"]);
    assert!(
        near(island, [14.0, 15.0, 5.0], 0.1),
        "the island cures over (14, 15) mm from the floor at 5 mm, found {island:?}"
    );
}

#[test]
fn contacts_nothing_can_stand_under_are_located_under_what_they_hold() {
    // A 10 mm slab 2 mm over the middle of a 40 mm block, by a profile that may not stand
    // on the model: nothing can reach the plate from under the slab.
    let model = write_boxes_stl(
        "located-unheld",
        &[
            ([0.0, 0.0, 0.0], [40.0, 40.0, 5.0]),
            ([15.0, 15.0, 7.0], [25.0, 25.0, 9.0]),
        ],
    );
    let profile = std::env::temp_dir().join("encrust-cli-tests/no-landing-on-the-model.toml");
    fs::write(&profile, "name = \"No landing\"\nland_on_model = false\n")
        .expect("write the support profile");
    let out = output_dir("located-unheld");
    let report = json_of(&[
        "slice",
        model.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--support-profile",
        profile.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]);

    let supports = &report["supports"];
    let unheld = supports["no_room_at_mm"]
        .as_array()
        .expect("supports lists the contacts with no room");
    assert!(!unheld.is_empty(), "the slab cannot be held: {supports}");
    assert_eq!(
        unheld.len() as u64,
        supports["no_room_for"]
            .as_u64()
            .expect("supports counts them"),
        "one point per contact counted"
    );
    for contact in unheld {
        let [x, y, _] = point(contact);
        assert!(
            (15.0..=25.0).contains(&x) && (15.0..=25.0).contains(&y),
            "a contact with no room lies under the slab, 15..25 mm across, found {contact}"
        );
    }
}
