mod check;
mod combine;
mod cut;
pub(super) mod drain;
mod hollow;
mod layers;
mod measure;
mod paint;
mod position;
mod presets;
mod print_settings;
mod relief;
mod repair;
mod scope;
mod select;
mod shape;
mod support_picker;
mod supports;
mod text;

pub use check::tint as risk_tint;
pub(super) use position::auto_orient;

use egui::{Align, Layout, RichText, Sense, UiBuilder, vec2};

use crate::files::{self, Wanted};
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{
    body_and_foot, card_foot, empty_state, hairline, help_mark, icon, inline_button, mode_group,
    theme,
};
use crate::workspace::Tool;

/// The card beside the rail: the open tool, no taller than `max_h` points; its sections
/// scroll and its action stays pinned (`docs/decisions/0218`, `0221`).
pub fn ui(ui: &mut egui::Ui, window: &mut Window, max_h: f32) {
    let tool = window.tool.card();
    header(ui, window, tool);
    if window.doc.scene.is_empty() && tool != Tool::PrintSettings {
        empty_plate(ui, window);
        return;
    }
    if tool == Tool::Supports {
        supports::top(ui, window);
    }
    body_and_foot(
        ui,
        max_h,
        window,
        |ui, window| body(ui, window, tool),
        |ui, window| {
            if has_action(tool) {
                card_foot().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 6.0;
                    action(ui, window, tool);
                });
            }
        },
    );
}

/// Every tool with no model to work on: what is missing, and the way to put one there.
fn empty_plate(ui: &mut egui::Ui, window: &mut Window) {
    hairline(ui);
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            empty_state(
                ui,
                icon::EMPTY_PLATE,
                &tr!("stage-empty"),
                &tr!("stage-empty-how"),
            );
            let open = ui.vertical_centered(|ui| {
                inline_button(ui, icon::OPEN, &tr!("stage-open"), false, true).clicked()
            });
            if open.inner
                && let Some(file) = files::pick(Wanted::ModelOrProject)
            {
                crate::app::open_by_what_it_is(window, file);
            }
            ui.add_space(theme::ITEM_GAP);
        });
}

fn body(ui: &mut egui::Ui, window: &mut Window, tool: Tool) {
    match tool {
        Tool::Position => position::ui(ui, window),
        Tool::Measure => measure::ui(ui, window.tools),
        Tool::Repair => repair::ui(ui, window),
        Tool::Hollow => hollow::ui(ui, window),
        Tool::Drain => drain::ui(ui, window),
        Tool::Cut => cut::ui(ui, window),
        Tool::Relief => relief::ui(ui, window),
        Tool::Supports | Tool::Paint | Tool::Shape => supports::ui(ui, window),
        Tool::Check => check::ui(ui, window),
        Tool::PrintSettings => print_settings::ui(ui, window.machine),
    }
}

/// Whether the tool has one thing it is for, which stands at the foot rather than
/// scrolling away with its sections.
fn has_action(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Repair
            | Tool::Hollow
            | Tool::Drain
            | Tool::Cut
            | Tool::Relief
            | Tool::Supports
            | Tool::PrintSettings
    )
}

fn action(ui: &mut egui::Ui, window: &mut Window, tool: Tool) {
    match tool {
        Tool::Repair => repair::action(ui, window),
        Tool::Hollow => hollow::action(ui, window),
        Tool::Drain => drain::action(ui, window),
        Tool::Cut => cut::action(ui, window),
        Tool::Relief => relief::action(ui, window),
        Tool::Supports => supports::action(ui, window),
        Tool::PrintSettings => print_settings::action(ui, window.machine),
        _ => {}
    }
}

/// The one fact the open tool owns, in the heading's right end, and the colour it is
/// read in.
fn fact(window: &Window, tool: Tool) -> Option<(String, egui::Color32)> {
    let colors = theme::colors();
    let plain = |text: String| Some((text, colors.text_mid));
    match tool {
        Tool::Position => select::fact(window.doc).and_then(plain),
        Tool::Hollow => plain(tr!(
            "hollow-fact",
            value = format!("{:.2}", window.tools.hollow.state.thickness_mm)
        )),
        Tool::Drain => drain::fact(window),
        Tool::Check => check::fact(window),
        Tool::PrintSettings => plain(crate::panels::layers_and_exposure(window.machine)),
        Tool::Cut => Some((cut::fact(window), colors.text_mid)),
        // These cards have their modes where the fact would stand.
        Tool::Measure
        | Tool::Relief
        | Tool::Supports
        | Tool::Paint
        | Tool::Shape
        | Tool::Repair => None,
    }
}

/// What the tool is for, read from the question mark beside its name.
fn help(tool: Tool) -> String {
    match tool {
        Tool::Position => tr!("inspector-about-select"),
        Tool::Measure => tr!("inspector-about-measure"),
        Tool::Repair => tr!("inspector-about-repair"),
        Tool::Hollow => tr!("inspector-about-hollow"),
        Tool::Drain => tr!("inspector-about-drain"),
        Tool::Cut => tr!("inspector-about-cut"),
        Tool::Relief => tr!("inspector-about-relief"),
        Tool::Supports | Tool::Paint | Tool::Shape => tr!("inspector-about-supports"),
        Tool::Check => tr!("inspector-about-check"),
        Tool::PrintSettings => tr!("inspector-about-print-settings"),
    }
}

/// The tool's glyph, name and help, its fact at the right, and the fold beside it.
fn header(ui: &mut egui::Ui, window: &mut Window, tool: Tool) {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), theme::INSPECTOR_HEAD_H),
        Sense::hover(),
    );
    let inner = rect.shrink2(vec2(theme::PANEL_PAD, 0.0));
    let layout = Layout::left_to_right(Align::Center);
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner).layout(layout));
    child.spacing_mut().item_spacing.x = 8.0;
    child.label(
        RichText::new(tool.glyph())
            .font(theme::icon(16.0))
            .color(colors.accent_soft),
    );
    child.label(
        RichText::new(tool.label())
            .font(theme::tool_title())
            .color(colors.text_high),
    );
    let mut texts = vec![help(tool)];
    if tool == Tool::Relief {
        texts.push(relief::mode_note(window));
    }
    help_mark(&mut child, &texts);
    child.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let modes: Option<fn(&mut egui::Ui, &mut Window)> = match tool {
            Tool::Supports | Tool::Paint | Tool::Shape => Some(supports::modes),
            Tool::Repair => Some(repair::modes),
            Tool::Measure => Some(measure::modes),
            Tool::Relief => Some(relief::modes),
            _ => None,
        };
        if let Some(modes) = modes {
            mode_group(ui, |ui| modes(ui, window));
        } else if let Some((text, color)) = fact(window, tool) {
            ui.add(
                egui::Label::new(RichText::new(text).font(theme::figures(12.0)).color(color))
                    .truncate(),
            );
        }
    });
    hairline(ui);
}
