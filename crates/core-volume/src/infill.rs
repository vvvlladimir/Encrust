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

/// A crossing hole's diameter, first row and row spacing as shares of the cell; why these,
/// see "Crossing holes" in `docs/design/hollowing.md`.
const HOLE_SHARE: Scalar = 0.4;
const FIRST_ROW_SHARE: Scalar = 0.6;
const ROW_SPACING_SHARE: Scalar = 1.25;

/// What fills the cavity: the pattern, how coarse it is and how much of the cavity it takes.
/// Walls, struts and crossing holes follow from the two numbers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "StoredInfill")]
pub struct InfillSettings {
    pub pattern: InfillPattern,
    /// Across one cell, in millimetres: a hexagon's corners for a hive, a square's side for
    /// a grid, the cube's edge for a scaffold.
    pub cell_mm: Scalar,
    /// How much of the cavity the lattice fills, from 0 to 1.
    pub density: Scalar,
}

impl Default for InfillSettings {
    fn default() -> Self {
        Self {
            pattern: InfillPattern::default(),
            cell_mm: 8.0,
            density: 0.25,
        }
    }
}

impl InfillSettings {
    /// How thick the lattice stands, in millimetres: the wall between two cells of a hive or
    /// a grid, or the side of a scaffold's strut.
    pub fn wall_mm(&self) -> Scalar {
        match self.pattern {
            InfillPattern::Scaffold => self.cell_mm * scaffold::strut_share(self.density),
            _ => self.opening_mm() * wall_share(self.density),
        }
    }

    /// Across a hive or grid cell's opening between two of its walls, in millimetres.
    fn opening_mm(&self) -> Scalar {
        match self.pattern {
            InfillPattern::Hive => self.cell_mm * (3.0 as Scalar).sqrt() / 2.0,
            _ => self.cell_mm,
        }
    }

    fn hole_mm(&self) -> Scalar {
        self.cell_mm * HOLE_SHARE
    }

    fn validate(&self) -> Result<(), VolumeError> {
        if !self.cell_mm.is_finite() || self.cell_mm <= 0.0 {
            return Err(VolumeError::BadCell(self.cell_mm));
        }
        if !self.density.is_finite() || self.density <= 0.0 || self.density >= 1.0 {
            return Err(VolumeError::BadDensity(self.density));
        }
        Ok(())
    }
}

/// The wall as a share of the opening that leaves `density` of a plane of cells solid, see
/// "The wall a density makes" in `docs/design/hollowing.md`.
fn wall_share(density: Scalar) -> Scalar {
    1.0 / (1.0 - density).sqrt() - 1.0
}

/// Infill as a project stores it. One saved with a wall of its own, before ADR 0320, reads
/// as the density that wall made.
#[derive(Deserialize)]
#[serde(default)]
struct StoredInfill {
    pattern: InfillPattern,
    #[serde(alias = "size_mm")]
    cell_mm: Scalar,
    density: Scalar,
    wall_mm: Option<Scalar>,
}

impl Default for StoredInfill {
    fn default() -> Self {
        let settings = InfillSettings::default();
        Self {
            pattern: settings.pattern,
            cell_mm: settings.cell_mm,
            density: settings.density,
            wall_mm: None,
        }
    }
}

impl From<StoredInfill> for InfillSettings {
    fn from(stored: StoredInfill) -> Self {
        let mut settings = Self {
            pattern: stored.pattern,
            cell_mm: stored.cell_mm,
            density: stored.density,
        };
        let wall_mm = stored.wall_mm.filter(|mm| mm.is_finite() && *mm > 0.0);
        if let Some(wall_mm) = wall_mm.filter(|_| stored.pattern.has_walls()) {
            let opening = settings.opening_mm();
            settings.density = 1.0 - (opening / (opening + wall_mm)).powi(2);
        }
        settings
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
            first: floor_mm + settings.cell_mm * FIRST_ROW_SHARE,
            spacing: settings.cell_mm * ROW_SPACING_SHARE,
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
    let reach = Vec3::splat(settings.cell_mm + settings.wall_mm());
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
mod tests;
