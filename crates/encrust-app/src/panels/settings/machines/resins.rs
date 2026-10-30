//! The Resins tab: the resins set up on the machine as a table, narrowed by type, with the
//! one picked opened in its form or put under the plate.

use egui::{Align, Layout, Rect, RichText, Ui, pos2, vec2};
use printer_profiles::{Kind, MaterialProfile};

use super::{Action, ZONE_GAP};
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::confirm::cost;
use crate::panels::settings::frame::{SIDE_PAD, Table, child, scrolled, with_footer};
use crate::panels::settings::resin;
use crate::settings::{Deleting, Node, pool_for, resins_of};
use crate::ui::{
    BinLook, Confirm, confirm_bin, confirm_button, describe, filter_chip, icon, inline_button,
    quiet_button, theme, zone_button,
};

/// The resins' columns, each starting at a share of the width; the figures are set flush
/// right against the start of the next. The room before the name holds the resin's colour.
const TABLE: Table = Table {
    heads: || {
        vec![
            tr!("table-profile"),
            tr!("resins-type"),
            tr!("resins-layer"),
            tr!("print-exposure"),
            tr!("resins-bottom"),
        ]
    },
    columns: &[0.0, 0.46, 0.66, 0.78, 0.9],
    words: 2,
    lead: 26.0,
};
/// The row of filters over the table.
const FILTERS_H: f32 = 52.0;
/// The Add a resin menu: wide enough for a resin's name beside its delete button, and no
/// wider, because a row with its button at the right end takes whatever it is offered.
const MENU_W: f32 = 240.0;

/// The tab, laid out in `content`: the form of the resin being edited, the table, or the
/// ways to a first resin when there is nothing to put in it.
pub(super) fn tab(
    ui: &mut Ui,
    content: Rect,
    window: &mut Window,
    printer: &str,
    action: &mut Option<Action>,
) {
    let settings = &window.machine.settings;
    let picked = match &settings.node {
        Some(Node::Resin { resin, .. }) => Some(resin.clone()),
        _ => None,
    };
    let catalogue = &window.machine.slicing.catalogue;
    if resins_of(catalogue, printer).next().is_none() && !settings.editing_resin {
        let room = content.shrink2(vec2(SIDE_PAD, 20.0));
        return first_resin(ui, room, catalogue, printer, action);
    }
    let (mut body, mut bar) = with_footer(ui, content);
    if settings.editing_resin && settings.resin.is_some() {
        scrolled(&mut body, |ui| {
            if quiet_button(ui, icon::BACK, &tr!("resins-every"), false, true).clicked() {
                window.machine.settings.editing_resin = false;
            }
            resin::form(ui, window);
        });
    } else {
        let top = body.max_rect();
        let filters = top.with_max_y(top.top() + FILTERS_H);
        filter_row(
            &mut child(&mut body, filters.shrink2(vec2(SIDE_PAD, 0.0))),
            window,
            printer,
            action,
        );
        let rows = Rect::from_min_max(
            pos2(top.left() + SIDE_PAD, filters.bottom()),
            pos2(top.right() - SIDE_PAD, top.bottom()),
        );
        table(
            &mut child(&mut body, rows),
            window,
            printer,
            picked.as_deref(),
            action,
        );
    }
    actions(&mut bar, window, printer, picked.as_deref(), action);
}

/// The two ways to a machine's first resin, side by side over the whole tab. Adding one
/// asks what from only when the pool has something to offer.
fn first_resin(
    ui: &mut Ui,
    room: Rect,
    catalogue: &printer_profiles::Catalogue,
    printer: &str,
    action: &mut Option<Action>,
) {
    let half = (room.width() - ZONE_GAP) / 2.0;
    let left = room.with_max_x(room.left() + half);
    let right = room.with_min_x(left.right() + ZONE_GAP);
    let by_hand = tr!("resins-by-hand");
    let add = zone_button(ui, left, icon::ADD, &tr!("resins-add"), &by_hand);
    if pool_for(catalogue, printer).next().is_none() {
        if add.clicked() {
            *action = Some(Action::NewResin(printer.to_owned()));
        }
    } else {
        egui::Popup::menu(&add).show(|ui| add_choices(ui, catalogue, printer, action));
    }
    let from_file = tr!("resins-from-file");
    if zone_button(ui, right, icon::IMPORT, &tr!("resins-import"), &from_file).clicked() {
        *action = Some(Action::Import(Kind::Resin));
    }
}

