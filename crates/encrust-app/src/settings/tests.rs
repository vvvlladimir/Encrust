use super::drafts::{Draft, ResinDraft};
use super::library::{
    add_resin, can_delete, delete_resin, duplicate_resin, is_thrown_away_with_the_printer,
    new_printer, new_resin, other_printers, pool_for, remove_resin, rename_resin, resins_of,
    take_resin_off,
};
use super::*;
use crate::undo::Frame;
use printer_profiles::{
    Catalogue, Connection, Kind, MaterialProfile, NetworkMachine, PrinterProfile, PrinterTuning,
    SupportProfile,
};

const RESTING: Frame = Frame {
    settled: true,
    typing: false,
};

/// A draft of one number, edited to each of `edits` and recorded after each.
fn edited(edits: &[u32]) -> Draft<u32> {
    let mut draft = Draft::new("count".to_owned(), 0);
    for edit in edits {
        draft.values = *edit;
        draft.observe(RESTING);
    }
    draft
}

#[test]
fn a_forms_edits_are_taken_back_and_put_back_in_order() {
    let mut draft = edited(&[1, 2]);
    assert!(draft.undo());
    assert_eq!(draft.values, 1);
    assert!(draft.undo());
    assert_eq!(draft.values, 0);
    assert!(!draft.undo(), "nothing was there before the form opened");
    assert!(draft.redo());
    assert!(draft.redo());
    assert_eq!(draft.values, 2);
    assert!(!draft.redo());
}

#[test]
fn a_number_still_being_typed_is_one_edit_once_it_is_left() {
    let mut draft = edited(&[]);
    let typing = Frame {
        typing: true,
        ..RESTING
    };
    for digit in [1, 12, 123] {
        draft.values = digit;
        draft.observe(typing);
    }
    draft.observe(RESTING);
    assert!(draft.undo());
    assert_eq!(draft.values, 0, "the whole number goes in one step");
}

#[test]
fn an_edit_not_yet_recorded_is_the_first_taken_back() {
    let mut draft = edited(&[1]);
    draft.values = 2;
    assert!(draft.undo());
    assert_eq!(draft.values, 1);
    assert!(draft.redo());
    assert_eq!(draft.values, 2);
}

#[test]
fn an_edit_after_an_undo_drops_what_it_took_away() {
    let mut draft = edited(&[1, 2]);
    draft.undo();
    draft.values = 5;
    assert!(!draft.redo(), "the form was edited since");
    draft.observe(RESTING);
    assert!(!draft.redo());
    draft.undo();
    assert_eq!(draft.values, 1);
}

#[test]
fn undo_reaches_only_the_form_in_view() {
    let mut settings = Settings {
        machines: true,
        tab: Tab::Machine,
        support: Some(Draft::new("support".to_owned(), SupportProfile::default())),
        ..Settings::default()
    };
    if let Some(support) = settings.support.as_mut() {
        support.values.name = "Edited".to_owned();
    }
    settings.observe(RESTING);
    assert!(
        !settings.undo(),
        "the support form is not open in this window"
    );
}

#[test]
fn moving_a_resins_measured_height_carries_its_exposure_and_can_be_taken_back() {
    let resin = MaterialProfile {
        layer_height_mm: 0.05,
        exposure_s: 2.5,
        penetration_depth_mm: None,
        ..MaterialProfile::default()
    };
    let mut draft = ResinDraft::new(MARS.to_owned(), "grey".to_owned(), resin);
    draft.set_layer_height(0.1);
    assert!((draft.draft.values.exposure_s - 4.1218).abs() < 1e-3);
    assert!(
        draft.carried().is_some(),
        "the change is shown until touched"
    );

    draft.revert_exposure();
    assert_eq!(draft.draft.values.exposure_s, 2.5);
    assert!((draft.draft.values.layer_height_mm - 0.1).abs() < 1e-6);
    assert!(draft.carried().is_none());
}

const MARS: &str = "elegoo-mars-3-pro";
const SATURN: &str = "elegoo-saturn-4-ultra";

