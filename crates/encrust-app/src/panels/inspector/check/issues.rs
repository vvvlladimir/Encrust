//! The list of issues the check found: a row per kind and layer, filtered by kind.

use core_analysis::{Risk, RiskKind};
use egui::Color32;

use crate::check::covered_mm2;
use crate::i18n::tr;
use crate::ui::{Segment, Segmented, hint, issue_list, issue_row, kept};

use super::tint;

/// Rows listed before the rest are only counted: a stack with hundreds of islands needs
/// supports, not scrolling.
const LISTED: usize = 200;

/// Which kind of issue the list shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Filter {
    #[default]
    All,
    Islands,
    Levers,
    Peel,
    Cups,
    Thin,
}

impl Filter {
    pub(super) fn of(kind: &RiskKind) -> Self {
        match kind {
            RiskKind::Island { .. } | RiskKind::Floating { .. } => Self::Islands,
            RiskKind::Lever { .. } => Self::Levers,
            RiskKind::Peel { .. } => Self::Peel,
            RiskKind::SuctionCup { .. } => Self::Cups,
            RiskKind::Thin { .. } => Self::Thin,
        }
    }

    pub(super) fn name(self) -> String {
        match self {
            Self::All => tr!("check-filter-all"),
            Self::Islands => tr!("check-filter-islands"),
            Self::Levers => tr!("check-filter-levers"),
            Self::Peel => tr!("check-filter-peel"),
            Self::Cups => tr!("check-filter-cups"),
            Self::Thin => tr!("check-filter-thin"),
        }
    }

    pub(super) fn admits(self, kind: &RiskKind) -> bool {
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

/// Every issue of one kind on one layer, which is one row of the list.
pub(super) struct Row {
    pub(super) layer: usize,
    pub(super) filter: Filter,
    /// What stood on an island taken out, rather than an island itself.
    pub(super) hanging: bool,
    pub(super) tint: Color32,
    pub(super) count: usize,
    pub(super) area_mm2: f32,
    /// The worst reading among them: stress for a lever, force for a peel, resin held
    /// for a cup, and the inverse of the width for a thin section.
    pub(super) worst: f32,
    pub(super) lever_mm: f32,
    pub(super) width_mm: f32,
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
pub(super) fn rows(risks: &[Risk]) -> Vec<Row> {
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
pub(super) fn filter(ui: &mut egui::Ui, risks: &[Risk]) -> Filter {
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
pub(super) fn listing(
    ui: &mut egui::Ui,
    shown: usize,
    risks: &[Risk],
    filter: Filter,
) -> Option<Vec<Risk>> {
    let rows: Vec<Row> = rows(risks)
        .into_iter()
        .filter(|row| filter == Filter::All || row.filter == filter)
        .collect();
    let picked = issue_list(ui, |ui| {
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

/// The issues of the row `filter` admits on the next layer past `shown`, or the one
/// before it when not `forward`.
pub(super) fn neighbour(
    risks: &[Risk],
    filter: Filter,
    shown: usize,
    forward: bool,
) -> Option<Vec<Risk>> {
    let rows: Vec<Row> = rows(risks)
        .into_iter()
        .filter(|row| filter == Filter::All || row.filter == filter)
        .collect();
    let row = match forward {
        true => rows.iter().find(|row| row.layer > shown),
        false => rows.iter().rev().find(|row| row.layer < shown),
    }?;
    Some(
        risks
            .iter()
            .filter(|risk| row.holds(risk))
            .copied()
            .collect(),
    )
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
pub(super) fn issue_name(risk: &Risk) -> String {
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
pub(super) fn explain(risk: &Risk) -> String {
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

#[cfg(test)]
mod tests;
