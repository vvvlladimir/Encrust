use core_volume::{HollowMode, InfillPattern, InfillSettings, MIN_WALL_MM};

use printer_profiles::Kind;

use super::presets;
use super::scope::{self, Scoped};
use crate::i18n::tr;
use crate::job::HollowJob;
use crate::panels::Window;
use crate::state::{Doc, Tools};
use crate::ui::{
    Segment, Segmented, describe, hint, icon, nested, notice, number_row, primary_button,
    progress_bar, secondary_button, section, section_with_switch, stats, subheading, theme, tone,
};
use crate::workspace::Tool;

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

/// Bounds on a hive or grid wall: under half a millimetre the field cannot carry it, and
/// past ten it is solid with a few holes in it.
const MIN_INFILL_WALL_MM: f32 = 0.5;
const MAX_INFILL_WALL_MM: f32 = 10.0;

/// The narrowest crossing hole resin still runs through, and the share of the widest the
/// pattern allows that a hole may take.
const MIN_HOLE_MM: f32 = 0.5;
const MAX_HOLE_SHARE: f32 = 0.9;

/// How high the rows of crossing holes may start, the least wall left between two rows and
/// the furthest apart they may stand, mm.
const MAX_HOLE_FROM_MM: f32 = 40.0;
const MIN_HOLE_GAP_MM: f32 = 1.0;
const MAX_HOLE_SPACING_MM: f32 = 40.0;

/// Bounds on a scaffold's density. Under a twentieth the lattice holds nothing up, and
/// past half it is a solid with a texture rather than an infill.
const MIN_DENSITY: f32 = 0.05;
const MAX_DENSITY: f32 = 0.5;

/// The cavity inside the models on the plate, and what stands in it.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let scoped = Scoped {
        plate: |tools| &mut tools.hollow.state,
        own: |object| &mut object.own.hollow,
        read: |object| &object.own.hollow,
    };
    let own = section(ui, &tr!("common-settings"), None, |ui| {
        let own = scope::switch(ui, window, &scoped);
        scope::editing(window, own, &scoped, |window| {
            presets::bar(
                ui,
                &mut window.machine.slicing.catalogue,
                &mut window.machine.status,
                Kind::Hollow,
                &mut window.tools.hollow.state,
            );
        });
        own
    })
    .flatten();
    scope::editing(window, own, &scoped, |window| settings(ui, window));
    section(ui, &tr!("common-already-done"), None, |ui| {
        done(ui, window);
        drainage(ui, window);
    });
}

/// The numbers the panel edits: the plate's, or the picked model's own.
fn settings(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, &tr!("hollow-shell"), None, |ui| {
        describe(ui, &tr!("hollow-shell-help"));
        modes(ui, window.tools);
        wall(ui, window);
    });
    // A mould's void is the model itself: nothing stands in it and no wall is kept solid
    // (docs/design/hollowing.md).
    if window.tools.hollow.state.mode == HollowMode::External {
        return;
    }
    // The body reads the switch as it stood before this frame's click; the knob's slide
    // draws the frame after, which catches it up.
    let mut on = window.tools.hollow.state.infill_on;
    section_with_switch(ui, &tr!("hollow-infill"), &mut on, |ui| {
        infill(ui, window.tools);
    });
    window.tools.hollow.state.infill_on = on;
    section(ui, &tr!("hollow-keep-solid"), None, |ui| {
        describe(ui, &tr!("hollow-keep-solid-note"));
        blockers(ui, window);
    });
}

/// What the last drainage check found, where the cavity it was found in is. The check
/// itself and the holes are the Drain tool's; see ADR 0190, 0198.
fn drainage(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.drain.job.is_some() {
        subheading(ui, &tr!("hollow-drainage"));
        tone(ui, &tr!("hollow-looking"), theme::colors().text_mid);
        progress_bar(ui, None);
        return;
    }

    let trapped: usize = window
        .doc
        .scene
        .targets()
        .map(|object| object.traps.found().len())
        .sum();
    if trapped == 0 {
        if window.tools.drain.checked {
            subheading(ui, &tr!("hollow-drainage"));
            tone(ui, &tr!("hollow-drains"), theme::colors().ok);
        }
        return;
    }

    let colors = theme::colors();
    subheading(ui, &tr!("hollow-drainage"));
    notice(
        ui,
        icon::WARNING,
        colors.danger,
        &tr!("hollow-trapped", count = trapped),
        &tr!("hollow-trapped-note"),
    );
    if secondary_button(ui, icon::DRAIN, &tr!("hollow-drain-trapped")).clicked() {
        crate::shortcuts::pick(window, Tool::Drain);
    }
}

