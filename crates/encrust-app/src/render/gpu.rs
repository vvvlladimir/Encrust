use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use core_geometry::{Mat4, Mesh, Scalar, Vec3};
use wgpu::util::DeviceExt as _;

use crate::render::target::SceneTarget;
use crate::render::vertex::{
    BodyVertex, LabelVertex, LineVertex, ModelInstance, ModelVertex, ReliefVertex,
    flat_shaded_vertices, textured_vertices,
};
use crate::scene::Mapped;
use crate::ui::theme;

/// Depth and stencil format of the viewport's own target, `render::target`. The stencil
/// plane is what caps the section cut, see `docs/decisions/0062` and `0184`.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

/// Multisampling is off, matching [`MULTISAMPLING`]. A pipeline whose sample count
/// disagrees with the render pass is rejected at draw time.
pub const SAMPLE_COUNT: u32 = 1;

/// What `NativeOptions::multisampling` must be set to for [`SAMPLE_COUNT`]. Zero and one
/// both mean one sample.
#[cfg(not(target_arch = "wasm32"))]
pub const MULTISAMPLING: u16 = 0;

/// Direction the key light travels, in plate coordinates: down, from the front left.
const LIGHT_DIRECTION: [f32; 4] = [0.35, 0.55, -1.0, 0.0];

const INITIAL_CAPACITY_BYTES: u64 = 4096;

/// Drain cuts the viewport can subtract in one frame.
///
/// The cuts ride in the globals rather than in a buffer of their own, so the count is
/// fixed; sixty-four is more holes than a plate is drilled with, and what is past it is
/// drawn uncut rather than dropped from the print. See `docs/decisions/0073`.
pub const MAX_CUTS: usize = 64;

/// How many exposure bands the globals carry room for. Must match `MAX_BANDS` in
/// `shader.wgsl`.
pub const MAX_BANDS: usize = 8;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_projection: [[f32; 4]; 4],
    light_direction: [f32; 4],
    /// `x` is the height the models are cut at, plate millimetres, and `y` is 1.0 while
    /// there is a cut at all. `z` and `w` pad the field out to a `vec4`.
    section: [f32; 4],
    /// `x` is how many of `cuts` carry a drain this frame, `y` how many of `bands` carry
    /// an exposure, and `z` the height below which a band has no effect.
    counts: [f32; 4],
    /// `xyz` washes a surface that needs holding up; `w` is unused padding.
    overhang_color: [f32; 4],
    /// `xyz` washes whatever shows of a model's inside; `w` is unused padding.
    inside_color: [f32; 4],
    /// `xyz` is the build volume, plate millimetres; `w` is 1.0 while what stands past it
    /// is marked.
    volume: [f32; 4],
    /// `xyz` marks whatever stands past the build volume; `w` is unused padding.
    outside_color: [f32; 4],
    cuts: [DrainCut; MAX_CUTS],
    bands: [ExposureBand; MAX_BANDS],
}

/// One band of print height that takes an exposure of its own, for the fragment shader to
/// tint the models with.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct ExposureBand {
    /// `x` is the bottom of the band and `y` its top, plate millimetres.
    span: [f32; 4],
    /// `xyz` is what the band washes the surface with.
    tint: [f32; 4],
}

impl ExposureBand {
    /// Takes a token rather than floats so that no colour can be spelled in the renderer.
    pub fn new(from_mm: f32, to_mm: f32, tint: egui::Color32) -> Self {
        Self {
            span: [from_mm, to_mm, 0.0, 0.0],
            tint: theme::gamma(tint),
        }
    }
}

/// One drain hole or channel segment, in plate millimetres, for the fragment shader to
/// subtract from whatever is drawn.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct DrainCut {
    /// `xyz` is the mouth, `w` the radius there.
    pub mouth: [f32; 4],
    /// `xyz` is the tip, `w` the radius there.
    pub tip: [f32; 4],
}

