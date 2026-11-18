use std::f64::consts::PI;

use super::bend::{Bend, Slice};
use super::frame::{Frame, Stiffness, own, series};
use super::read::{Hold, Layer, read};
use super::section::{Pull, Section};
use super::{CURED_N_PER_MM3, Footing, GREEN_MODULUS_MPA, Spring, Tip, TipLoads};
use crate::layer::Piece;

fn tip(x: f32, y: f32, radius_mm: f64) -> Tip {
    Tip {
        at_mm: [x, y],
        z_mm: 0.0,
        radius_mm,
        spring: None,
        stands_on: None,
    }
}

fn close(got: f64, want: f64) {
    assert!(
        (got - want).abs() < 1e-3 * want.abs().max(1e-6),
        "expected {want}, got {got}"
    );
}

#[test]
fn one_tip_reads_as_the_necks_tension_and_bending() {
    let at = tip(1.0, 2.0, 0.2);
    let section = Section::of(std::iter::once(&at));
    // 2 N acting 3 mm off, with a further 0.5 mm of peel lever.
    let pull = Pull {
        force_n: 2.0,
        moment_x: 2.0 * 4.0,
        moment_y: 2.0 * 2.0,
        peel: [2.0 * 0.5, 0.0, 2.0 * 0.5],
    };
    let area = PI * 0.04;
    let modulus = PI * 0.2f64.powi(3) / 4.0;
    close(
        section.stress_mpa(&at, &pull),
        2.0 / area + 2.0 * 3.5 / modulus,
    );
}

#[test]
fn four_tips_under_a_centred_pull_share_it_equally() {
    let tips = [
        tip(0.0, 0.0, 0.1),
        tip(4.0, 0.0, 0.1),
        tip(0.0, 4.0, 0.1),
        tip(4.0, 4.0, 0.1),
    ];
    let section = Section::of(tips.iter());
    let pull = Pull {
        force_n: 1.0,
        moment_x: 2.0,
        moment_y: 2.0,
        peel: [0.0; 3],
    };
    for tip in &tips {
        close(section.stress_mpa(tip, &pull), 1.0 / (4.0 * PI * 0.01));
    }
}

#[test]
fn a_pull_held_off_to_one_side_loads_the_near_tip_and_lifts_off_the_far_one() {
    let tips = [tip(0.0, 0.0, 0.1), tip(10.0, 0.0, 0.1)];
    let section = Section::of(tips.iter());
    // 1 N over the second tip: by statics all of it there, none on the first.
    let pull = Pull {
        force_n: 1.0,
        moment_x: 10.0,
        ..Pull::default()
    };
    let area = PI * 0.01;
    let near = section.stress_mpa(&tips[1], &pull);
    assert!(
        (near * area - 1.0).abs() < 0.05,
        "the near tip takes the whole newton, got {} N",
        near * area
    );
    assert!(
        section.stress_mpa(&tips[0], &pull) * area < 0.05,
        "the far tip takes next to none of it"
    );
}

fn sprung(x: f32, y: f32, axial_n_per_mm: f64, trunk: u32) -> Tip {
    Tip {
        spring: Some(Spring {
            axial_n_per_mm,
            lateral_n_per_mm: axial_n_per_mm,
            turning_n_mm_per_rad: 1e-9,
            trunk,
            buckles_at_n: 1.0,
        }),
        ..tip(x, y, 0.15)
    }
}

fn held(tips: &[Tip]) -> Vec<(&Tip, Stiffness)> {
    tips.iter()
        .map(|tip| (tip, own(tip).expect("every tip here has a spring")))
        .collect()
}

/// `force` pulling straight down at `x`, `y` on a layer 2 mm up.
fn pulled(force: f64, x: f64, y: f64) -> Layer {
    Layer {
        pull: Pull {
            force_n: force,
            moment_x: force * x,
            moment_y: force * y,
            peel: [0.0; 3],
        },
        weight: (0.0, [0.0; 3]),
        passed: Vec::new(),
        z_mm: 2.0,
    }
}

#[test]
fn a_stiffer_middle_tip_takes_more_of_a_centred_pull() {
    let tips = [
        sprung(-5.0, 0.0, 1.0, 0),
        sprung(0.0, 0.0, 2.0, 1),
        sprung(5.0, 0.0, 1.0, 2),
    ];
    let held = held(&tips);
    let moved = Frame::of(&held)
        .moved(&[([0.0, 0.0, 1.0], [0.0, 0.0, 3.0])])
        .expect("three springs hold the part");
    // Symmetric, so the part only drops: u = F / Σk and the middle takes 2u.
    close(moved.reaction(&tips[1], &held[1].1).axial_n, 0.5);
    close(moved.reaction(&tips[0], &held[0].1).axial_n, 0.25);
}

