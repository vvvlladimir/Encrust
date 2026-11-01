//! The plate's figures at a glance, and what stands behind them on demand.

use egui::{
    Align2, CornerRadius, Rect, Response, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};

use super::focus_ring;
use crate::ui::{icon, kept, theme, theme::motion};

/// One cell of a [`figure_grid`]: what it is, the figure, and the figure's unit.
pub struct Figure<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub unit: &'a str,
}

/// Two by two figures in one bordered box: each name over its figure, the name cut with
/// an ellipsis, the figure never wrapped.
pub fn figure_grid(ui: &mut Ui, figures: &[Figure<'_>; 4]) {
    let width = ui.available_width();
    let (rect, _) =
        ui.allocate_exact_size(vec2(width, 2.0 * FIGURE_H + 3.0 * HAIRLINE), Sense::hover());
    let colors = theme::colors();
    ui.painter()
        .rect_filled(rect, theme::R_CONTROL, colors.hairline);
    let cell_w = ((width - 3.0 * HAIRLINE) / 2.0).max(0.0);
    for (index, figure) in figures.iter().enumerate() {
        let (column, row) = ((index % 2) as f32, (index / 2) as f32);
        let min = rect.min
            + vec2(HAIRLINE, HAIRLINE)
            + vec2(column, row) * vec2(cell_w + HAIRLINE, FIGURE_H + HAIRLINE);
        let cell = Rect::from_min_size(min, vec2(cell_w, FIGURE_H));
        paint_cell(ui, cell, corner(index), figure);
        let response = ui.interact(cell, ui.id().with(("figure", index)), Sense::hover());
        let read = format!("{} {} {}", figure.name, figure.value, figure.unit);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, read.trim_end()));
    }
}

/// The outer corner of the box cell `index` stands in, rounded to fit inside its border.
fn corner(index: usize) -> CornerRadius {
    let r = theme::R_CONTROL.nw.saturating_sub(1);
    match index {
        0 => CornerRadius {
            nw: r,
            ..CornerRadius::ZERO
        },
        1 => CornerRadius {
            ne: r,
            ..CornerRadius::ZERO
        },
        2 => CornerRadius {
            sw: r,
            ..CornerRadius::ZERO
        },
        _ => CornerRadius {
            se: r,
            ..CornerRadius::ZERO
        },
    }
}

fn paint_cell(ui: &Ui, cell: Rect, corner: CornerRadius, figure: &Figure<'_>) {
    let colors = theme::colors();
    let painter = ui.painter_at(cell);
    painter.rect_filled(cell, corner, colors.base);
    let inner = cell.shrink2(FIGURE_PAD);
    let mut name = egui::text::LayoutJob::simple_singleline(
        figure.name.to_owned(),
        theme::small(),
        colors.text_low,
    );
    name.wrap = egui::text::TextWrapping::truncate_at_width(inner.width());
    let name = painter.layout_job(name);
    let value_top = inner.top() + name.size().y + 1.0;
    painter.galley(inner.left_top(), name, colors.text_low);
    let value = painter.layout_job(figure_job(figure.value));
    let unit_left = inner.left() + value.size().x + VALUE_UNIT_GAP;
    let baseline = value_top + value.size().y;
    painter.galley(pos2(inner.left(), value_top), value, colors.text_high);
    if !figure.unit.is_empty() {
        painter.text(
            pos2(unit_left, baseline - 1.0),
            Align2::LEFT_BOTTOM,
            figure.unit,
            theme::small(),
            colors.text_low,
        );
    }
}

/// `figure` with each run of letters in it — `m`, `s` of `17m26s` — set as a unit beside
/// the numbers.
fn figure_job(figure: &str) -> egui::text::LayoutJob {
    let colors = theme::colors();
    let mut job = egui::text::LayoutJob::default();
    let mut rest = figure;
    while let Some(first) = rest.chars().next() {
        let letters = first.is_alphabetic();
        let end = rest
            .find(|c: char| c.is_alphabetic() != letters)
            .unwrap_or(rest.len());
        let (font, color) = if letters {
            (theme::small(), colors.text_low)
        } else {
            (theme::figure(), colors.text_high)
        };
        let leading = if job.is_empty() { 0.0 } else { UNIT_GAP };
        job.append(&rest[..end], leading, egui::TextFormat::simple(font, color));
        rest = &rest[end..];
    }
    job
}

/// Room between a figure and a unit inside it (`17 m 26 s`), and the unit after it, points.
const UNIT_GAP: f32 = 3.0;
const VALUE_UNIT_GAP: f32 = 6.0;

