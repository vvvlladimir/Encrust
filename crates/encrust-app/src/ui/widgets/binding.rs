//! One action of the key sheet: its name, each key that fires it as a button that records
//! a new chord, a place for one more, and the way back to the keys it came with.

use egui::{Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::button::{focus_ring, hover_mix};
use super::keycap::{keycap, keycap_width};
use crate::i18n::tr;
use crate::ui::{icon, theme};

/// The row, and the buttons standing in it.
const ROW_H: f32 = 38.0;
const CHIP_H: f32 = theme::SMALL_ICON_SIZE;
/// Room around a key's caps inside its button, between two caps, and between two buttons.
const CHIP_PAD: f32 = 4.0;
const CAP_GAP: f32 = 3.0;
const CHIP_GAP: f32 = 6.0;

/// How a row of the key sheet stands.
pub struct BindingRow<'a> {
    pub name: &'a str,
    /// The caps of each key that fires the action, modifiers first.
    pub chords: &'a [Vec<&'a str>],
    /// The key recording a new chord, if one of this row's is.
    pub recording: Option<Recording<'a>>,
    /// Whether the action has room for another key.
    pub room: bool,
    /// Whether its keys are the user's rather than the ones it came with.
    pub changed: bool,
    /// Whether its keys are listed only, as a gesture of the pointer is.
    pub fixed: bool,
}

/// A key of a [`binding_row`] recording, until Done keeps what was pressed.
pub struct Recording<'a> {
    /// Which key, `chords.len()` for the place of a new one.
    pub index: usize,
    /// The caps pressed last, or the modifiers held so far; empty while nothing is.
    pub caps: Vec<&'a str>,
    /// Whether a whole chord was pressed, so Done has something to keep.
    pub complete: bool,
}

/// What a press in a [`binding_row`] asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingPress {
    /// Record a new chord in place of key `n`, or stop recording it and keep nothing.
    Key(usize),
    /// Keep the chord recorded.
    Done,
    /// Record a chord for the place of a new key.
    Add,
    /// Take key `n` off the action.
    Clear(usize),
    /// Give the action back the keys it came with.
    Reset,
}

/// The name at the left, then from the right the way back while changed, the place for a
/// new key while there is room, and the keys; a key recording is washed in the accent and
/// shows what is pressed, Done and the way to take the key off beside it.
pub fn binding_row(ui: &mut Ui, row: &BindingRow<'_>) -> Option<BindingPress> {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let painter = ui.painter();
    painter.text(
        pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        row.name,
        theme::label(),
        colors.text_high,
    );
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, colors.hairline),
    );
    // The way back and the place for a new key keep their columns on every row, so the
    // keys of every row end on one line.
    let mut right = rect.right();
    if row.fixed {
        right -= (CHIP_H + CHIP_GAP) * 2.0 + CHIP_GAP;
        for chord in row.chords.iter().rev() {
            right = caps(ui, chord, right - CHIP_PAD, rect.center().y) - CHIP_PAD - CHIP_GAP;
        }
        return None;
    }
    let name = row.name;
    let mut pressed = None;
    let reset = (icon::RESET, tr!("binding-reset"));
    let reset = (reset.0, reset.1.as_str());
    if !row.changed {
        right -= CHIP_H + CHIP_GAP;
    } else if glyph_button(ui, rect, &mut right, reset, (name, Place::Reset)) {
        pressed = Some(BindingPress::Reset);
    }
    right -= CHIP_GAP;
    let recording = row.recording.as_ref();
    let adding = recording.filter(|recording| recording.index == row.chords.len());
    let add = (icon::ADD, tr!("binding-add"));
    if let Some(recording) = adding {
        pressed = pressed.or(recording_key(
            ui,
            rect,
            &mut right,
            (recording, false),
            name,
        ));
    } else if !row.room {
        right -= CHIP_H + CHIP_GAP;
    } else if glyph_button(ui, rect, &mut right, (add.0, &add.1), (name, Place::Add)) {
        pressed = Some(BindingPress::Add);
    }
    for (index, chord) in row.chords.iter().enumerate().rev() {
        match recording.filter(|recording| recording.index == index) {
            Some(recording) => {
                let at = (recording, true);
                pressed = pressed.or(recording_key(ui, rect, &mut right, at, name));
            }
            None => {
                let place = (name, Place::Key(index));
                if key_button(ui, rect, &mut right, chord, place).clicked() {
                    pressed = Some(BindingPress::Key(index));
                }
            }
        }
    }
    pressed
}

/// The caps of one chord drawn leftwards from `right`; answers their left edge.
fn caps(ui: &Ui, chord: &[&str], right: f32, center_y: f32) -> f32 {
    let mut left = right;
    for cap in chord.iter().rev() {
        left = keycap(ui.painter(), cap, left, center_y) - CAP_GAP;
    }
    left + CAP_GAP
}

fn caps_width(ui: &Ui, chord: &[&str]) -> f32 {
    let caps: f32 = chord
        .iter()
        .map(|cap| keycap_width(ui.painter(), cap) + CAP_GAP)
        .sum();
    caps - CAP_GAP
}

/// Where a control of the row stands, for its id: steady while the row scrolls.
#[derive(Clone, Copy, Debug, Hash)]
enum Place {
    Reset,
    Add,
    Key(usize),
    Clear(usize),
    Done,
}

