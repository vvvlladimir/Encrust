//! Nothing here starts a thread; the `*_file` functions are thin wrappers (ADR 0175).
//! See `docs/architecture.md`.

mod analysis;
mod bake;
mod cut;
mod drains;
mod error;
mod file_name;
mod held;
mod kept_poses;
mod open;
mod outputs;
mod plate;
mod prepare;
pub mod project;
mod run;
mod stand;
mod strengthen;
mod trapped;
mod trial;

pub use analysis::{ANALYSIS_LAYER_MM, analysis_slicing};
// What an arrange edit is asked with where nothing is typed (`core_edit::defaults`).
pub use bake::{Baked, bake, parts};
pub use core_plate::ArrangeSettings;
pub use cut::{Cutting, cut};
pub use drains::{
    DrainSearch, Drained, HoleRole, MOST_TRIES, Scanning, SuggestedHole, suggest_drains,
    suggest_drains_heard,
};
pub use error::EngineError;
pub use file_name::{DEFAULT_TEMPLATE, NameFields, file_stem, unknown_tags};
pub use held::{
    CupVent, Hanging, Unvented, cup_vents, held_by_shape, island_floors, island_holds, lever_holds,
    neck_profile,
};
pub use kept_poses::{KEPT_POSES, KEPT_POSES_ENV, kept_poses_folder, orient_for_print_kept};
pub use open::{Machine, Opening, object_model, open_plate, open_plate_of, plate_models};
pub use outputs::{PlateFile, plate_outputs};
pub use plate::{Model, Plate};
pub use prepare::{Hollowing, PREPARE_REPAIR, hollowing, needs_repair};
pub use run::{QUICK_CHECK_BIN, Run};
pub use stand::{
    ByHand, Lift, NeedSides, NeedsAsked, ShownAt, Stood, SupportPlan, SupportShapes, ToStand,
    merge_refusal, nearest_named, needs_on, needs_on_both, plan_of, planned_lift, shown_on,
    stand_at, stand_seated, stand_under,
};
pub use strengthen::{Strengthened, Underside, WANTED_MARGIN, carried, strengthen};
pub use trapped::trapped_in;
pub use trial::{
    Choice, KEEP_CLEAR_RADIUS_MM, KeepClear, Printout, Proven, Shows, Trial, Tried,
    orient_for_print, print_out, prove,
};
