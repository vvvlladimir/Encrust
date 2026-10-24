use std::collections::HashSet;

use core_geometry::{cuboid, diagnose, glam::IVec3, signed_volume, winding_number};

use super::*;
use crate::csg::assemble;
use crate::grid::{VoxelGrid, tile_of};

/// A cavity of `radius` centred on the origin, on a 0.5 mm lattice.
pub(super) fn ball_cavity(radius: Scalar) -> Sdf {
    let grid = VoxelGrid::new(0.5);
    let reach = (radius / 0.5).ceil() as i32 + 8;
    let first = tile_of(IVec3::splat(-reach));
    let last = tile_of(IVec3::splat(reach));
    let mut candidates = HashSet::new();
    for z in first.z..=last.z {
        for y in first.y..=last.y {
            for x in first.x..=last.x {
                candidates.insert(IVec3::new(x, y, z));
            }
        }
    }
    assemble(grid, 1.5, candidates, |voxel| {
        grid.position(voxel).length() - radius
    })
}

/// The share of a box of points, `step_mm` apart, that `lattice` holds as solid.
pub(super) fn solid_share(lattice: &dyn Lattice, low: Vec3, high: Vec3, step_mm: Scalar) -> Scalar {
    let count = ((high - low) / step_mm).ceil().as_ivec3();
    let (mut solid, mut total) = (0usize, 0usize);
    for z in 0..count.z {
        for y in 0..count.y {
            for x in 0..count.x {
                let at = low + (IVec3::new(x, y, z).as_vec3() + 0.5) * step_mm;
                solid += usize::from(lattice.distance(at) < 0.0);
                total += 1;
            }
        }
    }
    solid as Scalar / total as Scalar
}

/// The share of a 40 mm cube of cavity `pattern` takes, as the exact cut makes it.
pub(super) fn carved_share(pattern: InfillPattern) -> Scalar {
    let cube = cuboid(Vec3::splat(-20.0), Vec3::splat(20.0));
    let settings = InfillSettings {
        pattern,
        ..InfillSettings::default()
    };
    let field = box_cavity(20.0);
    let carved = cut(&field, cube.clone(), &settings, 0, Cancel::never()).expect("valid");
    for (part, what) in [(&carved.void, "void"), (&carved.solid, "solid")] {
        let report = diagnose(part);
        assert_eq!(
            report.boundary_edges,
            0,
            "{} cuts a closed {what}",
            pattern.label()
        );
    }
    let left = signed_volume(&carved.void) - signed_volume(&carved.solid);
    1.0 - left / signed_volume(&cube)
}

/// A cubic cavity reaching `half` either side of the origin, on a 0.5 mm lattice.
fn box_cavity(half: Scalar) -> Sdf {
    let grid = VoxelGrid::new(0.5);
    let reach = (half / 0.5).ceil() as i32 + 8;
    let (first, last) = (tile_of(IVec3::splat(-reach)), tile_of(IVec3::splat(reach)));
    let candidates = (first.z..=last.z)
        .flat_map(|z| {
            (first.y..=last.y)
                .flat_map(move |y| (first.x..=last.x).map(move |x| IVec3::new(x, y, z)))
        })
        .collect();
    assemble(grid, 1.5, candidates, |voxel| {
        (grid.position(voxel).abs() - Vec3::splat(half)).max_element()
    })
}

fn walled(pattern: InfillPattern) -> InfillSettings {
    InfillSettings {
        pattern,
        ..InfillSettings::default()
    }
}

#[test]
fn the_exact_cut_and_the_field_agree_on_how_much_a_lattice_takes() {
    for pattern in InfillPattern::ALL {
        let exact = carved_share(pattern);
        let lattice = lattice_of(&walled(pattern), -20.0);
        let sampled = solid_share(
            lattice.as_ref(),
            Vec3::splat(-20.0),
            Vec3::splat(20.0),
            0.25,
        );
        assert!(
            (exact - sampled).abs() < 0.02,
            "{} takes {sampled} of the cube by its distance and {exact} by its meshes",
            pattern.label()
        );
    }
}

#[test]
fn a_crossing_hole_opens_its_wall_right_through() {
    // In a 40 mm cube a row of holes stands at z = -20 + 0.6 c + k 1.25 c.
    for pattern in [InfillPattern::Grid, InfillPattern::Hive] {
        let settings = walled(pattern);
        let rows = Rows::new(&settings, -20.0);
        let row = rows
            .between(-5.0, 5.0)
            .next()
            .expect("one row is near the middle");
        let pitch = settings.opening_mm() + settings.wall_mm();
        let wall = match pattern {
            InfillPattern::Grid => Vec3::new(0.0, pitch / 2.0, row),
            _ => Vec3::new(pitch / 2.0, 0.0, row),
        };
        let cube = cuboid(Vec3::splat(-20.0), Vec3::splat(20.0));
        let carved = cut(&box_cavity(20.0), cube, &settings, 0, Cancel::never()).expect("valid");
        let solid = |at: Vec3| winding_number(&carved.solid, at) > 0.5;
        let name = pattern.label();
        let between = wall.with_z(row - rows.spacing / 2.0);
        assert!(solid(between), "{name}: the wall between two rows");
        assert!(!solid(wall), "{name}: the hole through it at its row");
    }
}

#[test]
fn a_cavity_that_is_not_a_solid_is_cut_from_the_field_instead() {
    let cavity = ball_cavity(10.0);
    let mut torn = cuboid(Vec3::splat(-9.0), Vec3::splat(9.0));
    torn.faces.truncate(10);
    let carved = cut(
        &cavity,
        torn,
        &walled(InfillPattern::Grid),
        0,
        Cancel::never(),
    )
    .expect("the field always answers");
    assert_eq!(
        diagnose(&carved.void).boundary_edges,
        0,
        "the field's void is closed"
    );
    assert!(signed_volume(&carved.void) > 0.0);
}

#[test]
fn a_cell_of_no_size_is_refused() {
    let settings = InfillSettings {
        cell_mm: 0.0,
        ..InfillSettings::default()
    };
    assert_eq!(settings.validate(), Err(VolumeError::BadCell(0.0)));
}

#[test]
fn a_density_outside_nought_to_one_is_refused() {
    for pattern in InfillPattern::ALL {
        for density in [0.0, 1.0] {
            let settings = InfillSettings {
                density,
                ..walled(pattern)
            };
            assert_eq!(settings.validate(), Err(VolumeError::BadDensity(density)));
        }
    }
}

#[test]
fn rows_start_over_the_floor_and_repeat_at_their_spacing() {
    let rows = Rows {
        first: 4.0,
        spacing: 8.0,
    };
    assert_eq!(rows.off_row(4.0), 0.0);
    assert_eq!(rows.off_row(12.0), 0.0);
    assert_eq!(rows.off_row(1.0), -3.0, "nothing under the first row");
    assert_eq!(
        rows.between(0.0, 21.0).collect::<Vec<_>>(),
        vec![4.0, 12.0, 20.0]
    );
    assert_eq!(rows.between(5.0, 13.0).collect::<Vec<_>>(), vec![12.0]);
}
