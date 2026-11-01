//! The sheet that carries an update through: what the release brings, then its download,
//! signature check and install. See `docs/design/ui-screens.md`, Update.

use egui::{Align, Layout, ScrollArea, Ui};

use crate::i18n::tr;
use crate::state::Machine;
use crate::ui::{
    StepState, bullet, dialog, dialog_head, hairline, hint, icon, inline_button, progress_bar,
    quiet_button, release_hero, steps, tag_label, theme,
};
use crate::updates::{Note, Offer, RELEASES, Stage, Step, Updates, from_store, open_store};

/// The sheet's width, the room left round it, and the height of what is not the changes.
const SHEET_W: f32 = 640.0;
const SCREEN_MARGIN: f32 = 96.0;
const CHROME_H: f32 = 230.0;
const CHANGES_MAX_H: f32 = 380.0;
const CHANGES_MIN_H: f32 = 120.0;
/// Room above a group of the changes but the first.
const GROUP_GAP: f32 = 12.0;

/// Shows the sheet while it is open and a release is in hand; Escape or the cross puts it
/// away, and a download goes on behind it.
pub fn ui(ctx: &egui::Context, machine: &mut Machine) {
    let updates = &mut machine.updates;
    if !updates.sheet {
        return;
    }
    let Some(offer) = in_hand(updates).cloned() else {
        updates.sheet = false;
        return;
    };
    let screen = ctx.viewport_rect();
    let width = (screen.width() - SCREEN_MARGIN).min(SHEET_W);
    let changes_h =
        (screen.height() - SCREEN_MARGIN - CHROME_H).clamp(CHANGES_MIN_H, CHANGES_MAX_H);
    let shown = dialog(ctx, egui::Id::new("update-sheet"), 18).show(ctx, |ui| {
        ui.set_width(width);
        let close = dialog_head(ui, icon::UPDATE, &tr!("update-sheet-title"));
        hairline(ui);
        hero(ui, &offer);
        hairline(ui);
        ScrollArea::vertical()
            .max_height(changes_h)
            .auto_shrink([false, true])
            .show(ui, |ui| changes(ui, &offer.notes));
        hairline(ui);
        let put_by = foot(ui, updates, &offer);
        close || put_by
    });
    if shown.inner || shown.should_close() {
        updates.sheet = false;
    }
}

/// The release the window holds: offered, being installed, installed or failed on.
fn in_hand(updates: &Updates) -> Option<&Offer> {
    match &updates.stage {
        Stage::Offered(offer)
        | Stage::Installing { offer, .. }
        | Stage::Installed(offer)
        | Stage::Failed {
            offer: Some(offer), ..
        } => Some(offer),
        _ => None,
    }
}

/// What it is against what this is, and the release page for the long notes.
fn hero(ui: &mut Ui, offer: &Offer) {
    ui.horizontal(|ui| {
        let from = Updates::running().to_string();
        let to = offer.version.to_string();
        let page = format!("{RELEASES}/tag/v{to}");
        let line = tr!("update-sheet-from-to", from = from, to = to.clone());
        let button_w = 140.0;
        ui.allocate_ui(egui::vec2(ui.available_width() - button_w, 40.0), |ui| {
            release_hero(ui, icon::RELEASE, &format!("Encrust {to}"), &line);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = tr!("updates-release-page");
            if quiet_button(ui, icon::ELSEWHERE, &label, false, true).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(page));
            }
        });
    });
}

/// The release notes, a pill over each group and a bullet before each change.
fn changes(ui: &mut Ui, notes: &[Note]) {
    let colors = theme::colors();
    for (index, note) in notes.iter().enumerate() {
        match note {
            Note::Heading(text) => {
                if index > 0 {
                    ui.add_space(GROUP_GAP);
                }
                let (name, tone) = group_of(text);
                tag_label(ui, &name, tone(colors));
            }
            Note::Item(text) => bullet(ui, text),
            Note::Text(text) => hint(ui, text),
        }
    }
}

type Tone = fn(&theme::Palette) -> (egui::Color32, egui::Color32);

