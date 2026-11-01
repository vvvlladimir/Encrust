use super::*;
use crate::scene::Scene;
use printer_profiles::Raft;

#[test]
fn what_is_remembered_is_what_the_window_is_set_to() {
    let mut slicing = Slicing::default();
    slicing.resin_id = Some("standard-grey".to_owned());
    let prefs = Preferences::of(
        &slicing,
        &Network::default(),
        &UpdatePrefs::default(),
        &Tools::default(),
    );
    assert_eq!(prefs.resin.as_deref(), Some("standard-grey"));
}

/// The printer a profile last sent to, and the board it lists, travel through the
/// file so the next run sends there before any scan has answered.
#[test]
fn the_printer_a_profile_last_sent_to_survives_the_round_trip() {
    let mut network = Network::default();
    let board = a_board();
    let address = board.address.to_string();
    network.restore(vec![board], BTreeMap::new());
    let profile = printer_profiles::PrinterProfile {
        connection: printer_profiles::Connection::Sdcp,
        machines: vec![printer_profiles::NetworkMachine {
            name: "Bench".to_owned(),
            address: address.clone(),
            board: None,
        }],
        ..printer_profiles::PrinterProfile::default()
    };
    network.bind_to(Some("elegoo-saturn-4-ultra".to_owned()), Some(&profile));
    network.choose(address.clone());

    let prefs = Preferences::of(
        &Slicing::default(),
        &network,
        &UpdatePrefs::default(),
        &Tools::default(),
    );
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    let read: Preferences = serde_json::from_str(&text).expect("and read back");

    assert_eq!(read.boards.len(), 1, "the listed board is kept");
    assert_eq!(read.chosen.get("elegoo-saturn-4-ultra"), Some(&address));
}

#[test]
fn what_each_panel_was_switched_to_survives_the_round_trip() {
    let mut tools = Tools::default();
    tools.supports.placing = supports::Placing::Block;
    tools.drain.placing = drain::Placing::Channel;
    tools.measure.target = Target::Planes;
    let prefs = Preferences::of(
        &Slicing::default(),
        &Network::default(),
        &UpdatePrefs::default(),
        &tools,
    )
    .with_panels(Axes::Local, &egui::Context::default());
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    let read: Preferences = serde_json::from_str(&text).expect("and read back");

    let mut tools = Tools::default();
    let mut axes = Axes::Global;
    read.modes.apply(&mut tools, &mut axes);
    assert_eq!(tools.supports.placing, supports::Placing::Block);
    assert_eq!(tools.drain.placing, drain::Placing::Channel);
    assert_eq!(tools.measure.target, Target::Planes);
    assert_eq!(axes, Axes::Local);
}

#[test]
fn a_profile_moved_to_its_own_id_is_followed() {
    let mut prefs = Preferences {
        printer: Some("bench".to_owned()),
        resin: Some("grey".to_owned()),
        chosen: BTreeMap::from([("bench".to_owned(), "192.168.1.42".to_owned())]),
        ..Preferences::default()
    };
    let moved = |kind, from: &str, to: &str| Renamed {
        kind,
        from: from.to_owned(),
        to: to.to_owned(),
    };

    prefs.follow(&[
        moved(Kind::Printer, "bench", "bench-0a1b2c3d"),
        moved(Kind::Resin, "grey", "grey-4e5f6a7b"),
    ]);

    assert_eq!(prefs.printer.as_deref(), Some("bench-0a1b2c3d"));
    assert_eq!(prefs.resin.as_deref(), Some("grey-4e5f6a7b"));
    assert!(
        prefs.chosen.contains_key("bench-0a1b2c3d"),
        "the printer it sends to follows"
    );
    assert!(!prefs.chosen.contains_key("bench"));
}

/// Only the boards a profile lists: one that merely answered a scan announces
/// itself again next run, at whatever address it has then.
#[test]
fn a_board_nobody_chose_is_not_written_to_the_file() {
    let mut network = Network::default();
    network.restore(vec![a_board()], BTreeMap::new());
    let prefs = Preferences::of(
        &Slicing::default(),
        &network,
        &UpdatePrefs::default(),
        &Tools::default(),
    );
    assert!(prefs.boards.is_empty());
}

fn a_board() -> Printer {
    Printer {
        name: "Saturn".to_owned(),
        model: "Saturn 4 Ultra".to_owned(),
        brand: "ELEGOO".to_owned(),
        address: IpAddr::from([192, 168, 1, 42]),
        mainboard_id: "aa".to_owned(),
        brand_id: "00".to_owned(),
        firmware: String::new(),
        protocol: String::new(),
        transport: net_sdcp::Transport::WebSocket,
    }
}

/// A machine that has to be typed in again every run is a machine nobody sends to, so
/// the credentials travel through the file with it.
#[test]
fn a_prusa_machine_survives_the_round_trip_through_the_file() {
    let mut network = Network::default();
    network.add_prusa(Link::digest(
        "sl1.local",
        net_prusalink::DEFAULT_USER,
        "secret",
    ));
    let prefs = Preferences::of(
        &Slicing::default(),
        &network,
        &UpdatePrefs::default(),
        &Tools::default(),
    );
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    let read: Preferences = serde_json::from_str(&text).expect("and read back");
    assert_eq!(read.machines, prefs.machines);
    assert_eq!(
        read.machines.first().map(|link| link.host.as_str()),
        Some("sl1.local")
    );
}

