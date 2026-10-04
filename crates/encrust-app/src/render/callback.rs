use std::sync::Arc;

use core_format::ExposureRange;
use core_geometry::{Mat4, Mesh, Scalar, Transform, Vec3};
use core_volume::{Channel, DrainHole, MOUTH_LIFT_MM};
use egui::Color32;
use egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};

use crate::camera::OrbitCamera;
use crate::plate::BuildPlate;
use crate::render::gpu::{
    DrainCut, ExposureBand, FrameInput, MAX_BANDS, MAX_CUTS, ModelDraw, ReliefDraw,
};
use crate::render::grid::plate_lines;
use crate::render::label;
use crate::render::machine::machine_faces;
use crate::render::vertex::{BodyVertex, LabelVertex, LineVertex, ModelInstance, NOT_MARKED};
use crate::scene::{Scene, SceneObject};
use crate::ui::theme;
use crate::workspace::ViewOptions;

/// Millimetres the cap quad is grown past what it has to cover. It is clipped by the
/// stencil, so the only cost of a generous one is a few more fragments discarded.
const CAP_MARGIN_MM: f32 = 10.0;

/// One frame of the viewport, handed to egui and replayed inside its render pass.
///
/// It carries plain data rather than a borrow of the scene: egui runs `prepare` and
/// `paint` after the UI closure has returned, when the app state is borrowed elsewhere.
pub struct ViewportCallback {
    view_projection: Mat4,
    section_mm: Option<Scalar>,
    lines: Vec<LineVertex>,
    models: Vec<ModelDraw>,
    /// What the cut is capped against: the same solids that are drawn, counted into the
    /// stencil plane so that the cap covers the material the plane runs through.
    solids: Vec<ModelDraw>,
    cap: Vec<LineVertex>,
    /// The machine under the plate: the platform and the arm it hangs from.
    body: Vec<BodyVertex>,
    /// The word lying on the platform, and the font atlas it samples.
    label: Vec<LabelVertex>,
    atlas: Option<std::sync::Arc<egui::ColorImage>>,
    /// The holes already cut into the models being drawn, in plate millimetres. The
    /// meshes still carry every triangle they had, so the shader is what subtracts them;
    /// see `docs/decisions/0073`.
    cuts: Vec<DrainCut>,
    /// The models drawn with their own texture on them: only the Relief tool asks for it,
    /// and only a model that still carries one can be drawn that way.
    reliefs: Vec<ReliefDraw>,
    bands: Vec<ExposureBand>,
    band_floor_mm: f32,
    volume_mm: Vec3,
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
    pub banding: Banding<'a>,
    /// Whether a model still carrying a texture is drawn with it, which is what the Relief
    /// tool shows.
    pub textured: bool,
}

impl ViewportCallback {
    /// `aspect` is the width of the viewport rectangle over its height.
    pub fn new(
        scene: &Scene,
        plate: &BuildPlate,
        camera: &OrbitCamera,
        view: ViewOptions,
        shading: Shading<'_>,
        aspect: f32,
    ) -> Self {
        let Shading {
            overhang_deg,
            section_mm,
            banding,
            textured,
        } = shading;
        let Draws {
            models,
            solids,
            cuts,
            reliefs,
        } = Draws::of(scene, overhang_deg.map_or(NOT_MARKED, marking), textured);

        let (label, atlas) = label::front(plate);
        Self {
            view_projection: camera.view_projection(aspect),
            section_mm,
            lines: plate_lines(plate, view.grid),
            models,
            solids,
            cap: section_mm
                .map(|height| cap_quad(scene, plate, height))
                .unwrap_or_default(),
            body: machine_faces(plate),
            label,
            atlas,
            cuts,
            reliefs,
            bands: bands_of(banding.ranges),
            band_floor_mm: banding.floor_mm,
            volume_mm: Vec3::new(plate.x_mm, plate.y_mm, plate.z_mm),
        }
    }
}

/// What the objects on the plate come to on the GPU, gathered one object at a time.
#[derive(Default)]
struct Draws {
    models: Vec<ModelDraw>,
    solids: Vec<ModelDraw>,
    cuts: Vec<DrainCut>,
    reliefs: Vec<ReliefDraw>,
}

impl Draws {
    /// `mark` is the overhang threshold the models are washed at, or `NOT_MARKED`.
    fn of(scene: &Scene, mark: f32, textured: bool) -> Self {
        let mut draws = Self::default();
        for object in scene.printable(scene.active_plate()) {
            let colour = color_for(scene.is_selected(object.id), object.summary.is_sound());
            draws.model(object, colour, mark, textured);
            draws.hollowing(object, colour);
            draws.supports(object);
        }
        draws
    }

