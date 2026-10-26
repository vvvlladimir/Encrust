use std::sync::Arc;
use std::sync::mpsc::Sender;

use core_format::ExposureRange;
use core_geometry::{Mat4, Mesh, Scalar, Transform, Vec2, Vec3};
use core_volume::DrainHole;
use egui::Color32;
use egui::epaint::ViewportInPixels;
use egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};

use crate::camera::OrbitCamera;
use crate::plate::BuildPlate;
use crate::relief::Textures;
use crate::render::frame_time::CardTime;
use crate::render::gpu::{
    CutDraw, CutLine, DrainCut, ExposureBand, Floor, FrameInput, MAX_BANDS, ModelDraw, ReliefDraw,
    TrapBox,
};
use crate::render::grid::{plate_lines, section_frame};
use crate::render::label;
use crate::render::lod::Light;
use crate::render::vertex::{LabelVertex, LineVertex, NOT_MARKED, WideLine};
use crate::scene::{ObjectId, Scene};
use crate::ui::theme;
use crate::workspace::ViewOptions;

/// How far each support group is drawn pulled towards the eye over the one after it, as a
/// share of its distance: pads of two groups, or a pad and the raft, lie in one plane, and
/// the lower group wins it. About sixteen steps of a 24-bit depth at the orbit distance.
const GROUP_PULL: f32 = 1e-4;

/// Millimetres a cut is drawn inside what it cuts: one level with a face — the top of a
/// raft, the floor of a step — would leave that face and the cap fighting for one depth.
const SECTION_INSET_MM: Scalar = 1e-3;

/// Millimetres the cap quad is grown past what it has to cover. It is clipped by the
/// stencil, so the only cost of a generous one is a few more fragments discarded.
const CAP_MARGIN_MM: f32 = 10.0;

/// One frame of the viewport, handed to egui and replayed inside its render pass.
///
/// It carries plain data rather than a borrow of the scene: egui runs `prepare` and
/// `paint` after the UI closure has returned, when the app state is borrowed elsewhere.
pub struct ViewportCallback {
    /// The panel the viewport fills, in points.
    rect: egui::Rect,
    view_projection: Mat4,
    eye: Vec3,
    section_mm: Option<Scalar>,
    bottom_cut_mm: Option<Scalar>,
    lines: Lines,
    models: Vec<ModelDraw>,
    markers: Vec<ModelDraw>,
    /// What the cut is capped against: the same solids that are drawn, counted into the
    /// stencil plane so that the cap covers the material the plane runs through.
    solids: Vec<ModelDraw>,
    cap: Vec<LineVertex>,
    bottom_cap: Vec<LineVertex>,
    /// The word lying in front of the plate, and the font atlas it samples.
    label: Vec<LabelVertex>,
    atlas: Option<std::sync::Arc<egui::ColorImage>>,
    /// The holes already cut into the models being drawn, in plate millimetres. The
    /// meshes still carry every triangle they had, so the shader is what subtracts them;
    /// see `docs/decisions/0073`.
    cuts: Vec<DrainCut>,
    /// The bodies of those cuts with the objects they are cut into: what the inside of a
    /// hole is drawn from, in place of a mesh of its own; see ADR 0201.
    cut_surfaces: Vec<CutDraw>,
    /// Where the last drainage check found resin with no way out, in plate millimetres:
    /// the cavity is painted red inside these boxes and nowhere else; see ADR 0200.
    pockets: Vec<TrapBox>,
    /// The models drawn with their own texture on them: only the Relief tool asks for it,
    /// and only a model that still carries one can be drawn that way.
    reliefs: Vec<ReliefDraw>,
    bands: Vec<ExposureBand>,
    band_floor_mm: f32,
    volume_mm: Vec3,
    cut_line: Option<CutLine>,
    /// Whether the models are drawn seen through, so a cavity that holds resin can be
    /// looked into; see `docs/design/viewport.md`.
    xray: bool,
    /// Whether the plate's grid is drawn into it.
    grid: bool,
    /// Whether the models' levels are still changing from frame to frame.
    light: bool,
    /// The plate the models cast their shadow on.
    floor: Floor,
    /// How long the window's pass over its panels took on the frame before, milliseconds.
    ui_ms: f32,
    /// Where the card's time over each frame goes, with the models' faces it drew.
    report: Option<Sender<CardTime>>,
    faces: Option<usize>,
}

/// Lines in plate millimetres: the plate's own, one pixel across, and the edges
/// `highlighting` marks over the models, wider.
struct Lines {
    thin: Vec<LineVertex>,
    wide: Vec<WideLine>,
}

impl Lines {
    /// The plate's outline, grid and volume, with its outline again at each cut height.
    fn of_plate(plate: &BuildPlate, cuts_mm: impl Iterator<Item = Scalar>) -> Self {
        let mut thin = plate_lines(plate);
        for height in cuts_mm {
            thin.extend(section_frame(plate, height));
        }
        Self {
            thin,
            wide: Vec::new(),
        }
    }
}

