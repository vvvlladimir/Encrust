use super::*;
use crate::layer::cure;
use crate::layer::tests::paint;
use core_raster::PixelPitch;

/// Half a millimetre a pixel, so a 60 pixel panel holds parts heavy enough to pull.
const PITCH: PixelPitch = PixelPitch { x: 0.5, y: 0.5 };

fn fold(held: usize, layers: &[Cured]) -> Measured {
    let mut measured = Measured::new(held);
    for layer in layers {
        measured.push(layer.clone(), 0.05);
    }
    measured
}

fn layer(lit: impl Fn(u32, u32) -> bool) -> Cured {
    cure(&paint(60, 60, |x, y| u8::from(lit(x, y)) * 255), PITCH)
}

fn square(x0: u32, y0: u32, side: u32) -> Cured {
    layer(move |x, y| (x0..x0 + side).contains(&x) && (y0..y0 + side).contains(&y))
}

/// A 20 by 20 pixel square with a 10 by 10 hole in its middle, at 0.5 mm a pixel.
fn ring() -> Cured {
    layer(|x, y| {
        let outer = (10..30).contains(&x) && (10..30).contains(&y);
        let hole = (15..25).contains(&x) && (15..25).contains(&y);
        outer && !hole
    })
}

fn cups(measured: &Measured) -> Vec<Risk> {
    measured
        .risks()
        .into_iter()
        .filter(|risk| matches!(risk.kind, RiskKind::SuctionCup { .. }))
        .collect()
}

/// `count` rings, a millimetre of them at 20.
fn rings(count: usize) -> Vec<Cured> {
    vec![ring(); count]
}

#[test]
fn a_ring_on_the_plate_is_one_suction_cup() {
    let measured = fold(0, &rings(20));
    let found = cups(&measured);
    assert_eq!(found.len(), 1, "{found:?}");
    let RiskKind::SuctionCup {
        area_mm2,
        volume_mm3,
    } = found[0].kind
    else {
        unreachable!("filtered to cups");
    };
    // The hole is 10 px of 0.5 mm a side, 25 mm2, twenty layers of 0.05 mm.
    assert!((area_mm2 - 25.0).abs() < 1e-3, "got {area_mm2}");
    assert!((volume_mm3 - 25.0).abs() < 1e-3, "got {volume_mm3}");
}

#[test]
fn a_shallow_cup_is_too_little_resin_to_name() {
    // 25 mm2 three layers deep is under 4 mm3.
    assert!(cups(&fold(0, &rings(3))).is_empty());
}

#[test]
fn a_narrow_cup_is_too_little_pull_to_name() {
    let narrow = layer(|x, y| {
        let outer = (10..30).contains(&x) && (10..30).contains(&y);
        let hole = (18..22).contains(&x) && (18..22).contains(&y);
        outer && !hole
    });
    // 4 mm2 however deep: the counter of a letter.
    assert!(cups(&fold(0, &vec![narrow; 200])).is_empty());
}

#[test]
fn a_ring_on_posts_lets_the_air_in_from_below() {
    let posts = layer(|x, y| {
        let corner = |at: u32| (at..at + 3).contains(&x);
        (corner(10) || corner(27)) && ((10..13).contains(&y) || (27..30).contains(&y))
    });
    let measured = fold(0, &[&[posts][..], &rings(20)].concat());
    assert!(cups(&measured).is_empty(), "air runs in between the posts");
}

#[test]
fn a_ring_over_a_solid_floor_is_a_cup() {
    let measured = fold(0, &[&[square(0, 0, 40)][..], &rings(20)].concat());
    assert_eq!(
        cups(&measured).len(),
        1,
        "the floor seals it as the plate does"
    );
}

#[test]
fn a_needle_is_named_once_where_it_thins() {
    // At 0.02 mm a pixel, a 3 px square is 0.06 mm across: past any support tip.
    let fine = PixelPitch { x: 0.02, y: 0.02 };
    let base = cure(
        &paint(60, 60, |x, y| u8::from(x < 40 && y < 40) * 255),
        fine,
    );
    let needle = cure(
        &paint(60, 60, |x, y| {
            u8::from((20..23).contains(&x) && (20..23).contains(&y)) * 255
        }),
        fine,
    );
    let thins = |needle_layers: usize| {
        let stack = [vec![base.clone()], vec![needle.clone(); needle_layers]].concat();
        fold(0, &stack)
            .risks()
            .into_iter()
            .filter(|risk| matches!(risk.kind, RiskKind::Thin { .. }))
            .collect::<Vec<Risk>>()
    };
    // A millimetre is twenty layers of 0.05 mm.
    let thin = thins(30);
    assert_eq!(thin.len(), 1, "{thin:?}");
    assert_eq!(thin[0].layer, 1, "named where it starts");
    assert!(
        thins(5).is_empty(),
        "a quarter of a millimetre is the tip of something, not a wire"
    );
}

