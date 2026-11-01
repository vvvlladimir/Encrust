use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use bytemuck::{Pod, Zeroable};
use core_geometry::{Mesh, Scalar, SmoothNormals};
use wgpu::util::DeviceExt as _;

use crate::render::frame_time::{CardTime, FrameLog};
use crate::render::grid::{GRID_MAJOR_MM, GRID_SPACING_MM};
use crate::render::target::SceneTarget;
use crate::render::vertex::{LineVertex, ModelInstance};
use crate::scene::Mapped;
use crate::ui::theme;

mod buffers;
mod can_print;
mod clear;
mod cost;
mod draws;
mod environment;
#[cfg(test)]
mod fixtures;
mod globals;
mod light;
mod paint;
mod pipelines;
mod settle;
mod shadow;
mod textures;
mod timer;

pub use draws::{
    CutDraw, CutLine, Desk, DrainCut, ExposureBand, Floor, FrameInput, Look, ModelDraw, ReliefDraw,
    TrapBox,
};

use buffers::{DynamicBuffer, Piece, pieces};
use can_print::CanPrintTexture;
use clear::Clear;
use draws::{Splits, cut_draws, splits_of};
use environment::Environment;
use globals::{Globals, globals_of};
use light::KeyShadow;
use pipelines::{
    Builder, Capping, Cutting, Facing, Markers, Models, Shaded, Studio, globals_layout,
};
pub use settle::SETTLED_SAMPLES;
use settle::Settling;
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

/// A mesh already on the GPU, flat or smooth-shaded and ready to draw.
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
    /// Whether its corners carry normals smoothed over their faces rather than a face's own.
    smooth: bool,
    /// When a frame last drew it: a mesh drawn only while the camera moves, a light copy,
    /// stays on the card between moves rather than going up again with each one.
    drawn_at: web_time::Instant,
}

/// How long a mesh no frame draws stays on the card.
const KEEP_UNDRAWN: std::time::Duration = std::time::Duration::from_secs(10);

/// One mesh to draw this frame: its cache key, whether its placement mirrors it, which of
/// its faces to draw, and whether it is a surface nothing shows through.
struct Drawn {
    key: usize,
    mirrored: bool,
    faces: Range<usize>,
    opaque: bool,
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
    globals_layout: wgpu::BindGroupLayout,
    /// The globals with the key's map stood in for, bound while the map is drawn.
    casting_bind_group: wgpu::BindGroup,
    key: KeyShadow,
    /// The studio as reflections see it, baked for the render view.
    environment: Environment,
    /// The print round the render view's can.
    can_print: CanPrintTexture,
    settling: Settling,
    models: Models,
    xray_pipeline: Facing,
    /// The clear-resin view. `reads_planes` is false on a WebGL canvas, where the render view
    /// is drawn opaque and its creases unshaded.
    clear: Clear,
    reads_planes: bool,
    /// The render view's room, drawn in place of the plate.
    backdrop: Studio<wgpu::RenderPipeline>,
    marker_pipelines: Markers,
    line_pipeline: wgpu::RenderPipeline,
    wide_line_pipeline: wgpu::RenderPipeline,
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
    wide_lines: DynamicBuffer,
    cap: DynamicBuffer,
    label: DynamicBuffer,
    meshes: HashMap<usize, CachedMesh>,
    frame: Frame,
    target: SceneTarget,
    shadow: ContactShadow,
    /// What this frame cost so far, and the frames before it a second at a time.
    uploaded_faces: usize,
    shadow_baked: bool,
    frame_log: FrameLog,
    /// The card's own clock over the scene pass, where it has one, where its times go
    /// with the models' faces this frame draws, and the latest time, milliseconds.
    timer: Option<timer::CardTimer>,
    report: Option<Sender<CardTime>>,
    planned_faces: Option<usize>,
    card_ms: Option<f32>,
}

