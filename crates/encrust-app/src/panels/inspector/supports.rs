use anyhow::Context as _;
use core_geometry::Scalar;

use crate::job::SupportJob;
use crate::panels::Window;
use crate::panels::support_fields::Group;
use crate::settings::keep_support;
use crate::state::Doc;
use crate::status::Status;
use crate::supports::{Editing, Parameters, Placing};
use crate::ui::{
    Segment, Segmented, describe, fold, hint, icon, icon_button, list, list_row, nested,
    number_row, primary_button, secondary_button, section, stats, subheading, switch, theme, tone,
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

/// What each of the placing modes does, in the order they are drawn.
const PAINT_HINT: [&str; 4] = [
    "Click the model to stand a support under it, alt-click one to take it away.",
    "Drag to paint what needs holding up, ctrl-click for the whole surface, alt to erase.",
    "Drag to paint where no support may go, ctrl-click for the whole surface, alt to erase.",
    "Click a part to pick it, drag a picked part to carry it, shift adds, alt removes.",
];

/// What a support is made of, and what the plate has on it.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let profile_name = window.tools.supports.profile.name.clone();
    section(ui, "Supports", Some(&profile_name), |ui| {
        painting(ui, window);

        subheading(ui, "Group");
        groups(ui, window);

        subheading(ui, "Profile");
        profile_picker(ui, window);
        ui.add_space(4.0);
        for group in Group::ALL {
            fold(ui, group.label(), |ui| {
                group.show(ui, &mut window.tools.supports.profile);
            });
        }

        subheading(ui, "Off the plate");
        lift(ui, window);

        subheading(ui, "Automatic");
        match window.tools.supports.job.as_ref() {
            Some(job) => running(ui, job),
            None => generate(ui, window),
        }

        subheading(ui, "On the plate");
        placed(ui, window.doc);
    });
}

/// Which saved profile the group in hand is built to, and the way to the screen where
/// profiles are made, edited and thrown away.
fn profile_picker(ui: &mut egui::Ui, window: &mut Window) {
    let catalogue = &window.machine.slicing.catalogue;
    let current = &window.tools.supports.profile;
    let matching = catalogue
        .supports()
        .find(|entry| entry.profile == *current)
        .map(|entry| entry.id.clone());
    let shown = match &matching {
        Some(_) => current.name.clone(),
        None => format!("{} (edited)", current.name),
    };

    let mut picked = None;
    let mut manage = false;
    let mut drawing = false;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("support-profile")
            .width((ui.available_width() - 2.0 * (theme::ICON_SIZE + theme::ITEM_GAP)).max(80.0))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for entry in catalogue.supports() {
                    let chosen = matching.as_deref() == Some(entry.id.as_str());
                    if ui.selectable_label(chosen, &entry.profile.name).clicked() {
                        picked = Some(entry.profile.clone());
                    }
                }
            });
        if icon_button(ui, icon::PARAMETERS, "Set the measurements on a drawing").clicked() {
            drawing = true;
        }
        manage = icon_button(ui, icon::SETTINGS, "Edit support profiles").clicked();
    });
    if drawing {
        window.tools.supports.parameters = Some(Parameters::new(Editing::Tool));
    }

    if let Some(profile) = picked {
        window.tools.supports.profile = profile;
    }
    if matching.is_none() {
        ui.add_space(4.0);
        if secondary_button(ui, icon::SAVE, "Save as a profile").clicked() {
            let edited = window.tools.supports.profile.clone();
            let kept = keep_support(&mut window.machine.slicing.catalogue, &edited)
                .context("cannot save the support profile");
            if window
                .machine
                .status
                .report("Saved the support profile", kept)
                .is_some()
            {
                window
                    .machine
                    .settings
                    .open_supports(&window.machine.slicing.catalogue, &edited);
            }
        }
    } else if manage {
        let profile = window.tools.supports.profile.clone();
        window
            .machine
            .settings
            .open_supports(&window.machine.slicing.catalogue, &profile);
    }
}

