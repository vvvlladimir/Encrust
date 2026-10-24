use core_geometry::Scalar;
use printer_profiles::{IslandSpacing, SupportProfile};

/// Cell size of the grid layers are read on, mm: four times finer than the thinnest head and
/// coarse enough that a large layer is a few hundred rows (ADR 0032).
pub const GRID_PITCH_MM: Scalar = 0.1;

/// How far up the stack an overhang is measured over, mm: 0.05 mm of rise hides a 45 degree
/// lean under one cell, a millimetre turns it into ten.
pub const REFERENCE_RISE_MM: Scalar = 1.0;

/// An island narrower than this is a speck: it is held where it began only once what
/// grows out of it is more than dust, see [`MICRO_ISLAND_MM3`].
pub const MIN_ISLAND_MM: Scalar = 0.4;

/// Dust until grown to this much resin, mm3, and this tall, mm; one that never does prints
/// loose. See `docs/design/supports.md`.
pub const MICRO_ISLAND_MM3: Scalar = 0.01;
pub const MICRO_ISLAND_HEIGHT_MM: Scalar = 0.08;

/// How many bands an overhang is cut into by how flat it is, from the one just past the
/// angle to the one nearly level; see `docs/design/supports.md`.
pub const LEAN_BANDS: usize = 4;

/// A layer whose underside is no higher than this stands on the build plate itself,
/// millimetres.
pub const PLATE_CONTACT_MM: Scalar = 1e-3;

/// The lowest density that still means anything. Below it the spacing runs away.
const MIN_DENSITY: Scalar = 0.1;

/// The closest two contacts are ever put, millimetres: under this they are one contact
/// on any panel Encrust writes for.
const MIN_SPACING_MM: Scalar = 0.5;

/// Angles this far from vertical or from a ceiling are clamped: the tangent of a right
/// angle is not a number, and a wall needs no support however it is asked for.
const MIN_OVERHANG_DEG: Scalar = 1.0;
const MAX_OVERHANG_DEG: Scalar = 89.0;

/// What one run of automatic placement measures with, all in mm, derived from the profile:
/// head area, density and overhang angle.
#[derive(Debug, Clone, Copy, PartialEq)]
// Every one of these is a length, and a length without its unit in the name is what
// `AGENTS.md` forbids.
#[allow(clippy::struct_field_names)]
pub struct SampleConfig {
    /// How far apart supports are put under a face looking straight down; doubling the density
    /// halves the ground one covers, not the distance.
    pub spacing_mm: Scalar,
    /// How far apart they go under a face leaning just past the overhang angle.
    pub steep_spacing_mm: Scalar,
    /// How far a support already standing keeps another one away.
    pub coverage_radius_mm: Scalar,
    /// How far a surface may lean before it needs holding up: the horizontal reach that
    /// [`REFERENCE_RISE_MM`] of rise is allowed to cover.
    pub reach_mm: Scalar,
    /// Overhang narrower and shorter than this gets no column: a few cells is a step of the
    /// staircase every sloped surface is printed as.
    pub min_overhang_mm: Scalar,
    /// How far in from a layer's outline a contact is put: head radius and a cell more, so the head
    /// ends on the surface it holds.
    pub edge_inset_mm: Scalar,
    /// The overhang angle itself, radians from vertical.
    pub lean_rad: Scalar,
    /// How closely an island is held against `spacing_mm`, as the profile says.
    pub islands: IslandSpacing,
}

impl SampleConfig {
    /// The measurements a profile asks for; spacing is the profile's, not the head's (ADR 0131).
    /// The constants 2.9 and 1.3 fit measured carry: a 0.4 mm head holds ~1.65 mm, 0.5 mm ~1.85 mm.
    pub fn for_profile(profile: &SupportProfile) -> Self {
        let head_radius_mm = profile.contact_radius_mm();
        let head_area = std::f32::consts::PI * head_radius_mm * head_radius_mm;
        let carry_mm = head_area.mul_add(2.9, 1.3);

        // Density counts supports over an area, not along a line, so it divides the
        // spacing by its own root. See `docs/decisions/0081`.
        let density = profile.density.max(MIN_DENSITY);
        let spacing_mm = profile.contact_spacing_mm.max(MIN_SPACING_MM) / density.sqrt();
        let steep_spacing_mm = profile
            .steep_contact_spacing_mm
            .max(profile.contact_spacing_mm)
            .max(MIN_SPACING_MM)
            / density.sqrt();
        let lean = profile
            .max_overhang_deg
            .clamp(MIN_OVERHANG_DEG, MAX_OVERHANG_DEG)
            .to_radians();

        Self {
            spacing_mm,
            steep_spacing_mm,
            lean_rad: lean,
            // Three quarters of the spacing: a 3D radius that suppresses the same lattice point
            // on the next layer and is how far up a support holds (ADR 0079).
            coverage_radius_mm: spacing_mm * 0.75,
            reach_mm: REFERENCE_RISE_MM * lean.tan(),
            // Half of what one head carries: a ledge that small is inside the head that
            // would be put under it.
            min_overhang_mm: carry_mm / 2.0,
            edge_inset_mm: head_radius_mm + GRID_PITCH_MM,
            islands: profile.islands,
        }
    }

    /// How far apart supports go on an island of `area_mm2`.
    pub fn island_spacing_mm(&self, area_mm2: Scalar) -> Scalar {
        let reached = self
            .islands
            .area_mm2
            .iter()
            .take_while(|least_mm2| area_mm2 >= **least_mm2)
            .count();
        let share = self.islands.share[reached];
        (self.spacing_mm * share).max(MIN_SPACING_MM)
    }

    /// Whether the spacing follows the angle of the face at all.
    pub fn spacing_leans(&self) -> bool {
        self.steep_spacing_mm > self.spacing_mm
    }

    /// The lean from vertical, radians, past which an overhang is in band `band` or flatter: its
    /// normal is `(LEAN_BANDS - band) / LEAN_BANDS` of the way from straight down to the angle.
    pub fn band_lean_rad(&self, band: usize) -> Scalar {
        let past = std::f32::consts::FRAC_PI_2 - self.lean_rad;
        std::f32::consts::FRAC_PI_2 - past * (LEAN_BANDS - band) as Scalar / LEAN_BANDS as Scalar
    }

    /// How far apart supports go in band `band` of an overhang: the spacing its flattest
    /// face asks for, which is linear in the angle between that face's normal and down.
    pub fn band_spacing_mm(&self, band: usize) -> Scalar {
        let band = band.min(LEAN_BANDS - 1);
        let share = (LEAN_BANDS - 1 - band) as Scalar / LEAN_BANDS as Scalar;
        share.mul_add(self.steep_spacing_mm - self.spacing_mm, self.spacing_mm)
    }

    /// How far a support put down at `spacing_mm` keeps another away: a quarter short of the
    /// spacing; under a steep face, where layers add slivers, it is what spaces them.
    pub fn coverage_at(&self, spacing_mm: Scalar) -> Scalar {
        (spacing_mm - (self.spacing_mm - self.coverage_radius_mm)).max(GRID_PITCH_MM)
    }
}

#[cfg(test)]
mod tests;
