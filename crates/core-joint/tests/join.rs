#![expect(
    clippy::expect_used,
    reason = "a test fails loudly on a block that does not cut"
)]

use core_csg::CsgError;
use core_geometry::{Mesh, Plane, Scalar, Vec3, cuboid, cut, diagnose, signed_volume};
use core_joint::{Halves, JointError, JointKind, Joints, Pegs, cut_and_join, join};

const PI: Scalar = std::f32::consts::PI;

/// A 40 by 10 by 20 mm block, cut through the middle of its height.
fn block() -> (Mesh, Plane) {
    let mesh = cuboid(Vec3::ZERO, Vec3::new(40.0, 10.0, 20.0));
    (mesh, Plane::at(Vec3::Z, 10.0).expect("+Z is a direction"))
}

/// The block hollowed to a 2 mm wall: its outside, and the outside with the cavity turned
/// inward inside it.
fn hollow_block() -> (Mesh, Mesh, Plane) {
    let (solid, plane) = block();
    let cavity = cuboid(Vec3::splat(2.0), Vec3::new(38.0, 8.0, 18.0));
    let mut shell = solid.clone();
    let offset = shell.vertices.len() as u32;
    shell.vertices.extend(&cavity.vertices);
    shell.faces.extend(
        cavity
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );
    (solid, shell, plane)
}

fn joints(kind: JointKind, pegs: Pegs) -> Joints {
    Joints {
        kind: Some(kind),
        pegs,
        ..Joints::default()
    }
}

/// Two 4 mm pegs 6 mm long, so the volumes have a closed form.
fn fixed_pegs() -> Pegs {
    Pegs {
        auto_size: false,
        diameter_mm: 4.0,
        length_mm: 6.0,
        keyed: false,
        ..Pegs::default()
    }
}

fn solid_volume(mesh: &Mesh, what: &str) -> Scalar {
    let report = diagnose(mesh);
    assert_eq!(report.boundary_edges, 0, "{what} is open: {report:?}");
    signed_volume(mesh)
}

fn near(actual: Scalar, expected: Scalar, share: Scalar, why: &str) {
    assert!(
        (actual - expected).abs() <= share * expected.abs(),
        "{why}: expected {expected}, got {actual}"
    );
}

#[test]
fn pins_stand_on_the_lower_half_and_sockets_go_into_the_upper() {
    let (mesh, plane) = block();
    let halves = Halves {
        cut: cut(&mesh, plane),
        plane,
        printed: &mesh,
        solid: None,
    };
    let joined = join(halves, &joints(JointKind::Pins, fixed_pegs())).expect("a block joins");
    assert_eq!((joined.placed, joined.bare), (2, 0));

    let top = joined
        .below
        .vertices
        .iter()
        .map(|vertex| vertex.z)
        .fold(0.0, Scalar::max);
    near(top, 16.0, 1e-4, "6 mm pins stand on the cut at 10 mm");
    // Two pins of radius 2 and 6 mm, less their vents and chamfers.
    let pins = solid_volume(&joined.below, "the lower half") - 4000.0;
    near(pins, 2.0 * PI * 4.0 * 6.0, 0.08, "two pins of pi r2 h");
    // Two sockets of radius 2.1 and 6.3 mm, plus their mouths.
    let sockets = 4000.0 - solid_volume(&joined.above, "the upper half");
    near(
        sockets,
        2.0 * PI * 2.1 * 2.1 * 6.3,
        0.08,
        "two sockets of pi r2 h",
    );
    assert!(joined.loose.is_empty(), "a pin is part of its half");
}

#[test]
fn an_automatic_pin_is_half_as_wide_as_the_face_it_stands_on() {
    let (mesh, plane) = block();
    let halves = Halves {
        cut: cut(&mesh, plane),
        plane,
        printed: &mesh,
        solid: None,
    };
    let pegs = Pegs {
        auto_size: true,
        keyed: false,
        ..Pegs::default()
    };
    let joined = join(halves, &joints(JointKind::Pins, pegs)).expect("a block joins");
    assert!(joined.placed > 0, "a 10 mm wide face holds a pin");
    let top = joined
        .below
        .vertices
        .iter()
        .map(|vertex| vertex.z)
        .fold(0.0, Scalar::max);
    // A 10 mm face holds a 10 mm circle: a pin about 5 mm wide and 7.5 mm long.
    assert!(
        (16.5..=17.5 + 1e-3).contains(&top),
        "pins of about 5 mm by 7.5 mm stand on the cut at 10 mm, got a top at {top}"
    );
}

