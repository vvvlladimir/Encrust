use core_analysis::{Measured, Risk, RiskKind, equivalent_disc_mm};
use egui::{Color32, RichText};

use crate::panels::Window;
use crate::state::Machine;
use crate::ui::{
    Segment, Segmented, describe, hint, icon, issue_row, list, readings, secondary_button, section,
    theme,
};
use crate::workspace::Mode;

/// Rows listed before the rest are only counted: a stack with hundreds of islands needs
/// supports, not scrolling.
const LISTED: usize = 200;

/// Which kind of issue the list shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Filter {
    #[default]
    All,
    Islands,
    Levers,
    Peel,
}

impl Filter {
    fn of(kind: &RiskKind) -> Self {
        match kind {
            RiskKind::Island { .. } => Self::Islands,
            RiskKind::Lever { .. } => Self::Levers,
            RiskKind::Peel { .. } => Self::Peel,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Islands => "Islands",
            Self::Levers => "Levers",
            Self::Peel => "Peel",
        }
    }

    fn admits(self, kind: &RiskKind) -> bool {
        matches!(
            (self, kind),
            (Self::All, _)
                | (Self::Islands, RiskKind::Island { .. })
                | (Self::Levers, RiskKind::Lever { .. })
                | (Self::Peel, RiskKind::Peel { .. })
        )
    }
}

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

    section(ui, "Verdict", None, |ui| {
        if let Some(layer) = verdict(ui, worst.as_ref()) {
            go_to(window.machine, layer);
        }
        islands_action(ui, window.machine, &risks, removed);
    });
    if !risks.is_empty() {
        let layers = rows(&risks)
            .iter()
            .map(|row| row.layer)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let found = format!(
            "{} on {}",
            count(risks.len(), "issue", "issues"),
            count(layers, "layer", "layers")
        );
        section(ui, "Issues", Some(&found), |ui| {
            let filter = filter(ui, &risks);
            if let Some(layer) = listing(ui, window.machine.preview.layer(), &risks, filter) {
                go_to(window.machine, layer);
            }
        });
    }
    pull(ui, window);
}

/// What the check has found so far, for the inspector's heading, and its colour.
pub fn fact(window: &Window) -> Option<(String, Color32)> {
    let colors = theme::colors();
    let Some(measured) = window.measured() else {
        return window
            .machine
            .preview
            .is_measuring()
            .then(|| ("checking".to_owned(), colors.text_low));
    };
    Some(match measured.worst() {
        None => ("no issues".to_owned(), colors.text_mid),
        Some(worst) => (
            count(measured.risks().len(), "issue", "issues"),
            tint(&worst),
        ),
    })
}

/// Before there is anything to read: the stack being read, or the way to have it cut.
fn waiting(ui: &mut egui::Ui, window: &mut Window) {
    if window.machine.preview.is_measuring() || window.machine.preview.is_building() {
        section(ui, "Checking", None, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                hint(ui, "Reading every layer of the stack...");
            });
        });
        return;
    }
    section(ui, "Checking", None, |ui| {
        hint(
            ui,
            "The check reads the layers, which are cut once the model is shown beside its \
             layer.",
        );
        if secondary_button(ui, icon::SIDE_BY_SIDE, "Cut the layers").clicked() {
            *window.mode = Mode::Preview;
            window.view.options.mask_only = false;
        }
    });
}

/// Which layers pull hardest on the film as the plate lifts, above the bottom block.
///
/// Every row is listed whether its peak is known or not: a print setting edited meanwhile
/// is measured again, and a block that comes and goes takes the field under it out from
/// under the pointer.
fn pull(ui: &mut egui::Ui, window: &mut Window) {
    let measured = window.measured();
    if measured.is_none() && !window.machine.preview.is_measuring() {
        return;
    }
    let waiting = || "measuring".to_owned();
    let hardest = measured.and_then(Measured::hardest_pull);
    let widest = measured.and_then(Measured::largest_growth);
    let rows = vec![
        (
            "Hardest pull",
            hardest.map_or_else(waiting, |peak| format!("layer {}", peak.layer + 1)),
        ),
        (
            "Pulls like a disc of",
            hardest.map_or_else(waiting, |peak| {
                format!("{:.1} mm", equivalent_disc_mm(peak.value))
            }),
        ),
        (
            "Widest step",
            widest.map_or_else(waiting, |peak| format!("layer {}", peak.layer + 1)),
        ),
        (
            "Grows by",
            widest.map_or_else(waiting, |peak| format!("{:.0} mm2", peak.value)),
        ),
    ];
    section(ui, "Peel", None, |ui| readings(ui, &rows));
}

