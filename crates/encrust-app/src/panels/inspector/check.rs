use core_analysis::{Measured, Risk, RiskKind, equivalent_disc_mm};
use core_geometry::Vec3;
use egui::{Color32, RichText};

use crate::check::{Place, Stage, covered_mm2};
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::section::{cut_height, height_range};
use crate::preview::{on_plate, plate_fingerprint};
use crate::state::Machine;
use crate::status::Status;
use crate::supports::ISLANDS_GROUP;
use crate::ui::{
    Segment, Segmented, describe, focus_ring, hint, icon, issue_row, kept, list, readings,
    secondary_button, section, theme,
};
use crate::workspace::{Mode, Tool};

/// Rows listed before the rest are only counted: a stack with hundreds of islands needs
/// supports, not scrolling.
const LISTED: usize = 200;

/// Which kind of issue the list shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Filter {
    #[default]
    All,
    Islands,
    Levers,
    Peel,
    Cups,
    Thin,
}

impl Filter {
    fn of(kind: &RiskKind) -> Self {
        match kind {
            RiskKind::Island { .. } | RiskKind::Floating { .. } => Self::Islands,
            RiskKind::Lever { .. } => Self::Levers,
            RiskKind::Peel { .. } => Self::Peel,
            RiskKind::SuctionCup { .. } => Self::Cups,
            RiskKind::Thin { .. } => Self::Thin,
        }
    }

    fn name(self) -> String {
        match self {
            Self::All => tr!("check-filter-all"),
            Self::Islands => tr!("check-filter-islands"),
            Self::Levers => tr!("check-filter-levers"),
            Self::Peel => tr!("check-filter-peel"),
            Self::Cups => tr!("check-filter-cups"),
            Self::Thin => tr!("check-filter-thin"),
        }
    }

