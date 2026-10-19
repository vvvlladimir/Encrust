//! A line of text in a font, outlined, stood up as closed prisms and laid on a model's
//! surface, flat or bent onto it, to be raised out of it or sunk into it. See
//! `docs/design/labels.md`.

mod drape;
mod error;
mod font;
mod label;
mod outline;
mod prism;
mod sheet;

pub use error::TextError;
pub use font::Font;
pub use label::{Frame, Label, LabelSettings, label};
pub use outline::{Shape, outline};
pub use prism::prism;
