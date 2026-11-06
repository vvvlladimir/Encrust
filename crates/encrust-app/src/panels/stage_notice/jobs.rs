//! The cards of the jobs that report their progress at the stage's foot (ADR 0263).

use super::{Stack, remembered};
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{NoticeReply, icon, job_notice, notice_card, theme};

/// One card per job that reports progress here, the latest to start nearest the foot; a
/// job that ended fades out by `CLOSE` as it last stood.
pub(super) fn jobs(stack: &mut Stack, window: &mut Window) {
    let preview = window
        .machine
        .preview
        .is_building()
        .then(|| (tr!("notice-cutting-preview"), None, false));
    if job_card(stack, "preview", preview) {
        window.machine.preview.cancel();
    }
    let slicing = window.machine.slicing.job.as_ref();
    let shown = slicing.map(|job| (job.label(), job.fraction(), job.is_cancelling()));
    if job_card(stack, "slicing", shown) {
        slicing.inspect(|job| job.cancel());
    }
    let walk = window.tools.measure_walk.notice();
    let shown = walk.map(|share| (tr!("notice-finding-walk"), Some(share), false));
    if job_card(stack, "measure-walk", shown) {
        window.tools.measure_walk.cancel(&mut window.tools.measure);
    }
    // The Prepare row tells the run itself wherever it stands; Preview hides it.
    let away = *window.mode == crate::workspace::Mode::Preview;
    let paused = window.tools.prepare.paused().is_some();
    let prepare = window.tools.prepare.progress().filter(|_| away && !paused);
    let prepare = prepare.map(|(step, share)| {
        let label = tr!("notice-preparing", step = step.label());
        (label, Some(share), false)
    });
    if job_card(stack, "prepare", prepare) {
        crate::prepare::cancel(window.doc, window.tools, window.machine);
    }
    paused_card(stack, window);
    let network = window.machine.network.job.as_ref();
    let shown = network.map(|job| (job.label(), job.fraction(), job.is_cancelling()));
    if job_card(stack, "network", shown) {
        network.inspect(|job| job.cancel());
    }
}

/// A run waiting at a pause: what the pause says, Continue, and Cancel as its close.
fn paused_card(stack: &mut Stack, window: &mut Window) {
    let Some(message) = window.tools.prepare.paused().map(str::to_owned) else {
        return;
    };
    let colors = theme::colors();
    let (go_on, cancel) = (tr!("notice-prepare-continue"), tr!("common-cancel"));
    let shown = stack.card("prepare-paused", |ui| {
        notice_card(
            ui,
            icon::PAUSE,
            colors.accent_soft,
            &message,
            None,
            Some(&go_on),
            Some(&cancel),
        )
    });
    let NoticeReply { acted, closed } = shown.inner;
    if acted {
        window.tools.prepare.resume();
        stack.ctx.request_repaint();
    } else if closed {
        crate::prepare::cancel(window.doc, window.tools, window.machine);
    }
}

/// What a job is doing — its words, how far along, whether it is stopping — and a way out,
/// or the card it last stood as while that fades. Answers whether to stop.
fn job_card(stack: &mut Stack, name: &str, job: Option<(String, Option<f32>, bool)>) -> bool {
    let Some(((label, fraction, cancelling), left)) = remembered(&stack.ctx, name, job) else {
        return false;
    };
    let going = left < 1.0;
    let stop = stack
        .fading_card(name, left, |ui| {
            job_notice(ui, &label, fraction, cancelling || going)
        })
        .inner;
    stop && !going
}
