use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use core_geometry::{Mesh, Scalar};
use wgpu::util::DeviceExt as _;

use crate::render::grid::{GRID_MAJOR_MM, GRID_SPACING_MM};
use crate::render::target::SceneTarget;
use crate::render::vertex::ModelInstance;
use crate::scene::Mapped;
use crate::ui::theme;

mod buffers;
mod draws;
mod paint;
mod pipelines;
mod shadow;
mod textures;

pub use draws::{
    CutDraw, CutLine, DrainCut, ExposureBand, Floor, FrameInput, ModelDraw, ReliefDraw, TrapBox,
};

use buffers::{DynamicBuffer, Piece, pieces};
use draws::{Splits, cut_draws, splits_of};
use pipelines::{Builder, Capping, Cutting, Facing, globals_layout};
use shadow::ContactShadow;

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

/// Drain cuts the viewport can subtract in one frame.
///
/// The cuts ride in the globals rather than in a buffer of their own, so the count is
/// fixed; sixty-four is more holes than a plate is drilled with, and what is past it is
/// drawn uncut rather than dropped from the print. See `docs/decisions/0073`.
pub const MAX_CUTS: usize = 64;

/// How many exposure bands the globals carry room for. Must match `MAX_BANDS` in
/// `shader.wgsl`.
pub const MAX_BANDS: usize = 8;

/// How many pockets of trapped resin the globals carry room for. Must match
/// `MAX_POCKETS` in `shader.wgsl`. A plate with more than this many pockets in it has a
/// cavity to rethink rather than a picture; what is past it goes unpainted, never
/// unreported. See `docs/decisions/0200`.
pub const MAX_POCKETS: usize = 32;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_projection: [[f32; 4]; 4],
    /// The lights and the ambient, from `theme::Lighting`; every `w` is unused padding.
    key_towards: [f32; 4],
    fill_towards: [f32; 4],
    key_color: [f32; 4],
    fill_color: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    /// The plate the contact shadow lies on, plate millimetres: `xy` its near corner and
    /// `zw` its size. `shadow_color.a` is how dark the shadow goes.
    shadow_area: [f32; 4],
    shadow_color: [f32; 4],
    /// The colour the plate's own surface is filled with, over the same area, and the
    /// grid drawn into it: `grid` is the minor and the major spacing, millimetres, and
    /// 1.0 in `z` while the grid is shown.
    plate_color: [f32; 4],
    grid_minor: [f32; 4],
    grid_major: [f32; 4],
    grid: [f32; 4],
    /// `xyz` is where the camera stands, plate millimetres; `w` is how much of itself a
    /// surface keeps at most while the models are drawn seen through, and 0.0 while they
    /// are not. See `docs/design/viewport.md`.
    eye: [f32; 4],
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
    /// The Cut tool's plane as `xyz` its normal and `w` its offset along it, millimetres.
    cut_plane: [f32; 4],
    /// The box the plane is traced on, plate millimetres; `cut_low.w` is 1.0 while it is.
    cut_low: [f32; 4],
    cut_high: [f32; 4],
    /// `xyz` is the colour the plane is traced in; `w` is unused padding.
    cut_color: [f32; 4],
    cuts: [DrainCut; MAX_CUTS],
    bands: [ExposureBand; MAX_BANDS],
    pockets: [TrapBox; MAX_POCKETS],
}

/// A mesh already on the GPU, flat-shaded and ready to draw.
///
/// The `Arc` is held so the address it was cached under cannot be reused by a different
/// mesh while the buffer is still in the map. The vertices are one buffer per piece: a
/// hollowed model runs to millions of triangles and no single buffer may hold them, see
/// `docs/decisions/0070-a-mesh-is-drawn-in-buffer-sized-pieces.md`.
struct CachedMesh {
    _mesh: Arc<Mesh>,
    pieces: Vec<Piece>,
    /// The breaks the pieces were cut at. A mesh is first seen drawn whole and only later
    /// drawn in part — the drainage check lands frames after the shell does — so a cache
    /// entry whose breaks no longer cover what is asked for is uploaded again.
    splits: Vec<usize>,
}

