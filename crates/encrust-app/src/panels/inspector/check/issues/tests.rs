use super::*;

fn island(layer: usize, area_mm2: f32) -> Risk {
    Risk {
        layer,
        at_mm: [0.0; 2],
        kind: RiskKind::Island { area_mm2 },
    }
}

#[test]
fn islands_on_one_layer_are_one_row_with_their_area_summed() {
    let peel = Risk {
        layer: 515,
        at_mm: [0.0; 2],
        kind: RiskKind::Peel { force_n: 60.0 },
    };
    let risks = [
        island(464, 0.01),
        island(464, 0.01),
        island(515, 1.8),
        island(515, 2.5),
        peel,
    ];
    let rows = rows(&risks);
    let described: Vec<(usize, String)> = rows
        .iter()
        .map(|row| (row.layer + 1, detail(row)))
        .collect();
    assert_eq!(
        described,
        vec![
            (465, "2 islands · 0.02 mm²".to_owned()),
            (516, "2 islands · 4.30 mm²".to_owned()),
            (516, "60 N".to_owned()),
        ]
    );
}

#[test]
fn what_hangs_over_a_removed_island_is_its_own_row_and_asks_for_supports() {
    let hanging = Risk {
        layer: 40,
        at_mm: [0.0; 2],
        kind: RiskKind::Floating { area_mm2: 3.5 },
    };
    let rows = rows(&[island(40, 1.0), hanging]);
    let described: Vec<String> = rows.iter().map(detail).collect();
    assert_eq!(
        described,
        ["1.00 mm²", "hangs over nothing · 3.50 mm²"],
        "a lone island's card names its kind, its row only the area"
    );
    assert!(
        kind_of(&hanging.kind) == IssueKind::Islands,
        "Support every island takes it in"
    );
    assert_eq!(issue_name(&hanging), "Part left hanging");
}

#[test]
fn a_cup_and_a_thin_section_read_as_what_they_measure() {
    let cup = Risk {
        layer: 9,
        at_mm: [0.0; 2],
        kind: RiskKind::SuctionCup {
            area_mm2: 25.0,
            volume_mm3: 1800.0,
        },
    };
    let thin = Risk {
        layer: 20,
        at_mm: [0.0; 2],
        kind: RiskKind::Thin { width_mm: 0.08 },
    };
    let described: Vec<String> = rows(&[cup, thin]).iter().map(detail).collect();
    assert_eq!(described, ["25 mm²", "0.08 mm"]);
}

fn lever(layer: usize) -> Risk {
    Risk {
        layer,
        at_mm: [0.0; 2],
        kind: RiskKind::Lever {
            stress_mpa: 14.0,
            lever_mm: 6.0,
            neck_mm2: 1.0,
            force_n: 3.0,
            gyration_mm: 3.0,
        },
    }
}

fn layer_of(stop: Option<Stop>) -> Option<usize> {
    match stop {
        Some(Stop::Layer { layer, .. }) => Some(layer),
        _ => None,
    }
}

#[test]
fn the_arrows_step_across_every_kind_in_layer_order() {
    let mut risks = [island(30, 1.0), lever(20), island(10, 1.0)];
    risks.sort_by_key(|risk| risk.layer);
    let stops = layer_stops(&risks);
    let from_ten = Some(stops[0]);
    let second = next_stop(&stops, from_ten, 10, true);
    assert_eq!(layer_of(second), Some(20), "the neck between two islands");
    assert!(matches!(
        second,
        Some(Stop::Layer {
            kind: IssueKind::Necks,
            ..
        })
    ));
    assert_eq!(layer_of(next_stop(&stops, second, 20, true)), Some(30));
}

#[test]
fn two_kinds_on_one_layer_are_two_stops() {
    let risks = [island(20, 1.0), lever(20)];
    let stops = layer_stops(&risks);
    assert_eq!(stops.len(), 2);
    assert_eq!(next_stop(&stops, Some(stops[0]), 20, true), Some(stops[1]));
}

#[test]
fn the_arrows_go_round_past_either_end() {
    let risks = [island(10, 1.0), island(20, 1.0)];
    let stops = layer_stops(&risks);
    assert_eq!(next_stop(&stops, Some(stops[1]), 20, true), Some(stops[0]));
    assert_eq!(next_stop(&stops, Some(stops[0]), 10, false), Some(stops[1]));
}

#[test]
fn from_a_slider_moved_by_hand_the_arrows_start_at_the_layers_either_side() {
    let risks = [island(10, 1.0), island(20, 1.0), island(30, 1.0)];
    let stops = layer_stops(&risks);
    assert_eq!(layer_of(next_stop(&stops, None, 15, true)), Some(20));
    assert_eq!(layer_of(next_stop(&stops, None, 15, false)), Some(10));
}

#[test]
fn from_past_the_last_layer_next_starts_over_at_the_first_stop() {
    let stops = layer_stops(&[island(10, 1.0)]);
    assert_eq!(layer_of(next_stop(&stops, None, 400, true)), Some(10));
}

#[test]
fn with_no_issues_the_arrows_go_nowhere() {
    assert_eq!(next_stop(&[], None, 10, true), None);
}
