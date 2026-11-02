//! A Model Context Protocol server over stdio: a language model prepares a model for print by
//! driving the `encrust` command line, a job of numbered revisions at a time. See
//! `docs/design/ai-integration.md`.

mod jobs;
mod recipe;
mod runner;
mod server;

pub use jobs::{Jobs, Revision};
pub use recipe::{Drains, Hollow, Pose, Recipe, Supports};
pub use runner::{Cli, Outcome};
pub use server::Server;
