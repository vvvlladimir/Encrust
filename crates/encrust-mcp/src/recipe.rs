//! What a job asks of one model, written down so every revision can be cut again by the
//! command line: each field is one or more `encrust` flags (`docs/cli.md`).

use std::path::{Path, PathBuf};

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

mod shape;
mod slicing;

pub use shape::{Mend, Modelling, Relief};
pub use slicing::Slicing;

/// The preparation of one model for one printer and resin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub model: PathBuf,
    /// A printer id of the catalogue, or the path of a printer TOML.
    pub printer: String,
    /// A resin id of the user's, or the path of a resin TOML.
    pub resin: String,
    /// The engine's autopilot, `--prepare`, decides every step this recipe leaves out.
    #[serde(default)]
    pub prepare: bool,
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
    },
    /// Degrees around X, Y and Z, as `--rotate`.
    Rotate { degrees: [f32; 3] },
}

/// The lattice that fills a cavity, as `--infill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Infill {
    Hive,
    Grid,
    Scaffold,
}

/// Which surface the wall is measured from, as `--hollow-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HollowMode {
    /// The wall grows inward: a hollow model.
    Internal,
    /// The wall grows outward: a mould of the model.
    External,
}

/// A cavity grown inside the model, as `--hollow` and its flags.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hollow {
    pub wall_mm: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<HollowMode>,
    /// How fine the lattice the cavity is cut on, 0 to 1, as `--precision`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<f32>,
    /// Plate points round which the wall stays solid, mm, as `--keep-solid`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_solid_at_mm: Vec<[f32; 3]>,
    /// The radius of each of those, mm; 4 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_solid_radius_mm: Option<f32>,
    /// Opens the shell through the floor it stands on, so it drains with no hole.
    #[serde(default)]
    pub open_floor: bool,
    /// Absent for an empty cavity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infill: Option<Infill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infill_cell_mm: Option<f32>,
    /// From 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infill_density: Option<f32>,
}

/// Holes that let the resin out of a cavity, as `--drain` and `--drain-at`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Drains {
    pub diameter_mm: f32,
    /// Lets the engine find and prove a drain low on each trapped pocket, as `--drain-at auto`.
    #[serde(default)]
    pub auto: bool,
    /// With `auto`, a vent high on each pocket as well, so air can follow the resin out.
    #[serde(default = "yes")]
    pub vent: bool,
    /// Plate millimetres, from the plate's front left corner: the surface nearest each is drilled.
    #[serde(default)]
    pub at_mm: Vec<[f32; 3]>,
    /// Prints a plug for each hole beside the model.
    #[serde(default)]
    pub plugs: bool,
    /// How much narrower than its hole a plug is, mm a side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plug_gap_mm: Option<f32>,
    /// How far past the surface a hole reaches, mm; 3 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_mm: Option<f32>,
    /// A hole's far end as a fraction of its mouth: 1 is a cylinder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taper: Option<f32>,
    /// Cuts the holes and channels as polygons of this many sides, 3 to 12; round when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 3, max = 12))]
    pub sides: Option<u8>,
    /// Turns a polygonal hole about its axis, degrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_deg: Option<f32>,
    /// `walled` drills the holes before the cavity, so each keeps a wall: a recess, not a drain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<HoleKind>,
    /// Plate points nearest which a hole is drilled into the supports or raft, mm.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub support_at_mm: Vec<[f32; 3]>,
    /// Drainage channels, each a polyline of plate points, mm, as `--channel`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<Vec<[f32; 3]>>,
    /// The channels' diameter, mm; 2 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_mm: Option<f32>,
}

/// What a hole is drilled as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HoleKind {
    /// Through the wall into the cavity.
    Drain,
    /// Before the cavity, keeping a wall round it.
    Walled,
}

/// What the model stands on, as `--supports`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Supports {
    /// `auto`, a support profile `list_profiles` names (`light`, `medium`, `heavy`, …), or the
    /// absolute path of a support profile TOML.
    pub profile: String,
    /// Holds up the cavity's ceiling as well as the outside.
    #[serde(default)]
    pub cavity: bool,
    /// Plate points nearest which a support is stood as well, mm, as `--support-at`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub at_mm: Vec<[f32; 3]>,
    /// Stands only the supports at `at_mm`, none automatically.
    #[serde(default)]
    pub only_at: bool,
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
            repair: None,
            mend: None,
            modelling: None,
            relief: None,
            pose: None,
            hollow: None,
            drains: None,
            supports: None,
            slicing: None,
        }
    }

    /// The flags after the subcommand that stage the model as this recipe says. Every value
    /// rides on its flag as `--flag=value`, so a negative number is never read as a flag.
    pub fn flags(&self) -> Vec<String> {
        let mut flags = vec![self.model.display().to_string()];
        let others = self.modelling.iter().flat_map(Modelling::others);
        flags.extend(others.map(|other| other.display().to_string()));
        flags.extend([
            profile_flag(&self.printer, "--printer", "--profile"),
            profile_flag(&self.resin, "--resin", "--material"),
            "--center".into(),
            "--check-drainage".into(),
        ]);
        if self.prepare {
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
        flags
    }
}

