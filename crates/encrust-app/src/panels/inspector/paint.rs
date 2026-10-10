use core_geometry::Scalar;

use crate::panels::Window;
use crate::status::Status;
use crate::supports::Placing;
use crate::ui::{
    Segment, Segmented, hint, icon, list, list_row, nested, number_row, primary_button,
    secondary_button, section, switch, theme,
};

/// Millimetres per point of drag on the brush and the fill.
const COARSE_STEP: f64 = 0.05;
const ANGLE_STEP: f64 = 0.5;

/// Bounds on the brush and on the fill, millimetres of plate. A brush finer than a tenth
/// of a millimetre paints one triangle at a time; one wider than the plate paints all of
/// it.
const MIN_BRUSH_MM: f32 = 0.1;
const MAX_BRUSH_MM: f32 = 25.0;
const MIN_SPACING_MM: f32 = 0.5;
const MAX_SPACING_MM: f32 = 20.0;
const DEFAULT_SPACING_MM: f32 = 3.0;

/// Bounds on how far a painted surface may turn from the face that was clicked.
const MIN_FLOOD_DEG: f32 = 1.0;
const MAX_FLOOD_DEG: f32 = 180.0;

/// What each of the placing modes does, in the order they are drawn. Short, because it
/// stays on screen under the pill for as long as the tool is in hand.
const PAINT_HINT: [&str; 3] = [
    "Click to place, alt-click to remove.",
    "Drag to paint, ctrl-click a surface, alt to erase.",
    "Drag to block, ctrl-click a surface, alt to erase.",
];

/// Supports by hand: placed one at a time, or filled into a painted patch, in the group
/// the next ones join.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Brush", None, |ui| painting(ui, window));
    section(ui, "Fill", None, |ui| {
        // How dense a fill comes out is worth reading whichever brush is in hand.
        spacing_row(ui, "Rim", &mut window.tools.supports.fill.border_spacing_mm);
        spacing_row(
            ui,
            "Inside",
            &mut window.tools.supports.fill.infill_spacing_mm,
        );
        clear(ui, window);
    });
    section(ui, "Groups", None, |ui| groups(ui, window));
}

/// Filling what is painted with supports.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let painted = window
        .doc
        .scene
        .targets()
        .any(|object| !object.supports.painted().is_empty());
    if primary_button(ui, icon::SUPPORTS, "Fill painted areas", painted).clicked() {
        let added = window.tools.supports.fill(&mut window.doc.scene);
        window.machine.status = Status::Info(match added {
            0 => "The painted areas had nowhere to put a support".to_owned(),
            1 => "Filled the painted areas with 1 support".to_owned(),
            added => format!("Filled the painted areas with {added} supports"),
        });
    }
}

/// Taking the paint, or the blocking, off again.
fn clear(ui: &mut egui::Ui, window: &mut Window) {
    let blocking = window.tools.supports.placing.blocks();
    let label = if blocking {
        "Clear blocked areas"
    } else {
        "Clear painted areas"
    };
    if secondary_button(ui, icon::REMOVE, label).clicked() {
        for object in window.doc.scene.targets_mut() {
            object.supports.clear_paint(blocking);
        }
    }
}

/// Whether a click places a support or a stroke paints a patch, and how wide the brush is.
fn painting(ui: &mut egui::Ui, window: &mut Window) {
    let segments = Placing::ALL.map(|placing| Segment::new(placing, placing.label()));
    let width = ui.available_width();
    let mut chosen = window.tools.supports.placing;
    if Segmented::new(&segments).width(width).show(ui, &mut chosen) {
        window.tools.supports.placing = chosen;
    }

    ui.add_space(4.0);
    hint(ui, PAINT_HINT[window.tools.supports.placing as usize]);

    if window.tools.supports.placing.paints() {
        ui.add_space(4.0);
        number_row(
            ui,
            "Brush",
            &mut window.tools.supports.brush_radius_mm,
            "mm",
            COARSE_STEP,
            MIN_BRUSH_MM..=MAX_BRUSH_MM,
            2,
        );
        number_row(
            ui,
            "Surface angle",
            &mut window.tools.supports.flood_angle_deg,
            "deg",
            ANGLE_STEP,
            MIN_FLOOD_DEG..=MAX_FLOOD_DEG,
            0,
        );
    }
}

