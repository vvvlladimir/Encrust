use core_geometry::{Vec3, cuboid, signed_volume};

use super::*;
use crate::drain::{Channel, DrainHole, HoleKind, HoleShape, drill};

/// A square hole 2 mm a side straight down through the middle of a 20 by 20 by 10 slab.
fn square_hole_through() -> DrainHole {
    DrainHole {
        at: Vec3::new(10.0, 10.0, 10.0),
        axis: Vec3::NEG_Z,
        diameter_mm: 2.0 * std::f32::consts::SQRT_2,
        depth_mm: 12.0,
        taper: 1.0,
        lift_mm: 0.0,
        shape: HoleShape::Polygon { sides: 4 },
        turn_deg: 45.0,
        kind: HoleKind::Drain,
    }
}

fn slab() -> Mesh {
    cuboid(Vec3::ZERO, Vec3::new(20.0, 20.0, 10.0))
}

#[test]
fn a_hole_through_a_slab_takes_out_its_bore_exactly() {
    let cuts = drill(&[square_hole_through()], &[]).expect("the hole drills");
    let left = drilled(&slab(), &cuts).expect("a slab is a solid");
    let volume = signed_volume(&left);
    // 4000 mm3 of slab less a 2 by 2 mm square bore through its 10 mm.
    assert!(
        (volume - 3960.0).abs() < 1e-2,
        "the slab less its 40 mm3 bore is 3960 mm3, got {volume}"
    );
}

#[test]
fn a_channel_and_the_holes_it_crosses_are_taken_out_as_one() {
    let channel = Channel {
        points: vec![Vec3::new(2.0, 10.0, 5.0), Vec3::new(18.0, 10.0, 5.0)],
        diameter_mm: 2.0 * std::f32::consts::SQRT_2,
        shape: HoleShape::Polygon { sides: 4 },
        turn_deg: 45.0,
    };
    let cuts = drill(&[square_hole_through()], &[channel]).expect("both cut");
    let left = drilled(&slab(), &cuts).expect("a slab is a solid");
    let volume = signed_volume(&left);
    // The 40 mm3 bore and a 2 by 2 by 16 mm tunnel, the up to 8 mm3 they share once.
    assert!(
        volume > 4000.0 - 40.0 - 64.0 + 1.0 && volume <= 4000.0 - 40.0 - 64.0 + 8.0 + 1e-2,
        "the crossing is taken out once, got {volume}"
    );
}

#[test]
fn a_surface_with_no_inside_leaves_nothing_to_drill() {
    let sheet = Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(20.0, 0.0, 10.0),
            Vec3::new(20.0, 20.0, 10.0),
        ],
        vec![[0, 1, 2]],
    );
    let cuts = drill(&[square_hole_through()], &[]).expect("the hole drills");
    assert_eq!(drilled(&sheet, &cuts), Err(VolumeError::NothingLeft));
}

#[test]
fn a_hole_that_takes_the_whole_model_leaves_nothing() {
    let tiny = cuboid(Vec3::new(9.5, 9.5, 4.0), Vec3::new(10.5, 10.5, 5.0));
    let cuts = drill(&[square_hole_through()], &[]).expect("the hole drills");
    assert_eq!(drilled(&tiny, &cuts), Err(VolumeError::NothingLeft));
}

#[test]
fn a_drilled_shell_keeps_its_cavity_apart_from_the_bore() {
    // A 20 mm box with a 10 mm cavity in its middle wound inward, and the hole down
    // through the top wall into it.
    let mut shell = cuboid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(20.0, 20.0, 20.0));
    let outer_faces = shell.faces.len();
    let cavity = cuboid(Vec3::new(5.0, 5.0, 5.0), Vec3::new(15.0, 15.0, 15.0));
    let offset = shell.vertices.len() as u32;
    shell.vertices.extend(&cavity.vertices);
    shell.faces.extend(
        cavity
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );
    let hole = DrainHole {
        at: Vec3::new(10.0, 10.0, 20.0),
        depth_mm: 8.0,
        ..square_hole_through()
    };
    let cuts = drill(&[hole], &[]).expect("the hole drills");
    let cut = Drilled::of(&shell, Some(outer_faces..shell.faces.len()), &cuts)
        .expect("a box with a cavity is a solid");
    let range = cut.cavity.expect("the cavity is carried over");
    let cavity_mesh = Mesh::new(cut.mesh.vertices.clone(), cut.mesh.faces[range].to_vec());
    // The cavity's faces less the 4 mm2 of roof the bore opened at z = 15, and no bore
    // wall: the open roof counts the 1000 mm3 short by 4 * 15 / 3 = 20.
    let volume = -signed_volume(&cavity_mesh);
    assert!(
        (volume - 980.0).abs() < 1e-2,
        "only the cavity's own faces, open where the bore meets it, got {volume}"
    );
}

