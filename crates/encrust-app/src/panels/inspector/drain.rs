use core_volume::HoleShape;
use printer_profiles::Kind;

use super::presets;
use super::scope::{self, Scoped};
use crate::drain::Placing;
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::inspector::hollow;
use crate::scene::ObjectId;
use crate::state::Tools;
use crate::ui::{
    Segment, Segmented, busy, count_row, describe, hint, icon, inline_button, notice, number_row,
    primary_button, secondary_button, section, theme, tone,
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

/// The sides a hole turns to polygon with, and how far it may be turned: past half a turn
/// every polygon repeats itself.
const DEFAULT_SIDES: u8 = 6;
const MAX_TURN_DEG: f32 = 180.0;

/// Bounds on a plug's gap, millimetres a side: past half a millimetre it rattles in its
/// hole, and under nothing it is pressed in.
const MIN_PLUG_GAP_MM: f32 = 0.0;
const MAX_PLUG_GAP_MM: f32 = 0.5;

/// The holes that let the resin out of a hollow model, and the resin that cannot get out
/// yet.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let scoped = Scoped {
        plate: |tools| &mut tools.drain.state,
        own: |object| &mut object.own.drain,
        read: |object| &object.own.drain,
    };
    let own = section(ui, &tr!("common-settings"), None, |ui| {
        let own = scope::switch(ui, window, &scoped);
        scope::editing(window, own, &scoped, |window| {
            presets::bar(
                ui,
                &mut window.machine.slicing.catalogue,
                &mut window.machine.status,
                Kind::Drain,
                &mut window.tools.drain.state,
            );
        });
        own
    })
    .flatten();
    section(ui, &tr!("drain-trapped-resin"), None, |ui| {
        drainage(ui, window)
    });
    section(ui, &tr!("drain-opening"), None, |ui| {
        describe(ui, &tr!("drain-opening-note"));
        placing(ui, window);
        scope::editing(window, own, &scoped, |window| {
            sizes(ui, window.tools);
            outline(ui, window.tools);
        });
        placed(ui, window);
        if let Some(job) = window.tools.hollow.job.as_ref() {
            hollow::running(ui, job);
        }
    });
    section(ui, &tr!("drain-plugs"), None, |ui| {
        describe(ui, &tr!("drain-plugs-note"));
        scope::editing(window, own, &scoped, |window| plug_gap(ui, window.tools));
        drift(ui, window);
    });
}

/// How many pockets hold resin, for the inspector's heading.
pub fn fact(window: &Window) -> Option<String> {
    let trapped = pockets(window).len();
    (trapped > 0).then(|| tr!("drain-fact", count = trapped))
}

/// Digging the channel laid out, while there is one; otherwise the check for resin with
/// no way out, or its progress.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let points = channel_points(window);
    if window.tools.drain.placing == Placing::Channel && points > 0 {
        let label = tr!("drain-dig", count = points);
        let what = tr!("drain-dig-tooltip");
        if primary_button(ui, icon::DRAIN, &label, true)
            .on_hover_text(what)
            .clicked()
        {
            let dug = window.tools.drain.finish_channels(&mut window.doc.scene);
            window.machine.status = crate::status::Status::Info(tr!("drain-dug", count = dug));
            rebuild_around_cuts(window);
        }
        return;
    }
    if window.tools.drain.job.is_some() {
        busy(ui, &tr!("hollow-looking"), None, false);
        return;
    }
    let layer_height_mm = window.machine.slicing.layer_height_mm();
    let label = if window.tools.drain.checked {
        tr!("drain-check-again")
    } else {
        tr!("drain-check")
    };
    let what = tr!("drain-check-tooltip");
    if primary_button(ui, icon::DRAIN, &label, !window.doc.scene.is_empty())
        .on_hover_text(what)
        .clicked()
    {
        let started = window.tools.drain.start(&window.doc.scene, layer_height_mm);
        window
            .machine
            .status
            .report(&tr!("drain-checking"), started);
    }
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

/// Every pocket of resin the last check found: the model it is in, its place among that
/// model's pockets, and its volume, cubic millimetres.
fn pockets(window: &Window) -> Vec<(ObjectId, usize, f32)> {
    window
        .doc
        .scene
        .targets()
        .flat_map(|object| {
            let id = object.id;
            let found = object.traps.found().iter().enumerate();
            found.map(move |(index, trapped)| (id, index, trapped.volume_mm3))
        })
        .collect()
}

/// Whether a click drills a hole or lays out a channel.
fn placing(ui: &mut egui::Ui, window: &mut Window) {
    let segments = [
        Segment::new(Placing::Hole, tr!("drain-hole")),
        Segment::new(Placing::Channel, tr!("drain-channel")),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut window.tools.drain.placing);

    if window.tools.drain.placing == Placing::Channel {
        describe(ui, &tr!("drain-channel-note"));
    }
}

fn sizes(ui: &mut egui::Ui, tools: &mut Tools) {
    number_row(
        ui,
        &tr!("drain-hole-size"),
        &mut tools.drain.state.diameter_mm,
        &tr!("unit-mm"),
        COARSE_STEP,
        MIN_HOLE_MM..=MAX_HOLE_MM,
        1,
    );
    number_row(
        ui,
        &tr!("presets-depth"),
        &mut tools.drain.state.depth_mm,
        &tr!("unit-mm"),
        COARSE_STEP,
        MIN_DEPTH_MM..=MAX_DEPTH_MM,
        1,
    );
    number_row(
        ui,
        &tr!("presets-taper"),
        &mut tools.drain.state.taper,
        "",
        FINE_STEP,
        0.1..=1.0,
        2,
    );

    describe(ui, &tr!("drain-sizes-note"));
}

