use core_volume::{HollowMode, InfillPattern, InfillSettings, MIN_WALL_MM};

use printer_profiles::Kind;

use super::presets;
use super::scope::{self, Scoped};
use crate::i18n::tr;
use crate::job::HollowJob;
use crate::panels::Window;
use crate::state::{Doc, Tools};
use crate::ui::{
    Segment, Segmented, blocked_hover, busy, describe, hint, icon, nested, notice, number_row,
    primary_button, progress_bar, secondary_button, section, section_with_switch, stats,
    subheading, theme, tone,
};
use crate::workspace::Tool;

/// Per point of drag. A wall is measured in whole millimetres, precision and density in
/// hundredths of their own range.
const COARSE_STEP: f64 = 0.05;
const FINE_STEP: f64 = 0.01;

/// Past fifteen millimetres there is nothing left to hollow on anything that fits the
/// machine. The floor is `core_volume::MIN_WALL_MM`, which is what the lattice can resolve.
const MAX_WALL_MM: f32 = 15.0;

/// Bounds on one infill cell. Under two millimetres its crossing holes, sized to it, are
/// too narrow for resin to run through, and past forty there is one cell in the model.
const MIN_CELL_MM: f32 = 2.0;
const MAX_CELL_MM: f32 = 40.0;

/// Bounds on how much of the cavity a lattice fills. Under a twentieth it holds nothing up,
/// and past half it is a solid with a texture rather than an infill.
const MIN_DENSITY: f32 = 0.05;
const MAX_DENSITY: f32 = 0.5;

/// A wall or strut thinner than this, mm, is finer than the field can carry.
const THIN_INFILL_MM: f32 = 0.5;

/// The cavity inside the models on the plate, and what stands in it.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let scoped = Scoped {
        plate: |tools| &mut tools.hollow.state,
        own: |object| &mut object.own.hollow,
        read: |object| &object.own.hollow,
    };
    let own = section(ui, &tr!("common-settings"), None, |ui| {
        let own = scope::owner(ui, window, &scoped);
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
        busy(ui, &tr!("hollow-looking"), None, false);
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

/// What the Shell picker offers: the two ways to measure a wall, and the inside shell
/// opened through the floor it stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shell {
    Inside,
    Outside,
    BottomThrough,
}

/// Which surface the wall is measured from, and whether a shell is open through its floor.
fn modes(ui: &mut egui::Ui, tools: &mut Tools) {
    let segments = [
        Segment::new(Shell::Inside, tr!("hollow-inside")),
        Segment::new(Shell::Outside, tr!("hollow-outside")),
        Segment::new(Shell::BottomThrough, tr!("hollow-bottom-through")),
    ];
    let state = &mut tools.hollow.state;
    let mut chosen = match (state.mode, state.open_floor) {
        (HollowMode::External, _) => Shell::Outside,
        (HollowMode::Internal, false) => Shell::Inside,
        (HollowMode::Internal, true) => Shell::BottomThrough,
    };
    let width = ui.available_width();
    if Segmented::new(&segments).width(width).show(ui, &mut chosen) {
        state.mode = match chosen {
            Shell::Outside => HollowMode::External,
            Shell::Inside | Shell::BottomThrough => HollowMode::Internal,
        };
        state.open_floor = chosen == Shell::BottomThrough;
    }

    describe(
        ui,
        &match chosen {
            Shell::Inside => tr!("hollow-inside-note"),
            Shell::Outside => tr!("hollow-outside-note"),
            Shell::BottomThrough => tr!("hollow-bottom-through-note"),
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
/// Bounds the lattice: cavity cost follows area over spacing squared, not spacing alone.
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
        let mut percent = infill.density * 100.0;
        let range = MIN_DENSITY * 100.0..=MAX_DENSITY * 100.0;
        let density = tr!("hollow-density");
        number_row(ui, &density, &mut percent, "%", COARSE_STEP, range, 0);
        infill.density = percent / 100.0;
        thickness(ui, infill);

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

/// How thick the walls or struts the cell and density make come out.
fn thickness(ui: &mut egui::Ui, infill: &InfillSettings) {
    let wall_mm = infill.wall_mm();
    let mm = format!("{wall_mm:.2}");
    if wall_mm < THIN_INFILL_MM {
        tone(
            ui,
            &tr!("hollow-infill-thin", mm = mm),
            theme::colors().warn,
        );
    } else if infill.pattern.has_walls() {
        hint(ui, &tr!("hollow-infill-walls-are", mm = mm));
    } else {
        hint(ui, &tr!("hollow-infill-struts-are", mm = mm));
    }
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
    let button = primary_button(ui, icon::HOLLOW, &tr!("hollow-hollow"), blocked.is_none());
    if blocked_hover(button, blocked).clicked() {
        let started = window.tools.hollow.start(&window.doc.scene);
        window
            .machine
            .status
            .report(&tr!("hollow-hollowing"), started);
    }
}

/// The progress bar and the cancel button of the run that is going. The Drain tool shows
/// it too, since a channel dug into a hollow model runs the shell again.
pub(super) fn running(ui: &mut egui::Ui, job: &HollowJob) {
    tone(ui, &job.label(), theme::colors().text_mid);
    progress_bar(ui, job.fraction());
    if let Some(models) = job.models() {
        tone(ui, &models, theme::colors().text_low);
    }

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