/// A row of the grid, the rule between cells, and the room inside a cell, points.
const FIGURE_H: f32 = 46.0;
const HAIRLINE: f32 = 1.0;
const FIGURE_PAD: egui::Vec2 = vec2(10.0, 6.0);

/// A *Details* row that opens what `add` draws between whatever stands above it and the
/// row itself, so a card standing on the stage's foot grows upward. `summary` stands at the
/// row's right while it is closed. The row's state is kept between runs.
pub fn details_fold<R>(
    ui: &mut Ui,
    label: &str,
    summary: Option<&str>,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let id = ui.id().with(("details", label));
    let open = kept::get::<bool>(ui.ctx(), id.with("open")).unwrap_or(false);
    let how_open = motion::fold(ui.ctx(), id, open);

    let inner = (how_open > 0.0).then(|| unfold(ui, id, how_open, Grows::Up, add));
    let row = details_row(ui, label, summary, open, how_open);
    if row.clicked() {
        kept::set(ui.ctx(), id.with("open"), &!open);
    }
    inner
}

/// The edge a fold opens from: a block's body grows down from its heading, a card's details
/// up from the row under them.
pub(super) enum Grows {
    Down,
    Up,
}

/// Draws `add` at its full height but shows `how_open` of it, from the edge `grows` names,
/// sliding and fading in with it. Fully open, it is drawn as it stands, and measured.
pub(super) fn unfold<R>(
    ui: &mut Ui,
    id: egui::Id,
    how_open: f32,
    grows: Grows,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let height_id = id.with("height");
    if how_open >= 1.0 {
        let drawn = ui.scope(add);
        let measured = drawn.response.rect.height();
        ui.data_mut(|data| data.insert_temp(height_id, measured));
        return drawn.inner;
    }
    let full_h = ui
        .data(|data| data.get_temp::<f32>(height_id))
        .unwrap_or(0.0);
    // Whole points: a card standing on its foot is placed from its height, and a fraction
    // there sets the whole card shivering by a pixel as the fold opens.
    let shown = Rect::from_min_size(
        ui.cursor().min,
        vec2(ui.available_width(), (full_h * how_open).round()),
    );
    let slide = match grows {
        Grows::Up => (1.0 - how_open) * (full_h + UNFOLD_SLIDE),
        Grows::Down => (1.0 - how_open) * UNFOLD_SLIDE,
    };
    let room = Rect::from_min_size(
        shown.min - vec2(0.0, slide),
        vec2(shown.width(), f32::INFINITY),
    );
    let mut child = ui.new_child(UiBuilder::new().max_rect(room).layout(*ui.layout()));
    child.set_clip_rect(shown.intersect(ui.clip_rect()));
    child.set_opacity(how_open);
    let inner = add(&mut child);
    let measured = child.min_rect().height();
    if (measured - full_h).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(height_id, measured));
        ui.ctx().request_discard("a fold measured what it holds");
    }
    ui.advance_cursor_after_rect(shown);
    inner
}

/// Points what a fold holds slides with it as it opens.
const UNFOLD_SLIDE: f32 = 6.0;

