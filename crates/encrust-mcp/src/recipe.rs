//! What a job asks of one model, written down so every revision can be cut again by the
//! command line: each field is one or more `encrust` flags (`docs/cli.md`).

use std::path::{Path, PathBuf};

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

mod finish;
mod shape;
mod slicing;

pub use finish::{Drains, Fixes, HoleKind, Hollow, Supports};
pub use shape::{Cut, Mend, Modelling, Relief};
pub use slicing::Slicing;

/// The preparation of one model for one printer and resin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub model: PathBuf,
    /// A printer id of the catalogue, or the path of a printer TOML; empty for the standard one.
    pub printer: String,
    /// A resin id of the user's, or the path of a resin TOML; empty for the stock numbers.
    pub resin: String,
    /// The engine's autopilot, `--prepare`, decides every step this recipe leaves out.
    #[serde(default)]
    pub prepare: bool,
    /// A recipe of `core-recipe` decides them instead, as `--recipe`: an id or a TOML's path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<Repair>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mend: Option<Mend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelling: Option<Modelling>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relief: Option<Relief>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose: Option<Pose>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hollow: Option<Hollow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drains: Option<Drains>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports: Option<Supports>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slicing: Option<Slicing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixes: Option<Fixes>,
}

/// The repair `--repair` runs: all of it, or one part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Repair {
    Full,
    CloseHoles,
    CloseSmallHoles,
    Turn,
    Wrap,
    Deep,
}

/// What the orientation search looks for, as `--orient-goal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Goal {
    FewestSupports,
    LeastPeel,
    ShortestPrint,
    /// Balances the other three.
    Auto,
}

/// How the model stands: the orientation search, or a turn given outright.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "by")]
pub enum Pose {
    /// The engine's search, as `--orient`.
    Search {
        goal: Goal,
        /// Keeps the turn within this many degrees of the way the file stands.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_tilt_deg: Option<f32>,
        /// Takes the Nth way the search ranks, from one, as `compare_poses` numbers them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(range(min = 1, max = 64))]
        pick: Option<u16>,
        /// Points no support should touch — a face, a lip, a detail — in plate mm as the latest
        /// revision stands; the search ranks first the sound poses that leave them clear.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        keep_clear_at_mm: Vec<[f32; 3]>,
        /// Ranks first the poses with the fewest contacts where the model shows, read off its
        /// shape, as `--clean-surface`.
        #[serde(default)]
        clean_surface: bool,
        /// How far from a point a support still touches it; 4 mm when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keep_clear_radius_mm: Option<f32>,
        /// The turn the points were seen in, lift taken off: the server's, kept with them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(skip)]
        keep_clear_seen_deg: Option<[f32; 3]>,
    },
    /// Degrees around X, Y and Z, as `--rotate`.
    Rotate { degrees: [f32; 3] },
}

impl Repair {
    /// The `--repair` flag that runs this repair.
    pub fn flag(self) -> String {
        match self {
            Self::Full => "--repair".into(),
            mode => format!("--repair={}", kebab(&mode)),
        }
    }
}

impl Recipe {
    /// A recipe of nothing but its model and profiles: the model as its file stands it.
    pub fn bare(model: PathBuf, printer: String, resin: String) -> Self {
        Self {
            model,
            printer,
            resin,
            prepare: false,
            steps_by: None,
            repair: None,
            mend: None,
            modelling: None,
            relief: None,
            pose: None,
            hollow: None,
            drains: None,
            supports: None,
            slicing: None,
            fixes: None,
        }
    }

    /// Whether the recipe cuts, splits, parts off, joins or copies its model, so it is no
    /// longer one object on the plate.
    pub fn makes_pieces(&self) -> bool {
        self.modelling.as_ref().is_some_and(|m| {
            m.cut.is_some()
                || m.split_smallest_mm3.is_some()
                || m.part_off_at_mm.is_some()
                || m.combine.is_some()
                || m.copies.is_some_and(|copies| copies > 0)
        })
    }