impl Recipe {
    /// Refuses a path from the client that is not absolute and a name that would read as a
    /// flag, before anything reaches the command line.
    pub fn check_paths(&self) -> anyhow::Result<()> {
        if let Some(supports) = &self.supports {
            crate::params::plain(&supports.profile, "the support profile")?;
        }
        let others = self.modelling.iter().flat_map(Modelling::others);
        let image = self
            .relief
            .iter()
            .filter_map(|relief| relief.image.as_ref());
        for path in others.chain(image) {
            crate::params::absolute(path, "the file")?;
        }
        Ok(())
    }
}

impl Supports {
    fn push_flags(&self, flags: &mut Vec<String>) {
        flags.push(profile_flag(
            &self.profile,
            "--supports",
            "--support-profile",
        ));
        if self.cavity {
            flags.push("--support-cavity".into());
        }
        flags.extend(
            self.at_mm
                .iter()
                .map(|at| format!("--support-at={}", triple(*at))),
        );
        if self.only_at {
            flags.push("--support-only-at".into());
        }
    }
}

impl Pose {
    fn push_flags(&self, flags: &mut Vec<String>) {
        match self {
            Self::Search { goal, max_tilt_deg } => {
                flags.extend(["--orient".into(), format!("--orient-goal={}", kebab(goal))]);
                if let Some(tilt) = max_tilt_deg {
                    flags.push(format!("--max-tilt={tilt}"));
                }
            }
            Self::Rotate { degrees } => flags.push(format!("--rotate={}", triple(*degrees))),
        }
    }
}

impl Hollow {
    fn push_flags(&self, flags: &mut Vec<String>) {
        flags.push(format!("--hollow={}", self.wall_mm));
        if self.open_floor {
            flags.push("--hollow-open-floor".into());
        }
        if let Some(infill) = &self.infill {
            flags.push(format!("--infill={}", kebab(infill)));
            if let Some(cell) = self.infill_cell_mm {
                flags.push(format!("--infill-cell={cell}"));
            }
            if let Some(density) = self.infill_density {
                flags.push(format!("--infill-density={density}"));
            }
        }
        if let Some(mode) = self.mode {
            flags.push(format!("--hollow-mode={}", kebab(&mode)));
        }
        if let Some(precision) = self.precision {
            flags.push(format!("--precision={precision}"));
        }
        flags.extend(
            self.keep_solid_at_mm
                .iter()
                .map(|at| format!("--keep-solid={}", triple(*at))),
        );
        if let Some(mm) = self
            .keep_solid_radius_mm
            .filter(|_| !self.keep_solid_at_mm.is_empty())
        {
            flags.push(format!("--keep-solid-radius={mm}"));
        }
    }
}

impl Drains {
    fn push_flags(&self, flags: &mut Vec<String>) {
        let cutting = !self.at_mm.is_empty() || self.auto || !self.channels.is_empty();
        if !cutting && self.support_at_mm.is_empty() {
            return;
        }
        flags.push(format!("--drain={}", self.diameter_mm));
        for (flag, value) in [
            ("--drain-depth", self.depth_mm),
            ("--drain-taper", self.taper),
            ("--drain-turn", self.turn_deg),
            ("--plug-gap", self.plug_gap_mm),
            ("--channel-size", self.channel_mm),
        ] {
            if let Some(value) = value {
                flags.push(format!("{flag}={value}"));
            }
        }
        if let Some(sides) = self.sides {
            flags.push(format!("--drain-sides={sides}"));
        }
        if self.kind == Some(HoleKind::Walled) {
            flags.push("--drain-walled".into());
        }
        flags.extend(
            self.support_at_mm
                .iter()
                .map(|at| format!("--drain-support-at={}", triple(*at))),
        );
        for channel in &self.channels {
            let points: Vec<String> = channel.iter().map(|at| triple(*at)).collect();
            flags.push(format!("--channel={}", points.join(":")));
        }
        for at in &self.at_mm {
            flags.push(format!("--drain-at={}", triple(*at)));
        }
        if self.auto {
            flags.push("--drain-at=auto".into());
            if !self.vent {
                flags.push("--no-drain-vent".into());
            }
        }
        if self.plugs {
            flags.push("--drain-plugs".into());
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

fn yes() -> bool {
    true
}

fn triple([x, y, z]: [f32; 3]) -> String {
    format!("{x},{y},{z}")
}

#[cfg(test)]
mod tests;
