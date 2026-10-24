use super::playback::{fraction, height_range};
use super::{LAYER_FIELD_MIN_W, READOUT_H, floor_height, layer_at};
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{focus_ring, theme};
use crate::workspace::Mode;
use core_geometry::Scalar;
use egui::{Align, Align2, Layout, Rect, Sense, pos2, vec2};

/// Where the cut stands on the strip, and what it counts in: layers of the stack in the
/// Preview mode, layer heights of the model in the Prepare mode.
pub(super) struct Reading {
    /// Share of the way up, from zero at the plate to one at the top, or `None` with
    /// nothing to scrub.
    pub(super) at: Option<f32>,
    /// Share of the way up the floor stands and the first layer it leaves, counted from
    /// one, or `None` while nothing is cut away from below.
    pub(super) floor: Option<(f32, usize)>,
    pub(super) layer: usize,
    pub(super) layers: usize,
    pub(super) height_mm: Scalar,
    pub(super) top_mm: Scalar,
}

impl Reading {
    pub(super) fn of(window: &Window) -> Self {
        match *window.mode {
            Mode::Preview => {
                let preview = &window.machine.preview;
                let count = preview.layer_count();
                let last = count.saturating_sub(1);
                Self {
                    at: (count > 0)
                        .then(|| fraction(0.0, last as Scalar, preview.layer() as Scalar)),
                    floor: floor_height(Mode::Preview, &window.view.section, preview).map(
                        |floor| {
                            let first =
                                (preview.layer_topping_near(floor) + 1).min(preview.layer());
                            (fraction(0.0, last as Scalar, first as Scalar), first + 1)
                        },
                    ),
                    layer: if count > 0 { preview.layer() + 1 } else { 0 },
                    layers: count,
                    height_mm: preview.layer_z().unwrap_or_default(),
                    top_mm: preview.stack_top_mm().unwrap_or_default(),
                }
            }
            Mode::Prepare => {
                let Some((bottom, top)) = height_range(window.doc) else {
                    return Self::nothing();
                };
                let height = window.view.section.height_mm.unwrap_or(top);
                let (layer, layers) = layer_at(
                    height,
                    (bottom, top),
                    window.machine.slicing.layer_height_mm(),
                );
                let floor =
                    floor_height(Mode::Prepare, &window.view.section, &window.machine.preview).map(
                        |floor| {
                            let (under, _) = layer_at(
                                floor,
                                (bottom, top),
                                window.machine.slicing.layer_height_mm(),
                            );
                            (fraction(bottom, top, floor), under + 1)
                        },
                    );
                Self {
                    at: Some(fraction(bottom, top, height)),
                    floor,
                    layer,
                    layers,
                    height_mm: height,
                    top_mm: top,
                }
            }
        }
    }

    pub(super) fn nothing() -> Self {
        Self {
            at: None,
            floor: None,
            layer: 0,
            layers: 0,
            height_mm: 0.0,
            top_mm: 0.0,
        }
    }

    /// The layer the cut stands at, over how many there are and its height, centred.
    /// A click on the layer opens it for typing; answers with the layer typed, on Enter.
    pub(super) fn show(&self, ui: &mut egui::Ui) -> Option<usize> {
        let colors = theme::colors();
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, READOUT_H), Sense::hover());
        if self.at.is_none() {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                tr!("section-no-layers"),
                theme::small(),
                colors.text_low,
            );
            response.on_hover_text(tr!("section-no-layers-yet"));
            return None;
        }
        let painter = ui.painter();
        let row_h = painter
            .layout_no_wrap("0".to_owned(), theme::figures(12.5), colors.text_low)
            .size()
            .y;
        painter.text(
            rect.center_top() + vec2(0.0, row_h),
            Align2::CENTER_TOP,
            format!("/ {}", self.layers),
            theme::figures(11.0),
            colors.text_low,
        );
        painter.text(
            rect.center_bottom(),
            Align2::CENTER_BOTTOM,
            mm(self.height_mm),
            theme::figures(10.0),
            colors.text_low,
        );
        response.on_hover_text(tr!(
            "section-height-of",
            height = format!("{:.2}", self.height_mm),
            top = format!("{:.2}", self.top_mm)
        ));
        let layer_rect = Rect::from_min_size(rect.min, vec2(width, row_h));
        self.layer_field(ui, layer_rect)
    }

    /// The layer's own figure, or the box it is typed into once clicked.
    pub(super) fn layer_field(&self, ui: &mut egui::Ui, rect: Rect) -> Option<usize> {
        let colors = theme::colors();
        let id = egui::Id::new("layer-readout");
        let typing: Option<String> = ui.data(|data| data.get_temp(id));
        let Some(mut text) = typing else {
            let figure = ui.painter().layout_no_wrap(
                self.layer.to_string(),
                theme::figures(12.5),
                colors.accent_soft,
            );
            let at = pos2(rect.center().x - figure.size().x / 2.0, rect.top());
            let spot = Rect::from_min_size(at, figure.size());
            let response = ui
                .interact(spot, id.with("figure"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::Text)
                .on_hover_text(tr!("section-type-layer"));
            response.widget_info(|| {
                let name = tr!("section-layer-field", layer = self.layer);
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name)
            });
            focus_ring(ui, &response, theme::R_CONTROL);
            ui.painter().galley(at, figure, colors.accent_soft);
            if response.clicked() {
                ui.data_mut(|data| data.insert_temp(id, self.layer.to_string()));
                ui.memory_mut(|memory| memory.request_focus(id.with("field")));
            }
            return None;
        };
        // The box is the figure itself: no frame, no margin, as wide as what is typed and
        // centred where it stood, so opening it moves nothing on the strip.
        let typed_w = ui
            .painter()
            .layout_no_wrap(text.clone(), theme::figures(12.5), colors.accent_soft)
            .size()
            .x;
        let field = Rect::from_center_size(
            rect.center(),
            vec2(typed_w.max(LAYER_FIELD_MIN_W), rect.height()),
        );
        // A child, not `put`: allocating the box would pull the strip's cursor back up
        // to it, and the track under the readout with it.
        let layout = Layout::centered_and_justified(egui::Direction::LeftToRight);
        let response = ui
            .new_child(egui::UiBuilder::new().max_rect(field).layout(layout))
            .add(
                egui::TextEdit::singleline(&mut text)
                    .id(id.with("field"))
                    .font(theme::figures(12.5))
                    .text_color(colors.accent_soft)
                    .horizontal_align(Align::Center)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .desired_width(field.width()),
            );
        if !response.lost_focus() {
            ui.data_mut(|data| data.insert_temp(id, text));
            return None;
        }
        ui.data_mut(|data| data.remove::<String>(id));
        let entered = ui.input(|input| input.key_pressed(egui::Key::Enter));
        entered.then(|| text.trim().parse().ok()).flatten()
    }
}

/// A height in the strip's narrow column: two decimals, one from a hundred millimetres up.
pub(super) fn mm(height_mm: Scalar) -> String {
    let value = if height_mm < 100.0 {
        format!("{height_mm:.2}")
    } else {
        format!("{height_mm:.1}")
    };
    tr!("value-mm", value = value)
}