/// What the last `prepare` decided to draw, replayed by `paint`.
///
/// Instance `i` of the instance buffer belongs to model `i` of `models`, and the solids
/// counted into the stencil follow them in the same buffer, from `solid_base` on.
#[derive(Default)]
struct Frame {
    line_vertices: u32,
    wide_lines: u32,
    label_vertices: u32,
    /// Each mesh with whether its placement mirrors it, which picks the pipeline.
    models: Vec<Drawn>,
    /// Where the markers start among `models`: they are drawn after the cap.
    markers_from: usize,
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
    /// The top cap's vertices, then the bottom cap's after them in the same buffer.
    cap_vertices: u32,
    bottom_cap_vertices: u32,
    cutting: bool,
    cutting_bottom: bool,
    /// Whether a section or a drain cut may take a fragment of any model away.
    clipped: bool,
    xray: bool,
    /// Whether the models are drawn as a clear resin against the thickness pass.
    clear: bool,
    /// Whether the thickness is measured: the render view at rest, where a thin part of any
    /// resin glows with the key behind it, and a clear one is seen through.
    thick: bool,
    /// Whether the render view puts the printer away and stands the models in its room.
    staged: bool,
    /// Whether the camera moves, which draws the key's shadow hard.
    moving: bool,
    /// The projection the copy shades creases through, or `None` while it does not.
    occlusion: Option<core_geometry::Mat4>,
    /// The frame's view before any jitter, and which frame of the picture at rest it is.
    view_projection: core_geometry::Mat4,
    /// The lens a picture at rest is seen through, where the render view asks for one.
    lens: Option<settle::Lens>,
    sample: Option<u32>,
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
        let build = Builder::new(device, target_format, &globals_layout);
        let key = KeyShadow::new(device, build.key_cast());
        let environment = build.environment();
        let can_print = CanPrintTexture::new(device);
        let both = (&globals_layout, &globals);
        let textures = (&environment, &can_print);
        let globals_bind_group = key.globals_group(device, both, textures, false);
        let casting_bind_group = key.globals_group(device, both, textures, true);

        let solid = build.solid();
        let label = build.label();
        let relief = build.relief();
        let shadow = ContactShadow::new(device, build.shadowing());

