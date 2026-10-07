//! Frames drawn offscreen and looked at, for what only the picture can answer: a cap
//! painted where the cutting plane misses the model (ADR 0074), and an exposure band
//! washed over the height it does not cover (ADR 0090).

use std::sync::{Arc, OnceLock};

use core_geometry::{Bvh, Heightmap, Mesh, Scalar, Transform, UvMap, Vec2, Vec3};
use core_volume::{DrainHole, HollowSettings, bores, drill, hollow};

use crate::camera::OrbitCamera;
use crate::plate::BuildPlate;
use crate::render::callback::cuts_of;
use crate::render::gpu::{
    CutLine, DEPTH_FORMAT, DrainCut, ExposureBand, FrameInput, ModelDraw, ReliefDraw, TrapBox,
    ViewportResources,
};
use crate::render::machine::machine_faces;
use crate::render::vertex::{BodyVertex, LineVertex, ModelInstance, NOT_MARKED};
use crate::scene::Mapped;
use crate::ui::theme;

const SIZE: u32 = 768;
const SECTION_MM: Scalar = 50.0;

/// The ball every frame is drawn from: the model, its hierarchy and its hollowed shell.
/// Hollowing it is the dearest thing in this file and the answer never changes.
fn hollowed_ball() -> &'static (Mesh, Bvh, Arc<Mesh>, std::ops::Range<usize>) {
    static BALL: OnceLock<(Mesh, Bvh, Arc<Mesh>, std::ops::Range<usize>)> = OnceLock::new();
    BALL.get_or_init(|| {
        let mesh = core_geometry::transform_mesh(
            &ball(40.0, 48, 64),
            Transform::from_translation(Vec3::new(75.0, 40.0, 45.0)),
        );
        let bvh = Bvh::build(&mesh);
        let hollowed = hollow(
            &mesh,
            &bvh,
            &HollowSettings {
                thickness_mm: 2.0,
                precision: 0.6,
                ..HollowSettings::default()
            },
        )
        .expect("the ball hollows");
        (mesh, bvh, Arc::new(hollowed.mesh), hollowed.cavity)
    })
}

/// A hollow ball, with the holes of [`holes`] in it or without them.
fn drilled_ball(drilled: bool) -> (Arc<Mesh>, Arc<Mesh>, Arc<Mesh>, Vec<DrainHole>) {
    let (mesh, bvh, shell, _) = hollowed_ball();
    let holes = if drilled { holes() } else { Vec::new() };
    let bodies = drill(&holes, &[]).expect("the holes drill");
    let walls = bores(mesh, bvh, &holes, &[], Some(2.4)).expect("the walls mesh");
    (Arc::clone(shell), Arc::new(bodies), Arc::new(walls), holes)
}

/// Both mouths stand eight millimetres clear of the surface, the way `lift_for` puts them
/// where something stands over a hole: one hole through the top, one on the equator, whose
/// lifted tube stands in the air across the cutting plane.
fn holes() -> Vec<DrainHole> {
    vec![
        DrainHole {
            at: Vec3::new(75.0, 40.0, 85.0),
            axis: Vec3::NEG_Z,
            diameter_mm: 4.0,
            depth_mm: 4.0,
            taper: 1.0,
            lift_mm: 8.0,
        },
        DrainHole {
            at: Vec3::new(114.7, 40.0, 50.0),
            axis: Vec3::new(-39.7, 0.0, -5.0),
            diameter_mm: 4.0,
            depth_mm: 4.0,
            taper: 1.0,
            lift_mm: 8.0,
        },
    ]
}

/// A hole takes material away and can repaint what shows through itself. What it can never
/// do is put something on screen where the model was not, and a cap painted over a tube
/// standing in the air does exactly that.
#[test]
fn drilling_a_model_paints_nothing_where_the_model_was_not() {
    let Some(solid) = render(false) else {
        return;
    };
    let drilled = render(true).expect("the second frame draws too");

    let background = |pixel: &[u8; 4]| pixel[..3] == [0, 0, 0];
    let painted = solid
        .as_chunks::<4>()
        .0
        .iter()
        .zip(drilled.as_chunks::<4>().0)
        .filter(|(solid, drilled)| background(solid) && !background(drilled))
        .count();
    assert_eq!(
        painted, 0,
        "{painted} pixel(s) stand in the air around a hole"
    );
}

