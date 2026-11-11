use std::f64::consts::PI;

use core_geometry::{Transform, cuboid, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use printer_profiles::{Catalogue, SupportProfile};

use super::*;

const LAYER_MM: f32 = 0.05;

fn at(mesh: &Mesh, x: f32, y: f32, z: f32) -> Mesh {
    transform_mesh(mesh, Transform::from_translation(Vec3::new(x, y, z)))
}

fn sliced(mesh: &Mesh) -> Sliced {
    let settings = SliceSettings {
        layer_height: LAYER_MM,
        ..SliceSettings::default()
    };
    PlaneSliceEngine.slice(mesh, &settings).expect("a box cuts")
}

fn loads(mesh: &Mesh, tips: &[Tip]) -> Vec<Load> {
    carried(&sliced(mesh), mesh, LAYER_MM, tips).expect("a box reads")
}

fn seat(x: f32, y: f32, z: f32, radius_mm: f64) -> Tip {
    Tip {
        at_mm: [x, y],
        z_mm: z,
        radius_mm,
    }
}

fn close(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() < tolerance * want,
        "{what} is {want} MPa, got {got}"
    );
}

#[test]
fn a_slab_on_one_centred_tip_is_loaded_by_its_whole_section() {
    let side = 6.0_f64;
    let slab = at(
        &cuboid(Vec3::ZERO, Vec3::new(6.0, 6.0, 1.0)),
        50.0,
        50.0,
        2.0,
    );
    let radius = 0.2;
    let got = loads(&slab, &[seat(53.0, 53.0, 2.0, radius)])[0];
    assert!(got.seated, "the tip stands under the slab");

    let area = side * side;
    let polar = area * 2.0 * side * side / 12.0;
    let force = core_analysis::pull_n(area, polar);
    let lever = (polar / area).sqrt() / 2.0;
    let want = force / (PI * radius * radius) + force * lever / (PI * radius.powi(3) / 4.0);
    close(
        got.stress_mpa,
        want,
        0.02,
        "a 6 mm square centred over one tip",
    );
}

#[test]
fn a_strip_on_a_row_of_tips_is_levered_across_by_its_width() {
    // The row is weak across, but the film's last grip across a 1 mm strip is only
    // w / sqrt(24) off its centre; along, L / sqrt(24).
    let (length, width) = (12.0_f64, 1.0_f64);
    let strip = at(
        &cuboid(Vec3::ZERO, Vec3::new(12.0, 1.0, 1.0)),
        50.0,
        50.0,
        2.0,
    );
    let radius = 0.2_f64;
    let got = loads(&strip, &[51.0, 61.0].map(|x| seat(x, 50.5, 2.0, radius)));

    let area = length * width;
    let force = core_analysis::pull_n(area, area * (length * length + width * width) / 12.0);
    let disc = PI * radius * radius;
    let along_mm4 = 2.0 * disc * (25.0 + radius * radius / 4.0);
    let across_mm4 = 2.0 * disc * radius * radius / 4.0;
    let (lever_along, lever_across) = (length / 24.0_f64.sqrt(), width / 24.0_f64.sqrt());
    let bend = (lever_along / along_mm4).max(lever_across / across_mm4);
    let want = force / (2.0 * disc) + force * (lever_along * 5.0 / along_mm4 + radius * bend);
    for load in got {
        assert!(load.seated, "both tips stand under the strip");
        close(load.stress_mpa, want, 0.03, "each end of the strip");
    }
}

#[test]
fn parts_share_only_once_they_have_joined() {
    // Two 2 mm posts on a tip each, under a bridge 4 mm up.
    let post = cuboid(Vec3::ZERO, Vec3::new(2.0, 2.0, 4.0));
    let mut joined = at(&post, 50.0, 50.0, 2.0);
    joined.append(&at(&post, 60.0, 50.0, 2.0));
    joined.append(&at(
        &cuboid(Vec3::ZERO, Vec3::new(12.0, 2.0, 1.0)),
        50.0,
        50.0,
        6.0,
    ));
    let got = loads(&joined, &[51.0, 61.0].map(|x| seat(x, 51.0, 2.0, 0.15)));
    for load in &got {
        assert!(load.seated, "each post stands on its tip");
        assert!(
            load.z_mm.is_some_and(|z| z > 5.9),
            "the bridge at 6 mm is what pulls hardest, got {:?}",
            load.z_mm
        );
    }
    close(
        got[0].stress_mpa,
        got[1].stress_mpa,
        1e-3,
        "the second post, as the first,",
    );
}

#[test]
fn a_tip_under_a_part_standing_on_the_plate_carries_nothing() {
    let block = at(
        &cuboid(Vec3::ZERO, Vec3::new(5.0, 5.0, 2.0)),
        50.0,
        50.0,
        0.0,
    );
    let got = loads(&block, &[seat(52.0, 52.0, 0.0, 0.2)])[0];
    assert!(got.seated, "the tip touches the block");
    assert_eq!(got.z_mm, None, "the plate holds what stands on it");
}

/// A 20 mm slab 5 mm over the plate, its stack, and a single need under its middle.
struct Slab {
    mesh: Mesh,
    bvh: Bvh,
    stack: Sliced,
}

impl Slab {
    fn new(side_mm: f32) -> Self {
        let mesh = at(
            &cuboid(Vec3::ZERO, Vec3::new(side_mm, side_mm, 1.0)),
            50.0,
            50.0,
            5.0,
        );
        let bvh = Bvh::build(&mesh);
        let stack = sliced(&mesh);
        Self { mesh, bvh, stack }
    }