        Self {
            globals,
            globals_bind_group,
            casting_bind_group,
            key,
            environment,
            can_print,
            settling: Settling::default(),
            models: solid.models,
            xray_pipeline: solid.xray,
            clear: build.clear(),
            backdrop: build.backdrop(),
            reads_planes: device.adapter_info().backend != wgpu::Backend::Gl,
            marker_pipelines: solid.markers,
            line_pipeline: solid.line,
            wide_line_pipeline: build.wide_line(),
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
            wide_lines: DynamicBuffer::new(device, "viewport_wide_lines"),
            cap: DynamicBuffer::new(device, "viewport_section_cap"),
            label: DynamicBuffer::new(device, "viewport_label"),
            meshes: HashMap::new(),
            frame: Frame::default(),
            target: SceneTarget::new(device, target_format),
            shadow,
            uploaded_faces: 0,
            shadow_baked: false,
            frame_log: FrameLog::default(),
            timer: timer::CardTimer::new(device),
            report: None,
            planned_faces: None,
            card_ms: None,
            globals_layout,
        }
    }

    /// Uploads one frame's camera, plate geometry and objects, and records what to draw.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: FrameInput<'_>) {
        self.uploaded_faces = 0;
        self.shadow_baked = false;
        self.read_card_times(device, queue);
        let globals = globals_of(&frame);
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        self.frame.view_projection = frame.view_projection;
        self.frame.lens = frame
            .look
            .filter(|look| look.aperture_mm > 0.0)
            .map(|look| settle::Lens {
                aperture_mm: look.aperture_mm,
                focus_mm: look.focus_mm,
                projection: look.projection,
            });
        self.frame.sample = self.settle(&frame, &globals);
        self.frame.clipped = globals.clips_models();
        self.write_geometry(device, queue, &frame);
        self.count_frame(&frame);
        self.record(device, queue, &frame);
        self.shadow.want(&frame);
        self.want_key_shadow(device, &frame);
        self.want_can_print(device, queue, &frame);
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
        self.wide_lines.write(device, queue, frame.wide_lines);
        let caps: Vec<LineVertex> = frame.cap.iter().chain(frame.bottom_cap).copied().collect();
        self.cap.write(device, queue, &caps);
        self.label.write(device, queue, frame.label);
        if let Some(atlas) = frame.atlas.as_ref() {
            self.upload_atlas(device, queue, atlas);
        }
        let instances: Vec<ModelInstance> = frame
            .models
            .iter()
            .chain(frame.markers)
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
        self.frame.wide_lines = frame.wide_lines.len() as u32;
        self.frame.label_vertices = frame.label.len() as u32;
        self.frame.cap_vertices = frame.cap.len() as u32;
        // A cap counts the crossings on its far side, which stand in front of it only for an
        // eye on that side; from the other, through a hole, the count reads backwards.
        self.frame.cutting = frame.section_mm.is_some_and(|mm| frame.eye.z > mm);
        self.frame.bottom_cap_vertices = frame.bottom_cap.len() as u32;
        self.frame.cutting_bottom = frame.bottom_cut_mm.is_some_and(|mm| frame.eye.z < mm);
        self.frame.xray = frame.xray;
        self.frame.staged = frame.look.is_some();
        self.frame.moving = frame.light;
        // While the camera moves a clear resin is drawn opaque: its four passes over every
        // model would leave the levels a quarter of the faces.
        self.frame.thick = self.reads_planes && !frame.xray && !frame.light && frame.look.is_some();
        self.frame.clear =
            self.frame.thick && frame.look.is_some_and(|look| look.attenuation_mm > 0.0);
        self.frame.occlusion = frame
            .look
            .filter(|_| self.reads_planes && !frame.light)
            .map(|look| look.projection);
        self.frame.markers_from = frame.models.len();
        let drawn = frame.models.len() + frame.markers.len();
        self.frame.solid_base = drawn as u32;
        self.frame.relief_base = (drawn + frame.solids.len()) as u32;
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
                .chain(frame.markers)
                .chain(frame.solids)
                .chain(cut_draws(frame)),
        );
        let kinds = [
            (frame.models, false),
            (frame.markers, false),
            (frame.solids, true),
        ];
        for (draws, solid) in kinds {
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
            draw.normals.as_deref(),
        );
        Drawn {
            key,
            mirrored: draw.instance.is_mirrored(),
            faces: draw.faces.clone(),
            opaque: draw.instance.is_opaque(),
        }
    }

    /// Sends on what the card took over the frames it has finished, to whoever asked.
    fn read_card_times(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some(timer) = &mut self.timer else {
            return;
        };
        for (card_s, faces) in timer.collect(device, queue) {
            self.card_ms = Some(card_s * 1000.0);
            if let (Some(report), Some(faces)) = (&self.report, faces) {
                // Nobody listening any more is no reason to stop drawing.
                let _ = report.send(CardTime { card_s, faces });
            }
        }
    }

    /// Where the card's time over each frame is sent from now on, and the models' faces
    /// this frame draws, or `None` for a frame not to be learnt from.
    pub fn report_to(&mut self, report: Option<Sender<CardTime>>, faces: Option<usize>) {
        self.report = report;
        self.planned_faces = faces;
    }

    /// Lets go of every mesh no frame has drawn for `KEEP_UNDRAWN`, and every texture
    /// nobody drew this frame: it was taken off the plate or replaced.
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
        let now = web_time::Instant::now();
        self.meshes.retain(|key, held| {
            if live.contains(key) {
                held.drawn_at = now;
            }
            now.duration_since(held.drawn_at) < KEEP_UNDRAWN
        });
        let textured: HashSet<usize> = self.frame.reliefs.iter().map(|(key, _)| *key).collect();
        self.reliefs.retain(|key, _| textured.contains(key));
    }

    /// Uploads a mesh the first time it is seen, or again once a draw asks for a part of
    /// it the pieces do not break at, and returns its cache key. `splits` are face indices
    /// no piece may straddle, so that a draw of part of the mesh is a whole number of
    /// pieces. A smooth upload serves a flat draw too, as the section's stencil only counts.
    fn cache(
        &mut self,
        device: &wgpu::Device,
        mesh: &Arc<Mesh>,
        splits: &[usize],
        normals: Option<&SmoothNormals>,
    ) -> usize {
        let key = Arc::as_ptr(mesh) as usize;
        let smooth = normals.is_some();
        if self
            .meshes
            .get(&key)
            .is_some_and(|held| held.splits == splits && (held.smooth || !smooth))
        {
            return key;
        }
        self.uploaded_faces += mesh.faces.len();
        self.meshes.insert(
            key,
            CachedMesh {
                _mesh: Arc::clone(mesh),
                pieces: pieces(device, mesh, splits, normals),
                splits: splits.to_vec(),
                smooth,
                drawn_at: web_time::Instant::now(),
            },
        );
        key
    }
}