/// A band tints the surface it covers and leaves the rest of the model alone, and shows
/// nothing at all below the floor, because there the bottom block keeps the resin's own
/// exposure whatever the band says. See ADR 0090.
#[test]
fn an_exposure_band_tints_only_the_height_it_covers() {
    let band = ExposureBand::new(40.0, 60.0, theme::band_tint(0));
    let Some(plain) = paint_ball(&[], 0.0, None) else {
        return;
    };
    let banded = paint_ball(&[band], 0.0, None).expect("the second frame draws too");
    let floored = paint_ball(&[band], 100.0, None).expect("the third frame draws too");

    let changed = |a: &Vec<u8>, b: &Vec<u8>| {
        a.as_chunks::<4>()
            .0
            .iter()
            .zip(b.as_chunks::<4>().0)
            .filter(|(before, after)| before != after)
            .count()
    };
    assert!(
        changed(&plain, &banded) > 0,
        "a band has to show on the model at all"
    );
    assert_eq!(
        changed(&plain, &floored),
        0,
        "a band below the floor is not printed, so it may not be drawn either"
    );
}

/// A model standing past the build volume is marked there and nowhere else: the ball
/// reaches x = 115 mm, so a volume 100 mm wide cuts through it and one 200 mm wide does not.
#[test]
fn a_model_past_the_build_volume_is_marked_there() {
    let Some(plain) = paint_ball(&[], 0.0, None) else {
        return;
    };
    let roomy = paint_ball(&[], 0.0, Some(Vec3::splat(200.0))).expect("the second frame");
    let tight = paint_ball(&[], 0.0, Some(Vec3::new(100.0, 200.0, 200.0))).expect("the third");

    assert_eq!(plain, roomy, "a model inside the volume is drawn as it was");
    assert_ne!(plain, tight, "the part past x = 100 mm has to show");
}

/// The Cut tool's plane is traced where it crosses the model it is set on, and nowhere on
/// a model whose box it does not reach.
#[test]
fn the_cut_plane_is_traced_across_the_model_it_is_set_on() {
    let Some(plain) = paint_ball(&[], 0.0, None) else {
        return;
    };
    let line = |bounds| CutLine {
        normal: Vec3::Z,
        offset_mm: 45.0,
        bounds,
    };
    // The ball of radius 40 stands at z = 45, so the plane runs round its equator.
    let ball = core_geometry::Aabb::new(Vec3::new(35.0, 0.0, 5.0), Vec3::new(115.0, 80.0, 85.0));
    let elsewhere = core_geometry::Aabb::new(Vec3::splat(-50.0), Vec3::splat(-10.0));
    let traced = paint_marked_ball(Marks {
        cut_line: Some(line(ball)),
        ..Marks::default()
    })
    .expect("the second frame draws too");
    let missed = paint_marked_ball(Marks {
        cut_line: Some(line(elsewhere)),
        ..Marks::default()
    })
    .expect("the third frame draws too");

    // Compared with the untraced frame rather than with the token: this target is sRGB and
    // lightens what is written to it, so no pixel holds the token as it is.
    let brightness = |pixel: &[u8; 4]| {
        pixel[..3]
            .iter()
            .map(|channel| u32::from(*channel))
            .sum::<u32>()
    };
    let darkened = plain
        .as_chunks::<4>()
        .0
        .iter()
        .zip(traced.as_chunks::<4>().0)
        .filter(|(before, after)| brightness(after) + 60 < brightness(before))
        .count();
    assert!(
        darkened > 500,
        "a ball 80 mm across has a long equator to darken, got {darkened} pixels"
    );
    assert_eq!(
        plain, missed,
        "a plane set on another model traces nothing on this one"
    );
}

