use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use core_geometry::{Mat3, Mat4, Mesh, Transform, UvMap, Vec2, Vec3};
use egui::Color32;

use crate::ui::theme;

mod smooth;
pub use smooth::smooth_vertices;

/// One corner of a mesh, shared by the faces that meet at it, carrying the normal of the
/// one face it leads; see `docs/design/viewport.md`.
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

/// `faces` of a mesh as its corners and the triangles over them, each turned to start on
/// the corner carrying its normal. A range, since a cavity goes to the card in pieces (ADR 0070).
pub fn indexed_vertices(mesh: &Mesh, faces: Range<usize>) -> (Vec<ModelVertex>, Vec<u32>) {
    let mut corners = Corners {
        local: vec![u32::MAX; mesh.vertices.len()],
        vertices: Vec::new(),
        leading: Vec::new(),
    };
    let mut indices = Vec::with_capacity(faces.len() * 3);
    for face in faces {
        let Some(triangle) = mesh.triangle(face) else {
            continue;
        };
        // A degenerate face has no normal. It is kept so face indices still line up with
        // what the diagnostics reported, and shaded as if it faced up.
        let normal = normalized_or_up(triangle.normal_unnormalized()).to_array();
        let at = mesh.faces[face].map(|corner| corners.of(mesh, corner));
        let first = match (0..3).find(|&turn| !corners.leading[at[turn] as usize]) {
            Some(turn) => {
                corners.vertices[at[turn] as usize].normal = normal;
                [at[turn], at[(turn + 1) % 3], at[(turn + 2) % 3]]
            }
            None => [corners.copy_of(at[0]), at[1], at[2]],
        };
        corners.leading[first[0] as usize] = true;
        corners.vertices[first[0] as usize].normal = normal;
        indices.extend(first);
    }
    (corners.vertices, indices)
}

/// Turns every face one step round so the corner it leads with comes last, for a card
/// that reads a flat input off a face's last corner, as GL does.
pub fn lead_last(indices: &mut [u32]) {
    for face in indices.as_chunks_mut::<3>().0 {
        face.rotate_left(1);
    }
}

/// The corners of one range laid out so far, and which of them already lead a face.
struct Corners {
    local: Vec<u32>,
    vertices: Vec<ModelVertex>,
    leading: Vec<bool>,
}

impl Corners {
    /// The shared corner standing for mesh vertex `corner`, laid out on first use.
    fn of(&mut self, mesh: &Mesh, corner: u32) -> u32 {
        let at = &mut self.local[corner as usize];
        if *at == u32::MAX {
            *at = self.vertices.len() as u32;
            self.vertices.push(ModelVertex {
                position: mesh.vertices[corner as usize].to_array(),
                normal: [0.0; 3],
            });
            self.leading.push(false);
        }
        *at
    }

    /// A second corner where `corner` stands, for a face whose every corner already leads.
    fn copy_of(&mut self, corner: u32) -> u32 {
        self.vertices.push(self.vertices[corner as usize]);
        self.leading.push(false);
        (self.vertices.len() - 1) as u32
    }
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

/// The triangles of a textured model, one corner of each a vertex, carrying the texture
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
    /// Share of the way to the eye the instance is drawn pulled, depth only: what wins
    /// between two coplanar faces of different meshes.
    pull: f32,
    /// 1.0 where the mesh goes to the card with normals smoothed over its corners and is
    /// shaded by them, 0.0 for flat faces.
    smooth: f32,
}

/// What [`ModelInstance::new`] is given for a mesh that is not marked: no lean can be
/// past it, because no surface leans less than nothing.
pub const NOT_MARKED: f32 = -1.0;

impl ModelInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 12] = wgpu::vertex_attr_array![
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
        14 => Float32,
        15 => Float32,
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
            pull: 0.0,
            smooth: 0.0,
        }
    }

    /// The same instance shaded smooth over its mesh's corners, which then goes to the card
    /// with [`smooth_vertices`]: a tube of few sides reads round.
    pub fn smoothed(mut self) -> Self {
        self.smooth = 1.0;
        self
    }

    /// The same instance drawn `share` of the way nearer the eye in depth, and nowhere
    /// else: a face level with another mesh's then wins it every frame instead of by turns.
    pub fn pulled(mut self, share: f32) -> Self {
        self.pull = share;
        self
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

/// One line drawn wider than the card's own one pixel: a quad the shader lays across the
/// screen between its two ends.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WideLine {
    from: [f32; 3],
    to: [f32; 3],
    color: [f32; 4],
    /// How far each side of the line its border stands, in clip units across and up.
    half_width: [f32; 2],
}

impl WideLine {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x2
    ];

    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }

    /// The line from `ends[0]` to `ends[1]`, `width` across on a view `size` large, both
    /// in the same unit.
    pub fn new(ends: [Vec3; 2], color: Color32, width: f32, size: Vec2) -> Self {
        Self {
            from: ends[0].to_array(),
            to: ends[1].to_array(),
            color: theme::gamma(color),
            half_width: (width / size.max(Vec2::ONE)).to_array(),
        }
    }
}

#[cfg(test)]
mod tests;
