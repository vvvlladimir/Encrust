use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use core_edit::Edit;
use core_geometry::{Scalar, signed_volume};

use super::*;
use crate::apply::apply;
use crate::fixtures::{SIDE_MM, cube};

const CUBE: ObjectId = ObjectId(1);
/// How far a 0.5 mm relief may move a face, with a lattice's rounding on top, mm.
const BOUNDS_MM: Scalar = 0.6;

/// A black-and-white checker of 2 by 2 squares as a 24-bit BMP, in a file of its own.
fn checker_picture(name: &str) -> PathBuf {
    const SIDE_PX: u32 = 16;
    let mut bytes = Vec::new();
    let pixels = SIDE_PX * SIDE_PX * 3;
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(54 + pixels).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&54u32.to_le_bytes());
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&SIDE_PX.to_le_bytes());
    bytes.extend_from_slice(&SIDE_PX.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&24u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 24]);
    for row in 0..SIDE_PX {
        for column in 0..SIDE_PX {
            let white = (row / 8 + column / 8) % 2 == 0;
            bytes.extend_from_slice(&[if white { 255 } else { 0 }; 3]);
        }
    }
    let path = std::env::temp_dir().join(format!("{}-{name}.bmp", std::process::id()));
    std::fs::write(&path, bytes).expect("the temp folder takes a file");
    path
}

fn relief_of(image: PathBuf) -> Edit {
    Edit::Relief {
        object: CUBE,
        depth_mm: 0.5,
        image,
        tile_mm: 5.0,
        angle_deg: 0.0,
        invert: false,
        symmetric: false,
        smoothing_px: 0.0,
        hold_bottom_deg: 0.0,
        hold_top_deg: 0.0,
        precision: 0.5,
    }
}

#[test]
fn a_checker_pressed_into_a_cube_changes_its_volume_and_keeps_its_bounds() {
    let mut document = Document::from_project(cube());
    let picture = checker_picture("checker");

    let applied = apply(&mut document, &relief_of(picture.clone()), Cancel::never())
        .expect("a readable picture presses into a cube");
    std::fs::remove_file(&picture).expect("the picture is there to remove");

    let (_, meshes) = document.object(CUBE).expect("the cube stays on the plate");
    let volume = signed_volume(&meshes.source);
    let wanted = SIDE_MM.powi(3);
    assert!(
        (volume - wanted).abs() > 1.0,
        "a 0.5 mm checker moves more than 1 mm³ of the cube's {wanted}, got {volume}"
    );
    let bounds = meshes.source.aabb().expect("the pressed cube has faces");
    for axis in 0..3 {
        assert!(
            bounds.mins[axis].abs() <= BOUNDS_MM
                && (bounds.maxs[axis] - SIDE_MM).abs() <= BOUNDS_MM,
            "the bounds stay within {BOUNDS_MM} mm of 0..{SIDE_MM}: {bounds:?}"
        );
    }
    assert!(matches!(applied.report, Report::Relieved { .. }));
}

#[test]
fn a_picture_that_is_not_there_is_refused() {
    let mut document = Document::from_project(cube());
    let missing = std::env::temp_dir().join("encrust-no-such-picture.bmp");

    let error = apply(&mut document, &relief_of(missing), Cancel::never())
        .expect_err("there is no such file");

    assert!(matches!(error, ApplyError::BadPicture { .. }), "{error:?}");
}

#[test]
fn a_stopped_relief_is_cancelled() {
    let mut document = Document::from_project(cube());
    let picture = checker_picture("stopped");
    let stopped = || true;

    let error = apply(
        &mut document,
        &relief_of(picture.clone()),
        Cancel::when(&stopped),
    )
    .expect_err("a stopped run does not finish");
    std::fs::remove_file(&picture).expect("the picture is there to remove");

    assert_eq!(error, ApplyError::Cancelled);
}

fn hollow_cube() -> Document {
    let mut document = Document::from_project(cube());
    let walled = Edit::Hollow {
        object: CUBE,
        settings: core_engine::project::HollowState {
            thickness_mm: 2.0,
            mode: core_volume::HollowMode::Internal,
            precision: 1.0,
            infill_on: false,
            infill: core_volume::InfillSettings::default(),
            open_floor: false,
            solid_under_mm: None,
        },
        keep_solid: Vec::new(),
        keep_solid_reach_mm: None,
    };
    apply(&mut document, &walled, Cancel::never()).expect("the cube has room for a 2 mm wall");
    document
}

#[test]
fn a_relief_stopped_while_hollowing_again_leaves_the_cube_as_it_was() {
    let mut document = hollow_cube();
    let (state, meshes) = document.object(CUBE).expect("the cube is there");
    let (transform, source) = (state.transform, Arc::clone(&meshes.source));
    let picture = checker_picture("stopped-again");
    let progress = core_volume::Progress::default();
    let pressed = std::sync::atomic::AtomicBool::new(false);
    let at_the_hollow = || match progress.read() {
        Some((core_volume::Step::Preparing, ..)) => pressed.load(Relaxed),
        Some(_) => {
            pressed.store(true, Relaxed);
            false
        }
        None => false,
    };

    let error = apply(
        &mut document,
        &relief_of(picture.clone()),
        Cancel::when(&at_the_hollow).reporting(&progress),
    )
    .expect_err("the run is stopped once the hollow is built again");
    std::fs::remove_file(&picture).expect("the picture is there to remove");

    assert_eq!(error, ApplyError::Cancelled);
    let (after, meshes) = document.object(CUBE).expect("the cube stays");
    assert!(
        Arc::ptr_eq(&meshes.source, &source) && after.transform == transform,
        "a stopped relief leaves the cube's surface and place as they were"
    );
    assert!(
        after.hollow.built.is_some() && meshes.shell.is_some(),
        "the cube keeps its shell"
    );
}

#[test]
fn a_hollow_cube_pressed_is_hollowed_again_keeping_its_holes() {
    let mut document = hollow_cube();
    let drilled = Edit::Drain {
        object: CUBE,
        at_mm: vec![core_geometry::Vec3::new(
            SIDE_MM / 2.0,
            SIDE_MM / 2.0,
            SIDE_MM,
        )],
        size: core_volume::HoleSize::default(),
        kind: core_volume::HoleKind::default(),
        auto: false,
        vent: false,
    };
    apply(&mut document, &drilled, Cancel::never()).expect("the top of the hollow cube drains");
    let picture = checker_picture("hollow");

    apply(&mut document, &relief_of(picture.clone()), Cancel::never())
        .expect("a readable picture presses into a hollow cube");
    std::fs::remove_file(&picture).expect("the picture is there to remove");

    let (state, meshes) = document.object(CUBE).expect("the cube stays on the plate");
    assert!(
        state.hollow.built.is_some() && meshes.shell.is_some(),
        "the window's Relief hollows the pressed model again, so the edit does"
    );
    assert_eq!(
        state.hollow.drains.len(),
        1,
        "the hole is carried onto the new surface"
    );
}
