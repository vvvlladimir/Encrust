use core_geometry::{Heightmap, Mesh, Scalar, UvMap};

use crate::MeshIoError;

/// Everything a loader read out of one file.
///
/// The mesh is what gets sliced; the rest is there for the tools that press a texture into
/// it, and is `None` for a file that carries none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Loaded {
    pub mesh: Mesh,
    /// Texture coordinates, one pair per corner of each face of `mesh`, each naming which
    /// of `textures` it lands on.
    pub uvs: Option<UvMap>,
    /// The images those coordinates address, one per material that named one, in the order
    /// `uvs` indexes them.
    pub textures: Vec<Texture>,
}

impl Loaded {
    /// A file that carried geometry and nothing else, which is every STL.
    pub fn plain(mesh: Mesh) -> Self {
        Self {
            mesh,
            uvs: None,
            textures: Vec::new(),
        }
    }
}

/// An image a mesh's UVs address, exactly as the file stored it.
///
/// Undecoded on purpose: a loader has no business choosing an image decoder, and the
/// consumer knows which formats it can read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    /// Where it came from — a file name or a part path — for a message.
    pub name: String,
    pub bytes: Vec<u8>,
}

impl Texture {
    /// The image as one height per pixel, its brightness taken as how far the surface
    /// moves there: black stays put, white moves the whole way.
    ///
    /// PNG and JPEG are the two the 3MF specification allows; BMP and TGA are what an
    /// OBJ old enough to name them still travels with.
    pub fn decode(&self) -> Result<Heightmap, MeshIoError> {
        let image = image::load_from_memory(&self.bytes).map_err(|source| {
            MeshIoError::UndecodableTexture {
                name: self.name.clone(),
                reason: source.to_string(),
            }
        })?;

        let grey = image.to_luma8();
        let (width, height) = grey.dimensions();
        let samples = grey
            .pixels()
            .map(|pixel| Scalar::from(pixel.0[0]) / 255.0)
            .collect();

        Heightmap::new(width as usize, height as usize, samples).ok_or_else(|| {
            MeshIoError::UndecodableTexture {
                name: self.name.clone(),
                reason: "the image has no pixels".to_owned(),
            }
        })
    }
}
