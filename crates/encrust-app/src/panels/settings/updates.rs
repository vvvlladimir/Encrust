//! The Updates page: the release found, with the one action that applies, and the daily
//! look under it.

use egui::{Align, Layout, Ui};

use crate::i18n::tr;
use crate::panels::settings::form_card;
use crate::ui::{
    ago, boxed, bullet, describe, hint, icon, inline_button, quiet_button, switch, tag_label, theme,
};
use crate::updates::{Note, Offer, STORE_PAGE, Stage, Updates, from_store, now_s, open_store};

/// How many changes the box lists; the sheet has every one.
const CHANGES_SHOWN: usize = 3;

pub fn form(ui: &mut Ui, updates: &mut Updates) {
    if let Some(offer) = updates.in_hand().cloned() {
        form_card(ui, &tr!("updates-release-found"), |ui| {
            boxed(ui, |ui| release(ui, updates, &offer));
        });
    }
    form_card(ui, &tr!("updates-looking-block"), |ui| looking(ui, updates));
}

/// Its name, its first changes, and what can be done with it.
fn release(ui: &mut Ui, updates: &mut Updates, offer: &Offer) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        tag_label(ui, &tr!("update-group-new"), (colors.ok, colors.ok_wash));
        hint(ui, &tr!("app-version", version = offer.version.to_string()));
        if let Some(at) = offer.published_s {
            hint(ui, &tr!("update-released", when = ago(Some(at), now_s())));
        }
    });
    let items: Vec<&str> = offer
        .notes
        .iter()
        .filter_map(|note| match note {
            Note::Item(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for item in items.iter().take(CHANGES_SHOWN) {
        bullet(ui, item);
    }
    if items.len() > CHANGES_SHOWN {
        hint(
            ui,
            &tr!("updates-and-more", count = items.len() - CHANGES_SHOWN),
        );
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        act(ui, updates, offer);
        let every = tr!("updates-every-change");
        if quiet_button(ui, icon::CHANGES, &every, false, true).clicked() {
            updates.sheet = true;
        }
        let skipped = updates.prefs.skipped == Some(offer.version.to_string());
        let offered = matches!(updates.stage, Stage::Offered(_));
        if offered && !skipped && quiet_button(ui, "", &tr!("updates-skip"), false, true).clicked()
        {
            updates.skip();
        }
    });
    if let Stage::Failed { message, .. } = &updates.stage {
        hint(ui, message);
    }
}

/// The one action that applies: install, how far it has got, restart, or the Store.
fn act(ui: &mut Ui, updates: &mut Updates, offer: &Offer) {
    match &updates.stage {
        Stage::Installing { .. } => {
            let percent = updates.progress().and_then(|(progress, _)| {
                let (received, total) = progress.bytes();
                total.map(|total| (received * 100 / total.max(1)).min(100))
            });
            let text = percent.map_or_else(
                || tr!("updates-installing"),
                |percent| tr!("bar-update-percent", percent = percent),
            );
            crate::ui::spinner(ui);
            hint(ui, &text);
        }
        Stage::Installed(_) => {
            if inline_button(ui, "", &tr!("updates-restart-now"), true, true).clicked() {
                updates.restart();
            }
        }
        _ if from_store() => {
            if inline_button(ui, "", &tr!("updates-in-store"), true, true).clicked() {
                open_store();
            }
        }
        _ => {
            let blocked = Updates::blocked(offer);
            let install = tr!("updates-install");
            if inline_button(ui, "", &install, true, blocked.is_none()).clicked() {
                updates.install();
            }
            if let Some(why) = blocked {
                describe(ui, &why);
            }
        }
    }
}

/// The daily look, when it last looked with Check now beside it, and what that found.
fn looking(ui: &mut Ui, updates: &mut Updates) {
    if from_store() {
        hint(ui, &tr!("updates-by-store"));
        ui.hyperlink_to(tr!("updates-store-link"), STORE_PAGE);
    } else {
        switch(ui, &mut updates.prefs.check, &tr!("updates-daily"));
        // What the switch costs, behind the block's question mark; see the README's privacy table.
        describe(ui, &tr!("updates-privacy"));
    }
    ui.horizontal(|ui| {
        let when = ago(updates.prefs.checked_at_s, now_s());
        hint(ui, &tr!("updates-last-looked", when = when));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let busy = updates.is_busy();
            if inline_button(ui, icon::RESCAN, &tr!("updates-check-now"), false, !busy).clicked() {
                updates.check_now();
            }
        });
    });
    match &updates.stage {
        Stage::Checking { .. } => {
            ui.horizontal(|ui| {
                crate::ui::spinner(ui);
                hint(ui, &tr!("updates-looking"));
            });
        }
        Stage::Current => hint(ui, &tr!("updates-newest")),
        Stage::Failed {
            offer: None,
            message,
        } => hint(ui, message),
        _ => {}
    }
}