    fn model(&mut self, object: &SceneObject, colour: Color32, mark: f32, textured: bool) {
        let (drains, channels) = object.hollow.cut();
        self.cuts
            .extend(cuts_of(drains, channels, object.transform));
        // A hollowed model is drawn as its shell: the cavity faces inward, so it is
        // culled from outside and only shows once the camera is in the wall.
        let mesh = object.hollow.shell().unwrap_or(&object.mesh).clone();
        let instance = ModelInstance::new(object.transform, colour, mark);
        // The map is indexed by the model's own faces, so a hollowed model — whose shell
        // carries the cavity's as well — is drawn flat like everything else.
        let mapped = object
            .mapped
            .as_ref()
            .filter(|_| textured && object.hollow.shell().is_none());
        match mapped {
            Some(mapped) => self.reliefs.push(ReliefDraw {
                mesh: Arc::clone(&mesh),
                mapped: Arc::clone(mapped),
                instance,
            }),
            None => self.models.push(ModelDraw {
                mesh: Arc::clone(&mesh),
                instance,
            }),
        }

        // The cap is counted against the same mesh that is drawn, cavity and lattice
        // included: the inward-wound cavity subtracts itself from the count exactly as it
        // does from the fill rule, so the face is filled where there is material and left
        // open where the cut runs through the hollow.
        self.solid(mesh, object.transform);
    }

    fn hollowing(&mut self, object: &SceneObject, colour: Color32) {
        // Counted but never drawn: a cut is wound inward, so it takes itself back out of
        // the count and the cap opens over the bore. See ADR 0074, 0075.
        if let Some(cuts) = object.hollow.cut_bodies() {
            self.solid(Arc::clone(cuts), object.transform);
        }
        // The inside of every hole, drawn like the model and never counted: it is what
        // the fragment test leaves a window into. See ADR 0073.
        if let Some(bore) = object.hollow.bore() {
            self.flat(Arc::clone(bore), object.transform, colour);
        }
        // Blockers are kept in the model's own space, so they ride its placement.
        if let Some(markers) = object.hollow.markers() {
            self.flat(
                Arc::clone(markers),
                object.transform,
                theme::scene().blocker,
            );
        }
        // The pockets the last drainage check found, marked the same way.
        if let Some(traps) = object.traps.markers() {
            self.flat(Arc::clone(traps), object.transform, theme::scene().blocker);
        }
    }

    /// Patches and columns are already in plate coordinates: they were built against the
    /// placed model, not alongside it.
    fn supports(&mut self, object: &SceneObject) {
        let (painted, blocked) = object.supports.patches();
        for (patch, colour) in [
            (painted, theme::scene().painted),
            (blocked, theme::scene().blocked),
        ] {
            if let Some(patch) = patch {
                self.flat(Arc::clone(patch), Transform::default(), colour);
            }
        }

        let groups = object.supports.meshes().unwrap_or_default();
        for (group, mesh) in groups
            .iter()
            .enumerate()
            .filter(|(_, mesh)| !mesh.is_empty())
        {
            // A column holds nothing up but the model, so marking its own overhangs would
            // only paint the scaffolding red.
            self.flat(
                Arc::clone(mesh),
                Transform::default(),
                theme::support_tint(group),
            );
            self.solid(Arc::clone(mesh), Transform::default());
        }
    }

    /// Drawn in one flat colour and never marked for overhangs.
    fn flat(&mut self, mesh: Arc<Mesh>, transform: Transform, colour: Color32) {
        self.models.push(ModelDraw {
            mesh,
            instance: ModelInstance::new(transform, colour, NOT_MARKED),
        });
    }