/// Which surface the wall is measured from.
fn modes(ui: &mut egui::Ui, tools: &mut Tools) {
    let segments = [
        Segment::new(HollowMode::Internal, tr!("hollow-inside")),
        Segment::new(HollowMode::External, tr!("hollow-outside")),
    ];
    let width = ui.available_width();
    let mut chosen = tools.hollow.state.mode;
    if Segmented::new(&segments).width(width).show(ui, &mut chosen) {
        tools.hollow.state.mode = chosen;
    }

    describe(
        ui,
        &match tools.hollow.state.mode {
            HollowMode::Internal => tr!("hollow-inside-note"),
            HollowMode::External => tr!("hollow-outside-note"),
        },
    );
}

fn wall(ui: &mut egui::Ui, window: &mut Window) {
    number_row(
        ui,
        &tr!("hollow-wall"),
        &mut window.tools.hollow.state.thickness_mm,
        &tr!("unit-mm"),
        COARSE_STEP,
        MIN_WALL_MM..=MAX_WALL_MM,
        2,
    );
    number_row(
        ui,
        &tr!("hollow-precision"),
        &mut window.tools.hollow.state.precision,
        "",
        FINE_STEP,
        0.0..=1.0,
        2,
    );
    ui.add_space(2.0);
    let lattice_mm = core_volume::lattice_mm(
        window.tools.hollow.state.thickness_mm,
        window.tools.hollow.state.precision,
        widest(window.doc),
    );
    describe(
        ui,
        &tr!(
            "hollow-precision-note",
            lattice = format!("{lattice_mm:.2}")
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
    if !tools.hollow.state.infill_on {
        ui.add_space(2.0);
        describe(ui, &tr!("hollow-empty"));
        return;
    }

    nested(ui, |ui| {
        ui.add_space(2.0);
        let segments = [
            Segment::new(InfillPattern::Hive, tr!("hollow-hive")),
            Segment::new(InfillPattern::Grid, tr!("hollow-grid")),
            Segment::new(InfillPattern::Scaffold, tr!("hollow-scaffold")),
        ];
        let width = ui.available_width();
        Segmented::new(&segments)
            .width(width)
            .show(ui, &mut tools.hollow.state.infill.pattern);

        ui.add_space(2.0);
        let infill = &mut tools.hollow.state.infill;
        let cell = match infill.pattern {
            InfillPattern::Hive => tr!("hollow-cell-hive"),
            InfillPattern::Grid => tr!("hollow-cell-grid"),
            InfillPattern::Scaffold => tr!("hollow-cell"),
        };
        let mm = tr!("unit-mm");
        let cells = MIN_CELL_MM..=MAX_CELL_MM;
        number_row(ui, &cell, &mut infill.cell_mm, &mm, COARSE_STEP, cells, 1);
        if infill.pattern.has_walls() {
            walls(ui, infill);
        } else {
            let mut percent = infill.density * 100.0;
            let range = MIN_DENSITY * 100.0..=MAX_DENSITY * 100.0;
            number_row(
                ui,
                &tr!("hollow-density"),
                &mut percent,
                "%",
                COARSE_STEP,
                range,
                0,
            );
            infill.density = percent / 100.0;
        }

        ui.add_space(2.0);
        describe(
            ui,
            &match infill.pattern.has_walls() {
                true => tr!("hollow-walled-note"),
                false => tr!("hollow-scaffold-note"),
            },
        );
    });
}

/// The wall between two hive or grid cells, and the rows of crossing holes through it.
fn walls(ui: &mut egui::Ui, infill: &mut InfillSettings) {
    let mm = tr!("unit-mm");
    let wall = MIN_INFILL_WALL_MM..=MAX_INFILL_WALL_MM;
    number_row(
        ui,
        &tr!("hollow-infill-wall"),
        &mut infill.wall_mm,
        &mm,
        COARSE_STEP,
        wall,
        2,
    );
    let holes = MIN_HOLE_MM..=(infill.widest_hole_mm() * MAX_HOLE_SHARE).max(MIN_HOLE_MM);
    number_row(
        ui,
        &tr!("hollow-hole"),
        &mut infill.hole_mm,
        &mm,
        COARSE_STEP,
        holes,
        2,
    );
    let from = 0.0..=MAX_HOLE_FROM_MM;
    let first = &mut infill.first_hole_mm;
    number_row(
        ui,
        &tr!("hollow-hole-from"),
        first,
        &mm,
        COARSE_STEP,
        from,
        2,
    );
    let spacing = (infill.hole_mm + MIN_HOLE_GAP_MM)..=MAX_HOLE_SPACING_MM;
    let rows = &mut infill.hole_spacing_mm;
    number_row(
        ui,
        &tr!("hollow-hole-spacing"),
        rows,
        &mm,
        COARSE_STEP,
        spacing,
        2,
    );
}

/// How wide a blocker the next click drops, and how many are standing.
fn blockers(ui: &mut egui::Ui, window: &mut Window) {
    number_row(
        ui,
        &tr!("hollow-blocker-size"),
        &mut window.tools.hollow.state.blocker_mm,
        &tr!("unit-mm"),
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
    if secondary_button(
        ui,
        icon::REMOVE,
        &tr!("hollow-clear-blockers", count = placed),
    )
    .clicked()
    {
        for object in window.doc.scene.targets_mut() {
            object.hollow.clear_blockers();
        }
    }
}

/// The Hollow button, or the progress of the run that is going.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    match window.tools.hollow.job.as_ref() {
        Some(job) => running(ui, job),
        None => start(ui, window),
    }
}

fn start(ui: &mut egui::Ui, window: &mut Window) {
    let blocked = window.tools.hollow.blocker(&window.doc.scene);
    let what = tr!("hollow-tooltip", scope = window.doc.scene.scope());
    if primary_button(ui, icon::HOLLOW, &tr!("hollow-hollow"), blocked.is_none())
        .on_hover_text(what)
        .clicked()
    {
        let started = window.tools.hollow.start(&window.doc.scene);
        window
            .machine
            .status
            .report(&tr!("hollow-hollowing"), started);
    }
    if let Some(reason) = blocked {
        ui.vertical_centered(|ui| tone(ui, &reason, theme::colors().text_low));
    }
}

/// The progress bar and the cancel button of the run that is going. The Drain tool shows
/// it too, since a channel dug into a hollow model runs the shell again.
pub(super) fn running(ui: &mut egui::Ui, job: &HollowJob) {
    tone(ui, &job.label(), theme::colors().text_mid);
    progress_bar(ui, job.fraction());

    ui.add_space(4.0);
    ui.add_enabled_ui(!job.is_cancelling(), |ui| {
        if secondary_button(ui, icon::CANCEL, &tr!("common-cancel")).clicked() {
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
            (tr!("hollow-hollowed"), hollowed.to_string()),
            (
                tr!("hollow-resin-saved"),
                tr!("value-ml", value = format!("{:.1}", saved / 1000.0)),
            ),
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
            &tr!("hollow-coarsened", lattice = format!("{voxel_mm:.2}")),
        );
    }

    let hollow = &window.tools.hollow;
    let stale = window.doc.scene.targets().any(|object| {
        object
            .hollow
            .is_stale(&hollow.settings_for(object), object.transform)
    });
    if stale {
        ui.add_space(4.0);
        hint(ui, &tr!("hollow-stale"));
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::REMOVE, &tr!("hollow-make-solid")).clicked() {
        for object in window.doc.scene.targets_mut() {
            object.hollow.clear();
        }
    }
}
