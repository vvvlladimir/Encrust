//! The render view's studio as its reflections see it: worked out per direction, then baked
//! once into an equirectangular map whose mips are prefiltered for rougher and rougher
//! surfaces, as Karis's split sum does (UE4, 2013). See docs/design/viewport.md.

use std::hash::{DefaultHasher, Hash, Hasher};

use super::*;

pub(super) const ENVIRONMENT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Texels of the sharpest mip, and how many mips, roughness 0.0 to 1.0 evenly between.
/// Must match `ENVIRONMENT_MIPS` in shader.wgsl.
const SIZE: [u32; 2] = [256, 128];
pub(super) const MIPS: u32 = 6;

/// The map, a stand-in for the passes that draw into it, and what it was baked under.
pub(super) struct Environment {
    pub(super) view: wgpu::TextureView,
    pub(super) stand_in: wgpu::TextureView,
    pub(super) sampler: wgpu::Sampler,
    mips: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
    prefilter: wgpu::RenderPipeline,
    baked: Option<u64>,
}

/// One mip's pass: the roughness it is prefiltered for and its size, texels.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Mip {
    roughness_and_size: [f32; 4],
}

fn texture(device: &wgpu::Device, size: [u32; 2], mips: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("viewport_environment"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: ENVIRONMENT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

impl Environment {
    pub(super) fn new(
        device: &wgpu::Device,
        prefilter: wgpu::RenderPipeline,
        mip_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let map = texture(device, SIZE, MIPS);
        let mips = (0..MIPS)
            .map(|mip| {
                let view = map.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    ..Default::default()
                });
                let size = [(SIZE[0] >> mip).max(1), (SIZE[1] >> mip).max(1)];
                let roughness = mip as f32 / (MIPS - 1) as f32;
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("viewport_environment_mip"),
                    contents: bytemuck::bytes_of(&Mip {
                        roughness_and_size: [roughness, size[0] as f32, size[1] as f32, 0.0],
                    }),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("viewport_environment_mip"),
                    layout: mip_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                });
                (view, group)
            })
            .collect();
        Self {
            view: map.create_view(&wgpu::TextureViewDescriptor::default()),
            stand_in: texture(device, [1, 1], 1)
                .create_view(&wgpu::TextureViewDescriptor::default()),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("viewport_environment"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            }),
            mips,
            prefilter,
            baked: None,
        }
    }
}

/// What the studio is made of, as one number: the lights' colours and directions.
fn studio_of(light: &theme::Lighting) -> u64 {
    let mut hasher = DefaultHasher::new();
    for colour in [light.sky, light.ground, light.key, light.fill] {
        colour.to_array().hash(&mut hasher);
    }
    for towards in [light.key_towards, light.fill_towards] {
        towards.map(f32::to_bits).hash(&mut hasher);
    }
    hasher.finish()
}

impl ViewportResources {
    /// Bakes the studio's map again if the render view is on and its lights changed since.
    pub fn bake_environment(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if !self.frame.staged {
            return;
        }
        let studio = studio_of(&theme::scene().light);
        if self.environment.baked == Some(studio) {
            return;
        }
        for (view, mip) in &self.environment.mips {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport_environment"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.environment.prefilter);
            pass.set_bind_group(0, &self.casting_bind_group, &[]);
            pass.set_bind_group(1, mip, &[]);
            pass.draw(0..3, 0..1);
        }
        self.environment.baked = Some(studio);
    }
}