#[test]
fn a_pull_off_the_middle_of_four_springs_turns_the_layer_above_them_aside() {
    let (k, a, e, h, force) = (2.0, 4.0, 3.0, 20.0, 0.5);
    let tips = [
        sprung(-a as f32, -a as f32, k, 0),
        sprung(a as f32, -a as f32, k, 1),
        sprung(-a as f32, a as f32, k, 2),
        sprung(a as f32, a as f32, k, 3),
    ];
    let moved = Frame::of(&held(&tips))
        .moved(&[([0.0, 0.0, force], [e, 0.0, h])])
        .expect("four springs hold the part");
    // The part turns by F e / (4 k a²) and the layer h above the tips moves that much times h.
    let want = force * e * h / (4.0 * k * a * a);
    let [dx, dy, _] = moved.at([e, 0.0, h]);
    close(dx.hypot(dy), want);
}

#[test]
fn a_square_beam_bends_under_an_end_moment_as_a_cantilever_does() {
    let (side, height, rise, force, e): (f64, f64, f64, f64, f64) = (2.0, 10.0, 0.05, 0.2, 1.5);
    let piece = Piece {
        area_mm2: (side * side) as f32,
        centre_mm: [0.0, 0.0],
        pull_n: 0.0,
        spread_mm2: [
            (side * side / 12.0) as f32,
            0.0,
            (side * side / 12.0) as f32,
        ],
    };
    let slice = Slice::of(std::iter::once(&piece)).expect("a square has a section");
    let mut bend = Bend::default();
    let layers = (height / rise).round() as usize;
    for layer in 0..layers {
        bend.add((layer as f64 + 0.5) * rise, rise, &slice);
    }
    let [x, y] = bend.shift(height, [e, 0.0], force);
    let second_moment = side.powi(4) / 12.0;
    // M H² / (2 E I), with M = F e.
    let want = force * e * height * height / (2.0 * GREEN_MODULUS_MPA * second_moment);
    assert!(
        (x.hypot(y) - want).abs() < 1e-2 * want,
        "expected {want} mm, got {}",
        x.hypot(y)
    );
}

#[test]
fn the_squeeze_on_the_way_down_presses_each_trunk_with_its_share() {
    let tips = [sprung(-3.0, 0.0, 1.0, 7), sprung(3.0, 0.0, 1.0, 7)];
    let refs: Vec<&Tip> = tips.iter().collect();
    let layer = Layer {
        pull: Pull {
            force_n: 0.4,
            ..Pull::default()
        },
        weight: (0.0, [0.0; 3]),
        passed: Vec::new(),
        z_mm: 2.0,
    };
    let felt = read(&refs, &[Hold::Still; 2], &layer, &Bend::default()).felt;
    // Both tips stand on trunk 7, which is pressed with the whole push.
    close(felt[0].squeeze_n, 0.4);
    close(felt[1].squeeze_n, 0.4);
}

#[test]
fn a_tall_part_pulls_its_tip_by_its_own_weight_too() {
    use crate::layer::cure;
    use crate::layer::tests::paint;
    let pitch = core_raster::PixelPitch { x: 0.5, y: 0.5 };
    let square = cure(
        &paint(40, 40, |x, y| {
            u8::from((10..31).contains(&x) && (10..31).contains(&y)) * 255
        }),
        pitch,
    );
    // 21 pixels from the 10th, so the middle pixel's centre is 10.25 mm in.
    let centre = 10.25;
    let tips = [Tip {
        z_mm: 1.0,
        ..tip(centre, centre, 0.3)
    }];
    let (rise, layers) = (0.05f32, 400);
    let first = {
        let mut fold = TipLoads::new(&tips, rise);
        fold.push(0.5 * rise + 1.0, square.clone());
        fold.loads()[0].stress_mpa
    };
    let mut fold = TipLoads::new(&tips, rise);
    for layer in 0..layers {
        fold.push((layer as f32 + 0.5) * rise + 1.0, square.clone());
    }
    let last = fold.loads()[0].stress_mpa;
    let area = PI * 0.09;
    let weight = f64::from(square.pieces()[0].area_mm2)
        * f64::from(rise)
        * f64::from(layers - 1)
        * CURED_N_PER_MM3;
    let grew = (last - first) * area;
    assert!(
        (grew - weight).abs() < 0.02 * weight,
        "the tip carries {weight} N more for the part grown over it, got {grew} N"
    );
}

#[test]
fn a_spring_standing_on_a_part_that_gives_holds_as_the_two_in_series() {
    let diagonal = |k: f64| [[k, 0.0, 0.0], [0.0, k, 0.0], [0.0, 0.0, k]];
    let got = series(diagonal(2.0), diagonal(0.5)).expect("both give a little");
    // 1 / (1/2 + 0.5) = 1 N/mm each way.
    for (axis, row) in got.iter().enumerate() {
        close(row[axis], 1.0);
    }
}

