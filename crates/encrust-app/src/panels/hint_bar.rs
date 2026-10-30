//! How to work the viewport with the tool in hand, centred under the tool row; drawn only
//! for a tool the pointer does something with there. See `docs/design/ui-screens.md`.

use egui::{Align2, Id, Modifiers, Rect};

use crate::drain::Placing as Drain;
use crate::i18n::tr;
use crate::measure::{Snap, Target};
use crate::panels::Window;
use crate::relief::ReliefMode;
use crate::supports::Placing as Supports;
use crate::ui::{hint_bar, icon};
use crate::workspace::Tool;

/// The bar, centred at the top of `room` and no wider, when the tool in hand has one.
pub fn ui(ui: &egui::Ui, window: &Window, room: Rect) {
    let Some(named) = pairs(window, &Keys::words(ui.ctx())) else {
        return;
    };
    let glyphs = pairs(window, &Keys::glyphs(ui.ctx())).unwrap_or_default();
    let named: Vec<(&str, &str)> = named
        .iter()
        .map(|(key, what)| (key.as_str(), what.as_str()))
        .collect();
    let keys: Vec<&str> = glyphs.iter().map(|(key, _)| key.as_str()).collect();
    egui::Area::new(Id::new("hint-bar"))
        .order(egui::Order::Middle)
        .fixed_pos(room.center_top())
        .pivot(Align2::CENTER_TOP)
        .show(ui.ctx(), |ui| hint_bar(ui, &named, &keys, room.width()));
}

/// What the keys and the pointer's moves are called: in words, or as glyphs where the bar
/// is short of room.
struct Keys {
    alt: String,
    command: String,
    shift: String,
    click: String,
    drag: String,
    glyphs: bool,
}

impl Keys {
    fn words(ctx: &egui::Context) -> Self {
        Self {
            alt: ctx.format_modifiers(Modifiers::ALT),
            command: ctx.format_modifiers(Modifiers::COMMAND),
            shift: tr!("hint-shift"),
            click: tr!("hint-click"),
            drag: tr!("hint-drag"),
            glyphs: false,
        }
    }

    /// The mouse's glyphs, and the Mac's key symbols; elsewhere the modifiers keep their names.
    fn glyphs(ctx: &egui::Context) -> Self {
        let words = Self::words(ctx);
        let mac = cfg!(target_os = "macos");
        let glyph = |glyph: &str, word: String| if mac { glyph.to_owned() } else { word };
        Self {
            alt: glyph(icon::KEY_OPTION, words.alt),
            command: glyph(icon::KEY_COMMAND, words.command),
            shift: glyph(icon::KEY_SHIFT, words.shift),
            click: icon::KEY_CLICK.to_owned(),
            drag: icon::KEY_DRAG.to_owned(),
            glyphs: true,
        }
    }

    /// A click with `modifier` held.
    fn click_with(&self, modifier: &str) -> String {
        match self.glyphs {
            true => format!("{modifier} {}", self.click),
            false => tr!("hint-modifier-click", key = modifier.to_owned()),
        }
    }
}

/// The key and what it does, pair by pair, for the tool in hand; `None` for a tool the
/// pointer does nothing with in the viewport.
fn pairs(window: &Window, keys: &Keys) -> Option<Vec<(String, String)>> {
    let click = || keys.click.clone();
    let alt_click = || keys.click_with(&keys.alt);
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
                    keys.click_with(&keys.command),
                    tr!("hint-pick-without-writing"),
                ),
            ],
            ReliefMode::Texture if tools.relief.paints() => vec![
                (keys.drag.clone(), tr!("hint-paint")),
                (keys.alt.clone(), tr!("hint-erase")),
            ],
            _ => return None,
        },
        Tool::Supports => vec![
            (click(), tr!("hint-supports-pick")),
            (keys.drag.clone(), tr!("hint-supports-carry")),
            (keys.shift.clone(), tr!("hint-add")),
            (keys.alt.clone(), tr!("hint-remove")),
        ],
        Tool::Paint if tools.supports.placing == Supports::Point => vec![
            (click(), tr!("hint-supports-place")),
            (keys.click_with(&keys.shift), tr!("hint-supports-line")),
            (alt_click(), tr!("hint-remove")),
        ],
        Tool::Cut if tools.cut.picking_part => vec![(click(), tr!("hint-cut-part"))],
        _ => return None,
    };
    Some(pairs)
}
