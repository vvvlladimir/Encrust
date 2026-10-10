use bytemuck::{Pod, Zeroable};
use core_geometry::{Mat3, Mat4, Mesh, Transform, UvMap, Vec2, Vec3};
use egui::Color32;

use crate::ui::theme;

/// One corner of one triangle, carrying its face normal.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl ModelVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// Expands `faces` of an indexed mesh into a flat-shaded triangle list.
///
/// Vertices are not shared between faces: a sliceable model is a hard-surfaced object and
/// per-face normals show its facets honestly, including the ones that should not be there.
/// A range rather than the whole mesh, because a cavity runs to millions of triangles and
/// goes to the card in pieces; see `docs/decisions/0070-a-mesh-is-drawn-in-buffer-sized-pieces.md`.
pub fn flat_shaded_vertices(mesh: &Mesh, faces: std::ops::Range<usize>) -> Vec<ModelVertex> {
    let mut vertices = Vec::with_capacity(faces.len() * 3);
    for triangle in faces.filter_map(|face| mesh.triangle(face)) {
        // A degenerate face has no normal. It is kept so face indices still line up with
        // what the diagnostics reported, and shaded as if it faced up.
        let normal = normalized_or_up(triangle.normal_unnormalized()).to_array();
        for position in [triangle.a, triangle.b, triangle.c] {
            vertices.push(ModelVertex {
                position: position.to_array(),
                normal,
            });
        }
    }
    vertices
}

/// One corner of one triangle of a textured model: its face normal, where it lands on the
/// texture, and which of the model's images that is.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ReliefVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    /// Layer of the texture array, or a negative number on a face the map does not cover.
    layer: f32,
}

impl ReliefVertex {
    // Past the instance's own locations, which run from 2 to 10 and are shared with the
    // flat pass.
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 11 => Float32x2, 12 => Float32];

    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// The same flat-shaded triangles as [`flat_shaded_vertices`], carrying the texture
/// coordinates of each corner so the viewport can show what a relief would press in.
pub fn textured_vertices(
    mesh: &Mesh,
    uvs: &UvMap,
    faces: std::ops::Range<usize>,
) -> Vec<ReliefVertex> {
    let mut vertices = Vec::with_capacity(faces.len() * 3);
    for face in faces {
        let Some(triangle) = mesh.triangle(face) else {
            continue;
        };
        let normal = normalized_or_up(triangle.normal_unnormalized()).to_array();
        let mapping = uvs.of_face(face);
        let layer = mapping.map_or(-1.0, |mapping| mapping.texture as f32);
        let corners = mapping.map_or([Vec2::ZERO; 3], |mapping| mapping.corners);
        for (position, uv) in [triangle.a, triangle.b, triangle.c]
            .into_iter()
            .zip(corners)
        {
            vertices.push(ReliefVertex {
                position: position.to_array(),
                normal,
                uv: uv.to_array(),
                layer,
            });
        }
    }
    vertices
}

fn normalized_or_up(normal: Vec3) -> Vec3 {
    let normalized = normal.normalize_or_zero();
    if normalized == Vec3::ZERO {
        Vec3::Z
    } else {
        normalized
    }
}

/// Placement, colour, overhang marking and x-ray behaviour of one object, read once per
/// instance.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ModelInstance {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
    color: [f32; 4],
    overhang: f32,
    /// 1.0 for a surface of a model, 0.0 for a volume painted in its own colour: the
    /// space inside a model that fills with resin is not a surface, so no light shades
    /// it, no inside wash lightens its far wall, and the x-ray does not wash it down.
    surface: f32,
}

/// What [`ModelInstance::new`] is given for a mesh that is not marked: no lean can be
/// past it, because no surface leans less than nothing.
pub const NOT_MARKED: f32 = -1.0;

impl ModelInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 10] = wgpu::vertex_attr_array![
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x3,
        7 => Float32x3,
        8 => Float32x3,
        9 => Float32x4,
        10 => Float32,
        13 => Float32,
    ];

    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }

    /// Takes a token rather than floats so that no colour can be spelled in the renderer.
    /// `overhang` is the sine of the lean a surface may have before it is washed with the
    /// overhang colour, or [`NOT_MARKED`].
    pub fn new(transform: Transform, color: Color32, overhang: f32) -> Self {
        let model = transform.to_matrix();
        Self {
            model: model.to_cols_array_2d(),
            normal: normal_matrix(&model).to_cols_array_2d(),
            color: theme::gamma(color),
            overhang,
            surface: 1.0,
        }
    }

    /// The same instance painted as a volume rather than as a surface: it keeps its own
    /// colour, unshaded and unwashed, so the space it bounds reads through whatever
    /// stands in front of it.
    pub fn as_volume(mut self) -> Self {
        self.surface = 0.0;
        self
    }

    /// Whether the placement mirrors the model, which turns its winding on screen.
    pub fn is_mirrored(&self) -> bool {
        Mat3::from_mat4(Mat4::from_cols_array_2d(&self.model)).determinant() < 0.0
    }
}

