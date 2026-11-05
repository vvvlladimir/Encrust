//! A recipe as its file reads: its steps, each with its condition and settings. The layout is
//! in `docs/design/recipes.md`; what may follow what is `validate`'s.

use std::collections::BTreeMap;

use core_geometry::Scalar;

use crate::condition::Condition;
use crate::error::RecipeError;

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
        // TODO(step-2): the `do` column of docs/design/recipes.md.
        todo!()
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// The keys its `set` takes, and what each takes.
    pub fn settings(self) -> &'static [(&'static str, SettingKind)] {
        // TODO(step-2): the Settings column of docs/design/recipes.md.
        todo!()
    }

    /// Whether it changes the model, so it appears once and has a place in the order.
    pub fn is_work(self) -> bool {
        !matches!(self, Self::Check | Self::Gate | Self::Pause)
    }
}

/// One setting as written: a value, or an expression after `=`.
#[derive(Debug, Clone, PartialEq)]
pub enum Setting {
    Number(Scalar),
    Bool(bool),
    Word(String),
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
        // TODO(step-2): serde over the `toml` crate into a raw table, then each step checked.
        let _ = text;
        todo!()
    }

    /// The file's text, which `from_toml` reads back to the same recipe.
    pub fn to_toml(&self) -> String {
        // TODO(step-2): the editor saves through this.
        todo!()
    }
}
