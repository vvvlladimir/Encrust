//! The clear-resin view: a pass ahead of the frame adds up the resin each ray crosses, and
//! after the rest is drawn the models refract a copy of it. See docs/design/viewport.md.

use egui::epaint::ViewportInPixels;

use super::*;

/// Half-floats blend everywhere, and thickness measured from the pivot stays small in them.
pub(super) const THICKNESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
/// Where the light inside a clear model first leaves it: the outward normal of the nearest
/// back face, and its distance from the eye less the pivot's.
pub(super) const EXIT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The pipelines of the clear view, and the planes it reads: the thickness, and a copy of
/// what stands behind the models.
pub(super) struct Clear {
    /// Adds each surface's signed distance into the plane; no depth, nothing culled.
    pub(super) thickness: Facing,
    /// Keeps the nearest back face of the models in the exit plane.
    pub(super) exit: Facing,
    /// Lays every face of the models into the scene faintly, inside walls included.
    pub(super) inner: Facing,
    /// Draws the models over the copy, their front faces only.
    pub(super) models: Facing,
    /// Draws an opaque resin's models against the thickness, which a thin part glows by.
    pub(super) glowing: Shaded,
    pub(super) layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The scene's colour format, which the copy is made in.
    format: wgpu::TextureFormat,
    plane: Option<Plane>,
}

struct Plane {
    size_px: [u32; 2],
    /// Whether the exit and the copy behind are the window's size; for an opaque resin, which
    /// never reads them, they are a texel each.
    clear: bool,
    view: wgpu::TextureView,
    exit: wgpu::TextureView,
    exit_depth: wgpu::TextureView,
    behind: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl Clear {
    pub(super) fn new(
        [thickness, exit, inner, models]: [Facing; 4],
        glowing: Shaded,
        layout: wgpu::BindGroupLayout,
        sampler: wgpu::Sampler,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            thickness,
            exit,
            inner,
            models,
            glowing,
            layout,
            sampler,
            format,
            plane: None,
        }
    }

    /// Begins a pass over the plane cleared to nothing crossed, drawing into `viewport` of a
    /// window `size_px` pixels large, for a `clear` resin or an opaque one. The plane is remade
    /// whenever either changes.
    pub(super) fn begin(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        (size_px, clear): ([u32; 2], bool),
        viewport: &ViewportInPixels,
    ) -> wgpu::RenderPass<'static> {
        if self
            .plane
            .as_ref()
            .is_some_and(|plane| plane.size_px != size_px || plane.clear != clear)
        {
            self.plane = None;
        }
        let (layout, sampler, format) = (&self.layout, &self.sampler, self.format);
        let plane = self
            .plane
            .get_or_insert_with(|| plane(device, (layout, sampler), format, (size_px, clear)));
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport_thickness"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &plane.view,
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
            .forget_lifetime();
        pass.set_viewport(
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
            0.0,
            1.0,
        );
        pass
    }

    /// What the models read the planes through, once a pass has made them.
    pub(super) fn planes(&self) -> Option<&wgpu::BindGroup> {
        self.plane.as_ref().map(|plane| &plane.bind_group)
    }

    /// Begins a pass over the exit plane, cleared to no exit, the plane `begin` made.
    pub(super) fn begin_exit(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        viewport: &ViewportInPixels,
    ) -> Option<wgpu::RenderPass<'static>> {
        let plane = self.plane.as_ref()?;
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport_exit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &plane.exit,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &plane.exit_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        pass.set_viewport(
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
            0.0,
            1.0,
        );
        Some(pass)
    }
}

fn plane(
    device: &wgpu::Device,
    (layout, sampler): (&wgpu::BindGroupLayout, &wgpu::Sampler),
    format: wgpu::TextureFormat,
    (size_px, clear): ([u32; 2], bool),
) -> Plane {
    let seen_through_px = if clear { size_px } else { [1, 1] };
    let texture = |label, format, usage, size_px: [u32; 2]| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size_px[0].max(1),
                height: size_px[1].max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | usage,
            view_formats: &[],
        })
    };
    let view = texture(
        "viewport_thickness",
        THICKNESS_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        size_px,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let exit = texture(
        "viewport_exit",
        EXIT_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        seen_through_px,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let exit_depth = texture(
        "viewport_exit_depth",
        DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        seen_through_px,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let behind = texture(
        "viewport_behind",
        format,
        wgpu::TextureUsages::COPY_DST,
        seen_through_px,
    );
    let behind_view = behind.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = reading(device, (layout, sampler), [&view, &behind_view, &exit]);
    Plane {
        size_px,
        clear,
        view,
        exit,
        exit_depth,
        behind,
        bind_group,
    }
}

/// What the models read the thickness, the copy behind and the exit through.
fn reading(
    device: &wgpu::Device,
    (layout, sampler): (&wgpu::BindGroupLayout, &wgpu::Sampler),
    [thickness, behind, exit]: [&wgpu::TextureView; 3],
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport_clear"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(thickness),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(behind),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(exit),
            },
        ],
    })
}

impl ViewportResources {
    /// Adds up the resin each pixel's ray crosses, ahead of the frame, when the clear view
    /// is on; see `Clear`.
    pub fn measure_thickness(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        size_px: [u32; 2],
        viewport: &ViewportInPixels,
    ) {
        if !self.frame.thick {
            // At 4K the planes are some 150 MB, which only the render view reads.
            if !self.frame.staged {
                self.clear.plane = None;
            }
            return;
        }
        let mut pass = self
            .clear
            .begin(device, encoder, (size_px, self.frame.clear), viewport);
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
        let drawn = &self.frame.models[..self.frame.markers_from];
        self.draw_meshes(&mut pass, &self.clear.thickness, drawn, 0);
        drop(pass);
        if !self.frame.clear {
            return;
        }
        let Some(mut pass) = self.clear.begin_exit(encoder, viewport) else {
            return;
        };
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
        self.draw_meshes(&mut pass, &self.clear.exit, drawn, 0);
    }

    /// Copies what the scene pass drew out of the colour plane, and draws the models over
    /// it, each refracting what stands behind it.
    pub(super) fn paint_clear(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        viewport: &ViewportInPixels,
    ) {
        let (Some(colour), Some(plane)) = (self.target.colour(), self.clear.plane.as_ref()) else {
            return;
        };
        encoder.copy_texture_to_texture(
            colour.as_image_copy(),
            plane.behind.as_image_copy(),
            colour.size(),
        );
        let Some(mut pass) = self.target.resume(encoder, viewport) else {
            return;
        };
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        pass.set_bind_group(1, &plane.bind_group, &[]);
        pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
        let drawn = &self.frame.models[..self.frame.markers_from];
        self.draw_meshes(&mut pass, &self.clear.models, drawn, 0);
    }
}

#[cfg(test)]
impl ViewportResources {
    /// Whether it holds a plane only the render view draws: the resin's or the averaged one.
    pub fn holds_render_planes(&self) -> bool {
        self.clear.plane.is_some() || self.target.holds_accumulated()
    }
}
