use super::rows::{Act, InHand, Pick, Presets, Row};
use super::{LINE_H, SHAPE_INDENT, count_at, swatch};
use crate::i18n::tr;
use crate::ui::{BinLook, confirm_bin, icon, theme};
use egui::{Sense, vec2};

/// The open picker: the automatic preset with the shapes it built under it, then the
/// presets by hand, each line the group built to it where one stands.
pub(super) struct List<'a> {
    pub(super) rows: &'a [Row],
    pub(super) presets: &'a Presets,
    pub(super) hand: &'a InHand,
    pub(super) shaping: bool,
}

impl List<'_> {
    pub(super) fn show(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
        let auto = self.auto(ui, acts);
        ui.separator();
        let by_hand = self.by_hand(ui, acts);
        auto.or(by_hand)
    }

    pub(super) fn standing(&self, name: &str) -> Option<&Row> {
        self.rows
            .iter()
            .find(|row| row.holds > 0 && row.name == name)
    }

    /// Auto, and under it its shapes: those that stand, or on Shape every one, since each
    /// is edited there and Auto itself is not.
    pub(super) fn auto(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
        let shape = |row: &&Row| {
            self.presets
                .shapes
                .iter()
                .any(|offered| offered.name == row.name)
        };
        let mut built: Vec<&Row> = self
            .rows
            .iter()
            .filter(|row| row.holds > 0)
            .filter(shape)
            .collect();
        built.sort_by(|a, b| a.weight.total_cmp(&b.weight));
        let total: usize = built.iter().map(|row| row.holds).sum();
        let mut pick = None;
        for offered in &self.presets.autos {
            let line = Line::heading(&offered.name, total, self.shaping)
                .selected(!self.shaping && self.hand.in_auto && self.hand.name == offered.name);
            if line.show(ui, self.rows.len(), acts) {
                pick = Some(Pick::Auto(offered.id.clone()));
            }
        }
        if self.shaping {
            for offered in &self.presets.shapes {
                let on = self.hand.matching.as_deref() == Some(offered.id.as_str());
                let line = Line::preset(&offered.name, self.standing(&offered.name)).selected(on);
                if line.indent(SHAPE_INDENT).show(ui, self.rows.len(), acts) {
                    pick = Some(Pick::Shape(offered.id.clone()));
                }
            }
        } else {
            for row in built {
                let line = Line::preset(&row.name, Some(row)).selected(self.hand.name == row.name);
                if line.indent(SHAPE_INDENT).show(ui, self.rows.len(), acts) {
                    acts.push(Act::Choose(row.group));
                }
            }
        }
        pick
    }

    /// The presets by hand, then any group whose preset is in neither list.
    pub(super) fn by_hand(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
        let mut pick = None;
        for offered in &self.presets.by_hand {
            let group = self.standing(&offered.name);
            let on = !self.hand.in_auto && self.hand.name == offered.name;
            if Line::preset(&offered.name, group)
                .selected(on)
                .show(ui, self.rows.len(), acts)
            {
                match group {
                    Some(row) => acts.push(Act::Choose(row.group)),
                    None => pick = Some(Pick::Preset(offered.id.clone())),
                }
            }
        }
        let presets = &self.presets;
        let listed = |name: &str| {
            let mut all = presets.shapes.iter().chain(&presets.by_hand);
            all.any(|offered| offered.name == name)
        };
        for row in self
            .rows
            .iter()
            .filter(|row| row.holds > 0 && !listed(&row.name))
        {
            let line = Line::preset(&row.name, Some(row)).selected(self.hand.name == row.name);
            if line.show(ui, self.rows.len(), acts) {
                acts.push(Act::Choose(row.group));
            }
        }
        pick
    }
}

/// One line of the open picker: a preset, and the group built to it where one stands.
pub(super) struct Line<'a> {
    pub(super) name: &'a str,
    pub(super) group: Option<&'a Row>,
    /// A figure for a line that is no group: Auto's sum of its shapes.
    pub(super) count: Option<usize>,
    pub(super) selected: bool,
    pub(super) indent: f32,
    /// A heading only names what is under it, and takes no click.
    pub(super) heading: bool,
}

impl<'a> Line<'a> {
    pub(super) fn preset(name: &'a str, group: Option<&'a Row>) -> Self {
        Self {
            name,
            group,
            count: None,
            selected: false,
            indent: 0.0,
            heading: false,
        }
    }

    pub(super) fn heading(name: &'a str, total: usize, heading: bool) -> Self {
        Self {
            count: (total > 0).then_some(total),
            heading,
            ..Self::preset(name, None)
        }
    }

