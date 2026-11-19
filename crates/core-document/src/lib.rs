//! The plate as one document with stable object ids, which every front end edits the same
//! way (ADR 0466, `docs/design/document.md`).

mod apply;
mod combine;
mod cut;
mod document;
mod drain;
mod error;
#[cfg(test)]
mod fixtures;
mod hollow;
mod label;
mod piece;
mod placement;
mod plate_contents;
mod relief;
mod repair;
mod split;
mod supports;

pub use apply::{Applied, Report, apply};
pub use combine::{boolean_of, combined};
pub use cut::{CutAsked, CutModel, axis_normal, cut, pieces};
pub use document::{Document, ImportedModel};
pub use drain::hole_near;
pub use error::ApplyError;
pub use hollow::{NoRoom, settings_of, shell_from, shelled};
pub use label::{LabelAsked, Lettering, label_prisms, labelled};
pub use piece::{Piece, Role};
pub use placement::{euler_degrees, moved, sized_to, turned_to};
pub use plate_contents::mirrored;
pub use relief::{ReliefAsked, Relieved, press_picture, read_picture, relieved};
pub use repair::{MendAsked, Mended, mended, repaired};
pub use split::{part_off_at, part_settings, split};
pub use supports::AUTO;
