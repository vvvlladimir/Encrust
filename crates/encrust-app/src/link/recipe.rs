//! A job's revision as `encrust-mcp` hands it over (`docs/design/window-link.md`), and the plan
//! the window replays it by. What the window cannot replay yet is named, not guessed.

use std::path::PathBuf;

use core_geometry::{MendMethod, Scalar};
use core_plate::{OrientGoal, OrientPreset};
use core_volume::RepairMethod;
use serde::{Deserialize, Serialize};

use crate::prepare::{Choice, Plan};

/// The schema this window reads.
pub const SCHEMA: u32 = 1;

/// One line from the server: a revision it kept.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Request {
    pub schema: u32,
    pub token: String,
    pub job: String,
    pub revision: u32,
    pub recipe: Recipe,
}

/// The line the window answers with.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Reply {
    pub shown: bool,
    /// What of the recipe the plate does not show, in English for the model to read.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unshown: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why_not: Option<String>,
}

/// The parts of a recipe the window reads; a field it does not know is passed over.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Recipe {
    pub model: PathBuf,
    #[serde(default)]
    pub prepare: bool,
    pub repair: Option<Repair>,
    pub pose: Option<Pose>,
    pub hollow: Option<Hollow>,
    pub drains: Option<Drains>,
    pub supports: Option<Supports>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Repair {
    Full,
    CloseHoles,
    CloseSmallHoles,
    Turn,
    Wrap,
    Deep,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", tag = "by")]
pub enum Pose {
    Search {
        goal: Goal,
        max_tilt_deg: Option<f32>,
    },
    /// Its degrees are passed over: a turn is not replayed yet.
    Rotate {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Goal {
    FewestSupports,
    LeastPeel,
    ShortestPrint,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Hollow {
    pub wall_mm: Scalar,
    #[serde(default)]
    pub open_floor: bool,
    pub infill: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Drains {
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub at_mm: Vec<[f32; 3]>,
    #[serde(default)]
    pub plugs: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Supports {
    pub profile: String,
    #[serde(default)]
    pub cavity: bool,
}

impl Recipe {
    /// The plan that replays this recipe, and what of it the plate will not show.
    pub fn plan(&self) -> (Plan, Vec<String>) {
        let mut unshown = Vec::new();
        let orient = match &self.pose {
            None => self.left_out(),
            Some(Pose::Search { goal, max_tilt_deg }) => {
                if max_tilt_deg.is_some() {
                    unshown.push("max_tilt_deg: the window searches every tilt".to_owned());
                }
                Choice::Given(goal.preset())
            }
            Some(Pose::Rotate { .. }) => {
                // TODO(step-8): stand a turn given outright as `--rotate` does, with no job.
                unshown.push("a turn given in degrees: the model stands as it came in".to_owned());
                Choice::Skip
            }
        };
        let hollow = match &self.hollow {
            None => self.left_out(),
            Some(hollow) => {
                if hollow.open_floor || hollow.infill.is_some() {
                    // TODO(step-8): carry open_floor and infill into Hollow's settings for the run.
                    unshown
                        .push("open_floor and infill: the cavity is drawn empty and closed".into());
                }
                Choice::Given(hollow.wall_mm)
            }
        };
        let drain = match &self.drains {
            None => self.prepare,
            Some(drains) => {
                if !drains.at_mm.is_empty() || drains.plugs {
                    // TODO(step-8): drill the points given and lay the plugs, as the Drain tool does.
                    unshown
                        .push("drain points and plugs: only automatic drains are drilled".into());
                }
                drains.auto
            }
        };
        let (supports, cavity) = match &self.supports {
            None => (self.prepare, self.prepare),
            Some(supports) => {
                if supports.profile != "auto" {
                    // TODO(step-8): stand the model on the named profile's group.
                    unshown.push(format!(
                        "the {} profile: supports are sized by the automatic one",
                        supports.profile
                    ));
                }
                (true, supports.cavity)
            }
        };
        let repair = self
            .repair
            .map_or_else(|| self.left_out(), |repair| Choice::Given(repair.method()));
        let plan = Plan {
            orient,
            repair,
            hollow,
            drain,
            supports,
            cavity,
        };
        (plan, unshown)
    }
}

impl Recipe {
    /// A step the recipe says nothing of: the autopilot's under `prepare`, else not done.
    fn left_out<T>(&self) -> Choice<T> {
        if self.prepare {
            Choice::Decide
        } else {
            Choice::Skip
        }
    }
}

impl Goal {
    fn preset(self) -> OrientPreset {
        match self {
            Self::FewestSupports => OrientPreset::Goal(OrientGoal::FewestSupports),
            Self::LeastPeel => OrientPreset::Goal(OrientGoal::LeastPeel),
            Self::ShortestPrint => OrientPreset::Goal(OrientGoal::ShortestPrint),
            Self::Auto => OrientPreset::Auto,
        }
    }
}

impl Repair {
    /// The method `--repair` runs for this mode.
    fn method(self) -> RepairMethod {
        match self {
            Self::Full => RepairMethod::Mend(MendMethod::Repair),
            Self::CloseHoles => RepairMethod::Mend(MendMethod::CloseHoles),
            Self::CloseSmallHoles => RepairMethod::Mend(MendMethod::CloseSmallHoles),
            Self::Turn => RepairMethod::Mend(MendMethod::Turn),
            Self::Wrap => RepairMethod::Wrap,
            Self::Deep => RepairMethod::Deep,
        }
    }
}
