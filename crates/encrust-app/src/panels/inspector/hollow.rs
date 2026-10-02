use core_volume::{HollowMode, InfillPattern, MIN_WALL_MM};

use crate::job::HollowJob;
use crate::panels::Window;
use crate::state::{Doc, Tools};
use crate::ui::{
    Segment, Segmented, describe, hint, icon, nested, number_row, primary_button, secondary_button,
    section, stats, subheading, switch, theme, tone,
};

/// Per point of drag. A wall is measured in whole millimetres, precision and density in
/// hundredths of their own range.
const COARSE_STEP: f64 = 0.05;
const FINE_STEP: f64 = 0.01;

/// Past fifteen millimetres there is nothing left to hollow on anything that fits the
/// machine. The floor is `core_volume::MIN_WALL_MM`, which is what the lattice can resolve.
const MAX_WALL_MM: f32 = 15.0;

/// Bounds on one infill cell. Under a millimetre no resin drains out of it, and past
/// forty there is one cell in the model.
const MIN_CELL_MM: f32 = 1.0;
const MAX_CELL_MM: f32 = 40.0;

/// Bounds on the density. Under a twentieth the lattice holds nothing up, and past half
/// it is a solid with a texture rather than an infill.
const MIN_DENSITY: f32 = 0.05;
const MAX_DENSITY: f32 = 0.5;

/// The cavity inside the models on the plate, and what stands in it.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Hollow", None, |ui| {
        describe(
            ui,
            "Click the model to keep its wall solid there, alt-click a blocker to take it away.",
        );
        ui.add_space(4.0);

        modes(ui, window.tools);
        ui.add_space(4.0);
        wall(ui, window);

        subheading(ui, "Infill");
        infill(ui, window.tools);

        subheading(ui, "Blockers");
        blockers(ui, window);

        ui.add_space(8.0);
        action(ui, window);

        subheading(ui, "On the plate");
        done(ui, window);
    });
}

/// Which surface the wall is measured from.
fn modes(ui: &mut egui::Ui, tools: &mut Tools) {
    let segments = [
        Segment::new(HollowMode::Internal, "Inside"),
        Segment::new(HollowMode::External, "Outside"),
        Segment::new(HollowMode::BottomThrough, "Through"),
    ];
    let width = ui.available_width();
    let mut chosen = tools.hollow.mode;
    if Segmented::new(&segments).width(width).show(ui, &mut chosen) {
        tools.hollow.mode = chosen;
    }

    ui.add_space(2.0);
    describe(
        ui,
        match tools.hollow.mode {
            HollowMode::Internal => "The wall grows inward; the outside is untouched.",
            HollowMode::External => "The wall grows outward, making a mould of the model.",
            HollowMode::BottomThrough => "Inside, with the floor open so the resin drains.",
        },
    );
}

fn wall(ui: &mut egui::Ui, window: &mut Window) {
    number_row(
        ui,
        "Wall",
        &mut window.tools.hollow.thickness_mm,
        "mm",
        COARSE_STEP,
        MIN_WALL_MM..=MAX_WALL_MM,
        2,
    );
    number_row(
        ui,
        "Precision",
        &mut window.tools.hollow.precision,
        "",
        FINE_STEP,
        0.0..=1.0,
        2,
    );
    ui.add_space(2.0);
    let lattice_mm = core_volume::lattice_mm(
        window.tools.hollow.thickness_mm,
        window.tools.hollow.precision,
        widest(window.doc),
    );
    describe(
        ui,
        &format!(
            "Precision is how smooth the cavity comes out, on a {lattice_mm:.2} mm lattice. It is \
             not the layer height.",
        ),
    );
}

/// The largest surface of any model on the plate, in square millimetres.
///
/// The lattice is bounded by it: the cost of a cavity follows the model's area over the
/// square of the spacing, so a spacing in millimetres alone would let a big model cost
/// what the machine has not got.
fn widest(doc: &Doc) -> f32 {
    doc.scene
        .targets()
        .map(|object| object.mesh.surface_area())
        .fold(0.0, f32::max)
}