/// What a group of the notes is called on the sheet, and its tint: what is new in `ok`, what
/// is better in the accent, what was fixed plain.
fn group_of(heading: &str) -> (String, Tone) {
    let lower = heading.to_lowercase();
    if lower.contains("feature") {
        (tr!("update-group-new"), |c| (c.ok, c.ok_wash))
    } else if lower.contains("perf") {
        (tr!("update-group-better"), |c| {
            (c.accent_soft, c.accent_wash)
        })
    } else if lower.contains("fix") {
        (tr!("update-group-fixed"), |c| (c.text_mid, c.active))
    } else {
        (heading.to_owned(), |c| (c.text_mid, c.active))
    }
}

/// Skip, Later and the install, or how far the install has got. Whether the sheet is put
/// away by it.
fn foot(ui: &mut Ui, updates: &mut Updates, offer: &Offer) -> bool {
    if let Some((progress, elapsed_s)) = updates.progress() {
        let (received, total) = progress.bytes();
        let step = progress.step();
        let cancel = installing(ui, (received, total), step, elapsed_s);
        if cancel {
            updates.cancel();
        }
        return false;
    }
    let mut put_by = false;
    ui.horizontal(|ui| {
        let offered = matches!(updates.stage, Stage::Offered(_));
        if offered && quiet_button(ui, "", &tr!("updates-skip"), false, true).clicked() {
            updates.skip();
            put_by = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            put_by |= act(ui, updates, offer);
            if inline_button(ui, "", &tr!("update-later"), false, true).clicked() {
                put_by = true;
            }
        });
    });
    if let Stage::Failed { message, .. } = &updates.stage {
        hint(ui, message);
    }
    put_by
}

/// The one action that applies: install, restart into what was installed, or the Store.
fn act(ui: &mut Ui, updates: &mut Updates, offer: &Offer) -> bool {
    match &updates.stage {
        Stage::Installed(_) => {
            if inline_button(ui, "", &tr!("updates-restart-now"), true, true).clicked() {
                updates.restart();
            }
        }
        _ if from_store() => {
            if inline_button(ui, "", &tr!("updates-in-store"), true, true).clicked() {
                open_store();
                return true;
            }
        }
        _ => {
            let blocked = Updates::blocked(offer);
            if inline_button(ui, "", &tr!("updates-install"), true, blocked.is_none()).clicked() {
                updates.install();
            }
            if let Some(why) = blocked {
                hint(ui, &why);
            }
        }
    }
    false
}

/// The download's amount and time left over its bar, the three steps, and Cancel while the
/// download can still stop. Whether Cancel was pressed.
fn installing(
    ui: &mut Ui,
    (received, total): (u64, Option<u64>),
    step: Step,
    elapsed_s: f64,
) -> bool {
    let mb = |bytes: u64| format!("{:.1}", bytes as f64 / 1_048_576.0);
    let amount = match total {
        Some(total) => tr!(
            "update-downloading-of",
            got = mb(received),
            total = mb(total)
        ),
        None => tr!("update-downloading", got = mb(received)),
    };
    let left = total
        .filter(|_| received > 0 && elapsed_s > 0.0)
        .map(|total| {
            let rate = received as f64 / elapsed_s;
            let seconds = (total.saturating_sub(received) as f64 / rate).ceil() as u64;
            tr!("update-time-left", seconds = seconds)
        });
    let mut cancel = false;
    ui.horizontal(|ui| {
        hint(ui, &amount);
        if let Some(left) = &left {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| hint(ui, left));
        }
    });
    let fraction = total.map(|total| (received as f64 / total as f64) as f32);
    progress_bar(
        ui,
        fraction.filter(|_| step == Step::Download).or(Some(1.0)),
    );
    ui.horizontal(|ui| {
        let state = |at: Step| match (at as u8).cmp(&(step as u8)) {
            std::cmp::Ordering::Less => StepState::Done,
            std::cmp::Ordering::Equal => StepState::Now,
            std::cmp::Ordering::Greater => StepState::Later,
        };
        let (download, verify, install) = (
            tr!("update-step-download"),
            tr!("update-step-verify"),
            tr!("update-step-install"),
        );
        steps(
            ui,
            &[
                (&download, state(Step::Download)),
                (&verify, state(Step::Verify)),
                (&install, state(Step::Install)),
            ],
        );
        if step == Step::Download {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                cancel = quiet_button(ui, "", &tr!("update-cancel"), false, true).clicked();
            });
        }
    });
    cancel
}
