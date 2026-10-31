//! The plate card on the stage's foot, as the window draws it frame by frame.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::SlicerApp;
use super::states::{SETTLE_FRAMES, app};

/// BUG: egui places an area by its pivot from last frame's size, so while Details opened
/// the card was drawn off the stage's foot by each frame's growth and jumped back.
#[test]
fn the_plate_card_keeps_its_foot_while_details_opens_and_closes() {
    let mut app = app(true);
    app.view.options.started = true;
    let mut dressed = false;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            move |ui, app: &mut SlicerApp| {
                if dressed {
                    app.window().show(ui);
                    app.finish_frame(ui.ctx());
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            app,
        );
    harness.run_steps(SETTLE_FRAMES);
    // Where the press at the card's foot is drawn: accesskit bounds stay in the layer's own
    // space, so the shift `card_at` gives the layer is added back.
    let foot = |harness: &Harness<'_, SlicerApp>| {
        let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("this-plate"));
        let shift = harness
            .ctx
            .layer_transform_to_global(layer)
            .map_or(0.0, |transform| transform.translation.y);
        harness.get_by_label_contains("Slice").rect().bottom() + shift
    };
    let standing = foot(&harness);
    for way in ["opening", "closing"] {
        harness.get_by_label("Details").click();
        for frame in 0..SETTLE_FRAMES {
            harness.step();
            let now = foot(&harness);
            assert!(
                (now - standing).abs() < 0.01,
                "{way}, frame {frame}: the card's foot at {now}, not {standing}"
            );
        }
    }
}
