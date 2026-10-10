//! Pictures of the window's panels, compared with the ones in `tests/snapshots/`. The
//! viewport draws nothing here: its GPU resources are never installed, so these pin the
//! chrome alone. Run by hand, see docs/decisions/0215.

use std::path::PathBuf;
use std::sync::Arc;

use egui_kittest::{Harness, SnapshotResults};

use super::SlicerApp;
use crate::files::Handed;
use crate::ui::theme;
use crate::workspace::{Mode, Tool};

/// The machine the panels are pictured with, so its volume and its fields are a real one.
const PRINTER: &str = "elegoo-saturn-4-ultra";

/// The size the window opens at, `lib::viewport`.
const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// Frames enough for every fold, fade and knob to come to rest.
const SETTLE_FRAMES: usize = 30;

/// Twelve facets of a 20 mm cube, as an exporter writes an ASCII STL.
fn cube_stl() -> String {
    let corner = |index: usize| {
        [index & 1, index >> 1 & 1, index >> 2 & 1].map(|bit| if bit == 0 { 0.0 } else { 20.0 })
    };
    let faces: [[usize; 3]; 12] = [
        [0, 2, 3],
        [0, 3, 1],
        [4, 5, 7],
        [4, 7, 6],
        [0, 1, 5],
        [0, 5, 4],
        [1, 3, 7],
        [1, 7, 5],
        [3, 2, 6],
        [3, 6, 7],
        [2, 0, 4],
        [2, 4, 6],
    ];
    let mut text = String::from("solid cube\n");
    for face in faces {
        text.push_str("facet normal 0 0 0\nouter loop\n");
        for index in face {
            let [x, y, z] = corner(index);
            text.push_str(&format!("vertex {x} {y} {z}\n"));
        }
        text.push_str("endloop\nendfacet\n");
    }
    text.push_str("endsolid cube\n");
    text
}

/// The window on the pictured machine, with one cube picked on the plate when `cube`.
fn app(cube: bool) -> SlicerApp {
    let mut app = SlicerApp::default();
    let entry = app
        .machine
        .slicing
        .catalogue
        .printer(PRINTER)
        .expect("the pictured printer ships");
    let profile = entry.profile.clone();
    crate::profiles::apply_printer(&mut app.window(), profile, Some(PRINTER.to_owned()));
    if cube {
        let file = Handed::Bytes {
            name: PathBuf::from("Cube.stl"),
            bytes: Arc::from(cube_stl().into_bytes()),
            beside: Arc::from([]),
        };
        let imported = crate::import::prepare(&file, &app.doc.plate, &mut |_| {})
            .expect("a closed cube imports");
        let id = app.doc.scene.insert(imported);
        app.doc.scene.select(Some(id));
    }
    app
}

/// Draws `app` as the window would and compares it with the picture called `name`. The
/// results fail the test when dropped, so a test of several pictures gathers them first.
fn picture(name: &str, app: SlicerApp) -> SnapshotResults {
    // The faces a context is given reach it a frame later, so the first frame only dresses
    // it and draws nothing.
    let mut dressed = false;
    let mut harness = Harness::builder().with_size(WINDOW).wgpu().build_ui_state(
        move |ui, app: &mut SlicerApp| {
            if dressed {
                app.window().show(ui);
            } else {
                theme::apply(ui.ctx());
                dressed = true;
            }
        },
        app,
    );
    harness.run_steps(SETTLE_FRAMES);
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
fn the_settings_screen() {
    let mut app = app(false);
    crate::panels::toggle_settings(&mut app.machine);
    picture("settings", app);
}

#[test]
fn the_sheet_of_keys() {
    let mut app = app(false);
    app.view.options.started = true;
    app.view.options.sheet = true;
    picture("sheet", app);
}

/// The pictured machine and two resins on it, in a directory of the test's own, so the
/// pictures never read or write the user's profiles.
fn with_machines(mut app: SlicerApp, name: &str) -> SlicerApp {
    use printer_profiles::{Catalogue, MaterialProfile, PrinterTuning, ResinDetails};

    let dir = std::env::temp_dir().join(format!("encrust-snapshots-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut catalogue = Catalogue::with_root(&dir).expect("a missing directory reads as empty");
    let printer = catalogue
        .printer(PRINTER)
        .expect("the pictured printer ships")
        .profile
        .clone();
    catalogue
        .save_printer(PRINTER, &printer)
        .expect("the directory is writable");
    let resins = [
        (
            "standard-grey",
            "Standard grey",
            "Standard",
            [0x8a, 0x8f, 0x96],
            2.2,
        ),
        (
            "abs-like-tough",
            "ABS-like tough",
            "ABS-like",
            [0x5d, 0x6f, 0x7e],
            2.4,
        ),
    ];
    for (id, resin_name, kind, color, exposure_s) in resins {
        let mut resin = MaterialProfile {
            name: resin_name.to_owned(),
            details: ResinDetails {
                kind: kind.to_owned(),
                color,
                ..ResinDetails::default()
            },
            ..MaterialProfile::default()
        };
        let tuning = PrinterTuning {
            exposure_s: Some(exposure_s),
            ..PrinterTuning::default()
        };
        resin.printers.insert(PRINTER.to_owned(), tuning);
        catalogue
            .save_resin(id, &resin)
            .expect("the directory is writable");
    }
    app.machine.slicing.catalogue = catalogue;
    crate::panels::open_machines(&mut app.machine);
    app
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
