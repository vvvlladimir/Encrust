use super::errands::{Errand, Picture};
use super::reach::{Reach, Trouble, advice};
use super::scan::{PrusaDraft, merged};
use super::send::{is_temporary, temporary_path};
use super::target::{Prusa, Target, resolve};
use super::watch::Reading;
use super::*;
use crate::status::Status;
use core_pipeline::Sl1Flavour;
use core_pipeline::SlicedFormat;
use net_prusalink::Link;
use net_sdcp::Printer;
use printer_link::{Found, State};
use printer_profiles::{Connection, NetworkMachine, PrinterProfile};
use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::path::Path;
use std::sync::mpsc::{self};

fn a_printer(id: &str) -> Printer {
    Printer {
        name: format!("Saturn {id}"),
        model: "Saturn 4 Ultra".into(),
        brand: "ELEGOO".into(),
        address: IpAddr::from([192, 168, 1, 42]),
        mainboard_id: id.into(),
        brand_id: "00".into(),
        firmware: String::new(),
        protocol: String::new(),
        transport: net_sdcp::Transport::WebSocket,
    }
}

fn at(last: u8, printer: Printer) -> Printer {
    Printer {
        address: IpAddr::from([192, 168, 1, last]),
        ..printer
    }
}

/// Stands `network` under a profile `saturn` speaking `speaks` and listing `machines`
/// as `(address, board)`.
fn listing(network: &mut Network, speaks: Connection, machines: &[(&str, Option<&str>)]) {
    let profile = PrinterProfile {
        connection: speaks,
        machines: machines
            .iter()
            .map(|(address, board)| NetworkMachine {
                name: String::new(),
                address: (*address).to_owned(),
                board: board.map(str::to_owned),
            })
            .collect(),
        ..PrinterProfile::default()
    };
    network.bind_to(Some("saturn".to_owned()), Some(&profile));
}

/// A Saturn's board listed in a profile switched to PrusaLink: it still answers and
/// says so, and only sending to it is refused.
#[test]
fn a_board_listed_under_another_protocol_is_found_but_not_sent_to() {
    let mut network = Network {
        printers: vec![a_printer("aa")],
        seen: BTreeSet::from(["sdcp:aa".to_owned()]),
        asked: true,
        ..Network::default()
    };
    listing(
        &mut network,
        Connection::PrusaLink,
        &[("192.168.1.42", Some("aa"))],
    );
    let machine = network
        .chosen_machine()
        .expect("one printer is listed")
        .clone();
    assert_eq!(network.reach_of(&machine), Reach::Answered);
    let target = network.target().expect("the board is found");
    assert_eq!(target.protocol(), Connection::Sdcp);
    assert!(!target.speaks(Connection::PrusaLink));
}

fn a_prusa(host: &str) -> Prusa {
    Prusa {
        link: Link::digest(host, net_prusalink::DEFAULT_USER, "secret"),
        text: None,
    }
}

#[test]
fn a_listed_printer_is_found_by_its_board_id_wherever_it_answers() {
    let mut network = Network {
        printers: vec![a_printer("aa"), at(77, a_printer("bb"))],
        ..Network::default()
    };
    listing(
        &mut network,
        Connection::Sdcp,
        &[("192.168.1.9", Some("bb"))],
    );
    assert_eq!(
        network.target().map(|target| target.name()),
        Some("Saturn bb".to_owned()),
        "the board moved to .77 and still answers by its id"
    );
    listing(&mut network, Connection::Sdcp, &[("192.168.1.42", None)]);
    assert_eq!(network.target().map(|t| t.name()), Some("Saturn aa".into()));
    listing(&mut network, Connection::Sdcp, &[]);
    assert!(
        network.target().is_none(),
        "a profile listing nothing writes a file"
    );
}

#[test]
fn the_two_protocols_cannot_collide_on_a_key() {
    let network = Network {
        printers: vec![a_printer("sl1.local")],
        prusa: vec![a_prusa("sl1.local")],
        ..Network::default()
    };
    let keys: Vec<String> = network
        .targets()
        .iter()
        .map(|target| target.key())
        .collect();
    assert_eq!(keys, vec!["sdcp:sl1.local", "prusa:sl1.local"]);
}

