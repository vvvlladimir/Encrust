//! A blocker a click puts down keeps the wall solid past the wall itself, can fit the part
//! it lands on, and can run as a rod from the one before.
#![expect(
    clippy::expect_used,
    reason = "a fixture that does not land fails the test"
)]

use core_geometry::{Bvh, Mesh, Ray, Scalar, Transform, Vec3, ball, cuboid, raycast_placed};
use core_volume::{
    Cancel, HollowMode, HollowSettings, ModelHollow, across_mm, blocker_markers, fitted_blocker,
    hollow,
};

const PI: Scalar = std::f32::consts::PI;

fn shell_of(wall_mm: Scalar) -> HollowSettings {
    HollowSettings {
        thickness_mm: wall_mm,
        mode: HollowMode::Internal,
        precision: 0.5,
        ..HollowSettings::default()
    }
}

/// Volume two balls of radii `big` and `small` share, their centres `apart` mm apart.
fn lens_mm3(big: Scalar, small: Scalar, apart: Scalar) -> Scalar {
    let overlap = big + small - apart;
    PI * overlap.powi(2)
        * (apart.powi(2) + 2.0 * apart * small - 3.0 * small.powi(2)
            + 2.0 * apart * big
            + 6.0 * small * big
            - 3.0 * big.powi(2))
        / (12.0 * apart)
}

#[test]
fn a_blocker_smaller_than_the_wall_still_reaches_into_the_cavity() {
    let mesh = ball(Vec3::ZERO, 10.0, 48, 48);
    let bvh = Bvh::build(&mesh);
    let asked = shell_of(2.0);
    let open = hollow(&mesh, &bvh, &asked, Cancel::never())
        .expect("a closed ball hollows")
        .cavity_mm3;

    let mut model = ModelHollow::default();
    model.place_blocker(
        ModelHollow::blocker_at(Vec3::new(10.0, 0.0, 0.0), 2.0, Transform::default())
            .expect("the placement inverts"),
    );
    let blocked = hollow(&mesh, &bvh, &model.asking(&asked), Cancel::never())
        .expect("a closed ball hollows")
        .cavity_mm3;

    // A ball of 4 about the surface point shares a lens with the cavity, a ball of 8 at 10
    // mm from it; a lens 2 mm thick is a few voxels, so its volume is good to 15 %.
    let expected = lens_mm3(8.0, 4.0, 10.0);
    let kept = open - blocked;
    assert!(
        (kept - expected).abs() / expected < 0.15,
        "a 2 mm blocker on a 2 mm wall keeps a lens of {expected} mm3 solid, got {kept}"
    );
}

#[test]
fn a_fitted_blocker_fills_the_part_it_was_clicked_on() {
    let bar = cuboid(Vec3::ZERO, Vec3::new(40.0, 8.0, 6.0));
    let bvh = Bvh::build(&bar);

    let blocker = fitted_blocker(
        &bar,
        &bvh,
        Transform::default(),
        Vec3::new(20.0, 4.0, 6.0),
        Vec3::Z,
    )
    .expect("the bar has a far side");

    assert!(
        blocker.from.abs_diff_eq(Vec3::new(20.0, 4.0, 3.0), 1e-3),
        "halfway down a 6 mm bar, (20, 4, 3), got {:?}",
        blocker.from
    );
    assert!(
        (blocker.radius_mm - 3.0).abs() < 1e-3,
        "as wide as the 6 mm bar, a radius of 3, got {}",
        blocker.radius_mm
    );
}

#[test]
fn a_fitted_blocker_on_a_scaled_model_fits_the_part_as_it_stands() {
    let bar = cuboid(Vec3::ZERO, Vec3::new(40.0, 8.0, 6.0));
    let bvh = Bvh::build(&bar);
    let transform = Transform {
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };

    let blocker = fitted_blocker(&bar, &bvh, transform, Vec3::new(40.0, 8.0, 12.0), Vec3::Z)
        .expect("the bar has a far side");

    assert!(
        blocker.from.abs_diff_eq(Vec3::new(20.0, 4.0, 3.0), 1e-3),
        "kept in the model's own space, (20, 4, 3), got {:?}",
        blocker.from
    );
    assert!(
        (blocker.radius_mm - 3.0).abs() < 1e-3,
        "12 mm across on the plate is 3 mm of radius in the model, got {}",
        blocker.radius_mm
    );
}

