use crate::cut::{Keep, split_parts};
use crate::panels::Window;
use crate::scene::Axis;
use crate::ui::{
    Segment, Segmented, describe, hint, icon, later, number_row, primary_button, secondary_button,
    section, switch,
};

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
        // TODO(step-8): lay each half on its cut face, and pegs and sockets so the halves
        // glue back true.
        later(ui, |ui| {
            let mut flat = false;
            switch(ui, &mut flat, "Lay the halves flat");
            let mut pins = false;
            switch(ui, &mut pins, "Alignment pins");
        });
    });
}

/// Cutting the selected model along the plane, or splitting it into the parts it is
/// already made of.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let picked = window.doc.scene.selected();
    if primary_button(ui, icon::SECTION, "Cut", picked.is_some()).clicked()
        && let Some(id) = picked
    {
        window.machine.status = window.tools.cut.apply(&mut window.doc.scene, id);
    }
    let split = ui
        .add_enabled_ui(picked.is_some(), |ui| {
            secondary_button(ui, icon::SPLIT, "Split into its parts")
        })
        .inner;
    if split.clicked()
        && let Some(id) = picked
    {
        window.machine.status = split_parts(&mut window.doc.scene, id);
    }
    hint(ui, "Two models replace one. Undo puts it back.");
}
