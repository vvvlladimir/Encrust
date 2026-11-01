use core_analysis::{Measured, Risk, RiskKind, equivalent_disc_mm};
use core_geometry::Vec3;
use egui::{Color32, RichText};

use crate::check::{Place, Stage};
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::section::{cut_height, height_range};
use crate::preview::{on_plate, plate_fingerprint};
use crate::state::Machine;
use crate::status::Status;
use crate::supports::ISLANDS_GROUP;
use crate::ui::{describe, focus_ring, hint, icon, readings, secondary_button, section, theme};
use crate::workspace::{Mode, Tool};

mod issues;

use issues::{Filter, explain, filter, issue_name, listing, neighbour, rows};

/// What the stack will fail on: the verdict, a way to take the islands out, every issue a
/// click away from its layer, and what pulls hardest on the film.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let Some((risks, worst, removed)) = window.measured().map(|measured| {
        (
            measured.risks(),
            measured.worst(),
            measured.removed_islands(),
        )
    }) else {
        waiting(ui, window);
        return;
    };

    let found = tr!(
        "check-found",
        issues = risks.len(),
        layers = layer_count(&risks)
    );
    section(ui, &tr!("check-issues"), Some(&found), |ui| {
        if verdict(ui, worst.as_ref())
            && let Some(worst) = worst
        {
            go_to(window, &[worst]);
        }
        if risks.is_empty() {
            return;
        }
        describe(ui, &tr!("check-issues-help"));
        let filter = filter(ui, &risks);
        let shown = window.machine.preview.layer();
        if let Some(picked) = listing(ui, shown, &risks, filter) {
            go_to(window, &picked);
        }
        if let Some(picked) = stepping(ui, &risks, filter, shown) {
            go_to(window, &picked);
        }
    });
    let islands = risks.iter().any(|risk| Filter::Islands.admits(&risk.kind));
    if islands || removed > 0 {
        section(ui, &tr!("check-filter-islands"), None, |ui| {
            islands_action(ui, window.machine, &risks, removed);
            if islands
                && secondary_button(ui, icon::SUPPORTS, &tr!("check-support-islands")).clicked()
            {
                support_islands(window, &risks);
            }
        });
    }
    if risks.iter().any(|risk| Filter::Cups.admits(&risk.kind)) {
        section(ui, &tr!("check-filter-cups"), None, |ui| {
            if secondary_button(ui, icon::DRAIN, &tr!("check-drill-cups")).clicked() {
                crate::shortcuts::pick(window, Tool::Drain);
            }
        });
    }
    pull(ui, window);
    section(ui, &tr!("check-estimate"), None, |ui| {
        readings(ui, &crate::panels::plate_estimate(ui.ctx(), window));
    });
}

/// How many layers the issues stand on.
fn layer_count(risks: &[Risk]) -> usize {
    rows(risks)
        .iter()
        .map(|row| row.layer)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

/// Previous and Next, to the issue rows either side of the layer in view.
fn stepping(ui: &mut egui::Ui, risks: &[Risk], filter: Filter, shown: usize) -> Option<Vec<Risk>> {
    let mut picked = None;
    ui.columns(2, |columns| {
        let before = neighbour(risks, filter, shown, false);
        let after = neighbour(risks, filter, shown, true);
        let back = columns[0]
            .add_enabled_ui(before.is_some(), |ui| {
                secondary_button(ui, icon::LAYER_DOWN, &tr!("check-previous"))
            })
            .inner;
        let on = columns[1]
            .add_enabled_ui(after.is_some(), |ui| {
                secondary_button(ui, icon::LAYER_UP, &tr!("check-next"))
            })
            .inner;
        picked = match (back.clicked(), on.clicked()) {
            (true, _) => before,
            (_, true) => after,
            _ => None,
        };
    });
    picked
}

/// Stands a support under every island the check found, in a group of their own built
/// to the support numbers in hand, and says how many stood.
fn support_islands(window: &mut Window, risks: &[Risk]) {
    let Some(settings) = window.machine.slicing.raster_settings() else {
        return;
    };
    let Some(plan) = window.machine.preview.plan() else {
        return;
    };
    let floors: Vec<Vec3> = risks
        .iter()
        .filter(|risk| Filter::Islands.admits(&risk.kind))
        .filter_map(|risk| {
            let floor_mm = plan.top_of(risk.layer.checked_sub(1)?)?;
            Some(on_plate(&settings, risk.at_mm).extend(floor_mm))
        })
        .collect();
    let layer_mm = plan.nominal_thickness();
    let stood = window
        .tools
        .supports
        .support_islands(&mut window.doc.scene, &floors, layer_mm);
    window.machine.status = Status::Info(tr!("check-stood", count = stood, group = ISLANDS_GROUP));
}

/// What the check has found so far, for the inspector's heading, and its colour.
pub fn fact(window: &Window) -> Option<(String, Color32)> {
    let colors = theme::colors();
    let Some(measured) = window.measured() else {
        return window
            .machine
            .preview
            .is_measuring()
            .then(|| (tr!("check-fact-checking"), colors.text_low));
    };
    Some(match measured.worst() {
        None => (tr!("check-fact-none"), colors.text_mid),
        Some(worst) => (
            tr!("check-fact-issues", count = measured.risks().len()),
            tint(&worst),
        ),
    })
}

/// Before there is anything to read: the stack being read, or the way to have it cut.
fn waiting(ui: &mut egui::Ui, window: &mut Window) {
    if window.machine.preview.is_measuring() || window.machine.preview.is_building() {
        section(ui, &tr!("check-checking"), None, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                hint(ui, &tr!("check-reading"));
            });
        });
        return;
    }
    section(ui, &tr!("check-checking"), None, |ui| {
        hint(ui, &tr!("check-reads-layers"));
    });
}

