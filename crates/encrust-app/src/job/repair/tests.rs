use super::*;
use core_geometry::{Vec3, cuboid, diagnose};

/// The unit cube with the two triangles of its top taken off: one square hole.
fn open_box() -> Mesh {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
    let vertices = mesh.vertices.clone();
    mesh.faces
        .retain(|face| face.iter().any(|&corner| vertices[corner as usize].z < 1.0));
    mesh
}

#[test]
fn a_repaired_model_comes_back_closed() {
    let mut job = RepairJob::spawn(RepairRequest {
        id: ObjectId::for_test(7),
        name: "open.stl".to_owned(),
        mesh: Arc::new(open_box()),
    });
    let outcome = loop {
        if let Some(outcome) = job.poll() {
            break outcome;
        }
        std::thread::yield_now();
    };

    let RepairOutcome::Done(repaired) = outcome else {
        panic!("an open box has a hole to close");
    };
    assert_eq!(repaired.id, ObjectId::for_test(7));
    assert_eq!(repaired.mended.filled.loops_filled, 1);
    assert!(diagnose(&repaired.shaped.mesh).is_closed());
    assert!(repaired.mended.orientation.orientable);
}

#[test]
fn a_running_repair_says_which_model_it_is_on() {
    let job = RepairJob::spawn(RepairRequest {
        id: ObjectId::for_test(1),
        name: "dragon.stl".to_owned(),
        mesh: Arc::new(open_box()),
    });
    assert_eq!(job.label(), "Repairing dragon.stl");
}
