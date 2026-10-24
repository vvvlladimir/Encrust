use super::*;
use core_geometry::{Vec3, diagnose};

/// A box of ten triangles: a cube with the two of its top missing.
fn open_box() -> Mesh {
    Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
    )
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
    assert_eq!(repaired.filled.loops_filled, 1);
    assert!(diagnose(&repaired.shaped.mesh).is_closed());
    assert!(repaired.orientation.orientable);
}

#[test]
fn a_face_drawn_twice_beside_a_hole_comes_back_sound() {
    // What a broken exporter leaves: one triangle of a face missing, and one of the
    // triangles written twice, which makes three edges look as if they branched.
    let mut mesh = open_box();
    mesh.faces.push([1, 2, 6]);
    assert!(!diagnose(&mesh).is_sound());

    let mut job = RepairJob::spawn(RepairRequest {
        id: ObjectId::for_test(3),
        name: "broken.stl".to_owned(),
        mesh: Arc::new(mesh),
    });
    let outcome = loop {
        if let Some(outcome) = job.poll() {
            break outcome;
        }
        std::thread::yield_now();
    };

    let RepairOutcome::Done(repaired) = outcome else {
        panic!("the box has both to repair");
    };
    assert_eq!(repaired.duplicates_removed, 1);
    assert_eq!(repaired.filled.loops_filled, 1);
    assert!(
        diagnose(&repaired.shaped.mesh).is_sound(),
        "nothing is left for the window to call it broken for"
    );
}

/// Two of those boxes meeting along one vertical edge, the near one open at the top:
/// four faces use the shared edge, twice each way, which is a seam rather than a hole.
fn two_boxes_sharing_an_edge() -> Mesh {
    let mut mesh = open_box();
    let other = core_geometry::transform_mesh(
        &closed_box(),
        core_geometry::Transform::from_translation(Vec3::new(1.0, 1.0, 0.0)),
    );
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend(other.vertices);
    mesh.faces.extend(
        other
            .faces
            .iter()
            .map(|face| face.map(|index| index + offset)),
    );
    core_geometry::weld(&mesh, core_geometry::DEFAULT_WELD_TOLERANCE).mesh
}

/// The box of [`open_box`] with its top on.
fn closed_box() -> Mesh {
    let mut mesh = open_box();
    mesh.faces.push([4, 5, 6]);
    mesh.faces.push([4, 6, 7]);
    mesh
}

#[test]
fn a_seam_two_solids_share_is_not_something_to_repair() {
    let mesh = two_boxes_sharing_an_edge();
    assert_eq!(diagnose(&mesh).non_manifold_edges, 1, "the shared edge");

    let mut job = RepairJob::spawn(RepairRequest {
        id: ObjectId::for_test(5),
        name: "pair.stl".to_owned(),
        mesh: Arc::new(mesh),
    });
    let outcome = loop {
        if let Some(outcome) = job.poll() {
            break outcome;
        }
        std::thread::yield_now();
    };

    let RepairOutcome::Done(repaired) = outcome else {
        panic!("the open box has a hole to close");
    };
    assert_eq!(repaired.duplicates_removed, 0, "a seam costs no face");
    assert!(!repaired.kept_as_found);
    assert_eq!(repaired.filled.loops_filled, 1);
    assert!(
        diagnose(&repaired.shaped.mesh).is_sound(),
        "closing the hole leaves nothing to call it broken for"
    );
}

#[test]
fn a_fin_is_cancelled_by_its_patch_and_the_model_comes_back_as_it_was() {
    // A fin standing on one edge of the box, which is what a boolean leaves behind:
    // no winding covers the three faces that edge now has.
    let mut mesh = closed_box();
    mesh.vertices.push(Vec3::new(0.5, 0.5, 2.0));
    mesh.faces.push([4, 5, 8]);

    let mut job = RepairJob::spawn(RepairRequest {
        id: ObjectId::for_test(9),
        name: "tangled.stl".to_owned(),
        mesh: Arc::new(mesh),
    });
    let outcome = loop {
        if let Some(outcome) = job.poll() {
            break outcome;
        }
        std::thread::yield_now();
    };

    let RepairOutcome::Done(repaired) = outcome else {
        panic!("the fin is something to repair");
    };
    assert_eq!(repaired.duplicates_removed, 2, "the fin and its reverse");
    assert_eq!(repaired.shaped.mesh.faces, closed_box().faces);
    assert!(diagnose(&repaired.shaped.mesh).is_sound());
}

#[test]
fn a_model_that_is_one_sheet_is_kept_rather_than_cancelled_away() {
    let mut mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
    let mended = mend(&mut mesh);

    assert!(mended.kept_as_found);
    assert_eq!(mesh.faces, vec![[0, 1, 2]]);
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

#[test]
fn a_face_with_no_area_between_two_corners_on_one_point_is_repaired() {
    // A tetrahedron with its edge 0-1 split at vertex 0 by a vertex standing on it.
    let mut mesh = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z, Vec3::ZERO],
        vec![
            [0, 2, 4],
            [4, 2, 1],
            [0, 4, 3],
            [4, 1, 3],
            [1, 2, 3],
            [2, 0, 3],
        ],
    );
    let mended = mend(&mut mesh);
    assert!(!mended.kept_as_found);
    assert_eq!(wrong(&diagnose(&mesh)), 0, "got {:?}", diagnose(&mesh));
}
