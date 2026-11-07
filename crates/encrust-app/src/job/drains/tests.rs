use super::*;
use core_geometry::{Quat, Vec3};

use crate::scene::fixtures::hollow_cube;

const SIDE_MM: f32 = 20.0;
const WALL_MM: f32 = 2.0;
/// How far off a face a mouth may stand: the lattice the wall was cut on is finer than this.
const ON_FACE_MM: f32 = 0.05;

fn three_mm() -> HoleSize {
    HoleSize::round(3.0, 3.0, 1.0)
}

/// The cube turned a quarter about X, so its own -Y face looks down, and lifted 5 mm as
/// supports would stand it, its corner moved off the origin.
fn posed() -> Transform {
    Transform {
        translation: Vec3::new(40.0, 30.0, 5.0 + SIDE_MM),
        rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ..Transform::default()
    }
}

fn wait(job: &mut DrainsJob) -> DrainsOutcome {
    loop {
        if let Some(outcome) = job.poll() {
            return outcome;
        }
        std::thread::yield_now();
    }
}

/// The outward normal of the face of the cube `at` lies on, in its own space, or `None` off
/// every face.
fn face_under(at: Vec3) -> Option<Vec3> {
    let inside = |value: f32| (-ON_FACE_MM..=SIDE_MM + ON_FACE_MM).contains(&value);
    if !(inside(at.x) && inside(at.y) && inside(at.z)) {
        return None;
    }
    (0..3).find_map(|axis| {
        let unit = Vec3::AXES[axis];
        if at[axis].abs() <= ON_FACE_MM {
            Some(-unit)
        } else if (at[axis] - SIDE_MM).abs() <= ON_FACE_MM {
            Some(unit)
        } else {
            None
        }
    })
}

#[test]
fn a_turned_lifted_hollow_cube_gets_holes_on_its_own_faces_drilled_inward() {
    let scene = hollow_cube(SIDE_MM, WALL_MM, posed());
    let task = DrainsTask::of(&scene.objects()[0], three_mm());
    let mut job = DrainsJob::spawn(DrainsRequest {
        tasks: vec![task],
        layer_height_mm: 0.5,
        vents: true,
    });

    let DrainsOutcome::Found(found) = wait(&mut job) else {
        panic!("a hollow cube slices");
    };
    let proposed = &found[0];
    assert!(proposed.trapped.is_empty(), "left {:?}", proposed.trapped);
    assert_eq!(proposed.holes.len(), 2, "a drain and a vent");
    for hole in &proposed.holes {
        let normal = face_under(hole.at)
            .unwrap_or_else(|| panic!("{} is on no face of the 20 mm cube", hole.at));
        assert!(
            hole.axis.dot(normal) < -0.99,
            "drilled square into the face {normal}, got {}",
            hole.axis
        );
        assert!(
            (hole.diameter_mm - 3.0).abs() < 1e-4,
            "unscaled, a 3 mm hole stays 3 mm, got {}",
            hole.diameter_mm
        );
    }
    let low = proposed.holes[0].at;
    assert!(
        low.y.abs() <= ON_FACE_MM,
        "lifted, the drain goes through the face looking down, its own -Y, got {low}"
    );
    assert!(
        (job.fraction() - 1.0).abs() < 1e-6,
        "a search that ended fills its bar, got {}",
        job.fraction()
    );
}

#[test]
fn a_search_asked_to_stop_hands_nothing_over() {
    let scene = hollow_cube(SIDE_MM, WALL_MM, posed());
    let request = DrainsRequest {
        tasks: vec![DrainsTask::of(&scene.objects()[0], three_mm())],
        layer_height_mm: 0.5,
        vents: true,
    };
    let outcome = search(&request, &AtomicBool::new(true), &Heard::default());
    assert_eq!(outcome, DrainsOutcome::Cancelled);
}

#[test]
fn a_model_standing_on_the_plate_gets_no_mouth_where_the_first_layers_spread() {
    let standing = Transform::from_translation(Vec3::new(30.0, 30.0, 0.0));
    let scene = hollow_cube(SIDE_MM, WALL_MM, standing);
    let mut job = DrainsJob::spawn(DrainsRequest {
        tasks: vec![DrainsTask::of(&scene.objects()[0], three_mm())],
        layer_height_mm: 0.5,
        vents: false,
    });

    let DrainsOutcome::Found(found) = wait(&mut job) else {
        panic!("a hollow cube slices");
    };
    let drain = found[0].holes.first().expect("a side drains the cavity");
    assert_eq!(found[0].holes.len(), 1, "no vent asked for");
    let lowest = drain.at.z - drain.diameter_mm / 2.0;
    assert!(
        lowest >= 1.0 - ON_FACE_MM,
        "the mouth stands a millimetre clear of the plate, its lowest point at {lowest} mm"
    );
}
