use core_edit::Edit;
use core_engine::project::{ModelMeshes, load};
use core_geometry::{Scalar, signed_volume};
use core_volume::{HollowMode, InfillSettings};

use super::*;
use crate::apply::apply;
use crate::fixtures::cube_of;

const CUBE: ObjectId = ObjectId(1);
/// What the cavity lattice's rounded corners move the material of a hollowed cube by, mm³:
/// 39 at precision 1.0, the finest lattice.
const LATTICE_MM3: Scalar = 50.0;

fn walled(thickness_mm: Scalar) -> Edit {
    Edit::Hollow {
        object: CUBE,
        settings: HollowState {
            thickness_mm,
            mode: HollowMode::Internal,
            precision: 1.0,
            infill_on: false,
            infill: InfillSettings::default(),
            open_floor: false,
            solid_under_mm: None,
        },
        keep_solid: Vec::new(),
        keep_solid_reach_mm: None,
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

#[test]
fn a_run_kept_solid_lays_a_ball_and_a_rod_on_and_the_object_keeps_them() {
    let mut document = Document::from_project(cube_of(20.0));
    let Edit::Hollow { settings, .. } = walled(2.0) else {
        panic!("a hollow edit");
    };
    let run = vec![
        core_geometry::Vec3::new(5.0, 10.0, 20.0),
        core_geometry::Vec3::new(15.0, 10.0, 20.0),
    ];
    let edit = Edit::Hollow {
        object: CUBE,
        settings,
        keep_solid: vec![run],
        // Sized, so the blockers leave the cavity standing; fitted ones fill a cube whole.
        keep_solid_reach_mm: Some(2.0),
    };

    apply(&mut document, &edit, Cancel::never()).expect("a 20 mm cube has room for a cavity");

    let (state, _) = document.object(CUBE).expect("the cube stays");
    assert_eq!(
        state.hollow.blockers.len(),
        2,
        "a ball at the first point and a rod on to the second"
    );
}

#[test]
fn a_hollowed_cube_scaled_twice_over_keeps_the_wall_it_was_hollowed_to() {
    let mut document = Document::from_project(cube_of(20.0));
    apply(&mut document, &walled(2.0), Cancel::never()).expect("the cube hollows");
    let doubled = Edit::Scale {
        object: CUBE,
        factors: Vec3::splat(2.0),
    };

    apply(&mut document, &doubled, Cancel::never()).expect("the cube scales");

    let (state, meshes) = document.object(CUBE).expect("the cube stays");
    let shell = meshes.shell.as_ref().expect("the cube stays hollow");
    let material = signed_volume(&core_geometry::transform_mesh(shell, state.transform));
    let wanted = 40.0_f32.powi(3) - 36.0_f32.powi(3);
    let tolerance = 4.0 * LATTICE_MM3;
    assert!(
        (material - wanted).abs() < tolerance,
        "a 2 mm wall on a 40 mm cube leaves 40³ - 36³ = {wanted} mm³ within {tolerance}, got \
         {material}"
    );
}

#[test]
fn an_open_floor_turned_over_opens_toward_the_plate_it_now_stands_on() {
    let mut document = Document::from_project(cube_of(20.0));
    let mut open = walled(2.0);
    if let Edit::Hollow { settings, .. } = &mut open {
        settings.open_floor = true;
    }
    apply(&mut document, &open, Cancel::never()).expect("the cube hollows");
    let over = Edit::Rotate {
        object: CUBE,
        degrees: Vec3::new(180.0, 0.0, 0.0),
    };

    apply(&mut document, &over, Cancel::never()).expect("the cube turns");

    let (state, _) = document.object(CUBE).expect("the cube stays");
    let built = state.hollow.built.as_ref().expect("the cube stays hollow");
    let toward = built.wall.open_toward.expect("the floor stays open");
    let down = core_volume::plate_down(state.transform);
    assert!(
        toward.abs_diff_eq(down, 1e-4),
        "the opening faces the plate, {down}, got {toward}"
    );
}
