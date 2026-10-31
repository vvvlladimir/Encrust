//! The printers the pictured window finds on the network, and what they report.

use crate::network::{Picture, Reading};
use printer_link::State;

use super::SlicerApp;

/// A board of the pictured model, named `name`, at `192.168.1.last`.
pub(super) fn board(name: &str, id: &str, last: u8) -> net_sdcp::Printer {
    net_sdcp::Printer {
        name: name.to_owned(),
        model: "Saturn 4 Ultra".to_owned(),
        brand: "ELEGOO".to_owned(),
        address: std::net::IpAddr::from([192, 168, 1, last]),
        mainboard_id: id.to_owned(),
        brand_id: "00".to_owned(),
        firmware: "V1.5.2".to_owned(),
        protocol: "V3.0.0".to_owned(),
        transport: net_sdcp::Transport::WebSocket,
    }
}

/// The pictured window with Workshop printing and Studio idle on the network, the profile
/// listing `listing` as (name, address's last byte, board).
pub(super) fn on_the_network(listing: &[(&str, u8, &str)]) -> SlicerApp {
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
    let mut app = listed(super::app(true), listing);
    let network = &mut app.machine.network;
    network.hear("sdcp:aa", Reading::State(printing));
    let extras = network.extras_mut("sdcp:aa");
    extras
        .pictures
        .insert("0e8a".to_owned(), Picture::Decoded(a_plate_picture()));
    extras.camera = Some("rtsp://192.168.1.133:554/video".to_owned());
    extras.history_asked = true;
    extras.history = Some(remembered());
    app
}

/// `app` with Workshop and Studio on the network, Studio idle, and the profile listing
/// `listing` as (name, address's last byte, board).
pub(super) fn listed(mut app: SlicerApp, listing: &[(&str, u8, &str)]) -> SlicerApp {
    let idle = State {
        state: "idle".to_owned(),
        stage: Some("complete".to_owned()),
        file: Some("Hex tray.goo".to_owned()),
        uv_led_c: Some(22.0),
        ..State::default()
    };
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
    app.machine.network.bind_to(
        Some(super::super::states::PRINTER.to_owned()),
        Some(&profile),
    );
    app.machine.network.learn_names([&profile]);
    let network = &mut app.machine.network;
    network.printers = vec![board("Workshop", "aa", 133), board("Studio", "bb", 140)];
    network.hear("sdcp:bb", Reading::State(idle));
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
