//! What a job finishes the model with — a cavity, the holes that drain it and what it stands
//! on — each field one or more flags.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

use super::{kebab, profile_flag, triple};

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
    /// How far each of those reaches past the wall into the cavity, mm, as `--keep-solid-size`;
    /// fitted to the model when absent.
    #[serde(
        default,
        alias = "keep_solid_radius_mm",
        skip_serializing_if = "Option::is_none"
    )]
    pub keep_solid_reach_mm: Option<f32>,
    /// Keeps the wall solid wherever the model is thinner than this, mm — fingers, limbs,
    /// blades — as `--keep-solid-under`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_solid_under_mm: Option<f32>,
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
    /// The ids of the job's objects this shapes, as `inspect_model` lists them on its plate;
    /// every one when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<u64>,
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
    /// The ids of the job's objects this shapes, as `inspect_model` lists them on its plate;
    /// every one when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<u64>,
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
    /// Stands the model this high off the plate, mm, room for supports under it, as `--lift`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lift_mm: Option<f32>,
    /// Holds up the cavity's ceiling as well as the outside.
    #[serde(default)]
    pub cavity: bool,
    /// Plate points nearest which a support is stood as well, mm, as `--support-at`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub at_mm: Vec<[f32; 3]>,
    /// Stands only the supports at `at_mm`, none automatically.
    #[serde(default)]
    pub only_at: bool,
    /// Plate points round which the surface is the side meant to be seen — a face, the front
    /// of a figure — where contacts stand sparser and lightest, mm, as `--shown-at`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shown_at_mm: Vec<[f32; 3]>,
    /// How far round each of those the side reaches, mm; 4 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shown_radius_mm: Option<f32>,
    /// Groups of two plate points or more, mm: the supports whose contacts lie nearest one
    /// group's points are joined into one tree, as `--merge-supports`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merge_at_mm: Vec<Vec<[f32; 3]>>,
    /// The ids of the job's objects this shapes, as `inspect_model` lists them on its plate;
    /// every one when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<u64>,
}

/// Check's automatic answers, run on the plate once the rest is done, as `--support-islands`
/// and the flags beside it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Fixes {
    /// A support under every island, checking again up to three times.
    #[serde(default)]
    pub support_islands: bool,
    /// A vent drilled into every suction cup, through its thinnest wall.
    #[serde(default)]
    pub drill_cups: bool,
    /// Supports under every weak neck, thick enough to hold it.
    #[serde(default)]
    pub hold_necks: bool,
}

impl Fixes {
    pub(super) fn push_flags(self, flags: &mut Vec<String>) {
        for (asked, flag) in [
            (self.support_islands, "--support-islands"),
            (self.drill_cups, "--drill-cups"),
            (self.hold_necks, "--hold-necks"),
        ] {
            if asked {
                flags.push(flag.into());
            }
        }
    }
}

impl Supports {
    pub(super) fn push_flags(&self, flags: &mut Vec<String>) {
        flags.push(profile_flag(
            &self.profile,
            "--supports",
            "--support-profile",
        ));
        if self.cavity {
            flags.push("--support-cavity".into());
        }
        if let Some(mm) = self.lift_mm {
            flags.push(format!("--lift={mm}"));
        }
        flags.extend(
            self.at_mm
                .iter()
                .map(|at| format!("--support-at={}", triple(*at))),
        );
        if self.only_at {
            flags.push("--support-only-at".into());
        }
        let radius = self
            .shown_radius_mm
            .map(|mm| format!(":{mm}"))
            .unwrap_or_default();
        flags.extend(
            self.shown_at_mm
                .iter()
                .map(|at| format!("--shown-at={}{radius}", triple(*at))),
        );
        for group in &self.merge_at_mm {
            let points: Vec<String> = group.iter().map(|at| triple(*at)).collect();
            flags.push(format!("--merge-supports={}", points.join(";")));
        }
    }
}

impl Hollow {
    pub(super) fn push_flags(&self, flags: &mut Vec<String>) {
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
        if let Some(mm) = self.keep_solid_under_mm {
            flags.push(format!("--keep-solid-under={mm}"));
        }
        if let Some(mm) = self
            .keep_solid_reach_mm
            .filter(|_| !self.keep_solid_at_mm.is_empty())
        {
            flags.push(format!("--keep-solid-size={mm}"));
        }
    }
}

impl Drains {
    pub(super) fn push_flags(&self, flags: &mut Vec<String>) {
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

fn yes() -> bool {
    true
}
