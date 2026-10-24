use crate::i18n::tr;
use crate::panels::Window;
use crate::text::{TextState, TextTool, reason};
use crate::ui::{Segment, Segmented, busy, describe, hint, number_row, section, theme, tone};

/// Bounds on the letters, millimetres: under a millimetre a capital is finer than resin
/// holds, past a hundred one letter is the model.
const MIN_HEIGHT_MM: f32 = 1.0;
const MAX_HEIGHT_MM: f32 = 100.0;
/// Past ten millimetres a label is a part of its own rather than lettering on one.
const MIN_DEPTH_MM: f32 = 0.1;
const MAX_DEPTH_MM: f32 = 10.0;

/// How tall the list of families stands, points: some ten names.
const LIST_H: f32 = 280.0;
/// The room the family field keeps on its right for the caret, and the caret's size, points.
const CARET_W: f32 = 28.0;
const CARET_GLYPH: f32 = 14.0;

/// Per point of drag.
const SIZE_STEP: f64 = 0.1;
const DEPTH_STEP: f64 = 0.05;

/// What a click on a model writes, in what, and how big.
pub fn sections(ui: &mut egui::Ui, window: &mut Window) {
    let tool = &mut window.tools.text;
    section(ui, &tr!("text-words"), None, |ui| {
        let field = crate::ui::text_box(egui::TextEdit::multiline(&mut tool.state.text))
            .hint_text(tr!("text-words-hint"))
            .font(theme::label())
            .desired_rows(2)
            .desired_width(ui.available_width());
        ui.add(field);
        family(ui, tool);
    });
    section(ui, &tr!("text-lettering"), None, |ui| {
        shape(ui, &mut tool.state);
        let note = if tool.state.follow_surface {
            tr!("text-follow-note")
        } else {
            tr!("text-shape-note")
        };
        describe(ui, &note);
    });
}

/// Where the next click goes, why it cannot, or the stamp that is running.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.text.is_busy() {
        if busy(ui, &tr!("text-stamping"), None, true) {
            window.tools.text.cancel(&mut window.machine.status);
        }
        return;
    }
    let tool = &window.tools.text;
    match tool.shapes() {
        Ok(_) => tone(ui, &tr!("text-click-note"), theme::colors().text_mid),
        Err(error) => hint(ui, &reason(&error)),
    }
}

/// The family to write in, filtered as a name is typed. The machine's faces are read on
/// first focus: reading every installed face takes a while.
fn family(ui: &mut egui::Ui, tool: &mut TextTool) {
    let id = ui.make_persistent_id("text-family");
    let list = id.with("list");
    let ctx = ui.ctx().clone();
    let open = egui::Popup::is_id_open(&ctx, list);
    let current = tool
        .family()
        .map(|family| family.name.clone())
        .unwrap_or_default();
    let mut query: String = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    let mut shown = if open { query.clone() } else { current.clone() };
    let response = family_field(ui, &mut shown, current);
    if response.gained_focus() || (response.clicked() && !open) {
        tool.families.ask();
        query.clear();
        egui::Popup::open_id(&ctx, list);
    } else if open && response.changed() {
        query = shown;
    }
    let first = matching(tool, &query).next();
    if open && response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
        if let Some(index) = first {
            tool.state.family = tool.families.list[index].name.clone();
        }
        egui::Popup::close_id(&ctx, list);
    }
    let shown = egui::Popup::new(list, ctx.clone(), &response, ui.layer_id())
        .open_memory(None)
        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
        .width(response.rect.width())
        .show(|ui| choices(ui, tool, &query));
    if let Some(shown) = shown {
        let picked = shown.inner;
        if picked || (shown.response.clicked_elsewhere() && response.clicked_elsewhere()) {
            egui::Popup::close_id(&ctx, list);
        }
    }
    ui.data_mut(|data| data.insert_temp(id, query));
}

/// The field the family is typed into, with the caret that says a list drops from it.
fn family_field(ui: &mut egui::Ui, shown: &mut String, current: String) -> egui::Response {
    let field = crate::ui::text_box(egui::TextEdit::singleline(shown))
        .hint_text(current)
        .font(theme::label())
        .margin(egui::Margin {
            left: 10,
            right: CARET_W as i8,
            ..egui::Margin::ZERO
        })
        .desired_width(ui.available_width());
    let response = ui.add(field);
    let caret = response.rect.right_center() - egui::vec2(CARET_W / 2.0, 0.0);
    ui.painter().text(
        caret,
        egui::Align2::CENTER_CENTER,
        crate::ui::icon::CARET_DOWN,
        theme::icon(CARET_GLYPH),
        theme::colors().text_mid,
    );
    response
}

/// The families whose name holds `query`, any case, by their place in the list.
fn matching<'a>(tool: &'a TextTool, query: &str) -> impl Iterator<Item = usize> + 'a {
    let query = query.trim().to_lowercase();
    tool.families
        .list
        .iter()
        .enumerate()
        .filter(move |(_, family)| family.name.to_lowercase().contains(&query))
        .map(|(index, _)| index)
}

/// The list under the family field, its full height from the first frame however few
/// families have arrived; only a typed name shrinks it. Answers whether one was picked.
fn choices(ui: &mut egui::Ui, tool: &mut TextTool, query: &str) -> bool {
    let found: Vec<usize> = matching(tool, query).collect();
    let searched = !query.trim().is_empty() && !tool.families.is_reading();
    let mut picked = false;
    egui::ScrollArea::vertical()
        .max_height(LIST_H)
        .min_scrolled_height(LIST_H)
        .auto_shrink([false, searched])
        .show(ui, |ui| {
            for index in found.iter().copied() {
                let name = &tool.families.list[index].name;
                if ui
                    .selectable_label(*name == tool.state.family, name)
                    .clicked()
                {
                    tool.state.family = name.clone();
                    picked = true;
                }
            }
            if found.is_empty() && !tool.families.is_reading() {
                tone(ui, &tr!("text-no-family"), theme::colors().text_low);
            }
        });
    if tool.families.is_reading() {
        tone(ui, &tr!("text-reading-fonts"), theme::colors().text_low);
    }
    picked
}

fn shape(ui: &mut egui::Ui, state: &mut TextState) {
    let segments = [
        Segment::new(true, tr!("text-raised")),
        Segment::new(false, tr!("text-sunk")),
    ];
    let mut raised = state.raised;
    if Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, &mut raised)
    {
        state.raised = raised;
    }
    let segments = [
        Segment::new(false, tr!("text-flat")),
        Segment::new(true, tr!("text-follow")),
    ];
    let mut follow = state.follow_surface;
    if Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, &mut follow)
    {
        state.follow_surface = follow;
    }
    let mm = tr!("unit-mm");
    number_row(
        ui,
        &tr!("text-height"),
        &mut state.height_mm,
        &mm,
        SIZE_STEP,
        MIN_HEIGHT_MM..=MAX_HEIGHT_MM,
        1,
    );
    number_row(
        ui,
        &tr!("text-depth"),
        &mut state.depth_mm,
        &mm,
        DEPTH_STEP,
        MIN_DEPTH_MM..=MAX_DEPTH_MM,
        2,
    );
    number_row(
        ui,
        &tr!("text-angle"),
        &mut state.angle_deg,
        &tr!("unit-degrees"),
        1.0,
        0.0..=360.0,
        0,
    );
}
