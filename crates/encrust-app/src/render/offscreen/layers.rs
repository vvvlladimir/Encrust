//! The render view's layers, stepped into a model by the shader.

use super::marks::{Marks, paint_marked_ball};

/// Far coarser than any printer cuts, so the steps span many pixels of the 80 mm ball.
const COARSE_LAYER_MM: f32 = 2.0;

#[test]
fn layers_step_into_the_model_only_while_asked_for() {
    let Some(plain) = paint_marked_ball(Marks::default()) else {
        return;
    };
    let unlayered = paint_marked_ball(Marks {
        layer_height_mm: 0.0,
        ..Marks::default()
    })
    .expect("the adapter drew once");
    let layered = paint_marked_ball(Marks {
        layer_height_mm: COARSE_LAYER_MM,
        ..Marks::default()
    })
    .expect("the adapter drew once");
    assert_eq!(
        plain, unlayered,
        "no layer height leaves the working view as it was"
    );
    let changed = plain
        .as_chunks::<4>()
        .0
        .iter()
        .zip(layered.as_chunks::<4>().0)
        .filter(|(before, after)| before != after)
        .count();
    assert!(
        changed > 5_000,
        "{COARSE_LAYER_MM} mm layers over a ball 80 mm across shade thousands of pixels, got {changed}"
    );
}
