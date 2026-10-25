use super::*;
use crate::scene::ImportSummary;
use core_geometry::{Orientation, diagnose};

/// An axis-aligned box spanning `min`..`max`, wound outwards.
fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
    core_geometry::cuboid(min, max)
}

fn summary(mesh: &Mesh) -> ImportSummary {
    ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: diagnose(mesh),
    }
}

/// A scene holding one 10 mm cube, standing wherever `transform` puts it.
fn scene_with_a_cube(transform: Transform) -> Scene {
    let mesh = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let mut scene = Scene::default();
    scene.insert(crate::scene::Imported::new(
        "cube".to_owned(),
        Arc::new(mesh.clone()),
        transform,
        summary(&mesh),
    ));
    scene
}

fn request(scene: &Scene) -> SupportRequest {
    SupportRequest {
        tasks: tasks_of(scene, |_| {
            SupportProfile::shipped("medium").expect("the medium preset ships")
        }),
        layer_height_mm: 0.1,
        sizing: None,
    }
}

/// Runs a request to its end on this thread, with nothing cancelling it.
fn placed(request: &SupportRequest) -> SupportOutcome {
    place(request, &AtomicBool::new(false), &mut |_| {})
}

#[test]
fn a_cube_on_the_plate_needs_nothing_standing_under_it() {
    let scene = scene_with_a_cube(Transform::default());
    let outcome = placed(&request(&scene));

    let SupportOutcome::Placed(placements) = outcome else {
        panic!("a sound cube places");
    };
    assert_eq!(placements.len(), 1, "the model was still looked at");
    assert!(placements[0].needs.is_empty());
}

#[test]
fn a_cube_lifted_off_the_plate_is_held_up_from_underneath() {
    let lift = Transform::from_translation(Vec3::new(0.0, 0.0, 10.0));
    let scene = scene_with_a_cube(lift);
    let outcome = placed(&request(&scene));

    let SupportOutcome::Placed(placements) = outcome else {
        panic!("a sound cube places");
    };
    assert!(
        placements
            .iter()
            .any(|placement| !placement.needs.is_empty()),
        "a floating cube is an island"
    );
    assert_eq!(placements[0].transform, lift, "the placement it was cut in");
    for contact in placements[0].needs.iter().map(|need| need.contact) {
        assert!(
            (contact.z - 10.0).abs() < 0.2,
            "{contact} is not under the lifted cube"
        );
    }
}

#[test]
fn a_hidden_model_is_not_worked_on() {
    let mut scene = scene_with_a_cube(Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)));
    scene.objects_mut()[0].visible = false;
    assert!(
        tasks_of(&scene, |_| SupportProfile::shipped("medium")
            .expect("the medium preset ships"))
        .is_empty()
    );
}

#[test]
fn the_patch_painted_out_of_bounds_is_handed_over_with_the_model() {
    let mut scene = scene_with_a_cube(Transform::default());
    let mesh = Arc::clone(&scene.objects()[0].mesh);
    let adjacency = core_geometry::Adjacency::of(&mesh);
    scene.objects_mut()[0]
        .supports
        .paint_face(&mesh, &adjacency, 0, 30.0, true, true);

    let tasks = tasks_of(&scene, |_| {
        SupportProfile::shipped("medium").expect("the medium preset ships")
    });
    assert!(
        !tasks[0].blocked.is_empty(),
        "the run has to know what it may not put a support on"
    );
}

#[test]
fn the_supports_already_under_a_model_are_handed_over_as_seeds() {
    let lift = Transform::from_translation(Vec3::new(0.0, 0.0, 10.0));
    let mut scene = scene_with_a_cube(lift);
    scene.objects_mut()[0]
        .supports
        .add(Vec3::new(5.0, 5.0, 10.0), lift, 0);

    let tasks = tasks_of(&scene, |_| {
        SupportProfile::shipped("medium").expect("the medium preset ships")
    });
    assert_eq!(tasks.len(), 1);
    assert!(
        tasks[0].seeds[0].abs_diff_eq(Vec3::new(5.0, 5.0, 10.0), 1e-4),
        "a seed stands in plate coordinates, got {}",
        tasks[0].seeds[0]
    );
}

#[test]
fn a_cancelled_run_hands_back_no_points() {
    let scene = scene_with_a_cube(Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)));
    let cancel = AtomicBool::new(true);
    assert_eq!(
        place(&request(&scene), &cancel, &mut |_| {}),
        SupportOutcome::Cancelled
    );
}

#[test]
fn a_layer_height_of_zero_comes_back_as_a_failure() {
    let scene = scene_with_a_cube(Transform::default());
    let broken = SupportRequest {
        layer_height_mm: 0.0,
        ..request(&scene)
    };
    let SupportOutcome::Failed(message) = placed(&broken) else {
        panic!("a layer height of zero cannot cut a stack");
    };
    assert!(message.contains("cannot slice the model"), "got {message}");
}

#[test]
fn a_run_reports_its_way_to_the_end() {
    let scene = scene_with_a_cube(Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)));
    let mut fractions = Vec::new();
    place(&request(&scene), &AtomicBool::new(false), &mut |fraction| {
        fractions.push(fraction);
    });

    assert_eq!(fractions.last(), Some(&1.0), "the run finishes at the end");
    assert!(
        fractions.windows(2).all(|pair| pair[0] <= pair[1]),
        "progress never runs backwards"
    );
}

#[test]
fn a_worker_that_dies_without_an_outcome_is_a_failure() {
    let (sender, reports) = mpsc::channel();
    let mut job = SupportJob {
        reports,
        cancel: Arc::new(AtomicBool::new(false)),
        fraction: None,
    };
    assert_eq!(job.poll(), None, "an empty channel means still running");

    drop(sender);
    let Some(SupportOutcome::Failed(message)) = job.poll() else {
        panic!("a dead worker must end the run");
    };
    assert!(message.contains("stopped"), "got {message}");
}

#[test]
fn a_cancelling_job_says_so_instead_of_a_percentage() {
    let (_sender, reports) = mpsc::channel();
    let job = SupportJob {
        reports,
        cancel: Arc::new(AtomicBool::new(false)),
        fraction: Some(0.5),
    };
    assert_eq!(job.label(), "Placing supports, 50 %");

    job.cancel();
    assert!(job.is_cancelling());
    assert_eq!(job.label(), "Cancelling");
}

/// The bar runs rather than standing at nought while the first stack is cut.
#[test]
fn a_run_with_nothing_reported_yet_has_no_share_to_show() {
    let (_sender, reports) = mpsc::channel();
    let job = SupportJob {
        reports,
        cancel: Arc::new(AtomicBool::new(false)),
        fraction: None,
    };
    assert_eq!(job.fraction(), None);
    assert_eq!(job.label(), "Cutting the plate...");
}
