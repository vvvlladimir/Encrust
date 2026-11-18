use std::f64::consts::PI;

use core_geometry::{Transform, Vec3};

use super::{Strut, give_at, gives};
use crate::fixtures::{box_mesh, profile, trees_on};
use crate::{Placed, Profiles, SupportPoint};

const RADIUS_MM: f32 = 0.5;
const LENGTH_MM: f32 = 20.0;

fn close(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() < 1e-3 * want.abs(),
        "{what}: expected {want}, got {got}"
    );
}

/// A straight strut from the plate up to a contact at its top.
fn column(shared: f64) -> (Vec3, Strut) {
    let top = Vec3::new(0.0, 0.0, LENGTH_MM);
    (
        top,
        Strut {
            from: Vec3::ZERO,
            to: top,
            radius_mm: RADIUS_MM,
            shared,
        },
    )
}

fn second_moment() -> f64 {
    PI * f64::from(RADIUS_MM).powi(4) / 4.0
}

#[test]
fn a_lone_column_gives_as_a_cantilever_fixed_at_its_foot() {
    let (top, strut) = column(1.0);
    let give = give_at(top, &[strut], f32::MIN, 0, (0.0, 0.0));
    let (length, area) = (f64::from(LENGTH_MM), PI * f64::from(RADIUS_MM).powi(2));
    close(
        give.lateral_per_mm,
        length.powi(3) / (3.0 * second_moment()),
        "across, L³/3I",
    );
    close(give.axial_per_mm, length / area, "along, L/A");
    close(
        give.turning_per_mm3,
        length / second_moment(),
        "turning, L/I",
    );
}

#[test]
fn a_brace_holds_the_trunk_still_across_below_it() {
    let (top, strut) = column(1.0);
    let braced_at = 12.0;
    let give = give_at(top, &[strut], braced_at, 0, (0.0, 0.0));
    let free = f64::from(LENGTH_MM - braced_at);
    close(
        give.lateral_per_mm,
        free.powi(3) / (3.0 * second_moment()),
        "only the free (L−h)³/3I",
    );
}

#[test]
fn tips_sharing_a_trunk_each_feel_it_give_under_all_of_them() {
    let (top, strut) = column(3.0);
    let give = give_at(top, &[strut], f32::MIN, 0, (0.0, 0.0));
    close(
        give.lateral_per_mm,
        3.0 * f64::from(LENGTH_MM).powi(3) / (3.0 * second_moment()),
        "three tips on one trunk",
    );
}

#[test]
fn a_taller_support_gives_more_across_than_a_short_one() {
    let mut model = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(4.0, 4.0, 12.0));
    model.append(&box_mesh(
        Vec3::new(30.0, 0.0, 40.0),
        Vec3::new(34.0, 4.0, 42.0),
    ));
    let points = [
        SupportPoint::new(Vec3::new(2.0, 2.0, 10.0)),
        SupportPoint::new(Vec3::new(32.0, 2.0, 40.0)),
    ];
    let fixture = profile();
    let profiles = Profiles::single(&fixture);
    let trees = trees_on(&points, &model, Transform::default(), profiles);
    let bvh = core_geometry::Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let mut found = gives(&trees, &points, &placed, profiles);
    found.sort_by_key(|give| give.point);
    assert_eq!(found.len(), 2, "one give a tip");
    assert!(
        found[1].lateral_per_mm > 10.0 * found[0].lateral_per_mm,
        "a column four times as tall gives far more: {} against {}",
        found[1].lateral_per_mm,
        found[0].lateral_per_mm
    );
}