/// Which layers pull hardest on the film as the plate lifts, above the bottom block.
/// Every row is listed, peak known or not: a setting edited meanwhile is measured again.
fn pull(ui: &mut egui::Ui, window: &mut Window) {
    let measured = window.measured();
    if measured.is_none() && !window.machine.preview.is_measuring() {
        return;
    }
    let waiting = || tr!("check-measuring");
    let hardest = measured.and_then(Measured::hardest_pull);
    let widest = measured.and_then(Measured::largest_growth);
    let rows = vec![
        (
            tr!("check-hardest-pull"),
            hardest.map_or_else(waiting, |peak| {
                tr!("check-layer-lower", layer = peak.layer + 1)
            }),
        ),
        (
            tr!("check-disc"),
            hardest.map_or_else(waiting, |peak| {
                let mm = format!("{:.1}", equivalent_disc_mm(f64::from(peak.value)));
                tr!("value-mm", value = mm)
            }),
        ),
        (
            tr!("check-widest-step"),
            widest.map_or_else(waiting, |peak| {
                tr!("check-layer-lower", layer = peak.layer + 1)
            }),
        ),
        (
            tr!("check-grows-by"),
            widest.map_or_else(waiting, |peak| {
                tr!("value-mm2", value = format!("{:.0}", peak.value))
            }),
        ),
    ];
    section(ui, &tr!("check-peel"), None, |ui| {
        describe(ui, &tr!("check-peel-help"));
        readings(ui, &rows);
    });
}

/// The colour an issue is drawn in: islands in the colour the mask paints them, the rest
/// as warnings, since they only might fail.
pub fn tint(risk: &Risk) -> Color32 {
    let colors = theme::colors();
    match risk.kind {
        RiskKind::Island { .. } | RiskKind::Floating { .. } => colors.danger,
        RiskKind::Lever { .. }
        | RiskKind::Peel { .. }
        | RiskKind::SuctionCup { .. }
        | RiskKind::Thin { .. } => colors.warn,
    }
}

/// The worst issue, where it is and what to do about it, on a card that goes there when
/// clicked. Answers whether it was clicked.
fn verdict(ui: &mut egui::Ui, worst: Option<&Risk>) -> bool {
    let colors = theme::colors();
    let (glyph, tint, wash, title, why) = match worst {
        None => (
            icon::CLEAR,
            colors.text_mid,
            colors.raised,
            tr!("check-nothing-found"),
            tr!("check-nothing-found-why"),
        ),
        Some(risk) => {
            let wash = match risk.kind {
                RiskKind::Island { .. } | RiskKind::Floating { .. } => colors.danger_wash,
                _ => colors.warn_wash,
            };
            (
                icon::WARNING,
                tint(risk),
                wash,
                tr!(
                    "check-worst",
                    issue = issue_name(risk),
                    layer = risk.layer + 1
                ),
                explain(risk),
            )
        }
    };
    let name = title.clone();
    let card = egui::Frame::new()
        .fill(wash)
        .corner_radius(theme::R_SURFACE)
        .inner_margin(theme::CARD_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(glyph).font(theme::icon(22.0)).color(tint));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.label(RichText::new(title).heading().color(colors.text_high));
                    ui.label(
                        RichText::new(why)
                            .font(theme::small())
                            .color(colors.text_mid),
                    );
                });
            });
        });
    if worst.is_none() {
        return false;
    }
    let response = card
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    focus_ring(ui, &response, theme::R_SURFACE);
    response.clicked()
}