/// The colour an issue is drawn in: islands in the colour the mask paints them, the rest
/// as warnings, since they only might fail.
pub fn tint(risk: &Risk) -> Color32 {
    let colors = theme::colors();
    match risk.kind {
        RiskKind::Island { .. } => colors.danger,
        RiskKind::Lever { .. } | RiskKind::Peel { .. } => colors.warn,
    }
}

/// The layer the print fails on, and why, on a card that goes there when clicked. Answers
/// the layer to go to.
fn verdict(ui: &mut egui::Ui, worst: Option<&Risk>) -> Option<usize> {
    let colors = theme::colors();
    let (glyph, tint, wash, title, why) = match worst {
        None => (
            icon::CLEAR,
            colors.text_mid,
            colors.raised,
            "Nothing found to fail on".to_owned(),
            "No islands, no neck pulled past its limit, and no layer pulling the film too hard."
                .to_owned(),
        ),
        Some(risk) => {
            let wash = match risk.kind {
                RiskKind::Island { .. } => colors.danger_wash,
                _ => colors.warn_wash,
            };
            (
                icon::WARNING,
                tint(risk),
                wash,
                format!("{} at layer {}", verdict_word(risk), risk.layer + 1),
                explain(risk),
            )
        }
    };
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
    let layer = worst.map(|risk| risk.layer)?;
    let response = card
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.clicked().then_some(layer)
}

/// Taking every island out of the written file, or putting them back.
fn islands_action(ui: &mut egui::Ui, machine: &mut Machine, risks: &[Risk], removed: usize) {
    if machine.slicing.remove_islands {
        removed_block(ui, removed);
        if secondary_button(ui, icon::RESET, "Put the islands back").clicked() {
            machine.slicing.remove_islands = false;
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
    let label = format!("Remove {}", count(islands, "island", "islands"));
    if secondary_button(ui, icon::ERASE, &label).clicked() {
        machine.slicing.remove_islands = true;
    }
    describe(
        ui,
        "Erases them from the written file and checks again: whatever stood only on an \
         island goes with it.",
    );
}

/// What taking the islands out came to, on a quiet card of its own.
fn removed_block(ui: &mut egui::Ui, removed: usize) {
    let colors = theme::colors();
    let title = removed_note(removed).unwrap_or_else(|| "No islands to take out".to_owned());
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
                        RichText::new("Erased from the file; what stood only on them went too.")
                            .font(theme::small())
                            .color(colors.text_low),
                    );
                });
            });
        });
}

/// Every issue of one kind on one layer, which is one row of the list.
struct Row {
    layer: usize,
    filter: Filter,
    tint: Color32,
    count: usize,
    area_mm2: f32,
    /// The worst reading among them: stress for a lever, force for a peel.
    worst: f32,
    lever_mm: f32,
}

/// The issues folded into rows. They arrive sorted by layer, and within a layer by kind.
fn rows(risks: &[Risk]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for risk in risks {
        let filter = Filter::of(&risk.kind);
        let (area_mm2, worst, lever_mm) = match risk.kind {
            RiskKind::Island { area_mm2 } => (area_mm2, 0.0, 0.0),
            RiskKind::Lever {
                stress_mpa,
                lever_mm,
                ..
            } => (0.0, stress_mpa, lever_mm),
            RiskKind::Peel { force_n } => (0.0, force_n, 0.0),
        };
        match rows.last_mut() {
            Some(row) if row.layer == risk.layer && row.filter == filter => {
                row.count += 1;
                row.area_mm2 += area_mm2;
                if worst > row.worst {
                    (row.worst, row.lever_mm) = (worst, lever_mm);
                }
            }
            _ => rows.push(Row {
                layer: risk.layer,
                filter,
                tint: tint(risk),
                count: 1,
                area_mm2,
                worst,
                lever_mm,
            }),
        }
    }
    rows
}

