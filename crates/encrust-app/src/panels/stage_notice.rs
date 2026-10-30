use egui::{Align2, Id, Rect, RichText, pos2, vec2};

use crate::i18n::tr;
use crate::panels::{Window, unreadable_profiles};
use crate::scene::ObjectId;
use crate::status::{Change, Status};
use crate::ui::{
    NoticeReply, card, icon, icon_button, job_notice, notice_card, theme, theme::motion,
};

/// Points between the cards, and how far above the stage's foot the lowest stands.
const GAP: f32 = 6.0;
const FOOT_RISE: f32 = 8.0;
/// The widest a message may grow before it wraps.
const MESSAGE_W: f32 = 440.0;
/// Points a notice slides up as it comes in.
const COME_IN: f32 = 6.0;
/// How long a message that is not a failure stays up after it changed, or after the
/// pointer left it, seconds.
const INFO_LINGER_S: f64 = 4.0;
/// Where egui keeps when the message was last seen changing.
const SEEN: &str = "stage-notice-seen";

/// Every notice, centred at the foot of `stage`, from what is running to what happened last,
/// nearest the edge. See `docs/decisions/0217` and `0263`.
pub fn ui(ui: &egui::Ui, window: &mut Window, stage: Rect) {
    let machine = &window.machine;
    let running = machine.network.job.is_some()
        || machine.slicing.job.is_some()
        || machine.preview.is_building();
    let mut stack = Stack {
        ctx: ui.ctx().clone(),
        room: stage,
        bottom: stage.bottom() - FOOT_RISE,
    };
    let width = (stage.width() - theme::CARD_MARGIN.sum().x).clamp(0.0, MESSAGE_W);
    message(&mut stack, &mut window.machine.status, running, width);
    unready(&mut stack, window);
    release(&mut stack, window);
    broken(&mut stack, window);
    unreadable(&mut stack, window);
    jobs(&mut stack, window);
}

/// The notices stacked up from the foot of `room`, each in an area of its own centred on
/// the room, so cards of different widths share one middle.
struct Stack {
    ctx: egui::Context,
    room: Rect,
    bottom: f32,
}

impl Stack {
    /// The card `name` on top of those already stacked, as `add` draws it, fading and
    /// sliding up as it comes in.
    fn card<R>(
        &mut self,
        name: &str,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        self.fading_card(name, 1.0, add)
    }

    /// The same, `left` of it still there as it goes away, 1 until it starts to.
    fn fading_card<R>(
        &mut self,
        name: &str,
        left: f32,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let id = Id::new("stage-notice").with(name);
        let came_in = motion::appear(&self.ctx, id) * left;
        let shown = egui::Area::new(id)
            .order(egui::Order::Middle)
            .fixed_pos(
                pos2(self.room.center().x, self.bottom) + vec2(0.0, (1.0 - came_in) * COME_IN),
            )
            .pivot(Align2::CENTER_BOTTOM)
            .constrain_to(self.room)
            .show(&self.ctx, |ui| {
                ui.set_opacity(came_in);
                add(ui)
            });
        self.bottom = shown.response.rect.top() - GAP;
        shown
    }
}

/// One card per job that reports progress here, the latest to start nearest the foot; a
/// job that ended fades out by `CLOSE` as it last stood.
fn jobs(stack: &mut Stack, window: &mut Window) {
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

/// What card `name` shows: `now` while it stands, kept as it goes; once gone, what it last
/// showed for the `CLOSE` it fades out over. Answers it with how much of the card is left.
fn remembered<T: Clone + Send + Sync + 'static>(
    ctx: &egui::Context,
    name: &str,
    now: Option<T>,
) -> Option<(T, f32)> {
    let key = Id::new("stage-notice-last").with(name);
    let time = ctx.input(|input| input.time);
    if let Some(now) = now {
        ctx.data_mut(|data| data.insert_temp(key, (now.clone(), time)));
        return Some((now, 1.0));
    }
    let (last, at_s) = ctx.data(|data| data.get_temp::<(T, f64)>(key))?;
    let close_s = f64::from(motion::CLOSE.seconds(ctx));
    let gone_s = time - at_s;
    if gone_s >= close_s {
        ctx.data_mut(|data| data.remove::<(T, f64)>(key));
        return None;
    }
    ctx.request_repaint();
    Some((last, (1.0 - gone_s / close_s) as f32))
}

/// The user's profile files that would not read, and the way to reset them.
fn unreadable(stack: &mut Stack, window: &mut Window) {
    if !unreadable_profiles::any(&stack.ctx, window) {
        return;
    }
    stack.card("unreadable-profiles", |ui| {
        card()
            .inner_margin(theme::CARD_MARGIN)
            .show(ui, |ui| unreadable_profiles::row(ui, window));
    });
}

