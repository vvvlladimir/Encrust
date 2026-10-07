use crate::drain::Placing;
use crate::panels::Window;
use crate::panels::inspector::hollow;
use crate::state::Tools;
use crate::ui::{
    Segment, Segmented, describe, hint, icon, notice, number_row, primary_button, secondary_button,
    section, stats, subheading, theme, tone,
};

/// Per point of drag, in millimetres for the sizes and hundredths for the taper.
const COARSE_STEP: f64 = 0.05;
const FINE_STEP: f64 = 0.01;

/// Bounds on a hole. Under half a millimetre the resin holds itself in by surface tension,
/// and past twenty it is a window rather than a drain.
const MIN_HOLE_MM: f32 = 0.5;
const MAX_HOLE_MM: f32 = 20.0;

/// Bounds on the depth. It has to cross the wall to be a drain, and there is nothing to
/// drill through past the height of a plate.
const MIN_DEPTH_MM: f32 = 0.5;
const MAX_DEPTH_MM: f32 = 50.0;

/// The holes that let the resin out of a hollow model.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Drain holes", None, |ui| {
        describe(
            ui,
            "Click the model to put one down, alt-click a hole to take it away. Each one \
             is cut the moment it is placed, hollow or not.",
        );
        ui.add_space(4.0);

        placing(ui, window);
        ui.add_space(4.0);
        sizes(ui, window.tools);

        subheading(ui, "Placed");
        placed(ui, window);

        ui.add_space(8.0);
        hollow::action(ui, window);

        subheading(ui, "Drainage");
        drainage(ui, window);
    });
}

/// Whether a click drills a hole or lays out a channel.
fn placing(ui: &mut egui::Ui, window: &mut Window) {
    let segments = [
        Segment::new(Placing::Hole, "Holes"),
        Segment::new(Placing::Channel, "Channel"),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut window.tools.drain.placing);

    if window.tools.drain.placing == Placing::Hole {
        return;
    }
    let points: usize = window
        .doc
        .scene
        .targets()
        .map(|object| object.hollow.pending().len())
        .sum();

    ui.add_space(2.0);
    describe(
        ui,
        "Each click adds a point; the channel is a tube of the hole size running under \
         all of them, open at its two ends alone.",
    );
    if points == 0 {
        return;
    }

    ui.add_space(4.0);
    if secondary_button(
        ui,
        icon::DRAIN,
        &format!("Dig the channel ({points} points)"),
    )
    .clicked()
    {
        let dug = window.tools.drain.finish_channels(&mut window.doc.scene);
        window.machine.status = crate::status::Status::Info(format!("{dug} channel(s) dug"));
        rebuild_around_cuts(window);
    }
}

fn sizes(ui: &mut egui::Ui, tools: &mut Tools) {
    number_row(
        ui,
        "Hole size",
        &mut tools.drain.diameter_mm,
        "mm",
        COARSE_STEP,
        MIN_HOLE_MM..=MAX_HOLE_MM,
        1,
    );
    number_row(
        ui,
        "Depth",
        &mut tools.drain.depth_mm,
        "mm",
        COARSE_STEP,
        MIN_DEPTH_MM..=MAX_DEPTH_MM,
        1,
    );
    number_row(
        ui,
        "Taper",
        &mut tools.drain.taper,
        "",
        FINE_STEP,
        0.1..=1.0,
        2,
    );

    ui.add_space(2.0);
    describe(
        ui,
        "The depth is measured from the surface inwards, so a hole has to be deeper than \
         the wall. A taper under 1 makes it a cone.",
    );
}

/// How many holes are standing, and the one way to take them all away.
fn placed(ui: &mut egui::Ui, window: &mut Window) {
    let drilled: usize = window
        .doc
        .scene
        .targets()
        .map(|object| object.hollow.drains().len() + object.hollow.channels().len())
        .sum();
    if drilled == 0 {
        hint(ui, "No hole and no channel is cut yet.");
        return;
    }

    if secondary_button(ui, icon::REMOVE, &format!("Clear {drilled} cut(s)")).clicked() {
        for object in window.doc.scene.targets_mut() {
            object.hollow.clear_drains();
            object.hollow.clear_channels();
        }
        window.tools.drain.stale();
        rebuild_around_cuts(window);
    }
}

/// A channel dug or cleared on a hollow model moves the sleeve its cavity keeps off it, so
/// the shell is built again on the spot rather than left open into the pipe.
fn rebuild_around_cuts(window: &mut Window) {
    if window.tools.hollow.rebuild(&window.doc.scene) {
        window.machine.status =
            crate::status::Status::Info("Hollowing again around the channels".to_owned());
    }
}

/// The check for resin that cannot get out, and what it found.
fn drainage(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.drain.job.is_some() {
        tone(ui, "Looking for trapped resin", theme::colors().text_mid);
        ui.add(
            egui::ProgressBar::new(0.0)
                .desired_height(6.0)
                .corner_radius(theme::R_CONTROL),
        );
        return;
    }

    let layer_height_mm = window.machine.slicing.layer_height_mm();
    if primary_button(
        ui,
        icon::DRAIN,
        "Check drainage",
        !window.doc.scene.is_empty(),
    )
    .clicked()
    {
        let started = window.tools.drain.start(&window.doc.scene, layer_height_mm);
        window.machine.status.report("Checking drainage", started);
    }
    ui.add_space(4.0);

    let pockets: Vec<f32> = window
        .doc
        .scene
        .targets()
        .flat_map(|object| object.traps.found())
        .map(|trapped| trapped.volume_mm3)
        .collect();
    if pockets.is_empty() {
        let text = if window.tools.drain.checked {
            "Nothing is trapped: the resin can get out of every model on the plate."
        } else {
            "Cuts the plate and follows the air through it, looking for resin with no way out."
        };
        ui.vertical_centered(|ui| tone(ui, text, theme::colors().text_low));
        return;
    }

    let held: f32 = pockets.iter().sum();
    let colors = theme::colors();
    notice(
        ui,
        icon::WARNING,
        colors.danger,
        colors.danger_wash,
        &format!("Resin is trapped in {} place(s)", pockets.len()),
        "Each one is painted red under the x-ray, and each needs a hole of its own.",
    );
    ui.add_space(4.0);
    stats(
        ui,
        &[
            ("Trapped", pockets.len().to_string()),
            ("Resin held", format!("{:.2} ml", held / 1000.0)),
        ],
    );
    ui.add_space(4.0);
    if secondary_button(ui, icon::DRAIN, "Drill a hole into each").clicked() {
        let drilled = window.tools.drain.drill_found(&mut window.doc.scene);
        window.machine.status = crate::status::Status::Info(format!("{drilled} hole(s) placed"));
    }
    ui.add_space(2.0);
    describe(ui, "Each hole goes in at the lowest point of its pocket.");
}
