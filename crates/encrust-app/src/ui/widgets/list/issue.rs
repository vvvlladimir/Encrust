use egui::{Align2, Color32, Response, Sense, Ui, WidgetInfo, WidgetType, vec2};

use crate::ui::theme;
use crate::ui::widgets::{focus_ring, hover_mix};

/// Findings one under another, each its own rounded row, `ISSUE_GAP` apart; past
/// `ISSUE_ROWS_SHOWN` they scroll inside the list, so the sections under it stay put.
pub fn issue_list<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let shown = ISSUE_ROWS_SHOWN as f32;
    egui::ScrollArea::vertical()
        .id_salt("issue-list")
        .max_height(shown * theme::BUTTON_H + (shown - 1.0) * ISSUE_GAP)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = ISSUE_GAP;
            add(ui)
        })
        .inner
}

const ISSUE_GAP: f32 = 8.0;

/// Rows a list of findings shows before the rest scroll inside it.
const ISSUE_ROWS_SHOWN: usize = 7;

/// How strongly the accent rings the finding in hand.
const ISSUE_RING: f32 = 0.35;

/// Room between a finding's name and the aside after it, points.
const ASIDE_GAP: f32 = 8.0;

/// One finding in a list of them: a dot in its colour, what it is, any `aside` quieter
/// beside it, and `detail` at the right. The whole row is the button that goes there.
pub fn issue_row(
    ui: &mut Ui,
    tint: Color32,
    (place, aside): (&str, Option<&str>),
    detail: &str,
    selected: bool,
) -> Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::BUTTON_H), Sense::click());
    let fill = match selected {
        true => colors.picked_wash,
        false => hover_mix(ui, &response, colors.raised, colors.hover),
    };
    let ring = match selected {
        true => egui::Stroke::new(1.0, colors.accent.gamma_multiply(ISSUE_RING)),
        false => egui::Stroke::NONE,
    };
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            true,
            selected,
            format!("{place}, {detail}"),
        )
    });
    let painter = ui.painter();
    painter.rect(rect, theme::R_CONTROL, fill, ring, egui::StrokeKind::Inside);

    issue_words(painter, rect, tint, (place, aside), detail);
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// An issue row's dot in `tint`, its place and aside, and its detail at the right end.
fn issue_words(
    painter: &egui::Painter,
    rect: egui::Rect,
    tint: Color32,
    (place, aside): (&str, Option<&str>),
    detail: &str,
) {
    let colors = theme::colors();
    let dot = rect.left_center() + vec2(12.0, 0.0);
    painter.circle_filled(dot, 4.0, tint);
    let at = dot + vec2(11.0, 0.0);
    let named = painter.text(
        at,
        Align2::LEFT_CENTER,
        place,
        theme::label(),
        colors.text_high,
    );
    if let Some(aside) = aside {
        let at = named.right_center() + vec2(ASIDE_GAP, 0.0);
        painter.text(
            at,
            Align2::LEFT_CENTER,
            aside,
            theme::small(),
            colors.text_low,
        );
    }
    let at = rect.right_center() - vec2(12.0, 0.0);
    painter.text(
        at,
        Align2::RIGHT_CENTER,
        detail,
        theme::figures(12.0),
        colors.text_mid,
    );
}
