use core_analysis::{Risk, RiskKind};

use super::*;

fn risk(layer: usize, kind: RiskKind) -> Risk {
    Risk {
        layer,
        at_mm: [0.0; 2],
        kind,
    }
}

fn island(layer: usize) -> Risk {
    risk(layer, RiskKind::Island { area_mm2: 1.0 })
}

fn thin(layer: usize) -> Risk {
    risk(layer, RiskKind::Thin { width_mm: 0.2 })
}

fn card(kind: IssueKind) -> Card {
    Card {
        kind,
        count: 1,
        fixable: 1,
        tint: Color32::RED,
    }
}

#[test]
fn islands_are_fixable_and_thin_sections_are_left_to_the_hand() {
    let cards = layer_cards(&[island(3), island(9), thin(5)], 0, &[]);
    let found: Vec<_> = cards.iter().map(|c| (c.kind, c.count, c.fixable)).collect();
    assert_eq!(found, [(IssueKind::Islands, 2, 2), (IssueKind::Thin, 1, 0)]);
}

#[test]
fn a_kind_whose_fix_had_its_turn_is_left_to_the_hand() {
    let cards = layer_cards(&[island(3)], 0, &[IssueKind::Islands]);
    assert_eq!(cards[0].fixable, 0);
    assert_eq!(cards[0].count, 1);
}

#[test]
fn islands_taken_out_keep_their_card_with_nothing_left_on_it() {
    let cards = layer_cards(&[], 4, &[]);
    assert_eq!(cards.len(), 1);
    assert_eq!((cards[0].kind, cards[0].count), (IssueKind::Islands, 0));
}

#[test]
fn the_first_card_opens_until_one_is_chosen() {
    let cards = [card(IssueKind::InTheWay), card(IssueKind::Cups)];
    assert_eq!(open_card(None, &cards), Some(IssueKind::InTheWay));
    let chosen = Some(Some(IssueKind::Cups));
    assert_eq!(open_card(chosen, &cards), Some(IssueKind::Cups));
}

#[test]
fn a_chosen_card_that_was_answered_gives_way_to_the_next() {
    let cards = [card(IssueKind::Cups)];
    let chosen = Some(Some(IssueKind::Islands));
    assert_eq!(open_card(chosen, &cards), Some(IssueKind::Cups));
}

#[test]
fn a_card_the_user_folded_stays_folded() {
    assert_eq!(open_card(Some(None), &[card(IssueKind::Cups)]), None);
}
