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
use crate::ui::{
    describe, hint, icon, readings, secondary_button, section, section_with_actions, theme,
};
use crate::workspace::{Mode, Tool};

mod issues;

pub(crate) use issues::issue_name;
use issues::{Filter, filter, listing, neighbour};

/// What the stack will fail on: a way to take the islands out, the verdict, every issue a
/// click away from its layer, and what pulls hardest on the film.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let Some((risks, removed)) = window
        .measured()
        .map(|measured| (measured.risks(), measured.removed_islands()))
    else {
        waiting(ui, window);
        return;
    };

    let islands = risks.iter().any(|risk| Filter::Islands.admits(&risk.kind));
    if islands || removed > 0 {
        section(ui, &tr!("check-filter-islands"), None, |ui| {
            if islands_action(ui, window.machine, &risks, removed) {
                support_islands(window, &risks);
            }
        });
    }
    if risks.is_empty() {
        section(ui, &tr!("check-issues"), None, verdict);
    } else {
        found(ui, window, &risks);
    }
    if risks.iter().any(|risk| Filter::Cups.admits(&risk.kind)) {
        section(ui, &tr!("check-filter-cups"), None, |ui| {
            if secondary_button(ui, icon::DRAIN, &tr!("check-drill-cups")).clicked() {
                crate::shortcuts::pick(window, Tool::Drain);
            }
        });
    }
    pull(ui, window);
}

/// Every issue a click away from its layer, narrowed to one kind, and the heading's
/// arrows to the issues either side of the layer in view.
fn found(ui: &mut egui::Ui, window: &mut Window, risks: &[Risk]) {
    let shown = window.machine.preview.layer();
    let arrows = [
        (icon::LAYER_UP, tr!("check-previous")),
        (icon::LAYER_DOWN, tr!("check-next")),
    ];
    let arrows = arrows
        .each_ref()
        .map(|(glyph, name)| (*glyph, name.as_str()));
    let (arrow, listed) = section_with_actions(ui, &tr!("check-issues"), &arrows, |ui| {
        describe(ui, &tr!("check-issues-help"));
        let filter = filter(ui, risks);
        (filter, listing(ui, shown, risks, filter))
    });
    let (filter, picked) = listed.unwrap_or_default();
    let stepped = arrow.and_then(|index| neighbour(risks, filter, shown, index == 1));
    if let Some(picked) = picked.or(stepped) {
        go_to(window, &picked);
    }
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
                crate::ui::spinner(ui);
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

/// The card that says the check found nothing to fail on.
fn verdict(ui: &mut egui::Ui) {
    let colors = theme::colors();
    egui::Frame::new()
        .fill(colors.ok_wash)
        .corner_radius(theme::R_SURFACE)
        .inner_margin(theme::CARD_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.label(
                    RichText::new(icon::CLEAR)
                        .font(theme::icon(22.0))
                        .color(colors.ok),
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    let title = tr!("check-nothing-found");
                    ui.label(RichText::new(title).heading().color(colors.text_high));
                    ui.label(
                        RichText::new(tr!("check-nothing-found-why"))
                            .font(theme::small())
                            .color(colors.text_mid),
                    );
                });
            });
        });
}

/// Supporting the islands or taking them out of the written file, side by side, or once
/// taken out, putting them back. Answers whether Support was pressed.
fn islands_action(
    ui: &mut egui::Ui,
    machine: &mut Machine,
    risks: &[Risk],
    removed: usize,
) -> bool {
    let slicing = &mut machine.slicing;
    let islands = risks
        .iter()
        .any(|risk| matches!(risk.kind, RiskKind::Island { .. }));
    if slicing.remove_islands {
        removed_block(ui, removed);
        hanging_action(ui, slicing, risks);
        if secondary_button(ui, icon::RESET, &tr!("check-put-islands-back")).clicked() {
            slicing.remove_islands = false;
            slicing.hanging_passes = 0;
        }
        return islands
            && secondary_button(ui, icon::SUPPORTS, &tr!("check-support-islands")).clicked();
    }
    if !islands {
        return false;
    }
    let mut support = false;
    ui.columns(2, |columns| {
        support = secondary_button(
            &mut columns[0],
            icon::SUPPORTS,
            &tr!("check-support-islands"),
        )
        .clicked();
        if secondary_button(&mut columns[1], icon::ERASE, &tr!("check-remove-islands")).clicked() {
            slicing.remove_islands = true;
        }
    });
    describe(ui, &tr!("check-remove-islands-note"));
    support
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
pub fn go_to(window: &mut Window, risks: &[Risk]) {
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
