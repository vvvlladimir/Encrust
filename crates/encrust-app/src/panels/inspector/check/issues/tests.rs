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
            (516, "peel · 60 N".to_owned()),
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
        ["island · 1.00 mm²", "hangs over nothing · 3.50 mm²"]
    );
    assert!(
        Filter::Islands.admits(&hanging.kind),
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
    assert_eq!(described, ["suction cup · 25 mm²", "thin · 0.08 mm"]);
}

#[test]
fn previous_and_next_step_to_the_rows_either_side_of_the_layer_in_view() {
    let risks = [island(10, 1.0), island(20, 1.0), island(30, 1.0)];
    let layer =
        |found: Option<Vec<Risk>>| found.and_then(|risks| risks.first().map(|risk| risk.layer));
    assert_eq!(layer(neighbour(&risks, Filter::All, 20, true)), Some(30));
    assert_eq!(layer(neighbour(&risks, Filter::All, 20, false)), Some(10));
    assert_eq!(
        layer(neighbour(&risks, Filter::All, 30, true)),
        None,
        "nothing past the last"
    );
}