#[test]
fn a_drilled_shell_keeps_its_infill_after_its_cavity() {
    // A 30 mm box, a 20 mm cavity in it wound inward and a 6 mm block of infill in the
    // middle of that, with the hole down through the top wall into the cavity.
    let mut shell = cuboid(Vec3::ZERO, Vec3::splat(30.0));
    let outer_faces = shell.faces.len();
    let cavity = cuboid(Vec3::splat(5.0), Vec3::splat(25.0));
    let offset = shell.vertices.len() as u32;
    shell.vertices.extend(&cavity.vertices);
    shell.faces.extend(
        cavity
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );
    let cavity_end = shell.faces.len();
    shell.append(&cuboid(Vec3::splat(12.0), Vec3::splat(18.0)));
    let hole = DrainHole {
        at: Vec3::new(8.0, 8.0, 30.0),
        depth_mm: 8.0,
        ..square_hole_through()
    };
    let cuts = drill(&[hole], &[]).expect("the hole drills");
    let cut = Drilled::of(&shell, Some(outer_faces..cavity_end), &cuts)
        .expect("a box with a cavity is a solid");
    let range = cut.cavity.expect("the cavity is carried over");
    let infill = Mesh::new(
        cut.mesh.vertices.clone(),
        cut.mesh.faces[range.end..].to_vec(),
    );
    let volume = signed_volume(&infill);
    // The 6 mm block alone, 216 mm3, untouched by a bore that passes beside it.
    assert!(
        (volume - 216.0).abs() < 1e-2,
        "the infill follows the cavity to the end, got {volume} mm3 there"
    );
}

#[test]
fn cuts_not_laid_as_drill_lays_them_are_refused() {
    let block = cuboid(Vec3::new(9.0, 9.0, 4.0), Vec3::new(11.0, 11.0, 12.0));
    assert_eq!(drilled(&slab(), &block), Err(VolumeError::NotCuts));
}

#[test]
fn only_the_pieces_a_cut_reaches_are_drilled() {
    let mut two = slab();
    two.append(&cuboid(
        Vec3::new(40.0, 0.0, 0.0),
        Vec3::new(50.0, 10.0, 10.0),
    ));
    let cuts = drill(&[square_hole_through()], &[]).expect("the hole drills");
    let left = drilled_pieces(&two, &cuts);
    let volume = signed_volume(&left);
    // The slab loses its 40 mm3 bore; the 1000 mm3 block beside it is untouched.
    assert!(
        (volume - (3960.0 + 1000.0)).abs() < 1e-2,
        "only the slab is drilled, got {volume}"
    );
}

#[test]
fn a_piece_between_two_holes_is_not_drilled() {
    let far = DrainHole {
        at: Vec3::new(70.0, 10.0, 10.0),
        ..square_hole_through()
    };
    let mut three = slab();
    three.append(&cuboid(
        Vec3::new(60.0, 0.0, 0.0),
        Vec3::new(80.0, 20.0, 10.0),
    ));
    let between = cuboid(Vec3::new(30.0, 5.0, 2.0), Vec3::new(50.0, 15.0, 8.0));
    three.append(&between);
    let cuts = drill(&[square_hole_through(), far], &[]).expect("the holes drill");
    let left = drilled_pieces(&three, &cuts);
    let block = split(&between);
    assert!(
        split(&left).iter().any(|piece| block.contains(piece)),
        "the block in the box round both holes, but in neither, comes back as it went in"
    );
    assert!(
        (signed_volume(&left) - (2.0 * 3960.0 + 1200.0)).abs() < 1e-2,
        "each slab loses its 40 mm3 bore, the 1200 mm3 block nothing"
    );
}
