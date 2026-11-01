//! The sheet that carries an update through: what the release brings, then its download,
//! signature check and install. See `docs/design/ui-screens.md`, Update.

use egui::{Align, Layout, ScrollArea, Ui};

use crate::i18n::tr;
use crate::state::Machine;
use crate::ui::{
    StepState, ago, bullet, dialog, dialog_head, hairline, hint, icon, inline_button, progress_bar,
    quiet_button, release_hero, steps, tag_label, theme,
};
use crate::updates::{
    Note, NoteGroup, Offer, Progress, RELEASES, Stage, Step, Updates, from_store, now_s, open_store,
};

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
    let Some(offer) = updates.in_hand().cloned() else {
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

/// What it is against what this is, and the release page for the long notes.
fn hero(ui: &mut Ui, offer: &Offer) {
    ui.horizontal(|ui| {
        let from = Updates::running().to_string();
        let to = offer.version.to_string();
        let page = format!("{RELEASES}/tag/v{to}");
        let line = hero_line(offer, &from, &to);
        let button_w = 140.0;
        ui.allocate_ui(egui::vec2(ui.available_width() - button_w, 40.0), |ui| {
            release_hero(ui, &tr!("app-version", version = to.clone()), &line);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = tr!("updates-release-page");
            if quiet_button(ui, icon::ELSEWHERE, &label, false, true).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(page));
            }
        });
    });
}

/// What it replaces and with what, when it went out and how big it is, as the feed says.
fn hero_line(offer: &Offer, from: &str, to: &str) -> String {
    let mut parts = vec![tr!(
        "update-sheet-from-to",
        from = from.to_owned(),
        to = to.to_owned()
    )];
    if let Some(at) = offer.published_s {
        parts.push(tr!("update-released", when = ago(Some(at), now_s())));
    }
    if let Some(size) = offer.download.as_ref().and_then(|download| download.size) {
        parts.push(tr!("update-size", mb = megabytes(size)));
    }
    parts.join("  ·  ")
}

/// Bytes as the sheet writes them, in MB to one place.
fn megabytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.0)
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
                tag_label(ui, text, tone(NoteGroup::of(text), colors));
            }
            Note::Item(text) => bullet(ui, text),
            Note::Text(text) => hint(ui, text),
        }
    }
}

/// A group's tag: an area in `ok`, Faster in the accent, the rest plain.
fn tone(group: NoteGroup, colors: &theme::Palette) -> (egui::Color32, egui::Color32) {
    match group {
        NoteGroup::Area => (colors.ok, colors.ok_wash),
        NoteGroup::Faster => (colors.accent_soft, colors.accent_wash),
        NoteGroup::Plain => (colors.text_mid, colors.active),
    }
}

/// Skip, Later and the install, or how far the install has got. Whether the sheet is put
/// away by it.
fn foot(ui: &mut Ui, updates: &mut Updates, offer: &Offer) -> bool {
    if let Some((progress, elapsed_s)) = updates.progress() {
        let cancel = installing(ui, progress, elapsed_s);
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
fn installing(ui: &mut Ui, progress: &Progress, elapsed_s: f64) -> bool {
    let (received, total) = progress.bytes();
    let step = progress.step();
    let amount = amount_words(received, total);
    let left = progress
        .seconds_left(elapsed_s)
        .map(|seconds| tr!("update-time-left", seconds = seconds));
    ui.horizontal(|ui| {
        hint(ui, &amount);
        if let Some(left) = &left {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| hint(ui, left));
        }
    });
    let fraction = total.map(|total| (received as f64 / total as f64) as f32);
    progress_bar(
        ui,
        if step == Step::Download {
            fraction
        } else {
            Some(1.0)
        },
    );
    step_row(ui, step)
}

/// The download's size so far, of how much when known.
fn amount_words(received: u64, total: Option<u64>) -> String {
    let mb = megabytes;
    match total {
        Some(total) => tr!(
            "update-downloading-of",
            got = mb(received),
            total = mb(total)
        ),
        None => tr!("update-downloading", got = mb(received)),
    }
}

/// The three steps of an update, with Cancel while the download runs. Answers whether it was
/// pressed.
fn step_row(ui: &mut Ui, step: Step) -> bool {
    let mut cancel = false;
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