/// One mesh to draw this frame: its cache key, whether its placement mirrors it, and
/// which of its faces to draw.
struct Drawn {
    key: usize,
    mirrored: bool,
    faces: Range<usize>,
}

/// One textured model on the card: its triangles with their coordinates, and its images
/// as the layers of one array texture.
struct CachedRelief {
    _mapped: Arc<Mapped>,
    pieces: Vec<(wgpu::Buffer, u32)>,
    bind_group: wgpu::BindGroup,
}

/// Everything the viewport owns on the GPU, kept in egui's callback resources and reused
/// for the lifetime of the window.
pub struct ViewportResources {
    globals: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    model_pipeline: Facing,
    xray_pipeline: Facing,
    line_pipeline: wgpu::RenderPipeline,
    /// What caps the section cut.
    capping: Capping,
    /// What draws the inside of every cut.
    cutting: Cutting,
    /// Draws the word in front of the plate, sampled out of the font atlas.
    label_pipeline: wgpu::RenderPipeline,
    label_layout: wgpu::BindGroupLayout,
    label_sampler: wgpu::Sampler,
    /// Draws a model with its own texture washed over it, for the Relief tool.
    relief_pipeline: Facing,
    relief_layout: wgpu::BindGroupLayout,
    relief_sampler: wgpu::Sampler,
    reliefs: HashMap<usize, CachedRelief>,
    /// The uploaded atlas and the address it was uploaded under.
    atlas: Option<(usize, wgpu::BindGroup)>,
    instances: DynamicBuffer,
    lines: DynamicBuffer,
    cap: DynamicBuffer,
    label: DynamicBuffer,
    meshes: HashMap<usize, CachedMesh>,
    frame: Frame,
    target: SceneTarget,
    shadow: ContactShadow,
}

/// What the last `prepare` decided to draw, replayed by `paint`.
///
/// Instance `i` of the instance buffer belongs to model `i` of `models`, and the solids
/// counted into the stencil follow them in the same buffer, from `solid_base` on.
#[derive(Default)]
struct Frame {
    line_vertices: u32,
    label_vertices: u32,
    /// Each mesh with whether its placement mirrors it, which picks the pipeline.
    models: Vec<Drawn>,
    solids: Vec<Drawn>,
    solid_base: u32,
    /// Each object's cut bodies with the object itself, and where the pair's instances
    /// start: after the models, the solids and the reliefs, in the same buffer.
    cuts: Vec<(Drawn, Drawn)>,
    cut_base: u32,
    /// The textured models, and where their instances start: after the models and the
    /// solids, in the same buffer.
    reliefs: Vec<(usize, bool)>,
    relief_base: u32,
    cap_vertices: u32,
    cutting: bool,
    xray: bool,
}

/// The cut as the shader reads it. A height on its own could not say "no cut": a model
/// What a fixed-size array the shader reads takes of `from`, and how much it took.
fn clamped<T: Copy + Default, const N: usize>(from: &[T]) -> ([T; N], f32) {
    let mut into = [T::default(); N];
    let taken = from.len().min(N);
    into[..taken].copy_from_slice(&from[..taken]);
    (into, taken as f32)
}

