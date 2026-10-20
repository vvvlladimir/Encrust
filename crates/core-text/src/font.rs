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
        let face = font.face()?;
        let tables = face.tables();
        if tables.glyf.is_none() && tables.cff.is_none() && tables.cff2.is_none() {
            return Err(TextError::BadFont("the font has no outlines".to_owned()));
        }
        Ok(font)
    }

    pub(crate) fn face(&self) -> Result<Face<'_>, TextError> {
        refuse_overlong_collection(&self.data)?;
        Face::parse(&self.data, self.index).map_err(|error| TextError::BadFont(error.to_string()))
    }
}

/// Refuses a collection claiming more faces than its offset table has bytes for. The
/// parser debug-asserts on such a claim instead of answering, so it must not see one.
fn refuse_overlong_collection(data: &[u8]) -> Result<(), TextError> {
    // The collection header is a tag, a version and a count, then one 32-bit offset a face.
    const HEADER_BYTES: u64 = 12;
    let Some(faces) = ttf_parser::fonts_in_collection(data) else {
        return Ok(());
    };
    if HEADER_BYTES + 4 * u64::from(faces) > data.len() as u64 {
        return Err(TextError::BadFont(format!(
            "the collection claims {faces} faces in {} bytes",
            data.len()
        )));
    }
    Ok(())
}
