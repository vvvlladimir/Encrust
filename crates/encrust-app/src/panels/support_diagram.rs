//! The drawing over the Shape tool's fields: one support on its side, the picked part in the
//! accent, true to scale with idle stretches cut as breaks. See docs/design/supports.md.

use std::cell::RefCell;

use egui::{Align2, Rect, Ui, pos2, vec2};
use printer_profiles::SupportProfile;

use crate::i18n::tr;

mod body;
mod brace;
mod column;
mod draw;
mod raft;
mod tip;
mod view;
use body::{base_view, body_view};
use brace::brace_view;
use column::station;
use draw::{Pen, across, breaks, canvas, dimension, draw_revolved, far_wall, model_wall, text};
use raft::raft_view;
use tip::tip_view;
use view::View;

/// The drawing's height, points; its width is the panel's.
const HEIGHT: f32 = 196.0;
/// Room between the drawing and its frame, and the least width of a hatched wall, points.
const INSET: f32 = 12.0;
const WALL_W: f32 = 12.0;
const HATCH_GAP: f32 = 7.0;
/// Room over the walls for the word saying what each one is, points.
const CAPTION: f32 = 14.0;
/// Room kept either side of the drawing for two rows of figures, points.
const MARGIN: f32 = 34.0;
/// How wide a cut-out stretch is drawn, points.
const BREAK_W: f32 = 14.0;

/// Rows of figures off the widest part drawn, points: the first and the second.
const LANE: f32 = 11.0;
const LANE_STEP: f32 = 15.0;
/// How tall a figure stands, and the least width one is written inside, points.
const FIGURE_H: f32 = 14.0;
const FITS: f32 = 16.0;

const ARROW: f32 = 4.0;
const LINE_W: f32 = 1.2;

/// Room inside the model left of the tip for the contact's figure, points.
const TIP_LABEL_W: f32 = 44.0;

/// Room left of the braces for their figures, points.
const BRACE_LABEL_W: f32 = 52.0;

/// Rings a sphere tip is drawn with, and the middle levels drawn between two trunks: two,
/// so a zigzag shows it turns.
const BALL_STEPS: u32 = 12;
const BRACE_MIDDLE_LEVELS: u32 = 2;

/// One part of a regular support, which is what the Part switch picks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Part {
    #[default]
    Tip,
    Body,
    Base,
    Raft,
    Brace,
}

impl Part {
    pub const ALL: [Self; 5] = [Self::Tip, Self::Body, Self::Base, Self::Raft, Self::Brace];

    pub fn label(self) -> String {
        match self {
            Self::Tip => tr!("diagram-tip"),
            Self::Body => tr!("diagram-body"),
            Self::Base => tr!("diagram-base"),
            Self::Raft => tr!("diagram-raft"),
            Self::Brace => tr!("diagram-brace"),
        }
    }
}

fn mm(value: f32) -> String {
    format!("{value:.2}")
}

fn diameter(value: f32) -> String {
    format!("Ø{value:.2}")
}

/// Draws a regular support with `picked` in the accent and its measurements on it.
pub fn regular(ui: &mut Ui, profile: &SupportProfile, picked: Part) {
    clean_sheet();
    let frame = canvas(ui);
    let painter = ui.painter_at(frame.shrink(1.0));
    match picked {
        Part::Tip => tip_view(&painter, frame, profile),
        Part::Body => body_view(&painter, frame, profile),
        Part::Base => base_view(&painter, frame, profile),
        Part::Raft => raft_view(&painter, frame, profile),
        Part::Brace => brace_view(&painter, frame, profile),
    }
}

/// Draws the thin strut between two parts of the model, both ends sunk into a wall, the
/// stretch between them cut out.
pub fn small_pillar(ui: &mut Ui, profile: &SupportProfile) {
    clean_sheet();
    let frame = canvas(ui);
    let painter = ui.painter_at(frame.shrink(1.0));
    let pillar = &profile.small_pillar;
    let half = pillar.diameter_mm / 2.0;
    let (upper, lower) = (pillar.upper_depth_mm, pillar.lower_depth_mm);
    let shown = (pillar.diameter_mm * 4.0).max(upper.max(lower) * 3.0);
    let length = shown * 4.0;
    let along = (-upper - shown * 0.6, length + lower + shown * 0.6);
    let view = View::fit(frame, along, half, vec![(shown, length - shown)]);
    model_wall(&painter, &view);
    far_wall(&painter, &view, length);
    let stations = [
        station(-upper, half, Part::Body),
        station(length + lower, half, Part::Body),
    ];
    let pen = |_| Pen::Picked;
    draw_revolved(&painter, &view, &stations, 0.0, &pen, !pillar.enabled);
    breaks(&painter, &view, &stations);

    let y = view.origin.y;
    let thick = view.len(half);
    let middle = view.x(shown / 2.0);
    let above = (pos2(middle, view.lane(thick, 1)), Align2::CENTER_CENTER);
    let width = diameter(pillar.diameter_mm);
    across(&painter, pos2(middle, y), thick, &width, None, above);
    let under = view.lane(thick, -1);
    let (left, right) = (view.x(0.0), view.x(length));
    dimension(&painter, pos2(view.x(-upper), under), pos2(left, under), "");
    let far = pos2(view.x(length + lower), under);
    dimension(&painter, pos2(right, under), far, "");
    let upper_at = pos2(left + ARROW * 3.0, under);
    text(&painter, upper_at, Align2::LEFT_CENTER, &mm(upper));
    let lower_at = pos2(right - ARROW * 3.0, under);
    text(&painter, lower_at, Align2::RIGHT_CENTER, &mm(lower));
    let over = view.lane(thick, 2);
    let longest = tr!("diagram-max", value = mm(pillar.max_length_mm));
    dimension(&painter, pos2(left, over), pos2(right, over), &longest);
}

thread_local! {
    /// The figures written on the drawing in hand, so a later one keeps clear of them.
    static WRITTEN: RefCell<Vec<Rect>> = const { RefCell::new(Vec::new()) };
}

/// Starts a drawing with nothing written on it yet.
fn clean_sheet() {
    WRITTEN.with(|written| written.borrow_mut().clear());
}

/// Where a figure wanted at `wanted` goes: inside `room`, then row by row nearest first until
/// it covers no written figure, so none is cut off or overwritten.
fn free_spot(wanted: Rect, room: Rect) -> Rect {
    let inside = |rect: Rect| {
        let dx = (room.left() - rect.left()).max(0.0) + (room.right() - rect.right()).min(0.0);
        let dy = (room.top() - rect.top()).max(0.0) + (room.bottom() - rect.bottom()).min(0.0);
        rect.translate(vec2(dx, dy))
    };
    let row = wanted.height() + 2.0;
    WRITTEN.with(|written| {
        let mut written = written.borrow_mut();
        let clear = |rect: &Rect| {
            let padded = rect.expand2(vec2(3.0, 1.0));
            !written.iter().any(|other| other.intersects(padded))
        };
        let spot = (0..12)
            .map(|step| {
                let reach = ((step + 1) / 2) as f32 * row;
                let up = if step % 2 == 1 { -reach } else { reach };
                inside(wanted.translate(vec2(0.0, up)))
            })
            .find(clear)
            .unwrap_or_else(|| inside(wanted));
        written.push(spot);
        spot
    })
}
