use super::PAD;
use crate::i18n::tr;
use crate::panels::{Window, printer_card};
use crate::profiles;
use crate::shortcuts::{self};
use crate::state::Machine;
use crate::ui::{focus_ring, icon, theme};
use crate::workspace::Tool;
use egui::{Align, Align2, Layout, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

/// The print chip in the middle of the window or as near as `room` allows, names cut short if
/// needed. Placed by last frame's width, redrawn when that changes.
pub(super) fn centred_chip(
    ui: &mut egui::Ui,
    window: &mut Window,
    bar: Rect,
    room: std::ops::RangeInclusive<f32>,
) {
    let id = ui.id().with("print-chip-w");
    let width = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or(0.0);
    let (least, most) = (*room.start(), *room.end());
    let max_w = (most - least).max(0.0);
    let x = (bar.center().x - width / 2.0).min(most - width).max(least);
    let rect = Rect::from_x_y_ranges(x..=x + max_w, bar.y_range());
    let drawn = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                let ring_w = printer_card::ring::ui(ui, window);
                print_chip(ui, window, max_w - ring_w);
            },
        )
        .response
        .rect
        .width();
    if (drawn - width).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, drawn));
        ui.ctx().request_discard("the print chip changed width");
    }
}

/// Width of the slash between the machine and the resin, points.
const SLASH_W: f32 = 10.0;

/// The machine, resin, layers and exposure of the plate, no wider than `max_w`: names are cut
/// short, and the chip is left out below `NAME_LEAST_W` of each.
pub(super) fn print_chip(ui: &mut egui::Ui, window: &mut Window, max_w: f32) {
    let slicing = &window.machine.slicing;
    let untuned = slicing.printer_id.is_some() && slicing.has_resin() && !slicing.resin_is_tuned();
    let Some(parts) = chip_parts(ui, window, max_w, untuned) else {
        return;
    };
    let width: f32 = parts.iter().map(|part| part.width).sum::<f32>() + SLASH_W;
    let (rect, _) = ui.allocate_exact_size(vec2(width, theme::CHIP_H), Sense::hover());
    let mut x = rect.left();
    let zones = std::array::from_fn(|index| {
        let zone = Rect::from_x_y_ranges(x..=x + parts[index].width, rect.y_range());
        x += parts[index].width + if index == 0 { SLASH_W } else { 0.0 };
        zone
    });
    let names = [
        tr!("chip-machine", name = window.doc.plate.display_name()),
        tr!("chip-resin", name = window.machine.slicing.resin_name()),
        format!(
            "{}, {}",
            Tool::PrintSettings.label(),
            layers_and_exposure(window.machine)
        ),
    ];
    let responses = std::array::from_fn(|index: usize| {
        let response = ui.interact(
            zones[index],
            ui.id().with(("print-chip", index)),
            Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &names[index])
        });
        response
    });
    paint_chip(
        ui,
        parts,
        zones,
        rect,
        &responses,
        *window.tool == Tool::PrintSettings,
    );
    answer_chip(window, responses, untuned);
}

/// The chip's three parts sized to `max_w`, or `None` where the names have no room.
fn chip_parts(ui: &egui::Ui, window: &Window, max_w: f32, untuned: bool) -> Option<[Part; 3]> {
    let colors = theme::colors();
    let slicing = &window.machine.slicing;
    let layers = layers_and_exposure(window.machine);
    let settings = Part::new(
        ui,
        icon::PARAMETERS,
        &layers,
        colors.text_mid,
        "",
        f32::INFINITY,
    );
    let (printer, resin) = (window.doc.plate.display_name(), slicing.resin_name());
    let names_room =
        max_w - settings.width - SLASH_W - Part::chrome_w("") - Part::chrome_w(icon::CARET_DOWN);
    if names_room < 2.0 * NAME_LEAST_W {
        return None;
    }
    let (printer_w, resin_w) = share_room(names_room, text_w(ui, &printer), text_w(ui, &resin));
    let resin_tint = if untuned {
        colors.warn
    } else {
        colors.text_mid
    };
    Some([
        Part::new(ui, icon::PRINTER, &printer, colors.text_high, "", printer_w),
        Part::new(
            ui,
            icon::RESIN,
            &resin,
            resin_tint,
            icon::CARET_DOWN,
            resin_w,
        ),
        settings,
    ])
}

/// Paints the three parts in `zones` of the chip's `rect`, the settings lit while `open`.
fn paint_chip(
    ui: &egui::Ui,
    parts: [Part; 3],
    zones: [Rect; 3],
    rect: Rect,
    responses: &[egui::Response; 3],
    open: bool,
) {
    let colors = theme::colors();
    let painter = ui.painter();
    for (index, (part, zone)) in parts.into_iter().zip(zones).enumerate() {
        // Square where a part meets the rule between the resin and the settings.
        let mut radius = theme::R_CONTROL;
        match index {
            1 => (radius.ne, radius.se) = (0, 0),
            2 => (radius.nw, radius.sw) = (0, 0),
            _ => {}
        }
        part.paint(
            painter,
            zone,
            radius,
            index == 2 && open,
            responses[index].hovered(),
        );
    }
    let slash_at = pos2(zones[0].right() + SLASH_W / 2.0, rect.center().y);
    painter.text(
        slash_at,
        Align2::CENTER_CENTER,
        "/",
        theme::label(),
        colors.text_low,
    );
    painter.vline(
        zones[2].left(),
        rect.y_range(),
        Stroke::new(1.0, colors.hairline),
    );
    let stroke = Stroke::new(1.0, colors.hairline);
    painter.rect_stroke(rect, theme::R_CONTROL, stroke, egui::StrokeKind::Inside);
    for response in responses {
        focus_ring(ui, response, theme::R_CONTROL);
    }
}

