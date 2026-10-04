//! A project's bytes into a sliced file's bytes, the way a browser hands them over.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_engine::EngineError;
use core_volume::VolumeError;
use web_engine::{WebError, slice_project};

/// A 20 mm cube hollowed to a 2 mm wall, standing on one support, on a 400 x 400 panel.
fn fixture() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cube.encrust");
    std::fs::read(path).expect("the fixture is checked in")
}

#[test]
fn a_project_slices_into_the_container_it_names() {
    let sliced = slice_project(&fixture(), 256 << 20, 1, 0).expect("the fixture slices");

    assert_eq!(sliced.extension, "goo");
    // Every `.goo` opens with its version string, "V3.0"; see docs/formats/goo.md.
    assert_eq!(&sliced.bytes[..4], b"V3.0");
}

#[test]
fn bytes_that_are_not_a_project_are_refused() {
    let error = slice_project(b"solid cube", 256 << 20, 1, 0).expect_err("not a zip");
    assert!(matches!(error, WebError::Project(_)));
}

#[test]
fn a_cavity_that_cannot_be_built_fails_the_run() {
    // Under a byte of budget no lattice fits, and the engine says which model it was.
    let error = slice_project(&fixture(), 1, 1, 0).expect_err("nothing fits in one byte");
    assert!(
        matches!(
            error,
            WebError::Engine(EngineError::Hollow {
                source: VolumeError::TooFine { .. },
                ..
            })
        ),
        "{error:?}"
    );
}
