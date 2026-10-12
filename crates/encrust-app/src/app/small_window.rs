//! A window made smaller than its panels were drawn for: every screen still draws, down to
//! a single point, where a browser's canvas may go even though a desktop window may not.

use egui_kittest::Harness;

use super::SlicerApp;
use super::states::{app, drawn, with_machines};
use crate::workspace::{Mode, Tool};

/// From one point square up to the least a desktop window may be made, `ui::theme`.
const SIZES: [[f32; 2]; 5] = [
    [1.0, 1.0],
    [40.0, 40.0],
    [120.0, 90.0],
    [480.0, 320.0],
    [
        crate::ui::theme::WINDOW_MIN_W,
        crate::ui::theme::WINDOW_MIN_H,
    ],
];

/// Every screen the window has, named: the start page, the plate with each tool open on
/// a picked cube, Preview, and the windows over them.
fn every_screen() -> Vec<(String, SlicerApp)> {
    let started = || {
        let mut app = app(false);
        app.view.options.started = true;
        app
    };
    let mut screens = vec![("start page".to_owned(), app(false))];
    for tool in Tool::ALL {
        let mut app = app(true);
        app.tool = tool;
        screens.push((format!("{tool:?}"), app));
    }
    let mut preview = started();
    preview.mode = Mode::Preview;
    screens.push(("preview".to_owned(), preview));
    let mut mask = started();
    mask.mode = Mode::Preview;
    mask.view.options.mask_only = true;
    screens.push(("mask alone".to_owned(), mask));
    let mut settings = started();
    crate::panels::toggle_settings(&mut settings.machine);
    screens.push(("settings".to_owned(), settings));
    let mut sheet = started();
    sheet.view.options.sheet = true;
    screens.push(("sheet of keys".to_owned(), sheet));
    let mut find = app(true);
    find.view.options.find = true;
    screens.push(("find".to_owned(), find));
    screens.push((
        "machines".to_owned(),
        with_machines(app(false), "small-window"),
    ));
    screens
}

#[test]
fn every_screen_draws_in_a_window_smaller_than_its_panels() {
    for [width, height] in SIZES {
        for (name, app) in every_screen() {
            let builder = Harness::builder().with_size(egui::vec2(width, height));
            // egui's layout asserts on a negative size, which a panel too big for the
            // window asked for before the panels learned to give way.
            let drew = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                drawn(builder, app);
            }));
            assert!(
                drew.is_ok(),
                "{name} panicked in a {width} x {height} window"
            );
        }
    }
}