/// A board that is switched off between two scans is still the board this profile
/// prints on, so it stays listed rather than sending the user back to a file.
#[test]
fn a_board_that_did_not_answer_a_scan_stays_the_destination() {
    let (sender, receiver) = mpsc::channel();
    let mut network = Network {
        printers: vec![a_printer("aa")],
        scan: Some(receiver),
        hold: true,
        ..Network::default()
    };
    listing(
        &mut network,
        Connection::Sdcp,
        &[("192.168.1.42", Some("aa"))],
    );
    sender
        .send(Found::default())
        .expect("the scan holds the receiver");
    assert!(!network.poll(&mut Status::default()));
    assert_eq!(network.target().map(|t| t.key()), Some("sdcp:aa".into()));
}

#[test]
fn a_board_that_answered_from_a_new_address_is_listed_once_at_that_address() {
    let moved = Printer {
        address: IpAddr::from([192, 168, 1, 77]),
        ..a_printer("aa")
    };
    let printers = merged(vec![a_printer("aa")], vec![moved, a_printer("bb")]);
    assert_eq!(printers.len(), 2);
    assert_eq!(printers[0].address, IpAddr::from([192, 168, 1, 77]));
    assert_eq!(printers[1].mainboard_id, "bb");
}

#[test]
fn a_board_gone_quiet_is_dropped_when_another_answers_from_its_address() {
    let printers = merged(vec![a_printer("aa")], vec![a_printer("bb")]);
    let ids: Vec<&str> = printers.iter().map(|p| p.mainboard_id.as_str()).collect();
    assert_eq!(ids, ["bb"], "both boards stood at 192.168.1.42");
}

#[test]
fn an_address_taken_off_a_list_is_no_longer_asked() {
    let address = IpAddr::from([192, 168, 1, 42]);
    let mut network = Network {
        manual: vec![address],
        silent: vec![(address, None)],
        hold: true,
        ..Network::default()
    };
    network.forget(" 192.168.1.42 ");
    assert!(network.manual.is_empty() && network.silent.is_empty());
}

#[test]
fn a_listed_board_is_found_by_its_id_and_not_by_another_at_its_address() {
    let printers = [a_printer("aa"), at(77, a_printer("bb"))];
    let machine = NetworkMachine {
        name: String::new(),
        address: "192.168.1.42".into(),
        board: Some("bb".into()),
    };
    let found = resolve(&printers, &[], &machine).map(|target| target.key());
    assert_eq!(
        found,
        Some("sdcp:bb".into()),
        "bb moved to .77, aa stands at .42"
    );
}

/// Each profile lists its own printers and keeps the one it last sent to, so switching
/// profiles puts each back on its own machine.
#[test]
fn each_printer_profile_keeps_the_printer_it_last_sent_to() {
    let mut network = Network {
        printers: vec![at(10, a_printer("aa")), at(11, a_printer("bb"))],
        ..Network::default()
    };
    let both = [("192.168.1.10", None), ("192.168.1.11", None)];
    listing(&mut network, Connection::Sdcp, &both);
    assert_eq!(
        network.target().map(|t| t.name()),
        Some("Saturn aa".into()),
        "nothing chosen yet sends to the first listed"
    );
    network.choose("192.168.1.11".to_owned());
    assert_eq!(network.target().map(|t| t.name()), Some("Saturn bb".into()));

    let profile = PrinterProfile::default();
    network.bind_to(Some("mars".to_owned()), Some(&profile));
    assert!(network.target().is_none(), "mars lists no printer");
    listing(&mut network, Connection::Sdcp, &both);
    assert_eq!(network.target().map(|t| t.name()), Some("Saturn bb".into()));
}

/// The dot beside Send says what the last scan found, and tells a board that was
/// asked and said nothing from one nobody has asked yet.
#[test]
fn a_board_is_reachable_only_while_it_keeps_answering() {
    let (sender, receiver) = mpsc::channel();
    let mut network = Network {
        printers: vec![a_printer("aa"), at(43, a_printer("bb"))],
        scan: Some(receiver),
        hold: true,
        ..Network::default()
    };
    let answered = Target::Sdcp(&network.printers[0].clone());
    assert_eq!(network.reach(&answered), Reach::Unasked);

    sender
        .send(Found {
            boards: vec![a_printer("aa")],
            ..Found::default()
        })
        .expect("the scan holds the receiver");
    assert!(!network.poll(&mut Status::default()));

    let printers = network.printers.clone();
    let (first, second) = (Target::Sdcp(&printers[0]), Target::Sdcp(&printers[1]));
    assert_eq!(network.reach(&first), Reach::Answered);
    assert_eq!(
        network.reach(&second),
        Reach::Silent,
        "a board still listed because a profile is bound to it did not answer"
    );
}

