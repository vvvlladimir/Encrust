//! Loading meshes from disk into [`core_geometry::Mesh`].

mod error;
mod loaded;
mod loader;
mod obj;
mod stl;
mod three_mf;

pub use error::MeshIoError;
pub use loaded::{Loaded, Texture};
pub use loader::{MeshLoader, loader_for_extension};
pub use obj::ObjLoader;
pub use stl::StlLoader;
pub use three_mf::ThreeMfLoader;