/// Opens the machine or resin menu, or the print settings, for the part clicked.
fn answer_chip(window: &mut Window, responses: [egui::Response; 3], untuned: bool) {
    let [printer, resin, layers] = responses;
    let printer = printer.on_hover_text(tr!("chip-machine-tooltip"));
    egui::Popup::menu(&printer).show(|ui| profiles::printer_menu(ui, window));
    let resin = if untuned {
        resin.on_hover_text(tr!("chip-resin-untuned"))
    } else {
        resin.on_hover_text(tr!("chip-resin-tooltip"))
    };
    egui::Popup::menu(&resin).show(|ui| profiles::resin_menu(ui, window.machine));
    if layers.on_hover_text(Tool::PrintSettings.label()).clicked() {
        shortcuts::pick(window, Tool::PrintSettings);
    }
}

/// The layer height and the exposure a plate is cut with, as the chip and the print
/// settings' heading both read them.
pub fn layers_and_exposure(machine: &Machine) -> String {
    tr!(
        "chip-layers",
        layer = format!("{:.0}", machine.slicing.layer_height_mm() * 1000.0),
        exposure = format!("{:.2}", machine.slicing.material.exposure_s)
    )
}

/// One stretch of the print chip: a glyph, what is chosen, and a caret where it opens a
/// list.
pub(super) struct Part {
    pub(super) glyph: &'static str,
    pub(super) text: std::sync::Arc<egui::Galley>,
    pub(super) caret: &'static str,
    pub(super) width: f32,
}

impl Part {
    /// A part whose text is cut short with an ellipsis past `max_text_w` points.
    pub(super) fn new(
        ui: &egui::Ui,
        glyph: &'static str,
        text: &str,
        tint: egui::Color32,
        caret: &'static str,
        max_text_w: f32,
    ) -> Self {
        let whole = ui
            .painter()
            .layout_no_wrap(text.to_owned(), theme::label(), tint);
        // Laid out again only when cut: a truncating layout sets even a whole text apart.
        let text = if whole.size().x <= max_text_w {
            whole
        } else {
            let mut job =
                egui::text::LayoutJob::simple_singleline(text.to_owned(), theme::label(), tint);
            job.wrap = egui::text::TextWrapping::truncate_at_width(max_text_w.max(0.0));
            ui.painter().layout_job(job)
        };
        let width = Self::chrome_w(caret) + text.size().x;
        Self {
            glyph,
            text,
            caret,
            width,
        }
    }

    /// The part's width less its text: the room either side, the glyph and the caret.
    pub(super) fn chrome_w(caret: &str) -> f32 {
        let caret_w = if caret.is_empty() { 0.0 } else { 16.0 };
        PAD + 20.0 + caret_w + PAD
    }

    pub(super) fn paint(
        self,
        painter: &egui::Painter,
        zone: Rect,
        radius: egui::CornerRadius,
        lit: bool,
        hovered: bool,
    ) {
        let colors = theme::colors();
        if lit {
            painter.rect_filled(zone, radius, colors.raised);
        } else if hovered {
            painter.rect_filled(zone, radius, colors.hover);
        }
        let glyph_tint = if lit {
            colors.accent_soft
        } else {
            colors.text_low
        };
        painter.text(
            pos2(zone.left() + PAD + 7.0, zone.center().y),
            Align2::CENTER_CENTER,
            self.glyph,
            theme::icon(14.0),
            glyph_tint,
        );
        let at = pos2(
            zone.left() + PAD + 20.0,
            zone.center().y - self.text.size().y / 2.0,
        );
        let text_w = self.text.size().x;
        let fallback = if lit {
            colors.accent_soft
        } else {
            colors.text_high
        };
        painter.galley(at, self.text, fallback);
        if !self.caret.is_empty() {
            painter.text(
                pos2(at.x + text_w + 9.0, zone.center().y),
                Align2::CENTER_CENTER,
                self.caret,
                theme::icon(11.0),
                colors.text_low,
            );
        }
    }
}

/// The least of a name the chip shows: enough for a letter and the ellipsis.
pub(super) const NAME_LEAST_W: f32 = 24.0;

/// The width `text` takes in the chip, uncut.
pub(super) fn text_w(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), theme::label(), egui::Color32::WHITE)
        .size()
        .x
}

/// Two names' widths cut down to share `room`: the shorter keeps all it needs when it fits
/// in half, the longer takes the rest.
pub(super) fn share_room(room: f32, first_w: f32, second_w: f32) -> (f32, f32) {
    let room = room.max(0.0);
    let half = room / 2.0;
    if first_w + second_w <= room {
        (first_w, second_w)
    } else if first_w <= half {
        (first_w, room - first_w)
    } else if second_w <= half {
        (room - second_w, second_w)
    } else {
        (half, half)
    }
}