/// Standing every part clear of the plate, by the lift the profile carries.
fn lift(ui: &mut egui::Ui, window: &mut Window) {
    let label = format!(
        "Lift models {:.1} mm",
        window.tools.supports.profile.z_lift_mm
    );
    if secondary_button(ui, icon::SUPPORTS, &label).clicked() {
        let moved = window.tools.supports.lift(&mut window.doc.scene);
        window.machine.status = Status::Info(match moved {
            0 => "Nothing on the plate to lift".to_owned(),
            1 => "Lifted 1 model off the plate".to_owned(),
            moved => format!("Lifted {moved} models off the plate"),
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

    ui.add_space(4.0);
    if secondary_button(ui, icon::ADD, "New group").clicked() {
        window.tools.supports.add_group();
    }
    if let Some((group, name)) = renamed {
        window.tools.supports.groups[group as usize].name = name;
    }
    window.tools.supports.choose_group(chosen);
    if let Some(group) = drop {
        window
            .tools
            .supports
            .remove_group(group, &mut window.doc.scene);
    }
}

/// What the Edit mode is holding: how much of it, which groups it belongs to, and the
/// three things that can be done to it without the mouse.
fn editing(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.supports.picked.is_empty() {
        describe(
            ui,
            "Click a tip, a strut, a joint, the trunk or the foot to pick it.",
        );
        return;
    }

    let held = window.tools.supports.picked.clone();
    let supports = held
        .iter()
        .map(|picked| (picked.id, picked.frozen))
        .collect::<std::collections::BTreeSet<_>>();
    stats(
        ui,
        &[
            ("Parts", held.len().to_string()),
            ("Supports", supports.len().to_string()),
        ],
    );

    ui.add_space(4.0);
    let active = window.tools.supports.active;
    if primary_button(ui, icon::SUPPORTS, "Move to the group in hand", true).clicked() {
        for (id, frozen) in &supports {
            if let Some(object) = window.doc.scene.get_mut(*id)
                && let Some(tree) = object.supports.frozen_mut(*frozen)
            {
                tree.set_group(active);
            }
        }
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::SUPPORTS, "Grow these again").clicked() {
        // Highest first: thawing renumbers everything above what was taken out.
        for (id, frozen) in supports.iter().rev() {
            if let Some(object) = window.doc.scene.get_mut(*id) {
                let transform = object.transform;
                object.supports.thaw(*frozen, transform);
            }
        }
        window.tools.supports.picked.clear();
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::REMOVE, "Remove these supports").clicked() {
        for (id, frozen) in supports.iter().rev() {
            if let Some(object) = window.doc.scene.get_mut(*id) {
                object.supports.remove_frozen(*frozen);
            }
        }
        window.tools.supports.picked.clear();
    }
}

/// Painting a patch of the model, and filling it with supports or keeping them off it.
fn painting(ui: &mut egui::Ui, window: &mut Window) {
    let segments = Placing::ALL.map(|placing| Segment::new(placing, placing.label()));
    let width = ui.available_width();
    let mut chosen = window.tools.supports.placing;
    if Segmented::new(&segments).width(width).show(ui, &mut chosen) {
        window.tools.supports.placing = chosen;
    }

    ui.add_space(4.0);
    describe(ui, PAINT_HINT[window.tools.supports.placing as usize]);

    if window.tools.supports.placing.edits() {
        ui.add_space(4.0);
        editing(ui, window);
        return;
    }
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

    // How dense a fill comes out is worth reading whichever brush is in hand, so the two
    // spacings and the button stay put when the pill moves.
    subheading(ui, "Fill");
    spacing_row(ui, "Rim", &mut window.tools.supports.fill.border_spacing_mm);
    spacing_row(
        ui,
        "Inside",
        &mut window.tools.supports.fill.infill_spacing_mm,
    );

    ui.add_space(6.0);
    fill_buttons(ui, window);
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

/// Filling what is painted, and taking the paint off again.
fn fill_buttons(ui: &mut egui::Ui, window: &mut Window) {
    let blocking = window.tools.supports.placing.blocks();
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

    ui.add_space(4.0);

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

fn generate(ui: &mut egui::Ui, window: &mut Window) {
    let blocker = window.tools.supports.blocker(&window.doc.scene);
    if primary_button(ui, icon::SUPPORTS, "Generate supports", blocker.is_none()).clicked() {
        let started = window
            .tools
            .supports
            .start(&window.doc.scene, window.machine.slicing.layer_height_mm());
        window.machine.status.report("Placing supports", started);
    }

    ui.add_space(4.0);
    let scope = format!(
        "Cuts {} to find what hangs over nothing.",
        window.doc.scene.scope()
    );
    let text = blocker.unwrap_or(&scope);
    ui.vertical_centered(|ui| tone(ui, text, theme::colors().text_low));
}

/// The progress bar and the cancel button of the run that is going.
fn running(ui: &mut egui::Ui, job: &SupportJob) {
    tone(ui, &job.label(), theme::colors().text_mid);
    ui.add(
        egui::ProgressBar::new(job.fraction())
            .desired_height(6.0)
            .corner_radius(theme::R_CONTROL),
    );

    ui.add_space(4.0);
    ui.add_enabled_ui(!job.is_cancelling(), |ui| {
        if secondary_button(ui, icon::CANCEL, "Cancel").clicked() {
            job.cancel();
        }
    });
}

/// How many supports are on the plate, and the one way to take them all off again.
fn placed(ui: &mut egui::Ui, doc: &mut Doc) {
    let points: usize = doc
        .scene
        .targets()
        .map(|object| object.supports.point_count())
        .sum();
    let columns: usize = doc
        .scene
        .targets()
        .map(|object| object.supports.standing())
        .sum();

    stats(
        ui,
        &[
            ("Supports", points.to_string()),
            ("Standing", columns.to_string()),
        ],
    );

    if points == 0 {
        return;
    }
    if points != columns {
        ui.add_space(4.0);
        hint(
            ui,
            "Some supports have no room to stand and are not printed.",
        );
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::REMOVE, "Remove every support").clicked() {
        for object in doc.scene.targets_mut() {
            object.supports.clear();
        }
    }
    let _ = icon_button;
}
