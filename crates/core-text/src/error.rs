use core_geometry::Scalar;
use thiserror::Error;

/// What can go wrong outlining text or laying it on a model.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TextError {
    /// The bytes are not a font this crate can read; the reason is the parser's own.
    #[error("the font could not be read: {0}")]
    BadFont(String),

    /// The font has no glyph for a character of the text.
    #[error("the font has no glyph for {0:?}")]
    MissingGlyph(char),

    /// Nothing in the text has an outline: it is empty or only spaces.
    #[error("there is no text to write")]
    Blank,

    /// A size the label is asked for is not a positive length; the name says which.
    #[error("the {what} must be a positive length, got {value_mm} mm")]
    BadSize {
        what: &'static str,
        value_mm: Scalar,
    },

    /// No ray from the label met the model, so it stands on nothing.
    #[error("the label does not stand on the model")]
    OffTheModel,
}
