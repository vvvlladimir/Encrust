//! How to work the viewport with the tool in hand, centred under the tool row; drawn only
//! for a tool the pointer does something with there. See `docs/design/ui-screens.md`.

use egui::{Align2, Id, Modifiers, Pos2};

use crate::drain::Placing as Drain;
use crate::i18n::tr;
use crate::measure::{Snap, Target};
use crate::panels::Window;
use crate::relief::ReliefMode;
use crate::supports::Placing as Supports;
use crate::ui::hint_bar;
use crate::workspace::Tool;

/// The bar, its top centre at `top`, when the tool in hand has one.
pub fn ui(ui: &egui::Ui, window: &Window, top: Pos2) {
    let alt = ui.ctx().format_modifiers(Modifiers::ALT);
    let command = ui.ctx().format_modifiers(Modifiers::COMMAND);
    let Some(pairs) = pairs(window, &Keys { alt, command }) else {
        return;
    };
    let pairs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(key, what)| (key.as_str(), what.as_str()))
        .collect();
    egui::Area::new(Id::new("hint-bar"))
        .order(egui::Order::Middle)
        .fixed_pos(top)
        .pivot(Align2::CENTER_TOP)
        .interactable(false)
        .show(ui.ctx(), |ui| hint_bar(ui, &pairs));
}

/// What the modifier keys are called on this keyboard.
struct Keys {
    alt: String,
    command: String,
}

/// The key and what it does, pair by pair, for the tool in hand; `None` for a tool the
/// pointer does nothing with in the viewport.
fn pairs(window: &Window, keys: &Keys) -> Option<Vec<(String, String)>> {
    let click = || tr!("hint-click");
    let alt_click = || tr!("hint-modifier-click", key = keys.alt.clone());
    let tools = &window.tools;
    let pairs = match *window.tool {
        Tool::Measure => {
            let first = match (tools.measure.target, tools.measure.start().is_some()) {
                (Target::Points, false) => tr!("hint-measure-two-points"),
                (Target::Points, true) => tr!("hint-measure-second-point"),
                (Target::Planes, false) => tr!("hint-measure-two-faces"),
                (Target::Planes, true) => tr!("hint-measure-second-face"),
            };
            let mut pairs = vec![(click(), first)];
            if tools.measure.target == Target::Points && tools.measure.snap != Snap::Free {
                pairs.push((keys.alt.clone(), tr!("hint-measure-freely")));
            }
            pairs.push((tr!("hint-esc"), tr!("hint-clear")));
            pairs
        }
        Tool::Drain => {
            let first = match tools.drain.placing {
                Drain::Hole => tr!("hint-drain-hole"),
                Drain::Channel => tr!("hint-drain-channel"),
            };
            vec![(click(), first), (alt_click(), tr!("hint-remove"))]
        }
        Tool::Relief => match tools.relief.mode {
            ReliefMode::Text => vec![
                (click(), tr!("hint-relief-write")),
                (
                    tr!("hint-modifier-click", key = keys.command.clone()),
                    tr!("hint-pick-without-writing"),
                ),
            ],
            ReliefMode::Texture if tools.relief.paints() => vec![
                (tr!("hint-drag"), tr!("hint-paint")),
                (keys.alt.clone(), tr!("hint-erase")),
            ],
            _ => return None,
        },
        Tool::Supports => vec![
            (click(), tr!("hint-supports-pick")),
            (tr!("hint-drag"), tr!("hint-supports-carry")),
            (tr!("hint-shift"), tr!("hint-add")),
            (keys.alt.clone(), tr!("hint-remove")),
        ],
        Tool::Paint if tools.supports.placing == Supports::Point => vec![
            (click(), tr!("hint-supports-place")),
            (tr!("hint-shift-click"), tr!("hint-supports-line")),
            (alt_click(), tr!("hint-remove")),
        ],
        Tool::Cut if tools.cut.picking_part => vec![(click(), tr!("hint-cut-part"))],
        _ => return None,
    };
    Some(pairs)
}
