use core_geometry::{Bvh, Vec3, cuboid, signed_volume};

use super::*;
use crate::drain::{DrainHole, HoleKind, HoleShape, drill};
use crate::drilled::Drilled;
use crate::error::VolumeError;
use crate::hollow::{HollowMode, HollowSettings, Hollowed, hollow};

/// A 20 mm box, and the same box with a 10 mm cavity in its middle wound inward after it, the
/// cavity's faces' range with it.
fn box_and_shell() -> (Arc<Mesh>, Mesh, Range<usize>) {
    let model = cuboid(Vec3::ZERO, Vec3::splat(20.0));
    let mut shell = model.clone();
    let cavity = cuboid(Vec3::splat(5.0), Vec3::splat(15.0));
    let offset = shell.vertices.len() as u32;
    shell.vertices.extend(&cavity.vertices);
    shell.faces.extend(
        cavity
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );
    let range = model.faces.len()..shell.faces.len();
    (Arc::new(model), shell, range)
}

/// A 2 mm square hole down through the top wall into the cavity.
fn cuts() -> Mesh {
    let hole = DrainHole {
        at: Vec3::new(10.0, 10.0, 20.0),
        axis: Vec3::NEG_Z,
        diameter_mm: 2.0 * std::f32::consts::SQRT_2,
        depth_mm: 8.0,
        taper: 1.0,
        lift_mm: 0.0,
        shape: HoleShape::Polygon { sides: 4 },
        turn_deg: 45.0,
        kind: HoleKind::Drain,
    };
    drill(&[hole], &[]).expect("the hole drills")
}

#[test]
fn a_second_ask_gets_the_first_read() {
    let (model, _, _) = box_and_shell();
    let read = ModelRead::new(model);
    let first = read.solid(Cancel::never()).expect("a box reads");
    let second = read.solid(Cancel::never()).expect("a box reads");
    assert!(std::ptr::eq(first, second), "the model is read once");
    assert_eq!(
        read.progress().read(),
        Some((Step::Reading, 3, 3)),
        "a box reads as it stands, all three pieces of the read done at once"
    );
}

#[test]
fn a_shell_drilled_on_its_model_read_is_the_shell_drilled_on_its_own() {
    let (model, shell, cavity) = box_and_shell();
    let read = ModelRead::new(model);
    let built = read
        .shell(&shell, &cavity, Cancel::never())
        .expect("the shell begins with its model");
    // 8000 mm3 of box less the 1000 mm3 cavity.
    let volume = signed_volume(&built.mesh());
    assert!(
        (volume - 7000.0).abs() < 1e-2,
        "expected 7000 mm3, got {volume}"
    );

    let cuts = cuts();
    let shared = Drilled::of_read(&read, &shell, Some(cavity.clone()), &cuts, Cancel::never())
        .expect("the shell drills");
    let alone = Drilled::of(&shell, Some(cavity), &cuts).expect("the shell drills");
    let (shared_mm3, alone_mm3) = (signed_volume(&shared.mesh), signed_volume(&alone.mesh));
    // The bore takes 2 x 2 x 5 mm of the wall: 6980 mm3 either way.
    assert!(
        (shared_mm3 - 6980.0).abs() < 1e-2 && (alone_mm3 - 6980.0).abs() < 1e-2,
        "expected 6980 mm3 both ways, got {shared_mm3} and {alone_mm3}"
    );
    assert_eq!(
        shared.cavity.map(|range| range.len()),
        alone.cavity.map(|range| range.len())
    );
}

#[test]
fn a_shell_that_does_not_begin_with_its_model_is_not_built_on_it() {
    let (_, shell, cavity) = box_and_shell();
    let other = ModelRead::new(Arc::new(cuboid(Vec3::ZERO, Vec3::splat(21.0))));
    assert!(other.shell(&shell, &cavity, Cancel::never()).is_none());
    let drilled = Drilled::of_read(&other, &shell, Some(cavity), &cuts(), Cancel::never())
        .expect("the shell is read on its own instead");
    let volume = signed_volume(&drilled.mesh);
    assert!(
        (volume - 6980.0).abs() < 1e-2,
        "expected 6980 mm3, got {volume}"
    );
}

#[test]
fn a_drill_given_up_is_answered_as_given_up() {
    let (model, shell, cavity) = box_and_shell();
    let read = ModelRead::new(model);
    let asked = || true;
    let drilled = Drilled::of_read(&read, &shell, Some(cavity), &cuts(), Cancel::when(&asked));
    assert_eq!(drilled.err(), Some(VolumeError::Cancelled));
}