    pub(super) fn selected(self, selected: bool) -> Self {
        Self { selected, ..self }
    }

    pub(super) fn indent(self, indent: f32) -> Self {
        Self { indent, ..self }
    }

    /// Draws the line; a group's eye and bin show under the pointer. Answers whether the
    /// line itself was clicked, which closes the list.
    pub(super) fn show(&self, ui: &mut egui::Ui, groups: usize, acts: &mut Vec<Act>) -> bool {
        let sense = if self.heading {
            Sense::hover()
        } else {
            Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), LINE_H), sense);
        let hovered = ui.rect_contains_pointer(rect);
        self.paint(ui, rect, hovered);
        let on_icon = match self.group {
            Some(row) => group_icons(ui, rect, response.id, row, groups, hovered, acts),
            None => false,
        };
        let clicked = response.clicked() && !on_icon;
        if clicked {
            ui.close();
        }
        clicked
    }

    pub(super) fn paint(&self, ui: &egui::Ui, rect: egui::Rect, hovered: bool) {
        let colors = theme::colors();
        let fill = match (self.selected, hovered && !self.heading) {
            (true, _) => colors.picked_wash,
            (false, true) => colors.hover,
            (false, false) => egui::Color32::TRANSPARENT,
        };
        let painter = ui.painter();
        painter.rect_filled(rect, theme::R_CONTROL, fill);
        let left = rect.left() + 8.0 + self.indent;
        let hidden = self.group.is_some_and(|row| row.hidden);
        if let Some(row) = self.group {
            swatch(painter, egui::pos2(left + 4.0, rect.center().y), row);
        }
        let ink = match (self.heading, hidden, self.selected) {
            (true, ..) => colors.text_mid,
            (false, true, _) => colors.text_low,
            (false, false, _) => colors.text_high,
        };
        let icons_left = rect.right() - ICONS_W;
        let mut right = icons_left - 40.0;
        if let Some(count) = self.group.map(|row| row.holds).or(self.count) {
            right = count_at(painter, icons_left - 6.0, rect.center().y, count) - 6.0;
        }
        let clip = egui::Rect::from_x_y_ranges(left + 16.0..=right, rect.y_range());
        painter.with_clip_rect(clip).text(
            egui::pos2(left + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            self.name,
            theme::label(),
            ink,
        );
    }
}

/// Room at the right of a line for a group's eye and bin, points.
pub(super) const ICONS_W: f32 = 44.0;

/// The eye of a group, shown under the pointer or while it is hidden, and its bin, shown
/// under the pointer. Answers whether either took the click.
pub(super) fn group_icons(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: egui::Id,
    row: &Row,
    groups: usize,
    hovered: bool,
    acts: &mut Vec<Act>,
) -> bool {
    let at =
        |x: f32| egui::Rect::from_center_size(egui::pos2(x, rect.center().y), vec2(20.0, 20.0));
    let mut took = false;
    if hovered || row.hidden {
        let (glyph, tip) = if row.hidden {
            (icon::HIDDEN, tr!("list-show"))
        } else {
            (icon::VISIBLE, tr!("list-hide"))
        };
        let eye = ui.interact(at(rect.right() - 32.0), id.with("eye"), Sense::click());
        eye_glyph(ui, eye.rect, glyph, eye.hovered());
        if eye.on_hover_text(tip).clicked() {
            acts.push(Act::Toggle(row.group));
            took = true;
        }
    }
    if hovered {
        let bin = ui.interact(at(rect.right() - 11.0), id.with("bin"), Sense::click());
        took |= bin.clicked();
        let (act, tip) = removal(groups, row);
        if confirm_bin(ui, bin, BinLook::Quiet, &tip, true) {
            acts.push(act);
        }
    }
    took
}

/// What the bin of a group does among `groups`: the group goes with its supports, or,
/// the last one, is emptied; and the words that say so.
pub(super) fn removal(groups: usize, row: &Row) -> (Act, String) {
    if groups > 1 {
        let tip = tr!(
            "supports-drop-preset",
            name = row.name.clone(),
            count = row.holds
        );
        (Act::Drop(row.group), tip)
    } else {
        let tip = tr!("supports-clear-preset", count = row.holds);
        (Act::Clear(row.group), tip)
    }
}

pub(super) fn eye_glyph(ui: &egui::Ui, rect: egui::Rect, glyph: &str, hot: bool) {
    let colors = theme::colors();
    if hot {
        ui.painter()
            .rect_filled(rect, theme::R_CONTROL, colors.raised);
    }
    let ink = if hot {
        colors.text_high
    } else {
        colors.text_low
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        theme::icon(13.0),
        ink,
    );
}
