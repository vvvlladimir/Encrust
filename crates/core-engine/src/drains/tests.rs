use super::*;
use core_geometry::{ball, cuboid};
use core_volume::{Cancel, HollowSettings, VolumeError, hollow};

const LAYER_MM: Scalar = 0.5;
const WALL_MM: Scalar = 2.0;

fn three_mm() -> HoleSize {
    HoleSize::round(3.0, 1.0, 1.0)
}

/// `mesh` hollowed to a 2 mm wall, the floor opened when `open_floor` asks.
fn hollowed(mesh: &Mesh, open_floor: bool) -> (Mesh, (Scalar, Scalar)) {
    let asked = HollowSettings {
        open_toward: open_floor.then_some(Vec3::NEG_Z),
        ..HollowSettings::for_wall(WALL_MM)
    };
    let shell =
        hollow(mesh, &Bvh::build(mesh), &asked, Cancel::never()).expect("a closed body hollows");
    (shell.mesh, (WALL_MM, shell.voxel_mm))
}

/// `outer` with each of `cavities` sealed inside it, wound inward as a hollowed model carries them.
fn sealed(outer: (Vec3, Vec3), cavities: &[(Vec3, Vec3)]) -> Mesh {
    let mut mesh = cuboid(outer.0, outer.1);
    for (min, max) in cavities {
        let cavity = cuboid(*min, *max);
        mesh.append(&Mesh::new(
            cavity.vertices,
            cavity.faces.iter().map(|[a, b, c]| [*a, *c, *b]).collect(),
        ));
    }
    mesh
}

fn search(mesh: &Mesh, wall: Option<(Scalar, Scalar)>, plate_z: Option<Scalar>) -> DrainSearch<'_> {
    DrainSearch {
        mesh,
        cuts: None,
        size: three_mm(),
        wall,
        vents: true,
        plate_z,
        keep_clear: &[],
        layer_height_mm: LAYER_MM,
    }
}

fn of_role(drained: &Drained, role: HoleRole) -> Vec<DrainHole> {
    drained
        .holes
        .iter()
        .filter(|suggested| suggested.role == role)
        .map(|suggested| suggested.hole)
        .collect()
}

#[test]
fn a_hollow_cube_on_the_plate_drains_low_on_a_side_and_vents_through_its_top() {
    let (shell, wall) = hollowed(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), false);
    let drained = suggest_drains(&search(&shell, Some(wall), Some(0.0))).expect("the shell slices");

    assert_eq!(drained.pockets.len(), 1, "one cavity, one pocket");
    assert!(
        drained.trapped.is_empty(),
        "the holes empty it, left {:?}",
        drained.trapped
    );
    assert!(drained.is_drained(0));
    let (drains, vents) = (
        of_role(&drained, HoleRole::Drain),
        of_role(&drained, HoleRole::Vent),
    );
    assert_eq!((drains.len(), vents.len()), (1, 1), "a drain and a vent");
    let low = drains[0].at.z - three_mm().diameter_mm / 2.0;
    assert!(
        low - WALL_MM <= WALL_MM,
        "the drain's lowest point is within a wall of the 2 mm cavity floor, got {low} mm"
    );
    assert!(
        vents[0].at.z > 20.0 - WALL_MM,
        "the vent opens in the top 2 mm, got {}",
        vents[0].at
    );
}

#[test]
fn a_hollow_cube_on_supports_drains_through_its_bottom() {
    let (shell, wall) = hollowed(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), false);
    let drained = suggest_drains(&search(&shell, Some(wall), None)).expect("the shell slices");
    let drains = of_role(&drained, HoleRole::Drain);
    assert_eq!(drains.len(), 1);
    assert!(
        drains[0].at.z.abs() < 1e-3,
        "a lifted model drains through its bottom face at 0 mm, got {}",
        drains[0].at
    );
    assert!(drained.trapped.is_empty());
}