#[test]
fn the_volume_is_each_layer_at_its_own_thickness() {
    let mut measured = Measured::new(0);
    measured.push(square(0, 0, 40), 0.05);
    measured.push(square(0, 0, 40), 0.1);
    // 40 px at 0.5 mm is 20 mm a side.
    assert!((measured.volume_mm3() - 400.0 * 0.15).abs() < 1e-3);
    assert_eq!(measured.layer_count(), 2);
}

#[test]
fn the_hardest_pull_and_the_largest_growth_name_their_layers() {
    let strip = layer(|_, y| y < 2);
    let measured = fold(0, &[strip, square(0, 0, 40), square(0, 0, 45)]);
    assert_eq!(measured.hardest_pull().map(|peak| peak.layer), Some(2));
    assert_eq!(measured.largest_growth().map(|peak| peak.layer), Some(1));
}

#[test]
fn the_bottom_block_is_never_named() {
    let measured = fold(2, &[square(0, 0, 40), square(0, 0, 40), square(0, 0, 10)]);
    assert_eq!(measured.hardest_pull().map(|peak| peak.layer), Some(2));
    assert_eq!(
        measured.largest_growth(),
        None,
        "the stack only shrinks after it"
    );
}

#[test]
fn a_piece_that_starts_over_nothing_is_named_once_as_an_island() {
    let base = square(0, 0, 10);
    let both = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
    let measured = fold(0, &[base, both.clone(), both]);
    let risks = measured.risks();
    assert_eq!(risks.len(), 1, "{risks:?}");
    assert_eq!(risks[0].layer, 1);
    assert!(matches!(risks[0].kind, RiskKind::Island { .. }));
    assert_eq!(measured.worst().map(|risk| risk.layer), Some(1));
}

#[test]
fn a_column_standing_straight_carries_no_lever() {
    let column: Vec<Cured> = (0..6).map(|_| square(20, 20, 20)).collect();
    assert!(fold(0, &column).risks().is_empty());
}

#[test]
fn a_wide_slab_off_to_one_side_of_a_thin_neck_is_a_lever() {
    // A pin one pixel across in the corner, and a 20 mm slab hanging off it.
    let pin = square(0, 0, 1);
    let slab = square(0, 0, 40);
    let measured = fold(0, &[pin.clone(), pin, slab]);
    let risks = measured.risks();
    assert_eq!(risks.len(), 1, "{risks:?}");
    let RiskKind::Lever {
        lever_mm, neck_mm2, ..
    } = risks[0].kind
    else {
        panic!("a lever, not {:?}", risks[0].kind);
    };
    assert_eq!(risks[0].layer, 2);
    assert!(
        (neck_mm2 - 0.25).abs() < 1e-4,
        "the pin is the neck: {neck_mm2}"
    );
    // The slab's centre is 9.75 mm off the pin on both axes, and half its gyration —
    // a 20 mm square gyrates 20 / sqrt(6) about its centre — is added to that.
    let offset = 9.75 * std::f32::consts::SQRT_2;
    let gyration = 20.0 / 6.0_f32.sqrt();
    assert!(
        (lever_mm - offset - gyration / 2.0).abs() < 0.05,
        "the slab's centre is far off the pin: {lever_mm}"
    );
}

#[test]
fn removing_islands_takes_the_island_alone_and_names_nothing() {
    let base = square(0, 0, 10);
    let both = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
    let mut measured = Measured::new(0).removing_islands();
    let taken: Vec<usize> = [base, both.clone(), both]
        .into_iter()
        .map(|layer| measured.push(layer, 0.05).len())
        .collect();
    // The second part is 30 rows tall; only its first layer cured over nothing.
    assert_eq!(taken, vec![0, 30, 0], "what stood on the island stays");
    assert_eq!(measured.removed_islands(), 1);
    assert_eq!(measured.removed_layers(), 1);
    let risks = measured.risks();
    assert_eq!(risks.len(), 1, "{risks:?}");
    assert!(
        matches!(risks[0].kind, RiskKind::Floating { .. }) && risks[0].layer == 2,
        "what stood on it is named once, as hanging over nothing"
    );
    assert!(measured.removed_from(2).is_empty());
    // The base on three layers, 5 x 5 mm, and the part's second layer, 15 x 15 mm.
    let expected_mm3 = 25.0 * 0.15 + 225.0 * 0.05;
    assert!((measured.volume_mm3() - expected_mm3).abs() < 1e-3);
}

