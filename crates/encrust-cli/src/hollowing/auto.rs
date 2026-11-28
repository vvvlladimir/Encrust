//! `--drain-at auto`: the holes the engine proves drain each trapped pocket (ADR 0401).

use std::fmt;

use core_engine::{Drained, HoleRole};
use core_geometry::{Scalar, Vec3};

/// What `--drain-at auto` chose, for a terminal, each mouth where it prints: lifted by the
/// millimetres beside it.
pub struct AutoReport<'a>(pub &'a Drained, pub Scalar);

impl fmt::Display for AutoReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let AutoReport(drained, lifted_mm) = *self;
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
            let at = suggested.hole.at + Vec3::Z * lifted_mm;
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
