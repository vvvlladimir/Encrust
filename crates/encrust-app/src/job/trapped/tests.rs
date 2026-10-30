use super::*;
use core_geometry::Vec3;

/// An axis-aligned box spanning `min`..`max`, wound outward.
fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
    core_geometry::cuboid(min, max)
}

/// A 20 mm cube with a 10 mm cube of air sealed inside it, the cavity wound inward
/// the way a hollowed model carries it.
fn sealed_box() -> Mesh {
    let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(20.0));
    let cavity = box_mesh(Vec3::splat(5.0), Vec3::splat(15.0));
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&cavity.vertices);
    mesh.faces.extend(
        cavity
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );
    mesh
}

fn wait(job: &mut TrapJob) -> TrapOutcome {
    loop {
        if let Some(outcome) = job.poll() {
            return outcome;
        }
        std::thread::yield_now();
    }
}

#[test]
fn a_sealed_cavity_is_found_where_its_floor_is() {
    let mut job = TrapJob::spawn(TrapRequest {
        tasks: vec![TrapTask {
            id: ObjectId::for_test(1),
            mesh: Arc::new(sealed_box()),
            cuts: None,
            transform: Transform::default(),
        }],
        layer_height_mm: 0.5,
    });

    let TrapOutcome::Checked(models) = wait(&mut job) else {
        panic!("a closed mesh slices");
    };
    assert_eq!(models[0].trapped.len(), 1, "one sealed cavity");

    let pocket = models[0].trapped[0];
    assert!(
        (pocket.volume_mm3 - 1000.0).abs() < 60.0,
        "a 10 mm cube of air is 1000 mm3, got {}",
        pocket.volume_mm3
    );
    assert!(
        (pocket.at.z - 5.25).abs() < 0.3,
        "the hole goes at the cavity's floor, got {}",
        pocket.at
    );
}

fn task(id: u64, mesh: &Arc<Mesh>, transform: Transform) -> TrapTask {
    TrapTask {
        id: ObjectId::for_test(id),
        mesh: Arc::clone(mesh),
        cuts: None,
        transform,
    }
}

#[test]
fn a_copy_moved_along_the_plate_shares_the_check_and_one_lifted_or_turned_does_not() {
    let mesh = Arc::new(sealed_box());
    let original = task(1, &mesh, Transform::default());
    let moved = Transform::from_translation(Vec3::new(30.0, -12.0, 0.0));
    assert_eq!(twin_of(&[original], &task(2, &mesh, moved)), Some(0));

    let original = [task(1, &mesh, Transform::default())];
    let lifted = Transform::from_translation(Vec3::new(0.0, 0.0, 3.0));
    assert_eq!(
        twin_of(&original, &task(3, &mesh, lifted)),
        None,
        "a model standing higher is cut at other heights"
    );
    let turned = Transform {
        rotation: core_geometry::Quat::from_rotation_z(0.5),
        ..Transform::default()
    };
    assert_eq!(twin_of(&original, &task(4, &mesh, turned)), None);
    let other = Arc::new(sealed_box());
    assert_eq!(
        twin_of(&original, &task(5, &other, Transform::default())),
        None,
        "an equal mesh imported twice is not known to be one"
    );
}

#[test]
fn every_copy_gets_the_pockets_of_its_original() {
    let mesh = Arc::new(sealed_box());
    let moved = Transform::from_translation(Vec3::new(30.0, 0.0, 0.0));
    let mut job = TrapJob::spawn(TrapRequest {
        tasks: vec![task(1, &mesh, Transform::default()), task(2, &mesh, moved)],
        layer_height_mm: 0.5,
    });

    let TrapOutcome::Checked(models) = wait(&mut job) else {
        panic!("a closed mesh slices");
    };
    let ids: Vec<ObjectId> = models.iter().map(|pockets| pockets.id).collect();
    assert_eq!(ids, [1, 2].map(ObjectId::for_test));
    assert_eq!(models[1].trapped, models[0].trapped);
    assert_eq!(
        models[1].trapped.len(),
        1,
        "the copy's cavity is sealed too"
    );
}

#[test]
fn a_solid_model_has_nothing_trapped_in_it() {
    let mut job = TrapJob::spawn(TrapRequest {
        tasks: vec![TrapTask {
            id: ObjectId::for_test(2),
            mesh: Arc::new(box_mesh(Vec3::ZERO, Vec3::splat(10.0))),
            cuts: None,
            transform: Transform::default(),
        }],
        layer_height_mm: 0.5,
    });

    let TrapOutcome::Checked(models) = wait(&mut job) else {
        panic!("a closed mesh slices");
    };
    assert!(models[0].trapped.is_empty());
}

#[test]
fn a_finished_check_has_counted_every_layer_of_every_model() {
    let mesh = Arc::new(sealed_box());
    let moved = Transform::from_translation(Vec3::new(30.0, 0.0, 0.0));
    let mut job = TrapJob::spawn(TrapRequest {
        tasks: vec![task(1, &mesh, Transform::default()), task(2, &mesh, moved)],
        layer_height_mm: 0.5,
    });

    wait(&mut job);
    let (_, share) = job
        .counted()
        .expect("a model was cut, so its layers were counted");
    assert!(
        (share - 1.0).abs() < 1e-6,
        "both models are done, the copy without being cut, got {share}"
    );
    assert_eq!(job.looked.layers_total.load(Relaxed), 40, "20 mm at 0.5 mm");
}

#[test]
fn a_check_asked_to_stop_ends_without_a_finding() {
    let request = TrapRequest {
        tasks: vec![task(1, &Arc::new(sealed_box()), Transform::default())],
        layer_height_mm: 0.5,
    };
    let stop = AtomicBool::new(true);

    let outcome = check(&request, (&stop, &Looked::default()));
    assert!(
        matches!(outcome, TrapOutcome::Failed(_)),
        "a stopped check has nothing to hand over, got {outcome:?}"
    );
}
