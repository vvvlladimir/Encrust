//! Supports' Pick driven through the window's own pointer path: a press on a standing
//! support, with and without a modifier held.

use core_geometry::Vec3;
use egui_kittest::Harness;

use super::SlicerApp;
use super::states::{SETTLE_FRAMES, app};
use crate::workspace::Tool;

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// The cube lifted 10 mm with two supports under its floor, in Supports' Pick, drawn until
/// both stand.
fn two_supports() -> Harness<'static, SlicerApp> {
    let mut app = app(true);
    app.view.options.started = true;
    app.tool = Tool::Supports;
    let object = &mut app.doc.scene.objects_mut()[0];
    object.transform.translation.z += 10.0;
    let floor = object.transform.translation.z;
    let middle = object.transform.translation;
    for x in [-5.0, 5.0] {
        let contact = Vec3::new(middle.x + x, middle.y, floor);
        object.supports.add(contact, object.transform, 0);
    }
    let mut dressed = false;
    let mut harness = Harness::builder().with_size(SIZE).build_ui_state(
        move |ui, app: &mut SlicerApp| {
            if dressed {
                app.poll_jobs(ui.ctx());
                app.refresh_supports(ui.ctx().input(|input| input.pointer.any_down()));
                app.window().show(ui);
                app.finish_frame(ui.ctx());
            } else {
                crate::ui::theme::apply(ui.ctx());
                dressed = true;
            }
        },
        app,
    );
    for _ in 0..400 {
        harness.run_steps(2);
        if harness.state().doc.scene.objects()[0]
            .supports
            .trees()
            .len()
            == 2
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    harness.run_steps(SETTLE_FRAMES);
    harness
}

/// Where the column of support `tree` stands on the screen, halfway down it.
fn on_screen(harness: &Harness<'_, SlicerApp>, tree: usize) -> egui::Pos2 {
    let app = harness.state();
    let trees = app.doc.scene.objects()[0].supports.trees();
    let nodes = trees.get(tree).expect("both supports stand").nodes();
    let top = nodes.first().expect("a support has a tip").position;
    let foot = nodes.last().expect("and a foot").position;
    let middle = (top + foot) / 2.0;
    let viewport = egui::Rect::from_min_max(
        egui::pos2(0.0, crate::ui::theme::TOP_BAR_H),
        egui::pos2(SIZE.x, SIZE.y),
    );
    let aspect = viewport.width() / viewport.height();
    let clip = app.view.camera.view_projection(aspect) * middle.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    egui::pos2(
        viewport.left() + (ndc.x * 0.5 + 0.5) * viewport.width(),
        viewport.top() + (0.5 - ndc.y * 0.5) * viewport.height(),
    )
}

/// A press and release on `at` with `modifiers` held.
fn click(harness: &mut Harness<'_, SlicerApp>, at: egui::Pos2, modifiers: egui::Modifiers) {
    harness.hover_at(at);
    harness.event(egui::Event::ModifiersChanged(modifiers));
    harness.run_steps(2);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        });
        harness.run_steps(2);
    }
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    harness.run_steps(2);
}

fn picked(harness: &Harness<'_, SlicerApp>) -> usize {
    harness.state().tools.supports.picked.len()
}

#[test]
fn shift_adds_a_second_support_to_the_picked() {
    let mut harness = two_supports();
    let (first, second) = (on_screen(&harness, 0), on_screen(&harness, 1));
    click(&mut harness, first, egui::Modifiers::NONE);
    assert_eq!(picked(&harness), 1, "a click picks the support under it");
    click(&mut harness, second, egui::Modifiers::SHIFT);
    assert_eq!(picked(&harness), 2, "shift adds the second");
}
