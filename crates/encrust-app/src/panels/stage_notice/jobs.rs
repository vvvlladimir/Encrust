//! The cards of the jobs that report their progress at the stage's foot (ADR 0263).

use super::{Stack, remembered};
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::job_notice;

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
    let prepare = window.tools.prepare.progress().map(|(step, share)| {
        let label = tr!("notice-preparing", step = step.label());
        (label, Some(share), false)
    });
    if job_card(stack, "prepare", prepare) {
        crate::prepare::cancel(window.doc, window.tools, window.machine);
    }
    let network = window.machine.network.job.as_ref();
    let shown = network.map(|job| (job.label(), job.fraction(), job.is_cancelling()));
    if job_card(stack, "network", shown) {
        network.inspect(|job| job.cancel());
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
