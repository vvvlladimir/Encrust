use thiserror::Error;

use crate::condition::Kind;
use crate::recipe::StepKind;

/// What is wrong with a recipe, found when it is read or when a step is decided. `step` is
/// the step's place in the file, from 0.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RecipeError {
    #[error("the recipe is not TOML this layout reads: {message}")]
    Toml { message: String },
    #[error("the recipe is format {found}; this build reads up to {supported}")]
    Format { found: u32, supported: u32 },
    #[error("step {step}: there is no step `{name}`")]
    UnknownStep { step: usize, name: String },
    #[error("step {step}: `{kind:?}` takes no setting `{key}`")]
    UnknownSetting {
        step: usize,
        kind: StepKind,
        key: String,
    },
    #[error("step {step}: `{key}` must be {expected}, not `{found}`")]
    BadSetting {
        step: usize,
        key: String,
        expected: String,
        found: String,
    },
    #[error("step {step}: `{kind:?}` needs `{key}`")]
    MissingSetting {
        step: usize,
        kind: StepKind,
        key: &'static str,
    },
    #[error("`{text}`: {expected} expected at character {at}")]
    Syntax {
        text: String,
        at: usize,
        expected: &'static str,
    },
    #[error("`{name}` is no variable of a model nor of the recipe")]
    UnknownName { name: String },
    #[error("`{name}` is no function; min, max, abs and clamp are")]
    UnknownFunction { name: String },
    #[error("`{name}` takes {expected} arguments, not {found}")]
    Arity {
        name: &'static str,
        expected: usize,
        found: usize,
    },
    #[error("`{text}`: a {expected:?} is wanted where a {found:?} stands")]
    WrongKind {
        text: String,
        expected: Kind,
        found: Kind,
    },
    #[error("`{text}` divides by zero")]
    DivisionByZero { text: String },
    #[error("`{name}` in the recipe's vars has the name of a model's variable")]
    Shadowed { name: String },
    #[error("step {step}: `{name}` is measured only after `{after:?}`, which does not come first")]
    NotYetKnown {
        step: usize,
        name: String,
        after: StepKind,
    },
    #[error("`{name}` is not measured yet")]
    Unmeasured { name: String },
    #[error("step {step}: `{kind:?}` comes after `{after:?}`")]
    OutOfOrder {
        step: usize,
        kind: StepKind,
        after: StepKind,
    },
    #[error("step {step}: `{kind:?}` is already step {first}")]
    Repeated {
        step: usize,
        kind: StepKind,
        first: usize,
    },
    #[error("step {step}: a retry runs at most {limit} times, not {max}")]
    RetryTooMany { step: usize, max: u8, limit: u8 },
    #[error("there is no step {step}")]
    NoSuchStep { step: usize },
    #[error("there is no built-in recipe `{id}`")]
    UnknownBuiltIn { id: String },
}
