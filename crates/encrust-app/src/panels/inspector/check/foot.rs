//! The foot of Check: Fix all, or the run going with its way to stop.

use super::cards::Board;
use crate::i18n::tr;
use crate::panels::Window;
use crate::prepare;
use crate::status::Status;
use crate::ui::{blocked_hover, primary_button, step_bar};

/// Whether the foot has anything to say.
pub(in crate::panels::inspector) fn wanted(ctx: &egui::Context, window: &mut Window) -> bool {
    window.tools.prepare.is_holding() || Board::read(ctx, window).fixable() > 0
}

pub(in crate::panels::inspector) fn ui(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.prepare.is_holding() {
        running(ui, window);
        return;
    }
    let blocker = prepare::held_up(window.doc, window.tools, window.machine);
    let button = primary_button(ui, &tr!("check-fix-all"), blocker.is_none());
    let tip = blocker.or_else(|| Some(tr!("check-fix-issues-tip")));
    if blocked_hover(button, tip).clicked() {
        start(window);
    }
}

fn running(ui: &mut egui::Ui, window: &mut Window) {
    let Some((step, share)) = window.tools.prepare.progress() else {
        return;
    };
    if step_bar(ui, &step.label(), Some(share), true) {
        prepare::cancel(window.doc, window.tools, window.machine);
    }
}

fn start(window: &mut Window) {
    if let Err(error) = prepare::start_fixes(window.doc, window.tools, window.machine) {
        window.machine.status = Status::failed(&error);
    }
}
