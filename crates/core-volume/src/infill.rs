use core_geometry::{Aabb, Mesh, Scalar, Vec3};

use serde::{Deserialize, Serialize};

use crate::cancel::Cancel;
use crate::error::VolumeError;
use crate::extract::extract;
use crate::sdf::Sdf;

mod carve;
mod field;
mod grid;
mod hive;
mod scaffold;

use grid::GridCells;
use hive::HiveCells;
use scaffold::Scaffold;

/// How far a round hole may sag off the true circle, in millimetres: about a pixel of the
/// finest panel, inside a cavity nobody sees.
const ROUND_TOLERANCE_MM: Scalar = 0.02;

/// Which lattice stands in a cavity.
///
/// All three leave every cell joined to the next, because resin has to be able to leave;
/// see `docs/design/hollowing.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InfillPattern {
    /// Hexagonal cells standing on the plate, joined by round crossing holes.
    #[default]
    Hive,
    /// Square cells standing on the plate, joined by round crossing holes.
    Grid,
    /// Struts along the edges of a cubic lattice stood on a corner, so none is level.
    Scaffold,
}

impl InfillPattern {
    /// The patterns a picker offers, in the order it draws them.
    pub const ALL: [Self; 3] = [Self::Hive, Self::Grid, Self::Scaffold];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hive => "Hive",
            Self::Grid => "Grid",
            Self::Scaffold => "Scaffold",
        }
    }

    /// Whether the pattern is walls between cells, which crossing holes join, rather than
    /// struts.
    pub fn has_walls(self) -> bool {
        self != Self::Scaffold
    }
}

/// What fills the cavity and the size of each part of it, all lengths in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InfillSettings {
    pub pattern: InfillPattern,
    /// Across one cell: a hexagon's corners for a hive, a square's side for a grid, the
    /// cube's edge for a scaffold.
    pub cell_mm: Scalar,
    /// Thickness of the wall between two cells of a hive or a grid.
    pub wall_mm: Scalar,
    /// How much of the cavity a scaffold's struts fill, from 0 to 1.
    pub density: Scalar,
    /// Diameter of the crossing holes through a hive or grid wall.
    pub hole_mm: Scalar,
    /// Height of the first row of crossing holes over the cavity's floor.
    pub first_hole_mm: Scalar,
    /// Height from one row of crossing holes to the next.
    pub hole_spacing_mm: Scalar,
}

impl Default for InfillSettings {
    fn default() -> Self {
        Self {
            pattern: InfillPattern::default(),
            cell_mm: 6.0,
            wall_mm: 2.0,
            density: 0.3,
            hole_mm: 2.5,
            first_hole_mm: 4.0,
            hole_spacing_mm: 8.0,
        }
    }
}

impl InfillSettings {
    /// The widest a crossing hole may be, in millimetres: a hexagon's side, or a square's.
    /// A hole as wide as that would cut into the junctions the walls stand on.
    pub fn widest_hole_mm(&self) -> Scalar {
        match self.pattern {
            InfillPattern::Hive => self.cell_mm / 2.0,
            _ => self.cell_mm,
        }
    }

    fn validate(&self) -> Result<(), VolumeError> {
        let positive = |mm: Scalar| mm.is_finite() && mm > 0.0;
        if !positive(self.cell_mm) {
            return Err(VolumeError::BadCell(self.cell_mm));
        }
        if !self.pattern.has_walls() {
            if !self.density.is_finite() || self.density <= 0.0 || self.density >= 1.0 {
                return Err(VolumeError::BadDensity(self.density));
            }
            return Ok(());
        }
        if !positive(self.wall_mm) {
            return Err(VolumeError::BadInfillWall(self.wall_mm));
        }
        let fits = positive(self.hole_mm) && self.hole_mm < self.widest_hole_mm();
        let starts = self.first_hole_mm.is_finite() && self.first_hole_mm >= 0.0;
        // Rows closer than a hole would merge into a slot.
        let apart = self.hole_spacing_mm.is_finite() && self.hole_spacing_mm > self.hole_mm;
        if !(fits && starts && apart) {
            return Err(VolumeError::BadHoles {
                diameter_mm: self.hole_mm,
                first_mm: self.first_hole_mm,
                spacing_mm: self.hole_spacing_mm,
            });
        }
        Ok(())
    }
}

/// A lattice, written down two ways: as a signed distance, in millimetres and negative
/// inside its solid, and as the meshes it is cut from exactly.
trait Lattice: Sync {
    fn distance(&self, point: Vec3) -> Scalar;

    /// The lattice's shapes over `bounds`, which they reach past on every side, leaving
    /// out each one whose box `keep` refuses.
    fn shapes(&self, bounds: &Aabb, keep: &dyn Fn(&Aabb) -> bool) -> Shapes;
}

/// What a lattice cuts a cavity with.
enum Shapes {
    /// Cells, which never cross, and the holes through the walls between them in groups,
    /// no two of a group crossing: the walls are what the cavity keeps of neither.
    Openings { cells: Mesh, holes: Vec<Vec<Bar>> },
    /// Groups of struts, no two of a group crossing: they are what the cavity keeps solid.
    Struts(Vec<Mesh>),
}

/// A round bar from `from` to `to`, `radius` millimetres across its middle.
struct Bar {
    from: Vec3,
    to: Vec3,
    radius: Scalar,
}

impl Bar {
    fn aabb(&self) -> Aabb {
        let radius = Vec3::splat(self.radius);
        Aabb::new(
            self.from.min(self.to) - radius,
            self.from.max(self.to) + radius,
        )
    }
}