    /// Whether the model is a project the window saved, whose models, poses and supports
    /// stand as saved: the command line refuses every flag that would shape them.
    pub fn is_project(&self) -> bool {
        self.model
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("encrust"))
    }

    /// Whether the user named a printer and resin; a job with neither runs on the standard
    /// printer and stock resin until it is written (ADR 0475).
    pub fn names_a_machine(&self) -> bool {
        !self.printer.is_empty()
    }

    /// The printer and resin flags, none where the user named neither.
    fn machine_named(&self) -> Vec<String> {
        if !self.names_a_machine() {
            return Vec::new();
        }
        vec![
            profile_flag(&self.printer, "--printer", "--profile"),
            profile_flag(&self.resin, "--resin", "--material"),
        ]
    }

    /// The flags a check of a plate takes from this recipe: the machine and the slicing.
    pub fn machine_flags(&self) -> Vec<String> {
        let mut flags = self.machine_named();
        flags.push("--check-drainage".into());
        if let Some(slicing) = &self.slicing {
            slicing.push_flags(&mut flags);
        }
        flags
    }

    /// The flags after the subcommand that stage the model as this recipe says. Every value
    /// rides on its flag as `--flag=value`, so a negative number is never read as a flag.
    pub fn flags(&self) -> Vec<String> {
        let mut flags = vec![self.model.display().to_string()];
        let others = self.modelling.iter().flat_map(Modelling::others);
        flags.extend(others.map(|other| other.display().to_string()));
        flags.extend(self.machine_named());
        if !self.is_project() {
            flags.push("--center".into());
        }
        flags.push("--check-drainage".into());
        if let Some(recipe) = &self.steps_by {
            flags.push(format!("--recipe={recipe}"));
        } else if self.prepare {
            flags.push("--prepare".into());
        }
        if let Some(repair) = self.repair {
            flags.push(repair.flag());
        }
        if let Some(mend) = &self.mend {
            mend.push_flags(&mut flags);
        }
        if let Some(modelling) = &self.modelling {
            modelling.push_flags(&mut flags);
        }
        if let Some(relief) = &self.relief {
            relief.push_flags(&mut flags);
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
            supports.push_flags(&mut flags);
        }
        if let Some(slicing) = &self.slicing {
            slicing.push_flags(&mut flags);
        }
        if let Some(fixes) = self.fixes {
            fixes.push_flags(&mut flags);
        }
        flags
    }
}

impl Recipe {
    /// Refuses a path from the client that is not absolute and a name that would read as a
    /// flag, before anything reaches the command line.
    pub fn check_paths(&self) -> anyhow::Result<()> {
        if let Some(recipe) = &self.steps_by {
            crate::params::plain(recipe, "the recipe")?;
        }
        if let Some(supports) = &self.supports {
            crate::params::plain(&supports.profile, "the support profile")?;
        }
        let others = self.modelling.iter().flat_map(Modelling::others);
        let image = self
            .relief
            .iter()
            .filter_map(|relief| relief.image.as_ref());
        let font = (self.modelling.iter()).filter_map(|m| m.label.as_ref()?.font.as_ref());
        for path in others.chain(image).chain(font) {
            crate::params::absolute(path, "the file")?;
        }
        Ok(())
    }
}

impl Pose {
    /// The search for `goal` within `max_tilt_deg`, taking its first pose and keeping nothing
    /// clear.
    pub fn search(goal: Goal, max_tilt_deg: Option<f32>) -> Self {
        Self::Search {
            goal,
            max_tilt_deg,
            pick: None,
            clean_surface: false,
            keep_clear_at_mm: Vec::new(),
            keep_clear_radius_mm: None,
            keep_clear_seen_deg: None,
        }
    }

    fn push_flags(&self, flags: &mut Vec<String>) {
        match self {
            Self::Search {
                goal,
                max_tilt_deg,
                pick,
                clean_surface,
                keep_clear_at_mm,
                keep_clear_radius_mm,
                keep_clear_seen_deg,
            } => {
                flags.extend(["--orient".into(), format!("--orient-goal={}", kebab(goal))]);
                if *clean_surface {
                    flags.push("--clean-surface".into());
                }
                if let Some(tilt) = max_tilt_deg {
                    flags.push(format!("--max-tilt={tilt}"));
                }
                if let Some(pick) = pick {
                    flags.push(format!("--orient-pick={pick}"));
                }
                if keep_clear_at_mm.is_empty() {
                    return;
                }
                flags.extend(
                    keep_clear_at_mm
                        .iter()
                        .map(|at| format!("--orient-keep-clear={}", triple(*at))),
                );
                if let Some(seen) = keep_clear_seen_deg {
                    flags.push(format!("--orient-keep-clear-turn={}", triple(*seen)));
                }
                if let Some(radius) = keep_clear_radius_mm {
                    flags.push(format!("--orient-keep-clear-radius={radius}"));
                }
            }
            Self::Rotate { degrees } => flags.push(format!("--rotate={}", triple(*degrees))),
        }
    }
}

/// A catalogue id takes its own flag, a TOML file of the user's the path flag.
fn profile_flag(named: &str, by_id: &str, by_path: &str) -> String {
    let is_toml = Path::new(named)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"));
    format!("{}={named}", if is_toml { by_path } else { by_id })
}

/// The kebab-case name serde gives a unit variant, which is the command line's value too.
fn kebab<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn triple([x, y, z]: [f32; 3]) -> String {
    format!("{x},{y},{z}")
}

#[cfg(test)]
mod tests;
