//! A recipe as its file reads: its steps, each with its condition and settings. The layout is
//! in `docs/design/recipes.md`; what may follow what is `validate`'s.

use std::collections::BTreeMap;

use core_geometry::Scalar;

use crate::condition::Condition;
use crate::error::RecipeError;

mod read;
mod write;

#[cfg(test)]
mod tests;

/// The layout this build reads and writes, the file's `format`.
pub const FORMAT: u32 = 1;

/// The most times `check` supports islands again.
pub const MAX_RETRY: u8 = 5;

/// What a step does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StepKind {
    Repair,
    Orient,
    Hollow,
    Drain,
    Supports,
    Check,
    Arrange,
    Save,
    Gate,
    Pause,
}

/// What a setting takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    /// In the unit its key names.
    Number,
    Bool,
    /// One of these words.
    Word(&'static [&'static str]),
    /// Free text: a folder, a name template.
    Text,
    /// `[x, y, z]` fractions of the placed box, each from 0 to 1.
    Points,
}

impl StepKind {
    pub const ALL: [Self; 10] = [
        Self::Repair,
        Self::Orient,
        Self::Hollow,
        Self::Drain,
        Self::Supports,
        Self::Check,
        Self::Arrange,
        Self::Save,
        Self::Gate,
        Self::Pause,
    ];

    /// The word `do` names it by.
    pub fn name(self) -> &'static str {
        match self {
            Self::Repair => "repair",
            Self::Orient => "orient",
            Self::Hollow => "hollow",
            Self::Drain => "drain",
            Self::Supports => "supports",
            Self::Check => "check",
            Self::Arrange => "arrange",
            Self::Save => "save",
            Self::Gate => "gate",
            Self::Pause => "pause",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// The keys its `set` takes, and what each takes.
    pub fn settings(self) -> &'static [(&'static str, SettingKind)] {
        use SettingKind::{Bool, Number, Points, Text, Word};
        match self {
            Self::Repair => &[("method", Word(&["repair", "close-holes", "wrap", "deep"]))],
            Self::Orient => &[
                (
                    "goal",
                    Word(&["auto", "fewest-supports", "least-peel", "shortest-print"]),
                ),
                ("max_tilt_deg", Number),
            ],
            Self::Hollow => &[
                ("wall_mm", Number),
                ("mode", Word(&["inside", "outside", "open-floor"])),
                ("infill", Bool),
                ("keep_solid_under_mm", Number),
            ],
            Self::Drain => &[
                ("at", Word(&["auto", "points"])),
                ("points", Points),
                ("diameter_mm", Number),
                ("depth_mm", Number),
                ("vent", Bool),
                ("plugs", Bool),
            ],
            Self::Supports => &[("profile", Text), ("cavity", Bool)],
            Self::Arrange => &[("spacing_mm", Number)],
            Self::Save => &[("folder", Text), ("name", Text), ("send", Bool)],
            Self::Check | Self::Gate | Self::Pause => &[],
        }
    }
}

/// One setting as written: a value, or an expression after `=`.
#[derive(Debug, Clone, PartialEq)]
pub enum Setting {
    Number(Scalar),
    Bool(bool),
    Word(String),
    Text(String),
    Points(Vec<[Scalar; 3]>),
    Expr(Condition),
}

pub type Settings = BTreeMap<String, Setting>;

/// Settings merged over a step's own when `when` holds; the first that holds is taken.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub when: Condition,
    pub set: Settings,
}

/// What a failed step does to the run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OnFail {
    /// The plate put back as the run found it.
    #[default]
    Stop,
    Skip,
}

/// `check`'s islands supported again while `while_` holds, at most `max` times.
#[derive(Debug, Clone, PartialEq)]
pub struct Retry {
    pub while_: Condition,
    pub max: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub kind: StepKind,
    pub enabled: bool,
    pub when: Option<Condition>,
    pub set: Settings,
    pub choose: Vec<Choice>,
    pub on_fail: OnFail,
    /// A gate's or a pause's words to the user.
    pub message: Option<String>,
    /// A gate's condition.
    pub stop_if: Option<Condition>,
    pub retry: Option<Retry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub name: String,
    /// The `[vars]` table.
    pub vars: BTreeMap<String, Scalar>,
    pub steps: Vec<Step>,
}

impl Recipe {
    /// Reads a recipe file's text, its keys and kinds checked but not its order.
    pub fn from_toml(text: &str) -> Result<Self, RecipeError> {
        read::recipe_from_toml(text)
    }

    /// The file's text, which `from_toml` reads back to the same recipe.
    pub fn to_toml(&self) -> String {
        write::recipe_to_toml(self)
    }
}