/// The exposure bands washed over the models, and the height below which one shows
/// nothing because the bottom block keeps the resin's own ramp; see ADR 0090.
#[derive(Clone, Copy, Default)]
pub struct Banding<'a> {
    pub ranges: &'a [ExposureRange],
    pub floor_mm: Scalar,
}

/// How the models are painted this frame, as opposed to where they stand.
#[derive(Clone, Copy, Default)]
pub struct Shading<'a> {
    /// The angle past which a surface is washed with the overhang colour, or `None` while
    /// nothing is asking about supports.
    pub overhang_deg: Option<f32>,
    /// The height the models are cut at, or `None` to draw them whole.
    pub section_mm: Option<Scalar>,
    /// The height below which they are cut away, or `None` to draw them down to the plate.
    pub bottom_cut_mm: Option<Scalar>,
    pub banding: Banding<'a>,
    /// The texture each model is drawn with, which is what the Relief tool shows: the
    /// picture it would press, or the one the model carries. `None` draws every model flat.
    pub textures: Option<&'a Textures>,
    /// The Cut tool's plane, traced over the model it is set on.
    pub cut_line: Option<CutLine>,
    /// The support groups left out of the view, by group number.
    pub hidden_groups: &'a [bool],
    /// What a click under the resting cursor would put down, or `None`.
    pub ghost: Option<&'a Ghost>,
    /// The lighter levels drawn in place of heavy meshes.
    pub light: Option<&'a Light>,
    /// One model drawn as another mesh: the Repair tool's, with some shells hidden.
    pub stand_in: Option<(ObjectId, &'a Arc<Mesh>)>,
}

impl ViewportCallback {
    /// `rect` is the panel the viewport fills, in points.
    pub fn new(
        scene: &Scene,
        plate: &BuildPlate,
        camera: &OrbitCamera,
        view: ViewOptions,
        shading: Shading<'_>,
        rect: egui::Rect,
    ) -> Self {
        let draws = drawn(scene, &shading);
        let Shading {
            section_mm,
            bottom_cut_mm,
            banding,
            cut_line,
            light,
            ..
        } = shading;
        let section_mm = section_mm.map(|height| height - SECTION_INSET_MM);
        let bottom_cut_mm = bottom_cut_mm.map(|height| height + SECTION_INSET_MM);
        let Draws {
            models,
            markers,
            solids,
            cuts,
            cut_surfaces,
            pockets,
            reliefs,
        } = draws;

        let (label, atlas) = label::front(plate);
        Self {
            rect,
            view_projection: camera.view_projection(rect.width() / rect.height()),
            eye: camera.eye(),
            section_mm,
            bottom_cut_mm,
            lines: Lines::of_plate(plate, section_mm.into_iter().chain(bottom_cut_mm)),
            models,
            markers,
            solids,
            cap: section_mm
                .map(|height| cap_quad(scene, plate, height))
                .unwrap_or_default(),
            bottom_cap: bottom_cut_mm
                .map(|height| cap_quad(scene, plate, height))
                .unwrap_or_default(),
            label,
            atlas,
            cuts,
            cut_surfaces,
            pockets,
            reliefs,
            bands: bands_of(banding.ranges),
            band_floor_mm: banding.floor_mm,
            volume_mm: Vec3::new(plate.x_mm, plate.y_mm, plate.z_mm),
            cut_line,
            xray: view.xray,
            grid: view.grid,
            light: light.is_some_and(|light| light.settling),
            floor: Floor {
                near_corner_mm: Vec2::ZERO,
                size_mm: Vec2::new(plate.x_mm, plate.y_mm),
            },
            ui_ms: 0.0,
            report: None,
            faces: None,
        }
    }

    /// The same frame, logged with what the window's last pass over its panels took, and
    /// with the card's time over it sent to `report` beside the models' `faces` it draws,
    /// or not sent where `faces` is `None`.
    pub fn timed(mut self, ui_ms: f32, report: Sender<CardTime>, faces: Option<usize>) -> Self {
        self.ui_ms = ui_ms;
        self.report = Some(report);
        self.faces = faces;
        self
    }
}

/// What the objects on the plate come to this frame, the ghost of a click included.
fn drawn(scene: &Scene, shading: &Shading<'_>) -> Draws {
    let mut draws = Draws::of(
        scene,
        shading.overhang_deg.map_or(NOT_MARKED, marking),
        shading.textures,
        shading.hidden_groups,
        shading.light,
        shading.stand_in,
    );
    if let Some(ghost) = shading.ghost {
        draws.ghost(scene, shading.light, ghost);
    }
    draws
}

