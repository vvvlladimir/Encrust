//! The window in the states the tests that draw it start from: on a real machine, with or
//! without a cube picked on the plate.

use std::path::PathBuf;
use std::sync::Arc;

use egui_kittest::{Harness, HarnessBuilder};

use super::SlicerApp;
use crate::files::Handed;
use crate::ui::theme;

/// The machine the panels are pictured with, so its volume and its fields are a real one.
pub(super) const PRINTER: &str = "elegoo-saturn-4-ultra";

/// Frames enough for every fold, fade and knob to come to rest.
pub(super) const SETTLE_FRAMES: usize = 30;

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
pub(super) fn app(cube: bool) -> SlicerApp {
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

/// `app` drawn as the window would, by `builder`, until it comes to rest.
pub(super) fn drawn(
    builder: HarnessBuilder<SlicerApp>,
    app: SlicerApp,
) -> Harness<'static, SlicerApp> {
    // The faces a context is given reach it a frame later, so the first frame only dresses
    // it and draws nothing.
    let mut dressed = false;
    let mut harness = builder.build_ui_state(
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
    harness
}

/// The pictured machine and two resins on it, in a directory of the test's own, so the
/// pictures never read or write the user's profiles.
pub(super) fn with_machines(mut app: SlicerApp, name: &str) -> SlicerApp {
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