#[test]
fn a_hollow_ball_drains_below_its_middle_and_vents_above_it() {
    let centre = Vec3::new(0.0, 0.0, 15.0);
    let (shell, wall) = hollowed(&ball(centre, 15.0, 32, 48), false);
    let drained = suggest_drains(&search(&shell, Some(wall), Some(0.0))).expect("the shell slices");
    assert!(drained.trapped.is_empty(), "left {:?}", drained.trapped);
    let (drains, vents) = (
        of_role(&drained, HoleRole::Drain),
        of_role(&drained, HoleRole::Vent),
    );
    assert!(
        drains[0].at.z < 15.0 / 2.0,
        "a drain in the lower half of a 15 mm ball centred 15 mm up, got {}",
        drains[0].at
    );
    assert!(
        vents[0].at.z > 15.0 + 15.0 / 2.0,
        "a vent in the upper quarter, over 22.5 mm, got {}",
        vents[0].at
    );
}

#[test]
fn two_cavities_get_a_drain_each() {
    let mesh = sealed(
        (Vec3::ZERO, Vec3::new(40.0, 20.0, 20.0)),
        &[
            (Vec3::splat(4.0), Vec3::new(16.0, 16.0, 16.0)),
            (Vec3::new(24.0, 4.0, 4.0), Vec3::new(36.0, 16.0, 16.0)),
        ],
    );
    let drained = suggest_drains(&DrainSearch {
        vents: false,
        ..search(&mesh, None, Some(0.0))
    })
    .expect("the box slices");
    assert_eq!(drained.pockets.len(), 2);
    let mut served: Vec<usize> = drained.holes.iter().map(|hole| hole.pocket).collect();
    served.sort_unstable();
    assert_eq!(served, vec![0, 1], "one drain for each pocket");
    assert!(drained.trapped.is_empty(), "left {:?}", drained.trapped);
}

#[test]
fn an_open_floor_needs_no_hole() {
    let (shell, wall) = hollowed(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), true);
    let drained = suggest_drains(&search(&shell, Some(wall), Some(0.0))).expect("the shell slices");
    assert!(drained.pockets.is_empty() && drained.holes.is_empty());
    assert_eq!(drained.scans, 1, "one scan finds nothing to drain");
}

#[test]
fn a_pocket_no_hole_fits_stays_trapped() {
    let mesh = sealed(
        (Vec3::ZERO, Vec3::splat(20.0)),
        &[(Vec3::splat(5.0), Vec3::splat(15.0))],
    );
    let drained = suggest_drains(&DrainSearch {
        size: HoleSize::round(24.0, 1.0, 1.0),
        ..search(&mesh, None, None)
    })
    .expect("the box slices");
    assert!(drained.holes.is_empty(), "a 24 mm hole fits no 20 mm face");
    assert_eq!(drained.trapped.len(), 1, "the pocket stays trapped");
    assert!(!drained.is_drained(0));
}

#[test]
fn every_scan_is_counted_and_bounded() {
    let mesh = sealed(
        (Vec3::ZERO, Vec3::splat(20.0)),
        &[(Vec3::splat(5.0), Vec3::splat(15.0))],
    );
    let drained = suggest_drains(&search(&mesh, None, Some(0.0))).expect("the box slices");
    assert!(
        drained.scans <= 2 + 2 * MOST_TRIES,
        "at most {} scans, ran {}",
        2 + 2 * MOST_TRIES,
        drained.scans
    );
}

#[test]
fn a_hole_of_no_width_cannot_be_drilled() {
    let mesh = sealed(
        (Vec3::ZERO, Vec3::splat(20.0)),
        &[(Vec3::splat(5.0), Vec3::splat(15.0))],
    );
    let error = suggest_drains(&DrainSearch {
        size: HoleSize::round(0.0, 1.0, 1.0),
        ..search(&mesh, None, None)
    })
    .unwrap_err();
    assert!(
        matches!(error, EngineError::Drill(VolumeError::BadDrain { .. })),
        "got {error:?}"
    );
}

#[test]
fn a_layer_of_no_height_cannot_be_cut() {
    let mesh = sealed(
        (Vec3::ZERO, Vec3::splat(20.0)),
        &[(Vec3::splat(5.0), Vec3::splat(15.0))],
    );
    let error = suggest_drains(&DrainSearch {
        layer_height_mm: 0.0,
        ..search(&mesh, None, None)
    })
    .unwrap_err();
    assert!(matches!(error, EngineError::Plan(_)), "got {error:?}");
}
