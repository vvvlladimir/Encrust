use core_geometry::{Aabb, transform_mesh};
use printer_profiles::FrontEdge;

use super::*;

/// Millimetres a placed reference may miss its closed-form size by: the round flats.
const TOLERANCE_MM: Scalar = 0.05;

/// The placed boxes in the order `lanes` lists them: ruler, card, coin, figure.
fn bounds_of(plate: &BuildPlate) -> Vec<Aabb> {
    placed(plate)
        .iter()
        .map(|(mesh, transform)| {
            transform_mesh(mesh, *transform)
                .aabb()
                .expect("a reference is never empty")
        })
        .collect()
}

fn size(aabb: &Aabb) -> Vec3 {
    aabb.maxs - aabb.mins
}

fn near(got: Scalar, expected: Scalar, what: &str) {
    assert!(
        (got - expected).abs() < TOLERANCE_MM,
        "expected the {what} {expected} mm, got {got} mm"
    );
}

#[test]
fn the_card_and_the_coin_keep_their_real_size() {
    let boxes = bounds_of(&BuildPlate::default());
    let card = size(&boxes[1]);
    near(card.x, CARD_MM[0], "card's width");
    near(card.y, CARD_MM[1], "card's length");
    near(card.z, CARD_MM[2], "card's thickness");
    near(size(&boxes[2]).x, COIN_MM[0], "coin's diameter");
}

#[test]
fn the_ruler_is_a_hundred_millimetres_long() {
    near(
        size(&bounds_of(&BuildPlate::default())[0]).y,
        100.0,
        "ruler's length",
    );
}

#[test]
fn the_figure_has_its_eyes_32_mm_over_its_base() {
    let figure = size(&bounds_of(&BuildPlate::default())[3]);
    let head_mm = EYE_LEVEL_MM / 6.0;
    near(
        figure.z,
        BASE_MM[1] + EYE_LEVEL_MM + head_mm / 2.0,
        "figure's height",
    );
    near(figure.x, BASE_MM[0], "base's diameter");
}

#[test]
fn references_stand_on_the_floor_off_the_plate_and_apart_whichever_way_it_faces() {
    for front_edge in [FrontEdge::MinY, FrontEdge::MinX, FrontEdge::MaxX] {
        let plate = BuildPlate {
            front_edge,
            ..BuildPlate::default()
        };
        let boxes = bounds_of(&plate);
        let off_plate = |aabb: &Aabb| {
            aabb.mins.x >= plate.x_mm
                || aabb.maxs.x <= 0.0
                || aabb.mins.y >= plate.y_mm
                || aabb.maxs.y <= 0.0
        };
        for aabb in &boxes {
            near(aabb.mins.z, 0.0, "lowest point");
            assert!(
                off_plate(aabb),
                "{front_edge:?}: expected off the plate, got {aabb:?}"
            );
        }
        let apart = |a: &Aabb, b: &Aabb| {
            a.maxs.x <= b.mins.x
                || b.maxs.x <= a.mins.x
                || a.maxs.y <= b.mins.y
                || b.maxs.y <= a.mins.y
        };
        for (index, one) in boxes.iter().enumerate() {
            for other in &boxes[index + 1..] {
                assert!(
                    apart(one, other),
                    "{front_edge:?}: {one:?} overlaps {other:?}"
                );
            }
        }
    }
}