/// A model that will not print as it stands, with the repair beside it. Nothing waits on
/// the answer, so it is offered here rather than in a dialog, until it is taken or put by.
fn broken(stack: &mut Stack, window: &mut Window) {
    let offered: Vec<(ObjectId, String, Vec<String>)> = window
        .doc
        .repairs
        .offered(&window.doc.scene)
        .map(|object| (object.id, object.name.clone(), object.summary.defects()))
        .collect();
    let words = match offered.as_slice() {
        [] => None,
        [(_, name, _)] => Some((
            tr!("notice-broken-one", name = name.clone()),
            tr!("notice-repair"),
        )),
        several => Some((
            tr!("notice-broken-several", count = several.len()),
            tr!("notice-repair-all"),
        )),
    };
    let Some(((label, wording), left)) = remembered(&stack.ctx, "broken", words) else {
        return;
    };
    let lines: Vec<&str> = offered
        .iter()
        .flat_map(|(_, _, defects)| defects.iter().map(String::as_str))
        .collect();
    let colors = theme::colors();
    let shown = stack.fading_card("broken", left, |ui| {
        notice_card(
            ui,
            icon::WARNING,
            colors.warn,
            &label,
            None,
            Some(&wording),
            Some(&tr!("notice-keep-as-is")),
        )
    });
    shown.response.on_hover_text(lines.join("\n"));
    let NoticeReply {
        acted: repair,
        closed: keep,
    } = shown.inner;
    for (id, _, _) in &offered {
        if repair {
            let (repairs, state) = (&mut window.doc.repairs, window.tools.repair.state.plain());
            repairs.start(&window.doc.scene, *id, state, &mut window.machine.status);
        } else if keep {
            window.doc.repairs.keep_as_it_is(*id);
        }
    }
}

/// A release found by the daily look, said once a version: What is new opens the sheet,
/// the cross leaves only the pill in the top bar.
fn release(stack: &mut Stack, window: &mut Window) {
    let updates = &mut window.machine.updates;
    let Some(offer) = updates.notice() else {
        return;
    };
    let label = tr!("notice-release-out", version = offer.version.to_string());
    let items = offer
        .notes
        .iter()
        .filter_map(|note| match note {
            crate::updates::Note::Item(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let detail = match items.as_slice() {
        [] => None,
        [first] => Some((*first).to_owned()),
        [first, rest @ ..] => Some(tr!(
            "notice-release-and-more",
            first = (*first).to_owned(),
            count = rest.len()
        )),
    };
    let colors = theme::colors();
    let reply = stack
        .card("release", |ui| {
            notice_card(
                ui,
                icon::UPDATE,
                colors.accent_soft,
                &label,
                detail.as_deref(),
                Some(&tr!("notice-what-is-new")),
                Some(&tr!("notice-release-later")),
            )
        })
        .inner;
    if reply.acted {
        updates.sheet = true;
        updates.put_notice_by();
    } else if reply.closed {
        updates.put_notice_by();
    }
}

/// A plate with no machine or no resin cannot slice, so it says which and offers the
/// window both are set up in, for as long as either is missing (ADR 0299).
fn unready(stack: &mut Stack, window: &mut Window) {
    let slicing = &window.machine.slicing;
    let (label, why) = match (slicing.printer_id.is_some(), slicing.resin_id.is_some()) {
        (true, true) => return,
        (false, false) => (tr!("notice-no-printer-resin"), tr!("notice-no-printer-why")),
        (false, true) => (tr!("notice-no-printer"), tr!("notice-no-printer-why")),
        (true, false) => (tr!("notice-no-resin"), tr!("notice-no-resin-why")),
    };
    let colors = theme::colors();
    let open = stack
        .card("unready", |ui| {
            notice_card(
                ui,
                icon::ERROR,
                colors.danger,
                &label,
                Some(&why),
                Some(&tr!("notice-set-up")),
                None,
            )
        })
        .inner
        .acted;
    if open {
        crate::panels::open_machines(window.machine);
    }
}

/// What happened last, no wider than `max_w` points. A failure stays until it is put away, and its whole cause chain is
/// on hover; anything else stands for a moment, and not at all while a job is running.
fn message(stack: &mut Stack, status: &mut Status, running: bool, max_w: f32) {
    let error = status.is_error();
    let now = stack.ctx.input(|input| input.time);
    let since_s = now - changed_at(&stack.ctx, status, now);
    if matches!(status, Status::Idle) || !error && (running || since_s > INFO_LINGER_S) {
        return;
    }
    // The last `CLOSE` of its moment, a message fades out rather than vanishing.
    let left_s = INFO_LINGER_S - since_s;
    let close_s = f64::from(motion::CLOSE.seconds(&stack.ctx));
    let left = match error || left_s >= close_s {
        true => 1.0,
        false => (left_s / close_s) as f32,
    };
    if left < 1.0 {
        stack.ctx.request_repaint();
    } else if !error {
        let until_fade = std::time::Duration::from_secs_f64(left_s - close_s);
        stack.ctx.request_repaint_after(until_fade);
    }

    let colors = theme::colors();
    let frame = card().inner_margin(theme::CARD_MARGIN);
    let mut dismissed = false;
    let shown = stack.fading_card("message", left, |ui| {
        frame.show(ui, |ui| {
            ui.set_max_width(max_w);
            // Rows as tall as their text: a notice is a line or two, not a form.
            ui.spacing_mut().interact_size.y = 0.0;
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if error {
                    let glyph = RichText::new(icon::ERROR).font(theme::icon(16.0));
                    ui.label(glyph.color(colors.danger));
                }
                // A message longer than the room wraps rather than running past the card.
                let close_w = if error { theme::ICON_SIZE + 8.0 } else { 0.0 };
                let words_w = (ui.available_width() - close_w).max(0.0);
                ui.allocate_ui(egui::vec2(words_w, 0.0), |ui| {
                    ui.vertical(|ui| words(ui, status));
                });
                if error {
                    dismissed = icon_button(ui, icon::CANCEL, &tr!("notice-put-away")).clicked();
                }
            });
        })
    });
    // A message being read stays: its moment starts again once the pointer leaves it.
    if shown.response.contains_pointer() {
        seen_now(&stack.ctx, status, now);
    }
    if dismissed {
        *status = Status::Idle;
    }
}

/// The message, and under it what it notes, or the figures a step changed coloured by
/// whether they got better.
fn words(ui: &mut egui::Ui, status: &Status) {
    let colors = theme::colors();
    ui.label(
        RichText::new(status.text())
            .font(theme::label())
            .color(colors.text_high),
    );
    if let Status::Noted(_, notes) = status {
        for note in notes {
            ui.label(
                RichText::new(note)
                    .font(theme::label())
                    .color(colors.text_mid),
            );
        }
    }
    let Status::Readings(_, readings) = status else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        for reading in readings {
            let tint = match reading.change {
                Change::Same => colors.text_mid,
                Change::Better => colors.ok,
                Change::Worse => colors.danger,
            };
            ui.label(
                RichText::new(&reading.text)
                    .font(theme::label())
                    .color(tint),
            );
        }
    });
}