#[test]
fn a_prusa_machine_that_was_set_up_stays_a_destination_through_a_scan() {
    let (sender, receiver) = mpsc::channel();
    let mut network = Network {
        prusa: vec![a_prusa("sl1.local")],
        scan: Some(receiver),
        hold: true,
        ..Network::default()
    };
    listing(&mut network, Connection::PrusaLink, &[("sl1.local", None)]);
    sender
        .send(Found {
            reached: BTreeMap::from([("sl1.local".to_owned(), "Prusa SLA 1.8.0".to_owned())]),
            ..Found::default()
        })
        .expect("the scan holds the receiver");
    assert!(!network.poll(&mut Status::default()));
    assert_eq!(
        network.target().map(|target| target.detail()),
        Some("Prusa SLA 1.8.0".to_owned())
    );
}

#[test]
fn a_prusa_machine_is_only_sent_the_container_it_prints() {
    let mut network = Network {
        prusa: vec![a_prusa("sl1.local")],
        ..Network::default()
    };
    listing(&mut network, Connection::PrusaLink, &[("sl1.local", None)]);
    assert!(network.blocker(SlicedFormat::Goo).is_some());
    assert!(
        network
            .blocker(SlicedFormat::Sl1(Sl1Flavour::Sl1s))
            .is_none()
    );
}

#[test]
fn a_board_is_not_told_what_it_prints_before_it_is_asked() {
    let mut network = Network {
        printers: vec![a_printer("aa")],
        ..Network::default()
    };
    listing(&mut network, Connection::Sdcp, &[("192.168.1.42", None)]);
    assert!(network.blocker(SlicedFormat::Goo).is_none());
}

#[test]
fn setting_up_the_same_host_twice_replaces_it() {
    let mut network = Network::default();
    network.add_prusa(Link::api_key("sl1.local", "first"));
    network.add_prusa(Link::api_key("sl1.local", "second"));
    assert_eq!(network.prusa.len(), 1);
    assert_eq!(
        network.prusa[0].link.auth,
        net_prusalink::Auth::ApiKey("second".to_owned())
    );
}

#[test]
fn forgetting_a_login_leaves_the_listed_machine_unreachable() {
    let mut network = Network {
        prusa: vec![a_prusa("sl1.local")],
        ..Network::default()
    };
    listing(&mut network, Connection::PrusaLink, &[("sl1.local", None)]);
    network.forget_prusa("sl1.local");
    assert!(network.prusa.is_empty());
    assert!(network.target().is_none());
    assert_eq!(network.listed().len(), 1, "the profile still lists it");
}

#[test]
fn a_draft_needs_a_host_and_one_of_the_two_logins() {
    let mut draft = PrusaDraft::default();
    assert!(draft.link().is_none(), "a machine needs somewhere to be");
    draft.host = "sl1.local".to_owned();
    assert!(draft.link().is_none(), "and a way in");
    draft.password = "secret".to_owned();
    assert_eq!(
        draft.link(),
        Some(Link::digest(
            "sl1.local",
            net_prusalink::DEFAULT_USER,
            "secret"
        )),
        "an empty username is the one the firmware generates"
    );
    draft.key = "0123456789abcdef".to_owned();
    assert_eq!(
        draft.link(),
        Some(Link::api_key("sl1.local", "0123456789abcdef")),
        "a key answers on its own, so it wins over a half-typed login"
    );
}

#[test]
fn only_a_file_the_window_wrote_for_itself_is_temporary() {
    assert!(is_temporary(&temporary_path("model.goo")));
    assert!(!is_temporary(Path::new("/Users/someone/model.goo")));
}

#[test]
fn sending_without_a_destination_starts_nothing() {
    let mut network = Network::default();
    network.send(temporary_path("model.goo"), &mut Status::default());
    assert!(network.job.is_none());
}

fn printing(file: &str) -> State {
    State {
        state: "printing".to_owned(),
        busy: true,
        file: Some(file.to_owned()),
        progress: Some(0.5),
        ..State::default()
    }
}