/// The machine is the only thing on an empty plate, so if the pass that paints it is
/// wrong there is nothing else on screen to hide that.
#[test]
fn the_machine_shows_under_an_empty_plate() {
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 0.0),
        distance_mm: 320.0,
        ..OrbitCamera::default()
    };
    let Some(empty) = paint(camera, None, &[], &[], Marks::default(), &[]) else {
        return;
    };
    let machine = paint(
        camera,
        None,
        &[],
        &[],
        Marks::default(),
        &machine_faces(&BuildPlate::default()),
    )
    .expect("the second frame draws too");

    let painted = empty
        .as_chunks::<4>()
        .0
        .iter()
        .zip(machine.as_chunks::<4>().0)
        .filter(|(before, after)| before != after)
        .count();
    assert!(painted > 0, "the machine has to reach the screen at all");
}

/// The ball of [`drilled_ball`] undrilled and uncut, painted with `bands`.
fn paint_ball(
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
fn paint_marked_ball(marks: Marks<'_>) -> Option<Vec<u8>> {
    paint_ball_around(Vec::new(), marks)
}

/// The same ball with `inside` drawn in it, which is where the trapped resin shows.
fn paint_ball_around(inside: Vec<ModelDraw>, marks: Marks<'_>) -> Option<Vec<u8>> {
    paint_ball_cached(inside, marks, false)
}

/// The same again, and when `cached` the ball alone is drawn on an earlier frame so that
/// the mesh is already on the card when the one carrying `inside` arrives. That is the
/// order the window draws in: a shell is on screen long before the drainage check says
/// what is trapped in it.
fn paint_ball_cached(inside: Vec<ModelDraw>, marks: Marks<'_>, cached: bool) -> Option<Vec<u8>> {
    let (mesh, _, _, _) = drilled_ball(false);
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
    paint_after(camera, None, before, &models, &[], marks, &[])
}

/// The frame as the window would draw it, or `None` on a machine with no adapter to draw
/// it with.
fn render(drilled: bool) -> Option<Vec<u8>> {
    let (mesh, bodies, walls, drilled) = drilled_ball(drilled);
    let cap_colour =
        ModelInstance::new(Transform::default(), theme::scene().section_cap, NOT_MARKED);
    let models = vec![ModelDraw::whole(
        Arc::clone(&mesh),
        ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED),
    )];
    // The window counts the cut bodies into the cap without drawing them; see ADR 0075.
    let solids = vec![
        ModelDraw::whole(mesh, cap_colour),
        ModelDraw::whole(bodies, cap_colour),
    ];
    let cuts: Vec<DrainCut> = cuts_of(&drilled, &[], Transform::default());
    let models = {
        let mut models = models;
        models.push(ModelDraw::whole(
            walls,
            ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED),
        ));
        models
    };
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 45.0),
        distance_mm: 170.0,
        ..OrbitCamera::default()
    };
    let marks = Marks {
        cuts: &cuts,
        ..Marks::default()
    };
    paint(camera, Some(SECTION_MM), &models, &solids, marks, &[])
}

/// What the shader lays over the models besides their own colour.
#[derive(Clone, Copy, Default)]
struct Marks<'a> {
    cuts: &'a [DrainCut],
    /// Where the drainage check found resin, which is the only place a volume paints.
    pockets: &'a [TrapBox],
    bands: &'a [ExposureBand],
    band_floor_mm: f32,
    volume_mm: Option<Vec3>,
    cut_line: Option<CutLine>,
    /// Whether the models are drawn seen through; see `docs/design/viewport.md`.
    xray: bool,
}

/// Seen through, a model is a wash rather than a surface: it keeps less of itself than it
/// does drawn solid, so what stands behind and inside it comes through. It keeps a good
/// deal of itself all the same — every surface along a ray adds its own wash — which is why
/// this asks for a shift rather than for a faint picture.
#[test]
fn a_model_seen_through_keeps_less_of_itself_than_a_solid_one() {
    let Some(solid) = paint_marked_ball(Marks::default()) else {
        return;
    };
    let seen_through = paint_marked_ball(Marks {
        xray: true,
        ..Marks::default()
    })
    .expect("the second frame draws on the adapter the first one drew on");

    let (solid_pixels, solid_light) = lit(&solid);
    let (through_pixels, through_light) = lit(&seen_through);
    assert!(solid_pixels > 0, "the ball is on screen at all");
    assert!(
        through_pixels > solid_pixels / 2,
        "the model is still there: {through_pixels} pixels against {solid_pixels}"
    );

    let brightness = |light: u64, count: usize| light as f64 / count as f64;
    assert!(
        brightness(through_light, through_pixels) < brightness(solid_light, solid_pixels) * 0.95,
        "a surface seen through keeps a fraction of itself: {} against {}",
        brightness(through_light, through_pixels),
        brightness(solid_light, solid_pixels)
    );
}

