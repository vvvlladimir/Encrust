//! A recipe: the steps Prepare for print runs, in their order, each with its condition and
//! settings, and the answer for one step on one model. See `docs/design/recipes.md`.

mod builtin;
mod condition;
mod decide;
mod error;
mod metrics;
mod recipe;
mod validate;

pub use builtin::{BUILT_IN, built_in};
pub use condition::{BinaryOp, Condition, Expr, Function, Kind, Value};
pub use decide::{Decision, Foreseen, Reading, Resolved, decide, preview, retry};
pub use error::RecipeError;
pub use metrics::{Metric, Metrics, Model, measure};
pub use recipe::{
    Choice, FORMAT, MAX_RETRY, OnFail, Recipe, Retry, Setting, SettingKind, Settings, Step,
    StepKind,
};
pub use validate::{Plan, validate};
