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
    let column = carried(
        &trees[1],
        trees[1].landing(),
        &[Part::Trunk],
        step,
        &placed,
        profiles.of(1),
    )
    .expect("the column has room 1 mm further out");

    let moved = followed(&trees, 1, &column, &placed, profiles).expect("the pillar fits");

    assert_eq!(
        moved.len(),
        2,
        "the column and the pillar on it, got {moved:?}"
    );
    let (index, pillar) = &moved[1];
    assert_eq!(*index, 0);
    // The column's trunk now stands upright over its carried foot, under a knee.
    let (seat, foot) = (pillar.landing().base, column.landing().base);
    assert!(
        (seat - foot).truncate().length() < 1e-3
            && seat.z > foot.z
            && seat.z < column.root().position.z,
        "the pillar stands on the column's trunk over {foot}, got {seat}"
    );
    assert!(pillar.landing().on_support);
    assert!(
        pillar.contacts().eq(trees[0].contacts()),
        "its tip stays on the model"
    );
}

#[test]
fn a_support_standing_on_one_carried_into_the_model_may_not_stand_there() {
    let (block, table, trees) = grown();
    let profiles = Profiles::new(&table).expect("two groups");
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());
    // The column's trunk run through the middle of the cube, below its top at z = 10.
    let mut column = trees[1].clone();
    column.move_node(column.root_index(), Vec3::new(5.0, 5.0, 8.0));
    column.move_landing(Vec3::new(5.0, 5.0, 0.0));

    let moved = followed(&trees, 1, &column, &placed, profiles).expect("the pillar is seated");
    let (_, pillar) = moved
        .iter()
        .find(|(index, _)| *index == 0)
        .expect("the pillar goes with the column");
    assert_eq!(
        crate::allowed(&trees[0], pillar, &placed, profiles.of(0)),
        Err(crate::Refused::Collides),
        "its trunk now ends inside the cube"
    );
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

/// The pillar and the column grown on the cube as a model holds them, and which tree each is.
fn held_on_the_cube() -> (
    ModelSupports,
    [printer_profiles::SupportProfile; 2],
    usize,
    usize,
) {
    let (block, table, points) = pillar_beside_column();
    let mut supports = ModelSupports::default();
    for point in points {
        supports.add(point.contact, Transform::default(), point.group);
    }
    supports.refresh(&block, &Bvh::build(&block), Transform::default(), &table);
    let on_support = |wanted: bool| {
        supports
            .trees()
            .iter()
            .position(|tree| tree.landing().on_support == wanted)
            .expect("one stands on the other")
    };
    let (pillar, column) = (on_support(true), on_support(false));
    (supports, table, pillar, column)
}

#[test]
fn taking_hold_of_a_support_standing_on_another_freezes_that_one_too() {
    let (mut supports, table, pillar, _) = held_on_the_cube();
    let profiles = Profiles::new(&table).expect("two groups");

    let frozen = supports
        .freeze_with_riders(pillar, Transform::default(), profiles)
        .expect("the pillar is there to freeze");

    assert!(supports.frozen()[frozen].landing().on_support);
    assert_eq!(
        supports.frozen().len(),
        2,
        "the column under it is frozen too, so no regrowth moves it from under the pillar"
    );
}

#[test]
fn a_support_whose_host_is_taken_away_goes_back_to_the_automatic_run() {
    let (mut supports, table, _, column) = held_on_the_cube();
    let profiles = Profiles::new(&table).expect("two groups");
    let column = supports
        .freeze_with_riders(column, Transform::default(), profiles)
        .expect("the column is there to freeze");
    assert_eq!(
        supports.thaw_unseated(Transform::default(), profiles),
        0,
        "the pillar still stands on the column"
    );

    supports.remove_frozen(column);

    assert_eq!(supports.thaw_unseated(Transform::default(), profiles), 1);
    assert!(
        supports.frozen().is_empty(),
        "nothing is left standing in the air"
    );
    assert_eq!(
        supports.points().len(),
        1,
        "the pillar's tip is grown again"
    );
}

#[test]
fn a_support_standing_on_another_may_not_lean_onto_it_past_45_degrees() {
    let (block, table, trees) = grown();
    let profiles = Profiles::new(&table).expect("two groups");
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());
    let mut pillar = trees[0].clone();
    let root = pillar.root_index();
    let seat = pillar.landing().base;
    // Its top carried level with the seat and 3 mm out past the column, away from the cube.
    let top = Vec3::new(seat.x + 3.0, seat.y, seat.z + 1.0);
    let step = top - pillar.root().position;
    for node in 0..pillar.nodes().len() {
        let at = pillar.nodes()[node].position + step;
        pillar.move_node(node, at);
    }
    assert_eq!(pillar.root_index(), root);

    assert_eq!(
        crate::allowed(&trees[0], &pillar, &placed, profiles.of(0)),
        Err(crate::Refused::Leans),
        "growth runs nothing into a support flatter than 45 degrees"
    );
}

#[test]
fn a_support_carried_away_from_one_on_it_seats_that_one_higher_within_45_degrees() {
    let (block, table, trees) = grown();
    let profiles = Profiles::new(&table).expect("two groups");
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());
    // Away from the pillar, which growth seated as high as 45 degrees let it, and no further
    // than growth runs a strut across.
    let column = carried(
        &trees[1],
        trees[1].landing(),
        &[Part::Trunk],
        Vec3::X * 0.4,
        &placed,
        profiles.of(1),
    )
    .expect("the column has room further out");

    let moved = followed(&trees, 1, &column, &placed, profiles).expect("the pillar is seated");

    let (_, pillar) = &moved[1];
    assert_eq!(
        crate::allowed(&trees[0], pillar, &placed, profiles.of(0)),
        Ok(())
    );
    assert_eq!(
        standing_on(&[column, pillar.clone()], 0, profiles),
        vec![1],
        "it still stands on the column"
    );
}

#[test]
fn a_column_carried_by_its_trunk_leaves_its_tip_where_it_holds() {
    let (block, table, trees) = grown();
    let profiles = Profiles::new(&table).expect("two groups");
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());

    let column = carried(
        &trees[1],
        trees[1].landing(),
        &[Part::Trunk],
        Vec3::X * 0.4,
        &placed,
        profiles.of(1),
    )
    .expect("the column has room further out");

    assert!(
        column.contacts().eq(trees[1].contacts()),
        "a tip the hand did not carry is not pulled onto the cube's corner"
    );
}

#[test]
fn a_support_grown_again_stands_on_a_frozen_one_beside_it_and_off_it_once_that_goes() {
    let (block, table, points) = pillar_beside_column();
    let mut supports = ModelSupports::default();
    supports.add(points[1].contact, Transform::default(), points[1].group);
    let bvh = Bvh::build(&block);
    supports.refresh(&block, &bvh, Transform::default(), &table);
    let column = supports
        .freeze(0, Transform::default())
        .expect("the column is there to freeze");

    supports.add(points[0].contact, Transform::default(), points[0].group);
    supports.refresh(&block, &bvh, Transform::default(), &table);
    let on_support = |supports: &ModelSupports| {
        supports
            .trees()
            .iter()
            .filter(|tree| tree.landing().on_support)
            .count()
    };
    assert_eq!(
        on_support(&supports),
        1,
        "the pillar grown after the column froze stands on it"
    );

    supports.remove_frozen(column);
    supports.refresh(&block, &bvh, Transform::default(), &table);
    assert_eq!(
        on_support(&supports),
        0,
        "and grows again off it once it went"
    );
}