/// Trapped resin is the whole reason the x-ray exists, so the wall in front of the cavity
/// may not wash it out. The x-ray collapses a surface hardest where it faces the camera,
/// and that is exactly where the space behind the wall has to keep its own colour, which is
/// what `ModelInstance::as_volume` is for. It is drawn out of the shell's own cavity faces,
/// so this also pins that a part of a cached mesh draws.
#[test]
fn the_cavity_that_holds_resin_is_painted_through_the_wall_in_front_of_it() {
    let seen_through = Marks {
        xray: true,
        pockets: &pocket(-5.0),
        ..Marks::default()
    };
    let Some(plain) = paint_ball_around(Vec::new(), seen_through) else {
        return;
    };
    let volume = paint_ball_around(vec![trapped_cavity(true)], seen_through)
        .expect("the later frames draw on the adapter the first one drew on");
    let surface = paint_ball_around(vec![trapped_cavity(false)], seen_through)
        .expect("the later frames draw on the adapter the first one drew on");

    let red = redder_than(&volume, &plain);
    assert!(
        red > 10_000,
        "the whole cavity reads red, not a speck of it: {red} pixels"
    );

    // The camera looks at the middle of the ball, so the pixel at the middle of the frame
    // is the one the x-ray washes a surface down hardest at.
    let head_on = |frame: &[u8]| {
        let pixel = frame.as_chunks::<4>().0[(SIZE as usize / 2) * (SIZE as usize + 1)];
        i32::from(pixel[0]) - i32::from(pixel[1])
    };
    assert!(
        head_on(&volume) > 3 * head_on(&surface).max(1),
        "head-on the volume keeps its own colour where a surface loses it: {} against {}",
        head_on(&volume),
        head_on(&surface)
    );
}

/// The window has the shell on the card for many frames before the drainage check comes
/// back, and the cavity is painted out of that same cached mesh. A cache that answers the
/// later frame with the pieces the first one asked for draws nothing at all, which is the
/// shape of a bug that passed every single-frame test.
#[test]
fn the_cavity_paints_on_a_frame_after_the_shell_reached_the_card() {
    let seen_through = Marks {
        xray: true,
        pockets: &pocket(-5.0),
        ..Marks::default()
    };
    let Some(plain) = paint_ball_cached(Vec::new(), seen_through, true) else {
        return;
    };
    let marked = paint_ball_cached(vec![trapped_cavity(true)], seen_through, true)
        .expect("the later frames draw on the adapter the first one drew on");

    let red = redder_than(&marked, &plain);
    assert!(
        red > 10_000,
        "the cavity reads red on a frame after its mesh was cached: {red} pixels"
    );
}

/// A pocket is painted on its own: the cavity reads red where one stands in it, nowhere
/// else, and not at all while the check has found none. See ADR 0200.
#[test]
fn a_pocket_paints_only_the_cavity_it_stands_in() {
    fn marks(pockets: &[TrapBox]) -> Marks<'_> {
        Marks {
            xray: true,
            pockets,
            ..Marks::default()
        }
    }
    let whole = pocket(-5.0);
    // The same pocket cut off at the ball's own middle, so it holds the lower cavity alone.
    let [mut lower] = whole;
    lower.high[2] = 45.0;

    let Some(plain) = paint_ball_around(Vec::new(), marks(&whole)) else {
        return;
    };
    let frame = |pockets: &[TrapBox]| {
        paint_ball_around(vec![trapped_cavity(true)], marks(pockets))
            .expect("the later frames draw on the adapter the first one drew on")
    };
    let unpocketed = redder_than(&frame(&[]), &plain);
    let all = redder_than(&frame(&whole), &plain);
    let half = redder_than(&frame(&[lower]), &plain);

    assert_eq!(
        unpocketed, 0,
        "a cavity the check found nothing in is not painted at all"
    );
    assert!(
        half > 1_000,
        "the pocket that is there paints: {half} pixels"
    );
    // The camera looks down on the ball, so the lower half of the cavity is the greater
    // part of what is on screen; what matters is that it is not the whole of it.
    assert!(
        half * 4 < all * 3,
        "the lower pocket paints its own half and not the cavity: {half} against {all}"
    );
}