#[test]
fn a_bound_printer_in_the_middle_of_a_print_holds_the_send_back() {
    let mut network = Network {
        printers: vec![a_printer("aa")],
        ..Network::default()
    };
    listing(&mut network, Connection::Sdcp, &[("192.168.1.42", None)]);
    assert_eq!(
        network.busy(),
        None,
        "nothing heard yet is nothing to hold back"
    );
    network.hear("sdcp:aa", Reading::State(printing("dragon.goo")));
    assert_eq!(
        network.busy().as_deref(),
        Some("Saturn aa is printing dragon.goo")
    );
    let idle = State {
        state: "idle".to_owned(),
        ..State::default()
    };
    network.hear("sdcp:aa", Reading::State(idle));
    assert_eq!(network.busy(), None);
}

/// The dot follows the watch once it has heard anything, rather than a scan that may
/// be half a minute old.
#[test]
fn what_the_watch_heard_overrides_the_last_scan() {
    let mut network = Network {
        printers: vec![a_printer("aa")],
        ..Network::default()
    };
    let trouble = Trouble {
        message: "network call failed".to_owned(),
        unreachable: false,
    };
    network.hear("sdcp:aa", Reading::Failed(trouble));
    let printers = network.printers.clone();
    assert_eq!(network.reach(&Target::Sdcp(&printers[0])), Reach::Silent);
    network.hear("sdcp:aa", Reading::State(printing("cube.goo")));
    assert_eq!(network.reach(&Target::Sdcp(&printers[0])), Reach::Answered);
}

#[test]
fn a_broadcast_that_went_nowhere_is_kept_with_why() {
    let (sender, receiver) = mpsc::channel();
    let mut network = Network {
        scan: Some(receiver),
        manual: vec![IpAddr::from([192, 168, 1, 42])],
        hold: true,
        ..Network::default()
    };
    let no_route = std::io::Error::from(std::io::ErrorKind::HostUnreachable);
    sender
        .send(Found {
            broadcast: Some(net_sdcp::SdcpError::Io(no_route).into()),
            silent: vec![(IpAddr::from([192, 168, 1, 42]), None)],
            ..Found::default()
        })
        .expect("the scan holds the receiver");
    assert!(!network.poll(&mut Status::default()));
    let trouble = network.trouble.clone().expect("the broadcast failed");
    assert!(trouble.unreachable);
    assert!(
        trouble.message.contains("network call failed"),
        "{}",
        trouble.message
    );
    assert_eq!(
        network.silent,
        vec![(IpAddr::from([192, 168, 1, 42]), None)]
    );
}

#[test]
fn only_no_route_on_macos_is_blamed_on_the_privacy_setting() {
    let trouble = Trouble {
        message: String::new(),
        unreachable: true,
    };
    assert_eq!(trouble.advice().is_some(), cfg!(target_os = "macos"));
    assert_eq!(advice(false, true), None);
    assert_eq!(advice(false, false), None);
}

#[test]
fn a_window_started_from_a_terminal_is_told_whose_permission_it_has() {
    let bare = advice(true, false).expect("a blocked window is told what to do");
    assert!(bare.contains("terminal"), "{bare}");
    let bundled = advice(true, true).expect("a blocked app is told what to do");
    assert!(
        !bundled.contains("terminal"),
        "an app is asked about itself: {bundled}"
    );
}

#[test]
fn a_held_window_does_not_ask_the_network_on_its_own() {
    let mut network = Network {
        hold: true,
        ..Network::default()
    };
    network.keep_fresh(0.0);
    assert!(!network.is_scanning());
}

#[test]
fn the_list_in_view_is_asked_again_only_once_it_is_stale() {
    let mut network = Network::default();
    network.keep_fresh(100.0);
    assert!(network.is_scanning(), "the first look goes out at once");
    network.scan = None;
    network.keep_fresh(100.0 + RESCAN_S / 2.0);
    assert!(!network.is_scanning(), "a fresh list is left alone");
    network.keep_fresh(100.0 + RESCAN_S);
    assert!(network.is_scanning());
}

#[test]
fn a_printer_is_called_by_the_name_a_profile_gave_it_everywhere() {
    let mut network = Network {
        printers: vec![a_printer("aa"), at(50, a_printer("bb"))],
        ..Network::default()
    };
    let mars = PrinterProfile {
        machines: vec![
            NetworkMachine {
                name: "Bench A".to_owned(),
                address: "192.168.1.9".to_owned(),
                board: Some("aa".to_owned()),
            },
            NetworkMachine {
                name: "Bench B".to_owned(),
                address: "192.168.1.50".to_owned(),
                board: None,
            },
        ],
        ..PrinterProfile::default()
    };
    network.learn_names([&mars]);
    let printers = network.printers.clone();
    assert_eq!(
        network.label(&Target::Sdcp(&printers[0])),
        "Bench A",
        "by its board"
    );
    assert_eq!(
        network.label(&Target::Sdcp(&printers[1])),
        "Bench B",
        "by its address"
    );
    network.learn_names([]);
    assert_eq!(network.label(&Target::Sdcp(&printers[0])), "Saturn aa");
}

