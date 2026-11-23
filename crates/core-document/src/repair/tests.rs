use core_edit::Edit;
use core_geometry::MendMethod;
use core_geometry::{Scalar, diagnose, signed_volume};
use core_volume::Cancel;

use super::*;
use crate::apply::apply;
use crate::fixtures::{SIDE_MM, cube, cube_with_hole, cube_with_turned_face, cubes_with_plug};

/// What f32 sums over a cube's faces drift by, mm³.
const VOLUME_MM3: Scalar = 1e-2;
const CUBE: ObjectId = ObjectId(1);

fn mend_of(object: ObjectId, asked: MendAsked) -> Edit {
    Edit::Mend {
        object,
        flip_faces: asked.flip_faces,
        drop_faces: asked.drop_faces,
        drop_shells: asked.drop_shells,
        bridge: asked.bridge,
    }
}

fn source_of(document: &Document, id: ObjectId) -> Mesh {
    let (_, meshes) = document.object(id).expect("the object is on the plate");
    (*meshes.source).clone()
}

#[test]
fn a_cube_with_one_face_turned_is_closed_and_whole_again_once_that_face_is_flipped() {
    let mut document = Document::from_project(cube_with_turned_face(5));
    assert_ne!(
        diagnose(&source_of(&document, CUBE)).unbalanced_edges,
        0,
        "a turned face unbalances its edges"
    );
    let asked = MendAsked {
        flip_faces: vec![5],
        ..MendAsked::default()
    };

    let applied = apply(&mut document, &mend_of(CUBE, asked), Cancel::never())
        .expect("face 5 is on the cube");

    let mesh = source_of(&document, CUBE);
    let found = diagnose(&mesh);
    assert!(
        found.is_closed() && found.unbalanced_edges == 0,
        "{found:?}"
    );
    let volume = signed_volume(&mesh);
    let wanted = SIDE_MM.powi(3);
    assert!(
        (volume - wanted).abs() < VOLUME_MM3,
        "the cube holds {wanted} mm³, got {volume}"
    );
    assert!(applied.added.is_empty() && applied.removed.is_empty());
    assert_eq!(
        applied.report,
        Report::Mended(Mended {
            faces_flipped: 1,
            ..Mended::default()
        })
    );
}

#[test]
fn face_thirteen_is_no_face_of_a_cube_of_twelve_and_leaves_the_cube() {
    let mut document = Document::from_project(cube());
    assert_eq!(source_of(&document, CUBE).faces.len(), 12);
    let asked = MendAsked {
        flip_faces: vec![13],
        ..MendAsked::default()
    };

    let error = apply(&mut document, &mend_of(CUBE, asked), Cancel::never())
        .expect_err("a cube has faces 0 to 11");

    assert_eq!(
        error,
        ApplyError::NoSuchFace {
            object: CUBE,
            number: 13
        }
    );
    assert_eq!(source_of(&document, CUBE).faces.len(), 12);
}

#[test]
fn a_second_shell_is_no_shell_of_one_cube() {
    let mut document = Document::from_project(cube());
    let asked = MendAsked {
        drop_shells: vec![2],
        ..MendAsked::default()
    };

    let error = apply(&mut document, &mend_of(CUBE, asked), Cancel::never())
        .expect_err("a cube is one shell");

    assert_eq!(
        error,
        ApplyError::NoSuchShell {
            object: CUBE,
            number: 2
        }
    );
}

#[test]
fn a_bridge_of_an_edge_to_itself_is_refused_as_failed() {
    let mut document = Document::from_project(cube());
    let asked = MendAsked {
        bridge: vec![[[0, 1], [1, 0]]],
        ..MendAsked::default()
    };

    let error = apply(&mut document, &mend_of(CUBE, asked), Cancel::never())
        .expect_err("an edge cannot be bridged to itself");

    assert!(
        matches!(error, ApplyError::Failed { object: CUBE, .. }),
        "{error:?}"
    );
}

#[test]
fn a_cube_with_a_face_removed_is_closed_by_repair() {
    let mut document = Document::from_project(cube_with_hole(3));
    assert!(!diagnose(&source_of(&document, CUBE)).is_closed());
    let edit = Edit::Repair {
        object: CUBE,
        method: RepairMethod::Mend(MendMethod::Repair),
        seam_tolerance_mm: None,
        smallest_shell_mm2: None,
    };

    let applied = apply(&mut document, &edit, Cancel::never()).expect("a hole is mended");

    let mesh = source_of(&document, CUBE);
    assert!(diagnose(&mesh).is_closed());
    let volume = signed_volume(&mesh);
    let wanted = SIDE_MM.powi(3);
    assert!(
        (volume - wanted).abs() < VOLUME_MM3,
        "the cube holds {wanted} mm³, got {volume}"
    );
    assert!(matches!(
        applied.report,
        Report::Repaired { wrapped: false, .. }
    ));
}

#[test]
fn a_repair_asked_to_stop_is_cancelled_and_leaves_the_cube() {
    let mut document = Document::from_project(cube_with_hole(3));
    let stopped = || true;
    let edit = Edit::Repair {
        object: CUBE,
        method: RepairMethod::Wrap,
        seam_tolerance_mm: None,
        smallest_shell_mm2: None,
    };

    let error = apply(&mut document, &edit, Cancel::when(&stopped))
        .expect_err("a stopped run does not finish");

    assert_eq!(error, ApplyError::Cancelled);
    assert!(!diagnose(&source_of(&document, CUBE)).is_closed());
}

#[test]
fn a_reshaped_object_keeps_its_id_and_the_plugs_laid_for_it() {
    let mut document = Document::from_project(cubes_with_plug());
    let asked = MendAsked {
        drop_faces: vec![0],
        ..MendAsked::default()
    };

    let applied = apply(&mut document, &mend_of(ObjectId(2), asked), Cancel::never())
        .expect("face 0 is on the cube");

    assert!(applied.removed.is_empty(), "the plug stays");
    assert!(document.object(ObjectId(3)).is_some());
    assert_eq!(source_of(&document, ObjectId(2)).faces.len(), 11);
    let project = document.into_project();
    assert_eq!(
        project.manifest.objects[2].plug_of,
        Some(1),
        "it still names the cube"
    );
}

#[test]
fn a_repair_takes_the_lengths_asked_and_the_windows_where_unset() {
    let window = RepairSettings::default().mend;

    let asked = repair_settings(Some(0.25), None).mend;

    assert!(
        (asked.seam_tolerance_mm - 0.25).abs() < 1e-9,
        "the seam asked, 0.25 mm"
    );
    assert!(
        (asked.smallest_shell_mm2 - window.smallest_shell_mm2).abs() < 1e-9,
        "the window's smallest shell, {} mm²",
        window.smallest_shell_mm2
    );
}
