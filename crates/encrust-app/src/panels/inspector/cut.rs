use crate::cut::{Keep, split_parts};
use crate::panels::Window;
use crate::scene::Axis;
use crate::ui::{
    Segment, Segmented, describe, hint, icon, number_row, secondary_button, section, subheading,
};

/// Millimetres per point of drag on the position field.
const OFFSET_STEP: f64 = 0.1;

/// Where the plane cuts the selected model, and what to do with the halves.
///
/// The viewport draws the plane itself; the section slider is a view of its own and is
/// neither moved by the plane nor moves it.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let Some(id) = window.doc.scene.selected() else {
        section(ui, "Cut", None, |ui| {
            hint(ui, "Select a model to cut.");
        });
        return;
    };

    section(ui, "Cut", None, |ui| {
        subheading(ui, "Plane");
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

        ui.add_space(4.0);
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

        subheading(ui, "Halves");
        let keeps: Vec<Segment<'_, Keep>> = Keep::ALL
            .iter()
            .map(|keep| Segment::new(*keep, keep.label()))
            .collect();
        Segmented::new(&keeps)
            .width(width)
            .show(ui, &mut window.tools.cut.state.keep);

        ui.add_space(8.0);
        ui.columns(2, |columns| {
            if secondary_button(&mut columns[0], icon::SECTION, "Cut").clicked() {
                window.machine.status = window.tools.cut.apply(&mut window.doc.scene, id);
            }
            if secondary_button(&mut columns[1], icon::SPLIT, "Split").clicked() {
                window.machine.status = split_parts(&mut window.doc.scene, id);
            }
        });
    });
}