/// How many pixels of `frame` lead red over green by at least `RED_LEAD` more than the same
/// pixel of `against` does. The offscreen target is sRGB while the window's is not, so what
/// a token comes to in absolute bytes here is not what it comes to on screen; a shift
/// between two frames drawn the same way is.
const RED_LEAD: i32 = 20;

fn redder_than(frame: &[u8], against: &[u8]) -> usize {
    let lead = |pixel: &[u8; 4]| i32::from(pixel[0]) - i32::from(pixel[1]);
    frame
        .as_chunks::<4>()
        .0
        .iter()
        .zip(against.as_chunks::<4>().0)
        .filter(|(marked, plain)| lead(marked) - lead(plain) >= RED_LEAD)
        .count()
}

/// The box of a pocket standing in the hollow ball's cavity, its own bounds grown by
/// `slack_mm`: what the drainage check hands a frame for the resin it found. See ADR 0200.
fn pocket(slack_mm: f32) -> [TrapBox; 1] {
    let centre = Vec3::new(75.0, 40.0, 45.0);
    let reach = Vec3::splat(40.0 + slack_mm);
    [TrapBox {
        low: (centre - reach).extend(0.0).to_array(),
        high: (centre + reach).extend(0.0).to_array(),
    }]
}

/// The cavity of the hollow ball, painted in the red of resin with no way out. `volume` is
/// whether it is painted as the space it bounds rather than as one more surface.
fn trapped_cavity(volume: bool) -> ModelDraw {
    let (_, _, shell, cavity) = hollowed_ball();
    let instance = ModelInstance::new(Transform::default(), theme::scene().trapped, NOT_MARKED);
    ModelDraw::part(
        Arc::clone(shell),
        cavity.clone(),
        match volume {
            true => instance.as_volume(),
            false => instance,
        },
    )
}

/// How many pixels the frame paints at all, and how bright they come to in total.
fn lit(frame: &[u8]) -> (usize, u64) {
    frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[..3] != [0, 0, 0])
        .fold((0, 0), |(count, light), pixel| {
            (
                count + 1,
                light + u64::from(pixel[0].max(pixel[1]).max(pixel[2])),
            )
        })
}

/// The texture the Relief tool shows has to reach the surface: a model drawn with a map
/// that is white over half its area cannot look like the same model drawn flat.
#[test]
fn a_texture_shows_on_the_model_it_would_be_pressed_into() {
    let mesh = Arc::new(core_geometry::transform_mesh(
        &ball(30.0, 32, 48),
        Transform::from_translation(Vec3::new(75.0, 40.0, 40.0)),
    ));
    let instance = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 40.0),
        distance_mm: 150.0,
        ..OrbitCamera::default()
    };

    let Some(flat) = paint(
        camera,
        None,
        &[ModelDraw::whole(Arc::clone(&mesh), instance)],
        &[],
        Marks::default(),
        &[],
    ) else {
        return;
    };

    // White over the right half of the image, black over the left, planar over X.
    let heights = Heightmap::new(2, 1, vec![0.0, 1.0]).expect("two samples over two pixels");
    let bounds = mesh.aabb().expect("the ball has faces");
    let width = bounds.maxs.x - bounds.mins.x;
    let uvs = UvMap::whole(
        mesh.faces
            .iter()
            .map(|face| {
                face.map(|index| {
                    let vertex = mesh.vertices[index as usize];
                    Vec2::new((vertex.x - bounds.mins.x) / width, 0.5)
                })
            })
            .collect(),
    );
    let textured = paint_textured(
        camera,
        &ReliefDraw {
            mesh,
            mapped: Arc::new(Mapped {
                names: vec!["half.png".to_owned()],
                uvs,
                heights: vec![heights],
            }),
            instance,
        },
    )
    .expect("the second frame draws too");

    let changed = flat
        .as_chunks::<4>()
        .0
        .iter()
        .zip(textured.as_chunks::<4>().0)
        .filter(|(before, after)| before != after)
        .count();
    assert!(
        changed > SIZE as usize,
        "only {changed} pixel(s) differ, so the texture never reached the surface"
    );
}

