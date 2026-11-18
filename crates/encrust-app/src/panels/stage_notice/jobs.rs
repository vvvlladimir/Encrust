//! The cards of the jobs that report their progress at the stage's foot (ADR 0263).

use core_volume::Step;

use super::{Stack, remembered};
use crate::i18n::tr;
use crate::panels::Window;
use crate::status::Status;
use crate::ui::{NoticeReply, icon, job_notice, notice_card, theme};
use crate::workspace::Tool;

/// One card per job that reports progress here, the latest to start nearest the foot; a
/// job that ended fades out by `CLOSE` as it last stood.
pub(super) fn jobs(stack: &mut Stack, window: &mut Window) {
    let preview = window
        .machine
        .preview
        .is_cutting_in_view()
        .then(|| (tr!("notice-cutting-preview"), None, false));
    if job_card(stack, "preview", preview) {
        window.machine.preview.cancel();
    }
    let imports = &window.doc.imports;
    let shown = imports
        .label()
        .map(|label| (label, imports.fraction(), false));
    if job_card(stack, "import", shown) {
        window.doc.imports.cancel(&mut window.machine.status);
    }
    let slicing = window.machine.slicing.job.as_ref();
    let shown = slicing.map(|job| (job.label(), job.fraction(), job.is_cancelling()));
    if job_card(stack, "slicing", shown) {
        slicing.inspect(|job| job.cancel());
    }
    if job_card(stack, "drain-cuts", drain_cuts(window)) {
        window.doc.drilling.give_up(&mut window.doc.scene);
        window.tools.drain.give_up_plugs(&mut window.doc.scene);
        window.machine.status = Status::Info(tr!("notice-drain-stopped"));
    }
    let walk = window.tools.measure_walk.notice();
    let shown = walk.map(|share| (tr!("notice-finding-walk"), Some(share), false));
    if job_card(stack, "measure-walk", shown) {
        window.tools.measure_walk.cancel(&mut window.tools.measure);
    }
    if job_card(stack, "orient", orient(window)) {
        window.tools.orient.cancel(&mut window.machine.status);
    }
    if job_card(stack, "drain-auto", drain_auto(window)) {
        window.tools.drain.cancel_auto(&mut window.machine.status);
    }
    // The Prepare row tells the run itself wherever it stands; Preview hides it.
    let away = *window.mode == crate::workspace::Mode::Preview;
    let waiting = window.tools.prepare.waiting().is_some();
    let prepare = window.tools.prepare.progress().filter(|_| away && !waiting);
    let prepare = prepare.map(|(step, share)| {
        let label = tr!("notice-preparing", step = step.label());
        (label, Some(share), false)
    });
    if job_card(stack, "prepare", prepare) {
        crate::prepare::cancel(window.doc, window.tools, window.machine);
    }
    if away {
        waiting_card(stack, window);
    }
    let network = window.machine.network.job.as_ref();
    let shown = network.map(|job| (job.label(), job.fraction(), job.is_cancelling()));
    if job_card(stack, "network", shown) {
        network.inspect(|job| job.cancel());
    }
}

/// Auto-orient at its work, where the Position tool does not show it in its own panel.
fn orient(window: &Window) -> Option<(String, Option<f32>, bool)> {
    let job = window.tools.orient.job.as_ref()?;
    if window.tool.card() == Tool::Position {
        return None;
    }
    Some(match job.counted() {
        Some((what, share)) => (what, Some(share), false),
        None => (tr!("orient-looking"), None, false),
    })
}

/// Drain's search for holes, where the Drain tool does not show it in its own panel.
fn drain_auto(window: &Window) -> Option<(String, Option<f32>, bool)> {
    let job = window.tools.drain.auto.as_ref()?;
    let shown = window.tool.card() != Tool::Drain;
    shown.then(|| (tr!("drain-auto-looking"), Some(job.fraction()), false))
}

/// The holes being taken out and the plugs being cut, as the one least far along stands.
fn drain_cuts(window: &Window) -> Option<(String, Option<f32>, bool)> {
    let drill = window.doc.drilling.progress();
    let plugs = window.tools.drain.plugs_progress();
    let share = [
        drill.map(|(_, share)| share),
        plugs.map(|(_, _, share)| share),
    ]
    .into_iter()
    .flatten()
    .min_by(f32::total_cmp)?;
    let reading = drill.is_some_and(|(step, _)| step == Step::Reading)
        || plugs.is_some_and(|(_, step, _)| step == Step::Reading);
    let label = match plugs {
        _ if reading => tr!("notice-drain-reading"),
        Some((count, _, _)) => tr!("notice-drain-plugs", count = count),
        None => tr!("notice-drain-drilling"),
    };
    Some((label, Some(share), false))
}

/// A run waiting for Continue, in Preview where the Prepare row is not: why, Continue, and
/// End as its close.
fn waiting_card(stack: &mut Stack, window: &mut Window) {
    let Some(wait) = window.tools.prepare.waiting().cloned() else {
        return;
    };
    let colors = theme::colors();
    let (glyph, tint) = match wait.failed {
        true => (icon::WARNING, colors.danger),
        false => (icon::PAUSE, colors.accent_soft),
    };
    let (go_on, end) = (tr!("notice-prepare-continue"), tr!("prepare-row-end"));
    let shown = stack.card("prepare-waiting", |ui| {
        let what = tr!(
            "prepare-row-waits-at",
            number = wait.step,
            why = wait.why.as_str()
        );
        notice_card(ui, glyph, tint, &what, None, Some(&go_on), Some(&end))
    });
    let NoticeReply { acted, closed } = shown.inner;
    let (doc, tools, machine) = (&mut *window.doc, &mut *window.tools, &mut *window.machine);
    if acted {
        if let Err(error) = crate::prepare::resume(doc, tools, machine) {
            machine.status = crate::status::Status::failed(&error);
        }
        stack.ctx.request_repaint();
    } else if closed {
        crate::prepare::end_waiting(tools, &mut machine.status);
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
