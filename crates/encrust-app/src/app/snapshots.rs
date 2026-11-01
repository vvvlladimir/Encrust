//! Pictures of the window's panels, compared with the ones in `tests/snapshots/`. The
//! viewport draws nothing here: its GPU resources are never installed, so these pin the
//! chrome alone. Run by hand, see docs/decisions/0215.

use std::path::PathBuf;

use core_volume::InfillPattern;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotResults};
use printer_profiles::{Bracing, BracingPreset};

use super::SlicerApp;
use super::states::{SETTLE_FRAMES, app, drawn, with_machines, with_nothing};
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
fn the_supports_tool_sizing_automatically() {
    let mut app = app(true);
    app.tool = Tool::Supports;
    let catalogue = app.machine.slicing.catalogue.clone();
    app.tools.supports.set_automatic(true, &catalogue);
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_supports_auto", app));
}

/// Tall enough for the Shape tool to show the bracing's drawing and every number under it.
const TALL_WINDOW: egui::Vec2 = egui::vec2(1280.0, 1200.0);

#[test]
fn the_shape_tool_on_bracing_from_a_preset_and_by_the_numbers() {
    let mut results = SnapshotResults::new();
    let own = Bracing {
        rise_mm: 6.0,
        ..BracingPreset::Tall.bracing()
    };
    for (name, bracing) in [
        ("tool_shape_brace_preset", BracingPreset::Standard.bracing()),
        ("tool_shape_brace_numbers", own),
    ] {
        let mut app = app(true);
        app.tool = Tool::Shape;
        app.tools.supports.profile.bracing = bracing;
        let mut harness = drawn(Harness::builder().with_size(TALL_WINDOW).wgpu(), app);
        harness.get_by_label("Brace").click();
        harness.run_steps(SETTLE_FRAMES);
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}

#[test]
fn the_hollow_tool_with_each_kind_of_infill() {
    let mut results = SnapshotResults::new();
    for (name, pattern) in [
        ("tool_hollow_hive", InfillPattern::Hive),
        ("tool_hollow_scaffold", InfillPattern::Scaffold),
    ] {
        let mut app = app(true);
        app.tool = Tool::Hollow;
        app.tools.hollow.state.infill_on = true;
        app.tools.hollow.state.infill.pattern = pattern;
        results.extend(picture(name, app));
    }
}

#[test]
fn the_repair_tool_wrapping() {
    let mut app = app(true);
    app.tool = Tool::Repair;
    app.tools.repair.state.method = core_volume::RepairMethod::Wrap;
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_repair_wrap", app));
}

#[test]
fn the_repair_tool_picking_faces_and_edges_by_hand() {
    let mut results = SnapshotResults::new();
    for (name, picking) in [
        ("tool_repair_faces", crate::repair::Picking::Faces),
        ("tool_repair_edges", crate::repair::Picking::Edges),
    ] {
        let mut app = app(true);
        app.tool = Tool::Repair;
        app.tools.repair.mode = crate::repair::RepairMode::Faces;
        app.tools.repair.picking = picking;
        results.extend(picture(name, app));
    }
}

#[test]
fn the_repair_tool_listing_two_shells_one_picked() {
    let mut app = app(true);
    app.tool = Tool::Repair;
    let mut two =
        core_geometry::cuboid(core_geometry::Vec3::ZERO, core_geometry::Vec3::splat(20.0));
    let crumb = (
        core_geometry::Vec3::splat(25.0),
        core_geometry::Vec3::splat(26.0),
    );
    two.append(&core_geometry::cuboid(crumb.0, crumb.1));
    let shaped = crate::scene::Shaped::of(std::sync::Arc::new(two));
    app.doc.scene.objects_mut()[0].reshape(shaped);
    let object = &app.doc.scene.objects()[0];
    let shells = &mut app.doc.repairs.shells;
    shells.list(object);
    while shells.poll(&app.doc.scene) {
        std::thread::yield_now();
    }
    shells.toggle(object, 1);
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_repair_shells", app));
}

#[test]
fn the_repair_tool_marking_what_it_found() {
    let mut app = app(true);
    app.tool = Tool::Repair;
    let found = &mut app.doc.scene.objects_mut()[0].summary.diagnostics;
    (found.holes, found.boundary_edges, found.duplicate_faces) = (2, 12, 4);
    let scene = &app.doc.scene;
    app.doc
        .repairs
        .found
        .press(scene, crate::repair::Finding::OpenEdges);
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_repair_found", app));
}

#[test]
fn the_relief_tool_in_each_mode() {
    let mut results = SnapshotResults::new();
    for (name, mode) in [
        ("tool_relief_text", crate::relief::ReliefMode::Text),
        ("tool_relief_combine", crate::relief::ReliefMode::Combine),
    ] {
        let mut app = app(true);
        app.tool = Tool::Relief;
        app.tools.relief.mode = mode;
        results.extend(picture(name, app));
    }
    let mut app = app(true);
    app.tool = Tool::Relief;
    crate::relief::choose_builtin(&mut app.tools.relief, &mut app.machine.status, 2);
    results.extend(picture("tool_relief_picture", app));
}

#[test]
fn the_cut_tool_joining_its_halves() {
    let mut results = SnapshotResults::new();
    for (name, kind) in [
        ("tool_cut_dowels", core_joint::JointKind::Dowels),
        ("tool_cut_key", core_joint::JointKind::Key),
    ] {
        let mut app = app(true);
        app.tool = Tool::Cut;
        app.tools.cut.state.joints.kind = Some(kind);
        results.extend(picture(name, app));
    }
}

#[test]
fn the_preview_of_an_empty_plate() {
    let mut app = app(false);
    app.view.options.started = true;
    app.mode = Mode::Preview;
    picture("preview", app);
}

/// The app with the Settings window open, on the first support profile.
fn with_settings() -> SlicerApp {
    let mut app = app(false);
    app.view.options.started = true;
    crate::panels::toggle_settings(&mut app.machine);
    app
}

#[test]
fn the_settings_window() {
    let mut results = SnapshotResults::new();
    results.extend(picture("settings", with_settings()));
    let mut form = with_settings();
    let settings = &mut form.machine.settings;
    settings.pick_support(&form.machine.slicing.catalogue, "heavy");
    settings.editing_support = true;
    results.extend(picture("settings_support_form", form));
    let mut auto = with_settings();
    let settings = &mut auto.machine.settings;
    settings.pick_auto(&auto.machine.slicing.catalogue, "default");
    settings.editing_support = true;
    results.extend(picture("settings_auto_form", auto));
    // Not General: it names the profile folder, which is a different path on every desk.
    let mut hollow = with_settings();
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

/// Find over a picked model, nothing typed: every tool, action and model by group.
#[test]
fn the_find_palette() {
    let mut app = app(true);
    app.view.options.find = true;
    picture("find", app);
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
    results.extend(picture("machines_none", with_nothing(app(false), false)));
    results.extend(picture("machines_no_resin", with_nothing(app(false), true)));
}

/// This plate's card with two boards on the network: a profile listing one names it on a
/// plain row, one listing both offers the other, an idle one puts Slice and send over the
/// press and Start print once the file landed, and the ring opens the first's sheet.
#[test]
fn the_printers_on_the_network() {
    let (bench_a, bench_b) = (("Bench A", 133, "aa"), ("Bench B", 140, "bb"));
    let mut results = SnapshotResults::new();
    results.extend(picture("plate_send_bound", on_the_network(&[bench_a])));
    results.extend(picture(
        "plate_send_list",
        on_the_network(&[bench_a, bench_b]),
    ));
    results.extend(picture("plate_send_idle", on_the_network(&[bench_b])));
    let mut landed = on_the_network(&[bench_b]);
    landed.machine.network.sent = Some(crate::network::Sent {
        printer: "Bench B".to_owned(),
        filename: "Cube.goo".to_owned(),
    });
    results.extend(picture("plate_send_landed", landed));
    let mut sheet = on_the_network(&[bench_a]);
    sheet.machine.network.open = Some("sdcp:aa".to_owned());
    results.extend(picture("printer_sheet", sheet));
}

/// The pictured window with Workshop printing and Studio idle on the network, the profile
/// listing `listing` as (name, address's last byte, board).
fn on_the_network(listing: &[(&str, u8, &str)]) -> SlicerApp {
    use crate::network::{Picture, Reading};
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
    let mut app = app(true);
    let profile = app
        .machine
        .slicing
        .printer
        .as_mut()
        .expect("the pictured printer is in hand");
    profile.connection = printer_profiles::Connection::Sdcp;
    profile.machines = listing
        .iter()
        .map(|(name, last, board)| printer_profiles::NetworkMachine {
            name: (*name).to_owned(),
            address: format!("192.168.1.{last}"),
            board: Some((*board).to_owned()),
        })
        .collect();
    let profile = profile.clone();
    app.machine
        .network
        .bind_to(Some(super::states::PRINTER.to_owned()), Some(&profile));
    app.machine.network.learn_names([&profile]);
    let network = &mut app.machine.network;
    network.printers = vec![board("Workshop", "aa", 133), board("Studio", "bb", 140)];
    network.hear("sdcp:aa", Reading::State(printing));
    network.hear("sdcp:bb", Reading::State(idle));
    let extras = network.extras_mut("sdcp:aa");
    extras
        .pictures
        .insert("0e8a".to_owned(), Picture::Decoded(a_plate_picture()));
    extras.camera = Some("rtsp://192.168.1.133:554/video".to_owned());
    extras.history_asked = true;
    extras.history = Some(remembered());
    app
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
            begin_s: now_s - 90_000,
            end_s: now_s - 86_400,
            task_status: 1,
            already_print_layer: 412,
            slice_information: slice(412),
            ..printer_link::Task::default()
        },
        printer_link::Task {
            task_id: "10f2".to_owned(),
            task_name: "/media/sda1/Bench.goo".to_owned(),
            begin_s: now_s - 3 * 86_400,
            end_s: now_s - 3 * 86_400 + 600,
            task_status: 3,
            already_print_layer: 240,
            slice_information: slice(1710),
            ..printer_link::Task::default()
        },
    ]
}