/// What a lattice makes of a cavity: the void, to be wound inward, and the solid standing
/// in it, wound outward. Either may be several shells that overlap; the fill rule unions
/// them.
pub(crate) struct Carved {
    pub void: Mesh,
    pub solid: Mesh,
}

/// The heights crossing holes stand at: a first row over the cavity's floor and one every
/// spacing above it, all in millimetres.
#[derive(Clone, Copy)]
struct Rows {
    first: Scalar,
    spacing: Scalar,
}

impl Rows {
    fn new(settings: &InfillSettings, floor_mm: Scalar) -> Self {
        Self {
            first: floor_mm + settings.first_hole_mm,
            spacing: settings.hole_spacing_mm,
        }
    }

    /// How far `z` is above or below the nearest row, none of them under the first.
    fn off_row(self, z: Scalar) -> Scalar {
        let row = ((z - self.first) / self.spacing).round().max(0.0);
        z - (self.first + row * self.spacing)
    }

    /// Every row from `low` to `high`.
    fn between(self, low: Scalar, high: Scalar) -> impl Iterator<Item = Scalar> {
        let first = ((low - self.first) / self.spacing).ceil().max(0.0) as usize;
        (first..)
            .map(move |row| self.first + row as Scalar * self.spacing)
            .take_while(move |z| *z <= high)
    }
}

/// What the lattice `settings` ask for makes of the cavity bounded by `cavity_mesh`.
///
/// The lattice is cut exactly from `cavity_mesh`. Where that mesh is not a solid the exact
/// cut can read, the void is cut from the field instead, which is coarser but always
/// answers; see `docs/design/hollowing.md`.
pub(crate) fn cut(
    cavity: &Sdf,
    cavity_mesh: Mesh,
    settings: &InfillSettings,
    budget_bytes: usize,
    cancel: Cancel<'_>,
) -> Result<Carved, VolumeError> {
    settings.validate()?;
    let Some(bounds) = cavity_mesh.aabb() else {
        return Ok(Carved {
            void: cavity_mesh,
            solid: Mesh::default(),
        });
    };
    let lattice = lattice_of(settings, bounds.mins.z);
    let reach = Vec3::splat(settings.cell_mm + settings.wall_mm);
    let keep = |shape: &Aabb| cavity.reaches(shape);
    if let Ok(solid) = carve::carved(&cavity_mesh, lattice.as_ref(), reach, &keep) {
        return Ok(Carved {
            void: cavity_mesh,
            solid,
        });
    }
    drop(cavity_mesh);
    let filled = field::filled(cavity, settings, budget_bytes)?;
    Ok(Carved {
        void: extract(&filled, cancel),
        solid: Mesh::default(),
    })
}

fn lattice_of(settings: &InfillSettings, floor_mm: Scalar) -> Box<dyn Lattice> {
    match settings.pattern {
        InfillPattern::Hive => Box::new(HiveCells::new(settings, Rows::new(settings, floor_mm))),
        InfillPattern::Grid => Box::new(GridCells::new(settings, Rows::new(settings, floor_mm))),
        InfillPattern::Scaffold => Box::new(Scaffold::new(settings)),
    }
}

#[cfg(test)]
mod tests {
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
    pub(super) fn solid_share(
        lattice: &dyn Lattice,
        low: Vec3,
        high: Vec3,
        step_mm: Scalar,
    ) -> Scalar {
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
        // In a 40 mm cube the rows stand at z = -16, -8, 0, 8 and 16; z = -4 is between two.
        // A 6 mm hexagon is 3 sqrt(3) mm across its flats, and the wall adds 2 mm.
        let pitch = 3.0 * (3.0 as Scalar).sqrt() + 2.0;
        for (pattern, wall) in [
            (InfillPattern::Grid, Vec3::new(0.0, 4.0, 0.0)),
            (InfillPattern::Hive, Vec3::new(pitch / 2.0, 0.0, 0.0)),
        ] {
            let cube = cuboid(Vec3::splat(-20.0), Vec3::splat(20.0));
            let carved = cut(
                &box_cavity(20.0),
                cube,
                &walled(pattern),
                0,
                Cancel::never(),
            )
            .expect("valid");
            let solid = |at: Vec3| winding_number(&carved.solid, at) > 0.5;
            let name = pattern.label();
            assert!(
                solid(wall.with_z(-4.0)),
                "{name}: the wall between two rows"
            );
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
    fn a_scaffold_density_outside_nought_to_one_is_refused() {
        for density in [0.0, 1.0] {
            let settings = InfillSettings {
                density,
                ..walled(InfillPattern::Scaffold)
            };
            assert_eq!(settings.validate(), Err(VolumeError::BadDensity(density)));
        }
    }

    #[test]
    fn a_wall_of_no_thickness_is_refused() {
        let settings = InfillSettings {
            wall_mm: -1.0,
            ..InfillSettings::default()
        };
        assert_eq!(settings.validate(), Err(VolumeError::BadInfillWall(-1.0)));
    }

    #[test]
    fn a_hole_wider_than_a_side_or_rows_that_merge_are_refused() {
        let wide = InfillSettings {
            hole_mm: 3.0,
            ..InfillSettings::default()
        };
        let merged = InfillSettings {
            hole_spacing_mm: 2.0,
            ..InfillSettings::default()
        };
        let sunk = InfillSettings {
            first_hole_mm: -1.0,
            ..InfillSettings::default()
        };
        for settings in [wide, merged, sunk] {
            assert!(
                matches!(settings.validate(), Err(VolumeError::BadHoles { .. })),
                "{settings:?} was let through"
            );
        }
        let square = InfillSettings {
            hole_mm: 3.0,
            ..walled(InfillPattern::Grid)
        };
        assert_eq!(
            square.validate(),
            Ok(()),
            "a square's side is the whole cell"
        );
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
}
