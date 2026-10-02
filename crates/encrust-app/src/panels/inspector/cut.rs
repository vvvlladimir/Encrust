use core_geometry::Scalar;

use crate::cut::{Keep, split_parts};
use crate::panels::Window;
use crate::panels::section;
use crate::scene::Axis;
use crate::ui::{
    Segment, Segmented, describe, hint, icon, number_row, secondary_button, section, subheading,
};

/// Millimetres per point of drag on the height field.
const HEIGHT_STEP: f64 = 0.1;

/// Where the plane cuts the selected model, and what to do with the halves.
///
/// A cut across Z is the viewport's own section, one plane moved from either side: the
/// slider sets the field and the field sets the slider. The other axes have no preview.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    // What the field last put on the slider: anything else there is the slider moved.
    let synced = egui::Id::new("cut-synced-height");
    if window.tools.cut.axis == Axis::Z {
        let written = ui.data(|data| data.get_temp::<Option<Scalar>>(synced));
        if let Some(written) = written
            && written != window.view.section.height_mm
        {
            let top = section::height_range(window.doc).map(|(_, top)| top);
            if let Some(height_mm) = window.view.section.height_mm.or(top) {
                window.tools.cut.height_mm = height_mm;
            }
        }
    }

    let Some(id) = window.doc.scene.selected() else {
        section(ui, "Cut", None, |ui| {
            hint(ui, "Select a model to cut.");
        });
        onto_slider(ui, window, synced);
        return;
    };

    section(ui, "Cut", None, |ui| {
        subheading(ui, "Plane");
        let axes: Vec<Segment<'_, Axis>> = Axis::ALL
            .iter()
            .map(|axis| Segment::new(*axis, axis.label()))
            .collect();
        let width = ui.available_width();
        Segmented::new(&axes)
            .width(width)
            .show(ui, &mut window.tools.cut.axis);

        ui.add_space(4.0);
        let range = 0.0..=window
            .doc
            .plate
            .z_mm
            .max(window.doc.plate.x_mm)
            .max(window.doc.plate.y_mm);
        number_row(
            ui,
            "Height",
            &mut window.tools.cut.height_mm,
            "mm",
            HEIGHT_STEP,
            range,
            2,
        );

        subheading(ui, "Halves");
        let keeps: Vec<Segment<'_, Keep>> = Keep::ALL
            .iter()
            .map(|keep| Segment::new(*keep, keep.label()))
            .collect();
        Segmented::new(&keeps)
            .width(width)
            .show(ui, &mut window.tools.cut.keep);

        if window.tools.cut.axis != Axis::Z {
            ui.add_space(4.0);
            describe(ui, "The viewport previews a cut across Z only.");
        }

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
    onto_slider(ui, window, synced);
}

/// Puts the plane on the section slider, and remembers that it was the field that did.
fn onto_slider(ui: &egui::Ui, window: &mut Window, synced: egui::Id) {
    if window.tools.cut.axis == Axis::Z {
        window.view.section.height_mm = Some(window.tools.cut.height_mm);
        ui.data_mut(|data| data.insert_temp(synced, window.view.section.height_mm));
    }
}