#[test]
fn a_run_given_up_before_it_asks_starts_no_read() {
    let (model, shell, cavity) = box_and_shell();
    let read = ModelRead::new(model);
    let asked = || true;
    assert_eq!(
        read.solid(Cancel::when(&asked)).err(),
        Some(VolumeError::Cancelled)
    );
    assert!(read.shell(&shell, &cavity, Cancel::when(&asked)).is_none());
    assert_eq!(read.progress().read(), None, "the model was never read");
}

#[test]
fn a_shell_built_on_its_read_keeps_its_infill() {
    let (model, mut shell, cavity) = box_and_shell();
    shell.append(&cuboid(Vec3::splat(9.0), Vec3::splat(11.0)));
    let built = ModelRead::new(model)
        .shell(&shell, &cavity, Cancel::never())
        .expect("the shell begins with its model");
    // 7000 mm3 of walls and a 2 mm cube of infill standing in the cavity.
    let volume = signed_volume(&built.mesh());
    assert!(
        (volume - 7008.0).abs() < 1e-2,
        "expected 7008 mm3, got {volume}"
    );
}

#[test]
fn a_drill_that_leaves_nothing_of_the_model_is_answered_so() {
    let tiny = Arc::new(cuboid(
        Vec3::new(9.5, 9.5, 12.0),
        Vec3::new(10.5, 10.5, 13.0),
    ));
    let read = ModelRead::new(Arc::clone(&tiny));
    let drilled = Drilled::of_read(&read, &tiny, None, &cuts(), Cancel::never());
    assert_eq!(drilled.err(), Some(VolumeError::NothingLeft));
}

#[test]
fn a_model_the_engine_refuses_is_not_read_again_by_its_drill() {
    let mut torn = cuboid(Vec3::ZERO, Vec3::splat(20.0));
    torn.vertices[0] = Vec3::NAN;
    let model = Arc::new(torn);
    let read = ModelRead::new(Arc::clone(&model));
    let progress = Progress::default();
    let cancel = Cancel::never().reporting(&progress);
    let drilled = Drilled::of_read(&read, &model, None, &cuts(), cancel);
    assert!(
        matches!(drilled, Err(VolumeError::Csg(_))),
        "got {drilled:?}"
    );
    assert_eq!(
        progress.read(),
        Some((Step::Reading, 0, 0)),
        "the run waited on the one read and counted none of its own"
    );
}

#[test]
fn a_shell_printed_on_its_model_and_read_whole_come_to_the_same_body() {
    let (_, shell, cavity) = box_and_shell();
    let cuts = cuts();
    for on_its_model in [true, false] {
        let drilled = Drilled::of_printed(&shell, Some(cavity.clone()), on_its_model, &cuts)
            .expect("the shell drills");
        // 8000 mm3 less the 1000 mm3 cavity and the 2 x 2 x 5 mm bore through its wall.
        let volume = signed_volume(&drilled.mesh);
        assert!(
            (volume - 6980.0).abs() < 1e-2,
            "expected 6980 mm3 on_its_model {on_its_model}, got {volume}"
        );
        assert!(drilled.cavity.is_some_and(|faces| !faces.is_empty()));
    }
}

#[test]
fn the_model_of_a_shell_is_what_begins_it() {
    let (model, shell, cavity) = box_and_shell();
    let read = ModelRead::of_shell(&shell, &cavity);
    assert_eq!(read.model().faces, model.faces);
    assert_eq!(read.model().vertices, model.vertices);
}

/// A 20 mm box hollowed to a 2 mm wall as `settings` further ask, with the model it came from.
fn hollowed_box(change: impl FnOnce(&mut HollowSettings)) -> (Mesh, Hollowed) {
    let model = cuboid(Vec3::ZERO, Vec3::splat(20.0));
    let mut settings = HollowSettings::for_wall(2.0);
    change(&mut settings);
    let made = hollow(&model, &Bvh::build(&model), &settings, Cancel::never());
    (model, made.expect("a box hollows"))
}

#[test]
fn an_inward_shell_begins_with_its_model_and_a_mould_or_an_opened_floor_does_not() {
    let begun = |model: &Mesh, made: &Hollowed| {
        let read = ModelRead::of_shell(&made.mesh, &made.cavity);
        read.model().faces == model.faces && read.model().vertices == model.vertices
    };
    let (model, inward) = hollowed_box(|_| {});
    assert!(begun(&model, &inward), "an inward wall keeps the model");

    let (model, mould) = hollowed_box(|settings| settings.mode = HollowMode::External);
    assert!(
        !begun(&model, &mould),
        "a mould begins with the grown surface"
    );

    let (model, opened) = hollowed_box(|settings| settings.open_toward = Some(Vec3::NEG_Z));
    assert!(
        !begun(&model, &opened),
        "an opened floor has the floor cut out"
    );
}
