use egui::{Align, Align2, Id, Layout, Rect, RichText, vec2};

use crate::panels::Window;
use crate::status::Status;
use crate::ui::{card, icon, icon_button, inline_button, progress_bar, theme};

/// Points between the top of the stage and the first card, between the cards, and how
/// wide a job's card is drawn whatever its label measures.
const INSET: f32 = 12.0;
const GAP: f32 = 6.0;
const JOB_W: f32 = 360.0;
/// The widest a message may grow before it is cut.
const MESSAGE_W: f32 = 520.0;
/// How long a message that is not a failure stays up after it changed, seconds.
const INFO_LINGER_S: f64 = 4.0;

/// Top centre of the stage: what is running, with a way to stop it, then the one thing the
/// plate is waiting on, then what happened last. The window has no status line; see
/// `docs/decisions/0217`.
pub fn ui(ui: &egui::Ui, window: &mut Window, stage: Rect) {
    egui::Area::new(Id::new("stage-notice"))
        .order(egui::Order::Middle)
        .fixed_pos(stage.center_top() + vec2(0.0, INSET))
        .pivot(Align2::CENTER_TOP)
        .constrain_to(stage)
        .show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing.y = GAP;
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                let running = jobs(ui, window);
                waiting_to_print(ui, window);
                message(ui, &mut window.machine.status, running);
            });
        });
}

/// One card per job that reports progress here. Answers whether any is running.
fn jobs(ui: &mut egui::Ui, window: &mut Window) -> bool {
    let mut running = false;
    if let Some(job) = window.machine.network.job.as_ref() {
        running = true;
        if job_card(ui, &job.label(), job.fraction(), job.is_cancelling()) {
            job.cancel();
        }
    }
    if let Some(job) = window.machine.slicing.job.as_ref() {
        running = true;
        if job_card(ui, &job.label(), job.fraction(), job.is_cancelling()) {
            job.cancel();
        }
    }
    if window.machine.preview.is_building() {
        running = true;
        if job_card(ui, "Cutting the layers to preview", None, false) {
            window.machine.preview.cancel();
        }
    }
    running
}

/// What is happening, how far along, and a way out. Answers whether to stop.
fn job_card(ui: &mut egui::Ui, label: &str, fraction: Option<f32>, cancelling: bool) -> bool {
    let colors = theme::colors();
    let mut cancel = false;
    card().inner_margin(theme::CARD_MARGIN).show(ui, |ui| {
        ui.set_width(JOB_W);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled_ui(!cancelling, |ui| {
                    cancel = icon_button(ui, icon::CANCEL, "Cancel").clicked();
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
    });
    cancel
}

/// A stack that reached the printer is not printing yet: starting it is a second,
/// deliberate press, because nobody is standing at the machine when the first one lands.
fn waiting_to_print(ui: &mut egui::Ui, window: &mut Window) {
    let Some(sent) = window.machine.network.sent.clone() else {
        return;
    };
    let mut start = false;
    card().inner_margin(theme::CARD_MARGIN).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} is on {}", sent.filename, sent.printer))
                    .font(theme::label())
                    .color(theme::colors().text_high),
            );
            start = inline_button(ui, icon::PLAY, "Start print", true, true).clicked();
        });
    });
    if start {
        window
            .machine
            .network
            .start_print(&mut window.machine.status);
    }
}

/// What happened last. A failure stays until it is put away, and its whole cause chain is
/// on hover; anything else stands for a moment, and not at all while a job is running.
fn message(ui: &mut egui::Ui, status: &mut Status, running: bool) {
    let error = status.is_error();
    let now = ui.input(|input| input.time);
    let since_s = now - changed_at(ui, status, now);
    if matches!(status, Status::Idle) || !error && (running || since_s > INFO_LINGER_S) {
        return;
    }
    if !error {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(INFO_LINGER_S - since_s));
    }

    let colors = theme::colors();
    let (glyph, tint) = if error {
        (icon::WARNING, colors.danger)
    } else {
        (icon::INFO, colors.text_low)
    };
    let mut frame = card().inner_margin(theme::CARD_MARGIN);
    if error {
        frame = frame.stroke(egui::Stroke::new(1.0, colors.danger.gamma_multiply(0.5)));
    }
    let mut dismissed = false;
    frame.show(ui, |ui| {
        ui.set_max_width(MESSAGE_W);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(RichText::new(glyph).font(theme::icon(13.0)).color(tint));
            let text = status.text().to_owned();
            let label = RichText::new(&text)
                .font(theme::label())
                .color(colors.text_high);
            let response = ui.add(egui::Label::new(label).truncate());
            if response.hovered() {
                response.on_hover_text(&text);
            }
            if error {
                dismissed = icon_button(ui, icon::CANCEL, "Put away").clicked();
            }
        });
    });
    if dismissed {
        *status = Status::Idle;
    }
}

/// When the message last changed, on egui's clock, kept in egui's memory: the status is
/// replaced wholesale wherever something happens, and carries no time of its own.
fn changed_at(ui: &egui::Ui, status: &Status, now: f64) -> f64 {
    let id = Id::new("stage-notice-seen");
    let key = Id::new((status.is_error(), status.text())).value();
    let seen = ui.data(|data| data.get_temp::<(u64, f64)>(id));
    match seen {
        Some((seen_key, at)) if seen_key == key => at,
        _ => {
            ui.data_mut(|data| data.insert_temp(id, (key, now)));
            now
        }
    }
}