/// A mesh already on the GPU, flat-shaded and ready to draw.
///
/// The `Arc` is held so the address it was cached under cannot be reused by a different
/// mesh while the buffer is still in the map. The vertices are one buffer per piece: a
/// hollowed model runs to millions of triangles and no single buffer may hold them, see
/// `docs/decisions/0070-a-mesh-is-drawn-in-buffer-sized-pieces.md`.
struct CachedMesh {
    _mesh: Arc<Mesh>,
    pieces: Vec<(wgpu::Buffer, u32)>,
}

/// One textured model on the card: its triangles with their coordinates, and its images
/// as the layers of one array texture.
struct CachedRelief {
    _mapped: Arc<Mapped>,
    pieces: Vec<(wgpu::Buffer, u32)>,
    bind_group: wgpu::BindGroup,
}

/// How many pixels a side each image is resampled to before it goes to the card.
///
/// The layers of an array texture are all one size, and the models carry images that are
/// not. Five hundred is past what a viewport shows of a texture on a moving model, and
/// four images cost a megabyte between them.
const RELIEF_SIDE: u32 = 512;

/// One frame's worth of what to draw, handed over in one piece rather than as a row of
/// parallel arguments.
pub struct FrameInput<'a> {
    pub view_projection: Mat4,
    /// Height the models are cut at, plate millimetres, or `None` to draw them whole.
    pub section_mm: Option<Scalar>,
    pub lines: &'a [LineVertex],
    pub models: &'a [ModelDraw],
    /// The solids the cut is capped against, counted into the stencil plane.
    pub solids: &'a [ModelDraw],
    /// The quad lying in the cutting plane the cap is painted with.
    pub cap: &'a [LineVertex],
    /// The machine under the plate, drawn last and translucent.
    pub body: &'a [BodyVertex],
    /// The word lying on the machine, and the font atlas its triangles sample. The atlas
    /// goes to the card the first time it is seen and is kept by its address after that.
    pub label: &'a [LabelVertex],
    pub atlas: Option<Arc<egui::ColorImage>>,
    /// The holes and channels already cut into the models being drawn, subtracted per
    /// fragment because the meshes themselves are never cut; see ADR 0071, 0073.
    pub cuts: &'a [DrainCut],
    /// The models drawn with their own texture on them rather than flat, which is what
    /// the Relief tool shows; see ADR 0116.
    pub reliefs: &'a [ReliefDraw],
    /// The exposure bands washed over the models, and the height below which one has no
    /// effect because the bottom block keeps the resin's ramp; see ADR 0090.
    pub bands: &'a [ExposureBand],
    pub band_floor_mm: f32,
    /// The build volume in plate millimetres, or `None` to mark nothing standing past it.
    pub volume_mm: Option<Vec3>,
}

/// One object to draw: which cached mesh, and which instance slot holds its placement.
pub struct ModelDraw {
    pub mesh: Arc<Mesh>,
    pub instance: ModelInstance,
}

/// One object drawn with the texture a relief would be pressed from, instead of flat.
pub struct ReliefDraw {
    pub mesh: Arc<Mesh>,
    /// The map and its images, which are also what the cache is keyed by: pressing a
    /// relief replaces both the mesh and the map.
    pub mapped: Arc<Mapped>,
    pub instance: ModelInstance,
}

/// Everything the viewport owns on the GPU, kept in egui's callback resources and reused
/// for the lifetime of the window.
pub struct ViewportResources {
    globals: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    model_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    /// What caps the section cut.
    capping: Capping,
    /// Draws the translucent machine over everything already painted.
    body_pipeline: wgpu::RenderPipeline,
    /// Draws the word the machine carries, sampled out of the font atlas.
    label_pipeline: wgpu::RenderPipeline,
    label_layout: wgpu::BindGroupLayout,
    label_sampler: wgpu::Sampler,
    /// Draws a model with its own texture washed over it, for the Relief tool.
    relief_pipeline: wgpu::RenderPipeline,
    relief_layout: wgpu::BindGroupLayout,
    relief_sampler: wgpu::Sampler,
    reliefs: HashMap<usize, CachedRelief>,
    /// The uploaded atlas and the address it was uploaded under.
    atlas: Option<(usize, wgpu::BindGroup)>,
    instances: DynamicBuffer,
    lines: DynamicBuffer,
    cap: DynamicBuffer,
    body: DynamicBuffer,
    label: DynamicBuffer,
    meshes: HashMap<usize, CachedMesh>,
    frame: Frame,
    target: SceneTarget,
}