/// One frame of a model drawn with its texture on it.
fn paint_textured(camera: OrbitCamera, draw: &ReliefDraw) -> Option<Vec<u8>> {
    let (device, queue) = gpu()?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut resources = ViewportResources::new(device, format);
    resources.prepare(
        device,
        queue,
        FrameInput {
            reliefs: std::slice::from_ref(draw),
            view_projection: camera.view_projection(1.0),
            eye: camera.eye(),
            section_mm: None,
            lines: &[],
            models: &[],
            solids: &[],
            cap: &[],
            body: &[],
            label: &[],
            atlas: None,
            cuts: &[],
            pockets: &[],
            bands: &[],
            band_floor_mm: 0.0,
            volume_mm: None,
            cut_line: None,
            xray: false,
        },
    );
    Some(read_back(device, queue, format, &resources))
}

/// One frame as the window would paint it, read back, or `None` with no adapter to paint
/// it with.
fn paint(
    camera: OrbitCamera,
    section_mm: Option<Scalar>,
    models: &[ModelDraw],
    solids: &[ModelDraw],
    marks: Marks<'_>,
    body: &[BodyVertex],
) -> Option<Vec<u8>> {
    paint_after(camera, section_mm, &[], models, solids, marks, body)
}

/// The same, with each frame of `before` prepared against the same resources first and
/// thrown away. The window draws a shell for many frames before the drainage check comes
/// back and says what is trapped in it, so what the card holds for a mesh has to answer a
/// later frame asking for something the first one did not.
fn paint_after<'a>(
    camera: OrbitCamera,
    section_mm: Option<Scalar>,
    before: &[&'a [ModelDraw]],
    models: &'a [ModelDraw],
    solids: &'a [ModelDraw],
    marks: Marks<'a>,
    body: &'a [BodyVertex],
) -> Option<Vec<u8>> {
    let Marks {
        cuts,
        pockets,
        bands,
        band_floor_mm,
        volume_mm,
        cut_line,
        xray,
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
        lines: &[],
        models,
        solids,
        cap: &cap,
        body,
        label: &[],
        atlas: None,
        cuts,
        pockets,
        bands,
        band_floor_mm,
        volume_mm,
        cut_line,
        xray,
    };
    for earlier in before {
        resources.prepare(device, queue, input(earlier));
    }
    resources.prepare(device, queue, input(models));

    Some(read_back(device, queue, format, &resources))
}

/// Paints what `resources` recorded into an offscreen target and reads the pixels back.
fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    resources: &ViewportResources,
) -> Vec<u8> {
    let (colour, depth) = (
        target(device, format, wgpu::TextureUsages::COPY_SRC),
        target(device, DEPTH_FORMAT, wgpu::TextureUsages::empty()),
    );
    let colour_view = colour.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
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
fn gpu() -> Option<&'static (wgpu::Device, wgpu::Queue)> {
    static GPU: OnceLock<Option<(wgpu::Device, wgpu::Queue)>> = OnceLock::new();
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

fn target(
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

fn extent() -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: SIZE,
        height: SIZE,
        depth_or_array_layers: 1,
    }
}

/// Two triangles in the cutting plane, wide enough to reach past the model.
fn cap_quad(height_mm: Scalar) -> Vec<LineVertex> {
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
fn block_on<F: std::future::Future>(future: F) -> F::Output {
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
fn ball(radius: Scalar, rings: usize, segments: usize) -> Mesh {
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
