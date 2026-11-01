use super::SHAPE_INDENT;
use super::rows::{Act, InHand, Pick, Presets, Row};
use crate::i18n::tr;
use crate::ui::{
    PICKER_ICON_SQUARE, PickerLine, SquareLook, chooser_group, confirm_bin, eye_square, hairline,
    icon, picker_foot, picker_line, theme,
};
use egui::{Sense, vec2};

/// The open picker: the automatic preset and its shapes, each group by hand, the empty
/// profiles and the way to edit them; on Shape, every shape a click away.
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
        let idle_auto = self.idle_autos(ui, acts);
        let idle = self.by_hand(ui, acts, false);
        let edit = edit_profiles(ui).then_some(Pick::EditProfiles);
        auto.or(by_hand).or(idle_auto).or(idle).or(edit)
    }

    /// The shapes Auto built that stand, lightest first.
    fn built(&self) -> Vec<&Row> {
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
        built
    }

    /// The automatic presets among the profiles holding nothing, while Auto built none.
    fn idle_autos(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
        if self.shaping || !self.built().is_empty() {
            return None;
        }
        let mut pick = None;
        for offered in &self.presets.autos {
            let on = self.hand.in_auto && self.hand.name == offered.name;
            if Line::preset(&offered.name, None)
                .selected(on)
                .show(ui, self.rows.len(), acts)
            {
                pick = Some(Pick::Auto(offered.id.clone()));
            }
        }
        pick
    }

    pub(super) fn standing(&self, name: &str) -> Option<&Row> {
        self.rows
            .iter()
            .find(|row| row.holds > 0 && row.name == name)
    }

    /// Auto, and under it its shapes: those that stand, or on Shape every one, since each
    /// is edited there and Auto itself is not.
    pub(super) fn auto(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
        let built = self.built();
        let total: usize = built.iter().map(|row| row.holds).sum();
        let mut pick = None;
        for offered in &self.presets.autos {
            if !self.shaping && total == 0 {
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

    /// Draws the line; a group's eye and bin show under the pointer. Answers whether the
    /// line itself was clicked, which closes the list.
    pub(super) fn show(&self, ui: &mut egui::Ui, groups: usize, acts: &mut Vec<Act>) -> bool {
        let line = PickerLine {
            name: self.name,
            dot: self
                .group
                .map(|row| theme::support_tint(row.group as usize)),
            count: self.holds(),
            selected: self.selected,
            indent: self.indent,
            heading: self.heading,
            quiet: self.group.is_some_and(|row| row.hidden),
        };
        let drawn = picker_line(ui, &line);
        let on_icon = match self.group {
            Some(row) => {
                let id = drawn.response.id;
                group_icons(ui, drawn.icons, id, row, groups, drawn.hovered, acts)
            }
            None => false,
        };
        let clicked = drawn.response.clicked() && !on_icon;
        if clicked {
            ui.close();
        }
        clicked
    }
}

/// The room above the foot's rule.
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
            vec2(PICKER_ICON_SQUARE, PICKER_ICON_SQUARE),
        )
    };
    let mut took = false;
    if hovered || row.hidden {
        let (glyph, tip) = if row.hidden {
            (icon::HIDDEN, tr!("list-show"))
        } else {
            (icon::VISIBLE, tr!("list-hide"))
        };
        let eye_at = at(icons.left() + PICKER_ICON_SQUARE / 2.0);
        let eye = ui.interact(eye_at, id.with("eye"), Sense::click());
        eye_square(ui, eye.rect, glyph, eye.hovered());
        if eye.on_hover_text(tip).clicked() {
            acts.push(Act::Toggle(row.group));
            took = true;
        }
    }
    if hovered || crate::ui::is_armed(ui.ctx(), id.with("bin")) {
        let bin_at = at(icons.right() - PICKER_ICON_SQUARE / 2.0);
        let bin = ui.interact(bin_at, id.with("bin"), Sense::click());
        took |= bin.clicked();
        let (act, tip) = removal(groups, row);
        if confirm_bin(ui, bin, SquareLook::Quiet, &tip, true) {
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

/// The list's foot under a rule: Settings, at the support profiles. Answers whether it was
/// pressed, which closes the list.
fn edit_profiles(ui: &mut egui::Ui) -> bool {
    ui.add_space(FOOT_GAP);
    hairline(ui);
    let pressed = picker_foot(ui, icon::PROFILES, &tr!("supports-edit-profiles")).clicked();
    if pressed {
        ui.close();
    }
    pressed
}