/// Every type of resin on the machine as a filter, and the ways to add one.
fn filter_row(ui: &mut Ui, window: &mut Window, printer: &str, action: &mut Option<Action>) {
    let catalogue = &window.machine.slicing.catalogue;
    let mut kinds: Vec<String> = resins_of(catalogue, printer)
        .map(|(_, resin)| resin.details.kind.trim().to_owned())
        .filter(|kind| !kind.is_empty())
        .collect();
    kinds.sort_by_key(|kind| kind.to_lowercase());
    kinds.dedup();
    let chosen = &mut window.machine.settings.kind;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if filter_chip(ui, &tr!("resins-all-types"), chosen.is_none()).clicked() {
            *chosen = None;
        }
        for kind in kinds {
            if filter_chip(ui, &kind, chosen.as_deref() == Some(kind.as_str())).clicked() {
                *chosen = Some(kind);
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            add_menu(ui, catalogue, printer, action);
            if inline_button(ui, icon::IMPORT, &tr!("settings-import"), false, true).clicked() {
                *action = Some(Action::Import(Kind::Resin));
            }
        });
    });
}

/// A new resin, or one out of the pool of every resin some printer has.
fn add_menu(
    ui: &mut Ui,
    catalogue: &printer_profiles::Catalogue,
    printer: &str,
    action: &mut Option<Action>,
) {
    let button = inline_button(ui, icon::ADD, &tr!("resins-add"), false, true);
    // A pool resin's bin asks by a second press, so a click inside leaves the menu open.
    egui::Popup::menu(&button)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| add_choices(ui, catalogue, printer, action));
}

/// What the menu of a new resin offers: one from nothing, or one out of the pool.
fn add_choices(
    ui: &mut Ui,
    catalogue: &printer_profiles::Catalogue,
    printer: &str,
    action: &mut Option<Action>,
) {
    ui.set_width(MENU_W);
    if ui.button(tr!("settings-new-resin")).clicked() {
        *action = Some(Action::NewResin(printer.to_owned()));
        ui.close();
    }
    ui.separator();
    ui.label(
        RichText::new(tr!("resins-from-pool"))
            .font(theme::small())
            .color(theme::colors().text_low),
    );
    let mut empty = true;
    for (id, resin) in pool_for(catalogue, printer) {
        empty = false;
        ui.horizontal(|ui| {
            if ui.button(&resin.name).clicked() {
                *action = Some(Action::AddResin {
                    printer: printer.to_owned(),
                    resin: id.to_owned(),
                });
                ui.close();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let deleting = Deleting::Resin(id.to_owned());
                let side = theme::BUTTON_H;
                let (_, bin) = ui.allocate_exact_size(vec2(side, side), egui::Sense::click());
                let bin = bin.on_hover_text(cost(catalogue, &deleting));
                if confirm_bin(ui, bin, BinLook::Quiet, &tr!("resins-delete"), true) {
                    *action = Some(Action::Delete(deleting));
                }
            });
        });
    }
    if empty {
        describe(ui, &tr!("resins-all-here"));
    }
}

/// The resins on the machine, the type filter applied, under a row of column names.
fn table(
    ui: &mut Ui,
    window: &Window,
    printer: &str,
    picked: Option<&str>,
    action: &mut Option<Action>,
) {
    let settings = &window.machine.settings;
    let slicing = &window.machine.slicing;
    let in_hand = (slicing.printer_id.as_deref() == Some(printer))
        .then_some(slicing.resin_id.as_deref())
        .flatten();
    let wanted = settings.kind.as_deref();
    let shown: Vec<(&str, &MaterialProfile)> = resins_of(&slicing.catalogue, printer)
        .filter(|(_, resin)| wanted.is_none_or(|kind| kind == resin.details.kind.trim()))
        .collect();
    TABLE.head_row(ui);
    let scroll = egui::ScrollArea::vertical().id_salt("resin-table");
    scroll.auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if shown.is_empty() {
            ui.add_space(12.0);
            describe(ui, &tr!("resins-none-here"));
        }
        for (id, resin) in shown {
            let here = resin.starting_point(printer);
            let row = resin_row(ui, &here, resin, picked == Some(id), in_hand == Some(id));
            let node = Node::Resin {
                printer: printer.to_owned(),
                resin: id.to_owned(),
            };
            if row.double_clicked() {
                *action = Some(Action::Edit(node));
            } else if row.clicked() {
                *action = Some(Action::Pick(node));
            }
        }
    });
}