/// A file waiting on a printer, a model that came in broken, and what happened last,
/// stacked at the foot of the stage.
#[test]
fn the_notices_at_the_foot_of_the_stage() {
    let mut app = app(true);
    let cube = app.doc.scene.objects_mut().first_mut();
    cube.expect("the pictured plate holds a cube")
        .summary
        .diagnostics
        .boundary_edges = 3;
    app.machine.network.sent = Some(crate::network::Sent {
        printer: "Workshop".to_owned(),
        filename: "Dragon bust.goo".to_owned(),
    });
    app.machine.status = crate::status::Status::Info("Dragon bust.goo is on Workshop".to_owned());
    app.machine.slicing.resin_id = None;
    picture("notices", app);
}

/// The printer's Network tab with two boards of its model listed and named, and a third
/// found on the network still to tick.
#[test]
fn the_printers_a_profile_sends_to() {
    let mut app = with_machines(app(false), "network");
    app.machine.settings.tab = crate::settings::Tab::Network;
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
    app.machine.network.printers = vec![
        board("3D Printer", "aa", 133),
        board("3D Printer", "bb", 140),
        board("3D Printer", "cc", 151),
    ];
    if let Some(draft) = app.machine.settings.printer.as_mut() {
        draft.values.connection = printer_profiles::Connection::Sdcp;
        draft.values.machines = vec![
            printer_profiles::NetworkMachine {
                name: "Bench A".to_owned(),
                address: "192.168.1.133".to_owned(),
                board: Some("aa".to_owned()),
            },
            printer_profiles::NetworkMachine {
                name: "Bench B".to_owned(),
                address: "192.168.1.140".to_owned(),
                board: Some("bb".to_owned()),
            },
        ];
    }
    picture("machines_network", app);
}

#[test]
fn the_chip_opening_its_machines_and_its_resins() {
    let mut results = SnapshotResults::new();
    for (name, part) in [("chip_machines", "Machine, "), ("chip_resins", "Resin, ")] {
        let mut harness = drawn(Harness::builder().with_size(WINDOW).wgpu(), app(true));
        harness.get_by_label_contains(part).click();
        harness.run_steps(SETTLE_FRAMES);
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}