/// What the last `prepare` decided to draw, replayed by `paint`.
///
/// Instance `i` of the instance buffer belongs to model `i` of `models`, and the solids
/// counted into the stencil follow them in the same buffer, from `solid_base` on.
#[derive(Default)]
struct Frame {
    line_vertices: u32,
    body_vertices: u32,
    label_vertices: u32,
    models: Vec<usize>,
    solids: Vec<usize>,
    solid_base: u32,
    /// The textured models, and where their instances start: after the models and the
    /// solids, in the same buffer.
    reliefs: Vec<usize>,
    relief_base: u32,
    cap_vertices: u32,
    cutting: bool,
}

impl ViewportResources {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let globals_layout = globals_layout(device);
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport_globals"),
            size: size_of::<Globals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport_globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });

        let build = Builder::new(device, target_format, &globals_layout);
        let solid = build.solid();
        let label = build.label();
        let relief = build.relief();

        Self {
            globals,
            globals_bind_group,
            model_pipeline: solid.model,
            line_pipeline: solid.line,
            capping: solid.capping,
            body_pipeline: build.body(),
            label_pipeline: label.pipeline,
            label_layout: label.layout,
            label_sampler: label.sampler,
            relief_pipeline: relief.pipeline,
            relief_layout: relief.layout,
            relief_sampler: relief.sampler,
            reliefs: HashMap::new(),
            atlas: None,
            instances: DynamicBuffer::new(device, "viewport_instances"),
            lines: DynamicBuffer::new(device, "viewport_lines"),
            cap: DynamicBuffer::new(device, "viewport_section_cap"),
            body: DynamicBuffer::new(device, "viewport_machine"),
            label: DynamicBuffer::new(device, "viewport_label"),
            meshes: HashMap::new(),
            frame: Frame::default(),
            target: SceneTarget::new(device, target_format),
        }
    }

    /// Uploads one frame's camera, plate geometry and objects, and records what to draw.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: FrameInput<'_>) {
        let FrameInput {
            view_projection,
            section_mm,
            lines,
            models,
            solids,
            cap,
            body,
            label,
            atlas,
            cuts,
            reliefs,
            bands,
            band_floor_mm,
            volume_mm,
        } = frame;
        let mut drains = [DrainCut::default(); MAX_CUTS];
        let taken = cuts.len().min(MAX_CUTS);
        drains[..taken].copy_from_slice(&cuts[..taken]);
        let mut washes = [ExposureBand::default(); MAX_BANDS];
        let banded = bands.len().min(MAX_BANDS);
        washes[..banded].copy_from_slice(&bands[..banded]);
        queue.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals {
                view_projection: view_projection.to_cols_array_2d(),
                light_direction: LIGHT_DIRECTION,
                section: section(section_mm),
                counts: [taken as f32, banded as f32, band_floor_mm, 0.0],
                overhang_color: theme::gamma(theme::scene().overhang),
                inside_color: theme::gamma(theme::scene().section_wash),
                volume: volume_mm.map_or([0.0; 4], |volume| volume.extend(1.0).to_array()),
                outside_color: theme::gamma(theme::scene().outside),
                cuts: drains,
                bands: washes,
            }),
        );

        self.lines.write(device, queue, lines);
        self.cap.write(device, queue, cap);
        self.body.write(device, queue, body);
        self.label.write(device, queue, label);
        if let Some(atlas) = atlas {
            self.upload_atlas(device, queue, &atlas);
        }
        let instances: Vec<ModelInstance> = models
            .iter()
            .chain(solids)
            .map(|draw| draw.instance)
            .chain(reliefs.iter().map(|draw| draw.instance))
            .collect();
        self.instances.write(device, queue, &instances);

        self.frame.line_vertices = lines.len() as u32;
        self.frame.body_vertices = body.len() as u32;
        self.frame.label_vertices = label.len() as u32;
        self.frame.cap_vertices = cap.len() as u32;
        self.frame.cutting = section_mm.is_some();
        self.frame.solid_base = models.len() as u32;
        self.frame.relief_base = (models.len() + solids.len()) as u32;
        self.frame.models.clear();
        self.frame.solids.clear();
        self.frame.reliefs.clear();
        for draw in models {
            let key = self.cache(device, &draw.mesh);
            self.frame.models.push(key);
        }
        for draw in solids {
            let key = self.cache(device, &draw.mesh);
            self.frame.solids.push(key);
        }
        for draw in reliefs {
            let key = self.cache_relief(device, queue, draw);
            self.frame.reliefs.push(key);
        }

        // A mesh nobody drew this frame was removed from the scene or replaced.
        let live: HashSet<usize> = self
            .frame
            .models
            .iter()
            .chain(&self.frame.solids)
            .copied()
            .collect();
        self.meshes.retain(|key, _| live.contains(key));
        let textured: HashSet<usize> = self.frame.reliefs.iter().copied().collect();
        self.reliefs.retain(|key, _| textured.contains(key));
    }

    /// Uploads the font atlas the first time it is seen. It is kept by its address, the
    /// same way a mesh is: the window lays the word out once and hands the same image over
    /// every frame after that.
    fn upload_atlas(
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
    fn cache_relief(
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

    /// Uploads a mesh the first time it is seen and returns its cache key.
    fn cache(&mut self, device: &wgpu::Device, mesh: &Arc<Mesh>) -> usize {
        let key = Arc::as_ptr(mesh) as usize;
        self.meshes.entry(key).or_insert_with(|| CachedMesh {
            _mesh: Arc::clone(mesh),
            pieces: pieces(device, mesh),
        });
        key
    }

    /// Draws the frame `prepare` recorded into the viewport's own target, into `viewport`
    /// of a window `size_px` pixels large.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        size_px: [u32; 2],
        viewport: egui::epaint::ViewportInPixels,
    ) {
        let mut pass = self.target.begin(device, encoder, size_px, viewport);
        self.paint(&mut pass);
    }

    /// Lays what `draw` drew over egui's pass.
    pub fn present(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        self.target.present(render_pass);
    }

    /// Records the scene into a pass whose depth plane is [`DEPTH_FORMAT`].
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        render_pass.set_bind_group(0, &self.globals_bind_group, &[]);

        if self.frame.line_vertices > 0 {
            render_pass.set_pipeline(&self.line_pipeline);
            render_pass.set_vertex_buffer(0, self.lines.buffer.slice(..));
            render_pass.draw(0..self.frame.line_vertices, 0..1);
        }

        render_pass.set_vertex_buffer(1, self.instances.buffer.slice(..));

        // The cut is capped rather than left open, and the cap is stencilled: what the
        // cut took away is counted first, the models are drawn, and the face is filled
        // wherever the count says the plane runs through a solid. See
        // `docs/decisions/0062`.
        if self.frame.cutting {
            render_pass.set_pipeline(&self.capping.crossing);
            self.draw_meshes(render_pass, &self.frame.solids, self.frame.solid_base);
        }

        render_pass.set_pipeline(&self.model_pipeline);
        self.draw_meshes(render_pass, &self.frame.models, 0);

        if !self.frame.reliefs.is_empty() {
            render_pass.set_pipeline(&self.relief_pipeline);
            for (index, key) in self.frame.reliefs.iter().enumerate() {
                let Some(relief) = self.reliefs.get(key) else {
                    continue;
                };
                let instance = self.frame.relief_base + index as u32;
                render_pass.set_bind_group(1, &relief.bind_group, &[]);
                for (buffer, count) in &relief.pieces {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.draw(0..*count, instance..instance + 1);
                }
            }
        }

        if self.frame.cutting && self.frame.cap_vertices > 0 {
            render_pass.set_pipeline(&self.capping.cap);
            render_pass.set_stencil_reference(MATERIAL_ABOVE);
            render_pass.set_vertex_buffer(0, self.cap.buffer.slice(..));
            render_pass.draw(0..self.frame.cap_vertices, 0..1);
        }

        // Last, so the models are already in the depth buffer and the near wall of the
        // vat washes over whatever stands in front of it.
        if self.frame.body_vertices > 0 {
            render_pass.set_pipeline(&self.body_pipeline);
            render_pass.set_vertex_buffer(0, self.body.buffer.slice(..));
            render_pass.draw(0..self.frame.body_vertices, 0..1);
        }

        if let Some((_, atlas)) = self
            .atlas
            .as_ref()
            .filter(|_| self.frame.label_vertices > 0)
        {
            render_pass.set_pipeline(&self.label_pipeline);
            render_pass.set_bind_group(1, atlas, &[]);
            render_pass.set_vertex_buffer(0, self.label.buffer.slice(..));
            render_pass.draw(0..self.frame.label_vertices, 0..1);
        }
    }

    /// Draws one mesh per entry, each with the instance that many slots past `base`.
    fn draw_meshes(&self, render_pass: &mut wgpu::RenderPass<'static>, keys: &[usize], base: u32) {
        for (index, key) in keys.iter().enumerate() {
            let Some(mesh) = self.meshes.get(key).filter(|mesh| !mesh.pieces.is_empty()) else {
                continue;
            };
            let instance = base + index as u32;
            for (buffer, count) in &mesh.pieces {
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..*count, instance..instance + 1);
            }
        }
    }
}