/// When the message last changed, on egui's clock, kept in egui's memory: the status is
/// replaced wholesale wherever something happens, and carries no time of its own.
fn changed_at(ctx: &egui::Context, status: &Status, now: f64) -> f64 {
    let seen = ctx.data(|data| data.get_temp::<(u64, f64)>(Id::new(SEEN)));
    match seen {
        Some((key, at)) if key == Id::new(status).value() => at,
        _ => {
            seen_now(ctx, status, now);
            now
        }
    }
}

/// Shows the message for its moment again, though it is the one shown before: a second
/// click that misses says so as the first did.
pub(super) fn show_again(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove_temp::<(u64, f64)>(Id::new(SEEN)));
}

/// Counts the message's moment from `now`.
fn seen_now(ctx: &egui::Context, status: &Status, now: f64) {
    let key = Id::new(status).value();
    ctx.data_mut(|data| data.insert_temp(Id::new(SEEN), (key, now)));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `remembered` answers for a card in a frame at `time_s`.
    fn at(ctx: &egui::Context, time_s: f64, now: Option<&str>) -> Option<(String, f32)> {
        let input = egui::RawInput {
            time: Some(time_s),
            ..egui::RawInput::default()
        };
        let mut answer = None;
        let mut output = ctx.run_ui(input, |ui| {
            answer = remembered(ui.ctx(), "job", now.map(str::to_owned));
        });
        output.textures_delta.clear();
        answer
    }

    #[test]
    fn a_card_whose_job_ended_fades_out_as_it_last_stood() {
        let ctx = egui::Context::default();
        assert_eq!(
            at(&ctx, 0.0, Some("Slicing")),
            Some(("Slicing".into(), 1.0))
        );
        let close_s = f64::from(motion::CLOSE.seconds(&ctx));
        let (words, left) = at(&ctx, close_s / 2.0, None).expect("still fading");
        assert_eq!(words, "Slicing", "the words it last had");
        assert!((left - 0.5).abs() < 1e-3, "half of it is left, got {left}");
        assert_eq!(at(&ctx, close_s, None), None, "gone once the fade is over");
    }

    #[test]
    fn under_reduce_motion_a_card_goes_at_once() {
        let ctx = egui::Context::default();
        motion::reduce(&ctx, true);
        at(&ctx, 0.0, Some("Slicing"));
        assert_eq!(at(&ctx, 0.01, None), None);
    }
}