/// Taking every island out of the written file, or putting them back.
fn islands_action(ui: &mut egui::Ui, machine: &mut Machine, risks: &[Risk], removed: usize) {
    let slicing = &mut machine.slicing;
    if slicing.remove_islands {
        removed_block(ui, removed);
        hanging_action(ui, slicing, risks);
        if secondary_button(ui, icon::RESET, &tr!("check-put-islands-back")).clicked() {
            slicing.remove_islands = false;
            slicing.hanging_passes = 0;
        }
        return;
    }
    let islands = risks
        .iter()
        .filter(|risk| Filter::Islands.admits(&risk.kind))
        .count();
    if islands == 0 {
        return;
    }
    let label = tr!("check-remove-islands", count = islands);
    if secondary_button(ui, icon::ERASE, &label).clicked() {
        machine.slicing.remove_islands = true;
    }
    describe(ui, &tr!("check-remove-islands-note"));
}

/// Taking out the next layer of what the islands already taken out left hanging, one
/// layer a click, as though each were an island in turn.
fn hanging_action(ui: &mut egui::Ui, slicing: &mut crate::slicing::Slicing, risks: &[Risk]) {
    let hanging = risks
        .iter()
        .filter(|risk| matches!(risk.kind, RiskKind::Floating { .. }))
        .count();
    if slicing.hanging_passes > 0 {
        hint(
            ui,
            &tr!("check-hanging-passes", count = slicing.hanging_passes),
        );
    }
    if hanging == 0 {
        return;
    }
    let label = tr!("check-remove-next", count = hanging);
    if secondary_button(ui, icon::ERASE, &label).clicked() {
        slicing.hanging_passes += 1;
    }
    describe(ui, &tr!("check-remove-next-note"));
}

/// What taking the islands out came to, on a quiet card of its own.
fn removed_block(ui: &mut egui::Ui, removed: usize) {
    let colors = theme::colors();
    let title = removed_note(removed).unwrap_or_else(|| tr!("check-no-islands-to-take"));
    egui::Frame::new()
        .fill(colors.raised)
        .corner_radius(theme::R_SURFACE)
        .inner_margin(theme::CARD_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.label(
                    RichText::new(icon::ERASE)
                        .font(theme::icon(18.0))
                        .color(colors.text_mid),
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new(title)
                            .font(theme::label())
                            .color(colors.text_high),
                    );
                    ui.label(
                        RichText::new(tr!("check-erased-note"))
                            .font(theme::small())
                            .color(colors.text_low),
                    );
                });
            });
        });
}

/// Goes to the layer `risks` lie on: the slider there, the view cut at it, and the camera
/// flown to them ringed on the cut.
fn go_to(window: &mut Window, risks: &[Risk]) {
    let Some(first) = risks.first() else {
        return;
    };
    window.tools.check.clear();
    let preview = &mut window.machine.preview;
    preview.set_layer(first.layer);
    preview.set_playing(false);
    let Some(layer_mm) = preview.section_z() else {
        return;
    };
    if *window.mode == Mode::Prepare
        && let Some((_, top)) = height_range(window.doc)
    {
        // As the strip has it, a cut at the top or past it is no cut.
        window.view.section.height_mm = Some(layer_mm).filter(|z| *z < top);
        window.view.section.playing = false;
    }
    let Some(settings) = window.machine.slicing.raster_settings() else {
        return;
    };
    let places: Vec<Place> = risks
        .iter()
        .map(|risk| Place::of(risk, &settings))
        .collect();
    let stage = Stage {
        plate: plate_fingerprint(&window.doc.scene),
        cut_mm: cut_height(*window.mode, &window.view.section, &window.machine.preview),
    };
    window.tools.check.go(&places, layer_mm, stage, tint(first));
}

fn removed_note(removed: usize) -> Option<String> {
    (removed > 0).then(|| tr!("check-removed", count = removed))
}
