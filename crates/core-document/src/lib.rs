//! The plate as one document with stable object ids, which every front end edits the same
//! way (ADR 0466, `docs/design/document.md`).

mod apply;
mod cut;
mod document;
mod error;
#[cfg(test)]
mod fixtures;
mod piece;

pub use apply::{Applied, Report, apply};
pub use cut::{CutAsked, CutModel, axis_normal, cut, pieces};
pub use document::Document;
pub use error::ApplyError;
pub use piece::{Piece, Role};
