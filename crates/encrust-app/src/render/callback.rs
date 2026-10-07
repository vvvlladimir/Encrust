use std::sync::Arc;

use core_format::ExposureRange;
use core_geometry::{Mat4, Mesh, Scalar, Transform, Vec3};
use core_volume::{Channel, DrainHole, MOUTH_LIFT_MM};
use egui::Color32;
use egui::epaint::ViewportInPixels;
use egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};

use crate::camera::OrbitCamera;
use crate::plate::BuildPlate;
use crate::render::gpu::{
    CutLine, DrainCut, ExposureBand, FrameInput, MAX_BANDS, MAX_CUTS, MAX_POCKETS, ModelDraw,
    ReliefDraw, TrapBox,
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
    /// The panel the viewport fills, in points.
    rect: egui::Rect,
    view_projection: Mat4,
    eye: Vec3,
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
    /// The Cut tool's plane, traced over the model it is set on.
    pub cut_line: Option<CutLine>,
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
        let Shading {
            overhang_deg,
            section_mm,
            banding,
            textured,
            cut_line,
        } = shading;
        let Draws {
            models,
            solids,
            cuts,
            pockets,
            reliefs,
        } = Draws::of(scene, overhang_deg.map_or(NOT_MARKED, marking), textured);

        let (label, atlas) = label::front(plate);
        Self {
            rect,
            view_projection: camera.view_projection(rect.width() / rect.height()),
            eye: camera.eye(),
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
            pockets,
            reliefs,
            bands: bands_of(banding.ranges),
            band_floor_mm: banding.floor_mm,
            volume_mm: Vec3::new(plate.x_mm, plate.y_mm, plate.z_mm),
            cut_line,
            xray: view.xray,
        }
    }
}

/// What the objects on the plate come to on the GPU, gathered one object at a time.
#[derive(Default)]
struct Draws {
    models: Vec<ModelDraw>,
    solids: Vec<ModelDraw>,
    cuts: Vec<DrainCut>,
    pockets: Vec<TrapBox>,
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
        // Last, so that the resin inside one model is still seen when another stands in
        // front of it: nothing hides anything under the x-ray, but what is drawn later
        // washes over what came before.
        for object in scene.printable(scene.active_plate()) {
            draws.traps(object);
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
            // The cavity's own faces are declared even while nothing is trapped in it, so
            // that the frame which does paint them finds the mesh already cut to suit.
            None => self.models.push(match object.hollow.cavity_faces() {
                Some(cavity) => ModelDraw::whole_around(Arc::clone(&mesh), cavity, instance),
                None => ModelDraw::whole(Arc::clone(&mesh), instance),
            }),
        }

