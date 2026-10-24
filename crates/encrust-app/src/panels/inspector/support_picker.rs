use anyhow::Context as _;
use egui::{Sense, vec2};
use printer_profiles::Catalogue;

use super::scope;
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::support_fields::saved_as;
use crate::settings::keep_support;
use crate::status::Status;
use crate::ui::{BinLook, bin_button, confirm_bin, hairline, icon, icon_button, theme};
use crate::workspace::Tool;

/// Height of one line of the list, points.
const LINE_H: f32 = 26.0;
/// How far the shapes of the automatic preset stand in under it, points.
const SHAPE_INDENT: f32 = 16.0;

/// The preset in hand on one line under the card's heading: the picker that is also the
/// list of groups standing, Save, and the bin. See `docs/decisions/0338`.
pub(super) fn line(ui: &mut egui::Ui, window: &mut Window) {
    let mut acts = Vec::new();
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                // The group in hand may not change while a model's own numbers are held.
                scope::editing_supports(window, |window| acts = picker(ui, window));
            });
        });
    hairline(ui);
    apply(window, acts);
}

/// One group as the picker reads it: named for the preset it is built to, never for what
/// it was once called.
struct Row {
    group: u16,
    name: String,
    /// Tip contact diameter, mm: what orders the presets lightest first.
    weight: f32,
    holds: usize,
    hidden: bool,
    active: bool,
}

/// What a click on a group asked for, done once the group in hand may change.
enum Act {
    Choose(u16),
    Toggle(u16),
    Clear(u16),
    Drop(u16),
}

/// A preset chosen from the list, taken up while the picked models' numbers are held.
enum Pick {
    Auto(String),
    Preset(String),
    Shape(String),
}

fn rows(window: &Window) -> Vec<Row> {
    let tool = &window.tools.supports;
    let rows = tool.groups.iter().enumerate().map(|(index, entry)| {
        let group = index as u16;
        let active = group == tool.active;
        let profile = if active {
            &tool.profile
        } else {
            &entry.profile
        };
        Row {
            group,
            name: profile.name.clone(),
            weight: profile.tip.contact_diameter_mm,
            holds: tool.group_holds(group, &window.doc.scene),
            hidden: entry.hidden,
            active,
        }
    });
    rows.collect()
}

/// One preset of the catalogue as the list offers it: its id, name and weight.
struct Offered {
    id: String,
    name: String,
    weight: f32,
}

/// The presets the list offers: the automatic ones, the shapes they build to, and those
/// chosen by hand, each lightest first.
struct Presets {
    autos: Vec<Offered>,
    shapes: Vec<Offered>,
    by_hand: Vec<Offered>,
}

fn presets(catalogue: &Catalogue) -> Presets {
    let offered = |auto_shape: bool| {
        let mut list: Vec<Offered> = catalogue
            .supports()
            .filter(|entry| catalogue.is_auto_shape(&entry.id) == auto_shape)
            .map(|entry| Offered {
                id: entry.id.clone(),
                name: entry.profile.name.clone(),
                weight: entry.profile.tip.contact_diameter_mm,
            })
            .collect();
        list.sort_by(|a, b| a.weight.total_cmp(&b.weight).then(a.name.cmp(&b.name)));
        list
    };
    let autos = catalogue.autos().map(|entry| Offered {
        id: entry.id.clone(),
        name: entry.profile.name.clone(),
        weight: 0.0,
    });
    Presets {
        autos: autos.collect(),
        shapes: offered(true),
        by_hand: offered(false),
    }
}

/// Which preset is in hand, what the picker calls it, and where Save writes it.
struct InHand {
    /// The preset's name, or the group's while one stands in it.
    name: String,
    edited: bool,
    in_auto: bool,
    /// The support profile in hand, by catalogue id, while it is unedited.
    matching: Option<String>,
    /// The automatic preset an edit is saved over, while it is the one shown.
    auto_source: Option<String>,
    /// The support profile an edit is saved over.
    source: Option<String>,
}

