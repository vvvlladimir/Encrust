use core_geometry::{Heightmap, Scalar};

use crate::MeshIoError;

/// The widest a picture is read at, pixels. Past this a tile of a few millimetres has more
/// pixels than a lattice can show, and the samples only cost memory.
pub const MAX_PICTURE_PX: u32 = 1024;

/// A picture read as one height per pixel.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub heights: Heightmap,
    /// The size of the picture as stored, pixels, before it was read down to
    /// [`MAX_PICTURE_PX`].
    pub size_px: [u32; 2],
}

/// `bytes` of any picture format read as grey, no wider than [`MAX_PICTURE_PX`]; `name` is
/// what an error calls it.
pub fn decode_picture(name: &str, bytes: &[u8]) -> Result<Picture, MeshIoError> {
    let bad = |reason: String| MeshIoError::UndecodableTexture {
        name: name.to_owned(),
        reason,
    };
    let image = image::load_from_memory(bytes).map_err(|source| bad(source.to_string()))?;
    let size_px = [image.width(), image.height()];
    let image = if size_px[0].max(size_px[1]) > MAX_PICTURE_PX {
        image.thumbnail(MAX_PICTURE_PX, MAX_PICTURE_PX)
    } else {
        image
    };
    let grey = image.to_luma32f();
    let (width, height) = (grey.width() as usize, grey.height() as usize);
    let samples: Vec<Scalar> = grey.into_raw();
    let heights = Heightmap::new(width, height, samples)
        .ok_or_else(|| bad("the image has no pixels".to_owned()))?;
    Ok(Picture { heights, size_px })
}

#[cfg(test)]
mod tests;
