//! The print round the render view's can, drawn off the drawing thread when the window opens
//! and uploaded the first time the render view is; a dark texel stands in until then.

use std::sync::{Arc, OnceLock};

use crate::render::references::CanPrint;

use super::*;

pub(super) struct CanPrintTexture {
    pub(super) view: wgpu::TextureView,
    pub(super) sampler: wgpu::Sampler,
    /// The print being drawn, until it is uploaded.
    drawing: Option<Arc<OnceLock<CanPrint>>>,
}

impl CanPrintTexture {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let stand_in = texture(device, [1, 1], 1);
        let drawing = Arc::new(OnceLock::new());
        let drawn = Arc::clone(&drawing);
        crate::job::spawn(move || match CanPrint::new() {
            Ok(print) => {
                let _ = drawn.set(print);
            }
            Err(error) => tracing::warn!(%error, "the can's print cannot be drawn"),
        });
        Self {
            view: stand_in.create_view(&wgpu::TextureViewDescriptor::default()),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("viewport_can_print"),
                // Round the wall it repeats; up it, it stops at the print's edges.
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            }),
            drawing: Some(drawing),
        }
    }
}

fn texture(device: &wgpu::Device, [width, height]: [u32; 2], levels: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("viewport_can_print"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Read encoded, as every colour `finish` takes is: it linearises them itself.
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, print: &CanPrint) -> wgpu::TextureView {
    let texture = texture(device, print.size, print.levels.len() as u32);
    let [mut width, mut height] = print.size;
    for (level, texels) in print.levels.iter().enumerate() {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        (width, height) = ((width / 2).max(1), (height / 2).max(1));
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

impl ViewportResources {
    /// Binds the can's print once the render view stands the props and it is drawn.
    pub(super) fn want_can_print(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &FrameInput<'_>,
    ) {
        let Some(print) = self
            .can_print
            .drawing
            .as_ref()
            .filter(|_| frame.look.is_some())
            .and_then(|drawing| drawing.get())
        else {
            return;
        };
        self.can_print.view = upload(device, queue, print);
        self.can_print.drawing = None;
        self.rebind_globals(device);
    }
}
