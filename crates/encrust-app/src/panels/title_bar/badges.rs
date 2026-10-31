use crate::i18n::tr;
use crate::state::Machine;
use crate::ui::{focus_ring, icon, theme};
use crate::updates::Stage;
use egui::vec2;

/// A badge's padding either side of its words, its height, and how strongly its ring
/// carries the tint.
const PILL_PAD: f32 = 8.0;
const PILL_H: f32 = 22.0;
const PILL_RING: f32 = 0.35;

/// A release newer than this build, offered and never applied: a click opens the update
/// sheet, or restarts into a build already installed. See `crate::updates`.
pub(super) fn update_badge(ui: &mut egui::Ui, machine: &mut Machine, compact: bool) {
    let Some(offer) = machine.updates.waiting() else {
        return;
    };
    let installed = matches!(machine.updates.stage, Stage::Installed(_));
    let downloading = machine.updates.progress().and_then(|(progress, _)| {
        let (received, total) = progress.bytes();
        total.map(|total| received * 100 / total.max(1))
    });
    let text = match (installed, downloading) {
        (true, _) => tr!("bar-restart-to-update"),
        (false, Some(percent)) => tr!("bar-update-percent", percent = percent),
        (false, None) => tr!("app-version", version = offer.version.to_string()),
    };
    let colors = theme::colors();
    let response = pill(
        ui,
        icon::UPDATE,
        &text,
        compact,
        colors.accent,
        colors.accent_wash,
    );
    let tooltip = if installed {
        tr!("bar-restart-tooltip")
    } else {
        tr!("bar-update-tooltip")
    };
    if response.on_hover_text(tooltip).clicked() {
        if installed {
            machine.updates.restart();
        } else {
            machine.updates.sheet = true;
        }
    }
    ui.add_space(6.0);
}

/// What this build is, where it can be acted on: a click opens the sheet that writes a
/// bug report. See `crate::report`.
pub(super) fn alpha_badge(ui: &mut egui::Ui, machine: &mut Machine, compact: bool) {
    let colors = theme::colors();
    let response = pill(
        ui,
        icon::BUG,
        &tr!("bar-alpha"),
        compact,
        colors.warn,
        colors.warn_wash,
    );
    if response.on_hover_text(tr!("bar-alpha-tooltip")).clicked() {
        machine.report.open = true;
    }
    ui.add_space(6.0);
}

/// A rounded tag in the bar ringed in its tint, the one shape both badges are drawn as;
/// its glyph alone when `compact`, its words alone otherwise.
pub(super) fn pill(
    ui: &mut egui::Ui,
    glyph: &str,
    text: &str,
    compact: bool,
    tint: egui::Color32,
    wash: egui::Color32,
) -> egui::Response {
    let (shown, font) = if compact {
        (glyph, theme::icon(13.0))
    } else {
        (text, theme::small())
    };
    let label = ui.painter().layout_no_wrap(shown.to_owned(), font, tint);
    let size = vec2(label.size().x + PILL_PAD * 2.0, PILL_H);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = if response.hovered() {
        wash
    } else {
        egui::Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        egui::CornerRadius::same(255),
        fill,
        egui::Stroke::new(1.0, tint.gamma_multiply(PILL_RING)),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.center() - label.size() / 2.0, label, tint);
    focus_ring(ui, &response, egui::CornerRadius::same(255));
    response
}
