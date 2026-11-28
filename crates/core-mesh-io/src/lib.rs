//! Loading meshes from a file or its bytes into [`core_geometry::Mesh`].

mod error;
mod loaded;
mod loader;
mod obj;
mod picture;
mod stl;
mod three_mf;

pub use error::MeshIoError;
pub use loaded::{Loaded, Texture};
pub use loader::{MeshLoader, ModelFile, ReadSeek, loader_for_extension};
pub use obj::ObjLoader;
pub use picture::{MAX_PICTURE_PX, Picture, decode_picture};
pub use stl::StlLoader;
pub use three_mf::ThreeMfLoader;