/// What stands in the cavity, if anything.
fn infill(ui: &mut egui::Ui, tools: &mut Tools) {
    switch(ui, &mut tools.hollow.infill_on, "Infill");
    if !tools.hollow.infill_on {
        ui.add_space(2.0);
        describe(ui, "The cavity is left empty.");
        return;
    }

    nested(ui, |ui| {
        ui.add_space(2.0);
        let segments = [
            Segment::new(InfillPattern::Hive, "Hive"),
            Segment::new(InfillPattern::Grid, "Grid"),
            Segment::new(InfillPattern::Scaffold, "Scaffold"),
        ];
        let width = ui.available_width();
        Segmented::new(&segments)
            .width(width)
            .show(ui, &mut tools.hollow.infill.pattern);

        ui.add_space(2.0);
        number_row(
            ui,
            "Cell",
            &mut tools.hollow.infill.size_mm,
            "mm",
            COARSE_STEP,
            MIN_CELL_MM..=MAX_CELL_MM,
            1,
        );
        let mut percent = tools.hollow.infill.density * 100.0;
        number_row(
            ui,
            "Density",
            &mut percent,
            "%",
            COARSE_STEP,
            MIN_DENSITY * 100.0..=MAX_DENSITY * 100.0,
            0,
        );
        tools.hollow.infill.density = percent / 100.0;

        ui.add_space(2.0);
        describe(
            ui,
            &format!(
                "{:.2} mm walls. Hive and Grid stand on the plate and drain; Scaffold also \
                 braces sideways.",
                tools.hollow.infill.thickness_mm()
            ),
        );
    });
}

/// How wide a blocker the next click drops, and how many are standing.
fn blockers(ui: &mut egui::Ui, window: &mut Window) {
    number_row(
        ui,
        "Blocker size",
        &mut window.tools.hollow.blocker_mm,
        "mm",
        COARSE_STEP,
        0.5..=40.0,
        1,
    );

    let placed: usize = window
        .doc
        .scene
        .targets()
        .map(|object| object.hollow.blockers().len())
        .sum();
    if placed == 0 {
        return;
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::REMOVE, &format!("Clear {placed} blocker(s)")).clicked() {
        for object in window.doc.scene.targets_mut() {
            object.hollow.clear_blockers();
        }
    }
}

/// The Hollow button, or the progress of the run that is going. The Drain tool shows the
/// same one: a hole is cut by the hollow run.
pub(super) fn action(ui: &mut egui::Ui, window: &mut Window) {
    match window.tools.hollow.job.as_ref() {
        Some(job) => running(ui, job),
        None => start(ui, window),
    }
}

fn start(ui: &mut egui::Ui, window: &mut Window) {
    let blocked = window.tools.hollow.blocker(&window.doc.scene);
    if primary_button(ui, icon::HOLLOW, "Hollow", blocked.is_none()).clicked() {
        let started = window.tools.hollow.start(&window.doc.scene);
        window.machine.status.report("Hollowing", started);
    }

    ui.add_space(4.0);
    match blocked {
        Some(reason) => {
            ui.vertical_centered(|ui| tone(ui, reason, theme::colors().text_low));
        }
        None => describe(
            ui,
            &format!("Cuts a cavity inside {}.", window.doc.scene.scope()),
        ),
    }
}

/// The progress bar and the cancel button of the run that is going.
fn running(ui: &mut egui::Ui, job: &HollowJob) {
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

/// What the plate is carrying now, and the one way back to a solid model.
fn done(ui: &mut egui::Ui, window: &mut Window) {
    let hollowed = window
        .doc
        .scene
        .targets()
        .filter(|object| object.hollow.is_hollow())
        .count();
    if hollowed == 0 {
        return;
    }

    let saved: f32 = window
        .doc
        .scene
        .targets()
        .map(|object| object.hollow.cavity_mm3())
        .sum();
    stats(
        ui,
        &[
            ("Hollowed", hollowed.to_string()),
            ("Resin saved", format!("{:.1} ml", saved / 1000.0)),
        ],
    );

    // The lattice a run settled on, which is not always the one precision asked for: a
    // model big enough for the budget to bite is cut on a coarser one and says so.
    if let Some((voxel_mm, coarsened)) = window
        .doc
        .scene
        .targets()
        .find_map(|object| object.hollow.lattice())
        && coarsened
    {
        ui.add_space(4.0);
        hint(
            ui,
            &format!(
                "This model needed more memory than the budget allows, so the cavity was cut on \
                 a {voxel_mm:.2} mm lattice.",
            ),
        );
    }

    let asked = window.tools.hollow.settings();
    let stale = window
        .doc
        .scene
        .targets()
        .any(|object| object.hollow.is_stale(&asked, object.transform));
    if stale {
        ui.add_space(4.0);
        hint(
            ui,
            "The numbers have moved since. Hollow again to catch up.",
        );
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::REMOVE, "Make solid again").clicked() {
        for object in window.doc.scene.targets_mut() {
            object.hollow.clear();
        }
    }
}