    fn underside(&self) -> Underside<'_> {
        Underside {
            stack: &self.stack,
            layer_mm: LAYER_MM,
            surface: &self.mesh,
            bvh: &self.bvh,
            blocked: None,
        }
    }

    fn middle(&self) -> Vec<Need> {
        let bounds = self.mesh.aabb().expect("a slab has bounds");
        let middle = (bounds.mins + bounds.maxs) / 2.0;
        vec![Need::new(
            Vec3::new(middle.x, middle.y, bounds.mins.z),
            Reason::POINT,
        )]
    }
}

fn sized() -> SupportPlan {
    let catalogue = Catalogue::bundled().expect("the bundled profiles load");
    let auto = catalogue
        .autos()
        .next()
        .expect("an automatic profile ships")
        .profile
        .clone();
    SupportPlan::sized(auto, &catalogue).expect("its shapes ship")
}

fn light() -> SupportPlan {
    SupportPlan::of(SupportProfile::shipped("light").expect("light ships"))
}

#[test]
fn a_sized_tip_too_weak_for_its_slab_is_built_wider_then_given_a_neighbour() {
    let slab = Slab::new(20.0);
    let mut needs = slab.middle();
    let done = strengthen(slab.underside(), &mut needs, &[], &sized(), &mut || true)
        .expect("the slab reads");
    assert_eq!(
        done,
        Strengthened {
            rounds: MOST_ROUNDS,
            thickened: AUTO_SIZES - 1,
            added: 1
        },
        "two sizes up, then a neighbour in the last round"
    );
    assert_eq!(usize::from(needs[0].heavier), AUTO_SIZES - 1);
}

#[test]
fn under_one_profile_a_weak_tip_gets_neighbours_on_the_same_underside() {
    let slab = Slab::new(20.0);
    let mut needs = slab.middle();
    let done = strengthen(slab.underside(), &mut needs, &[], &light(), &mut || true)
        .expect("the slab reads");
    assert_eq!(done.thickened, 0, "one profile has no wider size");
    assert!(done.added > 0, "the lone tip under a 20 mm slab is weak");
    for need in &needs[1..] {
        assert!(
            (need.contact.z - 5.0).abs() < 1e-3,
            "a neighbour seats on the underside at 5 mm, got {}",
            need.contact
        );
    }
}

#[test]
fn a_tip_weak_round_after_round_is_never_given_the_same_neighbour_twice() {
    let slab = Slab::new(20.0);
    let mut needs = slab.middle();
    strengthen(slab.underside(), &mut needs, &[], &light(), &mut || true).expect("the slab reads");
    for (index, need) in needs.iter().enumerate() {
        for other in &needs[index + 1..] {
            assert!(
                need.contact.distance(other.contact) > 1e-3,
                "two needs share the contact {}",
                need.contact
            );
        }
    }
}

#[test]
fn a_tip_with_margin_to_spare_is_left_alone() {
    let slab = Slab::new(1.0);
    let mut needs = slab.middle();
    let done = strengthen(slab.underside(), &mut needs, &[], &sized(), &mut || true)
        .expect("the slab reads");
    assert_eq!(
        done,
        Strengthened::default(),
        "a 1 mm square is light for any tip"
    );
    assert_eq!(needs, slab.middle());
}

#[test]
fn a_run_told_to_stop_strengthens_nothing() {
    let slab = Slab::new(20.0);
    let mut needs = slab.middle();
    let done = strengthen(slab.underside(), &mut needs, &[], &sized(), &mut || false)
        .expect("nothing is read");
    assert_eq!(done, Strengthened::default());
}

#[test]
fn no_neighbour_seats_after_the_layer_the_tip_is_worst_at() {
    let slab = Slab::new(10.0);
    let tip = Vec3::new(55.0, 55.0, 5.0);
    let taken = Buckets::new(1.5);
    let found = neighbour(slab.underside(), tip, (4.9, 1.5), &taken);
    assert_eq!(
        found, None,
        "the underside at 5 mm is over the worst layer at 4.9 mm"
    );
    let found =
        neighbour(slab.underside(), tip, (6.0, 1.5), &taken).expect("the underside is all round");
    assert!(
        (found.truncate().distance(tip.truncate()) - 1.5).abs() < 1e-4,
        "expected a point 1.5 mm off the tip, got {found}"
    );
}

#[test]
fn sizes_thicken_up_to_the_widest_and_never_a_steadying_support() {
    let plan = sized();
    let need = |reason, heavier| Need {
        heavier,
        ..Need::new(Vec3::ZERO, reason)
    };
    let point = Reason::POINT;
    assert!(
        heavier(&plan, &need(point, 0)),
        "the lightest size has a wider one"
    );
    let widest = u8::try_from(AUTO_SIZES - 1).expect("a few sizes");
    assert!(
        !heavier(&plan, &need(point, widest)),
        "the widest gets a neighbour"
    );
    assert!(
        !heavier(&plan, &need(Reason::Steady, 0)),
        "a steadying support stays so"
    );
    assert!(
        !heavier(&light(), &need(point, 0)),
        "one profile has nothing wider"
    );
    let shown = Need {
        shown: true,
        ..need(point, 0)
    };
    assert!(
        !heavier(&plan, &shown),
        "a tip on the side meant to be seen gets a neighbour"
    );
}
