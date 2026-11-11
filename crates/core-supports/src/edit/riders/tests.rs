use super::*;
use crate::fixtures::pillar_beside_column;
use crate::pick::Part;
use crate::{ModelSupports, carried};
use core_geometry::{Bvh, Transform};

/// The pillar and the column it stands on, grown on the cube; the pillar is tree 0.
fn grown() -> (
    core_geometry::Mesh,
    [printer_profiles::SupportProfile; 2],
    Vec<SupportTree>,
) {
    let (block, table, points) = pillar_beside_column();
    let trees = crate::fixtures::trees_on(
        &points,
        &block,
        Transform::default(),
        Profiles::new(&table).expect("two groups"),
    );
    assert!(
        trees[0].landing().on_support,
        "the pillar stands on the column"
    );
    (block, table, trees)
}

#[test]
fn the_support_standing_on_another_is_found_and_nothing_stands_on_it() {
    let (_, table, trees) = grown();
    let profiles = Profiles::new(&table).expect("two groups");
    assert_eq!(standing_on(&trees, 1, profiles), vec![0]);
    assert!(standing_on(&trees, 0, profiles).is_empty());
}

#[test]
fn a_support_carried_across_the_plate_takes_the_one_standing_on_it_along() {
    let (block, table, trees) = grown();
    let profiles = Profiles::new(&table).expect("two groups");
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());
    let step = Vec3::new(1.0, 2.0, 0.0);
    let column = carried(&trees[1], &[Part::Trunk], step, &placed, profiles.of(1))
        .expect("the column has room 1 mm further out");

    let moved = followed(&trees, 1, &column, &placed, profiles).expect("the pillar fits");

    assert_eq!(
        moved.len(),
        2,
        "the column and the pillar on it, got {moved:?}"
    );
    let (index, pillar) = &moved[1];
    assert_eq!(*index, 0);
    let expected = trees[0].landing().base + step;
    assert!(
        pillar.landing().base.distance(expected) < 1e-3,
        "the pillar stands where it did on the column, at {expected}, got {}",
        pillar.landing().base
    );
    assert!(pillar.landing().on_support);
    assert!(
        pillar.contacts().eq(trees[0].contacts()),
        "its tip stays on the model"
    );
}

#[test]
fn a_step_that_would_sink_the_support_standing_on_it_is_refused() {
    let (block, table, trees) = grown();
    let profiles = Profiles::new(&table).expect("two groups");
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());
    // The column's trunk run through the middle of the cube, below its top at z = 10.
    let mut column = trees[1].clone();
    column.move_node(column.root_index(), Vec3::new(5.0, 5.0, 8.0));
    column.move_landing(Vec3::new(5.0, 5.0, 0.0));

    assert!(followed(&trees, 1, &column, &placed, profiles).is_none());
}

#[test]
fn taking_hold_of_a_support_freezes_the_one_standing_on_it_too() {
    let (block, table, points) = pillar_beside_column();
    let mut supports = ModelSupports::default();
    for point in points {
        supports.add(point.contact, Transform::default(), point.group);
    }
    supports.refresh(&block, &Bvh::build(&block), Transform::default(), &table);
    let column = supports
        .trees()
        .iter()
        .position(|tree| !tree.landing().on_support)
        .expect("the column stands on the plate");

    let profiles = Profiles::new(&table).expect("two groups");
    let frozen = supports
        .freeze_with_riders(column, Transform::default(), profiles)
        .expect("the column is there to freeze");

    assert!(
        !supports.frozen()[frozen].landing().on_support,
        "the index answered is the column's"
    );
    assert_eq!(supports.frozen().len(), 2, "the pillar is frozen with it");
    assert!(
        supports.points().is_empty(),
        "both points left the automatic run"
    );
}
