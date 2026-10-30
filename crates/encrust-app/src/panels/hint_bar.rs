//! How to work the viewport with the tool in hand, under the tool row; drawn only for a
//! tool the pointer does something with there. See `docs/design/ui-screens.md`.

use egui::{Align2, Id, Modifiers, Rect, pos2};

use crate::drain::Placing as Drain;
use crate::i18n::tr;
use crate::measure::{Snap, Target};
use crate::panels::Window;
use crate::relief::ReliefMode;
use crate::repair::{Picking, RepairMode};
use crate::supports::Placing as Supports;
use crate::ui::{hint_bar, hint_bar_names, icon};
use crate::workspace::Tool;

/// The bar, when the tool in hand has one, at the top of `room` and no wider: centred while
/// it names what each key does, the keys alone flush with the tool `row`'s left.
pub fn ui(ui: &egui::Ui, window: &Window, room: Rect, row: Option<Rect>) {
    let Some(named) = pairs(window, &Keys::words(ui.ctx())) else {
        return;
    };
    let glyphs = pairs(window, &Keys::glyphs(ui.ctx())).unwrap_or_default();
    let named: Vec<(&str, &str)> = named
        .iter()
        .map(|(key, what)| (key.as_str(), what.as_str()))
        .collect();
    let keys: Vec<&str> = glyphs.iter().map(|(key, _)| key.as_str()).collect();
    let flush = row.filter(|_| !hint_bar_names(ui, &named, room.width()));
    let (corner, pivot, width) = match flush {
        Some(row) => (
            pos2(row.left(), room.top()),
            Align2::LEFT_TOP,
            room.right() - row.left(),
        ),
        None => (room.center_top(), Align2::CENTER_TOP, room.width()),
    };
    egui::Area::new(Id::new("hint-bar"))
        .order(egui::Order::Middle)
        .fixed_pos(corner)
        .pivot(pivot)
        .show(ui.ctx(), |ui| hint_bar(ui, &named, &keys, width));
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

    /// A drag with `modifier` held.
    fn drag_with(&self, modifier: &str) -> String {
        match self.glyphs {
            true => format!("{modifier} {}", self.drag),
            false => tr!("hint-modifier-drag", key = modifier.to_owned()),
        }
    }
}

/// The key and what it does, pair by pair, for the tool in hand; `None` for a tool the
/// pointer does nothing with in the viewport. Every key is a click or a drag, with the
/// modifier it needs named before it.
fn pairs(window: &Window, keys: &Keys) -> Option<Vec<(String, String)>> {
    let click = |what: String| (keys.click.clone(), what);
    let remove = || (keys.click_with(&keys.alt), tr!("hint-remove"));
    let tools = &window.tools;
    let pairs = match *window.tool {
        Tool::Measure => measure(window, keys),
        Tool::Hollow => vec![click(tr!("hint-hollow-solid")), remove()],
        Tool::Drain => {
            let first = match tools.drain.placing {
                Drain::Hole => tr!("hint-drain-hole"),
                Drain::Channel => tr!("hint-drain-channel"),
            };
            vec![click(first), remove()]
        }
        Tool::Relief => match tools.relief.mode {
            ReliefMode::Text => vec![
                click(tr!("hint-relief-write")),
                (
                    keys.click_with(&keys.command),
                    tr!("hint-pick-without-writing"),
                ),
            ],
            ReliefMode::Texture if tools.relief.paints() => brush(keys, tr!("hint-paint")),
            _ => return None,
        },
        Tool::Supports => vec![
            click(tr!("hint-supports-pick")),
            (keys.drag.clone(), tr!("hint-supports-carry")),
            (keys.click_with(&keys.shift), tr!("hint-supports-add")),
        ],
        Tool::Paint => match tools.supports.placing {
            Supports::Point => vec![
                click(tr!("hint-supports-place")),
                (keys.click_with(&keys.shift), tr!("hint-supports-line")),
                remove(),
            ],
            Supports::Paint => brush(keys, tr!("hint-area-paint")),
            Supports::Block => brush(keys, tr!("hint-area-block")),
            Supports::Erase => vec![(keys.drag.clone(), tr!("hint-erase"))],
        },
        Tool::Repair if tools.repair.mode == RepairMode::Faces => match tools.repair.picking {
            Picking::Faces => vec![
                (keys.drag.clone(), tr!("hint-repair-faces")),
                (keys.drag_with(&keys.alt), tr!("hint-let-go")),
            ],
            Picking::Edges => vec![click(tr!("hint-repair-edge"))],
        },
        Tool::Cut if tools.cut.picking_part => vec![click(tr!("hint-cut-part"))],
        _ => return None,
    };
    Some(pairs)
}

/// Measure's keys: the next point or face, measuring off the snap, and starting again.
fn measure(window: &Window, keys: &Keys) -> Vec<(String, String)> {
    let measure = &window.tools.measure;
    let first = match (measure.target, measure.start().is_some()) {
        (Target::Points, false) => tr!("hint-measure-two-points"),
        (Target::Points, true) => tr!("hint-measure-second-point"),
        (Target::Planes, false) => tr!("hint-measure-two-faces"),
        (Target::Planes, true) => tr!("hint-measure-second-face"),
    };
    let mut pairs = vec![(keys.click.clone(), first)];
    if measure.target == Target::Points && measure.snap != Snap::Free {
        pairs.push((keys.click_with(&keys.alt), tr!("hint-measure-freely")));
    }
    pairs.push((tr!("hint-esc"), tr!("hint-clear")));
    pairs
}

/// A brush's keys: a drag lays `what` down, with Alt it takes it off.
fn brush(keys: &Keys, what: String) -> Vec<(String, String)> {
    vec![
        (keys.drag.clone(), what),
        (keys.drag_with(&keys.alt), tr!("hint-erase")),
    ]
}
