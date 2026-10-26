use super::SIZE;
use super::marks::Marks;
use crate::camera::OrbitCamera;
use crate::render::gpu::{DEPTH_FORMAT, FrameInput, ModelDraw, ViewportResources};
use crate::render::vertex::LineVertex;
use crate::ui::theme;
use core_geometry::{Mesh, Scalar, Vec3};
use std::sync::OnceLock;

pub(super) fn differing(a: &[u8], b: &[u8]) -> usize {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count()
}

/// One frame as the window would paint it, read back, or `None` with no adapter to paint
/// it with.
pub(super) fn paint(
    camera: OrbitCamera,
    section_mm: Option<Scalar>,
    models: &[ModelDraw],
    solids: &[ModelDraw],
    marks: Marks<'_>,
) -> Option<Vec<u8>> {
    paint_after(camera, section_mm, &[], models, solids, marks)
}

/// The same, with each frame of `before` prepared against the same resources first and
/// thrown away. The window draws a shell for many frames before the drainage check comes
/// back and says what is trapped in it, so what the card holds for a mesh has to answer a
/// later frame asking for something the first one did not.
pub(super) fn paint_after<'a>(
    camera: OrbitCamera,
    section_mm: Option<Scalar>,
    before: &[&'a [ModelDraw]],
    models: &'a [ModelDraw],
    solids: &'a [ModelDraw],
    marks: Marks<'a>,
) -> Option<Vec<u8>> {
    let Marks {
        markers,
        cuts,
        cut_surfaces,
        pockets,
        bands,
        band_floor_mm,
        volume_mm,
        cut_line,
        xray,
        floor,
        lines,
        wide_lines,
    } = marks;
    let (device, queue) = gpu()?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut resources = ViewportResources::new(device, format);
    let cap = cap_quad(section_mm.unwrap_or_default());
    let input = |models: &'a [ModelDraw]| FrameInput {
        reliefs: &[],
        view_projection: camera.view_projection(1.0),
        eye: camera.eye(),
        section_mm,
        lines,
        wide_lines,
        models,
        markers,
        solids,
        cap: &cap,
        bottom_cut_mm: None,
        bottom_cap: &[],
        label: &[],
        atlas: None,
        cuts,
        cut_surfaces,
        pockets,
        bands,
        band_floor_mm,
        volume_mm,
        cut_line,
        xray,
        floor,
        grid: false,
        light: false,
    };
    for earlier in before {
        resources.prepare(device, queue, input(earlier));
    }
    resources.prepare(device, queue, input(models));

    Some(read_back(device, queue, format, &mut resources))
}

/// Paints what `resources` recorded into an offscreen target and reads the pixels back.
pub(super) fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    resources: &mut ViewportResources,
) -> Vec<u8> {
    let (colour, depth) = (
        target(device, format, wgpu::TextureUsages::COPY_SRC),
        target(device, DEPTH_FORMAT, wgpu::TextureUsages::empty()),
    );
    let colour_view = colour.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    resources.bake_shadow(&mut encoder);
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &colour_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            multiview_mask: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        resources.paint(&mut pass.forget_lifetime());
    }

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(SIZE * SIZE * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        colour.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: Some(SIZE),
            },
        },
        extent(),
    );
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("the readback maps");
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let pixels = slice.get_mapped_range().expect("the readback is mapped");
    pixels.to_vec()
}

/// The one device the frames of this file are drawn on, or `None` on a machine with no
/// adapter to draw them with.
pub(super) fn gpu() -> Option<&'static (wgpu::Device, wgpu::Queue)> {
    pub(super) static GPU: OnceLock<Option<(wgpu::Device, wgpu::Queue)>> = OnceLock::new();
    GPU.get_or_init(|| {
        let instance = wgpu::Instance::default();
        let adapter =
            block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        Some(
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device"),
        )
    })
    .as_ref()
}

pub(super) fn target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: extent(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | usage,
        view_formats: &[],
    })
}

pub(super) fn extent() -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: SIZE,
        height: SIZE,
        depth_or_array_layers: 1,
    }
}

/// Two triangles in the cutting plane, wide enough to reach past the model.
pub(super) fn cap_quad(height_mm: Scalar) -> Vec<LineVertex> {
    let colour = theme::scene().section_cap;
    let corner = |x: f32, y: f32| LineVertex::new(Vec3::new(x, y, height_mm), colour);
    vec![
        corner(-50.0, -50.0),
        corner(250.0, -50.0),
        corner(250.0, 200.0),
        corner(-50.0, -50.0),
        corner(250.0, 200.0),
        corner(-50.0, 200.0),
    ]
}

/// The device and adapter requests are the only async calls the window makes, and it
/// makes them through eframe; a test has to drive them itself.
pub(super) fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::yield_now();
    }
}

/// A closed sphere about the origin, wound outward.
pub(super) fn ball(radius: Scalar, rings: usize, segments: usize) -> Mesh {
    let mut vertices = vec![Vec3::new(0.0, 0.0, radius)];
    for ring in 1..rings {
        let theta = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        let (sin_theta, cos_theta) = theta.sin_cos();
        for segment in 0..segments {
            let phi = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            let (sin_phi, cos_phi) = phi.sin_cos();
            vertices.push(radius * Vec3::new(sin_theta * cos_phi, sin_theta * sin_phi, cos_theta));
        }
    }
    let south = vertices.len() as u32;
    vertices.push(Vec3::new(0.0, 0.0, -radius));

    let at = |ring: usize, segment: usize| (1 + (ring - 1) * segments + segment % segments) as u32;
    let mut faces = Vec::new();
    for segment in 0..segments {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
        faces.push([south, at(rings - 1, segment + 1), at(rings - 1, segment)]);
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            faces.push([
                at(ring, segment),
                at(ring + 1, segment),
                at(ring + 1, segment + 1),
            ]);
            faces.push([
                at(ring, segment),
                at(ring + 1, segment + 1),
                at(ring, segment + 1),
            ]);
        }
    }
    Mesh::new(vertices, faces)
}
