use super::rows::{Act, InHand, Pick, Presets, Row};
use super::{LINE_H, SHAPE_INDENT, count_at, swatch};
use crate::i18n::tr;
use crate::ui::{BinLook, chooser_group, confirm_bin, focus_ring, hairline, icon, theme};
use egui::{Sense, vec2};

/// The open picker: what stands on the plate — the automatic preset with the shapes it
/// built set in under it, then each group by hand — the profiles holding nothing, and the
/// way to edit them. On Shape, every shape a click away.
pub(super) struct List<'a> {
    pub(super) rows: &'a [Row],
    pub(super) presets: &'a Presets,
    pub(super) hand: &'a InHand,
    pub(super) shaping: bool,
}

impl List<'_> {
    pub(super) fn show(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
        chooser_group(ui, &tr!("supports-on-plate"));
        let auto = self.auto(ui, acts);
        let by_hand = self.by_hand(ui, acts, true);
        chooser_group(ui, &tr!("supports-profiles"));
        let idle = self.by_hand(ui, acts, false);
        let edit = edit_profiles(ui).then_some(Pick::EditProfiles);
        auto.or(by_hand).or(idle).or(edit)
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
        let in_auto = self.hand.in_auto;
        for offered in &self.presets.autos {
            if !self.shaping && total == 0 && !(in_auto && self.hand.name == offered.name) {
                continue;
            }
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

    /// The presets by hand with a group standing, then any group whose preset is in
    /// neither list; or, not `standing`, the profiles holding nothing.
    pub(super) fn by_hand(
        &self,
        ui: &mut egui::Ui,
        acts: &mut Vec<Act>,
        standing: bool,
    ) -> Option<Pick> {
        let mut pick = None;
        for offered in &self.presets.by_hand {
            let group = self.standing(&offered.name);
            if group.is_some() != standing {
                continue;
            }
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
        if !standing {
            return pick;
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

/// Where a line's parts stand: the name's room, the count's right edge, the eye and the bin.
struct Places {
    name: egui::Rect,
    count_right: f32,
    icons: egui::Rect,
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

    fn holds(&self) -> Option<usize> {
        self.group.map(|row| row.holds).or(self.count)
    }

    /// Right to left: the eye and the bin, the count; the name takes what is left.
    fn places(&self, rect: egui::Rect) -> Places {
        let mut right = rect.right() - RIGHT_PAD;
        let icons = egui::Rect::from_x_y_ranges(right - ICONS_W..=right, rect.y_range());
        let count_right = icons.left() - GAP;
        if self.holds().is_some() {
            right = count_right - COUNT_W - GAP;
        }
        let left = rect.left() + LEFT_PAD + self.indent + DOT_W + GAP;
        Places {
            name: egui::Rect::from_x_y_ranges(left..=right, rect.y_range()),
            count_right,
            icons,
        }
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
        let places = self.places(rect);
        self.paint(ui, rect, &places, hovered);
        let on_icon = match self.group {
            Some(row) => group_icons(ui, places.icons, response.id, row, groups, hovered, acts),
            None => false,
        };
        let clicked = response.clicked() && !on_icon;
        if clicked {
            ui.close();
        }
        clicked
    }

    fn paint(&self, ui: &egui::Ui, rect: egui::Rect, places: &Places, hovered: bool) {
        let colors = theme::colors();
        let fill = match (self.selected, hovered && !self.heading) {
            (true, _) => colors.picked_wash,
            (false, true) => colors.hover,
            (false, false) => egui::Color32::TRANSPARENT,
        };
        let painter = ui.painter();
        painter.rect_filled(rect, theme::R_CONTROL, fill);
        let y = rect.center().y;
        if let Some(row) = self.group {
            let dot = rect.left() + LEFT_PAD + self.indent + DOT_W / 2.0;
            swatch(painter, egui::pos2(dot, y), row);
        }
        let hidden = self.group.is_some_and(|row| row.hidden);
        let ink = match (self.heading || hidden, self.holds()) {
            (true, _) => colors.text_low,
            (false, None) => colors.text_mid,
            (false, Some(_)) => colors.text_high,
        };
        if let Some(count) = self.holds() {
            count_at(painter, places.count_right, y, count);
        }
        painter.with_clip_rect(places.name).text(
            places.name.left_center(),
            egui::Align2::LEFT_CENTER,
            self.name,
            theme::label(),
            ink,
        );
    }
}

/// A line's room at its ends, the square its dot stands in, the room between its parts,
/// the least a count takes, and a glyph of the foot, points.
const LEFT_PAD: f32 = 10.0;
const RIGHT_PAD: f32 = 4.0;
const DOT_W: f32 = 8.0;
const GAP: f32 = 8.0;
const COUNT_W: f32 = 20.0;
const GLYPH_W: f32 = 14.0;
/// The eye and the bin, each a 26 square, and the glyph in one.
const ICON_SQUARE: f32 = 26.0;
const ICONS_W: f32 = 2.0 * ICON_SQUARE;
const ICON_GLYPH: f32 = 15.0;
/// The foot's height and the room above its rule.
const FOOT_H: f32 = 34.0;
const FOOT_GAP: f32 = 4.0;

/// The eye of a group, shown under the pointer or while it is hidden, and its bin, shown
/// under the pointer, in `icons`. Answers whether either took the click.
fn group_icons(
    ui: &mut egui::Ui,
    icons: egui::Rect,
    id: egui::Id,
    row: &Row,
    groups: usize,
    hovered: bool,
    acts: &mut Vec<Act>,
) -> bool {
    let at = |x: f32| {
        egui::Rect::from_center_size(
            egui::pos2(x, icons.center().y),
            vec2(ICON_SQUARE, ICON_SQUARE),
        )
    };
    let mut took = false;
    if hovered || row.hidden {
        let (glyph, tip) = if row.hidden {
            (icon::HIDDEN, tr!("list-show"))
        } else {
            (icon::VISIBLE, tr!("list-hide"))
        };
        let eye_at = at(icons.left() + ICON_SQUARE / 2.0);
        let eye = ui.interact(eye_at, id.with("eye"), Sense::click());
        eye_glyph(ui, eye.rect, glyph, eye.hovered());
        if eye.on_hover_text(tip).clicked() {
            acts.push(Act::Toggle(row.group));
            took = true;
        }
    }
    if hovered {
        let bin_at = at(icons.right() - ICON_SQUARE / 2.0);
        let bin = ui.interact(bin_at, id.with("bin"), Sense::click());
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

fn eye_glyph(ui: &egui::Ui, rect: egui::Rect, glyph: &str, hot: bool) {
    let colors = theme::colors();
    if hot {
        ui.painter()
            .rect_filled(rect, theme::R_CONTROL, colors.raised);
    }
    let ink = if hot {
        colors.text_high
    } else {
        colors.text_mid
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        theme::icon(ICON_GLYPH),
        ink,
    );
}

/// The list's foot under a rule: Settings, at the support profiles. Answers whether it was
/// pressed, which closes the list.
fn edit_profiles(ui: &mut egui::Ui) -> bool {
    let colors = theme::colors();
    let text = tr!("supports-edit-profiles");
    ui.add_space(FOOT_GAP);
    hairline(ui);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), FOOT_H), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &text));
    let ink = if response.hovered() {
        colors.text_high
    } else {
        colors.text_mid
    };
    let painter = ui.painter();
    let y = rect.center().y;
    let glyph_x = rect.left() + LEFT_PAD + GLYPH_W / 2.0;
    painter.text(
        egui::pos2(glyph_x, y),
        egui::Align2::CENTER_CENTER,
        icon::PROFILES,
        theme::icon(GLYPH_W),
        ink,
    );
    painter.text(
        egui::pos2(glyph_x + GLYPH_W / 2.0 + GAP, y),
        egui::Align2::LEFT_CENTER,
        &text,
        theme::label(),
        ink,
    );
    painter.text(
        egui::pos2(rect.right() - LEFT_PAD - GLYPH_W / 2.0, y),
        egui::Align2::CENTER_CENTER,
        icon::ELSEWHERE,
        theme::icon(GLYPH_W),
        ink,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    if response.clicked() {
        ui.close();
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
