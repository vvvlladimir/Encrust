use super::SIZE;
use super::holes::hollowed_ball;
use super::marks::{Marks, paint_ball_around, paint_ball_cached, paint_marked_ball};
use crate::render::gpu::{ModelDraw, TrapBox};
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;
use core_geometry::{Transform, Vec3};
use std::sync::Arc;

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
    pub(super) fn marks(pockets: &[TrapBox]) -> Marks<'_> {
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
pub(super) const RED_LEAD: i32 = 20;

pub(super) fn redder_than(frame: &[u8], against: &[u8]) -> usize {
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
pub(super) fn pocket(slack_mm: f32) -> [TrapBox; 1] {
    let centre = Vec3::new(75.0, 40.0, 45.0);
    let reach = Vec3::splat(40.0 + slack_mm);
    [TrapBox {
        low: (centre - reach).extend(0.0).to_array(),
        high: (centre + reach).extend(0.0).to_array(),
    }]
}

/// The cavity of the hollow ball, painted in the red of resin with no way out. `volume` is
/// whether it is painted as the space it bounds rather than as one more surface.
pub(super) fn trapped_cavity(volume: bool) -> ModelDraw {
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
pub(super) fn lit(frame: &[u8]) -> (usize, u64) {
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
