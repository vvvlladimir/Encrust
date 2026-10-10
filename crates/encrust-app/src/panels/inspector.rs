mod check;
mod cut;
mod drain;
mod export;
mod hollow;
mod layers;
mod measure;
mod opened;
mod paint;
mod position;
mod print_settings;
mod relief;
mod select;
mod shape;
mod supports;

pub use check::tint as risk_tint;

use egui::{Align, Layout, RichText, Sense, UiBuilder, vec2};

use crate::panels::Window;
use crate::ui::{body_and_foot, card_foot, hairline, icon, icon_button, later, theme};
use crate::workspace::Tool;

/// The card beside the rail: the open tool, whatever the stage shows, no taller than
/// `max_h` points. A heading names it, its sections scroll once they outgrow the card,
/// and its action stays pinned to the foot; see `docs/decisions/0218`, `0221`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window, max_h: f32) {
    let tool = *window.tool;
    header(ui, window, tool);
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

fn body(ui: &mut egui::Ui, window: &mut Window, tool: Tool) {
    match tool {
        Tool::Select => select::ui(ui, window),
        Tool::Position => position::ui(ui, window),
        Tool::Measure => measure::ui(ui, window.tools),
        Tool::Hollow => hollow::ui(ui, window),
        Tool::Drain => drain::ui(ui, window),
        Tool::Cut => cut::ui(ui, window),
        Tool::Relief => relief::ui(ui, window),
        Tool::Supports => supports::ui(ui, window),
        Tool::Paint => paint::ui(ui, window),
        Tool::Shape => shape::ui(ui, window),
        Tool::Check => check::ui(ui, window),
        Tool::Export => export::ui(ui, window),
        Tool::PrintSettings => print_settings::ui(ui, window.machine),
    }
}

/// Whether the tool has one thing it is for, which stands at the foot rather than
/// scrolling away with its sections.
fn has_action(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Hollow
            | Tool::Drain
            | Tool::Cut
            | Tool::Relief
            | Tool::Supports
            | Tool::Paint
            | Tool::Export
    )
}

fn action(ui: &mut egui::Ui, window: &mut Window, tool: Tool) {
    match tool {
        Tool::Hollow => hollow::action(ui, window),
        Tool::Drain => drain::action(ui, window),
        Tool::Cut => cut::action(ui, window),
        Tool::Relief => relief::action(ui, window),
        Tool::Supports => supports::action(ui, window),
        Tool::Paint => paint::action(ui, window),
        Tool::Export => export::action(ui, window),
        _ => {}
    }
}

/// The one fact the open tool owns, in the heading's right end, and the colour it is
/// read in.
fn fact(window: &Window, tool: Tool) -> Option<(String, egui::Color32)> {
    let colors = theme::colors();
    let plain = |text: String| Some((text, colors.text_mid));
    match tool {
        Tool::Select | Tool::Position => select::fact(window.doc).and_then(plain),
        Tool::Hollow => plain(format!("{:.2} mm", window.tools.hollow.state.thickness_mm)),
        Tool::Drain => drain::fact(window).map(|text| (text, colors.danger)),
        Tool::Supports => supports::fact(window).and_then(plain),
        Tool::Paint => window
            .tools
            .supports
            .groups
            .get(usize::from(window.tools.supports.active))
            .and_then(|group| plain(group.name.clone())),
        Tool::Shape => plain(window.tools.supports.profile.name.clone()),
        Tool::Check => check::fact(window),
        Tool::Export => export::fact(window),
        Tool::PrintSettings => plain(crate::panels::layers_and_exposure(window.machine)),
        Tool::Measure | Tool::Cut | Tool::Relief => None,
    }
}

/// What the tool is for, read from the question mark beside its name.
fn help(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "Pick what the other tools work on.",
        Tool::Position => "Move, turn, size, lay down and mirror the selection.",
        Tool::Measure => "Click two points on a model.",
        Tool::Hollow => "Cut a cavity to save resin and lighten the peel.",
        Tool::Drain => "Let resin out of a hollow model.",
        Tool::Cut => "Split a model along a plane.",
        Tool::Relief => "Press a model's texture into its surface. White is the high point.",
        Tool::Supports => "Hold up what hangs over nothing.",
        Tool::Paint => "Tell the supports where to go and where not to.",
        Tool::Shape => "The shape of one support, part by part.",
        Tool::Check => "What will fail on the machine, with the layer it fails on.",
        Tool::Export => "From the plate to a file, and from the file to a printer.",
        Tool::PrintSettings => "How this plate is cut and exposed.",
    }
}

/// The tool's glyph, name and help, its fact at the right, and the fold beside it.
fn header(ui: &mut egui::Ui, window: &Window, tool: Tool) {
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
            .color(colors.text_low),
    );
    child.label(
        RichText::new(tool.label())
            .font(theme::tool_title())
            .color(colors.text_high),
    );
    child
        .label(
            RichText::new(icon::QUESTION)
                .font(theme::icon(13.0))
                .color(colors.text_low),
        )
        .on_hover_text(help(tool));
    child.with_layout(Layout::right_to_left(Align::Center), |ui| {
        // TODO(step-8): fold the inspector away, and a second press on the open tool too.
        later(ui, |ui| icon_button(ui, icon::FOLD, "Fold the inspector"));
        if let Some((text, color)) = fact(window, tool) {
            ui.add(
                egui::Label::new(RichText::new(text).font(theme::figures(11.5)).color(color))
                    .truncate(),
            );
        }
    });
    hairline(ui);
}
