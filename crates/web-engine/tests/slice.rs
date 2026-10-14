//! A project's bytes into a sliced file's bytes, the way a browser hands them over.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use web_engine::{Profiles, WebError, slice_project};

const PRINTER: &str = include_str!("fixtures/project-panel.toml");
const RESIN: &str = include_str!("fixtures/test-resin.toml");

fn profiles() -> Profiles<'static> {
    Profiles {
        printer: PRINTER,
        resin: RESIN,
    }
}

/// A 20 mm cube hollowed to a 2 mm wall, standing on one support, on a 400 x 400 panel.
fn fixture() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cube.encrust");
    std::fs::read(path).expect("the fixture is checked in")
}

#[test]
fn a_project_slices_into_the_container_it_names() {
    let sliced = slice_project(&fixture(), profiles(), 1, 0).expect("the fixture slices");

    assert_eq!(sliced.extension, "goo");
    // Every `.goo` opens with its version string, "V3.0"; see docs/formats/goo.md.
    assert_eq!(&sliced.bytes[..4], b"V3.0");
}

#[test]
fn bytes_that_are_not_a_project_are_refused() {
    let error = slice_project(b"solid cube", profiles(), 1, 0).expect_err("not a zip");
    assert!(matches!(error, WebError::Project(_)));
}

#[test]
fn a_profile_that_does_not_read_is_refused() {
    let broken = Profiles {
        printer: "name = ",
        resin: RESIN,
    };
    let error = slice_project(&fixture(), broken, 1, 0).expect_err("the printer is not TOML");
    assert!(matches!(error, WebError::Engine(_)));
}
