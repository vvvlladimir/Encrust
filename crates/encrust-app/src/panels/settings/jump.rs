//! The tabs over a profile's form: a press scrolls the form to its section, and the section
//! at the top of the view lights its tab.

use egui::{Align, Id, Rect, Ui, pos2, vec2};

use crate::ui::window_frame::section_tab;

/// Points under the top of the view within which a section counts as the one in view.
const IN_VIEW: f32 = 24.0;

/// One form's tabs, kept from frame to frame: the tab lit, the one the sections drawn so far
/// would light, and the one pressed and not yet reached or scrolled away from.
#[derive(Clone, Copy, Default)]
struct Spy {
    lit: usize,
    seen: usize,
    pressed: Option<usize>,
    reached: Option<usize>,
}

/// A tab: its section's name, and whether the section is switched off.
pub(super) struct Tab {
    pub name: String,
    pub off: bool,
}

/// The tabs in a row; a press keeps the section to scroll to for [`section`]. `off` is the
/// word after a tab whose section is switched off.
pub(super) fn tabs(ui: &mut Ui, id: Id, tabs: &[Tab], off: &str) {
    let mut spy = spy(ui, id);
    if ui.input(|input| input.smooth_scroll_delta.y != 0.0) {
        spy.reached = None;
    }
    spy.lit = spy.reached.unwrap_or(spy.seen);
    spy.seen = 0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (index, tab) in tabs.iter().enumerate() {
            let off = tab.off.then_some(off);
            if section_tab(ui, &tab.name, off, spy.lit == index).clicked() {
                spy.pressed = Some(index);
            }
        }
    });
    ui.data_mut(|data| data.insert_temp(id, spy));
}

/// Marks where section `index` of the form starts, at `top`: scrolls there if its tab was
/// pressed, and lights the tab once the section reaches the top of the view.
pub(super) fn section(ui: &Ui, id: Id, index: usize, top: f32) {
    let mut spy = spy(ui, id);
    if spy.pressed == Some(index) {
        let at = Rect::from_min_size(pos2(ui.min_rect().left(), top), vec2(1.0, 1.0));
        ui.scroll_to_rect(at, Some(Align::TOP));
        spy.pressed = None;
        spy.reached = Some(index);
    }
    if top <= ui.clip_rect().top() + IN_VIEW {
        spy.seen = index;
    }
    ui.data_mut(|data| data.insert_temp(id, spy));
}

fn spy(ui: &Ui, id: Id) -> Spy {
    ui.data(|data| data.get_temp(id)).unwrap_or_default()
}
