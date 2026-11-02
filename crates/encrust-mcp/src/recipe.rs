//! What a job asks of one model, written down so every revision can be cut again by the
//! command line: each field is one or more `encrust` flags (`docs/cli.md`).

use std::path::PathBuf;

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

/// The preparation of one model for one printer and resin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub model: PathBuf,
    /// A printer id of the catalogue, or the path of a printer TOML.
    pub printer: String,
    /// A resin id of the user's, or the path of a resin TOML.
    pub resin: String,
    /// `--repair` with its mode, `full` for the whole repair; absent for none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose: Option<Pose>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hollow: Option<Hollow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drains: Option<Drains>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports: Option<Supports>,
}

/// How the model stands: the orientation search, or a turn given outright.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "by")]
pub enum Pose {
    /// The engine's search, as `--orient`.
    Search {
        /// `fewest-supports`, `least-peel`, `shortest-print` or `auto`.
        goal: String,
        /// Keeps the turn within this many degrees of the way the file stands.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_tilt_deg: Option<f32>,
    },
    /// Degrees around X, Y and Z, as `--rotate`.
    Rotate { degrees: [f32; 3] },
}

/// A cavity grown inside the model, as `--hollow` and its flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hollow {
    pub wall_mm: f32,
    /// Opens the shell through the floor it stands on, so it drains with no hole.
    #[serde(default)]
    pub open_floor: bool,
    /// `hive`, `grid`, or another pattern `--infill` names; absent for an empty cavity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infill_cell_mm: Option<f32>,
    /// From 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infill_density: Option<f32>,
}

/// Holes that let the resin out of a cavity, as `--drain` and `--drain-at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Drains {
    pub diameter_mm: f32,
    /// Plate millimetres, from the plate's front left corner: the surface nearest each is drilled.
    pub at_mm: Vec<[f32; 3]>,
    /// Prints a plug for each hole beside the model.
    #[serde(default)]
    pub plugs: bool,
}

/// What the model stands on, as `--supports`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Supports {
    /// `auto`, or a support profile `list_profiles` names (`light`, `medium`, `heavy`, …).
    pub profile: String,
    /// Holds up the cavity's ceiling as well as the outside.
    #[serde(default)]
    pub cavity: bool,
}

impl Recipe {
    /// The flags after the subcommand that stage the model as this recipe says.
    pub fn flags(&self) -> Vec<String> {
        let mut flags = vec![
            self.model.display().to_string(),
            profile_flag(&self.printer, "--printer", "--profile").into(),
            self.printer.clone(),
            profile_flag(&self.resin, "--resin", "--material").into(),
            self.resin.clone(),
            "--center".into(),
            "--check-drainage".into(),
        ];
        match self.repair.as_deref() {
            None => {}
            Some("full") => flags.push("--repair".into()),
            Some(mode) => flags.push(format!("--repair={mode}")),
        }
        if let Some(pose) = &self.pose {
            pose.push_flags(&mut flags);
        }
        if let Some(hollow) = &self.hollow {
            hollow.push_flags(&mut flags);
        }
        if let Some(drains) = &self.drains {
            drains.push_flags(&mut flags);
        }
        if let Some(supports) = &self.supports {
            flags.extend(["--supports".into(), supports.profile.clone()]);
            if supports.cavity {
                flags.push("--support-cavity".into());
            }
        }
        flags
    }
}

impl Pose {
    fn push_flags(&self, flags: &mut Vec<String>) {
        match self {
            Self::Search { goal, max_tilt_deg } => {
                flags.extend(["--orient".into(), "--orient-goal".into(), goal.clone()]);
                if let Some(tilt) = max_tilt_deg {
                    flags.extend(["--max-tilt".into(), tilt.to_string()]);
                }
            }
            Self::Rotate { degrees } => flags.push(format!("--rotate={}", triple(*degrees))),
        }
    }
}

impl Hollow {
    fn push_flags(&self, flags: &mut Vec<String>) {
        flags.extend(["--hollow".into(), self.wall_mm.to_string()]);
        if self.open_floor {
            flags.push("--hollow-open-floor".into());
        }
        if let Some(infill) = &self.infill {
            flags.extend(["--infill".into(), infill.clone()]);
            if let Some(cell) = self.infill_cell_mm {
                flags.extend(["--infill-cell".into(), cell.to_string()]);
            }
            if let Some(density) = self.infill_density {
                flags.extend(["--infill-density".into(), density.to_string()]);
            }
        }
    }
}

impl Drains {
    fn push_flags(&self, flags: &mut Vec<String>) {
        if self.at_mm.is_empty() {
            return;
        }
        flags.extend(["--drain".into(), self.diameter_mm.to_string()]);
        for at in &self.at_mm {
            flags.push(format!("--drain-at={}", triple(*at)));
        }
        if self.plugs {
            flags.push("--drain-plugs".into());
        }
    }
}

/// A catalogue id takes its own flag, a TOML file of the user's the path flag.
fn profile_flag(named: &str, by_id: &'static str, by_path: &'static str) -> &'static str {
    let is_toml = std::path::Path::new(named)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"));
    if is_toml { by_path } else { by_id }
}

fn triple([x, y, z]: [f32; 3]) -> String {
    format!("{x},{y},{z}")
}

#[cfg(test)]
mod tests;
