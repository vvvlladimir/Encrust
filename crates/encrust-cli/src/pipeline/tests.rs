use super::Watch;
use super::place::{OrientSummary, centred, euler_degrees, place};
use crate::args::ImportArgs;
use crate::exit::{Stop, is_cancelled};
use crate::profiles::Chosen;
use core_engine::SupportPlan;
use core_geometry::{Mesh, Quat, Scalar, Vec3};
use printer_profiles::{MaterialProfile, SupportProfile};

fn summary(rotation: Quat) -> OrientSummary {
    OrientSummary {
        rotation,
        peak_mm2: 0.0,
        overhang_mm2: 0.0,
        on_plate: false,
    }
}

#[test]
fn a_model_pushed_under_the_plate_is_stood_back_on_it() {
    // What relief does to a model's own bottom, which is why centring is redone after it.
    let sunk = Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, -0.5),
            Vec3::new(1.0, 0.0, -0.5),
            Vec3::new(0.0, 1.0, 2.0),
        ],
        vec![[0, 1, 2]],
    );
    let stood = centred(sunk, None).expect("the mesh has vertices");
    let bounds = stood.aabb().expect("the mesh has vertices");

    assert!(
        bounds.mins.z.abs() < 1.0e-6,
        "nothing under the plate is cut, so --center has to end on it: {}",
        bounds.mins.z
    );
}

#[derive(clap::Parser)]
struct Flags {
    #[command(flatten)]
    import: ImportArgs,
}

#[test]
fn a_rotate_that_tips_a_seated_model_stands_it_on_supports() {
    use clap::Parser;

    let corners = (0..8)
        .map(|at| {
            Vec3::new(
                (at & 1) as Scalar,
                ((at >> 1) & 1) as Scalar,
                (at >> 2) as Scalar,
            ) * 10.0
        })
        .collect();
    let faces = vec![
        [0, 2, 1],
        [1, 2, 3],
        [4, 5, 6],
        [5, 7, 6],
        [0, 1, 4],
        [1, 5, 4],
        [2, 6, 3],
        [3, 6, 7],
        [0, 4, 2],
        [2, 4, 6],
        [1, 3, 5],
        [3, 7, 5],
    ];
    let cube = Mesh::new(corners, faces);
    let chosen = Chosen {
        printer: None,
        material: MaterialProfile::default(),
        invented_exposure: false,
    };
    let supports = SupportProfile::shipped("medium").expect("the medium preset ships");
    let seated = |flags: &[&str]| {
        let args = Flags::parse_from(flags).import;
        let (_, oriented) = place(
            cube.clone(),
            &args,
            Some(&SupportPlan::of(supports.clone())),
            &chosen,
            &Watch::quiet(&Stop::default()),
        )
        .expect("a cube orients");
        oriented.expect("--orient reports its turn").on_plate
    };

    assert!(seated(&["encrust", "--orient"]), "a cube needs no supports");
    assert!(
        !seated(&["encrust", "--orient", "--rotate", "30,0,0"]),
        "tipped onto an edge, it no longer rests on a face"
    );
    assert!(
        seated(&["encrust", "--orient", "--rotate", "0,0,30"]),
        "a turn about the vertical keeps the face down"
    );
}

#[test]
fn an_oriented_model_is_stood_on_the_plate_over_the_middle_it_stood_over() {
    use clap::Parser;

    // A pyramid on its tip, its base 100..110 by 50..60 mm: it prints best turned over.
    let base = [(100.0, 50.0), (110.0, 50.0), (110.0, 60.0), (100.0, 60.0)];
    let mut vertices: Vec<Vec3> = base.iter().map(|&(x, y)| Vec3::new(x, y, 30.0)).collect();
    vertices.push(Vec3::new(105.0, 55.0, 0.0));
    let faces = vec![
        [0, 1, 2],
        [0, 2, 3],
        [1, 0, 4],
        [2, 1, 4],
        [3, 2, 4],
        [0, 3, 4],
    ];
    let args = Flags::parse_from(["encrust", "--orient"]).import;
    let chosen = Chosen {
        printer: None,
        material: MaterialProfile::default(),
        invented_exposure: false,
    };

    let (mesh, oriented) = place(
        Mesh::new(vertices, faces),
        &args,
        None,
        &chosen,
        &Watch::quiet(&Stop::default()),
    )
    .expect("a pyramid orients");
    let bounds = mesh.aabb().expect("the mesh has vertices");
    let middle = (bounds.mins + bounds.maxs) / 2.0;

    assert!(oriented.is_some(), "--orient reports the turn it applied");
    assert!(
        bounds.mins.z.abs() < 1.0e-4,
        "stood on z = 0: {}",
        bounds.mins.z
    );
    assert!(
        (middle.x - 105.0).abs() < 1.0e-3 && (middle.y - 55.0).abs() < 1.0e-3,
        "the base's middle, (105, 55), stays where it was: {middle}"
    );
}

#[test]
fn a_turn_about_one_axis_names_it_and_the_flag_that_repeats_it() {
    let quarter = summary(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    assert_eq!(
        quarter.describe(),
        "turned 90 degrees around X (--rotate 90,0,0)"
    );
}

#[test]
fn the_angles_reported_are_the_angles_rotate_would_apply() {
    let turn = Vec3::new(-30.0, 0.0, 45.0);
    let summary = summary(euler_degrees(turn));
    assert_eq!(summary.rotate_degrees(), turn, "--rotate repeats the turn");
    assert!(
        summary.axis_name().is_none(),
        "two axes are not one of them"
    );
}

#[test]
fn an_orientation_search_stopped_beforehand_is_cancelled() {
    use clap::Parser;

    let args = Flags::parse_from(["encrust", "--orient"]).import;
    let chosen = Chosen {
        printer: None,
        material: MaterialProfile::default(),
        invented_exposure: false,
    };
    let stop = Stop::default();
    stop.request();
    let error = place(
        core_geometry::cuboid(Vec3::ZERO, Vec3::new(10.0, 20.0, 30.0)),
        &args,
        None,
        &chosen,
        &Watch::quiet(&stop),
    )
    .err()
    .expect("the search was told to stop first");
    assert!(
        is_cancelled(&error),
        "a stop is a cancellation, not a failure"
    );
}