    fn admits(self, kind: &RiskKind) -> bool {
        matches!(
            (self, kind),
            (Self::All, _)
                | (
                    Self::Islands,
                    RiskKind::Island { .. } | RiskKind::Floating { .. }
                )
                | (Self::Levers, RiskKind::Lever { .. })
                | (Self::Peel, RiskKind::Peel { .. })
                | (Self::Cups, RiskKind::SuctionCup { .. })
                | (Self::Thin, RiskKind::Thin { .. })
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

    section(ui, &tr!("check-verdict"), None, |ui| {
        if verdict(ui, worst.as_ref())
            && let Some(worst) = worst
        {
            go_to(window, &[worst]);
        }
        islands_action(ui, window.machine, &risks, removed);
        let islands = risks.iter().any(|risk| Filter::Islands.admits(&risk.kind));
        if islands && secondary_button(ui, icon::SUPPORTS, &tr!("check-support-islands")).clicked()
        {
            support_islands(window, &risks);
        }
        let cups = risks.iter().any(|risk| Filter::Cups.admits(&risk.kind));
        if cups && secondary_button(ui, icon::DRAIN, &tr!("check-drill-cups")).clicked() {
            crate::shortcuts::pick(window, Tool::Drain);
        }
    });
    if !risks.is_empty() {
        let layers = rows(&risks)
            .iter()
            .map(|row| row.layer)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let found = tr!("check-found", issues = risks.len(), layers = layers);
        section(ui, &tr!("check-issues"), Some(&found), |ui| {
            describe(ui, &tr!("check-issues-help"));
            let filter = filter(ui, &risks);
            if let Some(picked) = listing(ui, window.machine.preview.layer(), &risks, filter) {
                go_to(window, &picked);
            }
        });
    }
    pull(ui, window);
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

/// Every issue of one kind on one layer, which is one row of the list.
struct Row {
    layer: usize,
    filter: Filter,
    /// What stood on an island taken out, rather than an island itself.
    hanging: bool,
    tint: Color32,
    count: usize,
    area_mm2: f32,
    /// The worst reading among them: stress for a lever, force for a peel, resin held
    /// for a cup, and the inverse of the width for a thin section.
    worst: f32,
    lever_mm: f32,
    width_mm: f32,
}

impl Row {
    /// Whether `risk` is one of the issues folded into this row.
    fn holds(&self, risk: &Risk) -> bool {
        risk.layer == self.layer
            && Filter::of(&risk.kind) == self.filter
            && matches!(risk.kind, RiskKind::Floating { .. }) == self.hanging
    }
}

/// The issues folded into rows. They arrive sorted by layer, and within a layer by kind.
fn rows(risks: &[Risk]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for risk in risks {
        let filter = Filter::of(&risk.kind);
        let hanging = matches!(risk.kind, RiskKind::Floating { .. });
        let area_mm2 = covered_mm2(&risk.kind).unwrap_or(0.0);
        let (worst, lever_mm, width_mm) = match risk.kind {
            RiskKind::Island { .. } | RiskKind::Floating { .. } => (0.0, 0.0, 0.0),
            RiskKind::Lever {
                stress_mpa,
                lever_mm,
                ..
            } => (stress_mpa, lever_mm, 0.0),
            RiskKind::Peel { force_n } => (force_n, 0.0, 0.0),
            RiskKind::SuctionCup { volume_mm3, .. } => (volume_mm3, 0.0, 0.0),
            RiskKind::Thin { width_mm } => (1.0 / width_mm.max(f32::EPSILON), 0.0, width_mm),
        };
        match rows.last_mut() {
            Some(row) if row.holds(risk) => {
                row.count += 1;
                row.area_mm2 += area_mm2;
                if worst > row.worst {
                    (row.worst, row.lever_mm, row.width_mm) = (worst, lever_mm, width_mm);
                }
            }
            _ => rows.push(Row {
                layer: risk.layer,
                filter,
                hanging,
                tint: tint(risk),
                count: 1,
                area_mm2,
                worst,
                lever_mm,
                width_mm,
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
    let kinds = [
        Filter::Islands,
        Filter::Levers,
        Filter::Peel,
        Filter::Cups,
        Filter::Thin,
    ];
    let present: Vec<(Filter, usize)> = kinds
        .into_iter()
        .map(|filter| (filter, tally(filter)))
        .filter(|(_, count)| *count > 0)
        .collect();
    let mut chosen = kept::get::<Filter>(ui.ctx(), id)
        .filter(|chosen| present.iter().any(|(filter, _)| filter == chosen))
        .unwrap_or_default();
    if present.len() < 2 {
        return Filter::All;
    }

    let labels: Vec<(Filter, String)> = std::iter::once((Filter::All, Filter::All.name()))
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
    kept::set(ui.ctx(), id, &chosen);
    chosen
}

/// The rows `filter` admits. Answers the issues of the row clicked.
fn listing(ui: &mut egui::Ui, shown: usize, risks: &[Risk], filter: Filter) -> Option<Vec<Risk>> {
    let rows: Vec<Row> = rows(risks)
        .into_iter()
        .filter(|row| filter == Filter::All || row.filter == filter)
        .collect();
    let picked = list(ui, |ui| {
        let mut picked = None;
        for row in rows.iter().take(LISTED) {
            let place = tr!("check-layer", layer = row.layer + 1);
            let selected = row.layer == shown;
            if issue_row(ui, row.tint, &place, &detail(row), selected).clicked() {
                picked = Some(
                    risks
                        .iter()
                        .filter(|risk| row.holds(risk))
                        .copied()
                        .collect(),
                );
            }
        }
        picked
    });
    if rows.len() > LISTED {
        hint(ui, &tr!("check-more-layers", count = rows.len() - LISTED));
    }
    picked
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

/// What one row holds, short enough for the right of it.
fn detail(row: &Row) -> String {
    if row.hanging {
        let area = format!("{:.2}", row.area_mm2);
        return tr!("check-row-hanging", area = area);
    }
    let count = row.count;
    match (row.filter, count) {
        (Filter::Islands, _) => tr!(
            "check-row-islands",
            count = count,
            area = format!("{:.2}", row.area_mm2)
        ),
        (Filter::Levers, 1) => tr!(
            "check-row-lever",
            lever = format!("{:.1}", row.lever_mm),
            stress = format!("{:.0}", row.worst)
        ),
        (Filter::Levers, _) => tr!(
            "check-row-levers",
            count = count,
            stress = format!("{:.0}", row.worst)
        ),
        (Filter::Cups, 1) => tr!("check-row-cup", area = format!("{:.0}", row.area_mm2)),
        (Filter::Cups, _) => tr!(
            "check-row-cups",
            count = count,
            area = format!("{:.1}", row.area_mm2)
        ),
        (Filter::Thin, _) => tr!(
            "check-row-thin",
            count = count,
            width = format!("{:.2}", row.width_mm)
        ),
        _ => tr!("check-row-peel", force = format!("{:.0}", row.worst)),
    }
}

/// What the worst issue is, named plainly rather than as a failure.
fn issue_name(risk: &Risk) -> String {
    match risk.kind {
        RiskKind::Island { .. } => tr!("check-issue-island"),
        RiskKind::Floating { .. } => tr!("check-issue-hanging"),
        RiskKind::Lever { .. } => tr!("check-issue-lever"),
        RiskKind::Peel { .. } => tr!("check-issue-peel"),
        RiskKind::SuctionCup { .. } => tr!("check-issue-cup"),
        RiskKind::Thin { .. } => tr!("check-issue-thin"),
    }
}

/// Why the worst issue fails the print, in a sentence.
fn explain(risk: &Risk) -> String {
    match risk.kind {
        RiskKind::Floating { area_mm2 } => {
            tr!("check-explain-hanging", area = format!("{area_mm2:.2}"))
        }
        RiskKind::Island { area_mm2 } => {
            tr!("check-explain-island", area = format!("{area_mm2:.2}"))
        }
        RiskKind::Lever {
            stress_mpa,
            lever_mm,
            neck_mm2,
        } => tr!(
            "check-explain-lever",
            neck = format!("{neck_mm2:.2}"),
            stress = format!("{stress_mpa:.0}"),
            lever = format!("{lever_mm:.1}")
        ),
        RiskKind::Peel { force_n } => {
            tr!("check-explain-peel", force = format!("{force_n:.0}"))
        }
        RiskKind::SuctionCup {
            area_mm2,
            volume_mm3,
        } => tr!(
            "check-explain-cup",
            area = format!("{area_mm2:.0}"),
            volume = format!("{:.2}", volume_mm3 / 1000.0)
        ),
        RiskKind::Thin { width_mm } => {
            tr!("check-explain-thin", width = format!("{width_mm:.2}"))
        }
    }
}

fn removed_note(removed: usize) -> Option<String> {
    (removed > 0).then(|| tr!("check-removed", count = removed))
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

    #[test]
    fn what_hangs_over_a_removed_island_is_its_own_row_and_asks_for_supports() {
        let hanging = Risk {
            layer: 40,
            at_mm: [0.0; 2],
            kind: RiskKind::Floating { area_mm2: 3.5 },
        };
        let rows = rows(&[island(40, 1.0), hanging]);
        let described: Vec<String> = rows.iter().map(detail).collect();
        assert_eq!(
            described,
            ["island · 1.00 mm2", "hangs over nothing · 3.50 mm2"]
        );
        assert!(
            Filter::Islands.admits(&hanging.kind),
            "Support every island takes it in"
        );
        assert_eq!(issue_name(&hanging), "Part left hanging");
    }

    #[test]
    fn a_cup_and_a_thin_section_read_as_what_they_measure() {
        let cup = Risk {
            layer: 9,
            at_mm: [0.0; 2],
            kind: RiskKind::SuctionCup {
                area_mm2: 25.0,
                volume_mm3: 1800.0,
            },
        };
        let thin = Risk {
            layer: 20,
            at_mm: [0.0; 2],
            kind: RiskKind::Thin { width_mm: 0.08 },
        };
        let described: Vec<String> = rows(&[cup, thin]).iter().map(detail).collect();
        assert_eq!(described, ["suction cup · 25 mm2", "thin · 0.08 mm"]);
    }
}
