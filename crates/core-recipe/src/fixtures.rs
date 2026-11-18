//! Recipes the tests read: a body of steps under a name, and one step's text.

use crate::error::RecipeError;
use crate::recipe::Recipe;
use crate::validate::{Plan, validate};

/// `body` under the name `t`, read and checked.
pub fn checked(body: &str) -> Result<Plan, RecipeError> {
    validate(Recipe::from_toml(&format!("name = \"t\"\n{body}")).expect("the fixture reads"))
}

/// `body` under the name `t`, a recipe the test knows is valid.
pub fn plan(body: &str) -> Plan {
    checked(body).expect("the fixture is a valid recipe")
}

/// One step of `kind` with `extra` lines.
pub fn step(kind: &str, extra: &str) -> String {
    format!("[[step]]\ndo = \"{kind}\"\n{extra}\n")
}
