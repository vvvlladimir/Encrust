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

/// A 20 mm cube hollowed to a 2 mm wall, sealed.
fn sealed_cube() -> Mesh {
    hollowed(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), false).0
}

fn search(mesh: &Mesh, wall: Option<(Scalar, Scalar)>, plate_z: Option<Scalar>) -> DrainSearch<'_> {
    DrainSearch {
        mesh,
        cuts: None,
        size: three_mm(),
        wall,
        vents: true,
        plate_z,
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
    let mut cubes = cuboid(Vec3::ZERO, Vec3::splat(16.0));
    cubes.append(&cuboid(
        Vec3::new(24.0, 0.0, 0.0),
        Vec3::new(40.0, 16.0, 16.0),
    ));
    let (mesh, wall) = hollowed(&cubes, false);
    let drained = suggest_drains(&DrainSearch {
        vents: false,
        ..search(&mesh, Some(wall), Some(0.0))
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
    let mesh = sealed_cube();
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
    let mesh = sealed_cube();
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
    let mesh = sealed_cube();
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
    let mesh = sealed_cube();
    let error = suggest_drains(&DrainSearch {
        layer_height_mm: 0.0,
        ..search(&mesh, None, None)
    })
    .unwrap_err();
    assert!(matches!(error, EngineError::Plan(_)), "got {error:?}");
}

/// A pocket filling the box from `low` to `high`, found at the middle of its floor.
fn pocket(low: Vec3, high: Vec3) -> Trapped {
    Trapped {
        at: low.midpoint(high).with_z(low.z),
        volume_mm3: (high - low).element_product(),
        bounds: core_geometry::Aabb::new(low, high),
    }
}

fn hole_at(x: Scalar) -> DrainHole {
    DrainHole::sized(Vec3::new(x, 0.0, 0.0), Vec3::X, three_mm(), 0.0)
}

#[test]
fn a_pocket_another_pockets_failed_try_opened_waits_for_a_hole_of_its_own() {
    let pockets = [
        pocket(Vec3::splat(2.0), Vec3::splat(8.0)),
        pocket(Vec3::new(12.0, 2.0, 2.0), Vec3::new(18.0, 8.0, 8.0)),
    ];
    let mut chosen = Vec::new();
    // Round one: the first pocket's try reaches nothing, the second's goes through the first
    // and stops short of its own, so the scan finds only the second.
    let tries = vec![(0, hole_at(1.0)), (1, hole_at(11.0))];
    settle(&pockets, &mut chosen, tries, &[pockets[1]]);
    assert_eq!(
        chosen,
        vec![(0, hole_at(1.0))],
        "the first looks emptied by its own try"
    );

    // Round two: the second's next try empties it, and with the try through the first gone,
    // the scan finds the first again.
    let tries = vec![(1, hole_at(19.0))];
    settle(&pockets, &mut chosen, tries, &[pockets[0]]);
    assert_eq!(
        chosen,
        vec![(1, hole_at(19.0))],
        "the first's hole is dropped and the first waits for another"
    );
}

#[test]
fn a_pocket_left_partly_trapped_is_not_drained() {
    let whole = pocket(Vec3::splat(2.0), Vec3::splat(8.0));
    // What stays is the pocket's top, whose box does not hold the floor it was found at.
    let top = pocket(Vec3::new(2.0, 2.0, 6.0), Vec3::splat(8.0));
    let drained = Drained {
        pockets: vec![whole],
        trapped: vec![top],
        ..Drained::default()
    };
    assert!(!drained.is_drained(0), "resin is left in its top");
}
