use super::*;
use crate::{Vec2, Vec3, ball, cylinder, extrude, triangulate};

fn centroid(mesh: &Mesh, face: usize) -> Vec3 {
    let triangle = mesh
        .triangle(face)
        .expect("every face of a primitive is whole");
    (triangle.a + triangle.b + triangle.c) / 3.0
}

fn normal(mesh: &Mesh, face: usize) -> Vec3 {
    let triangle = mesh
        .triangle(face)
        .expect("every face of a primitive is whole");
    triangle.normal_unnormalized().normalize_or_zero()
}

/// A block 12 by 20 mm and 40 tall with a slot 2 mm wide cut 18 mm into it from +Y, open at
/// both ends along Z.
fn slotted_block() -> Mesh {
    let outline = [
        (0.0, 0.0),
        (12.0, 0.0),
        (12.0, 20.0),
        (7.0, 20.0),
        (7.0, 2.0),
        (5.0, 2.0),
        (5.0, 20.0),
        (0.0, 20.0),
    ]
    .map(|(x, y)| Vec2::new(x, y));
    let ring: Vec<u32> = (0..outline.len() as u32).collect();
    let cap = triangulate(&ring, &[], &outline);
    let count = outline.len();
    extrude(
        &outline,
        &cap,
        Vec2::extend,
        &vec![0.0; count],
        &vec![40.0; count],
    )
}

#[test]
fn an_empty_mesh_shows_nothing() {
    assert_eq!(Prominence::of(&Mesh::default()).face(0), 0.0);
}

#[test]
fn a_ball_is_open_and_smooth_all_over() {
    let mesh = ball(Vec3::ZERO, 10.0, 24, 48);
    let prominence = Prominence::of(&mesh);
    for face in 0..mesh.faces.len() {
        let shown = prominence.face(face);
        assert!(
            (shown - SMOOTH_SHOWS).abs() < 0.05,
            "face {face} of a ball shows {shown}, expected {SMOOTH_SHOWS}: open and no detail"
        );
    }
}

#[test]
fn the_floor_of_a_narrow_slot_shows_far_less_than_the_open_back() {
    let mesh = slotted_block();
    let prominence = Prominence::of(&mesh);
    let shown_where = |wanted: fn(Vec3, Vec3) -> bool| {
        (0..mesh.faces.len())
            .filter(|&face| wanted(centroid(&mesh, face), normal(&mesh, face)))
            .map(|face| prominence.face(face))
            .fold(0.0, Scalar::max)
    };
    let floor = shown_where(|at, facing| (at.y - 2.0).abs() < 1e-3 && facing.y > 0.9);
    let back = shown_where(|at, facing| at.y.abs() < 1e-3 && facing.y < -0.9);
    // A slot 2 wide and 18 deep lets sin(atan(1 / 18)) = 0.055 of the sky onto its floor,
    // and no detail shows more than all of what reaches it.
    assert!(
        floor < 0.15,
        "the floor shows {floor}, expected at most about 0.055"
    );
    assert!(
        back > 0.9 * SMOOTH_SHOWS,
        "the open back shows {back}, expected {SMOOTH_SHOWS}"
    );
}

#[test]
fn a_small_bump_on_a_smooth_ball_stands_out() {
    let mut mesh = ball(Vec3::ZERO, 10.0, 48, 96);
    let body = mesh.faces.len();
    mesh.append(&ball(Vec3::new(0.0, 0.0, 10.0), 0.8, 12, 24));
    let prominence = Prominence::of(&mesh);
    let mean = |faces: &mut dyn Iterator<Item = usize>| {
        let shown: Vec<Scalar> = faces.map(|face| prominence.face(face)).collect();
        shown.iter().sum::<Scalar>() / shown.len().max(1) as Scalar
    };
    let bump = mean(&mut (body..mesh.faces.len()).filter(|&face| centroid(&mesh, face).z > 10.3));
    let far = mean(&mut (0..body).filter(|&face| centroid(&mesh, face).z < -5.0));
    assert!(
        bump > 0.6,
        "the bump shows {bump} on average, expected near 1: open and detailed"
    );
    assert!(
        (far - SMOOTH_SHOWS).abs() < 0.05,
        "the far side shows {far}, expected {SMOOTH_SHOWS}: open and no detail"
    );
}

#[test]
fn one_sharp_needle_does_not_flatten_the_detail_everywhere_else() {
    let mut mesh = ball(Vec3::ZERO, 10.0, 48, 96);
    let body = mesh.faces.len();
    for at in 0..40 {
        let around = at as Scalar * 2.4;
        let out = 0.55 + 0.4 * (at as Scalar / 40.0);
        let ring = (1.0 - out * out).sqrt();
        let on = Vec3::new(out, ring * around.cos(), ring * around.sin()) * 10.0;
        mesh.append(&ball(on, 0.8, 12, 24));
    }
    let bumps = mesh.faces.len();
    mesh.append(&cylinder(Vec3::new(-9.9, 0.0, 0.0), Vec3::new(-14.0, 0.0, 0.0), 0.05, 6));
    let prominence = Prominence::of(&mesh);
    let shown: Vec<Scalar> = (body..bumps)
        .filter(|&face| centroid(&mesh, face).length() > 10.3)
        .map(|face| prominence.face(face))
        .collect();
    let mean = shown.iter().sum::<Scalar>() / shown.len().max(1) as Scalar;
    assert!(mean > 0.6, "the bumps show {mean} on average beside a needle, expected near 1");
}