/// The shipped catalogue over a directory of its own, which every write lands in.
fn writable(name: &str) -> Catalogue {
    let dir = std::env::temp_dir().join(format!("encrust-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Catalogue::with_root(&dir).expect("a missing directory reads as empty")
}

/// The same, with one resin the user measured on both machines: nothing ships an
/// exposure, so a test that needs a resin makes one (ADR 0196).
fn with_grey(name: &str) -> Catalogue {
    let mut catalogue = writable(name);
    let mut resin = MaterialProfile {
        name: "Standard grey".to_owned(),
        exposure_s: 2.6,
        ..MaterialProfile::default()
    };
    for (printer, exposure_s) in [(MARS, 3.2), (SATURN, 2.3)] {
        resin.printers.insert(
            printer.to_owned(),
            PrinterTuning {
                exposure_s: Some(exposure_s),
                ..PrinterTuning::default()
            },
        );
    }
    resin.last_printer = Some(MARS.to_owned());
    catalogue
        .save_resin("standard-grey", &resin)
        .expect("the directory is writable");
    catalogue
}

#[test]
fn a_resin_of_another_printer_is_in_the_pool_until_it_is_added_to_this_one() {
    let mut catalogue = writable("installed");
    assert!(
        resins_of(&catalogue, SATURN).next().is_none(),
        "a first run has no resin at all"
    );
    assert!(
        pool_for(&catalogue, SATURN).next().is_none(),
        "and nothing is shipped to put in the pool"
    );

    let mine = new_resin(&mut catalogue, MARS).expect("the directory is writable");
    assert!(
        pool_for(&catalogue, SATURN).any(|(id, _)| id == mine),
        "a resin measured on another machine is what the pool offers"
    );

    add_resin(&mut catalogue, SATURN, &mine).expect("the directory is writable");
    assert!(resins_of(&catalogue, SATURN).any(|(id, _)| id == mine));
    assert!(
        !pool_for(&catalogue, SATURN).any(|(id, _)| id == mine),
        "a resin on this printer is no longer offered for it"
    );
}

/// A resin somebody measured is theirs: off every printer it still waits in the pool,
/// and the pool is where it is deleted from.
#[test]
fn a_measured_resin_off_its_last_printer_waits_in_the_pool() {
    let mut catalogue = with_grey("last-printer");
    for printer in [MARS, SATURN] {
        take_resin_off(&mut catalogue, printer, "standard-grey").expect("writable");
    }
    assert!(resins_of(&catalogue, MARS).next().is_none());
    assert!(
        pool_for(&catalogue, MARS).any(|(id, _)| id == "standard-grey"),
        "on no printer, so every printer is offered it"
    );

    add_resin(&mut catalogue, MARS, "standard-grey").expect("writable");
    assert!(resins_of(&catalogue, MARS).any(|(id, _)| id == "standard-grey"));
}

/// BUG-16: a resin made and never typed into is not worth a file, so taking it off
/// the only printer it was ever on throws it away instead.
#[test]
fn a_resin_nobody_typed_into_goes_with_the_printer_it_was_made_on() {
    let mut catalogue = writable("untouched");
    let id = new_resin(&mut catalogue, MARS).expect("writable");
    assert!(is_thrown_away_with_the_printer(&catalogue, MARS, &id).expect("there"));

    take_resin_off(&mut catalogue, MARS, &id).expect("writable");
    assert!(
        catalogue.resin(&id).is_err(),
        "no file, nothing in the pool"
    );
}

#[test]
fn one_number_typed_into_a_new_resin_keeps_it() {
    let mut catalogue = writable("touched");
    let id = new_resin(&mut catalogue, MARS).expect("writable");
    let mut edited = catalogue.resin(&id).expect("saved").profile.clone();
    edited.printers.insert(
        MARS.to_owned(),
        PrinterTuning {
            exposure_s: Some(2.9),
            ..PrinterTuning::default()
        },
    );
    catalogue.save_resin(&id, &edited).expect("writable");
    assert!(!is_thrown_away_with_the_printer(&catalogue, MARS, &id).expect("there"));

    take_resin_off(&mut catalogue, MARS, &id).expect("writable");
    assert!(
        pool_for(&catalogue, MARS).any(|(other, _)| other == id),
        "a measured exposure waits in the pool"
    );
}

/// What the question before a deletion has to say: who else loses the resin.
#[test]
fn the_printers_a_resin_is_shared_with_are_named() {
    let catalogue = with_grey("shared");
    let others = other_printers(&catalogue, MARS, "standard-grey").expect("there");
    assert_eq!(others, ["Elegoo Saturn 4 Ultra"]);
}

#[test]
fn a_resin_deleted_from_the_pool_leaves_every_printer() {
    let mut catalogue = with_grey("delete-resin");
    delete_resin(&mut catalogue, "standard-grey").expect("writable");
    assert!(catalogue.resin("standard-grey").is_err());
    assert!(resins_of(&catalogue, MARS).next().is_none());
    assert!(resins_of(&catalogue, SATURN).next().is_none());
}

#[test]
fn a_resin_added_to_a_second_printer_is_on_both() {
    let mut catalogue = with_grey("both");
    add_resin(&mut catalogue, SATURN, "standard-grey").expect("the directory is writable");
    assert!(resins_of(&catalogue, SATURN).any(|(id, _)| id == "standard-grey"));
    assert!(
        !pool_for(&catalogue, SATURN).any(|(id, _)| id == "standard-grey"),
        "a resin on this printer is no longer offered for it"
    );
}

#[test]
fn the_window_opens_on_the_resin_the_plate_uses() {
    let catalogue = with_grey("open");
    let mut settings = Settings::default();
    settings.open_machines(&catalogue, Some(MARS), Some("standard-grey"));
    let resin = settings.resin.as_ref().expect("a resin is open");
    assert_eq!(resin.draft.id, "standard-grey");
    assert!(
        (resin.draft.values.exposure_s - 3.2).abs() < 1e-6,
        "the Mars 3 Pro's own exposure for this resin"
    );
}

#[test]
fn an_edit_for_one_printer_leaves_every_other_alone() {
    let catalogue = with_grey("edit");
    let mut settings = Settings::default();
    settings.open_machines(&catalogue, Some(MARS), Some("standard-grey"));
    let resin = settings.resin.as_mut().expect("open");
    let saturn_before = resin.base.for_printer(SATURN).exposure_s;

    resin.draft.values.exposure_s = 4.0;
    assert!(resin.draft.is_dirty());
    let saved = resin.to_saved();
    assert!((saved.for_printer(MARS).exposure_s - 4.0).abs() < 1e-6);
    assert!((saved.for_printer(SATURN).exposure_s - saturn_before).abs() < 1e-6);
}

#[test]
fn a_resins_edge_smoothing_is_saved_for_every_printer() {
    let catalogue = with_grey("edges");
    let mut settings = Settings::default();
    settings.open_machines(&catalogue, Some(MARS), Some("standard-grey"));
    let resin = settings.resin.as_mut().expect("open");

    resin.draft.values.edges.anti_alias = false;
    let saved = resin.to_saved();
    assert!(!saved.edges.anti_alias, "the resin's own default");
    assert!(
        !saved.for_printer(SATURN).edges.anti_alias,
        "and on another printer"
    );
}

#[test]
fn a_resin_taken_off_a_printer_stays_in_the_pool_and_comes_back() {
    let mut catalogue = with_grey("pool");
    remove_resin(&mut catalogue, MARS, "standard-grey").expect("writable");
    assert!(resins_of(&catalogue, MARS).all(|(id, _)| id != "standard-grey"));
    assert!(pool_for(&catalogue, MARS).any(|(id, _)| id == "standard-grey"));
    assert!(
        resins_of(&catalogue, SATURN).any(|(id, _)| id == "standard-grey"),
        "the other printers keep it"
    );

    add_resin(&mut catalogue, MARS, "standard-grey").expect("writable");
    assert!(resins_of(&catalogue, MARS).any(|(id, _)| id == "standard-grey"));
}

#[test]
fn a_duplicate_belongs_to_the_printer_it_was_made_on() {
    let mut catalogue = with_grey("duplicate");
    let id = duplicate_resin(&mut catalogue, MARS, "standard-grey").expect("writable");
    assert!(id.starts_with("standard-grey-copy-"), "{id}");
    let copy = &catalogue.resin(&id).expect("saved").profile;
    assert!(copy.is_tuned_for(MARS) && !copy.is_tuned_for(SATURN));
    assert!(
        (copy.for_printer(MARS).exposure_s - 3.2).abs() < 1e-6,
        "the Mars numbers"
    );
}

#[test]
fn renaming_a_shared_resin_splits_it_off_this_printer() {
    let mut catalogue = with_grey("rename");
    let id = rename_resin(&mut catalogue, MARS, "standard-grey", "Grey fast").expect("writable");
    assert!(id.starts_with("grey-fast-"), "{id}");
    let kept = &catalogue.resin("standard-grey").expect("kept").profile;
    assert_eq!(
        kept.name, "Standard grey",
        "the other printers keep the name"
    );
    assert!(!kept.is_tuned_for(MARS));

    let again = rename_resin(&mut catalogue, MARS, &id, "Grey faster").expect("writable");
    assert_eq!(
        again, id,
        "a resin only this printer has is renamed where it is"
    );
    assert_eq!(
        catalogue.resin(&id).expect("there").profile.name,
        "Grey faster"
    );
}

/// P-11: a copy made by an older release stands over the shipped profile and nothing
/// in it says so, so the form is told what the build ships under the same id.
#[test]
fn a_copy_that_differs_from_the_shipped_machine_carries_it() {
    let mut catalogue = writable("own-copy");
    let shipped = catalogue
        .shipped_printer(SATURN)
        .expect("a shipped machine");
    let edited = PrinterProfile {
        name: "Saturn, as I measured it".to_owned(),
        ..shipped.clone()
    };
    catalogue.save_printer(SATURN, &edited).expect("writable");

    let mut settings = Settings::default();
    settings.pick(&catalogue, Node::Printer(SATURN.to_owned()));
    let draft = settings.printer.as_ref().expect("a printer is open");
    assert_eq!(draft.shipped(), Some(shipped.clone()));

    catalogue.save_printer(SATURN, &shipped).expect("writable");
    settings.pick(&catalogue, Node::Printer(SATURN.to_owned()));
    let draft = settings.printer.as_ref().expect("a printer is open");
    assert_eq!(
        draft.shipped(),
        None,
        "a copy that matches what is shipped has nothing to say"
    );
}

/// The printers a machine is sent to are the user's: listing one is no edit of the model,
/// and taking the shipped numbers back keeps them.
#[test]
fn the_shipped_machine_comes_back_over_the_users_network() {
    let mut catalogue = writable("own-network");
    let shipped = catalogue
        .shipped_printer(SATURN)
        .expect("a shipped machine");
    let listed = PrinterProfile {
        connection: Connection::Sdcp,
        machines: vec![NetworkMachine {
            name: "Bench".to_owned(),
            address: "192.168.1.40".to_owned(),
            board: None,
        }],
        ..shipped.clone()
    };
    catalogue.save_printer(SATURN, &listed).expect("writable");
    let mut settings = Settings::default();
    settings.pick(&catalogue, Node::Printer(SATURN.to_owned()));
    let draft = settings.printer.as_ref().expect("a printer is open");
    assert_eq!(draft.shipped(), None, "a network alone is no own copy");

    let edited = PrinterProfile {
        name: "Saturn, as I measured it".to_owned(),
        ..listed.clone()
    };
    catalogue.save_printer(SATURN, &edited).expect("writable");
    settings.pick(&catalogue, Node::Printer(SATURN.to_owned()));
    let draft = settings.printer.as_ref().expect("a printer is open");
    assert_eq!(
        draft.shipped(),
        Some(listed),
        "the shipped numbers, the printers kept"
    );
}

#[test]
fn a_machine_nobody_shipped_has_no_shipped_profile_behind_it() {
    let mut catalogue = writable("own-machine");
    let id = new_printer(&mut catalogue).expect("writable");
    let mut settings = Settings::default();
    settings.pick(&catalogue, Node::Printer(id));
    assert!(settings.printer.as_ref().expect("open").shipped().is_none());
}

#[test]
fn only_a_profile_nobody_shipped_can_be_deleted() {
    let mut catalogue = writable("delete");
    assert!(!can_delete(&catalogue, Kind::Printer, MARS));
    let id = new_printer(&mut catalogue).expect("writable");
    assert!(can_delete(&catalogue, Kind::Printer, &id));
}

#[test]
fn the_support_page_opens_on_the_profile_a_group_was_taken_from() {
    let catalogue = writable("supports");
    let mut settings = Settings::default();
    settings.open_supports(
        &catalogue,
        &SupportProfile::shipped("heavy").expect("the heavy preset ships"),
    );
    assert_eq!(settings.section, Section::Supports);
    assert_eq!(settings.support.as_ref().expect("open").id, "heavy");
}

#[test]
fn a_shipped_support_profile_opens_over_what_the_build_ships() {
    let mut catalogue = writable("supports-shipped");
    let mut settings = Settings::default();
    settings.pick_support(&catalogue, "heavy");
    let shipped = SupportProfile::shipped("heavy").expect("the heavy preset ships");
    let draft = settings.support.as_ref().expect("open");
    assert_eq!(draft.as_shipped(), Some(&shipped));

    let own = new_support(&mut catalogue, Some("heavy")).expect("writable");
    settings.pick_support(&catalogue, &own);
    let draft = settings.support.as_ref().expect("open");
    assert!(draft.as_shipped().is_none(), "a copy ships nothing");
}

#[test]
fn the_automatic_profile_opens_in_the_support_profiles_place() {
    let catalogue = writable("supports-auto");
    let mut settings = Settings::default();
    settings.pick_auto(&catalogue, "default");
    let draft = settings
        .auto
        .as_ref()
        .expect("the default automatic profile ships");
    assert_eq!(
        draft.as_shipped(),
        AutoProfile::shipped("default").ok().as_ref()
    );

    settings.pick_support(&catalogue, "heavy");
    assert!(settings.auto.is_none(), "a support profile takes its place");
}
