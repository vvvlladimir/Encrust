//! The numbers a front end takes where none is typed, one home for the command line and the
//! agents' server so an edit asked of either is the same.

use core_geometry::Scalar;
use core_volume::{HoleSize, HollowSettings, InfillSettings};

/// The radius a shown-side point is brushed at where none is given, mm.
pub const SHOWN_RADIUS_MM: Scalar = 4.0;

/// How fine the lattice a hollow and a relief are cut on, 0 to 1.
pub fn lattice_precision() -> Scalar {
    HollowSettings::default().precision
}

/// The infill lattice's cell, mm, and its density, 0 to 1.
pub fn infill_cell_and_density() -> (Scalar, Scalar) {
    let infill = InfillSettings::default();
    (infill.cell_mm, infill.density)
}

/// How far apart an arrange spreads the plate's models, mm.
pub fn arrange_clearance_mm() -> Scalar {
    core_engine::ArrangeSettings::default().clearance_mm
}

/// How far past the surface a drain hole reaches, mm.
pub fn drain_depth_mm() -> Scalar {
    HoleSize::default().depth_mm
}
