use egui::{Align2, Frame, RichText, Ui, vec2};

use crate::i18n::tr;
use crate::ui::theme;
use crate::ui::theme::motion;

/// The title row of a window over the plate: its glyph, its name, and the way out.
/// Answers whether the way out was taken.
pub fn dialog_head(ui: &mut Ui, glyph: &str, title: &str) -> bool {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(glyph)
                .font(theme::icon(15.0))
                .color(colors.text_low),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(title)
                .font(theme::dialog_title())
                .color(colors.text_high),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            crate::ui::widgets::icon_button(ui, crate::ui::icon::CANCEL, &tr!("dialog-close"))
                .clicked()
        })
        .inner
    })
    .inner
}

/// A window over the plate: on the panel's fill, ringed in `line`, `margin` inside it, over
/// the stage dimmed by the scrim; both come in by `OPEN`, the window rising as it does.
pub fn dialog(ctx: &egui::Context, id: egui::Id, margin: i8) -> egui::Modal {
    placed(
        ctx,
        id,
        margin,
        (Align2::CENTER_CENTER, 0.0),
        theme::SCRIM_SHARE,
    )
}

/// The same hanging `top` points under the window's top over a lighter scrim, as Find does.
pub fn palette_dialog(ctx: &egui::Context, id: egui::Id, top: f32) -> egui::Modal {
    placed(
        ctx,
        id,
        0,
        (Align2::CENTER_TOP, top),
        theme::FIND_SCRIM_SHARE,
    )
}

fn placed(
    ctx: &egui::Context,
    id: egui::Id,
    margin: i8,
    (anchor, top): (Align2, f32),
    scrim_share: f32,
) -> egui::Modal {
    let came_in = motion::appear(ctx, id.with("dialog"));
    let lift = vec2(0.0, top + motion::LIFT_PT * (1.0 - came_in));
    egui::Modal::new(id)
        .area(egui::Modal::default_area(id).anchor(anchor, lift))
        .frame(dialog_frame().inner_margin(egui::Margin::same(margin)))
        .backdrop_color(theme::scrim(scrim_share * came_in))
}

fn dialog_frame() -> Frame {
    Frame::new()
        .fill(theme::colors().panel)
        .stroke(egui::Stroke::new(1.0, theme::colors().line))
        .corner_radius(theme::R_PANEL)
        .shadow(theme::window_shadow())
}
