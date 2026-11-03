//! Nothing here starts a thread; the `*_file` functions are thin wrappers (ADR 0175).
//! See `docs/architecture.md`.

mod bake;
mod cut;
mod drains;
mod error;
mod file_name;
mod open;
mod outputs;
mod plate;
mod prepare;
pub mod project;
mod run;
mod stand;
mod trapped;
mod trial;

pub use bake::{Baked, bake, parts};
pub use cut::{Cutting, cut};
pub use drains::{
    DrainSearch, Drained, HoleRole, MOST_TRIES, Scanning, SuggestedHole, suggest_drains,
    suggest_drains_heard,
};
pub use error::EngineError;
pub use file_name::{DEFAULT_TEMPLATE, NameFields, file_stem, unknown_tags};
pub use open::{Machine, Opening, open_plate};
pub use outputs::{PlateFile, plate_outputs};
pub use plate::{Model, Plate};
pub use prepare::{Hollowing, PREPARE_ORIENT, PREPARE_REPAIR, hollowing, needs_repair};
pub use run::Run;
pub use stand::{
    ByHand, Lift, Stood, SupportPlan, SupportShapes, ToStand, planned_lift, stand_seated,
    stand_under,
};
pub use trapped::trapped_in;
pub use trial::{
    Choice, KeepClear, Printout, Proven, Trial, Tried, orient_for_print, print_out, prove,
};
