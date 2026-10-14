use crate::cut::{CutTool, Keep, PIN_CLEARANCE_MM, Pins, split_parts};
use crate::panels::Window;
use crate::scene::Axis;
use crate::ui::{
    Segment, Segmented, count_row, describe, hint, icon, nested, number_row, primary_button,
    secondary_button, section, switch,
};

/// The most pins one cut takes.
const MAX_PINS: u32 = 6;

/// Millimetres per point of drag on the position field.
const OFFSET_STEP: f64 = 0.1;

/// Where the plane cuts the selected model, and what to do with the halves.
///
/// The viewport draws the plane itself; the section slider is a view of its own and is
/// neither moved by the plane nor moves it.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let Some(id) = window.doc.scene.selected() else {
        section(ui, "Plane", None, |ui| {
            hint(ui, "Select a model to cut.");
        });
        return;
    };

    if let Some(object) = window.doc.scene.get(id) {
        window.tools.cut.centre_on(id, object);
    }

    section(ui, "Plane", None, |ui| {
        let axes: Vec<Segment<'_, Axis>> = Axis::ALL
            .iter()
            .map(|axis| Segment::new(*axis, axis.label()))
            .collect();
        let width = ui.available_width();
        let mut axis = window.tools.cut.state.axis;
        Segmented::new(&axes).width(width).show(ui, &mut axis);
        if axis != window.tools.cut.state.axis
            && let Some(object) = window.doc.scene.get(id)
        {
            window.tools.cut.set_axis(axis, object);
        }

        let plate = &window.doc.plate;
        let across = plate.x_mm.max(plate.y_mm);
        let (label, range) = match window.tools.cut.state.axis {
            Axis::Z => ("Height", 0.0..=plate.z_mm),
            Axis::X | Axis::Y => ("Offset", -across..=across),
        };
        number_row(
            ui,
            label,
            &mut window.tools.cut.state.height_mm,
            "mm",
            OFFSET_STEP,
            range,
            2,
        );
        let from = match window.tools.cut.state.axis {
            Axis::Z => "Measured from the plate.",
            Axis::X | Axis::Y => "Measured from the model's centre of mass.",
        };
        describe(ui, from);
    });

    section(ui, "Afterwards", None, |ui| {
        let keeps: Vec<Segment<'_, Keep>> = Keep::ALL
            .iter()
            .map(|keep| Segment::new(*keep, keep.label()))
            .collect();
        let width = ui.available_width();
        Segmented::new(&keeps)
            .width(width)
            .show(ui, &mut window.tools.cut.state.keep);
        switch(ui, &mut window.tools.cut.lay_flat, "Lay the halves flat");
        describe(ui, "Each half stands on the face the cut left.");
        pins(ui, &mut window.tools.cut);
    });
}

/// Pins on the lower half and sockets in the upper, and how many and how big. A half kept
/// alone has nothing to be pinned to.
fn pins(ui: &mut egui::Ui, tool: &mut CutTool) {
    let both = tool.state.keep == Keep::Both;
    let mut on = tool.pins.is_some();
    let flipped = ui
        .add_enabled_ui(both, |ui| switch(ui, &mut on, "Alignment pins").changed())
        .inner;
    if flipped {
        tool.pins = on.then(Pins::default);
    }
    let Some(pins) = tool.pins.as_mut().filter(|_| both) else {
        describe(ui, "Pegs and sockets so the halves glue back true.");
        return;
    };
    nested(ui, |ui| {
        let mut count = pins.count as u32;
        if count_row(ui, "Pins", &mut count, "", 0.1, 1..=MAX_PINS) {
            pins.count = count as usize;
        }
        number_row(
            ui,
            "Diameter",
            &mut pins.diameter_mm,
            "mm",
            OFFSET_STEP,
            1.0..=20.0,
            1,
        );
        number_row(
            ui,
            "Length",
            &mut pins.length_mm,
            "mm",
            OFFSET_STEP,
            1.0..=30.0,
            1,
        );
    });
    describe(
        ui,
        &format!(
            "Pins stand on the lower half; the sockets are {PIN_CLEARANCE_MM:.1} mm wider \
             each side and show as holes in the Drain tool."
        ),
    );
}

/// Cutting the selected model along the plane, or splitting it into the parts it is
/// already made of.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let picked = window.doc.scene.selected();
    let what = "Two models replace one. Undo puts it back.";
    if primary_button(ui, icon::SECTION, "Cut", picked.is_some())
        .on_hover_text(what)
        .clicked()
        && let Some(id) = picked
    {
        window.machine.status = window.tools.cut.apply(&mut window.doc.scene, id);
    }
    let split = ui
        .add_enabled_ui(picked.is_some(), |ui| {
            secondary_button(ui, icon::SPLIT, "Split into its parts").on_hover_text(what)
        })
        .inner;
    if split.clicked()
        && let Some(id) = picked
    {
        window.machine.status = split_parts(&mut window.doc.scene, id);
    }
}
