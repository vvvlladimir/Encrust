//! The issues a card of the check lists: a row per kind and layer.

use core_analysis::{Risk, RiskKind};

use crate::check::{IssueKind, Stop, covered_mm2};
use crate::i18n::tr;
use crate::ui::{hint, issue_line, issue_lines};

/// Rows listed before the rest are only counted: a stack with hundreds of islands needs
/// supports, not scrolling.
const LISTED: usize = 200;

/// The card `kind` of issue stands on.
pub(super) fn kind_of(kind: &RiskKind) -> IssueKind {
    match kind {
        RiskKind::Island { .. } | RiskKind::Floating { .. } => IssueKind::Islands,
        RiskKind::Lever { .. } => IssueKind::Necks,
        RiskKind::Peel { .. } => IssueKind::Peel,
        RiskKind::SuctionCup { .. } => IssueKind::Cups,
        RiskKind::Thin { .. } => IssueKind::Thin,
    }
}

/// Every issue of one kind on one layer, which is one row of the list.
pub(super) struct Row {
    pub(super) layer: usize,
    pub(super) name: String,
    pub(super) kind: IssueKind,
    /// What stood on an island taken out, rather than an island itself.
    pub(super) hanging: bool,
    pub(super) count: usize,
    pub(super) area_mm2: f32,
    /// The worst reading among them: stress for a lever, force for a peel, resin held
    /// for a cup, and the inverse of the width for a thin section.
    pub(super) worst: f32,
    pub(super) lever_mm: f32,
    pub(super) width_mm: f32,
}

impl Row {
    /// The issues folded into this row.
    fn issues(&self, risks: &[Risk]) -> Vec<Risk> {
        risks
            .iter()
            .filter(|risk| self.holds(risk))
            .copied()
            .collect()
    }

    /// Whether `risk` is one of the issues folded into this row.
    fn holds(&self, risk: &Risk) -> bool {
        risk.layer == self.layer
            && kind_of(&risk.kind) == self.kind
            && matches!(risk.kind, RiskKind::Floating { .. }) == self.hanging
    }
}

/// The issues folded into rows. They arrive sorted by layer, and within a layer by kind.
pub(super) fn rows(risks: &[Risk]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for risk in risks {
        let kind = kind_of(&risk.kind);
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
                name: issue_name(risk),
                kind,
                hanging,
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

/// The rows of `kind`. Answers the issues of the row clicked.
pub(super) fn listing(
    ui: &mut egui::Ui,
    shown: usize,
    risks: &[Risk],
    kind: IssueKind,
) -> Option<Vec<Risk>> {
    let rows: Vec<Row> = rows(risks)
        .into_iter()
        .filter(|row| row.kind == kind)
        .collect();
    let ink = crate::ui::theme::colors().text_mid;
    let picked = issue_lines(ui, "layers", |ui| {
        let mut picked = None;
        for row in rows.iter().take(LISTED) {
            let place = tr!("check-layer", layer = row.layer + 1);
            let selected = row.layer == shown;
            let response = issue_line(ui, &place, (&detail(row), ink), selected);
            if response.on_hover_text(&row.name).clicked() {
                picked = Some(row.issues(risks));
            }
        }
        picked
    });
    if rows.len() > LISTED {
        hint(ui, &tr!("check-more-layers", count = rows.len() - LISTED));
    }
    picked
}

/// The stops of the layers, in the order the arrows take them.
pub(super) fn layer_stops(risks: &[Risk]) -> Vec<Stop> {
    let stop = |row: Row| Stop::Layer {
        layer: row.layer,
        kind: row.kind,
        hanging: row.hanging,
    };
    rows(risks).into_iter().map(stop).collect()
}

/// The issues of the row `stop` names.
pub(super) fn issues_at(risks: &[Risk], stop: Stop) -> Vec<Risk> {
    let at = |row: &Row| {
        matches!(stop, Stop::Layer { layer, kind, hanging }
            if row.layer == layer && row.kind == kind && row.hanging == hanging)
    };
    let rows = rows(risks);
    rows.iter()
        .find(|row| at(row))
        .map_or_else(Vec::new, |row| row.issues(risks))
}

/// The stop after `at` or, not `forward`, before it, going round; from nowhere, the first
/// layer past `shown` or the last before it, so a slider moved by hand is where they start.
pub(super) fn next_stop(
    stops: &[Stop],
    at: Option<Stop>,
    shown: usize,
    forward: bool,
) -> Option<Stop> {
    let last = stops.len().checked_sub(1)?;
    let here = at.and_then(|at| stops.iter().position(|stop| *stop == at));
    let layer_of = |stop: &Stop| match stop {
        Stop::Layer { layer, .. } => Some(*layer),
        Stop::Plate(_) => None,
    };
    let index = match (here, forward) {
        (Some(index), true) => (index + 1) % stops.len(),
        (Some(index), false) => index.checked_sub(1).unwrap_or(last),
        (None, true) => stops
            .iter()
            .position(|stop| layer_of(stop).is_some_and(|layer| layer > shown))
            .unwrap_or(0),
        (None, false) => stops
            .iter()
            .rposition(|stop| layer_of(stop).is_some_and(|layer| layer < shown))
            .unwrap_or(last),
    };
    stops.get(index).copied()
}

/// What one row holds, short enough for the right of it.
fn detail(row: &Row) -> String {
    if row.hanging {
        let area = format!("{:.2}", row.area_mm2);
        return tr!("check-row-hanging", area = area);
    }
    let count = row.count;
    match (row.kind, count) {
        (IssueKind::Islands, _) => tr!(
            "check-row-islands",
            count = count,
            area = format!("{:.2}", row.area_mm2)
        ),
        (IssueKind::Necks, 1) => tr!(
            "check-row-lever",
            lever = format!("{:.1}", row.lever_mm),
            stress = format!("{:.0}", row.worst)
        ),
        (IssueKind::Necks, _) => tr!(
            "check-row-levers",
            count = count,
            stress = format!("{:.0}", row.worst)
        ),
        (IssueKind::Cups, 1) => tr!("check-row-cup", area = format!("{:.0}", row.area_mm2)),
        (IssueKind::Cups, _) => tr!(
            "check-row-cups",
            count = count,
            area = format!("{:.1}", row.area_mm2)
        ),
        (IssueKind::Thin, _) => tr!(
            "check-row-thin",
            count = count,
            width = format!("{:.2}", row.width_mm)
        ),
        _ => tr!("check-row-peel", force = format!("{:.0}", row.worst)),
    }
}

/// What the worst issue is, named plainly rather than as a failure.
pub(crate) fn issue_name(risk: &Risk) -> String {
    match risk.kind {
        RiskKind::Island { .. } => tr!("check-issue-island"),
        RiskKind::Floating { .. } => tr!("check-issue-hanging"),
        RiskKind::Lever { .. } => tr!("check-issue-lever"),
        RiskKind::Peel { .. } => tr!("check-issue-peel"),
        RiskKind::SuctionCup { .. } => tr!("check-issue-cup"),
        RiskKind::Thin { .. } => tr!("check-issue-thin"),
    }
}

#[cfg(test)]
mod tests;
