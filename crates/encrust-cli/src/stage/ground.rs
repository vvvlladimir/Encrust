//! Where a model's supports will stand it, worked out before its holes are placed, so a point
//! the command line gives is the point the report locates (ADR 0396).

use anyhow::{Context, Result, bail};
use core_engine::{Lift, planned_lift};
use core_geometry::{Mesh, Scalar};

use super::Shaping;
use crate::hollowing::{Ground, HollowArgs};

/// Where `shaping`'s supports will stand `mesh`, worked out only for the holes and channels
/// it places, which are all that need it before the supports stand.
pub(super) fn ground_of(
    mesh: &Mesh,
    shaping: &Shaping,
    seated: bool,
    layer_height_mm: Scalar,
) -> Result<Ground> {
    let lift = match &shaping.supports {
        Some(plan) if shaping.hollow.cutting() => planned_lift(mesh, plan, seated, layer_height_mm)
            .context("cannot work out how far the supports stand the model up")?,
        _ => Lift::ON_THE_PLATE,
    };
    Ok(Ground {
        layer_height_mm,
        on_plate: lift.resting,
        lifted_mm: lift.lifted_mm,
    })
}

/// Nearer than this the supports stood the model where its holes were placed for, millimetres.
const SAME_LIFT_MM: Scalar = 1e-3;

/// Refuses points placed for a lift other than the one the supports gave, which drilled each
/// that much off: a mould's wall grown under the model, or holes that changed what hangs.
pub(super) fn check_lift(lifted_mm: Scalar, ground: Ground, hollow: &HollowArgs) -> Result<()> {
    if !hollow.pointing() || (lifted_mm - ground.lifted_mm).abs() <= SAME_LIFT_MM {
        return Ok(());
    }
    bail!(
        "the supports stood the model {lifted_mm:.2} mm up, not the {:.2} mm planned when \
         --drain-at and --channel were placed, so each would land off its point; drill the \
         model without --supports, or place the points on a run that is not a mould",
        ground.lifted_mm
    )
}
