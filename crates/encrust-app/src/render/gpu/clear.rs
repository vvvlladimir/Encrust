//! The clear-resin view: a pass ahead of the frame adds up the resin each ray crosses, and
//! after the rest is drawn the models refract a copy of it. See docs/design/viewport.md.

use egui::epaint::ViewportInPixels;

use super::*;

/// Half-floats blend everywhere, and thickness measured from the pivot stays small in them.
pub(super) const THICKNESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The pipelines of the clear view, and the planes it reads: the thickness, and a copy of
/// what stands behind the models.
pub(super) struct Clear {
    /// Adds each surface's signed distance into the plane; no depth, nothing culled.
    pub(super) thickness: Facing,
    /// Lays every face of the models into the scene faintly, inside walls included.
    pub(super) inner: Facing,
    /// Draws the models over the copy, their front faces only.
    pub(super) models: Facing,
    pub(super) layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The scene's colour format, which the copy is made in.
    format: wgpu::TextureFormat,
    plane: Option<Plane>,
}

struct Plane {
    size_px: [u32; 2],
    view: wgpu::TextureView,
    behind: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl Clear {
    pub(super) fn new(
        [thickness, inner, models]: [Facing; 3],
        layout: wgpu::BindGroupLayout,
        sampler: wgpu::Sampler,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            thickness,
            inner,
            models,
            layout,
            sampler,
            format,
            plane: None,
        }
    }

    /// Begins a pass over the plane cleared to nothing crossed, drawing into `viewport` of a
    /// window `size_px` pixels large. The plane is remade whenever the window changes size.
    pub(super) fn begin(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        size_px: [u32; 2],
        viewport: &ViewportInPixels,
    ) -> wgpu::RenderPass<'static> {
        if self
            .plane
            .as_ref()
            .is_some_and(|plane| plane.size_px != size_px)
        {
            self.plane = None;
        }
        let (layout, sampler, format) = (&self.layout, &self.sampler, self.format);
        let plane = self
            .plane
            .get_or_insert_with(|| plane(device, layout, sampler, format, size_px));
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
}

fn plane(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    format: wgpu::TextureFormat,
    size_px: [u32; 2],
) -> Plane {
    let texture = |label, format, usage| {
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
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let behind = texture("viewport_behind", format, wgpu::TextureUsages::COPY_DST);
    let behind_view = behind.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport_clear"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&behind_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    Plane {
        size_px,
        view,
        behind,
        bind_group,
    }
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
        if !self.frame.clear {
            return;
        }
        let mut pass = self.clear.begin(device, encoder, size_px, viewport);
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
        let drawn = &self.frame.models[..self.frame.markers_from];
        self.draw_meshes(&mut pass, &self.clear.thickness, drawn, 0);
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
