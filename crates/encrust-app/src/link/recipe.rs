//! A job's revision as `encrust-mcp` hands it over (`docs/design/window-link.md`), and the plan
//! the window replays it by. Where the window does a step differently it says so.

use std::path::PathBuf;

use core_engine::project::{DrainState, HollowState};
use core_geometry::{MendMethod, Scalar, Vec3, rotation_xyz_deg};
use core_plate::{OrientGoal, OrientPreset};
use core_volume::{HollowMode, InfillPattern, InfillSettings, RepairMethod};
use serde::{Deserialize, Serialize};

use crate::hollow::DEFAULT_BLOCKER_MM;
use crate::prepare::{Choice, Drains as PlanDrains, Orient, Plan};

/// What `--precision` is when the server gives none, as `docs/cli.md` states it.
const CLI_PRECISION: Scalar = 0.5;
/// What `--drain-depth` is when the server gives none.
const CLI_DRAIN_DEPTH_MM: Scalar = 3.0;

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
        max_tilt_deg: Option<Scalar>,
    },
    Rotate {
        degrees: [Scalar; 3],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Goal {
    FewestSupports,
    LeastPeel,
    ShortestPrint,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Infill {
    Hive,
    Grid,
    Scaffold,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Hollow {
    pub wall_mm: Scalar,
    #[serde(default)]
    pub open_floor: bool,
    pub infill: Option<Infill>,
    pub infill_cell_mm: Option<Scalar>,
    pub infill_density: Option<Scalar>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Drains {
    pub diameter_mm: Scalar,
    #[serde(default)]
    pub auto: bool,
    #[serde(default = "yes")]
    pub vent: bool,
    #[serde(default)]
    pub at_mm: Vec<[Scalar; 3]>,
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
    /// The plan that replays this recipe, and what of it the plate does not show.
    pub fn plan(&self) -> (Plan, Vec<String>) {
        let mut unshown = Vec::new();
        let orient = match &self.pose {
            None => self.left_out(),
            Some(Pose::Search { goal, max_tilt_deg }) => Choice::Given(Orient::Search {
                preset: goal.preset(),
                max_tilt_deg: *max_tilt_deg,
            }),
            Some(Pose::Rotate { degrees }) => {
                Choice::Given(Orient::Turn(rotation_xyz_deg(Vec3::from_array(*degrees))))
            }
        };
        let hollow = self
            .hollow
            .as_ref()
            .map_or_else(|| self.left_out(), |hollow| Choice::Given(hollow.state()));
        let drains = match &self.drains {
            None => self.prepare.then(|| Plan::autopilot().drains).flatten(),
            Some(drains) => {
                if !drains.plugs && (drains.auto || !drains.at_mm.is_empty()) {
                    unshown.push("plugs: the window lays a plug beside every hole".to_owned());
                }
                Some(drains.plan())
            }
        };
        let (supports, cavity) = match &self.supports {
            None => (self.prepare.then_some(None), self.prepare),
            Some(supports) => {
                let named = (supports.profile != "auto").then(|| supports.profile.clone());
                (Some(named), supports.cavity)
            }
        };
        let repair = self
            .repair
            .map_or_else(|| self.left_out(), |repair| Choice::Given(repair.method()));
        let plan = Plan {
            orient,
            repair,
            hollow,
            drains,
            supports,
            cavity,
        };
        (plan, unshown)
    }
}

impl Hollow {
    /// Hollow's settings as the command line's flags ask for them: a wall measured inside, at
    /// the command line's precision, every infill size left out at its default.
    fn state(&self) -> HollowState {
        let default = InfillSettings::default();
        HollowState {
            thickness_mm: self.wall_mm,
            mode: HollowMode::Internal,
            precision: CLI_PRECISION,
            infill_on: self.infill.is_some(),
            infill: InfillSettings {
                pattern: self.infill.map_or(default.pattern, Infill::pattern),
                cell_mm: self.infill_cell_mm.unwrap_or(default.cell_mm),
                density: self.infill_density.unwrap_or(default.density),
            },
            blocker_mm: DEFAULT_BLOCKER_MM,
            open_floor: self.open_floor,
        }
    }
}

impl Drains {
    fn plan(&self) -> PlanDrains {
        PlanDrains {
            at_mm: self.at_mm.iter().copied().map(Vec3::from_array).collect(),
            auto: self.auto,
            vent: self.vent,
            size: Some(DrainState::round(self.diameter_mm, CLI_DRAIN_DEPTH_MM, 1.0)),
        }
    }
}

impl Infill {
    fn pattern(self) -> InfillPattern {
        match self {
            Self::Hive => InfillPattern::Hive,
            Self::Grid => InfillPattern::Grid,
            Self::Scaffold => InfillPattern::Scaffold,
        }
    }
}

fn yes() -> bool {
    true
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
