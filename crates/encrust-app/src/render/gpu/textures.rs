//! The two textures the viewport samples: the font atlas the machine's word is drawn
//! from, and the image the Relief tool presses into a model (ADR 0116).

use std::sync::Arc;

use core_geometry::Scalar;

use crate::render::vertex::{ReliefVertex, textured_vertices};

use super::buffers::upload;
use super::*;

/// How many pixels a side each image is resampled to before it goes to the card.
///
/// The layers of an array texture are all one size, and the models carry images that are
/// not. Five hundred is past what a viewport shows of a texture on a moving model, and
/// four images cost a megabyte between them.
const RELIEF_SIDE: u32 = 512;

/// The model's images as the layers of one array texture, each resampled to
/// [`RELIEF_SIDE`] square.
pub(super) fn relief_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    heights: &[core_geometry::Heightmap],
) -> wgpu::TextureView {
    let layers = heights.len().max(1) as u32;
    let size = wgpu::Extent3d {
        width: RELIEF_SIDE,
        height: RELIEF_SIDE,
        depth_or_array_layers: layers,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("viewport_relief"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (layer, map) in heights.iter().enumerate() {
        let mut pixels = Vec::with_capacity((RELIEF_SIDE * RELIEF_SIDE) as usize);
        for row in 0..RELIEF_SIDE {
            for column in 0..RELIEF_SIDE {
                // Sampled through the map's own reader, so what the viewport shows and
                // what `press` reads are the same image the same way up.
                let u = (column as Scalar + 0.5) / RELIEF_SIDE as Scalar;
                let v = 1.0 - (row as Scalar + 0.5) / RELIEF_SIDE as Scalar;
                pixels.push((map.sample(core_geometry::Vec2::new(u, v)) * 255.0) as u8);
            }
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(RELIEF_SIDE),
                rows_per_image: Some(RELIEF_SIDE),
            },
            wgpu::Extent3d {
                width: RELIEF_SIDE,
                height: RELIEF_SIDE,
                depth_or_array_layers: 1,
            },
        );
    }

    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

impl ViewportResources {
    /// Uploads the font atlas the first time it is seen. It is kept by its address, the
    /// same way a mesh is: the window lays the word out once and hands the same image over
    /// every frame after that.
    pub(super) fn upload_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &Arc<egui::ColorImage>,
    ) {
        let key = Arc::as_ptr(atlas) as usize;
        if self.atlas.as_ref().is_some_and(|(seen, _)| *seen == key) {
            return;
        }
        let [width, height] = atlas.size;
        let size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport_label_atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            bytemuck::cast_slice(&atlas.pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width as u32),
                rows_per_image: Some(height as u32),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport_label_atlas"),
            layout: &self.label_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.label_sampler),
                },
            ],
        });
        self.atlas = Some((key, bind_group));
    }

    /// Uploads a textured model the first time it is seen and returns its cache key.
    ///
    /// Keyed by the map rather than the mesh: pressing a relief replaces both, and the
    /// coordinates are what the buffers carry that the flat ones do not.
    pub(super) fn cache_relief(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw: &ReliefDraw,
    ) -> usize {
        let key = Arc::as_ptr(&draw.mapped) as usize;
        if self.reliefs.contains_key(&key) {
            return key;
        }

        let ceiling = device.limits().max_buffer_size as usize / size_of::<ReliefVertex>();
        let faces_per_piece = (ceiling / 3).max(1);
        let pieces = (0..draw.mesh.faces.len())
            .step_by(faces_per_piece)
            .filter_map(|first| {
                let last = (first + faces_per_piece).min(draw.mesh.faces.len());
                let vertices = textured_vertices(&draw.mesh, &draw.mapped.uvs, first..last);
                (!vertices.is_empty()).then(|| {
                    (
                        upload(device, "viewport_relief", &vertices),
                        vertices.len() as u32,
                    )
                })
            })
            .collect();

        let view = relief_texture(device, queue, &draw.mapped.heights);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport_relief"),
            layout: &self.relief_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.relief_sampler),
                },
            ],
        });

        self.reliefs.insert(
            key,
            CachedRelief {
                _mapped: Arc::clone(&draw.mapped),
                pieces,
                bind_group,
            },
        );
        key
    }
}
