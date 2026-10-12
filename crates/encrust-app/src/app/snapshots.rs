//! Pictures of the window's panels, compared with the ones in `tests/snapshots/`. The
//! viewport draws nothing here: its GPU resources are never installed, so these pin the
//! chrome alone. Run by hand, see docs/decisions/0215.

use std::path::PathBuf;

use egui_kittest::{Harness, SnapshotResults};

use super::SlicerApp;
use super::states::{app, drawn, with_machines};
use crate::workspace::{Mode, Tool};

/// The size the window opens at, `lib::viewport`.
const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// Draws `app` as the window would and compares it with the picture called `name`. The
/// results fail the test when dropped, so a test of several pictures gathers them first.
fn picture(name: &str, app: SlicerApp) -> SnapshotResults {
    let mut harness = drawn(Harness::builder().with_size(WINDOW).wgpu(), app);
    harness.snapshot(name);
    harness.take_snapshot_results()
}

#[test]
fn the_start_page() {
    let mut app = app(false);
    let yesterday = crate::updates::now_s() - 90_000;
    for name in ["Hex tray.stl", "Dragon bust.encrust", "Bench.encrust"] {
        app.machine
            .recent
            .note(&PathBuf::from("/plates").join(name), yesterday);
    }
    picture("start", app);
}

#[test]
fn an_empty_plate() {
    let mut app = app(false);
    app.view.options.started = true;
    picture("empty_plate", app);
}

#[test]
fn a_picked_model() {
    picture("picked_model", app(true));
}

#[test]
fn every_tool_on_a_picked_model() {
    let mut results = SnapshotResults::new();
    for tool in Tool::ALL {
        let mut app = app(true);
        app.tool = tool;
        results.extend(picture(&format!("tool_{tool:?}").to_lowercase(), app));
    }
}

#[test]
fn the_preview_of_an_empty_plate() {
    let mut app = app(false);
    app.view.options.started = true;
    app.mode = Mode::Preview;
    picture("preview", app);
}

#[test]
fn the_settings_window() {
    let mut results = SnapshotResults::new();
    let mut supports = app(false);
    supports.view.options.started = true;
    crate::panels::toggle_settings(&mut supports.machine);
    results.extend(picture("settings", supports));
    // Not General: it names the profile folder, which is a different path on every desk.
    let mut hollow = app(false);
    hollow.view.options.started = true;
    crate::panels::toggle_settings(&mut hollow.machine);
    hollow
        .machine
        .settings
        .turn_to(crate::settings::Section::Hollow);
    results.extend(picture("settings_hollow", hollow));
}

#[test]
fn the_sheet_of_keys() {
    let mut app = app(false);
    app.view.options.started = true;
    app.view.options.sheet = true;
    picture("sheet", app);
}

#[test]
fn the_machine_and_resin_window() {
    let mut results = SnapshotResults::new();
    results.extend(picture(
        "machines_resins",
        with_machines(app(false), "resins"),
    ));
    let mut machine = with_machines(app(false), "machine");
    machine.machine.settings.tab = crate::settings::Tab::Machine;
    results.extend(picture("machines_machine", machine));
}
