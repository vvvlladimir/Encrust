//! The layer strip's readout: the layer in a sunken box to type into, how many there are,
//! and the height; see `docs/design/ui-design-system.md`, Layer readout.

use egui::{Align, Align2, Layout, Rect, Sense, pos2, vec2};

use super::focus_ring;
use crate::ui::theme;

/// What the readout says around its figure, in the window's language.
pub struct ReadoutWords {
    /// How many layers there are: *of 400*.
    pub of: String,
    pub height: String,
    pub hover: String,
    pub type_hint: String,
    /// The box's name, read out.
    pub field_name: String,
    /// What stands in place of the readout with no layers, and its hover.
    pub none: String,
    pub none_hover: String,
}

/// The readout's height; the box the layer stands in, and the least room the typed layer
/// is given, so an emptied box still takes a click.
const READOUT_H: f32 = 62.0;
const LAYER_BOX_H: f32 = 26.0;
const LAYER_FIELD_MIN_W: f32 = 8.0;

/// The layer the cut stands at in a sunken box, how many there are and its height under
/// it, centred; `None` says there are no layers. A click on the layer opens it for
/// typing; answers with the layer typed, on Enter.
pub fn layer_readout(
    ui: &mut egui::Ui,
    layer: Option<usize>,
    words: &ReadoutWords,
) -> Option<usize> {
    let colors = theme::colors();
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, READOUT_H), Sense::hover());
    let Some(layer) = layer else {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            &words.none,
            theme::small(),
            colors.text_low,
        );
        response.on_hover_text(&words.none_hover);
        return None;
    };
    let figure_w = ui
        .painter()
        .layout_no_wrap(layer.to_string(), figure_font(), colors.accent_soft)
        .size()
        .x;
    let box_w = (figure_w + 2.0 * LAYER_BOX_PAD).clamp(LAYER_BOX_MIN_W, width);
    let layer_box = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + LAYER_BOX_H / 2.0),
        vec2(box_w, LAYER_BOX_H),
    );
    let painter = ui.painter();
    painter.text(
        layer_box.center_bottom() + vec2(0.0, 4.0),
        Align2::CENTER_TOP,
        &words.of,
        theme::figures(10.5),
        colors.text_low,
    );
    painter.text(
        rect.center_bottom(),
        Align2::CENTER_BOTTOM,
        &words.height,
        theme::strip_height(),
        colors.text_mid,
    );
    response.on_hover_text(&words.hover);
    layer_field(ui, layer, layer_box, words)
}

/// The layer's own figure in its box, or the box it is typed into once clicked.
fn layer_field(ui: &mut egui::Ui, layer: usize, rect: Rect, words: &ReadoutWords) -> Option<usize> {
    let colors = theme::colors();
    let id = egui::Id::new("layer-readout");
    let typing: Option<String> = ui.data(|data| data.get_temp(id));
    let Some(mut text) = typing else {
        let response = ui
            .interact(rect, id.with("figure"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::Text)
            .on_hover_text(&words.type_hint);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &words.field_name)
        });
        paint_box(ui, rect, response.hovered());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            layer.to_string(),
            figure_font(),
            colors.accent_soft,
        );
        focus_ring(ui, &response, theme::R_CONTROL);
        if response.clicked() {
            ui.data_mut(|data| data.insert_temp(id, layer.to_string()));
            ui.memory_mut(|memory| memory.request_focus(id.with("field")));
        }
        return None;
    };
    paint_box(ui, rect, true);
    // The text is the figure itself, no frame and no margin, as wide as what is typed
    // and centred where it stood, so opening it moves nothing on the strip.
    let typed = ui
        .painter()
        .layout_no_wrap(text.clone(), figure_font(), colors.accent_soft)
        .size();
    let field =
        Rect::from_center_size(rect.center(), vec2(typed.x.max(LAYER_FIELD_MIN_W), typed.y));
    // A child, not `put`: allocating the box would pull the strip's cursor back up
    // to it, and the track under the readout with it.
    let layout = Layout::centered_and_justified(egui::Direction::LeftToRight);
    let response = ui
        .new_child(egui::UiBuilder::new().max_rect(field).layout(layout))
        .add(
            egui::TextEdit::singleline(&mut text)
                .id(id.with("field"))
                .font(figure_font())
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

/// The sunken box the layer stands in, ringed in `line` under the pointer or while typed in.
fn paint_box(ui: &egui::Ui, rect: Rect, lit: bool) {
    let colors = theme::colors();
    let ring = if lit { colors.line } else { colors.hairline };
    ui.painter().rect(
        rect,
        theme::R_CONTROL,
        colors.sunken,
        egui::Stroke::new(1.0, ring),
        egui::StrokeKind::Inside,
    );
}

fn figure_font() -> egui::FontId {
    theme::strip_layer()
}

/// Room either side of the layer in its box, and the least the box is wide, points.
const LAYER_BOX_PAD: f32 = 8.0;
const LAYER_BOX_MIN_W: f32 = 40.0;