#[test]
fn a_fitted_blocker_needs_a_far_side() {
    let bar = cuboid(Vec3::ZERO, Vec3::new(40.0, 8.0, 6.0));
    let bvh = Bvh::build(&bar);

    let outward = fitted_blocker(
        &bar,
        &bvh,
        Transform::default(),
        Vec3::new(20.0, 4.0, 6.0),
        -Vec3::Z,
    );

    assert!(outward.is_none(), "a ray out into the air finds nothing");
}

#[test]
fn a_rod_runs_from_the_last_blocker_to_the_next_and_is_drawn_whole() {
    let mut model = ModelHollow::default();
    let next = ModelHollow::blocker_at(Vec3::new(10.0, 0.0, 0.0), 1.0, Transform::default())
        .expect("an unscaled placement inverts");
    assert!(model.rod_to(&next).is_none(), "nothing to run from");

    model.place_blocker(
        ModelHollow::blocker_at(Vec3::ZERO, 2.0, Transform::default())
            .expect("the placement inverts"),
    );
    let rod = model.rod_to(&next).expect("a blocker to run from");
    assert!(rod.from.abs_diff_eq(Vec3::ZERO, 1e-6));
    assert!(rod.to.abs_diff_eq(Vec3::new(10.0, 0.0, 0.0), 1e-6));
    assert!((rod.radius_mm - 1.0).abs() < 1e-6, "as wide as the click");

    let drawn = blocker_markers([rod]).expect("a rod is drawn");
    let bounds = drawn.aabb().expect("the marker has vertices");
    assert!(
        bounds.mins.abs_diff_eq(Vec3::new(-1.0, -1.0, -1.0), 1e-4)
            && bounds.maxs.abs_diff_eq(Vec3::new(11.0, 1.0, 1.0), 1e-4),
        "a rod of 1 from 0 to 10 on X with round ends, got {bounds:?}"
    );
}

/// How far down `ray` a click reaches into `mesh`: to halfway across the part it lands on.
fn reach_through(mesh: &Mesh, bvh: &Bvh, ray: &Ray) -> Scalar {
    let hit = raycast_placed(mesh, bvh, Transform::default(), ray).expect("the ray lands");
    let across = across_mm(mesh, bvh, Transform::default(), hit.point, ray.direction)
        .expect("the part has a far side");
    hit.distance + across / 2.0
}

#[test]
fn a_click_through_a_fitted_blocker_deep_in_the_part_takes_that_one_away() {
    let bar = cuboid(Vec3::ZERO, Vec3::new(40.0, 8.0, 6.0));
    let bvh = Bvh::build(&bar);
    let mut model = ModelHollow::default();
    for x in [10.0, 30.0] {
        let point = Vec3::new(x, 4.0, 6.0);
        let fitted = fitted_blocker(&bar, &bvh, Transform::default(), point, Vec3::Z)
            .expect("the bar has a floor under its top");
        model.place_blocker(fitted);
    }

    // The ball under (12, 5) is 3 mm round its centre 3 mm down, so the top face it touches
    // only at (10, 4) is not inside it; the ray down through the top still crosses it.
    let down = Ray::new(Vec3::new(12.0, 5.0, 50.0), Vec3::NEG_Z);
    let reach = reach_through(&bar, &bvh, &down);
    assert!(model.remove_blocker(&down, reach, Transform::default()));
    assert_eq!(
        model.blockers().len(),
        1,
        "only the blocker the ray crossed goes"
    );
    assert!(
        (model.blockers()[0].from.x - 30.0).abs() < 1e-3,
        "the one at x = 30 stays, got {:?}",
        model.blockers()[0].from
    );
}

#[test]
fn a_click_where_no_blocker_is_spares_the_one_on_the_far_side() {
    let bar = cuboid(Vec3::ZERO, Vec3::new(40.0, 8.0, 6.0));
    let bvh = Bvh::build(&bar);
    let mut model = ModelHollow::default();
    let underneath = ModelHollow::blocker_at(Vec3::new(20.0, 4.0, 0.0), 2.0, Transform::default())
        .expect("an unscaled placement inverts");
    model.place_blocker(underneath);

    // Down through the top at (20, 4), 44 mm off, the click reaches 3 mm in, to z = 3; the
    // ball of 2 about the floor starts at z = 2.
    let down = Ray::new(Vec3::new(20.0, 4.0, 50.0), Vec3::NEG_Z);
    let reach = reach_through(&bar, &bvh, &down);
    assert!(
        !model.remove_blocker(&down, reach, Transform::default()),
        "the blocker under the floor is out of sight behind the top"
    );
    assert_eq!(model.blockers().len(), 1);
}