/// One spacing of the fill, with the switch that leaves that half of it out.
fn spacing_row(ui: &mut egui::Ui, label: &str, spacing_mm: &mut Option<Scalar>) {
    let mut on = spacing_mm.is_some();
    if switch(ui, &mut on, label).changed() {
        *spacing_mm = on.then_some(DEFAULT_SPACING_MM);
    }
    if let Some(value) = spacing_mm.as_mut() {
        nested(ui, |ui| {
            number_row(
                ui,
                "Spacing",
                value,
                "mm",
                COARSE_STEP,
                MIN_SPACING_MM..=MAX_SPACING_MM,
                2,
            );
        });
    }
}

/// Which group the fields below belong to, and the two ways the list of them changes.
///
/// A group is a set of supports built to one profile: every field of this panel edits the
/// group in hand, and a support is retuned by being moved to another one.
fn groups(ui: &mut egui::Ui, window: &mut Window) {
    let active = window.tools.supports.active;
    let mut chosen = active;
    let mut drop = None;
    let mut renamed = None;

    let names: Vec<String> = window
        .tools
        .supports
        .groups
        .iter()
        .map(|entry| entry.name.clone())
        .collect();
    list(ui, |ui| {
        for (group, name) in names.iter().enumerate() {
            let group = group as u16;
            // The first group is where every support goes by default, so it cannot go.
            let remove = (group != 0).then_some((icon::CANCEL, "Drop this group"));
            let tint = Some(theme::support_tint(group as usize));
            let rename = Some(egui::Id::new(("support-group-name", group)));
            let action = list_row(ui, name, tint, group == active, remove, rename);
            if action.picked {
                chosen = group;
            }
            if let Some(name) = action.renamed {
                renamed = Some((group, name));
            }
            if action.removed {
                drop = Some(group);
            }
        }
    });

    if secondary_button(ui, icon::ADD, "New group").clicked() {
        window.tools.supports.add_group();
    }
    if let Some((group, name)) = renamed {
        window.tools.supports.groups[group as usize].name = name;
    }
    window.tools.supports.choose_group(chosen);
    if let Some(group) = drop {
        let holds = window.tools.supports.group_holds(group, &window.doc.scene);
        if holds == 0 {
            window
                .tools
                .supports
                .remove_group(group, &mut window.doc.scene);
        } else {
            window.tools.supports.dropping = Some((group, holds));
        }
    }
    dropping(ui, window);
}

/// The question a group holding supports is taken away behind: they go with it, and only
/// `Cmd+Z` brings them back.
fn dropping(ui: &mut egui::Ui, window: &mut Window) {
    let Some((group, holds)) = window.tools.supports.dropping else {
        return;
    };
    // An undo under the open question can take the group away itself.
    let Some(entry) = window.tools.supports.groups.get(group as usize) else {
        window.tools.supports.dropping = None;
        return;
    };
    let name = entry.name.clone();
    egui::Modal::new(egui::Id::new("drop-support-group")).show(ui.ctx(), |ui| {
        ui.label(match holds {
            1 => format!("{name} holds 1 support, which goes with it."),
            holds => format!("{name} holds {holds} supports, which go with it."),
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Drop the group").clicked() {
                let gone = window
                    .tools
                    .supports
                    .remove_group(group, &mut window.doc.scene);
                window.machine.status = Status::Info(match gone {
                    1 => format!("Dropped {name} and 1 support"),
                    gone => format!("Dropped {name} and {gone} supports"),
                });
                window.tools.supports.dropping = None;
            }
            if ui.button("Keep it").clicked() {
                window.tools.supports.dropping = None;
            }
        });
    });
}
