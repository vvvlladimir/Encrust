//! The clear-resin view's pass ahead of the frame: how much resin each pixel's ray crosses,
//! added up into a plane the models are then drawn against. See docs/design/viewport.md.

use egui::epaint::ViewportInPixels;

use super::*;

/// Half-floats blend everywhere, and thickness measured from the pivot stays small in them.
pub(super) const THICKNESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The pipelines of the clear view, and the plane its first pass adds the thickness into.
pub(super) struct Clear {
    /// Adds each surface's signed distance into the plane; no depth, nothing culled.
    pub(super) thickness: Facing,
    /// Draws the models against the plane, their front faces only.
    pub(super) models: Facing,
    pub(super) layout: wgpu::BindGroupLayout,
    plane: Option<Plane>,
}

struct Plane {
    size_px: [u32; 2],
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl Clear {
    pub(super) fn new(thickness: Facing, models: Facing, layout: wgpu::BindGroupLayout) -> Self {
        Self {
            thickness,
            models,
            layout,
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
        let layout = &self.layout;
        let plane = self
            .plane
            .get_or_insert_with(|| plane(device, layout, size_px));
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

    /// What the clear models read the plane through, once a pass has made it.
    pub(super) fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.plane.as_ref().map(|plane| &plane.bind_group)
    }
}

fn plane(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, size_px: [u32; 2]) -> Plane {
    let view = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport_thickness"),
            size: wgpu::Extent3d {
                width: size_px[0].max(1),
                height: size_px[1].max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: THICKNESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport_thickness"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }],
    });
    Plane {
        size_px,
        view,
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
}
