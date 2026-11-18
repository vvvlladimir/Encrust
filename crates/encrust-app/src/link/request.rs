//! The line `encrust-mcp` sends for each revision it keeps, and the window's answer
//! (`docs/design/window-link.md`).

use std::path::PathBuf;

use anyhow::{Context, Result};
use core_recipe::{Plan, Recipe, validate};
use serde::{Deserialize, Serialize};

/// The schema of the line this window reads.
pub const SCHEMA: u64 = 2;

/// One revision of a chat client's job: its model, its machine and the recipe its check ran.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    pub schema: u64,
    pub token: String,
    pub job: String,
    pub revision: u32,
    pub model: PathBuf,
    /// A printer id of the catalogue, or the path of a printer TOML.
    pub printer: String,
    /// A resin id, or the path of a resin TOML.
    pub resin: String,
    /// A `core-recipe` file's text, the flags typed folded in (ADR 0459).
    pub recipe: String,
}

impl Request {
    /// The recipe read and checked, as Prepare for print runs it.
    pub fn plan(&self) -> Result<Plan> {
        let recipe = Recipe::from_toml(&self.recipe).context("the revision's recipe")?;
        validate(recipe).context("the revision's recipe")
    }
}

/// What the window says of a revision.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reply {
    pub shown: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why_not: Option<String>,
}

impl Reply {
    pub fn taken() -> Self {
        Self {
            shown: true,
            why_not: None,
        }
    }

    pub fn refused(why: String) -> Self {
        Self {
            shown: false,
            why_not: Some(why),
        }
    }
}