/// The chips that narrow the list to one kind, shown only when there is more than one
/// kind to choose between.
fn filter(ui: &mut egui::Ui, risks: &[Risk]) -> Filter {
    let id = ui.id().with("issue-filter");
    let tally = |filter: Filter| {
        risks
            .iter()
            .filter(|risk| filter.admits(&risk.kind))
            .count()
    };
    let present: Vec<(Filter, usize)> = [Filter::Islands, Filter::Levers, Filter::Peel]
        .into_iter()
        .map(|filter| (filter, tally(filter)))
        .filter(|(_, count)| *count > 0)
        .collect();
    let mut chosen = ui
        .data(|data| data.get_temp::<Filter>(id))
        .filter(|chosen| present.iter().any(|(filter, _)| filter == chosen))
        .unwrap_or_default();
    if present.len() < 2 {
        return Filter::All;
    }

    let labels: Vec<(Filter, String)> = std::iter::once((Filter::All, "All".to_owned()))
        .chain(
            present
                .iter()
                .map(|(filter, count)| (*filter, format!("{} {count}", filter.name()))),
        )
        .collect();
    let segments: Vec<Segment<'_, Filter>> = labels
        .iter()
        .map(|(filter, label)| Segment::new(*filter, label.as_str()))
        .collect();
    Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, &mut chosen);
    ui.data_mut(|data| data.insert_temp(id, chosen));
    chosen
}

/// The rows `filter` admits. Answers the layer of the row clicked.
fn listing(ui: &mut egui::Ui, shown: usize, risks: &[Risk], filter: Filter) -> Option<usize> {
    let rows: Vec<Row> = rows(risks)
        .into_iter()
        .filter(|row| filter == Filter::All || row.filter == filter)
        .collect();
    let picked = list(ui, |ui| {
        let mut picked = None;
        for row in rows.iter().take(LISTED) {
            let place = format!("Layer {}", row.layer + 1);
            let selected = row.layer == shown;
            if issue_row(ui, row.tint, &place, &detail(row), selected).clicked() {
                picked = Some(row.layer);
            }
        }
        picked
    });
    if rows.len() > LISTED {
        hint(ui, &format!("and {} more layers", rows.len() - LISTED));
    }
    picked
}

fn go_to(machine: &mut Machine, layer: usize) {
    machine.preview.set_layer(layer);
    machine.preview.set_playing(false);
}

/// What one row holds, short enough for the right of it.
fn detail(row: &Row) -> String {
    match (row.filter, row.count) {
        (Filter::Islands, 1) => format!("island · {:.2} mm2", row.area_mm2),
        (Filter::Islands, n) => format!("{n} islands · {:.2} mm2", row.area_mm2),
        (Filter::Levers, 1) => format!("lever · {:.1} mm · {:.0} MPa", row.lever_mm, row.worst),
        (Filter::Levers, n) => format!("{n} levers · {:.0} MPa", row.worst),
        _ => format!("peel · {:.0} N", row.worst),
    }
}

fn verdict_word(risk: &Risk) -> &'static str {
    match risk.kind {
        RiskKind::Island { .. } => "Fails",
        RiskKind::Lever { .. } | RiskKind::Peel { .. } => "Likely to fail",
    }
}

/// Why the worst issue fails the print, in a sentence.
fn explain(risk: &Risk) -> String {
    match risk.kind {
        RiskKind::Island { area_mm2 } => format!(
            "An island of {area_mm2:.2} mm2 cures over nothing: it sticks to the film and \
             stays in the vat. Support it, or remove it."
        ),
        RiskKind::Lever {
            stress_mpa,
            lever_mm,
            neck_mm2,
        } => format!(
            "The neck under it, {neck_mm2:.2} mm2, takes about {stress_mpa:.0} MPa from a \
             pull {lever_mm:.1} mm off to one side. Support it nearer its middle."
        ),
        RiskKind::Peel { force_n } => format!(
            "The film pulls on this layer with about {force_n:.0} N. Tilt the model, hollow \
             it, or lift slower."
        ),
    }
}

fn removed_note(removed: usize) -> Option<String> {
    (removed > 0).then(|| {
        format!(
            "{} taken out of the file",
            count(removed, "island", "islands")
        )
    })
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn island(layer: usize, area_mm2: f32) -> Risk {
        Risk {
            layer,
            at_mm: [0.0; 2],
            kind: RiskKind::Island { area_mm2 },
        }
    }

    #[test]
    fn islands_on_one_layer_are_one_row_with_their_area_summed() {
        let peel = Risk {
            layer: 515,
            at_mm: [0.0; 2],
            kind: RiskKind::Peel { force_n: 60.0 },
        };
        let risks = [
            island(464, 0.01),
            island(464, 0.01),
            island(515, 1.8),
            island(515, 2.5),
            peel,
        ];
        let rows = rows(&risks);
        let described: Vec<(usize, String)> = rows
            .iter()
            .map(|row| (row.layer + 1, detail(row)))
            .collect();
        assert_eq!(
            described,
            vec![
                (465, "2 islands · 0.02 mm2".to_owned()),
                (516, "2 islands · 4.30 mm2".to_owned()),
                (516, "peel · 60 N".to_owned()),
            ]
        );
    }
}