#[test]
fn dowels_leave_sockets_in_both_halves_and_print_beside_them() {
    let (mesh, plane) = block();
    let halves = Halves {
        cut: cut(&mesh, plane),
        plane,
        printed: &mesh,
        solid: None,
    };
    let joined = join(halves, &joints(JointKind::Dowels, fixed_pegs())).expect("a block joins");
    assert_eq!(joined.loose.len(), 2, "one dowel a socket pair");
    for (half, what) in [(&joined.below, "lower"), (&joined.above, "upper")] {
        let taken = 4000.0 - solid_volume(half, what);
        // Half a 6 mm dowel each side, and 0.3 mm of clearance and glue under it.
        near(
            taken,
            2.0 * PI * 2.1 * 2.1 * 3.3,
            0.1,
            "two sockets of pi r2 h",
        );
    }
    let dowel = solid_volume(&joined.loose[0].mesh, "a dowel");
    near(dowel, PI * 4.0 * 6.0, 0.1, "a dowel of pi r2 h");
}

#[test]
fn a_keyed_set_has_one_narrower_dowel() {
    let (mesh, plane) = block();
    let halves = Halves {
        cut: cut(&mesh, plane),
        plane,
        printed: &mesh,
        solid: None,
    };
    let pegs = Pegs {
        keyed: true,
        ..fixed_pegs()
    };
    let joined = join(halves, &joints(JointKind::Dowels, pegs)).expect("a block joins");
    let volumes: Vec<Scalar> = joined
        .loose
        .iter()
        .map(|dowel| solid_volume(&dowel.mesh, "a dowel"))
        .collect();
    near(
        volumes[0] / volumes[1],
        0.49,
        0.05,
        "the keyed dowel is 0.7 as wide, so 0.49 of the volume",
    );
}

#[test]
fn magnets_sit_in_sockets_their_own_size_either_side() {
    let (mesh, plane) = block();
    let halves = Halves {
        cut: cut(&mesh, plane),
        plane,
        printed: &mesh,
        solid: None,
    };
    let joined = join(halves, &joints(JointKind::Magnets, Pegs::default())).expect("a block joins");
    assert_eq!(joined.placed, 2);
    // A 5 by 2 mm magnet with 0.1 mm round it and under it.
    let taken = 4000.0 - solid_volume(&joined.above, "the upper half");
    near(
        taken,
        2.0 * PI * 2.6 * 2.6 * 2.1,
        0.1,
        "two sockets of pi r2 h",
    );
}

#[test]
fn a_hollow_model_carries_its_pins_on_pillars_over_the_cavity() {
    let (solid, shell, plane) = hollow_block();
    let halves = Halves {
        cut: cut(&shell, plane),
        plane,
        printed: &shell,
        solid: Some(&solid),
    };
    let lower_before = signed_volume(&halves.cut.below);
    let joined = join(halves, &joints(JointKind::Pins, fixed_pegs())).expect("a shell joins");
    assert_eq!(
        joined.placed, 2,
        "the 2 mm wall alone is too thin for a 4 mm pin"
    );
    let gained = solid_volume(&joined.below, "the lower half") - lower_before;
    assert!(
        gained > 2.0 * PI * 4.0 * 6.0 * 1.5,
        "pillars down to the floor as well as the pins, gained {gained}"
    );
}

#[test]
fn a_thin_wall_cut_beside_a_thick_body_still_takes_a_key() {
    let (solid, plane) = block();
    let cavity = cuboid(Vec3::splat(1.4), Vec3::new(38.6, 8.6, 18.6));
    let body = cuboid(Vec3::new(50.0, 0.0, 0.0), Vec3::new(70.0, 20.0, 20.0));
    let mut both = solid.clone();
    for (other, inward) in [(&cavity, true), (&body, false)] {
        let offset = both.vertices.len() as u32;
        both.vertices.extend(&other.vertices);
        both.faces.extend(other.faces.iter().map(|[a, b, c]| {
            if inward {
                [a + offset, c + offset, b + offset]
            } else {
                [a + offset, b + offset, c + offset]
            }
        }));
    }
    let halves = Halves {
        cut: cut(&both, plane),
        plane,
        printed: &both,
        solid: None,
    };
    let key = Joints {
        kind: Some(JointKind::Key),
        ..Joints::default()
    };
    let joined = join(halves, &key).expect("a shell and a block join");
    // The 20 mm body must not hold the 1.4 mm wall's tongue to its own 0.9 mm inset.
    assert_eq!((joined.placed, joined.bare), (2, 0));
}

