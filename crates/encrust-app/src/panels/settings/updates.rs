//! The Updates page: whether the window looks for a new release, and what it found.

use egui::{RichText, Ui};

use crate::panels::settings::form_card;
use crate::ui::{hint, icon, inline_button, primary_button, switch, theme};
use crate::updates::{Note, Offer, RELEASES, Stage, Updates, now_s};

/// How many lines of the release notes the page shows; the rest is on the release page. A
/// scroll area inside the page's own collapses to its first frame, so the list is cut instead.
const NOTES_SHOWN: usize = 24;

/// What the switch costs, stated where it is turned on; see the privacy table in the README.
const PRIVACY: &str = "One request a day to GitHub for the newest release. It carries this \
                       build's version and nothing else, and GitHub sees your address. Nothing \
                       is downloaded or installed until you ask.";

pub fn form(ui: &mut Ui, updates: &mut Updates) {
    form_card(ui, "Checking", |ui| {
        switch(
            ui,
            &mut updates.prefs.check,
            "Look for a new release once a day",
        );
        hint(ui, PRIVACY);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let busy = updates.is_busy();
            if inline_button(ui, icon::UPDATE, "Check now", false, !busy).clicked() {
                updates.check_now();
            }
            ui.add_space(8.0);
            hint(
                ui,
                &format!(
                    "This is {}, last looked {}",
                    Updates::running(),
                    ago(updates.prefs.checked_at_s, now_s())
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
        Stage::Checking { .. } => busy(ui, "Looking for a new release"),
        Stage::Current => hint(ui, "This is the newest release."),
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
        Stage::Installing { .. } => busy(ui, "Downloading and checking the signature"),
        Stage::Installed(_) => {
            hint(ui, "Installed. It runs from the next start.");
            if primary_button(ui, icon::UPDATE, "Restart now", true).clicked() {
                updates.restart();
            }
        }
        _ => {
            if let Stage::Failed { message, .. } = &updates.stage {
                failure(ui, message);
            }
            let blocked = Updates::blocked(offer);
            if let Some(reason) = blocked {
                hint(ui, reason);
            }
            if primary_button(ui, icon::UPDATE, "Install and restart", blocked.is_none()).clicked()
            {
                updates.install();
            }
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let skipped = updates.prefs.skipped == Some(offer.version.to_string());
        let offered = matches!(updates.stage, Stage::Offered(_));
        if offered && !skipped && inline_button(ui, "", "Skip this version", false, true).clicked()
        {
            updates.skip();
        }
        ui.hyperlink_to("Release page", page);
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
        hint(ui, "And more on the release page.");
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

/// When the feed was last read, as a person says it.
fn ago(checked_at_s: Option<u64>, now_s: u64) -> String {
    let Some(at) = checked_at_s else {
        return "never".to_owned();
    };
    match now_s.saturating_sub(at) {
        0..60 => "just now".to_owned(),
        seconds @ 60..3_600 => format!("{} min ago", seconds / 60),
        seconds @ 3_600..86_400 => format!("{} h ago", seconds / 3_600),
        seconds => match seconds / 86_400 {
            1 => "yesterday".to_owned(),
            days => format!("{days} days ago"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_look_reads_as_a_person_says_it() {
        let now = 10_000_000;
        assert_eq!(ago(None, now), "never");
        assert_eq!(ago(Some(now - 5), now), "just now");
        assert_eq!(ago(Some(now - 600), now), "10 min ago");
        assert_eq!(ago(Some(now - 7_200), now), "2 h ago");
        assert_eq!(ago(Some(now - 90_000), now), "yesterday");
        assert_eq!(ago(Some(now - 3 * 86_400), now), "3 days ago");
    }
}
