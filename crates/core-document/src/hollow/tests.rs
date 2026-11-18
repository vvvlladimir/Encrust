use core_edit::Edit;
use core_engine::project::{ModelMeshes, load};
use core_geometry::{Scalar, signed_volume};
use core_volume::{HollowMode, InfillSettings};

use super::*;
use crate::apply::apply;
use crate::fixtures::cube_of;

const CUBE: ObjectId = ObjectId(1);
/// What the cavity lattice's rounded corners move the material of a hollowed cube by, mm³:
/// 58 at precision 0.85, which is where the lattice stops getting closer.
const LATTICE_MM3: Scalar = 80.0;

fn walled(thickness_mm: Scalar) -> Edit {
    Edit::Hollow {
        object: CUBE,
        settings: HollowState {
            thickness_mm,
            mode: HollowMode::Internal,
            precision: 0.85,
            infill_on: false,
            infill: InfillSettings::default(),
            open_floor: false,
            solid_under_mm: None,
        },
    }
}

fn shelled_of(document: &Document) -> &ModelMeshes {
    document.object(CUBE).expect("the cube stays").1
}

#[test]
fn a_cube_with_a_2_mm_wall_keeps_its_wall_and_records_it() {
    let mut document = Document::from_project(cube_of(20.0));

    let applied = apply(&mut document, &walled(2.0), Cancel::never())
        .expect("a 20 mm cube has room for a 2 mm wall");

    let meshes = shelled_of(&document);
    let shell = meshes.shell.as_ref().expect("the cube has a shell now");
    let material = signed_volume(shell);
    let wanted = 20.0_f32.powi(3) - 16.0_f32.powi(3);
    assert!(
        (material - wanted).abs() < LATTICE_MM3,
        "a 2 mm wall leaves 20³ - 16³ = {wanted} mm³ of material within {LATTICE_MM3}, got {material}"
    );
    let (state, _) = document.object(CUBE).expect("the cube stays");
    let built = state.hollow.built.as_ref().expect("the shell is recorded");
    assert_eq!(built.wall.thickness_mm, 2.0);
    assert!(
        state.own_hollow.is_some(),
        "the numbers are the object's own"
    );
    assert!(matches!(applied.report, Report::Hollowed { .. }));
}

#[test]
fn a_wall_as_thick_as_the_model_leaves_no_room() {
    let mut document = Document::from_project(cube_of(2.0));

    let error = apply(&mut document, &walled(2.0), Cancel::never())
        .expect_err("a 2 mm cube has no room inside a 2 mm wall");

    assert_eq!(error, ApplyError::NoRoom(CUBE));
    assert!(
        shelled_of(&document).shell.is_none(),
        "the cube stays solid"
    );
}

#[test]
fn a_stopped_hollow_is_cancelled_and_leaves_the_cube_solid() {
    let mut document = Document::from_project(cube_of(20.0));
    let stopped = || true;

    let error = apply(&mut document, &walled(2.0), Cancel::when(&stopped))
        .expect_err("a stopped run does not finish");

    assert_eq!(error, ApplyError::Cancelled);
    assert!(shelled_of(&document).shell.is_none());
}

#[test]
fn a_hollowed_plate_written_and_read_back_keeps_its_shell() {
    let mut document = Document::from_project(cube_of(20.0));
    apply(&mut document, &walled(2.0), Cancel::never()).expect("the cube hollows");
    let before = signed_volume(shelled_of(&document).shell.as_ref().expect("a shell"));
    let path = std::env::temp_dir().join(format!("{}-hollowed.encrust", std::process::id()));

    core_engine::project::save(&path, &document.into_project()).expect("the plate is written");
    let read = load(&path).expect("the plate reads back");
    std::fs::remove_file(&path).expect("the file is there to remove");

    let shell = read.models[0]
        .shell
        .as_ref()
        .expect("the shell is read back");
    assert_eq!(signed_volume(shell), before);
    let built = read.manifest.objects[0].hollow.built.as_ref();
    assert!(built.is_some(), "the file records the wall");
}
