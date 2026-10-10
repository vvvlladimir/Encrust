//! The contact shadow under the models: baked into a texture over the plate when what
//! stands on it changes, and laid on the plate every frame; see `docs/design/viewport.md`.

use std::hash::{DefaultHasher, Hash, Hasher};

use super::pipelines::{SHADOW_FORMAT, Shadowing};
use super::*;

/// Texels along each side of the shadow's texture. A plate is a few hundred millimetres
/// across, so a texel is under a millimetre, finer than the blur that softens it.
const SHADOW_TEXELS: u32 = 512;

/// The two textures the shadow is baked through, and what it was last baked from.
pub(super) struct ContactShadow {
    pipelines: Shadowing,
    silhouette: wgpu::TextureView,
    blurred: wgpu::TextureView,
    /// The silhouette as the blur reads it, and the blurred shadow as the plate reads it.
    silhouette_group: wgpu::BindGroup,
    blurred_group: wgpu::BindGroup,
    baked: Option<u64>,
    wanted: Option<u64>,
}

impl ContactShadow {
    pub(super) fn new(device: &wgpu::Device, pipelines: Shadowing) -> Self {
        let texture = |label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: SHADOW_TEXELS,
                        height: SHADOW_TEXELS,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: SHADOW_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let silhouette = texture("viewport_shadow_silhouette");
        let blurred = texture("viewport_shadow_blurred");
        let group = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("viewport_shadow"),
                layout: &pipelines.floor.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&pipelines.floor.sampler),
                    },
                ],
            })
        };
        Self {
            silhouette_group: group(&silhouette),
            blurred_group: group(&blurred),
            pipelines,
            silhouette,
            blurred,
            baked: None,
            wanted: None,
        }
    }

    /// Notes what this frame's shadow is cast by, or that there is none to cast.
    pub(super) fn want(&mut self, frame: &FrameInput<'_>) {
        self.wanted = frame.floor.map(|floor| cast_by(frame, floor));
    }

    /// Whether the plate is to be darkened this frame.
    pub(super) fn shows(&self) -> bool {
        self.wanted.is_some()
    }
}

/// What the shadow is cast by, as one number: the plate, and every model's mesh, faces
/// and placement. Hashing a placement costs the same for a model of millions of triangles
/// as for one of twelve, which is what lets the bake be skipped on a frame nothing moved.
fn cast_by(frame: &FrameInput<'_>, floor: Floor) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytemuck::bytes_of(&floor.area()).hash(&mut hasher);
    for draw in frame.models {
        (Arc::as_ptr(&draw.mesh) as usize).hash(&mut hasher);
        draw.faces.hash(&mut hasher);
        bytemuck::bytes_of(&draw.instance).hash(&mut hasher);
    }
    for draw in frame.reliefs {
        (Arc::as_ptr(&draw.mesh) as usize).hash(&mut hasher);
        bytemuck::bytes_of(&draw.instance).hash(&mut hasher);
    }
    hasher.finish()
}

impl ViewportResources {
    /// Bakes the shadow again if what casts it changed since it was last baked.
    pub fn bake_shadow(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let shadow = &self.shadow;
        if shadow.wanted.is_none() || shadow.wanted == shadow.baked {
            return;
        }
        {
            let mut pass = shadow_pass(encoder, &shadow.silhouette, "viewport_shadow_cast");
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
            self.cast(&mut pass);
        }
        {
            let mut pass = shadow_pass(encoder, &shadow.blurred, "viewport_shadow_blur");
            pass.set_pipeline(&shadow.pipelines.blur);
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_bind_group(1, &shadow.silhouette_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.shadow.baked = self.shadow.wanted;
    }

    /// Lays every model of the frame flat onto the silhouette.
    fn cast(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.shadow.pipelines.silhouette);
        for (index, draw) in self.frame.models.iter().enumerate() {
            let Some(mesh) = self.meshes.get(&draw.key) else {
                continue;
            };
            let instance = index as u32;
            for piece in &mesh.pieces {
                if piece.faces.start < draw.faces.start || piece.faces.end > draw.faces.end {
                    continue;
                }
                pass.set_vertex_buffer(0, piece.buffer.slice(..));
                pass.draw(0..piece.vertices, instance..instance + 1);
            }
        }
        pass.set_pipeline(&self.shadow.pipelines.silhouette_relief);
        for (index, (key, _)) in self.frame.reliefs.iter().enumerate() {
            let Some(relief) = self.reliefs.get(key) else {
                continue;
            };
            let instance = self.frame.relief_base + index as u32;
            for (buffer, count) in &relief.pieces {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, instance..instance + 1);
            }
        }
    }

    /// Fills the plate's surface, before anything that stands on it is drawn.
    pub(super) fn paint_plate(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        if !self.shadow.shows() {
            return;
        }
        render_pass.set_pipeline(&self.shadow.pipelines.plate);
        render_pass.draw(0..6, 0..1);
    }

    /// Darkens the plate under the models by the baked shadow.
    pub(super) fn paint_shadow(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        if !self.shadow.shows() {
            return;
        }
        render_pass.set_pipeline(&self.shadow.pipelines.floor.pipelines);
        render_pass.set_bind_group(1, &self.shadow.blurred_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

/// A pass over one of the shadow's textures, cleared to no shadow at all.
fn shadow_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    label: &'static str,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        multiview_mask: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

#[cfg(test)]
mod tests {
    use core_geometry::{Transform, Vec2, Vec3};

    use super::*;
    use crate::render::vertex::NOT_MARKED;

    fn floor() -> Floor {
        Floor {
            near_corner_mm: Vec2::ZERO,
            size_mm: Vec2::new(200.0, 120.0),
        }
    }

    fn placed(mesh: &Arc<Mesh>, at: Vec3) -> ModelDraw {
        let instance = ModelInstance::new(
            Transform::from_translation(at),
            theme::scene().object,
            NOT_MARKED,
        );
        ModelDraw::whole(Arc::clone(mesh), instance)
    }

    fn frame(models: &[ModelDraw]) -> FrameInput<'_> {
        FrameInput {
            view_projection: core_geometry::Mat4::IDENTITY,
            eye: Vec3::Z,
            section_mm: None,
            lines: &[],
            models,
            solids: &[],
            cut_surfaces: &[],
            cap: &[],
            label: &[],
            atlas: None,
            cuts: &[],
            pockets: &[],
            reliefs: &[],
            bands: &[],
            band_floor_mm: 0.0,
            volume_mm: None,
            cut_line: None,
            xray: false,
            floor: Some(floor()),
            grid: true,
        }
    }

    /// The bake is skipped on a frame nothing moved in, which is every frame of an orbit;
    /// moving a model or resizing the plate bakes it again.
    #[test]
    fn the_shadow_is_baked_again_only_when_what_casts_it_changes() {
        let mesh = Arc::new(Mesh::new(vec![Vec3::ZERO; 3], vec![[0, 1, 2]]));
        let still = [placed(&mesh, Vec3::ZERO)];
        let moved = [placed(&mesh, Vec3::X)];

        let first = cast_by(&frame(&still), floor());
        assert_eq!(first, cast_by(&frame(&still), floor()));
        assert_ne!(first, cast_by(&frame(&moved), floor()));
        let wider = Floor {
            size_mm: Vec2::new(300.0, 120.0),
            ..floor()
        };
        assert_ne!(first, cast_by(&frame(&still), wider));
    }
}