fn in_hand(window: &Window, active: &Row) -> InHand {
    let catalogue = &window.machine.slicing.catalogue;
    let tool = &window.tools.supports;
    let current = &tool.profile;
    // On Shape the automatic preset opens into the shapes it builds to, each its own.
    let shaping = *window.tool == Tool::Shape;
    let sizing = tool.auto.as_ref().filter(|_| !shaping);
    let matching = catalogue
        .supports()
        .find(|entry| (tool.auto.is_none() || shaping) && entry.profile == *current)
        .map(|entry| entry.id.clone());
    let (name, edited, auto_source) = match sizing {
        Some(auto) => {
            let shipped = catalogue
                .autos()
                .find(|entry| entry.profile.name == auto.name);
            let edited = shipped.is_some_and(|entry| entry.profile != *auto);
            (
                auto.name.clone(),
                edited,
                shipped.map(|entry| entry.id.clone()),
            )
        }
        None => (current.name.clone(), matching.is_none(), None),
    };
    InHand {
        name: if active.holds > 0 && !shaping {
            active.name.clone()
        } else {
            name
        },
        edited,
        in_auto: tool.auto.is_some(),
        matching,
        auto_source,
        source: saved_as(catalogue, current).map(|entry| entry.id.clone()),
    }
}

/// The picker, Save and the bin. Answers what was asked of the groups.
fn picker(ui: &mut egui::Ui, window: &mut Window) -> Vec<Act> {
    let rows = rows(window);
    let mut acts = Vec::new();
    let Some(active) = rows.iter().find(|row| row.active) else {
        return acts;
    };
    let hand = in_hand(window, active);
    let presets = presets(&window.machine.slicing.catalogue);
    ui.spacing_mut().item_spacing.x = 4.0;
    let beside = 2.0 * (theme::ICON_SIZE + 4.0);
    let width = (ui.available_width() - beside).max(80.0);
    let (_, response) = ui.allocate_exact_size(vec2(width, theme::FIELD_H), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, tr!("supports-preset"))
    });
    let label = if hand.edited {
        tr!("shape-edited", name = hand.name.clone())
    } else {
        hand.name.clone()
    };
    closed(ui, &response, active, &label);
    let list = List {
        rows: &rows,
        presets: &presets,
        hand: &hand,
        shaping: *window.tool == Tool::Shape,
    };
    let mut pick = None;
    egui::Popup::menu(&response)
        .width(width + beside)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(width + beside - 12.0);
            ui.spacing_mut().item_spacing.y = 1.0;
            pick = list.show(ui, &mut acts);
        });
    save_button(ui, window, &hand);
    let (act, tip) = removal(rows.len(), active);
    if bin_button(ui, theme::ICON_SIZE, BinLook::Quiet, &tip, active.holds > 0) {
        acts.push(act);
    }
    take(window, pick);
    acts
}

/// The closed picker, drawn as every list of the window is: the group's colour and count
/// while it holds supports, the preset's name, and the caret.
fn closed(ui: &egui::Ui, response: &egui::Response, row: &Row, label: &str) {
    let rect = response.rect;
    let visuals = ui.style().interact(response);
    let painter = ui.painter();
    painter.rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let caret = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, rect.center().y),
        vec2(9.0, 6.0),
    );
    painter.add(egui::Shape::convex_polygon(
        vec![caret.left_top(), caret.right_top(), caret.center_bottom()],
        visuals.fg_stroke.color,
        egui::Stroke::NONE,
    ));
    let mut right = caret.left() - 10.0;
    let mut left = rect.left() + 10.0;
    if row.holds > 0 {
        right = count_at(painter, right, rect.center().y, row.holds) - 8.0;
        swatch(
            painter,
            egui::pos2(rect.left() + 14.0, rect.center().y),
            row,
        );
        left = rect.left() + 26.0;
    }
    let clip = egui::Rect::from_x_y_ranges(left..=right, rect.y_range());
    painter.with_clip_rect(clip).text(
        egui::pos2(left, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        theme::label(),
        visuals.text_color(),
    );
}

