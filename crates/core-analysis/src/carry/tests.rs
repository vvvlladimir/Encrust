use std::f64::consts::PI;

use super::Tip;
use super::section::{Pull, Section};

fn tip(x: f32, y: f32, radius_mm: f64) -> Tip {
    Tip {
        at_mm: [x, y],
        z_mm: 0.0,
        radius_mm,
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