/// Inverse transpose of the upper 3x3, which keeps normals perpendicular to the surface
/// under a non-uniform scale. A scale with a zero axis is not invertible; there is no
/// meaningful normal for a flattened object, so its untransformed normals are used.
fn normal_matrix(model: &core_geometry::Mat4) -> Mat3 {
    let linear = Mat3::from_mat4(*model);
    if linear.determinant().abs() < 1e-12 {
        return Mat3::IDENTITY;
    }
    linear.inverse().transpose()
}

/// One corner of the word lying in front of the plate: a place in plate millimetres and a
/// place in the font atlas.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LabelVertex {
    pub position: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
}

impl LabelVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4];

    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    /// Takes a token rather than floats so that no colour can be spelled in the renderer.
    pub fn new(position: Vec3, uv: [f32; 2], color: Color32) -> Self {
        Self {
            position: position.to_array(),
            uv,
            color: theme::gamma(color),
        }
    }
}

/// One end of one line of the plate, the grid or the build volume outline.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    color: [f32; 4],
}

impl LineVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    pub fn new(position: Vec3, color: Color32) -> Self {
        Self {
            position: position.to_array(),
            color: theme::gamma(color),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Quat, Vec3};

    fn one_triangle_facing_up() -> Mesh {
        Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]])
    }

    #[test]
    fn every_face_becomes_three_vertices() {
        let mesh = one_triangle_facing_up();
        let vertices = flat_shaded_vertices(&mesh, 0..mesh.faces.len());
        assert_eq!(vertices.len(), 3);
        for vertex in vertices {
            assert_eq!(vertex.normal, [0.0, 0.0, 1.0]);
        }
    }

    #[test]
    fn a_degenerate_face_still_produces_vertices() {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::X], vec![[0, 1, 2]]);
        assert_eq!(flat_shaded_vertices(&mesh, 0..mesh.faces.len()).len(), 3);
    }

    #[test]
    fn an_empty_mesh_produces_no_vertices() {
        assert!(flat_shaded_vertices(&Mesh::default(), 0..0).is_empty());
    }

    #[test]
    fn a_non_uniform_scale_keeps_normals_perpendicular() {
        let transform = Transform {
            translation: Vec3::new(3.0, 0.0, 0.0),
            rotation: Quat::from_rotation_z(0.7),
            scale: Vec3::new(1.0, 1.0, 4.0),
        };
        let model = transform.to_matrix();
        let normals = normal_matrix(&model);

        // A 45 degree slope in the XZ plane: stretching Z by four steepens the surface,
        // so the transformed normal must tilt the other way to stay perpendicular.
        let along = Vec3::new(1.0, 0.0, 1.0).normalize();
        let normal = Vec3::new(-1.0, 0.0, 1.0).normalize();
        let moved_along = Mat3::from_mat4(model) * along;
        let moved_normal = normals * normal;
        assert!(moved_along.dot(moved_normal).abs() < 1e-5);
    }

    #[test]
    fn a_flattened_object_falls_back_to_its_own_normals() {
        let flat = Transform {
            scale: Vec3::new(1.0, 1.0, 0.0),
            ..Transform::default()
        };
        assert_eq!(normal_matrix(&flat.to_matrix()), Mat3::IDENTITY);
    }

    #[test]
    fn one_flipped_axis_mirrors_and_two_turn_the_model_round() {
        let scaled = |scale| {
            let transform = Transform {
                scale,
                ..Transform::default()
            };
            ModelInstance::new(transform, Color32::WHITE, NOT_MARKED).is_mirrored()
        };
        assert!(scaled(Vec3::new(-1.0, 1.0, 1.0)), "one flip is a mirror");
        assert!(
            !scaled(Vec3::new(-1.0, -1.0, 1.0)),
            "two flips are a half turn about the third axis"
        );
        assert!(
            !scaled(Vec3::new(2.0, 1.0, 0.5)),
            "a stretch mirrors nothing"
        );
    }

    #[test]
    fn the_instance_stride_has_no_padding() {
        assert_eq!(
            size_of::<ModelInstance>(),
            (16 + 9 + 4 + 2) * size_of::<f32>()
        );
    }
}
