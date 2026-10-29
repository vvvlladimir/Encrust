//! The Updates page: whether the window looks for a new release, and what it found.

use egui::{RichText, Ui};

use crate::i18n::tr;
use crate::panels::settings::form_card;
use crate::ui::{ago, hint, icon, inline_button, primary_button, switch, theme};
use crate::updates::{
    Note, Offer, RELEASES, STORE_PAGE, Stage, Updates, from_store, now_s, open_store,
};

/// How many lines of the release notes the page shows; the rest is on the release page. A
/// scroll area inside the page's own collapses to its first frame, so the list is cut instead.
const NOTES_SHOWN: usize = 24;

pub fn form(ui: &mut Ui, updates: &mut Updates) {
    form_card(ui, &tr!("updates-checking"), |ui| {
        if from_store() {
            hint(ui, &tr!("updates-by-store"));
            ui.hyperlink_to(tr!("updates-store-link"), STORE_PAGE);
        } else {
            switch(ui, &mut updates.prefs.check, &tr!("updates-daily"));
            // What the switch costs, stated where it is turned on; see the README's privacy table.
            hint(ui, &tr!("updates-privacy"));
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let busy = updates.is_busy();
            if inline_button(ui, icon::UPDATE, &tr!("updates-check-now"), false, !busy).clicked() {
                updates.check_now();
            }
            ui.add_space(8.0);
            hint(
                ui,
                &tr!(
                    "updates-this-is",
                    version = Updates::running().to_string(),
                    when = ago(updates.prefs.checked_at_s, now_s())
                ),
            );
        });
    });
    found(ui, updates);
}

/// What the last look found, and what can be done with it.
fn found(ui: &mut Ui, updates: &mut Updates) {
    match &updates.stage {
        Stage::Idle => {}
        Stage::Checking { .. } => busy(ui, &tr!("updates-looking")),
        Stage::Current => hint(ui, &tr!("updates-newest")),
        Stage::Failed {
            offer: None,
            message,
        } => failure(ui, message),
        Stage::Offered(offer)
        | Stage::Installing { offer, .. }
        | Stage::Installed(offer)
        | Stage::Failed {
            offer: Some(offer), ..
        } => {
            let offer = offer.clone();
            form_card(ui, &format!("Encrust {}", offer.version), |ui| {
                notes(ui, &offer.notes);
                ui.add_space(10.0);
                actions(ui, updates, &offer);
            });
        }
    }
}

fn actions(ui: &mut Ui, updates: &mut Updates, offer: &Offer) {
    let page = format!("{RELEASES}/tag/v{}", offer.version);
    match &updates.stage {
        Stage::Installing { .. } => busy(ui, &tr!("updates-installing")),
        Stage::Installed(_) => {
            hint(ui, &tr!("updates-installed"));
            if primary_button(ui, &tr!("updates-restart-now"), true).clicked() {
                updates.restart();
            }
        }
        _ if from_store() => {
            hint(ui, &tr!("updates-store-has-it"));
            if primary_button(ui, &tr!("updates-in-store"), true).clicked() {
                open_store();
            }
        }
        _ => {
            if let Stage::Failed { message, .. } = &updates.stage {
                failure(ui, message);
            }
            let blocked = Updates::blocked(offer);
            if let Some(reason) = &blocked {
                hint(ui, reason);
            }
            if primary_button(ui, &tr!("updates-install"), blocked.is_none()).clicked() {
                updates.install();
            }
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let skipped = updates.prefs.skipped == Some(offer.version.to_string());
        let offered = matches!(updates.stage, Stage::Offered(_));
        if offered && !skipped && inline_button(ui, "", &tr!("updates-skip"), false, true).clicked()
        {
            updates.skip();
        }
        ui.hyperlink_to(tr!("updates-release-page"), page);
    });
}

/// The release notes, as release-please wrote them.
fn notes(ui: &mut Ui, notes: &[Note]) {
    let colors = theme::colors();
    for note in notes.iter().take(NOTES_SHOWN) {
        match note {
            Note::Heading(text) => {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(text)
                        .font(theme::section())
                        .color(colors.text_mid),
                );
            }
            Note::Item(text) => {
                ui.label(
                    RichText::new(format!("•  {text}"))
                        .font(theme::label())
                        .color(colors.text_high),
                );
            }
            Note::Text(text) => {
                ui.label(
                    RichText::new(text)
                        .font(theme::label())
                        .color(colors.text_mid),
                );
            }
        }
    }
    if notes.len() > NOTES_SHOWN {
        hint(ui, &tr!("updates-more-notes"));
    }
}

fn busy(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        hint(ui, text);
    });
}

fn failure(ui: &mut Ui, message: &str) {
    ui.label(
        RichText::new(message)
            .font(theme::small())
            .color(theme::colors().danger),
    );
}