/// The count ending at `right`; answers where it begins.
fn count_at(painter: &egui::Painter, right: f32, y: f32, count: usize) -> f32 {
    let low = theme::colors().text_low;
    let galley = painter.layout_no_wrap(count.to_string(), theme::figures(11.5), low);
    let left = right - galley.size().x;
    painter.galley(egui::pos2(left, y - galley.size().y / 2.0), galley, low);
    left
}

/// The group's colour, faded while it is hidden.
fn swatch(painter: &egui::Painter, center: egui::Pos2, row: &Row) {
    let tint = theme::support_tint(row.group as usize);
    let tint = if row.hidden {
        tint.gamma_multiply(0.35)
    } else {
        tint
    };
    painter.rect_filled(
        egui::Rect::from_center_size(center, vec2(8.0, 8.0)),
        2.0,
        tint,
    );
}

/// The open picker: the automatic preset with the shapes it built under it, then the
/// presets by hand, each line the group built to it where one stands.
struct List<'a> {
    rows: &'a [Row],
    presets: &'a Presets,
    hand: &'a InHand,
    shaping: bool,
}

impl List<'_> {
    fn show(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
        let auto = self.auto(ui, acts);
        ui.separator();
        let by_hand = self.by_hand(ui, acts);
        auto.or(by_hand)
    }

    fn standing(&self, name: &str) -> Option<&Row> {
        self.rows
            .iter()
            .find(|row| row.holds > 0 && row.name == name)
    }

    /// Auto, and under it its shapes: those that stand, or on Shape every one, since each
    /// is edited there and Auto itself is not.
    fn auto(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
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
    fn by_hand(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Option<Pick> {
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
struct Line<'a> {
    name: &'a str,
    group: Option<&'a Row>,
    /// A figure for a line that is no group: Auto's sum of its shapes.
    count: Option<usize>,
    selected: bool,
    indent: f32,
    /// A heading only names what is under it, and takes no click.
    heading: bool,
}

impl<'a> Line<'a> {
    fn preset(name: &'a str, group: Option<&'a Row>) -> Self {
        Self {
            name,
            group,
            count: None,
            selected: false,
            indent: 0.0,
            heading: false,
        }
    }

    fn heading(name: &'a str, total: usize, heading: bool) -> Self {
        Self {
            count: (total > 0).then_some(total),
            heading,
            ..Self::preset(name, None)
        }
    }

    fn selected(self, selected: bool) -> Self {
        Self { selected, ..self }
    }

    fn indent(self, indent: f32) -> Self {
        Self { indent, ..self }
    }

    /// Draws the line; a group's eye and bin show under the pointer. Answers whether the
    /// line itself was clicked, which closes the list.
    fn show(&self, ui: &mut egui::Ui, groups: usize, acts: &mut Vec<Act>) -> bool {
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

    fn paint(&self, ui: &egui::Ui, rect: egui::Rect, hovered: bool) {
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
            (false, false, true) => colors.accent_soft,
            (false, false, false) => colors.text_high,
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
const ICONS_W: f32 = 44.0;

/// The eye of a group, shown under the pointer or while it is hidden, and its bin, shown
/// under the pointer. Answers whether either took the click.
fn group_icons(
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
fn removal(groups: usize, row: &Row) -> (Act, String) {
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

/// Save: over the automatic preset shown, at once, since none is ever added; for any
/// other, over the one it came from or as a new one.
fn save_button(ui: &mut egui::Ui, window: &mut Window, hand: &InHand) {
    let tip = tr!("shape-save-tooltip");
    let button = ui
        .add_enabled_ui(hand.edited, |ui| icon_button(ui, icon::SAVE, &tip))
        .inner;
    if let Some(id) = &hand.auto_source {
        if button.clicked() {
            save_auto(window, id);
        }
        return;
    }
    let (mut over, mut as_new) = (false, false);
    egui::Popup::menu(&button).show(|ui| {
        let save = egui::Button::new(tr!("shape-save"));
        over = ui.add_enabled(hand.source.is_some(), save).clicked();
        as_new = ui.button(tr!("shape-save-as-new")).clicked();
    });
    if over && let Some(id) = &hand.source {
        save_over(window, id);
    }
    if as_new {
        save_new(window);
    }
}

fn save_auto(window: &mut Window, id: &str) {
    let Some(profile) = window.tools.supports.auto.clone() else {
        return;
    };
    let kept = window
        .machine
        .slicing
        .catalogue
        .save_auto(id, &profile)
        .context(tr!("support-cannot-save"));
    window.machine.status.report(&tr!("shape-saved"), kept);
}

fn save_over(window: &mut Window, id: &str) {
    let profile = window.tools.supports.profile.clone();
    let kept = window
        .machine
        .slicing
        .catalogue
        .save_support(id, &profile)
        .context(tr!("support-cannot-save"));
    if window
        .machine
        .status
        .report(&tr!("shape-saved"), kept)
        .is_some()
    {
        window.tools.supports.saved = Some(profile);
    }
}

/// Saves the preset in hand under a name of its own, and opens it in the settings.
fn save_new(window: &mut Window) {
    let profile = window.tools.supports.profile.clone();
    let kept = keep_support(&mut window.machine.slicing.catalogue, &profile)
        .context(tr!("support-cannot-save"));
    if window
        .machine
        .status
        .report(&tr!("shape-saved"), kept)
        .is_some()
    {
        window
            .machine
            .settings
            .open_supports(&window.machine.slicing.catalogue, &profile);
    }
}

/// Takes up the preset chosen: an automatic one switches the sizing on, one by hand off.
fn take(window: &mut Window, pick: Option<Pick>) {
    let catalogue = &window.machine.slicing.catalogue;
    let tool = &mut window.tools.supports;
    let found = match pick {
        Some(Pick::Auto(id)) => {
            if let Some(entry) = catalogue.autos().find(|entry| entry.id == id) {
                tool.set_automatic(true, catalogue);
                tool.auto = Some(entry.profile.clone());
            }
            Ok(())
        }
        Some(Pick::Preset(id)) => catalogue.support(&id).map(|entry| {
            tool.set_automatic(false, catalogue);
            tool.chosen = Some(entry.profile.clone());
        }),
        Some(Pick::Shape(id)) => catalogue.support(&id).map(|entry| {
            tool.chosen = Some(entry.profile.clone());
        }),
        None => Ok(()),
    };
    if let Err(error) = found {
        window.machine.status = Status::failed(&error.into());
    }
}

/// Does what the clicks on the groups asked. A group chosen brings its preset, and with
/// it the sizing that preset belongs to.
fn apply(window: &mut Window, acts: Vec<Act>) {
    let catalogue = &window.machine.slicing.catalogue;
    let tool = &mut window.tools.supports;
    let scene = &mut window.doc.scene;
    for act in acts {
        match act {
            Act::Choose(group) => {
                tool.choose_group(group);
                let name = &tool.profile.name;
                let found = catalogue
                    .supports()
                    .find(|entry| entry.profile.name == *name);
                if let Some(entry) = found {
                    tool.set_automatic(catalogue.is_auto_shape(&entry.id), catalogue);
                }
            }
            Act::Toggle(group) => {
                let entry = &mut tool.groups[group as usize];
                entry.hidden = !entry.hidden;
            }
            Act::Clear(group) => {
                for object in scene.objects_mut() {
                    object.supports.clear_group(group);
                }
                tool.picked.clear();
            }
            Act::Drop(group) => {
                let name = if group == tool.active {
                    tool.profile.name.clone()
                } else {
                    tool.groups[group as usize].profile.name.clone()
                };
                let gone = tool.remove_group(group, scene);
                window.machine.status =
                    Status::Info(tr!("paint-dropped", name = name, count = gone));
            }
        }
    }
}