/// One resin: its colour and name, its type, and what this machine exposes it at.
fn resin_row(
    ui: &mut Ui,
    here: &MaterialProfile,
    resin: &MaterialProfile,
    picked: bool,
    in_hand: bool,
) -> egui::Response {
    let colors = theme::colors();
    let cells = [
        resin.name.clone(),
        resin.details.kind.clone(),
        tr!(
            "value-um",
            value = format!("{:.0}", here.layer_height_mm * 1000.0)
        ),
        tr!("value-s", value = format!("{:.1}", here.exposure_s)),
        tr!("value-s", value = format!("{:.0}", here.bottom_exposure_s)),
    ];
    let response = TABLE.row(ui, &resin.name, picked, &cells);
    let rect = response.rect;
    let painter = ui.painter_at(rect);
    let chip = Rect::from_center_size(pos2(rect.left() + 12.0, rect.center().y), vec2(9.0, 9.0));
    painter.rect_filled(chip, 3.0, theme::resin_swatch(resin.details.color));
    if in_hand {
        let at = pos2(rect.right() - 10.0, rect.center().y);
        let glyph = theme::icon(13.0);
        painter.text(
            at,
            egui::Align2::RIGHT_CENTER,
            icon::IN_HAND,
            glyph,
            colors.accent_soft,
        );
    }
    response.on_hover_text_at_pointer(match in_hand {
        true => tr!("resins-row-in-hand"),
        false => tr!("resins-row"),
    })
}

/// What the bar offers for the resin picked in the table, and putting it under the plate.
fn actions(
    ui: &mut Ui,
    window: &Window,
    printer: &str,
    picked: Option<&str>,
    action: &mut Option<Action>,
) {
    let editing = window.machine.settings.editing_resin;
    let node = |resin: &str| Node::Resin {
        printer: printer.to_owned(),
        resin: resin.to_owned(),
    };
    let some = picked.is_some();
    if !editing && quiet_button(ui, icon::EDIT, &tr!("profiles-edit"), false, some).clicked() {
        *action = picked.map(|resin| Action::Edit(node(resin)));
    }
    if quiet_button(ui, icon::DUPLICATE, &tr!("action-duplicate"), false, some).clicked() {
        *action = picked.map(|resin| Action::DuplicateResin {
            printer: printer.to_owned(),
            resin: resin.to_owned(),
        });
    }
    if quiet_button(ui, icon::EXPORT, &tr!("common-export"), false, some).clicked() {
        *action = picked.map(|resin| Action::Export(Kind::Resin, resin.to_owned()));
    }
    let take_off = picked.map(|resin| Deleting::ResinOff {
        printer: printer.to_owned(),
        resin: resin.to_owned(),
    });
    let tooltip = take_off.as_ref().map_or_else(String::new, |deleting| {
        cost(&window.machine.slicing.catalogue, deleting)
    });
    let button = Confirm {
        glyph: icon::REMOVE,
        text: &tr!("resins-take-off"),
        asked: &tr!("confirm-asked-take-off"),
        tooltip: &tooltip,
        enabled: some,
    };
    if confirm_button(ui, &button) {
        *action = take_off.map(Action::Delete);
    }
    let slicing = &window.machine.slicing;
    let in_hand =
        slicing.printer_id.as_deref() == Some(printer) && slicing.resin_id.as_deref() == picked;
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let label = if some && in_hand {
            tr!("machines-in-hand")
        } else {
            tr!("resins-use")
        };
        let enabled = some && !in_hand;
        let glyph = if some && in_hand { icon::IN_HAND } else { "" };
        if inline_button(ui, glyph, &label, enabled, enabled).clicked() {
            *action = picked.map(|resin| Action::UseResin {
                printer: printer.to_owned(),
                resin: resin.to_owned(),
            });
        }
    });
}
