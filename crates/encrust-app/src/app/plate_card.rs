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
    // Where the press at the card's foot is drawn, not where egui keeps the area: the area's
    // rect in memory already holds this frame's size at the place it will be drawn next.
    let foot =
        |harness: &Harness<'_, SlicerApp>| harness.get_by_label_contains("Slice").rect().bottom();
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