/// One frame's uniform: the camera, the light, and everything subtracted or washed over a
/// model per fragment.
fn globals_of(frame: &FrameInput<'_>) -> Globals {
    let (cuts, taken) = clamped::<_, MAX_CUTS>(frame.cuts);
    let (bands, banded) = clamped::<_, MAX_BANDS>(frame.bands);
    let (pockets, held) = clamped::<_, MAX_POCKETS>(frame.pockets);
    let light = &theme::scene().light;
    let towards = |[x, y, z]: [f32; 3]| [x, y, z, 0.0];
    Globals {
        view_projection: frame.view_projection.to_cols_array_2d(),
        key_towards: towards(light.key_towards),
        fill_towards: towards(light.fill_towards),
        key_color: theme::gamma(light.key),
        fill_color: theme::gamma(light.fill),
        sky: theme::gamma(light.sky),
        ground: theme::gamma(light.ground),
        shadow_area: frame.floor.map_or([0.0; 4], |floor| floor.area()),
        shadow_color: theme::gamma(theme::scene().shadow),
        plate_color: theme::gamma(theme::scene().plate),
        grid_minor: theme::gamma(theme::scene().grid_minor),
        grid_major: theme::gamma(theme::scene().grid_major),
        grid: [
            GRID_SPACING_MM,
            GRID_MAJOR_MM,
            if frame.grid { 1.0 } else { 0.0 },
            0.0,
        ],
        eye: frame
            .eye
            .extend(if frame.xray { theme::SEEN_THROUGH } else { 0.0 })
            .to_array(),
        section: section(frame.section_mm),
        counts: [taken, banded, frame.band_floor_mm, held],
        overhang_color: theme::gamma(theme::scene().overhang),
        inside_color: theme::gamma(theme::scene().section_wash),
        volume: frame
            .volume_mm
            .map_or([0.0; 4], |volume| volume.extend(1.0).to_array()),
        outside_color: theme::gamma(theme::scene().outside),
        cut_plane: frame.cut_line.map_or([0.0; 4], |line| {
            line.normal.extend(line.offset_mm).to_array()
        }),
        cut_low: frame
            .cut_line
            .map_or([0.0; 4], |line| line.bounds.mins.extend(1.0).to_array()),
        cut_high: frame
            .cut_line
            .map_or([0.0; 4], |line| line.bounds.maxs.extend(0.0).to_array()),
        cut_color: theme::gamma(theme::scene().cut_line),
        cuts,
        bands,
        pockets,
    }
}

