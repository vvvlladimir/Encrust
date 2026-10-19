use crate::i18n::tr;
use crate::panels::Window;
use crate::text::{TextState, TextTool, reason};
use crate::ui::{
    Segment, Segmented, describe, hint, number_row, progress_bar, section, theme, tone,
};

/// Bounds on the letters, millimetres: under a millimetre a capital is finer than resin
/// holds, past a hundred one letter is the model.
const MIN_HEIGHT_MM: f32 = 1.0;
const MAX_HEIGHT_MM: f32 = 100.0;
/// Past ten millimetres a label is a part of its own rather than lettering on one.
const MIN_DEPTH_MM: f32 = 0.1;
const MAX_DEPTH_MM: f32 = 10.0;

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
    let tool = &window.tools.text;
    if tool.is_busy() {
        tone(ui, &tr!("text-stamping"), theme::colors().text_mid);
        progress_bar(ui, None);
        return;
    }
    match tool.shapes() {
        Ok(_) => tone(ui, &tr!("text-click-note"), theme::colors().text_mid),
        Err(error) => hint(ui, &reason(&error)),
    }
}

/// The family to write in. The machine's own are read the first time the list is opened,
/// not before: reading every installed face takes a while.
fn family(ui: &mut egui::Ui, tool: &mut TextTool) {
    let shown = tool
        .families
        .list
        .get(tool.state.family)
        .map(|family| family.name.clone())
        .unwrap_or_default();
    egui::ComboBox::from_id_salt("text-family")
        .width(ui.available_width())
        .selected_text(shown)
        .show_ui(ui, |ui| {
            tool.families.ask();
            for (index, family) in tool.families.list.iter().enumerate() {
                if ui
                    .selectable_label(index == tool.state.family, &family.name)
                    .clicked()
                {
                    tool.state.family = index;
                }
            }
            if tool.families.is_reading() {
                tone(ui, &tr!("text-reading-fonts"), theme::colors().text_low);
            }
        });
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
