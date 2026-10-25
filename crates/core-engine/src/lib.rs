//! Nothing here starts a thread; the `*_file` functions are thin wrappers (ADR 0175).
//! See `docs/architecture.md`.

mod bake;
mod cut;
mod error;
mod open;
mod outputs;
mod plate;
pub mod project;
mod run;
mod stand;
mod trial;

pub use bake::{Baked, bake, parts};
pub use cut::{Cutting, cut};
pub use error::EngineError;
pub use open::{Machine, Opening, open_plate};
pub use outputs::plate_outputs;
pub use plate::{Model, Plate};
pub use run::Run;
pub use stand::{Stood, SupportPlan, SupportShapes, ToStand, stand_seated, stand_under};
pub use trial::{Choice, Printout, Proven, Trial, Tried, orient_for_print, print_out, prove};
