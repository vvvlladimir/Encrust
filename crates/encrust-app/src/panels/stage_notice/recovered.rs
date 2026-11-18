//! The plate a run left unsaved when it ended without closing, offered back (ADR 0452).

use egui::Id;

use super::Stack;
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::theme::motion;
use crate::ui::{NoticeReply, icon, notice_card, theme};

/// The newest copy another run left, while the plate is empty: Restore opens it in its place,
/// the cross discards it at a second press.
pub(super) fn recovered(stack: &mut Stack, window: &mut Window) {
    if !window.doc.scene.is_empty() {
        return;
    }
    let Some(left) = window.machine.autosave.offered() else {
        return;
    };
    let now_s = stack.ctx.input(|input| input.time);
    // When the cross was first pressed, s of the input's clock.
    let armed_key = Id::new("stage-notice-recover-armed");
    let armed = stack
        .ctx
        .data(|data| data.get_temp::<f64>(armed_key))
        .is_some_and(|at_s| now_s - at_s <= motion::ARMED_S);
    let when = crate::ui::ago(Some(left.at_s), crate::updates::now_s());
    let detail = match left
        .belonged
        .as_deref()
        .and_then(std::path::Path::file_stem)
    {
        Some(name) => tr!(
            "notice-recover-of",
            name = name.to_string_lossy().into_owned(),
            when = when
        ),
        None => tr!("notice-recover-when", when = when),
    };
    let label = if armed {
        tr!("notice-recover-discard-again")
    } else {
        tr!("notice-recover")
    };
    let colors = theme::colors();
    let NoticeReply { acted, closed } = stack
        .card("recovered", |ui| {
            notice_card(
                ui,
                icon::RECENT,
                colors.warn,
                &label,
                Some(&detail),
                Some(&tr!("notice-recover-restore")),
                Some(&tr!("notice-recover-discard")),
            )
        })
        .inner;
    if armed {
        stack
            .ctx
            .request_repaint_after(std::time::Duration::from_secs_f64(motion::ARMED_S));
    }
    if acted || (closed && armed) {
        stack.ctx.data_mut(|data| data.remove::<f64>(armed_key));
    }
    if acted {
        restore(window);
    } else if closed && armed {
        if let Some(left) = window.machine.autosave.take_offered() {
            left.discard();
        }
    } else if closed {
        stack
            .ctx
            .data_mut(|data| data.insert_temp(armed_key, now_s));
    }
}

/// Opens the copy as the unsaved plate it was; one that will not read is discarded, since
/// it would only be offered again.
fn restore(window: &mut Window) {
    let Some(left) = window.machine.autosave.take_offered() else {
        return;
    };
    if crate::project::recover(window, &left.copy(), left.belonged.clone()) {
        window.machine.autosave.adopt(left);
    } else {
        left.discard();
    }
}
