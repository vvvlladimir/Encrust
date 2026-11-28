//! The hollow flags as the numbers a `Hollow` edit carries, and its report as the command line
//! says it (ADR 0458).

use std::time::Duration;

use core_engine::project::HollowState;
use core_geometry::{Scalar, Transform};

use super::{HollowArgs, HollowReport};

impl HollowArgs {
    /// The numbers `--hollow` and its flags ask for, cut on the lattice `precision` picks;
    /// `None` without `--hollow`.
    pub(crate) fn state(&self, precision: Scalar) -> Option<HollowState> {
        let thickness_mm = self.thickness_mm?;
        let infill = self.infill.map(|pattern| self.infill_settings(pattern));
        Some(HollowState {
            thickness_mm,
            mode: self.mode.into(),
            precision,
            infill_on: infill.is_some(),
            infill: infill.unwrap_or_default(),
            open_floor: self.open_floor,
            solid_under_mm: self.keep_solid.under_mm,
        })
    }
}

impl HollowReport {
    /// What a `Hollow` edit of `state` did to a model standing under `transform`: its faces
    /// before and after, the cavity in mm³, the lattice in mm, and the time it took.
    pub(crate) fn of(
        (state, transform): (&HollowState, Transform),
        (faces_before, faces_after): (usize, usize),
        (cavity_mm3, voxel_mm, coarsened): (Scalar, Scalar, bool),
        took: Duration,
    ) -> Self {
        Self {
            settings: core_document::settings_of(state, transform),
            faces_before,
            faces_after,
            cavity_mm3,
            voxel_mm,
            coarsened,
            took,
        }
    }
}
