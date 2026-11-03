//! One model part of a 3MF package, kept to what the walk into a mesh reads.

use core_geometry::{FastMap, Mat4, Vec2, Vec3};

/// The objects, build and texture groups of one model part.
pub(super) struct Model {
    /// Millimetres per unit of the part's coordinates.
    pub mm_per_unit: f32,
    pub objects: FastMap<u32, Object>,
    pub build: Vec<Placed>,
    /// Each `texture2d` id and the part name of its image.
    pub textures: FastMap<u32, String>,
    pub groups: FastMap<u32, TextureGroup>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            mm_per_unit: 1.0,
            objects: FastMap::default(),
            build: Vec::new(),
            textures: FastMap::default(),
            groups: FastMap::default(),
        }
    }
}

/// An object resource and the property its triangles default to.
#[derive(Default)]
pub(super) struct Object {
    pub pid: Option<u32>,
    pub pindex: Option<u32>,
    pub shape: Shape,
}

/// What an object is made of.
#[derive(Default)]
pub(super) enum Shape {
    #[default]
    Empty,
    Mesh(MeshPart),
    Components(Vec<Placed>),
    /// A kind the walk refuses rather than drops, named as `MeshIoError::Unimplemented` takes it.
    Unsupported(&'static str),
}

#[derive(Default)]
pub(super) struct MeshPart {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<Triangle>,
}

pub(super) struct Triangle {
    pub vertices: [u32; 3],
    pub pid: Option<u32>,
    /// `p1`, `p2` and `p3`: indices into the property group, each optional.
    pub properties: [Option<u32>; 3],
}

/// A build item or a component: the object it places, where, and the part holding it.
pub(super) struct Placed {
    pub objectid: u32,
    pub transform: Mat4,
    /// The production extension's `p:path`, the part name of another model part.
    pub path: Option<String>,
}

/// A `texture2dgroup`: the image it samples and its coordinates.
pub(super) struct TextureGroup {
    pub texture: u32,
    pub coordinates: Vec<Vec2>,
}
