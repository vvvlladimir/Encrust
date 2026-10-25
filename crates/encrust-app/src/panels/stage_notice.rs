use egui::{Align, Align2, Id, Layout, Rect, RichText, pos2};

use crate::i18n::tr;
use crate::panels::{Window, unreadable_profiles};
use crate::scene::ObjectId;
use crate::status::{Change, Status};
use crate::ui::{card, icon, icon_button, inline_button, progress_bar, theme};

/// Points between the cards, and how wide a job's card is drawn whatever its label
/// measures.
const GAP: f32 = 6.0;
const JOB_W: f32 = 360.0;
/// The widest a message may grow before it is cut.
const MESSAGE_W: f32 = 520.0;
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
        bottom: stage.bottom(),
    };
    let width = (stage.width() - theme::CARD_MARGIN.sum().x).clamp(0.0, MESSAGE_W);
    message(&mut stack, &mut window.machine.status, running, width);
    unready(&mut stack, window);
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
    /// The card `name` on top of those already stacked, as `add` draws it.
    fn card<R>(&mut self, name: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let shown = egui::Area::new(Id::new("stage-notice").with(name))
            .order(egui::Order::Middle)
            .fixed_pos(pos2(self.room.center().x, self.bottom))
            .pivot(Align2::CENTER_BOTTOM)
            .constrain_to(self.room)
            .show(&self.ctx, add);
        self.bottom = shown.response.rect.top() - GAP;
        shown.inner
    }
}

/// One card per job that reports progress here, the latest to start nearest the foot.
fn jobs(stack: &mut Stack, window: &mut Window) {
    if window.machine.preview.is_building()
        && job_card(
            stack,
            "preview",
            &tr!("notice-cutting-preview"),
            None,
            false,
        )
    {
        window.machine.preview.cancel();
    }
    if let Some(job) = window.machine.slicing.job.as_ref() {
        let (label, fraction) = (job.label(), job.fraction());
        if job_card(stack, "slicing", &label, fraction, job.is_cancelling()) {
            job.cancel();
        }
    }
    if let Some(job) = window.machine.network.job.as_ref() {
        let (label, fraction) = (job.label(), job.fraction());
        if job_card(stack, "network", &label, fraction, job.is_cancelling()) {
            job.cancel();
        }
    }
}

/// What is happening, how far along, and a way out. Answers whether to stop.
fn job_card(
    stack: &mut Stack,
    name: &str,
    label: &str,
    fraction: Option<f32>,
    cancelling: bool,
) -> bool {
    let colors = theme::colors();
    let mut cancel = false;
    stack.card(name, |ui| {
        card().inner_margin(theme::CARD_MARGIN).show(ui, |ui| {
            ui.set_width(JOB_W);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(!cancelling, |ui| {
                        cancel = icon_button(ui, icon::CANCEL, &tr!("common-cancel")).clicked();
                    });
                    if let Some(fraction) = fraction {
                        ui.label(
                            RichText::new(format!("{:.0} %", fraction * 100.0))
                                .font(theme::figures(11.5))
                                .color(colors.text_mid),
                        );
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        let text = RichText::new(label)
                            .font(theme::label())
                            .color(colors.text_high);
                        ui.add(egui::Label::new(text).truncate());
                    });
                });
            });
            // Slicing reports no layer counts until the stack is cut, so the bar runs rather
            // than lies.
            progress_bar(ui, fraction);
        })
    });
    cancel
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
    let label = match offered.as_slice() {
        [] => return,
        [(_, name, _)] => tr!("notice-broken-one", name = name.clone()),
        several => tr!("notice-broken-several", count = several.len()),
    };
    let (mut repair, mut keep) = (false, false);
    stack.card("broken", |ui| {
        card().inner_margin(theme::CARD_MARGIN).show(ui, |ui| {
            ui.horizontal(|ui| {
                let text = RichText::new(label)
                    .font(theme::label())
                    .color(theme::colors().warn);
                let lines: Vec<&str> = offered
                    .iter()
                    .flat_map(|(_, _, defects)| defects.iter().map(String::as_str))
                    .collect();
                ui.label(text).on_hover_text(lines.join("\n"));
                let wording = if offered.len() == 1 {
                    tr!("notice-repair")
                } else {
                    tr!("notice-repair-all")
                };
                repair = inline_button(ui, icon::REPAIR, &wording, true, true).clicked();
                keep = icon_button(ui, icon::CANCEL, &tr!("notice-keep-as-is")).clicked();
            });
        })
    });
    for (id, _, _) in &offered {
        if repair {
            let repairs = &mut window.doc.repairs;
            repairs.start(&window.doc.scene, *id, &mut window.machine.status);
        } else if keep {
            window.doc.repairs.keep_as_it_is(*id);
        }
    }
}

/// A plate with no machine or no resin cannot slice, so it says which and offers the
/// window both are set up in, for as long as either is missing (ADR 0299).
fn unready(stack: &mut Stack, window: &mut Window) {
    let slicing = &window.machine.slicing;
    let label = match (slicing.printer_id.is_some(), slicing.resin_id.is_some()) {
        (true, true) => return,
        (false, false) => tr!("notice-no-printer-resin"),
        (false, true) => tr!("notice-no-printer"),
        (true, false) => tr!("notice-no-resin"),
    };
    let mut open = false;
    stack.card("unready", |ui| {
        card().inner_margin(theme::CARD_MARGIN).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let colors = theme::colors();
                ui.label(
                    RichText::new(icon::INFO)
                        .font(theme::icon(13.0))
                        .color(colors.warn),
                );
                ui.label(
                    RichText::new(label)
                        .font(theme::label())
                        .color(colors.text_high),
                );
                open =
                    inline_button(ui, icon::PRINTER, &tr!("notice-set-up"), true, true).clicked();
            });
        })
    });
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
    if !error {
        stack
            .ctx
            .request_repaint_after(std::time::Duration::from_secs_f64(INFO_LINGER_S - since_s));
    }

    let colors = theme::colors();
    let mut frame = card().inner_margin(theme::CARD_MARGIN);
    if error {
        frame = frame.stroke(egui::Stroke::new(1.0, colors.danger.gamma_multiply(0.5)));
    }
    let mut dismissed = false;
    let shown = stack.card("message", |ui| {
        frame.show(ui, |ui| {
            ui.set_max_width(max_w);
            // Rows as tall as their text: a notice is a line or two, not a form.
            ui.spacing_mut().interact_size.y = 0.0;
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if error {
                    let glyph = RichText::new(icon::WARNING).font(theme::icon(13.0));
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

/// Counts the message's moment from `now`.
fn seen_now(ctx: &egui::Context, status: &Status, now: f64) {
    let key = Id::new(status).value();
    ctx.data_mut(|data| data.insert_temp(Id::new(SEEN), (key, now)));
}
