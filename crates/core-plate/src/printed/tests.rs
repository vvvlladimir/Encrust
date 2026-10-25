use super::*;
use crate::cut::cut;
use crate::orient::{OrientSettings, orient_printed};
use crate::score::OrientGoal;
use core_geometry::Scalar;
use std::f32::consts::{PI, TAU};

const RINGS: usize = 24;
const SEGMENTS: usize = 48;
const OUTER_MM: Scalar = 30.0;
const CAVITY_MM: Scalar = 28.0;
const MIN_CUP_MM2: Scalar = 25.0;

/// A ball about the origin, wound outward, or inward for a cavity.
fn ball(radius: Scalar, inward: bool) -> Mesh {
    let mut vertices = vec![Vec3::Z * radius];
    for ring in 1..RINGS {
        let polar = PI * ring as Scalar / RINGS as Scalar;
        for segment in 0..SEGMENTS {
            let around = TAU * segment as Scalar / SEGMENTS as Scalar;
            vertices.push(
                Vec3::new(
                    polar.sin() * around.cos(),
                    polar.sin() * around.sin(),
                    polar.cos(),
                ) * radius,
            );
        }
    }
    vertices.push(Vec3::NEG_Z * radius);
    let at = |ring: usize, segment: usize| (1 + (ring - 1) * SEGMENTS + segment % SEGMENTS) as u32;
    let bottom = (vertices.len() - 1) as u32;

    let mut faces = Vec::new();
    for segment in 0..SEGMENTS {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
        faces.push([bottom, at(RINGS - 1, segment + 1), at(RINGS - 1, segment)]);
        for ring in 1..RINGS - 1 {
            let [a, b, c, d] = [
                at(ring, segment),
                at(ring + 1, segment),
                at(ring + 1, segment + 1),
                at(ring, segment + 1),
            ];
            faces.extend([[a, b, c], [a, c, d]]);
        }
    }
    wound(vertices, faces, inward)
}

/// A 3 mm hole from `from` to `to`, wound inward the way a cut is.
fn hole(from: Vec3, to: Vec3) -> Mesh {
    let along = (to - from).normalize();
    let (side, up) = along.any_orthonormal_pair();
    let rim = |centre: Vec3| {
        (0..SEGMENTS).map(move |segment| {
            let around = TAU * segment as Scalar / SEGMENTS as Scalar;
            centre + (side * around.cos() + up * around.sin()) * 1.5
        })
    };
    let mut vertices: Vec<Vec3> = rim(from).chain(rim(to)).collect();
    vertices.extend([from, to]);
    let near = |segment: usize| (segment % SEGMENTS) as u32;
    let far = |segment: usize| (SEGMENTS + segment % SEGMENTS) as u32;
    let (near_cap, far_cap) = (2 * SEGMENTS as u32, 2 * SEGMENTS as u32 + 1);

    let mut faces = Vec::new();
    for segment in 0..SEGMENTS {
        faces.push([near(segment), near(segment + 1), far(segment + 1)]);
        faces.push([near(segment), far(segment + 1), far(segment)]);
        faces.push([near_cap, near(segment + 1), near(segment)]);
        faces.push([far_cap, far(segment), far(segment + 1)]);
    }
    wound(vertices, faces, true)
}

fn wound(vertices: Vec<Vec3>, mut faces: Vec<[u32; 3]>, inward: bool) -> Mesh {
    if inward {
        for face in &mut faces {
            face.swap(1, 2);
        }
    }
    Mesh::new(vertices, faces)
}

/// The ball with its cavity appended, the way a hollowed model is kept.
fn shell() -> Mesh {
    let mut shell = ball(OUTER_MM, false);
    let cavity = ball(CAVITY_MM, true);
    let offset = shell.vertices.len() as u32;
    shell.vertices.extend_from_slice(&cavity.vertices);
    shell.faces.extend(
        cavity
            .faces
            .iter()
            .map(|face| face.map(|index| index + offset)),
    );
    shell
}

#[test]
fn a_sealed_cavity_is_a_suction_cup_even_on_supports() {
    let (outer, shell) = (ball(OUTER_MM, false), shell());
    let printed = Printed {
        shell: Some(&shell),
        ..Printed::solid(&outer)
    };
    let solid =
        cut(&Printed::solid(&outer), Vec3::NEG_Z, 24, false, MIN_CUP_MM2).expect("a ball cuts");
    let hollow = cut(&printed, Vec3::NEG_Z, 24, false, MIN_CUP_MM2).expect("a shell cuts");

    assert!(solid.cup_mm2 <= 0.0, "a solid ball seals nothing");
    // The cavity's widest section is a disc of its own radius, pi * 28^2 = 2463 mm2.
    assert!(hollow.cup_mm2 > 2000.0, "got {}", hollow.cup_mm2);
    assert!(
        hollow.pull_n < solid.pull_n / 2.0,
        "a 2 mm ring pulls far less than the disc it is the rim of"
    );
}

#[test]
fn a_hole_lets_the_air_in_only_from_under_the_cavity() {
    let (outer, shell) = (ball(OUTER_MM, false), shell());
    let drilled = hole(Vec3::NEG_Z * 31.0, Vec3::NEG_Z * 27.0);
    let printed = Printed {
        shell: Some(&shell),
        cuts: Some(&drilled),
        ..Printed::solid(&outer)
    };

    let under = cut(&printed, Vec3::NEG_Z, 24, false, MIN_CUP_MM2).expect("a shell cuts");
    assert!(
        under.cup_mm2 <= 0.0,
        "the hole is under it, got {}",
        under.cup_mm2
    );
    let over = cut(&printed, Vec3::Z, 24, false, MIN_CUP_MM2).expect("a shell cuts");
    assert!(
        over.cup_mm2 > 2000.0,
        "the hole prints last, got {}",
        over.cup_mm2
    );
}

#[test]
fn a_hollow_ball_is_stood_on_its_drain_hole() {
    let (outer, shell) = (ball(OUTER_MM, false), shell());
    let drilled = hole(Vec3::X * 31.0, Vec3::X * 27.0);
    let printed = Printed {
        outer: &outer,
        shell: Some(&shell),
        cuts: Some(&drilled),
        vents: &[Vec3::X],
    };
    let settings = OrientSettings {
        on_supports: true,
        preset: OrientGoal::LeastPeel.into(),
        ..OrientSettings::default()
    };

    let found = orient_printed(&printed, &settings).expect("a shell can be oriented");
    assert_eq!(
        found.score.cup_mm2,
        Some(0.0),
        "the way it stands seals nothing"
    );
    let mouth = found.rotation * Vec3::X;
    assert!(mouth.z < -0.99, "the hole points at the plate, got {mouth}");
}
