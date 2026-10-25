use super::toggle_settings;
use crate::i18n::tr;
use crate::settings::Section;
use crate::state::Machine;
use crate::ui::{focus_ring, icon, theme};
use crate::updates::Stage;
use egui::vec2;

/// A release newer than this build, offered and never applied: a click opens the Updates
/// page, or restarts into a build already installed. See `crate::updates`.
pub(super) fn update_badge(ui: &mut egui::Ui, machine: &mut Machine, compact: bool) {
    let Some(offer) = machine.updates.waiting() else {
        return;
    };
    let installed = matches!(machine.updates.stage, Stage::Installed(_));
    let text = if installed {
        tr!("bar-restart-to-update")
    } else {
        format!("Encrust {}", offer.version)
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
            if !machine.settings.open {
                toggle_settings(machine);
            }
            machine.settings.turn_to(Section::Updates);
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

/// A rounded tag in the bar, the one shape both badges are drawn as, its glyph alone when
/// `compact`.
pub(super) fn pill(
    ui: &mut egui::Ui,
    glyph: &str,
    text: &str,
    compact: bool,
    tint: egui::Color32,
    wash: egui::Color32,
) -> egui::Response {
    let shown = if compact { "" } else { text };
    let label = ui
        .painter()
        .layout_no_wrap(shown.to_owned(), theme::small(), tint);
    let size = vec2(label.size().x + 34.0, theme::CHIP_H - 6.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = if response.hovered() {
        wash.gamma_multiply(1.6)
    } else {
        wash
    };
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(255), fill);
    painter.text(
        rect.left_center() + vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        glyph,
        theme::icon(13.0),
        tint,
    );
    painter.galley(
        rect.left_center() + vec2(26.0, -label.size().y / 2.0),
        label,
        tint,
    );
    focus_ring(ui, &response, egui::CornerRadius::same(255));
    response
}