        // The cap is counted against the same mesh that is drawn, cavity and lattice
        // included: the inward-wound cavity subtracts itself from the count exactly as it
        // does from the fill rule, so the face is filled where there is material and left
        // open where the cut runs through the hollow.
        //
        // A surface with a hole in it has no inside for the count to be inside of, so a
        // model that is not sound is left out of it and gets no cap: an open model reads
        // as open rather than as a cap spilling out into the air beside it (ADR 0195).
        if object.summary.is_sound() {
            self.solid(mesh, object.transform);
        }
    }

    fn hollowing(&mut self, object: &SceneObject, colour: Color32) {
        // Counted but never drawn: a cut is wound inward, so it takes itself back out of
        // the count and the cap opens over the bore. See ADR 0074, 0075.
        if let Some(cuts) = object.hollow.cut_bodies()
            && object.summary.is_sound()
        {
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
    }

    /// The cavity of a model the last drainage check found resin in, painted in its own
    /// red where a pocket stands in it: that space fills with resin unless a hole is
    /// drilled into it. It is the shell's own cavity faces rather than a mesh of its own,
    /// so nothing is uploaded twice, and the x-ray never washes it down; which part of
    /// them paints is the shader's own test against the pockets. See ADR 0190, 0200.
    fn traps(&mut self, object: &SceneObject) {
        if object.traps.found().is_empty() {
            return;
        }
        let (Some(shell), Some(cavity)) = (object.hollow.shell(), object.hollow.cavity_faces())
        else {
            return;
        };
        self.pockets
            .extend(pockets_of(object.traps.found(), object.transform));
        self.pockets.truncate(MAX_POCKETS);
        self.models.push(ModelDraw::part(
            Arc::clone(shell),
            cavity,
            ModelInstance::new(object.transform, theme::scene().trapped, NOT_MARKED).as_volume(),
        ));
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
        self.models.push(ModelDraw::whole(
            mesh,
            ModelInstance::new(transform, colour, NOT_MARKED),
        ));
    }

    /// Counted into the stencil for the section cap, never drawn.
    fn solid(&mut self, mesh: Arc<Mesh>, transform: Transform) {
        self.solids.push(ModelDraw::whole(
            mesh,
            ModelInstance::new(transform, theme::scene().section_cap, NOT_MARKED),
        ));
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

/// The boxes of the pockets found in one model, carried onto the plate: a pocket travels
/// with the model it was found in, the way a hole does.
fn pockets_of(found: &[core_supports::Trapped], transform: Transform) -> Vec<TrapBox> {
    let matrix = transform.to_matrix();
    found
        .iter()
        .map(|pocket| {
            let mut placed = core_geometry::Aabb::new(
                matrix.transform_point3(pocket.bounds.mins),
                matrix.transform_point3(pocket.bounds.mins),
            );
            for index in 1..8 {
                let pick = |axis: usize| match index >> axis & 1 {
                    0 => pocket.bounds.mins[axis],
                    _ => pocket.bounds.maxs[axis],
                };
                placed.take_point(matrix.transform_point3(Vec3::new(pick(0), pick(1), pick(2))));
            }
            TrapBox {
                low: placed.mins.extend(0.0).to_array(),
                high: placed.maxs.extend(0.0).to_array(),
            }
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
        screen: &ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        super::with_resources(callback_resources, |resources| {
            resources.prepare(
                device,
                queue,
                FrameInput {
                    view_projection: self.view_projection,
                    eye: self.eye,
                    section_mm: self.section_mm,
                    lines: &self.lines,
                    models: &self.models,
                    solids: &self.solids,
                    cap: &self.cap,
                    body: &self.body,
                    label: &self.label,
                    atlas: self.atlas.clone(),
                    cuts: &self.cuts,
                    pockets: &self.pockets,
                    reliefs: &self.reliefs,
                    bands: &self.bands,
                    band_floor_mm: self.band_floor_mm,
                    volume_mm: Some(self.volume_mm),
                    cut_line: self.cut_line,
                    xray: self.xray,
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

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Bvh, Quat, diagnose};
    use core_supports::Trapped;

    use crate::scene::{ImportSummary, Imported};

    /// A ball hollowed the way the window hollows one, with resin reported trapped in it.
    fn a_hollowed_ball_with_resin_in_it() -> Scene {
        let mesh = Arc::new(core_volume::markers([(Vec3::splat(10.0), 10.0)]).expect("a ball"));
        let mut scene = Scene::default();
        scene.insert(Imported::new(
            "ball".to_owned(),
            Arc::clone(&mesh),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: core_geometry::Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));

        let object = &mut scene.objects_mut()[0];
        let bvh = Bvh::build(&mesh);
        let asking = object
            .hollow
            .asking(&crate::hollow::HollowTool::default().settings());
        let hollowed = core_volume::hollow(&mesh, &bvh, &asking).expect("a ball hollows");
        object.hollow.take(core_volume::Shell {
            mesh: Arc::new(hollowed.mesh),
            cavity: hollowed.cavity,
            cavity_mm3: hollowed.cavity_mm3,
            voxel_mm: hollowed.voxel_mm,
            coarsened: hollowed.coarsened,
            scale: Vec3::ONE,
            settings: asking,
        });
        object.traps.set(vec![Trapped {
            at: Vec3::splat(10.0),
            volume_mm3: 100.0,
            bounds: core_geometry::Aabb::new(Vec3::splat(-50.0), Vec3::splat(50.0)),
        }]);
        scene
    }

    /// The whole window path from a drainage check to the picture: a model the check found
    /// resin in has the faces of its own cavity drawn once more, in the colour of resin
    /// that cannot get out and in a way the x-ray will not wash down.
    #[test]
    fn a_model_with_resin_trapped_in_it_has_its_cavity_drawn_in_its_own_colour() {
        let mut scene = a_hollowed_ball_with_resin_in_it();
        let cavity = scene.objects()[0]
            .hollow
            .cavity_faces()
            .expect("the ball is hollow");
        assert!(!cavity.is_empty(), "and the cavity has faces of its own");

        let draws = Draws::of(&scene, NOT_MARKED, false);
        let painted = draws
            .models
            .iter()
            .filter(|draw| draw.faces() == cavity)
            .count();
        assert_eq!(painted, 1, "the cavity is drawn once over the shell");

        scene.objects_mut()[0].traps.set(Vec::new());
        let drained = Draws::of(&scene, NOT_MARKED, false);
        assert!(
            drained.models.iter().all(|draw| draw.faces() != cavity),
            "and not at all once the resin can get out"
        );
    }

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

    /// A box of ten triangles — a cube with the two of its top missing — on a plate of
    /// its own, which is a model that reaches the viewport open.
    fn an_open_box() -> Scene {
        let mesh = Arc::new(Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
        ));
        let mut scene = Scene::default();
        scene.insert(Imported::new(
            "open.stl".to_owned(),
            Arc::clone(&mesh),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: core_geometry::Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        scene
    }

    #[test]
    fn a_model_that_is_not_closed_is_drawn_but_never_counted_into_the_cap() {
        let scene = an_open_box();
        let draws = Draws::of(&scene, NOT_MARKED, false);

        assert_eq!(draws.models.len(), 1, "an open model is still drawn");
        assert!(
            draws.solids.is_empty(),
            "a surface with a hole has no inside for the cap to count"
        );
    }

    #[test]
    fn a_closed_model_is_counted_into_the_cap() {
        let mut scene = an_open_box();
        let id = scene.objects()[0].id;
        let mut mesh = (*scene.get(id).expect("the box is there").mesh).clone();
        core_geometry::fill_holes(&mut mesh);
        scene
            .get_mut(id)
            .expect("the box is there")
            .reshape(Arc::new(mesh));

        assert_eq!(Draws::of(&scene, NOT_MARKED, false).solids.len(), 1);
    }

    #[test]
    fn selection_outranks_the_repair_warning() {
        assert_eq!(color_for(true, false), theme::scene().selected);
        assert_eq!(color_for(false, false), theme::scene().unsound);
        assert_eq!(color_for(false, true), theme::scene().object);
    }
}