/// Round, or a polygon of a number of sides turned about the hole's axis.
fn outline(ui: &mut egui::Ui, tools: &mut Tools) {
    let state = &mut tools.drain.state;
    let round = matches!(state.shape, HoleShape::Round);
    let segments = [
        Segment::new(true, tr!("drain-round")),
        Segment::new(false, tr!("drain-polygon")),
    ];
    let mut chosen = round;
    let width = ui.available_width();
    if Segmented::new(&segments).width(width).show(ui, &mut chosen) && chosen != round {
        state.shape = if chosen {
            HoleShape::Round
        } else {
            HoleShape::Polygon {
                sides: DEFAULT_SIDES,
            }
        };
    }
    let HoleShape::Polygon { sides } = &mut state.shape else {
        return;
    };
    let mut count = u32::from(*sides);
    let range = u32::from(HoleShape::MIN_SIDES)..=u32::from(HoleShape::MAX_SIDES);
    if count_row(ui, &tr!("drain-sides"), &mut count, "", 0.05, range) {
        *sides = u8::try_from(count).unwrap_or(HoleShape::MAX_SIDES);
    }
    number_row(
        ui,
        &tr!("drain-turn"),
        &mut state.turn_deg,
        &tr!("unit-degrees"),
        0.5,
        0.0..=MAX_TURN_DEG,
        0,
    );
    describe(ui, &tr!("drain-polygon-note"));
}

/// What the plugs are widened by where the resin's tolerance compensation is off.
fn drift(ui: &mut egui::Ui, window: &Window) {
    let drift_mm = window.machine.slicing.material.compensation.fit_drift_mm();
    if drift_mm.abs() > f32::EPSILON {
        describe(
            ui,
            &tr!("drain-plug-drift", drift = format!("{drift_mm:.2}")),
        );
    }
}

fn plug_gap(ui: &mut egui::Ui, tools: &mut Tools) {
    number_row(
        ui,
        &tr!("drain-plug-gap"),
        &mut tools.drain.state.plug_gap_mm,
        &tr!("unit-mm"),
        FINE_STEP,
        MIN_PLUG_GAP_MM..=MAX_PLUG_GAP_MM,
        2,
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
        hint(ui, &tr!("drain-none-cut"));
        return;
    }

    if secondary_button(ui, icon::REMOVE, &tr!("drain-clear-cuts", count = drilled)).clicked() {
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
        window.machine.status = crate::status::Status::Info(tr!("drain-rehollowing"));
    }
}

/// What the last check found: every pocket of resin with no way out, and the holes that
/// would let it go.
fn drainage(ui: &mut egui::Ui, window: &mut Window) {
    let pockets = pockets(window);
    if pockets.is_empty() {
        let text = if window.tools.drain.checked {
            tr!("drain-nothing-trapped")
        } else {
            tr!("drain-not-checked")
        };
        hint(ui, &text);
        return;
    }

    let held: f32 = pockets.iter().map(|(_, _, volume_mm3)| volume_mm3).sum();
    let colors = theme::colors();
    notice(
        ui,
        icon::WARNING,
        colors.danger,
        &tr!("drain-held", volume = format!("{:.2}", held / 1000.0)),
        &tr!("drain-held-note"),
    );
    let mut drill = None;
    for (row, (id, index, volume_mm3)) in pockets.iter().enumerate() {
        if pocket_row(ui, row, *volume_mm3) {
            drill = Some((*id, *index));
        }
    }
    if let Some((id, index)) = drill
        && window
            .tools
            .drain
            .drill_pocket(&mut window.doc.scene, id, index)
    {
        window.machine.status = crate::status::Status::Info(tr!("drain-holes-placed", count = 1));
    }
    if secondary_button(ui, icon::DRAIN, &tr!("drain-drill-each")).clicked() {
        let drilled = window.tools.drain.drill_found(&mut window.doc.scene);
        window.machine.status =
            crate::status::Status::Info(tr!("drain-holes-placed", count = drilled));
    }
    describe(ui, &tr!("drain-lowest-point"));
}

/// One pocket: a dot, how much it holds, and a drill of its own. Answers whether the
/// drill was pressed.
fn pocket_row(ui: &mut egui::Ui, index: usize, volume_mm3: f32) -> bool {
    let colors = theme::colors();
    let mut drill = false;
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(dot.center(), 3.5, colors.danger);
        ui.label(
            egui::RichText::new(tr!(
                "value-ml",
                value = format!("{:.2}", volume_mm3 / 1000.0)
            ))
            .font(theme::figures(12.0))
            .color(colors.text_high),
        );
        tone(ui, &tr!("drain-pocket", index = index + 1), colors.text_mid);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            drill = inline_button(ui, icon::DRAIN, &tr!("drain-drill"), false, true)
                .on_hover_text(tr!("drain-drill-tooltip"))
                .clicked();
        });
    });
    drill
}
