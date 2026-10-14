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

/// The Export tool with two boards on the network: the one this printer is bound to
/// printing and alone on its card, then both listed while nothing is bound.
#[test]
fn the_printers_on_the_network() {
    use crate::network::{Destination, Picture, Reading};
    use printer_link::State;

    let board = |name: &str, id: &str, last: u8| net_sdcp::Printer {
        name: name.to_owned(),
        model: "Saturn 4 Ultra".to_owned(),
        brand: "ELEGOO".to_owned(),
        address: std::net::IpAddr::from([192, 168, 1, last]),
        mainboard_id: id.to_owned(),
        brand_id: "00".to_owned(),
        firmware: "V1.5.2".to_owned(),
        protocol: "V3.0.0".to_owned(),
        transport: net_sdcp::Transport::WebSocket,
    };
    let printing = State {
        state: "printing".to_owned(),
        busy: true,
        stage: Some("exposing".to_owned()),
        file: Some("Dragon bust.goo".to_owned()),
        progress: Some(0.42),
        layer: Some(336),
        layer_count: Some(800),
        remaining_s: Some(4_920),
        uv_led_c: Some(34.0),
        job_id: Some("0e8a".to_owned()),
        film_lifts: Some(668),
        film_lifts_max: Some(60_000),
        camera: true,
        ..State::default()
    };
    let idle = State {
        state: "idle".to_owned(),
        stage: Some("complete".to_owned()),
        file: Some("Hex tray.goo".to_owned()),
        uv_led_c: Some(22.0),
        ..State::default()
    };
    let on_the_network = |bound: bool| {
        let mut app = app(true);
        app.tool = Tool::Export;
        if let Some(profile) = app.machine.slicing.printer.as_mut() {
            profile.connection = printer_profiles::Connection::Sdcp;
        }
        let network = &mut app.machine.network;
        network.printers = vec![board("Workshop", "aa", 133), board("Studio", "bb", 140)];
        network.hear("sdcp:aa", Reading::State(printing.clone()));
        network.hear("sdcp:bb", Reading::State(idle.clone()));
        let extras = network.extras_mut("sdcp:aa");
        extras
            .pictures
            .insert("0e8a".to_owned(), Picture::Decoded(a_plate_picture()));
        extras.camera = Some("rtsp://192.168.1.133:554/video".to_owned());
        extras.history_asked = true;
        extras.history = Some(remembered());
        if bound {
            network.bind(
                super::states::PRINTER,
                Destination::Printer("sdcp:aa".into()),
            );
        }
        app
    };
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_export_bound", on_the_network(true)));
    results.extend(picture("tool_export_list", on_the_network(false)));
    let mut sheet = on_the_network(true);
    sheet.machine.network.open = Some("sdcp:aa".to_owned());
    results.extend(picture("printer_sheet", sheet));
}

/// A plate as a board pictures it: a dark field with a lit model in the middle.
fn a_plate_picture() -> egui::ColorImage {
    let (width, height) = (96usize, 72usize);
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let inside = (30..66).contains(&x) && (14..58).contains(&y);
            let shade = if inside { 190 } else { 28 };
            pixels.extend_from_slice(&[shade, shade, shade + 10, 255]);
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels)
}

/// Two prints a board remembers: one finished, one stopped part of the way.
fn remembered() -> Vec<printer_link::Task> {
    let now_s = crate::updates::now_s();
    let slice = |layers| {
        Some(net_sdcp::SliceInformation {
            total_layer_numbers: layers,
            ..net_sdcp::SliceInformation::default()
        })
    };
    vec![
        printer_link::Task {
            task_id: "3add".to_owned(),
            task_name: "/media/sda1/Hex tray.goo".to_owned(),
            begin_time: now_s - 90_000,
            end_time: now_s - 86_400,
            task_status: 1,
            already_print_layer: 412,
            slice_information: slice(412),
            ..printer_link::Task::default()
        },
        printer_link::Task {
            task_id: "10f2".to_owned(),
            task_name: "/media/sda1/Bench.goo".to_owned(),
            begin_time: now_s - 3 * 86_400,
            end_time: now_s - 3 * 86_400 + 600,
            task_status: 3,
            already_print_layer: 240,
            slice_information: slice(1710),
            ..printer_link::Task::default()
        },
    ]
}

/// A file waiting on a printer, and what happened last, stacked at the foot of the stage.
#[test]
fn the_notices_at_the_foot_of_the_stage() {
    let mut app = app(true);
    app.machine.network.sent = Some(crate::network::Sent {
        printer: "Workshop".to_owned(),
        filename: "Dragon bust.goo".to_owned(),
    });
    app.machine.status = crate::status::Status::Info("Dragon bust.goo is on Workshop".to_owned());
    picture("notices", app);
}