/// Half the stencil's range, which the counting below wraps around: a pixel whose count
/// ran negative — the plane inside material — lands above it, one whose count ran positive
/// lands below. See `docs/decisions/0074-the-section-cap-counts-material-not-crossings.md`.
const MATERIAL_ABOVE: u32 = 127;

/// Counting the crossings: a ray leaving a solid above the cut counts up, one entering it
/// counts down, so the plane is inside material wherever the count ran negative. Back
/// faces are the ones a ray leaves through, and a body wound inward counts the other way
/// round, which is what takes a drain hole back out of the count.
fn counting_stencil() -> wgpu::StencilState {
    let face = |pass_op| wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Always,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op,
    };
    wgpu::StencilState {
        front: face(wgpu::StencilOperation::DecrementWrap),
        back: face(wgpu::StencilOperation::IncrementWrap),
        read_mask: 0xff,
        write_mask: 0xff,
    }
}

/// Drawing only where the count says the plane is inside material, and leaving the count
/// alone while doing it. Negative counts wrap past [`MATERIAL_ABOVE`]; a positive one is a
/// body wound inward standing in the air, which encloses nothing.
fn inside_only_stencil() -> wgpu::StencilState {
    let face = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Less,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };
    wgpu::StencilState {
        front: face,
        back: face,
        read_mask: 0xff,
        write_mask: 0x00,
    }
}

