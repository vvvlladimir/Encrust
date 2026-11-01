//! The render view: layers stepped into a model, a clear resin, creases shaded at rest.

use std::sync::Arc;

use core_geometry::{Transform, Vec2, Vec3, cuboid};

use super::marks::{Marks, paint_marked_ball};
use super::paint::paint;
use crate::camera::OrbitCamera;
use crate::render::gpu::{Floor, Look, ModelDraw};
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;

/// Far coarser than any printer cuts, so the steps span many pixels of the 80 mm ball.
const COARSE_LAYER_MM: f32 = 2.0;

/// Pixels that differ between two frames by more than `by` in summed channels, and those
/// of them where `after` is the darker.
fn darker(before: &[u8], after: &[u8], by: u32) -> (usize, usize) {
    let sum = |pixel: &[u8; 4]| pixel[..3].iter().map(|c| u32::from(*c)).sum::<u32>();
    let pairs = || {
        before
            .as_chunks::<4>()
            .0
            .iter()
            .zip(after.as_chunks::<4>().0)
    };
    let changed = pairs()
        .filter(|(a, b)| sum(a).abs_diff(sum(b)) > by)
        .count();
    let darkened = pairs().filter(|(a, b)| sum(b) + by < sum(a)).count();
    (changed, darkened)
}

fn studio() -> Look {
    Look {
        pivot_mm: 170.0,
        ..Look::default()
    }
}

#[test]
fn layers_step_into_the_model_only_while_asked_for() {
    let Some(plain) = paint_marked_ball(Marks {
        look: Some(studio()),
        ..Marks::default()
    }) else {
        return;
    };
    let layered = paint_marked_ball(Marks {
        look: Some(Look {
            layer_height_mm: COARSE_LAYER_MM,
            ..studio()
        }),
        ..Marks::default()
    })
    .expect("the adapter drew once");
    let (changed, _) = darker(&plain, &layered, 0);
    assert!(
        changed > 5_000,
        "{COARSE_LAYER_MM} mm layers over a ball 80 mm across shade thousands of pixels, got {changed}"
    );
}

#[test]
fn a_clear_resin_shows_the_plate_through_a_hollow_model() {
    let floor = Some(Floor {
        near_corner_mm: Vec2::ZERO,
        size_mm: Vec2::new(150.0, 80.0),
    });
    let frame = |attenuation_mm| {
        paint_marked_ball(Marks {
            look: Some(Look {
                attenuation_mm,
                ..studio()
            }),
            floor,
            presented: true,
            ..Marks::default()
        })
    };
    let Some(opaque) = frame(0.0) else {
        return;
    };
    let clear = frame(40.0).expect("the adapter drew once");
    let (changed, darkened) = darker(&opaque, &clear, 30);
    assert!(
        changed > 20_000,
        "a hollow ball 80 mm across lets the plate through much of itself, got {changed} pixels"
    );
    assert!(
        darkened * 2 > changed,
        "the plate behind is darker than a lit opaque wall, got {darkened} of {changed} darker"
    );
}

/// A wall standing on a slab: the crease along its foot is the only enclosed place.
fn wall_on_a_slab() -> Vec<ModelDraw> {
    let colour = theme::scene().object;
    let block = |low: Vec3, high: Vec3| {
        ModelDraw::whole(
            Arc::new(cuboid(low, high)),
            ModelInstance::new(Transform::default(), colour, NOT_MARKED),
        )
    };
    vec![
        block(Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 40.0, 4.0)),
        block(Vec3::new(18.0, 0.0, 4.0), Vec3::new(22.0, 40.0, 30.0)),
    ]
}

#[test]
fn creases_are_shaded_in_the_copy_and_nothing_else_is_lightened() {
    let camera = OrbitCamera {
        target: Vec3::new(20.0, 20.0, 6.0),
        distance_mm: 60.0,
        ..OrbitCamera::default()
    };
    let models = wall_on_a_slab();
    let look = Some(Look {
        projection: camera.projection(1.0),
        ..studio()
    });
    let frame = |presented| {
        paint(
            camera,
            None,
            &models,
            &[],
            Marks {
                look,
                presented,
                ..Marks::default()
            },
        )
    };
    let Some(drawn) = frame(false) else {
        return;
    };
    let copied = frame(true).expect("the adapter drew once");
    let (changed, darkened) = darker(&drawn, &copied, 6);
    assert!(
        darkened > 300,
        "the wall's foot is shaded, got {darkened} pixels darker"
    );
    assert_eq!(changed, darkened, "the copy only ever takes light away");
}