    /// Counted into the stencil for the section cap, never drawn.
    fn solid(&mut self, mesh: Arc<Mesh>, transform: Transform) {
        self.solids.push(ModelDraw {
            mesh,
            instance: ModelInstance::new(transform, theme::scene().section_cap, NOT_MARKED),
        });
    }
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

/// The holes and channels of one model, in plate millimetres, ready for the shader.
///
/// A channel is handed over a segment at a time with a ball at each joint, which is the
/// same union of capsules `core-volume` meshes it as. Past `MAX_CUTS` a plate is drawn
/// with its later holes uncut: only the picture is short, never the print.
pub(crate) fn cuts_of(
    drains: &[DrainHole],
    channels: &[Channel],
    transform: Transform,
) -> Vec<DrainCut> {
    let matrix = transform.to_matrix();
    let scale = transform.scale.abs();
    let smallest = scale.x.min(scale.y).min(scale.z);
    let place = |point: Vec3, radius_mm: Scalar| {
        let at = matrix.transform_point3(point);
        [at.x, at.y, at.z, radius_mm * smallest]
    };

    let mut cuts = Vec::new();
    for hole in drains {
        let Some(axis) = hole.axis.try_normalize() else {
            continue;
        };
        let radius = hole.diameter_mm / 2.0;
        cuts.push(DrainCut {
            mouth: place(hole.at - axis * hole.lift_mm.max(MOUTH_LIFT_MM), radius),
            tip: place(hole.at + axis * hole.depth_mm, radius * hole.taper),
        });
    }
    for channel in channels {
        let radius = channel.diameter_mm / 2.0;
        for pair in channel.points.windows(2) {
            let mut cut = DrainCut {
                mouth: place(pair[0], radius),
                tip: place(pair[1], radius),
            };
            // A channel is open at both ends and round at its joints, which the shader
            // reads off the sign of the far radius.
            cut.tip[3] = -cut.tip[3];
            cuts.push(cut);
        }
    }
    cuts.truncate(MAX_CUTS);
    cuts
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

fn color_for(selected: bool, sound: bool) -> Color32 {
    match (selected, sound) {
        (true, _) => theme::scene().selected,
        (false, true) => theme::scene().object,
        (false, false) => theme::scene().unsound,
    }
}

impl CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        super::with_resources(callback_resources, |resources| {
            resources.prepare(
                device,
                queue,
                FrameInput {
                    view_projection: self.view_projection,
                    section_mm: self.section_mm,
                    lines: &self.lines,
                    models: &self.models,
                    solids: &self.solids,
                    cap: &self.cap,
                    body: &self.body,
                    label: &self.label,
                    atlas: self.atlas.clone(),
                    cuts: &self.cuts,
                    reliefs: &self.reliefs,
                    bands: &self.bands,
                    band_floor_mm: self.band_floor_mm,
                    volume_mm: Some(self.volume_mm),
                },
            );
        });
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &CallbackResources,
    ) {
        super::resources(callback_resources, |resources| resources.paint(render_pass));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Quat;

    fn hole() -> DrainHole {
        DrainHole {
            at: Vec3::new(0.0, 0.0, 10.0),
            axis: Vec3::NEG_Z,
            diameter_mm: 4.0,
            depth_mm: 6.0,
            taper: 1.0,
            lift_mm: 0.0,
        }
    }

    #[test]
    fn a_cut_stands_where_the_model_it_was_drilled_in_stands() {
        let moved = Transform::from_translation(Vec3::new(50.0, 20.0, 0.0));
        let cuts = cuts_of(&[hole()], &[], moved);

        assert_eq!(cuts.len(), 1);
        assert!((cuts[0].mouth[0] - 50.0).abs() < 1e-4);
        assert!((cuts[0].mouth[1] - 20.0).abs() < 1e-4);
        assert!(
            cuts[0].mouth[2] > 10.0 && (cuts[0].tip[2] - 4.0).abs() < 1e-4,
            "the mouth stands clear of the surface and the tip is a depth below it"
        );
        assert!((cuts[0].mouth[3] - 2.0).abs() < 1e-4, "half the diameter");
    }

    #[test]
    fn a_cut_turns_and_scales_with_its_model() {
        let placed = Transform {
            rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };
        let cuts = cuts_of(&[hole()], &[], placed);

        // A quarter turn about x maps the model's own +z onto the plate's -y, doubled.
        assert!(
            (cuts[0].mouth[1] + 20.0).abs() < 0.2,
            "got {:?}",
            cuts[0].mouth
        );
        assert!(
            (cuts[0].mouth[3] - 4.0).abs() < 1e-4,
            "the radius doubles too"
        );
    }

    #[test]
    fn a_channel_is_handed_over_a_segment_at_a_time() {
        let channel = Channel {
            points: vec![
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, 5.0),
                Vec3::new(4.0, 0.0, 5.0),
            ],
            diameter_mm: 2.0,
        };
        let cuts = cuts_of(&[], &[channel], Transform::default());

        assert_eq!(cuts.len(), 2, "two segments between three points");
        assert!((cuts[1].tip[0] - 4.0).abs() < 1e-4);
    }

    #[test]
    fn a_plate_past_the_limit_is_drawn_with_its_later_holes_uncut() {
        let many: Vec<DrainHole> = (0..MAX_CUTS + 10).map(|_| hole()).collect();
        assert_eq!(cuts_of(&many, &[], Transform::default()).len(), MAX_CUTS);
    }

    #[test]
    fn a_wall_is_never_marked_and_a_ceiling_always_is() {
        assert!(marking(0.0).abs() < 1e-6, "a wall leans nowhere");
        assert!(
            (marking(90.0) - 1.0).abs() < 1e-6,
            "a ceiling leans all the way"
        );
        assert!(
            (marking(45.0) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6,
            "a 45 degree surface marks at the sine of 45 degrees"
        );
        assert!(marking(45.0) < marking(46.0), "a slacker angle marks less");
    }

    #[test]
    fn selection_outranks_the_repair_warning() {
        assert_eq!(color_for(true, false), theme::scene().selected);
        assert_eq!(color_for(false, false), theme::scene().unsound);
        assert_eq!(color_for(false, true), theme::scene().object);
    }
}