/// The cut as the shader reads it. A height on its own could not say "no cut": a model
/// standing below the plate is legal, so no value of it is free to mean nothing.
fn section(height_mm: Option<Scalar>) -> [f32; 4] {
    match height_mm {
        Some(height) => [height, 1.0, 0.0, 0.0],
        None => [0.0, 0.0, 0.0, 0.0],
    }
}

/// The one uniform every pipeline reads: the camera, the light and the section.
fn globals_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viewport_globals_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// The pipelines that draw the plate and the models in their own flat colours.
struct Solid {
    model: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    capping: Capping,
}

/// The two passes that fill the section cut with a flat face.
struct Capping {
    /// Counts, in the stencil plane, how often a view ray crosses a solid above the cut.
    crossing: wgpu::RenderPipeline,
    /// Fills the cut wherever that count says the plane is inside a solid.
    cap: wgpu::RenderPipeline,
}

/// A pipeline that samples one texture, with the layout its bind group is built against
/// and the sampler it reads through.
struct Textured {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// What every viewport pipeline is built from: one shader, one target, one uniform.
struct Builder<'a> {
    device: &'a wgpu::Device,
    shader: wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
    globals_layout: &'a wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
}

impl<'a> Builder<'a> {
    fn new(
        device: &'a wgpu::Device,
        target_format: wgpu::TextureFormat,
        globals_layout: &'a wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            device,
            shader: device.create_shader_module(wgpu::include_wgsl!("shader.wgsl")),
            target_format,
            globals_layout,
            layout: device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viewport"),
                bind_group_layouts: &[Some(globals_layout)],
                immediate_size: 0,
            }),
        }
    }

    fn plain(&self, kind: PipelineKind<'_>) -> wgpu::RenderPipeline {
        pipeline(
            self.device,
            &self.layout,
            &self.shader,
            self.target_format,
            kind,
        )
    }

    fn solid(&self) -> Solid {
        Solid {
            model: self.plain(PipelineKind {
                label: "viewport_models",
                vertex_entry: "model_vertex",
                fragment_entry: "model_fragment",
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                ..PipelineKind::default()
            }),
            line: self.plain(PipelineKind {
                label: "viewport_lines",
                vertex_entry: "line_vertex",
                fragment_entry: "line_fragment",
                buffers: &[Some(LineVertex::layout())],
                topology: wgpu::PrimitiveTopology::LineList,
                ..PipelineKind::default()
            }),
            capping: self.capping(),
        }
    }

    fn capping(&self) -> Capping {
        Capping {
            crossing: self.plain(PipelineKind {
                label: "viewport_section_crossings",
                vertex_entry: "model_vertex",
                fragment_entry: "section_crossing_fragment",
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                writes_color: false,
                // Every crossing counts, whatever is in front of it, so the depth buffer
                // takes no part in this pass.
                depth_write: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: counting_stencil(),
                ..PipelineKind::default()
            }),
            cap: self.plain(PipelineKind {
                label: "viewport_section_cap",
                vertex_entry: "line_vertex",
                fragment_entry: "line_fragment",
                buffers: &[Some(LineVertex::layout())],
                stencil: inside_only_stencil(),
                ..PipelineKind::default()
            }),
        }
    }

    /// Draws the translucent machine over everything already painted.
    fn body(&self) -> wgpu::RenderPipeline {
        self.plain(PipelineKind {
            label: "viewport_machine",
            vertex_entry: "body_vertex",
            fragment_entry: "body_fragment",
            buffers: &[Some(BodyVertex::layout())],
            // Painted last and see-through, so it must take no depth of its own: a near
            // wall would otherwise hide the far one it is meant to show.
            depth_write: false,
            ..PipelineKind::default()
        })
    }

    /// Draws the word the machine carries, sampled out of the font atlas.
    fn label(&self) -> Textured {
        self.textured(
            wgpu::TextureViewDimension::D2,
            wgpu::AddressMode::ClampToEdge,
            PipelineKind {
                label: "viewport_label",
                vertex_entry: "label_vertex",
                fragment_entry: "label_fragment",
                buffers: &[Some(LabelVertex::layout())],
                // Lies on the machine, which takes no depth of its own, so neither does
                // the word: what hides it is the model standing in front of it.
                depth_write: false,
                ..PipelineKind::default()
            },
        )
    }

    /// Draws a model with its own texture washed over it, for the Relief tool.
    fn relief(&self) -> Textured {
        self.textured(
            wgpu::TextureViewDimension::D2Array,
            // Wrapping, because a coordinate outside the unit square wraps everywhere else
            // a texture is read; see ADR 0116.
            wgpu::AddressMode::Repeat,
            PipelineKind {
                label: "viewport_relief",
                vertex_entry: "relief_vertex",
                fragment_entry: "relief_fragment",
                buffers: &[Some(ReliefVertex::layout()), Some(ModelInstance::layout())],
                ..PipelineKind::default()
            },
        )
    }

    fn textured(
        &self,
        dimension: wgpu::TextureViewDimension,
        address_mode: wgpu::AddressMode,
        kind: PipelineKind<'_>,
    ) -> Textured {
        let layout = texture_layout(self.device, kind.label, dimension);
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(kind.label),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(kind.label),
                bind_group_layouts: &[Some(self.globals_layout), Some(&layout)],
                immediate_size: 0,
            });
        Textured {
            pipeline: pipeline(
                self.device,
                &pipeline_layout,
                &self.shader,
                self.target_format,
                kind,
            ),
            layout,
            sampler,
        }
    }
}