/// A chip of `width` standing left of `right` in the row, which moves `right` past it.
fn chip(
    ui: &mut Ui,
    row: Rect,
    right: &mut f32,
    width: f32,
    place: (&str, Place),
) -> (Rect, Response) {
    let rect = Rect::from_min_size(
        pos2(*right - width, row.center().y - CHIP_H / 2.0),
        vec2(width, CHIP_H),
    );
    *right = rect.left() - CHIP_GAP;
    let id = egui::Id::new(("binding", place));
    (rect, ui.interact(rect, id, Sense::click()))
}

/// One key of the action, its caps on a surface that shows on hover.
fn key_button(
    ui: &mut Ui,
    row: Rect,
    right: &mut f32,
    chord: &[&str],
    place: (&str, Place),
) -> Response {
    let name = place.0;
    let colors = theme::colors();
    let width = caps_width(ui, chord) + CHIP_PAD * 2.0;
    let (rect, response) = chip(ui, row, right, width, place);
    let keys = chord.join(" ");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{name} {keys}"))
    });
    let fill = hover_mix(ui, &response, egui::Color32::TRANSPARENT, colors.hover);
    let ring = hover_mix(ui, &response, egui::Color32::TRANSPARENT, colors.line);
    ui.painter().rect(
        rect,
        theme::R_CONTROL,
        fill,
        Stroke::new(1.0, ring),
        StrokeKind::Inside,
    );
    caps(ui, chord, rect.right() - CHIP_PAD, rect.center().y);
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_text(tr!("binding-change"))
}

/// A key recording, from the right: the way to take it off when `held` is a key the
/// action has, Done, and what was pressed.
fn recording_key(
    ui: &mut Ui,
    row: Rect,
    right: &mut f32,
    (recording, held): (&Recording<'_>, bool),
    name: &str,
) -> Option<BindingPress> {
    let mut pressed = None;
    let index = recording.index;
    let clear = tr!("binding-clear");
    let off = (icon::CANCEL, clear.as_str());
    if held && glyph_button(ui, row, right, off, (name, Place::Clear(index))) {
        pressed = Some(BindingPress::Clear(index));
    }
    if done_button(ui, row, right, recording.complete, name) {
        pressed = Some(BindingPress::Done);
    }
    if listening(ui, row, right, recording, (name, Place::Key(index))).clicked() {
        pressed = Some(BindingPress::Key(index));
    }
    pressed
}

/// The key waiting for its chord: the accent's wash, ringed, showing what is pressed or
/// asking for it.
fn listening(
    ui: &mut Ui,
    row: Rect,
    right: &mut f32,
    recording: &Recording<'_>,
    place: (&str, Place),
) -> Response {
    let name = place.0;
    let colors = theme::colors();
    let ask = tr!("binding-listening");
    let galley = ui
        .painter()
        .layout_no_wrap(ask.clone(), theme::small(), colors.accent_soft);
    let width = match recording.caps.is_empty() {
        true => galley.size().x + CHIP_PAD * 4.0,
        false => caps_width(ui, &recording.caps) + CHIP_PAD * 2.0,
    };
    let (rect, response) = chip(ui, row, right, width, place);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{name} {ask}"))
    });
    ui.painter().rect(
        rect,
        theme::R_CONTROL,
        colors.accent_wash,
        Stroke::new(1.0, colors.accent),
        StrokeKind::Inside,
    );
    if recording.caps.is_empty() {
        let at = rect.center() - galley.size() / 2.0;
        ui.painter().galley(at, galley, colors.accent_soft);
    } else {
        caps(
            ui,
            &recording.caps,
            rect.right() - CHIP_PAD,
            rect.center().y,
        );
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_text(tr!("binding-stop"))
}

/// Done: the accent's own button once a whole chord was pressed, quiet before.
fn done_button(ui: &mut Ui, row: Rect, right: &mut f32, enabled: bool, name: &str) -> bool {
    let colors = theme::colors();
    let text = tr!("binding-done");
    let galley = ui
        .painter()
        .layout_no_wrap(text.clone(), theme::small(), colors.on_accent);
    let width = galley.size().x + CHIP_PAD * 4.0;
    let (rect, response) = chip(ui, row, right, width, (name, Place::Done));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &text));
    let (fill, ink) = match enabled {
        true => (
            hover_mix(ui, &response, colors.accent, colors.accent_soft),
            colors.on_accent,
        ),
        false => (colors.raised, colors.text_low),
    };
    ui.painter().rect_filled(rect, theme::R_CONTROL, fill);
    let at = rect.center() - galley.size() / 2.0;
    ui.painter().galley(at, galley, ink);
    focus_ring(ui, &response, theme::R_CONTROL);
    enabled && response.clicked()
}

/// A square of one glyph in the row, faint until hovered. Answers whether it was pressed.
fn glyph_button(
    ui: &mut Ui,
    row: Rect,
    right: &mut f32,
    (glyph, tooltip): (&str, &str),
    place: (&str, Place),
) -> bool {
    let colors = theme::colors();
    let (rect, response) = chip(ui, row, right, CHIP_H, place);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tooltip));
    let fill = hover_mix(ui, &response, egui::Color32::TRANSPARENT, colors.hover);
    let ink = hover_mix(ui, &response, colors.text_low, colors.text_high);
    ui.painter().rect_filled(rect, theme::R_CONTROL, fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        theme::icon(14.0),
        ink,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_text(tooltip).clicked()
}
