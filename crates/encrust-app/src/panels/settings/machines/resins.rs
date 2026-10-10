//! The Resins tab: the resins set up on the machine as a table, narrowed by type, with the
//! one picked opened in its form or put under the plate.

use egui::{Align, Color32, Layout, Rect, RichText, Sense, Ui, pos2, vec2};
use printer_profiles::MaterialProfile;

use super::{Action, SIDE_PAD, child, scrolled, stroke, with_footer};
use crate::panels::Window;
use crate::panels::settings::resin;
use crate::settings::{Deleting, Node, pool_for, resins_of};
use crate::ui::{describe, filter_chip, icon, icon_button, inline_button, quiet_button, theme};

/// Where each column of the table starts, as a share of its width; the figures are set
/// flush right against the start of the next.
const COLUMNS: [f32; 5] = [0.0, 0.46, 0.66, 0.78, 0.9];
const HEADS: [&str; 5] = ["Profile", "Type", "Layer", "Exposure", "Bottom"];
/// The row of filters over the table.
const FILTERS_H: f32 = 52.0;
/// The Add a resin menu: wide enough for a resin's name beside its delete button, and no
/// wider, because a row with its button at the right end takes whatever it is offered.
const MENU_W: f32 = 240.0;

/// The tab, laid out in `content`: the form of the resin being edited, or the table.
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
    let (mut body, mut bar) = with_footer(ui, content);
    if settings.editing_resin && settings.resin.is_some() {
        scrolled(&mut body, |ui| {
            if quiet_button(ui, icon::BACK, "Every resin", false, true).clicked() {
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
        if filter_chip(ui, "All types", chosen.is_none()).clicked() {
            *chosen = None;
        }
        for kind in kinds {
            if filter_chip(ui, &kind, chosen.as_deref() == Some(kind.as_str())).clicked() {
                *chosen = Some(kind);
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            add_menu(ui, catalogue, printer, action);
        });
    });
}

/// A new resin, or one out of the pool of every resin some printer has. With nothing on
/// the machine and nothing in the pool there is one thing to do, so the menu of one is a
/// button instead (ADR 0197).
fn add_menu(
    ui: &mut Ui,
    catalogue: &printer_profiles::Catalogue,
    printer: &str,
    action: &mut Option<Action>,
) {
    let first = resins_of(catalogue, printer).next().is_none()
        && pool_for(catalogue, printer).next().is_none();
    if first {
        if inline_button(ui, icon::RESIN, "Add the first resin", false, true).clicked() {
            *action = Some(Action::NewResin(printer.to_owned()));
        }
        return;
    }
    let button = inline_button(ui, icon::ADD, "Add a resin", false, true);
    egui::Popup::menu(&button).show(|ui| {
        ui.set_width(MENU_W);
        if ui.button("New resin").clicked() {
            *action = Some(Action::NewResin(printer.to_owned()));
        }
        ui.separator();
        ui.label(
            RichText::new("From the pool")
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
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, icon::REMOVE, "Delete this resin").clicked() {
                        *action = Some(Action::Ask(Deleting::Resin(id.to_owned())));
                    }
                });
            });
        }
        if empty {
            describe(ui, "Every resin you have is already on this machine.");
        }
    });
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
    head_row(ui);
    let scroll = egui::ScrollArea::vertical().id_salt("resin-table");
    scroll.auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if shown.is_empty() {
            ui.add_space(12.0);
            describe(
                ui,
                "No resin on this machine yet. Add one with Add a resin above.",
            );
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

fn head_row(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let colors = theme::colors();
    for (index, head) in HEADS.into_iter().enumerate() {
        let (at, align) = cell(rect, index);
        ui.painter()
            .text(at, align, head, theme::small(), colors.text_low);
    }
    ui.painter().hline(rect.x_range(), rect.bottom(), stroke());
}

/// Where cell `index` of a row in `rect` is drawn from: the name and the type from their
/// left, the figures from their right.
fn cell(rect: Rect, index: usize) -> (egui::Pos2, egui::Align2) {
    let x = |share: f32| rect.left() + rect.width() * share;
    match index {
        0 => (
            pos2(rect.left() + 26.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
        ),
        1 => (
            pos2(x(COLUMNS[1]), rect.center().y),
            egui::Align2::LEFT_CENTER,
        ),
        _ => {
            let right = COLUMNS
                .get(index + 1)
                .map_or(rect.right() - 30.0, |next| x(*next) - 12.0);
            (pos2(right, rect.center().y), egui::Align2::RIGHT_CENTER)
        }
    }
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
    let size = vec2(ui.available_width(), theme::TABLE_ROW_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = match (picked, response.hovered()) {
        (true, _) => colors.accent_wash,
        (false, true) => colors.hover,
        (false, false) => Color32::TRANSPARENT,
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let chip = Rect::from_center_size(pos2(rect.left() + 12.0, rect.center().y), vec2(9.0, 9.0));
    painter.rect_filled(chip, 3.0, theme::resin_swatch(resin.details.color));
    paint_cells(&painter, rect, here, resin);
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
    painter.hline(rect.x_range(), rect.bottom() - 0.5, stroke());
    response.on_hover_text_at_pointer(match in_hand {
        true => "The plate prints with this resin. Double-click to edit it.",
        false => "Double-click to edit it.",
    })
}

/// The name, the type and the three figures of a row, each in its column.
fn paint_cells(
    painter: &egui::Painter,
    rect: Rect,
    here: &MaterialProfile,
    resin: &MaterialProfile,
) {
    let colors = theme::colors();
    let cells = [
        resin.name.clone(),
        resin.details.kind.clone(),
        format!("{:.0} µm", here.layer_height_mm * 1000.0),
        format!("{:.1} s", here.exposure_s),
        format!("{:.0} s", here.bottom_exposure_s),
    ];
    for (index, text) in cells.into_iter().enumerate() {
        let (at, align) = cell(rect, index);
        let (font, tint) = match index {
            0 => (theme::label(), colors.text_high),
            1 => (theme::label(), colors.text_mid),
            _ => (theme::figures(12.5), colors.text_high),
        };
        painter.text(at, align, text, font, tint);
    }
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
    if !editing && quiet_button(ui, icon::EDIT, "Edit", false, some).clicked() {
        *action = picked.map(|resin| Action::Edit(node(resin)));
    }
    if quiet_button(ui, icon::DUPLICATE, "Duplicate", false, some).clicked() {
        *action = picked.map(|resin| Action::DuplicateResin {
            printer: printer.to_owned(),
            resin: resin.to_owned(),
        });
    }
    if quiet_button(ui, icon::REMOVE, "Take off this machine", true, some).clicked() {
        *action = picked.map(|resin| {
            Action::Ask(Deleting::ResinOff {
                printer: printer.to_owned(),
                resin: resin.to_owned(),
            })
        });
    }
    let slicing = &window.machine.slicing;
    let in_hand =
        slicing.printer_id.as_deref() == Some(printer) && slicing.resin_id.as_deref() == picked;
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let label = if some && in_hand {
            "The plate prints with this"
        } else {
            "Use this resin"
        };
        let enabled = some && !in_hand;
        if inline_button(ui, "", label, enabled, enabled).clicked() {
            *action = picked.map(|resin| Action::UseResin {
                printer: printer.to_owned(),
                resin: resin.to_owned(),
            });
        }
    });
}
