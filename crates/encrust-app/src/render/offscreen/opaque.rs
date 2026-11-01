//! A model nothing cuts is drawn by an opaque pipeline, and paints what the clipped one does.

use super::marks::{Marks, paint_marked_ball};
use super::paint::differing;
use crate::render::gpu::{DrainCut, Look};

/// A drain cut far off the plate: it reaches no fragment of the ball, but while there is one
/// every model is drawn by the clipped pipeline.
fn far_cut() -> DrainCut {
    DrainCut {
        mouth: [-900.0, -900.0, 0.0, 2.0],
        tip: [-900.0, -900.0, 10.0, 2.0],
        ..DrainCut::default()
    }
}

#[test]
fn the_opaque_pipeline_paints_what_the_clipped_one_does() {
    let studio = Look {
        pivot_mm: 170.0,
        ..Look::default()
    };
    let cuts = [far_cut()];
    let views = [
        ("the working view", None, false),
        ("the render view moving", Some(studio), true),
        ("the render view at rest", Some(studio), false),
    ];
    for (view, look, moving) in views {
        let marks = |cuts| Marks {
            cuts,
            look,
            moving,
            ..Marks::default()
        };
        let Some(opaque) = paint_marked_ball(marks(&[])) else {
            return;
        };
        let clipped = paint_marked_ball(marks(&cuts)).expect("the adapter drew once");
        assert_eq!(
            differing(&opaque, &clipped),
            0,
            "in {view} no pixel differs"
        );
    }
}
