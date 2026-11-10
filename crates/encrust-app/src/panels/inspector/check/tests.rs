use core_geometry::Vec3;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::*;
use crate::panels::fixtures::Parts;
use crate::scene::fixtures::scene_of_boxes;
use crate::state::Doc;
use crate::status::Status;
use crate::tool_settings::ToolSettings;
use crate::undo::Frame;
use crate::workspace::Tool;

/// Records the plate as it stands as the history's latest edit.
fn record(parts: &mut Parts) {
    let settings = ToolSettings::of(&parts.tools, &parts.machine.slicing);
    let chosen = parts.machine.slicing.chosen();
    let settled = Frame {
        settled: true,
        typing: false,
    };
    let history = &mut parts.doc.history;
    history.observe(&parts.doc.scene, &settings, chosen, settled);
}

/// A unit cube on a printer's plate, the plate's first edit recorded.
fn a_cube_on_a_printer() -> Parts {
    let mut doc = Doc::default();
    doc.plate.name = Some("printer".to_owned());
    doc.scene = scene_of_boxes(&[(Vec3::ZERO, Vec3::ONE)]);
    let mut parts = Parts::of(doc, Tool::Check);
    record(&mut parts);
    parts
}

/// BUG: once a fix had answered a kind, its button went, though a later edit left the kind
/// with issues that button answers.
#[test]
fn a_kind_a_fix_has_answered_keeps_that_fix_while_issues_are_left() {
    let mut parts = a_cube_on_a_printer();
    let past_mm = parts.doc.plate.x_mm * 2.0;
    parts.doc.scene.objects_mut()[0].transform.translation.x += past_mm;
    let said = Status::Info("moved".to_owned());
    let history = &parts.doc.history;
    let answers = &mut parts.tools.check.answers;
    answers.note(history, true, IssueKind::InTheWay, &said);
    record(&mut parts);

    let mut dressed = false;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 800.0))
        .build_ui_state(
            move |ui, parts: &mut Parts| {
                // The fonts are bound a frame after they are handed over.
                if dressed {
                    super::ui(ui, &mut parts.window());
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            parts,
        );
    harness.run_steps(3);
    let place = tr!("check-place");
    harness
        .query_by_label_contains(&place)
        .expect("Move to free space stays on the card the fix answered");
}
