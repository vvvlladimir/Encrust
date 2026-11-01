use core_geometry::{Aabb, transform_mesh};
use printer_profiles::FrontEdge;

use super::shapes::*;
use super::*;

/// Millimetres a reference may miss its closed-form size by: the flats of anything round.
const TOLERANCE_MM: Scalar = 0.05;

fn near(got: Scalar, expected: Scalar, what: &str) {
    assert!(
        (got - expected).abs() < TOLERANCE_MM,
        "expected the {what} {expected} mm, got {got} mm"
    );
}

/// The box a reference fills on the mat, in the mat's own millimetres.
fn on_mat(reference: &Reference) -> Aabb {
    let at = Transform::from_scale_rotation_translation(Vec3::ONE, reference.turn, reference.at);
    transform_mesh(&reference.shaded.mesh, at)
        .aabb()
        .expect("a reference is never empty")
}

fn find(material: Material) -> Aabb {
    let reference = desk()
        .iter()
        .find(|reference| reference.material == material)
        .expect("on the desk");
    on_mat(reference)
}

#[test]
fn the_can_coin_ruler_and_pen_keep_their_real_size() {
    let size = |material| {
        let aabb = find(material);
        aabb.maxs - aabb.mins
    };
    let can = size(Material::Can);
    near(can.x, 2.0 * CAN_RADIUS_MM, "can's diameter");
    near(can.z, CAN_HEIGHT_MM, "can's height");
    let coin = size(Material::Coin);
    near(coin.x, COIN_MM[0], "coin's diameter");
    near(coin.z, COIN_MM[1], "coin's thickness");
    near(size(Material::Steel).y, RULER_MM[0], "ruler's length");
    near(size(Material::Pen).y, PEN_MM, "pen's length");
}

#[test]
fn the_figure_has_its_eyes_32_mm_over_its_base() {
    let figure = find(Material::Primer);
    let height = figure.maxs.z - figure.mins.z;
    near(
        height,
        BASE_MM[1] + EYE_LEVEL_MM + EYE_LEVEL_MM / 12.0,
        "figure's height",
    );
}

#[test]
fn the_banana_is_as_long_as_a_banana() {
    let steps = 200;
    let length: Scalar = (0..steps)
        .map(|step| {
            let at = |step: usize| banana_spine(step as Scalar / steps as Scalar).truncate();
            at(step).distance(at(step + 1))
        })
        .sum();
    let expected = BANANA_ARC_MM * (BANANA_TO - BANANA_FROM);
    assert!(
        (length - expected).abs() < 0.5,
        "expected the spine {expected} mm, got {length} mm"
    );
}

#[test]
fn the_props_lie_on_the_mat_inside_its_edges_and_apart() {
    let props: Vec<Aabb> = desk()
        .iter()
        .filter(|reference| reference.material != Material::Mat)
        .map(on_mat)
        .collect();
    for aabb in &props {
        near(aabb.mins.z, MAT_MM[2], "lowest point");
        let inside = aabb.mins.x >= 0.0
            && aabb.mins.y >= 0.0
            && aabb.maxs.x <= MAT_MM[0]
            && aabb.maxs.y <= MAT_MM[1];
        assert!(inside, "expected on the mat, got {aabb:?}");
    }
    let apart = |a: &Aabb, b: &Aabb| {
        a.maxs.x <= b.mins.x || b.maxs.x <= a.mins.x || a.maxs.y <= b.mins.y || b.maxs.y <= a.mins.y
    };
    for (index, one) in props.iter().enumerate() {
        for other in &props[index + 1..] {
            assert!(apart(one, other), "{one:?} overlaps {other:?}");
        }
    }
}

#[test]
fn the_mat_stays_off_the_plate_whichever_way_it_faces() {
    for front_edge in [FrontEdge::MinY, FrontEdge::MinX, FrontEdge::MaxX] {
        let plate = BuildPlate {
            front_edge,
            ..BuildPlate::default()
        };
        let (mat, at) = placed(&plate)[0];
        let aabb = transform_mesh(&mat.shaded.mesh, at)
            .aabb()
            .expect("the mat is never empty");
        let off = aabb.mins.x >= plate.x_mm
            || aabb.maxs.x <= 0.0
            || aabb.mins.y >= plate.y_mm
            || aabb.maxs.y <= 0.0;
        assert!(off, "{front_edge:?}: expected off the plate, got {aabb:?}");
        near(aabb.mins.z, 0.0, "mat's underside");
    }
}

#[test]
fn every_prop_casts_its_shadow_inside_the_mat() {
    for shadow in shadows() {
        let [x0, y0, x1, y1] = shadow.ends;
        for (x, y) in [(x0, y0), (x1, y1)] {
            let inside = (0.0..=MAT_MM[0]).contains(&x) && (0.0..=MAT_MM[1]).contains(&y);
            assert!(inside, "expected a shadow on the mat, got ({x}, {y})");
        }
    }
}

#[test]
fn every_shape_is_closed_and_wound_outward() {
    for reference in desk() {
        let volume = core_geometry::signed_volume(&reference.shaded.mesh);
        assert!(
            volume > 0.0,
            "{:?}: got a volume of {volume} mm³",
            reference.material
        );
    }
}
