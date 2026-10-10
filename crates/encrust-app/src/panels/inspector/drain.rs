use crate::drain::Placing;
use crate::panels::Window;
use crate::panels::inspector::hollow;
use crate::state::Tools;
use crate::ui::{
    Segment, Segmented, describe, hint, icon, inline_button, later, notice, number_row,
    primary_button, progress_bar, secondary_button, section, theme, tone,
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

/// The holes that let the resin out of a hollow model, and the resin that cannot get out
/// yet.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Trapped resin", None, |ui| drainage(ui, window));
    section(ui, "Opening", None, |ui| {
        describe(
            ui,
            "Click the model to put one down, alt-click a hole to take it away. Each one \
             is cut the moment it is placed, hollow or not.",
        );
        placing(ui, window);
        sizes(ui, window.tools);
        placed(ui, window);
        if let Some(job) = window.tools.hollow.job.as_ref() {
            hollow::running(ui, job);
        }
    });
}

/// How many pockets hold resin, for the inspector's heading.
pub fn fact(window: &Window) -> Option<String> {
    let trapped = pockets(window).len();
    (trapped > 0).then(|| format!("{trapped} trapped"))
}

/// Digging the channel laid out, while there is one; otherwise the check for resin with
/// no way out, or its progress.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let points = channel_points(window);
    if window.tools.drain.placing == Placing::Channel && points > 0 {
        let label = format!("Dig the channel ({points} points)");
        if primary_button(ui, icon::DRAIN, &label, true).clicked() {
            let dug = window.tools.drain.finish_channels(&mut window.doc.scene);
            window.machine.status = crate::status::Status::Info(format!("{dug} channel(s) dug"));
            rebuild_around_cuts(window);
        }
        hint(
            ui,
            "A tube of the hole size runs under every point, open at its two ends alone.",
        );
        return;
    }
    if window.tools.drain.job.is_some() {
        tone(ui, "Looking for trapped resin", theme::colors().text_mid);
        progress_bar(ui, None);
        return;
    }
    let layer_height_mm = window.machine.slicing.layer_height_mm();
    let label = if window.tools.drain.checked {
        "Check again"
    } else {
        "Check drainage"
    };
    if primary_button(ui, icon::DRAIN, label, !window.doc.scene.is_empty()).clicked() {
        let started = window.tools.drain.start(&window.doc.scene, layer_height_mm);
        window.machine.status.report("Checking drainage", started);
    }
    hint(
        ui,
        "Cuts the plate and follows the air through it, looking for resin with no way out.",
    );
}

/// The points a channel is laid out along, not dug yet.
fn channel_points(window: &Window) -> usize {
    window
        .doc
        .scene
        .targets()
        .map(|object| object.hollow.pending().len())
        .sum()
}

/// The volume of every pocket of resin the last check found, cubic millimetres.
fn pockets(window: &Window) -> Vec<f32> {
    window
        .doc
        .scene
        .targets()
        .flat_map(|object| object.traps.found())
        .map(|trapped| trapped.volume_mm3)
        .collect()
}

/// Whether a click drills a hole or lays out a channel.
fn placing(ui: &mut egui::Ui, window: &mut Window) {
    let segments = [
        Segment::new(Placing::Hole, "Hole"),
        Segment::new(Placing::Channel, "Channel"),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut window.tools.drain.placing);

    if window.tools.drain.placing == Placing::Channel {
        describe(
            ui,
            "Each click adds a point; the channel is a tube of the hole size running under \
             all of them, open at its two ends alone.",
        );
    }
}

fn sizes(ui: &mut egui::Ui, tools: &mut Tools) {
    number_row(
        ui,
        "Hole size",
        &mut tools.drain.state.diameter_mm,
        "mm",
        COARSE_STEP,
        MIN_HOLE_MM..=MAX_HOLE_MM,
        1,
    );
    number_row(
        ui,
        "Depth",
        &mut tools.drain.state.depth_mm,
        "mm",
        COARSE_STEP,
        MIN_DEPTH_MM..=MAX_DEPTH_MM,
        1,
    );
    number_row(
        ui,
        "Taper",
        &mut tools.drain.state.taper,
        "",
        FINE_STEP,
        0.1..=1.0,
        2,
    );

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

/// What the last check found: every pocket of resin with no way out, and the holes that
/// would let it go.
fn drainage(ui: &mut egui::Ui, window: &mut Window) {
    let pockets = pockets(window);
    if pockets.is_empty() {
        let text = if window.tools.drain.checked {
            "Nothing is trapped: the resin can get out of every model on the plate."
        } else {
            "Not checked yet."
        };
        hint(ui, text);
        return;
    }

    let held: f32 = pockets.iter().sum();
    let colors = theme::colors();
    notice(
        ui,
        icon::WARNING,
        colors.danger,
        &format!("{:.2} ml cannot get out", held / 1000.0),
        "It cures inside and adds weight the peel has to pull. Each pocket is painted red \
         under the x-ray, and each needs a hole of its own.",
    );
    for (index, volume_mm3) in pockets.iter().enumerate() {
        pocket_row(ui, index, *volume_mm3);
    }
    if secondary_button(ui, icon::DRAIN, "Drill a hole into each").clicked() {
        let drilled = window.tools.drain.drill_found(&mut window.doc.scene);
        window.machine.status = crate::status::Status::Info(format!("{drilled} hole(s) placed"));
    }
    describe(ui, "Each hole goes in at the lowest point of its pocket.");
}

/// One pocket: a dot, how much it holds, and a drill of its own.
fn pocket_row(ui: &mut egui::Ui, index: usize, volume_mm3: f32) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(dot.center(), 3.5, colors.danger);
        ui.label(
            egui::RichText::new(format!("{:.2} ml", volume_mm3 / 1000.0))
                .font(theme::figures(12.0))
                .color(colors.text_high),
        );
        tone(ui, &format!("pocket {}", index + 1), colors.text_mid);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // TODO(step-8): drill this pocket alone, at its lowest point.
            later(ui, |ui| {
                inline_button(ui, icon::DRAIN, "Drill", false, true)
            });
        });
    });
}
