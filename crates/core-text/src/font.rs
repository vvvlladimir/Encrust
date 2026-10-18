use std::sync::Arc;

use ttf_parser::Face;

use crate::error::TextError;

/// A font file's bytes and the face in it to write with. Cheap to clone; the face is read
/// again each time it is asked for, which costs a table lookup.
#[derive(Debug, Clone)]
pub struct Font {
    data: Arc<Vec<u8>>,
    index: u32,
}

impl Font {
    /// Face `index` of the font file `data`, refused unless it parses and has outlines.
    pub fn new(data: Arc<Vec<u8>>, index: u32) -> Result<Self, TextError> {
        let font = Self { data, index };
        font.face()?;
        Ok(font)
    }

    pub(crate) fn face(&self) -> Result<Face<'_>, TextError> {
        Face::parse(&self.data, self.index).map_err(|error| TextError::BadFont(error.to_string()))
    }
}
