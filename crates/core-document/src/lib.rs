//! The plate as one document with stable object ids, which every front end edits the same
//! way (ADR 0466, `docs/design/document.md`).

mod apply;
mod combine;
mod cut;
mod document;
mod error;
#[cfg(test)]
mod fixtures;
mod piece;
mod placement;
mod plate_contents;
mod split;

pub use apply::{Applied, Report, apply};
pub use combine::{boolean_of, combined};
pub use cut::{CutAsked, CutModel, axis_normal, cut, pieces};
pub use document::Document;
pub use error::ApplyError;
pub use piece::{Piece, Role};
pub use placement::{euler_degrees, moved, sized_to, turned_to};
pub use plate_contents::mirrored;
pub use split::{part_off_at, part_settings, split};
