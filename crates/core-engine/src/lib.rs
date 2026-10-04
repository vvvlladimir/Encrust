//! A plate of placed models, cut and written into a printable sliced file.
//!
//! The stage above `core-pipeline`: what the window, the command line and a browser all
//! do between the plate a user arranged and the file a printer reads — bake the models
//! into one mesh in plate coordinates, work out the layers, and stream them into a sink.
//! Nothing here starts a thread, and every stage works over a sink or a source; the
//! handful of `*_file` functions are thin wrappers for a caller that happens to have a
//! path (ADR 0175). So one run drives a window, a terminal and a web worker alike.
//! [`project`] is the same plate written down: the `.encrust` file, as data and
//! (de)serialisation with no front end in it. See `docs/architecture.md`.

mod bake;
mod cut;
mod error;
mod open;
mod plate;
pub mod project;
mod run;

pub use bake::{bake, parts};
pub use cut::{Cutting, cut};
pub use error::EngineError;
pub use open::{Opening, open_plate};
pub use plate::{Model, Plate};
pub use run::Run;
