//! The foot of Check: Fix issues, or the run going with its way to stop.

use super::cards::Board;
use crate::i18n::tr;
use crate::panels::Window;
use crate::prepare;
use crate::status::Status;
use crate::ui::{blocked_hover, hint, primary_button, step_bar};

/// Whether the foot has anything to say.
pub(in crate::panels::inspector) fn wanted(ctx: &egui::Context, window: &mut Window) -> bool {
    let (fixable, left) = Board::read(ctx, window).tally();
    window.tools.prepare.is_holding() || fixable + left > 0
}

pub(in crate::panels::inspector) fn ui(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.prepare.is_holding() {
        running(ui, window);
        return;
    }
    let (fixable, left) = Board::read(ui.ctx(), window).tally();
    if fixable > 0 {
        let blocker = prepare::held_up(window.doc, window.tools, window.machine);
        let words = tr!("check-fix-issues", count = fixable);
        let button = primary_button(ui, &words, blocker.is_none());
        let tip = blocker
            .clone()
            .or_else(|| Some(tr!("check-fix-issues-tip")));
        if blocked_hover(button, tip).clicked() {
            start(window);
        }
    }
    if left > 0 {
        hint(ui, &tr!("check-left-to-you", count = left));
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