/// One filtered texture and the sampler it is read through, as a fragment shader sees them.
fn texture_layout(
    device: &wgpu::Device,
    label: &str,
    dimension: wgpu::TextureViewDimension,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{label}_layout")),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: dimension,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

struct PipelineKind<'a> {
    label: &'a str,
    vertex_entry: &'a str,
    fragment_entry: &'a str,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    topology: wgpu::PrimitiveTopology,
    writes_color: bool,
    depth_write: bool,
    depth_compare: wgpu::CompareFunction,
    stencil: wgpu::StencilState,
}

impl Default for PipelineKind<'_> {
    fn default() -> Self {
        Self {
            label: "viewport",
            vertex_entry: "model_vertex",
            fragment_entry: "model_fragment",
            buffers: &[],
            topology: wgpu::PrimitiveTopology::TriangleList,
            writes_color: true,
            depth_write: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
        }
    }
}

/// Back faces are kept: an imported mesh may have inverted faces, and hiding them would
/// hide the defect the Scene panel is reporting.
fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
    kind: PipelineKind<'_>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(kind.label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(kind.vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: kind.buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(kind.fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: if kind.writes_color {
                    wgpu::ColorWrites::ALL
                } else {
                    wgpu::ColorWrites::empty()
                },
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: kind.topology,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(kind.depth_write),
            depth_compare: Some(kind.depth_compare),
            stencil: kind.stencil.clone(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: SAMPLE_COUNT,
            ..Default::default()
        },
        cache: None,
        multiview_mask: None,
    })
}

