//! `--prepare`: each model's flags filled in where the user left them out, as the autopilot
//! of `core_engine` decides (ADR 0402), so the run stages as if they had been typed.

use std::fmt;
use std::path::PathBuf;

use anyhow::Result;
use core_engine::SupportPlan;
use core_geometry::Scalar;
use serde::Serialize;

use super::{Found, Shaping};
use crate::hollowing::{DrainAt, HollowArgs};
use crate::profiles::{AUTO, plan_of};

/// What `--prepare` made of one model, as the report states it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Prepared {
    /// Whether the model was repaired: it had defects, or `--repair` asked.
    pub repaired: bool,
    /// Whether it was turned by a search: no `--rotate` held it, or `--orient` asked.
    pub oriented: bool,
    pub hollowed: Option<Wall>,
    /// Whether `--drain-at auto` drilled the cavity.
    pub drained: bool,
    /// The profile the supports are built to, by name.
    pub supports: Option<String>,
}

/// The wall a prepared model was hollowed at, and what ADR 0393 reckons it saves, which is
/// left unsaid for a wall `--hollow` gave.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Wall {
    pub wall_mm: Scalar,
    pub saves_mm3: Option<Scalar>,
}

/// Supports by the catalogue's automatic profile for each prepared model none were asked for,
/// before import, where the orientation search is judged on them.
pub(super) fn supported(entries: &mut [(PathBuf, Shaping)]) -> Result<()> {
    for (_, shaping) in entries {
        if shaping.import.prepare && shaping.supports.is_none() {
            shaping.supports = Some(plan_of(AUTO)?);
        }
    }
    Ok(())
}

/// Each prepared model hollowed where the wall saves resin, drained and its cavity held up
/// once hollowed, by flags no one typed; `None` for a model not prepared.
pub(super) fn shaped(
    entries: &mut [(PathBuf, Shaping)],
    found: &[Found],
    talk: bool,
) -> Vec<Option<Prepared>> {
    // The imports are all reported before, so on a plate these lines name their model.
    let named = entries.len() > 1;
    entries
        .iter_mut()
        .zip(found)
        .map(|((path, shaping), found)| {
            let prepared = shaping.import.prepare.then(|| hollowed(shaping, found))?;
            if talk && named {
                println!("{}", path.display());
            }
            if talk {
                print!("{prepared}");
            }
            Some(prepared)
        })
        .collect()
}

/// Steps three to five of the autopilot on one imported model.
fn hollowed(shaping: &mut Shaping, (import, oriented): &Found) -> Prepared {
    let hollow = &mut shaping.hollow;
    let wall = match (hollow.thickness_mm, import.hollowing_saves) {
        (Some(wall_mm), _) => Some(Wall {
            wall_mm,
            saves_mm3: None,
        }),
        (None, Some(found)) => {
            hollow.thickness_mm = Some(found.wall_mm);
            Some(Wall {
                wall_mm: found.wall_mm,
                saves_mm3: Some(found.saves_mm3),
            })
        }
        (None, None) => None,
    };
    if wall.is_some() && !hollow.cutting() {
        hollow.drain_at.push(DrainAt::Auto);
    }
    set_aside_idle(hollow);
    if let Some(plan) = shaping.supports.as_mut() {
        plan.cavity = wall.is_some();
    }
    Prepared {
        repaired: import.mended.is_some(),
        oriented: oriented.is_some(),
        hollowed: wall,
        drained: hollow.auto(),
        supports: shaping.supports.as_ref().map(profile_name),
    }
}

/// Drops the flags that shape a step the autopilot left out, which a run without `--prepare`
/// would refuse as shaping nothing.
fn set_aside_idle(hollow: &mut HollowArgs) {
    if !hollow.wanted() {
        (hollow.infill, hollow.cell_mm, hollow.density) = (None, None, None);
        hollow.open_floor = false;
    }
    hollow.no_vent &= hollow.auto();
    hollow.plugs &= !hollow.drain_at.is_empty();
}

/// The name of the profile `plan` builds to: the automatic one, or the one alone.
fn profile_name(plan: &SupportPlan) -> String {
    plan.sizing()
        .map_or_else(|| plan.placing().name, |auto| auto.name.clone())
}

impl fmt::Display for Prepared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = |done: bool| if done { "yes" } else { "no" };
        writeln!(f, "Prepared")?;
        writeln!(f, "  repaired       {}", said(self.repaired))?;
        writeln!(f, "  oriented       {}", said(self.oriented))?;
        match self.hollowed {
            Some(Wall {
                wall_mm,
                saves_mm3: Some(saves_mm3),
            }) => writeln!(
                f,
                "  hollowed       {wall_mm:.1} mm wall, about {:.1} ml saved",
                saves_mm3 / 1000.0
            )?,
            Some(Wall { wall_mm, .. }) => writeln!(f, "  hollowed       {wall_mm:.1} mm wall")?,
            None => writeln!(f, "  hollowed       no")?,
        }
        writeln!(f, "  drained        {}", said(self.drained))?;
        let supports = self.supports.as_deref().unwrap_or("none");
        writeln!(f, "  supports       {supports}\n")
    }
}
