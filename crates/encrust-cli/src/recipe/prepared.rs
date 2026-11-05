//! What `--prepare` made of one model, as its report has said since ADR 0402.

use std::fmt;

use core_engine::SupportPlan;
use core_geometry::Scalar;
use serde::Serialize;

/// What `--prepare` made of one model, as the report states it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Prepared {
    /// Whether the model was repaired: it had defects, or `--repair` asked.
    pub repaired: bool,
    /// Whether it was turned by a search: no `--rotate` held it, or `--orient` asked.
    pub oriented: bool,
    pub hollowed: Option<Wall>,
    /// Whether the proven search drilled the cavity.
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

/// The name of the profile `plan` builds to: the automatic one, or the one alone.
pub(super) fn profile_name(plan: &SupportPlan) -> String {
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
