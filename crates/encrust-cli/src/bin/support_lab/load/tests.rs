use super::*;
use core_geometry::{Transform, Vec3, cuboid, transform_mesh};
use printer_profiles::Catalogue;
use std::f64::consts::PI;

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
        peel: 2.0 * 0.5,
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
        peel: 0.0,
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

fn at(mesh: &Mesh, x: f32, y: f32, z: f32) -> Mesh {
    transform_mesh(mesh, Transform::from_translation(Vec3::new(x, y, z)))
}

fn slab_loads(mesh: &Mesh, tips: &[Tip]) -> Vec<Load> {
    let catalogue = Catalogue::bundled().expect("the bundled profiles load");
    let printer = catalogue
        .printer("elegoo-saturn-4-ultra")
        .expect("the bench's printer ships")
        .profile
        .clone();
    loads(mesh, tips, &printer, &MaterialProfile::default()).expect("a slab cuts")
}

#[test]
fn a_slab_on_one_centred_tip_is_loaded_by_its_whole_section() {
    let side = 6.0;
    let slab = at(
        &cuboid(Vec3::ZERO, Vec3::new(side, side, 1.0)),
        50.0,
        50.0,
        2.0,
    );
    let radius = 0.2;
    let seat = [Tip {
        at_mm: [53.0, 53.0],
        z_mm: 2.0,
        radius_mm: radius,
    }];
    let got = slab_loads(&slab, &seat)[0];
    assert!(got.seated, "the tip stands under the slab");

    let area = f64::from(side * side);
    let polar = area * f64::from(2.0 * side * side) / 12.0;
    let force = core_analysis::pull_n(area, polar);
    let gyration = (polar / area).sqrt();
    let want =
        force / (PI * radius * radius) + force * (gyration / 2.0) / (PI * radius.powi(3) / 4.0);
    assert!(
        (got.stress_mpa - want).abs() < 0.02 * want,
        "a {side} mm square centred over one tip is {want} MPa, got {}",
        got.stress_mpa
    );
}

#[test]
fn parts_share_only_once_they_have_joined() {
    // Two 2 mm posts on a tip each, under a bridge 4 mm up: alone, then together.
    let post = cuboid(Vec3::ZERO, Vec3::new(2.0, 2.0, 4.0));
    let bridge = cuboid(Vec3::ZERO, Vec3::new(12.0, 2.0, 1.0));
    let mut joined = at(&post, 50.0, 50.0, 2.0);
    joined.append(&at(&post, 60.0, 50.0, 2.0));
    joined.append(&at(&bridge, 50.0, 50.0, 6.0));
    let radius = 0.15;
    let seats = [51.0, 61.0].map(|x| Tip {
        at_mm: [x, 51.0],
        z_mm: 2.0,
        radius_mm: radius,
    });
    let got = slab_loads(&joined, &seats);
    for load in &got {
        assert!(load.seated, "each post stands on its tip");
        assert!(
            load.z_mm.is_some_and(|z| z > 5.9),
            "the bridge at 6 mm is what pulls hardest, got {:?}",
            load.z_mm
        );
    }
    close(got[0].stress_mpa, got[1].stress_mpa);
}

#[test]
fn a_tip_under_a_part_standing_on_the_plate_carries_nothing() {
    let block = cuboid(Vec3::ZERO, Vec3::new(5.0, 5.0, 2.0));
    let seat = [Tip {
        at_mm: [52.0, 52.0],
        z_mm: 0.0,
        radius_mm: 0.2,
    }];
    let got = slab_loads(&at(&block, 50.0, 50.0, 0.0), &seat)[0];
    assert!(got.seated);
    assert_eq!(got.z_mm, None, "the plate holds what stands on it");
}