#[test]
fn a_run_of_layers_pulling_past_the_limit_is_named_once_at_its_hardest() {
    // A 30 mm square pulls about 4 N; a pitch of 3 mm makes the same pixels 180 mm.
    let big = PixelPitch { x: 3.0, y: 3.0 };
    let wide = |side: u32| {
        cure(
            &paint(60, 60, |x, y| u8::from(x < side && y < side) * 255),
            big,
        )
    };
    let mut measured = Measured::new(0);
    for side in [10, 40, 60, 40, 10, 50] {
        measured.push(wide(side), 0.05);
    }
    let peels: Vec<usize> = measured
        .risks()
        .iter()
        .filter(|risk| matches!(risk.kind, RiskKind::Peel { .. }))
        .map(|risk| risk.layer)
        .collect();
    assert_eq!(
        peels,
        vec![2, 5],
        "one run peaking at layer 2, and layer 5 alone"
    );
}

#[test]
fn a_plate_centred_on_a_stem_as_thin_as_a_wire_is_still_a_lever() {
    // The coupon of the manual run: a 30 x 30 mm plate on a 1.5 mm stem. Its centre
    // sits over the stem, so only the peel's own arm finds it (P-08).
    let pitch = PixelPitch { x: 0.05, y: 0.05 };
    let round = |cx: f64, cy: f64, radius: f64| {
        cure(
            &paint(700, 700, |x, y| {
                let (dx, dy) = (f64::from(x) + 0.5 - cx, f64::from(y) + 0.5 - cy);
                u8::from(dx * dx + dy * dy <= radius * radius) * 255
            }),
            pitch,
        )
    };
    let square = |side: u32| {
        cure(
            &paint(700, 700, |x, y| {
                let inside = (350 - side / 2..350 + side / 2).contains(&x)
                    && (350 - side / 2..350 + side / 2).contains(&y);
                u8::from(inside) * 255
            }),
            pitch,
        )
    };
    let mut measured = Measured::new(0);
    for _ in 0..3 {
        measured.push(round(350.0, 350.0, 15.0), 0.05);
    }
    measured.push(square(600), 0.05);

    let risks = measured.risks();
    assert_eq!(risks.len(), 1, "{risks:?}");
    let RiskKind::Lever {
        stress_mpa,
        neck_mm2,
        ..
    } = risks[0].kind
    else {
        panic!("a lever, not {:?}", risks[0].kind);
    };
    assert!(
        (neck_mm2 - 1.77).abs() < 0.05,
        "the stem is the neck: {neck_mm2}"
    );
    assert!(
        stress_mpa > NECK_LIMIT_MPA as f32,
        "past the limit: {stress_mpa} MPa"
    );
}

#[test]
fn each_pass_takes_one_layer_more_of_what_hangs() {
    let base = square(0, 0, 10);
    let both = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
    let stack = [base, both.clone(), both.clone(), both.clone(), both];
    let fold = |passes: u32| {
        let mut measured = Measured::new(0).removing_hanging(passes);
        let taken: Vec<usize> = stack
            .iter()
            .map(|layer| measured.push(layer.clone(), 0.05).len())
            .collect();
        (taken, measured)
    };

    let (taken, measured) = fold(1);
    assert_eq!(
        taken,
        vec![0, 30, 30, 0, 0],
        "the island and one layer over it"
    );
    assert_eq!(
        measured.removed_islands(),
        1,
        "one island, however many passes"
    );
    let risks = measured.risks();
    assert_eq!(risks.len(), 1, "{risks:?}");
    assert_eq!(
        risks[0].layer, 3,
        "what is left hanging is named where it starts"
    );

    let (taken, measured) = fold(3);
    assert_eq!(taken, vec![0, 30, 30, 30, 30], "three passes reach the top");
    assert!(
        measured.risks().is_empty(),
        "nothing is left hanging to name"
    );
}

#[test]
fn a_floating_part_loses_its_first_layer_alone_however_many_it_has() {
    let base = square(0, 0, 10);
    let floating = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
    let mut measured = Measured::new(0).removing_islands();
    // The base alone, then four layers with a second part beside it, then the base
    // alone again, then the part back: two parts, over five layers.
    for layer in [
        base.clone(),
        floating.clone(),
        floating.clone(),
        floating.clone(),
        floating.clone(),
        base,
        floating,
    ] {
        measured.push(layer, 0.05);
    }
    assert_eq!(measured.removed_islands(), 2);
    assert_eq!(measured.removed_layers(), 2, "the first layer of each part");
}

#[test]
fn the_same_slab_centred_on_a_broad_neck_is_not() {
    let post = square(15, 15, 30);
    let slab = square(5, 5, 50);
    assert!(fold(0, &[post.clone(), post, slab]).risks().is_empty());
}