#[test]
fn a_thin_shell_takes_a_key_round_its_wall() {
    let (_, shell, plane) = hollow_block();
    let halves = Halves {
        cut: cut(&shell, plane),
        plane,
        printed: &shell,
        solid: None,
    };
    let lower_before = signed_volume(&halves.cut.below);
    let joined = join(
        halves,
        &Joints {
            kind: Some(JointKind::Key),
            ..Joints::default()
        },
    )
    .expect("a shell joins");
    assert_eq!((joined.placed, joined.bare), (1, 0));
    // The 2 mm wall narrows the 1.2 mm tongue to its middle 1 mm, 2 mm high, round a
    // 2 × (38 + 8) = 92 mm ring; the 0.1 mm grid reads each side to half a cell.
    let tongue = solid_volume(&joined.below, "the lower half") - lower_before;
    near(
        tongue,
        92.0 * 1.0 * 2.0,
        0.15,
        "a tongue of length × width × height",
    );
}

#[test]
fn a_face_too_thin_for_a_peg_is_left_as_it_was() {
    let mesh = cuboid(Vec3::ZERO, Vec3::new(40.0, 3.0, 20.0));
    let plane = Plane::at(Vec3::Z, 10.0).expect("+Z is a direction");
    let halves = Halves {
        cut: cut(&mesh, plane),
        plane,
        printed: &mesh,
        solid: None,
    };
    let joined = join(halves, &joints(JointKind::Pins, fixed_pegs())).expect("nothing to build");
    assert_eq!((joined.placed, joined.bare), (0, 1));
    near(
        signed_volume(&joined.below),
        1200.0,
        1e-4,
        "the half is untouched",
    );
}

#[test]
fn a_half_that_is_not_a_solid_cannot_be_joined() {
    let (mut mesh, plane) = block();
    // A stray triangle above the block, clear of every ray from the cut, opens the upper half.
    let stray = mesh.vertices.len() as u32;
    mesh.vertices.extend([
        Vec3::new(100.0, 0.0, 15.0),
        Vec3::new(101.0, 0.0, 15.0),
        Vec3::new(100.0, 1.0, 15.0),
    ]);
    mesh.faces.push([stray, stray + 1, stray + 2]);
    let halves = Halves {
        cut: cut(&mesh, plane),
        plane,
        printed: &mesh,
        solid: None,
    };
    let error = join(halves, &joints(JointKind::Pins, fixed_pegs())).expect_err("an open half");
    assert!(
        matches!(error, JointError::Boolean(CsgError::NotSolid(_))),
        "got {error:?}"
    );
}

#[test]
fn a_size_that_cannot_be_built_is_refused_by_name() {
    let (mesh, plane) = block();
    for (pegs, what) in [
        (
            Pegs {
                diameter_mm: Scalar::NAN,
                ..fixed_pegs()
            },
            "peg diameter",
        ),
        (
            Pegs {
                clearance_mm: -0.1,
                ..fixed_pegs()
            },
            "peg clearance",
        ),
    ] {
        let halves = Halves {
            cut: cut(&mesh, plane),
            plane,
            printed: &mesh,
            solid: None,
        };
        let error =
            join(halves, &joints(JointKind::Pins, pegs)).expect_err("a size it cannot build");
        assert!(
            matches!(error, JointError::BadSize { what: named, .. } if named == what),
            "got {error:?}"
        );
    }
}

#[test]
fn a_plane_that_misses_the_model_cuts_nothing_and_one_through_it_joins_the_halves() {
    let (mesh, plane) = block();
    let away = Plane::at(Vec3::Z, 50.0).expect("+Z is a direction");
    assert!(
        cut_and_join(&mesh, None, away, None).is_none(),
        "a plane over the block"
    );
    let cut = cut_and_join(
        &mesh,
        None,
        plane,
        Some(&joints(JointKind::Pins, fixed_pegs())),
    )
    .expect("the plane crosses the block");
    assert!(cut.unjoined.is_none(), "a block takes pins");
    assert!(cut.joined.placed > 0);
}