/// The row that opens and closes a `details_fold`: a caret that turns, its label, and the
/// summary fading out as the fold opens.
fn details_row(
    ui: &mut Ui,
    label: &str,
    summary: Option<&str>,
    open: bool,
    how_open: f32,
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), DETAILS_ROW_H), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, true, open, label));
    let colors = theme::colors();
    // Open, the row stays washed, as the end of what it opened.
    let rest = egui::Color32::TRANSPARENT.lerp_to_gamma(colors.raised, how_open);
    let fill = super::hover_mix(ui, &response, rest, colors.raised);
    let ink = if response.hovered() {
        colors.text_mid
    } else {
        colors.text_low
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let caret = painter.layout_no_wrap(icon::CARET_DOWN.to_owned(), theme::icon(12.0), ink);
    let caret_at = rect.left_center() + vec2(DETAILS_PAD + 6.0, 0.0);
    let corner = caret_at - caret.size() / 2.0;
    let turned = egui::epaint::TextShape::new(corner, caret, ink)
        .with_angle_and_anchor(std::f32::consts::PI * how_open, Align2::CENTER_CENTER);
    painter.add(turned);
    painter.text(
        caret_at + vec2(12.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        theme::label(),
        ink,
    );
    if let Some(summary) = summary {
        painter.text(
            rect.right_center() - vec2(DETAILS_PAD, 0.0),
            Align2::RIGHT_CENTER,
            summary,
            theme::small(),
            colors.text_low.gamma_multiply(1.0 - how_open),
        );
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

const DETAILS_ROW_H: f32 = 26.0;
const DETAILS_PAD: f32 = 4.0;

/// Figures split two ways, as Details splits the plate's into the models' and the
/// supports': a row of `heads` over the columns, then each row's name and its two figures,
/// a hairline under every row.
pub fn split_readings(ui: &mut Ui, heads: [&str; 2], rows: &[(String, String, String)]) {
    let colors = theme::colors();
    let width = ui.available_width();
    let column_w = width * SPLIT_COLUMN_SHARE;
    let rights = |rect: Rect| [rect.right() - column_w - SPLIT_GAP, rect.right()];
    let rule = |ui: &Ui, rect: Rect| {
        let stroke = egui::Stroke::new(HAIRLINE, colors.hairline);
        ui.painter()
            .hline(rect.x_range(), rect.bottom() - 0.5, stroke);
    };
    let (rect, _) = ui.allocate_exact_size(vec2(width, SPLIT_ROW_H), Sense::hover());
    for (head, right) in heads.iter().zip(rights(rect)) {
        ui.painter().text(
            pos2(right, rect.center().y),
            Align2::RIGHT_CENTER,
            head,
            theme::split_head(),
            colors.text_mid,
        );
    }
    rule(ui, rect);
    for (name, first, second) in rows {
        let (rect, response) = ui.allocate_exact_size(vec2(width, SPLIT_ROW_H), Sense::hover());
        rule(ui, rect);
        let read = format!("{name}: {} {first}, {} {second}", heads[0], heads[1]);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &read));
        let painter = ui.painter();
        let room = (rights(rect)[0] - column_w - rect.left()).max(0.0);
        let mut job = egui::text::LayoutJob::simple_singleline(
            name.clone(),
            theme::reading(),
            colors.text_mid,
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(room);
        let name = painter.layout_job(job);
        painter.galley(
            rect.left_center() - vec2(0.0, name.size().y / 2.0),
            name,
            colors.text_mid,
        );
        for (figure, right) in [first, second].into_iter().zip(rights(rect)) {
            painter.text(
                pos2(right, rect.center().y),
                Align2::RIGHT_CENTER,
                figure,
                theme::figures(12.5),
                colors.text_high,
            );
        }
    }
}

/// A row of a split, the share of its width each figure's column takes, and the room
/// between the two columns, points.
const SPLIT_ROW_H: f32 = 26.0;
const SPLIT_COLUMN_SHARE: f32 = 0.3;
const SPLIT_GAP: f32 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::fixtures;

    #[test]
    fn a_grid_stands_two_rows_of_figures_tall_however_long_its_names() {
        let ctx = fixtures::context();
        let long = "Resin left over when everything on the plate is printed".repeat(4);
        let figures = [
            Figure {
                name: &long,
                value: "17 m 26 s",
                unit: "",
            },
            Figure {
                name: "Resin",
                value: "42.1",
                unit: "ml",
            },
            Figure {
                name: "Weight",
                value: "46.3",
                unit: "g",
            },
            Figure {
                name: "Price",
                value: "—",
                unit: "",
            },
        ];
        let mut height = None;
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            let top = ui.cursor().top();
            figure_grid(ui, &figures);
            height = Some(ui.cursor().top() - top - ui.spacing().item_spacing.y);
        });
        let expected = 2.0 * FIGURE_H + 3.0 * HAIRLINE;
        let height = height.expect("the grid is drawn");
        assert!(
            (height - expected).abs() < 0.5,
            "expected {expected}, got {height}"
        );
    }

    #[test]
    fn a_closed_fold_draws_nothing_and_remembers_being_opened() {
        let ctx = fixtures::context();
        motion::reduce(&ctx, true);
        let mut drawn = false;
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            details_fold(ui, "Details", Some("2 models"), |_| drawn = true);
        });
        assert!(!drawn, "closed, the body is not drawn");

        let mut opened = None;
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            let key = ui.id().with(("details", "Details")).with("open");
            kept::set(ui.ctx(), key, &true);
            opened = Some(key);
        });
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            details_fold(ui, "Details", None, |_| drawn = true);
        });
        assert!(
            drawn,
            "opened, under Reduce motion the body is there at once"
        );
        let key = opened.expect("the fold's key");
        assert_eq!(kept::get::<bool>(&ctx, key), Some(true));
    }
}