#[test]
fn a_file_written_before_machines_existed_still_reads() {
    let read: Preferences =
        serde_json::from_str(r#"{"format":".goo"}"#).expect("every field of the file is optional");
    assert!(read.machines.is_empty());
    assert!(read.chosen.is_empty());
    assert!(
        read.updates.check,
        "the update check is on in a file written before it existed"
    );
}

#[test]
fn the_preview_divide_and_the_card_widths_survive_the_round_trip() {
    let options = ViewOptions {
        mask_split: 0.62,
        inspector_w: Some(400.0),
        plate_w: 200.0,
        ..ViewOptions::default()
    };
    let prefs = Preferences::default().with_layout(&options);
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    let read: Preferences = serde_json::from_str(&text).expect("and read back");
    assert_eq!(read.mask_split, Some(0.62));
    assert_eq!(read.inspector_w, Some(400.0));
    assert_eq!(read.plate_w, Some(200.0));
}

/// The inspector no longer shares its width with the tool row, so a width dragged before
/// ADR 0382 would leave it oddly wide: it is forgotten.
#[test]
fn a_column_dragged_before_the_tool_row_moved_is_forgotten() {
    let read: Preferences =
        serde_json::from_str(r#"{"left_w": 420.0}"#).expect("an old key is not an error");
    assert_eq!(read.inspector_w, None);
}

/// A wall thickness typed in once is the user's from then on, so it travels through
/// the file beside the machine it was typed for; see ADR 0192.
#[test]
fn the_values_the_tools_were_left_at_survive_the_round_trip() {
    let mut tools = Tools::default();
    tools.hollow.state.thickness_mm = 1.25;
    tools.drain.state.depth_mm = 7.5;
    tools.supports.brush_radius_mm = 4.0;

    let prefs = Preferences::of(
        &Slicing::default(),
        &Network::default(),
        &UpdatePrefs::default(),
        &tools,
    );
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    let read: Preferences = serde_json::from_str(&text).expect("and read back");

    let mut back = Tools::default();
    let mut slicing = Slicing::default();
    read.tools
        .clone()
        .expect("the tool values were written")
        .apply(&mut back, &mut slicing);
    assert_eq!(back.hollow.state.thickness_mm, 1.25);
    assert_eq!(back.drain.state.depth_mm, 7.5);
    assert_eq!(back.supports.brush_radius_mm, 4.0);
}

#[test]
fn the_plates_raft_is_the_next_runs() {
    let mut tools = Tools::default();
    let plate = Raft {
        enabled: !tools.supports.raft.enabled,
        ..tools.supports.raft
    };
    tools.supports.keep_raft(plate, &mut Scene::default());
    let prefs = Preferences::of(
        &Slicing::default(),
        &Network::default(),
        &UpdatePrefs::default(),
        &tools,
    );
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    let back: Preferences = serde_json::from_str(&text).expect("and read back");

    let mut next = Tools::default();
    back.tools
        .expect("the tool values were written")
        .apply(&mut next, &mut Slicing::default());
    assert_eq!(
        next.supports.raft, plate,
        "the switch stays where it was left"
    );
}

#[test]
fn a_file_written_before_the_general_page_opens_on_the_start_page() {
    let read: Preferences =
        serde_json::from_str("{}").expect("every field of the file is optional");
    assert!(read.general.start_page);
    assert!(!read.general.invert_zoom);
    assert!(!read.general.reduce_motion);
    assert_eq!(
        read.general.appearance,
        Appearance::System,
        "an old file follows the system"
    );
}

#[test]
fn how_the_window_behaves_survives_the_round_trip() {
    let general = General {
        start_page: false,
        invert_zoom: true,
        sliced_dir: Some(std::path::PathBuf::from("/prints")),
        language: Some(crate::i18n::Language::Russian),
        reduce_motion: true,
        appearance: Appearance::Light,
    };
    let prefs = Preferences::default().with_general(&general);
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    let read: Preferences = serde_json::from_str(&text).expect("and read back");
    assert_eq!(read.general, general);
}

#[test]
fn the_update_check_survives_the_round_trip() {
    let updates = UpdatePrefs {
        check: true,
        checked_at_s: Some(1_700_000_000),
        skipped: Some("0.3.0".to_owned()),
        noticed: Some("0.3.0".to_owned()),
    };
    let prefs = Preferences::of(
        &Slicing::default(),
        &Network::default(),
        &updates,
        &Tools::default(),
    );
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    let read: Preferences = serde_json::from_str(&text).expect("and read back");
    assert_eq!(read.updates, updates);
}

/// The same text goes to a file on the desktop and to the page's storage in a browser.
#[test]
fn the_keys_the_user_chose_survive_the_round_trip() {
    use crate::shortcuts::Action;
    let mut keys = Keymap::default();
    let chord = egui::KeyboardShortcut::new(
        egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
        egui::Key::K,
    );
    keys.assign(Action::Find, 0, chord);
    let prefs = Preferences::default().with_keys(keys.clone());
    let text = serde_json::to_string(&prefs).expect("the preferences serialise");
    assert!(text.contains(r#""find":["Shift+Cmd+K"]"#), "{text}");
    let read: Preferences = serde_json::from_str(&text).expect("and read back");
    assert_eq!(read.keys, keys);
}

#[test]
fn a_file_written_before_keys_could_be_chosen_keeps_the_tables() {
    let read: Preferences = serde_json::from_str("{}").expect("an empty file reads");
    assert_eq!(read.keys, Keymap::default());
}