#[test]
fn the_print_under_way_and_each_remembered_one_want_a_picture_once() {
    let mut network = Network {
        printers: vec![a_printer("aa")],
        ..Network::default()
    };
    let state = State {
        job_id: Some("0e8a".to_owned()),
        ..printing("dragon.goo")
    };
    network.hear("sdcp:aa", Reading::State(state));
    let wanted = network.wanted_pictures();
    assert!(
        matches!(&wanted[..], [(key, Errand::Picture { id, task: None })]
                if key == "sdcp:aa" && id == "0e8a"),
        "{wanted:?}"
    );
    let extras = network.extras_mut("sdcp:aa");
    extras.pictures.insert("0e8a".to_owned(), Picture::Missing);
    extras.history = Some(vec![printer_link::Task {
        task_id: "3add".to_owned(),
        thumbnail: "http://192.168.1.42:3030/media/3add.bmp".to_owned(),
        ..printer_link::Task::default()
    }]);
    let wanted = network.wanted_pictures();
    assert!(
        matches!(&wanted[..], [(_, Errand::Picture { id, task: Some(_) })] if id == "3add"),
        "only the remembered print is still missing: {wanted:?}"
    );
}

/// A Prusa machine's status names no file, only the job's id.
#[test]
fn a_prusa_print_wants_its_picture_without_naming_its_file() {
    let mut network = Network {
        prusa: vec![Prusa {
            link: Link::api_key("sl1.local", "key"),
            text: None,
        }],
        ..Network::default()
    };
    let state = State {
        state: "printing".to_owned(),
        busy: true,
        job_id: Some("297".to_owned()),
        ..State::default()
    };
    network.hear("prusa:sl1.local", Reading::State(state));
    let wanted = network.wanted_pictures();
    assert!(
        matches!(&wanted[..], [(key, Errand::Picture { id, task: None })]
                if key == "prusa:sl1.local" && id == "297"),
        "{wanted:?}"
    );
}

#[test]
fn the_ring_stands_for_the_bound_printer_else_the_one_printing_else_the_first() {
    let mut network = Network {
        printers: vec![a_printer("aa"), a_printer("bb")],
        ..Network::default()
    };
    network.hear("sdcp:aa", Reading::State(State::default()));
    assert_eq!(network.current().map(|t| t.key()), Some("sdcp:aa".into()));
    network.hear("sdcp:bb", Reading::State(printing("dragon.goo")));
    assert_eq!(network.current().map(|t| t.key()), Some("sdcp:bb".into()));
    listing(
        &mut network,
        Connection::Sdcp,
        &[("192.168.1.42", Some("aa"))],
    );
    assert_eq!(
        network.current().map(|t| t.key()),
        Some("sdcp:aa".into()),
        "the printer in hand's own machine wins over a busy one"
    );
    assert!(Network::default().current().is_none());
}

/// A board remembered from the last run, or one switched off since, is no printer to
/// show: only what answers is.
#[test]
fn the_ring_lists_only_the_printers_that_answer() {
    let mut network = Network {
        printers: vec![a_printer("aa"), a_printer("bb"), a_printer("cc")],
        seen: BTreeSet::from(["sdcp:bb".to_owned(), "sdcp:cc".to_owned()]),
        asked: true,
        ..Network::default()
    };
    let down = Trouble {
        message: "no route to host".to_owned(),
        unreachable: true,
    };
    network.hear("sdcp:cc", Reading::Failed(down));
    let keys: Vec<String> = network.in_view().iter().map(Target::key).collect();
    assert_eq!(keys, ["sdcp:bb"], "aa kept silent at the scan, cc since");
    assert_eq!(network.current().map(|t| t.key()), Some("sdcp:bb".into()));
    network.hear("sdcp:aa", Reading::State(State::default()));
    assert_eq!(
        network.in_view().len(),
        2,
        "a watch that hears it brings it back"
    );
}
