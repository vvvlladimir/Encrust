//! `--drain-at auto`: the holes the engine proves drain each trapped pocket (ADR 0396).

use std::fmt;

use anyhow::{Context, Result};
use core_engine::{DrainSearch, Drained, HoleRole, suggest_drains};
use core_geometry::{Mesh, Scalar};
use core_volume::{Channel, DrainHole, drill};

use super::{HollowArgs, pierced};

/// Where a model prints, as the search for its drains needs it.
#[derive(Debug, Clone, Copy)]
pub struct Ground {
    pub layer_height_mm: Scalar,
    /// Whether the model prints standing on the plate, which no hole may open onto.
    pub on_plate: bool,
}

/// The holes `--drain-at auto` drills into `shell`, which already has `placed` cut into it,
/// as a model hollowed to `wall` cuts them.
pub fn auto_drains(
    shell: &Mesh,
    placed: (&[DrainHole], &[Channel]),
    args: &HollowArgs,
    wall: Option<(Scalar, Scalar)>,
    ground: Ground,
) -> Result<Drained> {
    let (holes, channels) = placed;
    let cuts = (!holes.is_empty() || !channels.is_empty())
        .then(|| drill(&pierced(holes, wall), channels))
        .transpose()
        .context("drilling the drains")?;
    let plate_z = ground
        .on_plate
        .then(|| shell.aabb().map(|bounds| bounds.mins.z))
        .flatten();
    suggest_drains(&DrainSearch {
        mesh: shell,
        cuts: cuts.as_ref(),
        size: args.hole_size(),
        wall,
        vents: !args.no_vent,
        plate_z,
        layer_height_mm: ground.layer_height_mm,
    })
    .context("looking for drain holes")
}

/// What `--drain-at auto` chose, for a terminal.
pub struct AutoReport<'a>(pub &'a Drained);

impl fmt::Display for AutoReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let drained = self.0;
        let emptied = (0..drained.pockets.len())
            .filter(|&pocket| drained.is_drained(pocket))
            .count();
        writeln!(
            f,
            "  auto drains   {} pocket(s), {emptied} drained, {} scan(s)",
            drained.pockets.len(),
            drained.scans
        )?;
        for suggested in &drained.holes {
            let role = match suggested.role {
                HoleRole::Drain => "drain",
                HoleRole::Vent => "vent ",
            };
            let at = suggested.hole.at;
            writeln!(
                f,
                "    {role}       pocket {} at {:.2},{:.2},{:.2}",
                suggested.pocket + 1,
                at.x,
                at.y,
                at.z
            )?;
        }
        Ok(())
    }
}
