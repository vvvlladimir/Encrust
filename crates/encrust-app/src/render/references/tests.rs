use core_geometry::Aabb;
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

fn size_of(material: Material) -> Vec3 {
    let prop = props()
        .iter()
        .find(|prop| prop.material == material)
        .expect("in the set");
    prop.bounds.maxs - prop.bounds.mins
}

#[test]
fn the_can_coin_ruler_and_pen_keep_their_real_size() {
    let can = size_of(Material::Can);
    near(can.x, 2.0 * CAN_RADIUS_MM, "can's diameter");
    near(can.z, CAN_HEIGHT_MM, "can's height");
    let coin = size_of(Material::Coin);
    near(coin.x, COIN_MM[0], "coin's diameter");
    near(coin.z, COIN_MM[1], "coin's thickness");
    near(size_of(Material::Steel).x, RULER_MM[0], "ruler's length");
    near(size_of(Material::Pen).x, PEN_MM, "pen's length");
}

#[test]
fn the_figure_has_its_eyes_32_mm_over_its_base() {
    let height = size_of(Material::Primer).z;
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

fn apart(a: Rect, b: Rect) -> bool {
    a.high.x <= b.low.x || b.high.x <= a.low.x || a.high.y <= b.low.y || b.high.y <= a.low.y
}

fn inside(inner: Rect, outer: Rect) -> bool {
    inner.low.x >= outer.low.x
        && inner.low.y >= outer.low.y
        && inner.high.x <= outer.high.x
        && inner.high.y <= outer.high.y
}

/// A tiny model, a miniature's worth, and one as large as a big plate takes.
fn models() -> [Rect; 3] {
    let square = |half: Scalar| Rect {
        low: Vec2::splat(-half),
        high: Vec2::splat(half),
    };
    [
        square(0.0),
        square(15.0),
        Rect {
            low: Vec2::new(-110.0, -60.0),
            high: Vec2::new(110.0, 60.0),
        },
    ]
}

#[test]
fn the_props_lie_round_the_models_apart_and_on_the_mat() {
    for models in models() {
        let layout = lay_out(models);
        for (index, (prop, rect)) in layout.placed.iter().enumerate() {
            assert!(
                apart(*rect, models),
                "{:?} stands on the models",
                prop.material
            );
            assert!(
                inside(*rect, layout.mat),
                "{:?} is off the mat",
                prop.material
            );
            for (other, other_rect) in &layout.placed[index + 1..] {
                let (one, two) = (prop.material, other.material);
                assert!(apart(*rect, *other_rect), "{one:?} overlaps {two:?}");
            }
        }
        assert!(inside(models, layout.mat), "the models stand off the mat");
    }
}

#[test]
fn the_mat_is_one_sold_whichever_way_round_while_one_is_large_enough() {
    let layout = lay_out(models()[1]);
    let size = layout.mat.high - layout.mat.low;
    let sold = MAT_SIZES_MM.iter().any(|&[long, short]| {
        let fits = |a: Scalar, b: Scalar| (a - b).abs() < TOLERANCE_MM;
        (fits(size.x, long) && fits(size.y, short)) || (fits(size.x, short) && fits(size.y, long))
    });
    assert!(sold, "expected a mat as sold, got {size}");
}

#[test]
fn the_mat_lies_under_the_models_whichever_way_the_plate_faces() {
    let models = Aabb::new(Vec3::new(60.0, 30.0, 0.0), Vec3::new(90.0, 50.0, 40.0));
    for front_edge in [FrontEdge::MinY, FrontEdge::MinX, FrontEdge::MaxX] {
        let plate = BuildPlate {
            front_edge,
            ..BuildPlate::default()
        };
        let (draws, desk) = set(&plate, Some(models));
        let [low_x, low_y, high_x, high_y] = desk.rect;
        let under = low_x <= models.mins.x
            && low_y <= models.mins.y
            && high_x >= models.maxs.x
            && high_y >= models.maxs.y;
        assert!(
            under,
            "{front_edge:?}: the mat {:?} misses the models",
            desk.rect
        );
        assert_eq!(draws.len(), props().len() + 1, "every prop and the mat");
    }
}

#[test]
fn every_shape_is_closed_and_wound_outward() {
    for prop in props() {
        let volume = core_geometry::signed_volume(&prop.shaded.mesh);
        assert!(
            volume > 0.0,
            "{:?}: got a volume of {volume} mm³",
            prop.material
        );
    }
}