/// A vertex buffer that grows to fit whatever the frame needs and never shrinks.
struct DynamicBuffer {
    buffer: wgpu::Buffer,
    capacity_bytes: u64,
    label: &'static str,
}

impl DynamicBuffer {
    fn new(device: &wgpu::Device, label: &'static str) -> Self {
        Self {
            buffer: allocate(device, label, INITIAL_CAPACITY_BYTES),
            capacity_bytes: INITIAL_CAPACITY_BYTES,
            label,
        }
    }

    fn write<T: Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[T]) {
        let bytes = bytemuck::cast_slice(data);
        if bytes.is_empty() {
            return;
        }
        if bytes.len() as u64 > self.capacity_bytes {
            self.capacity_bytes = (bytes.len() as u64).next_power_of_two();
            self.buffer = allocate(device, self.label, self.capacity_bytes);
        }
        queue.write_buffer(&self.buffer, 0, bytes);
    }
}

fn allocate(device: &wgpu::Device, label: &'static str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A mesh flat-shaded into as few buffers as the card's own ceiling allows.
///
/// Expanded a piece at a time as well as uploaded a piece at a time: a cavity of eleven
/// million triangles is eight hundred megabytes of vertices, and holding all of them to
/// hand one buffer over is the same wall from the other side.
fn pieces(device: &wgpu::Device, mesh: &Mesh) -> Vec<(wgpu::Buffer, u32)> {
    let ceiling = device.limits().max_buffer_size as usize / size_of::<ModelVertex>();
    let faces_per_piece = (ceiling / 3).max(1);

    (0..mesh.faces.len())
        .step_by(faces_per_piece)
        .filter_map(|first| {
            let last = (first + faces_per_piece).min(mesh.faces.len());
            let vertices = flat_shaded_vertices(mesh, first..last);
            (!vertices.is_empty()).then(|| {
                (
                    upload(device, "viewport_mesh", &vertices),
                    vertices.len() as u32,
                )
            })
        })
        .collect()
}

/// The model's images as the layers of one array texture, each resampled to
/// [`RELIEF_SIDE`] square.
fn relief_texture(
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

/// Creates a buffer holding exactly `data`. Used for meshes, which never change once
/// imported; anything that changes every frame goes through a [`DynamicBuffer`].
fn upload<T: Pod>(device: &wgpu::Device, label: &'static str, data: &[T]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::VERTEX,
    })
}