/// standing below the plate is legal, so no value of it is free to mean nothing.
fn section(height_mm: Option<Scalar>) -> [f32; 4] {
    match height_mm {
        Some(height) => [height, 1.0, 0.0, 0.0],
        None => [0.0, 0.0, 0.0, 0.0],
    }
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
        let shadow = ContactShadow::new(device, build.shadowing());

        Self {
            globals,
            globals_bind_group,
            model_pipeline: solid.model,
            xray_pipeline: solid.xray,
            line_pipeline: solid.line,
            capping: solid.capping,
            cutting: solid.cutting,
            label_pipeline: label.pipelines,
            label_layout: label.layout,
            label_sampler: label.sampler,
            relief_pipeline: relief.pipelines,
            relief_layout: relief.layout,
            relief_sampler: relief.sampler,
            reliefs: HashMap::new(),
            atlas: None,
            instances: DynamicBuffer::new(device, "viewport_instances"),
            lines: DynamicBuffer::new(device, "viewport_lines"),
            cap: DynamicBuffer::new(device, "viewport_section_cap"),
            label: DynamicBuffer::new(device, "viewport_label"),
            meshes: HashMap::new(),
            frame: Frame::default(),
            target: SceneTarget::new(device, target_format),
            shadow,
        }
    }

    /// Uploads one frame's camera, plate geometry and objects, and records what to draw.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: FrameInput<'_>) {
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals_of(&frame)));
        self.write_geometry(device, queue, &frame);
        self.count_frame(&frame);
        self.record(device, queue, &frame);
        self.shadow.want(&frame);
        self.drop_what_nobody_drew();
    }

    /// Writes the plate, the cap, the word and every object's placement.
    fn write_geometry(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &FrameInput<'_>,
    ) {
        self.lines.write(device, queue, frame.lines);
        self.cap.write(device, queue, frame.cap);
        self.label.write(device, queue, frame.label);
        if let Some(atlas) = frame.atlas.as_ref() {
            self.upload_atlas(device, queue, atlas);
        }
        let instances: Vec<ModelInstance> = frame
            .models
            .iter()
            .chain(frame.solids)
            .map(|draw| draw.instance)
            .chain(frame.reliefs.iter().map(|draw| draw.instance))
            .chain(cut_draws(frame).map(|draw| draw.instance))
            .collect();
        self.instances.write(device, queue, &instances);
    }

    /// Where each kind of draw starts in the one instance buffer, and how much of the
    /// plate geometry was written.
    fn count_frame(&mut self, frame: &FrameInput<'_>) {
        self.frame.line_vertices = frame.lines.len() as u32;
        self.frame.label_vertices = frame.label.len() as u32;
        self.frame.cap_vertices = frame.cap.len() as u32;
        self.frame.cutting = frame.section_mm.is_some();
        self.frame.xray = frame.xray;
        self.frame.solid_base = frame.models.len() as u32;
        self.frame.relief_base = (frame.models.len() + frame.solids.len()) as u32;
        self.frame.cut_base = self.frame.relief_base + frame.reliefs.len() as u32;
        self.frame.models.clear();
        self.frame.solids.clear();
        self.frame.reliefs.clear();
        self.frame.cuts.clear();
    }

    /// Uploads every mesh the frame draws and records which piece of it to paint.
    fn record(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &FrameInput<'_>) {
        // Every draw of a mesh is read before any of it is uploaded: a shell drawn whole
        // and its cavity drawn on its own share one cache entry, whose pieces have to
        // break where either of them starts and ends.
        let splits = splits_of(
            frame
                .models
                .iter()
                .chain(frame.solids)
                .chain(cut_draws(frame)),
        );
        for (draws, solid) in [(frame.models, false), (frame.solids, true)] {
            for draw in draws {
                let drawn = self.drawn(device, draw, &splits);
                match solid {
                    true => self.frame.solids.push(drawn),
                    false => self.frame.models.push(drawn),
                }
            }
        }
        for draw in frame.reliefs {
            let key = self.cache_relief(device, queue, draw);
            self.frame.reliefs.push((key, draw.instance.is_mirrored()));
        }
        for cut in frame.cut_surfaces {
            let body = self.drawn(device, &cut.body, &splits);
            let solid = self.drawn(device, &cut.solid, &splits);
            self.frame.cuts.push((body, solid));
        }
    }

    /// One draw's mesh in the cache, and which of its pieces this draw paints.
    fn drawn(&mut self, device: &wgpu::Device, draw: &ModelDraw, splits: &Splits) -> Drawn {
        let key = self.cache(
            device,
            &draw.mesh,
            &splits[&(Arc::as_ptr(&draw.mesh) as usize)],
        );
        Drawn {
            key,
            mirrored: draw.instance.is_mirrored(),
            faces: draw.faces.clone(),
        }
    }

    /// Lets go of every mesh and texture nobody drew this frame: it was taken off the
    /// plate or replaced.
    fn drop_what_nobody_drew(&mut self) {
        let live: HashSet<usize> = self
            .frame
            .models
            .iter()
            .chain(&self.frame.solids)
            .chain(
                self.frame
                    .cuts
                    .iter()
                    .flat_map(|(body, solid)| [body, solid]),
            )
            .map(|drawn| drawn.key)
            .collect();
        self.meshes.retain(|key, _| live.contains(key));
        let textured: HashSet<usize> = self.frame.reliefs.iter().map(|(key, _)| *key).collect();
        self.reliefs.retain(|key, _| textured.contains(key));
    }

    /// Uploads a mesh the first time it is seen, or again once a draw asks for a part of
    /// it the pieces do not break at, and returns its cache key. `splits` are face indices
    /// no piece may straddle, so that a draw of part of the mesh is a whole number of
    /// pieces.
    fn cache(&mut self, device: &wgpu::Device, mesh: &Arc<Mesh>, splits: &[usize]) -> usize {
        let key = Arc::as_ptr(mesh) as usize;
        if self
            .meshes
            .get(&key)
            .is_some_and(|held| held.splits == splits)
        {
            return key;
        }
        self.meshes.insert(
            key,
            CachedMesh {
                _mesh: Arc::clone(mesh),
                pieces: pieces(device, mesh, splits),
                splits: splits.to_vec(),
            },
        );
        key
    }
}
