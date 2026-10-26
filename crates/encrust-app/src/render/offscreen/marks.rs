use super::SECTION_MM;
use super::holes::{drilled_ball, hollowed_ball};
use super::paint::{paint, paint_after};
use crate::camera::OrbitCamera;
use crate::render::gpu::{CutDraw, CutLine, DrainCut, ExposureBand, Floor, ModelDraw, TrapBox};
use crate::render::vertex::{LineVertex, ModelInstance, NOT_MARKED, WideLine};
use crate::ui::theme;
use core_geometry::{Mesh, Transform, Vec3};
use core_volume::CUT_WEIGHT;
use std::sync::Arc;

/// The ball of [`drilled_ball`] undrilled and uncut, painted with `bands`.
pub(super) fn paint_ball(
    bands: &[ExposureBand],
    band_floor_mm: f32,
    volume_mm: Option<Vec3>,
) -> Option<Vec<u8>> {
    paint_marked_ball(Marks {
        bands,
        band_floor_mm,
        volume_mm,
        ..Marks::default()
    })
}

/// The ball of [`drilled_ball`] undrilled and uncut, painted with `marks`.
pub(super) fn paint_marked_ball(marks: Marks<'_>) -> Option<Vec<u8>> {
    paint_ball_around(Vec::new(), marks)
}

/// The same ball with `inside` drawn in it, which is where the trapped resin shows.
pub(super) fn paint_ball_around(inside: Vec<ModelDraw>, marks: Marks<'_>) -> Option<Vec<u8>> {
    paint_ball_cached(inside, marks, false)
}

/// The same again, and when `cached` the ball alone is drawn on an earlier frame so that
/// the mesh is already on the card when the one carrying `inside` arrives. That is the
/// order the window draws in: a shell is on screen long before the drainage check says
/// what is trapped in it.
pub(super) fn paint_ball_cached(
    inside: Vec<ModelDraw>,
    marks: Marks<'_>,
    cached: bool,
) -> Option<Vec<u8>> {
    let (mesh, _, _) = drilled_ball(false);
    let ball = || {
        ModelDraw::whole(
            Arc::clone(&mesh),
            ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED),
        )
    };
    let mut models = vec![ball()];
    models.extend(inside);
    let plain = [ball()];
    let before: &[&[ModelDraw]] = match cached {
        true => &[&plain],
        false => &[],
    };
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 45.0),
        distance_mm: 170.0,
        ..OrbitCamera::default()
    };
    paint_after(camera, None, before, &models, &[], marks)
}

/// A blocker standing in the material under the cut shows through the cap, and one inside
/// a model that is not cut stays hidden by its wall.
#[test]
fn a_marker_inside_the_material_shows_through_the_cut_and_nowhere_else() {
    let solid = Arc::new(hollowed_ball().0.clone());
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let cap_colour =
        ModelInstance::new(Transform::default(), theme::scene().section_cap, NOT_MARKED);
    let models = [ModelDraw::whole(Arc::clone(&solid), object)];
    let solids = [ModelDraw::whole(Arc::clone(&solid), cap_colour)];
    // Wholly under the cut at 50 mm and wholly inside the ball of 40 mm about z 45.
    let blocker = core_volume::markers([(Vec3::new(75.0, 40.0, 40.0), 8.0)]).expect("a ball");
    let markers = [ModelDraw::whole(
        Arc::new(blocker),
        ModelInstance::new(Transform::default(), theme::scene().blocker, NOT_MARKED),
    )];
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 45.0),
        distance_mm: 170.0,
        ..OrbitCamera::default()
    };
    let frame = |section_mm, markers| {
        let marks = Marks {
            markers,
            ..Marks::default()
        };
        paint(camera, section_mm, &models, &solids, marks)
    };
    let Some(capped) = frame(Some(SECTION_MM), &[]) else {
        return;
    };
    let changed = |a: &[u8], b: &[u8]| {
        a.as_chunks::<4>()
            .0
            .iter()
            .zip(b.as_chunks::<4>().0)
            .filter(|(a, b)| a != b)
            .count()
    };

    let marked = frame(Some(SECTION_MM), &markers).expect("the adapter drew once");
    assert!(
        changed(&capped, &marked) > 0,
        "the blocker under the cap is hidden by it"
    );
    let whole = frame(None, &[]).expect("the adapter drew once");
    let whole_marked = frame(None, &markers).expect("the adapter drew once");
    assert_eq!(
        changed(&whole, &whole_marked),
        0,
        "a blocker inside a model that is not cut shows through its wall"
    );
}

/// The inside of the cuts `bodies` take out of `mesh`, the way the window hands them over:
/// one copy of the bodies, which are laid `CUT_WEIGHT` deep for the slicer.
pub(super) fn cut_surfaces(
    mesh: &Arc<Mesh>,
    bodies: &Arc<Mesh>,
    instance: ModelInstance,
) -> Vec<CutDraw> {
    if bodies.faces.is_empty() {
        return Vec::new();
    }
    vec![CutDraw {
        body: ModelDraw::part(
            Arc::clone(bodies),
            0..bodies.faces.len() / CUT_WEIGHT,
            instance,
        ),
        solid: ModelDraw::whole(Arc::clone(mesh), instance),
    }]
}

/// What the shader lays over the models besides their own colour.
#[derive(Clone, Copy, Default)]
pub(super) struct Marks<'a> {
    /// What a tool put on the models, drawn after the cap.
    pub(super) markers: &'a [ModelDraw],
    pub(super) cuts: &'a [DrainCut],
    /// The bodies those cuts take out, with the objects they are cut into: what the inside
    /// of a hole is drawn from. See ADR 0201.
    pub(super) cut_surfaces: &'a [CutDraw],
    /// Where the drainage check found resin, which is the only place a volume paints.
    pub(super) pockets: &'a [TrapBox],
    pub(super) bands: &'a [ExposureBand],
    pub(super) band_floor_mm: f32,
    pub(super) volume_mm: Option<Vec3>,
    pub(super) cut_line: Option<CutLine>,
    /// Whether the models are drawn seen through; see `docs/design/viewport.md`.
    pub(super) xray: bool,
    pub(super) floor: Option<Floor>,
    /// Lines in plate millimetres, as the plate's outline and an open edge are drawn.
    pub(super) lines: &'a [LineVertex],
    /// Lines drawn wider than those, as a tool marks an edge.
    pub(super) wide_lines: &'a [WideLine],
}