/// What a click is about to put down, drawn over the plate in `colour` and never capped:
/// it is not part of the print yet.
#[derive(Clone)]
pub struct Ghost {
    pub shape: GhostShape,
    pub colour: Color32,
}

/// What a ghost is: a body drawn as it stands, or a hole drawn cut.
#[derive(Clone)]
pub enum GhostShape {
    /// A body standing on a model: a support, a blocker, the point of a channel.
    /// `mesh` is placed by `transform`: the identity for one already in plate coordinates.
    Body {
        mesh: Arc<Mesh>,
        transform: Transform,
    },
    /// A hole about to be drilled into object `id`, in its own space, cut the way a
    /// drilled one is; `into` is the support group it goes into, in plate coordinates, where
    /// it is not the model.
    Hole {
        id: ObjectId,
        hole: DrainHole,
        into: Option<Arc<Mesh>>,
    },
}

/// The bands the shader washes the models with, each in the tint its own fields carry.
/// Past `MAX_BANDS` a band is left untinted: only the picture is short, never the print.
fn bands_of(ranges: &[ExposureRange]) -> Vec<ExposureBand> {
    ranges
        .iter()
        .take(MAX_BANDS)
        .enumerate()
        .map(|(index, range)| {
            ExposureBand::new(range.from_mm, range.to_mm, theme::band_tint(index))
        })
        .collect()
}

/// Two triangles lying in the cutting plane, covering everything that could be cut. The
/// stencil decides which of their fragments survive, so the quad only has to be big
/// enough, never exact.
fn cap_quad(scene: &Scene, plate: &BuildPlate, height_mm: Scalar) -> Vec<LineVertex> {
    let (mut min_x, mut min_y) = (0.0_f32, 0.0_f32);
    let (mut max_x, mut max_y) = (plate.x_mm, plate.y_mm);
    if let Some(bounds) = scene.world_bounds() {
        min_x = min_x.min(bounds.mins.x);
        min_y = min_y.min(bounds.mins.y);
        max_x = max_x.max(bounds.maxs.x);
        max_y = max_y.max(bounds.maxs.y);
    }
    let corner = |x: f32, y: f32| {
        LineVertex::new(
            Vec3::new(x - CAP_MARGIN_MM, y - CAP_MARGIN_MM, height_mm),
            theme::scene().section_cap,
        )
    };
    let (right, top) = (max_x + 2.0 * CAP_MARGIN_MM, max_y + 2.0 * CAP_MARGIN_MM);
    vec![
        corner(min_x, min_y),
        corner(right, min_y),
        corner(right, top),
        corner(min_x, min_y),
        corner(right, top),
        corner(min_x, top),
    ]
}

/// The angle a surface may lean from vertical, as the sine the shader compares against.
/// A wall is zero and a ceiling one, which is what a face normal's downward part reads.
fn marking(overhang_deg: f32) -> f32 {
    overhang_deg.to_radians().sin().clamp(0.0, 1.0)
}

impl CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let started = web_time::Instant::now();
        super::with_resources(callback_resources, |resources| {
            resources.report_to(self.report.clone(), self.faces);
            resources.prepare(
                device,
                queue,
                FrameInput {
                    view_projection: self.view_projection,
                    eye: self.eye,
                    section_mm: self.section_mm,
                    lines: &self.lines.thin,
                    wide_lines: &self.lines.wide,
                    models: &self.models,
                    markers: &self.markers,
                    solids: &self.solids,
                    cap: &self.cap,
                    bottom_cut_mm: self.bottom_cut_mm,
                    bottom_cap: &self.bottom_cap,
                    label: &self.label,
                    atlas: self.atlas.clone(),
                    cuts: &self.cuts,
                    cut_surfaces: &self.cut_surfaces,
                    pockets: &self.pockets,
                    reliefs: &self.reliefs,
                    bands: &self.bands,
                    band_floor_mm: self.band_floor_mm,
                    volume_mm: Some(self.volume_mm),
                    cut_line: self.cut_line,
                    xray: self.xray,
                    floor: Some(self.floor),
                    grid: self.grid,
                    light: self.light,
                },
            );
            // The same pixels egui hands `paint` its viewport for, so the copy lands
            // exactly where the scene was drawn.
            let viewport = ViewportInPixels::from_points(
                &self.rect,
                screen.pixels_per_point,
                screen.size_in_pixels,
            );
            resources.draw(device, egui_encoder, screen.size_in_pixels, viewport);
            resources.log_frame(started, self.ui_ms);
        });
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &CallbackResources,
    ) {
        super::resources(callback_resources, |resources| {
            resources.present(render_pass)
        });
    }
}

mod draws;
mod highlight;

use draws::Draws;
#[cfg(test)]
pub(crate) use draws::cuts_of;

#[cfg(test)]
mod tests;
