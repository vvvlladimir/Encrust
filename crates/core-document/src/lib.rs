//! The plate as one document with stable object ids, which every front end edits the same
//! way (ADR 0466, `docs/design/document.md`).

mod apply;
mod arrange;
mod combine;
mod cut;
mod document;
mod drain;
mod error;
#[cfg(test)]
mod fixtures;
mod hollow;
mod label;
mod orient;
mod piece;
mod placement;
mod plate_contents;
mod relief;
mod repair;
mod split;
mod supports;
mod surface;

pub use apply::{Applied, Report, apply};
pub use arrange::{NoFootprint, Spread, Standing, spread, standing_rect, turn_to_fit, turned_by};
pub use combine::combined;
pub use cut::{CutAsked, CutModel, axis_normal, cut, pieces};
pub use document::{Document, ImportedModel};
pub use drain::{SearchAsked, channel_in, drains_proven, hole_near, plate_under, spaced_plugs};
pub use error::ApplyError;
pub use hollow::{NoRoom, blockers_along, settings_of, shell_from, shelled};
pub use label::{LabelAsked, Lettering, label_prisms, labelled};
pub use orient::{
    Body, Ranked, kept_clear, lifted, orient_plan, placed_by, preset_of, supported_settings,
    vents_of,
};
pub use piece::{Piece, Role};
pub use placement::{
    Footing, degrees_of, euler_degrees, lift_by, moved, onto_plate, sized_to, stood_turned,
    turned_to,
};
pub use plate_contents::mirrored;
pub use relief::{ReliefAsked, Relieved, press_picture, read_picture, relieved};
pub use repair::{MendAsked, Mended, mended, repair_settings, repaired};
pub use split::{part_off_at, part_settings, split};
pub use supports::{standing_meshes, support_near};
