use super::super::tests::{a_printer, listing, printing};
use super::super::watch::Reading;
use super::*;
use printer_link::State;
use printer_profiles::Connection;

fn idle() -> State {
    State {
        state: "idle".to_owned(),
        ..State::default()
    }
}

/// A network bound to one listed Saturn whose board is known but not heard yet.
fn bound(speaks: Connection) -> Network {
    let mut network = Network {
        printers: vec![a_printer("aa")],
        ..Network::default()
    };
    listing(&mut network, speaks, &[("192.168.1.42", None)]);
    network
}

#[test]
fn a_profile_that_sends_on_a_stick_offers_no_picked_file() {
    assert_eq!(bound(Connection::Sdcp).offer_picked(Connection::None), None);
    assert_eq!(Network::default().offer_picked(Connection::Sdcp), None);
}

#[test]
fn a_picked_file_waits_for_the_printer_to_answer() {
    let network = bound(Connection::Sdcp);
    let (name, refused) = network
        .offer_picked(Connection::Sdcp)
        .expect("the profile lists a printer");
    assert_eq!(
        name, "192.168.1.42",
        "not found yet, it is named by its address"
    );
    assert_eq!(refused.as_deref(), Some("192.168.1.42 is not answering"));
}

#[test]
fn a_free_printer_that_answered_takes_a_picked_file() {
    let mut network = bound(Connection::Sdcp);
    network.hear("sdcp:aa", Reading::State(idle()));
    assert_eq!(
        network.offer_picked(Connection::Sdcp),
        Some(("Saturn aa".to_owned(), None))
    );
}

#[test]
fn a_printing_printer_refuses_a_picked_file_saying_what_it_prints() {
    let mut network = bound(Connection::Sdcp);
    network.hear("sdcp:aa", Reading::State(printing("dragon.goo")));
    let (_, refused) = network
        .offer_picked(Connection::Sdcp)
        .expect("the profile lists a printer");
    assert_eq!(refused.as_deref(), Some("Saturn aa is printing dragon.goo"));
}

#[test]
fn a_board_under_a_prusa_profile_refuses_a_picked_file() {
    let mut network = bound(Connection::PrusaLink);
    network.hear("sdcp:aa", Reading::State(idle()));
    let (_, refused) = network
        .offer_picked(Connection::PrusaLink)
        .expect("the profile lists a printer");
    assert!(
        refused.is_some_and(|why| why.contains("SDCP")),
        "it speaks the other protocol"
    );
}