#[test]
fn a_support_standing_on_the_part_it_holds_holds_none_of_it() {
    let tips = [sprung(-3.0, 0.0, 1.0, 0), sprung(3.0, 0.0, 1.0, 1)];
    let refs: Vec<&Tip> = tips.iter().collect();
    let felt = read(
        &refs,
        &[Hold::Still, Hold::Loose],
        &pulled(0.2, -3.0, 0.0),
        &Bend::default(),
    )
    .felt;
    let area = PI * 0.15 * 0.15;
    close(felt[0].stress_mpa, 0.2 / area);
    assert!(
        felt[1].stress_mpa.abs() < 1e-12,
        "a support from the part to itself carries nothing, got {} MPa",
        felt[1].stress_mpa
    );
}

#[test]
fn every_tip_of_a_tree_on_the_model_loads_the_one_contact_it_stands_by() {
    let footing = Some(Footing {
        at_mm: [0.0, 0.0, 1.0],
        radius_mm: 0.15,
    });
    let tips = [
        Tip {
            stands_on: footing,
            ..sprung(-3.0, 0.0, 1.0, 4)
        },
        Tip {
            stands_on: footing,
            ..sprung(3.0, 0.0, 1.0, 4)
        },
    ];
    let refs: Vec<&Tip> = tips.iter().collect();
    let felt = read(
        &refs,
        &[Hold::Still; 2],
        &pulled(0.2, 0.0, 0.0),
        &Bend::default(),
    )
    .felt;
    // Each tip takes half; the foot under both takes all of it through one contact.
    let area = PI * 0.15 * 0.15;
    close(felt[0].stress_mpa, 0.2 / area);
    close(felt[1].stress_mpa, 0.2 / area);
}

#[test]
fn a_support_standing_on_another_part_passes_its_pull_down_to_it() {
    let tips = [Tip {
        stands_on: Some(Footing {
            at_mm: [1.0, 2.0, 0.5],
            radius_mm: 0.15,
        }),
        ..sprung(0.0, 0.0, 1.0, 0)
    }];
    let refs: Vec<&Tip> = tips.iter().collect();
    let on = Hold::On {
        part: 5,
        compliance: [[0.0; 3]; 3],
    };
    let reading = read(&refs, &[on], &pulled(0.3, 0.0, 0.0), &Bend::default());
    let [(part, force, at)] = reading.passed[..] else {
        panic!(
            "one support passes one force down, got {:?}",
            reading.passed
        );
    };
    assert_eq!(part, 5);
    close(force[2], 0.3);
    assert_eq!(at, [1.0, 2.0, 0.5]);
}

/// A 5.5 mm square at `x0` pixels in, at 0.5 mm a pixel.
fn block(x0: u32) -> crate::layer::Cured {
    let lit = |x: u32, y: u32| (x0..x0 + 11).contains(&x) && (10..21).contains(&y);
    crate::layer::cure(
        &crate::layer::tests::paint(50, 30, |x, y| u8::from(lit(x, y)) * 255),
        core_raster::PixelPitch { x: 0.5, y: 0.5 },
    )
}

/// The worst stress on the first of four tips under a lower block while an upper block
/// beside it grows on one support, standing on the lower block's corner or on the plate.
fn under_the_lower_block(standing_on_it: bool) -> f64 {
    let rise = 0.05f32;
    let corners = [[5.25, 5.25], [10.25, 5.25], [5.25, 10.25], [10.25, 10.25]];
    let mut tips: Vec<Tip> = corners
        .iter()
        .zip(0..)
        .map(|([x, y], trunk)| Tip {
            z_mm: 1.0,
            ..sprung(*x, *y, 5.0, trunk)
        })
        .collect();
    tips.push(Tip {
        z_mm: 2.0,
        stands_on: standing_on_it.then_some(Footing {
            at_mm: [5.75, 5.75, 2.0],
            radius_mm: 0.15,
        }),
        ..sprung(17.75, 7.75, 5.0, 9)
    });
    let mut fold = TipLoads::new(&tips, rise);
    for layer in 0..60 {
        let z = 1.0 + (layer as f32 + 0.5) * rise;
        fold.push(z, if z < 2.0 { block(10) } else { block(30) });
    }
    fold.loads()[0].stress_mpa
}

#[test]
fn a_part_standing_on_another_loads_the_tips_under_that_one() {
    let (alone, beside) = (under_the_lower_block(false), under_the_lower_block(true));
    assert!(
        beside > 1.5 * alone,
        "the upper block's pull, passed down by its support, loads the corner tip: {beside} MPa against {alone}"
    );
}
