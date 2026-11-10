use super::*;

/// A pull of `force_n` 6 mm off a neck of 2 mm², the piece gyrating at 4 mm.
fn lever_pulling(force_n: f32) -> RiskKind {
    RiskKind::Lever {
        stress_mpa: 30.0,
        lever_mm: 8.0,
        neck_mm2: 2.0,
        force_n,
        gyration_mm: 4.0,
    }
}

#[test]
fn a_heavier_pull_takes_more_supports_and_a_wider_tip_fewer() {
    let heavy = lever_pulling(1.0);
    let light = lever_pulling(0.3);
    let thin = 0.07;
    let wide = 0.28;
    assert!(holds_for_lever(&light, thin) < holds_for_lever(&heavy, thin));
    assert!(holds_for_lever(&heavy, wide) < holds_for_lever(&heavy, thin));
}

#[test]
fn the_supports_counted_bring_the_stress_under_the_limit() {
    let heavy = lever_pulling(1.0);
    let tip_mm2 = 0.28_f64;
    let count = holds_for_lever(&heavy, tip_mm2 as f32);
    assert!(count < MOST_NECK_HOLDS, "{count} is short of the cap");
    let area = 2.0 + count as f64 * tip_mm2;
    let lever = 6.0 * 2.0 / area + 2.0;
    let stress = neck_stress_mpa(1.0, lever, area);
    assert!(
        stress < NECK_LIMIT_MPA,
        "{count} supports leave {stress} MPa"
    );
}

#[test]
fn a_pull_no_supports_can_answer_stops_at_the_cap() {
    let hopeless = lever_pulling(1e6);
    assert_eq!(holds_for_lever(&hopeless, 0.28), MOST_NECK_HOLDS);
}

#[test]
fn only_a_lever_is_counted_supports() {
    assert_eq!(holds_for_lever(&RiskKind::Peel { force_n: 9.0 }, 0.28), 0);
}

#[test]
fn a_unit_pull_on_a_neck_of_one_mm_radius_bears_its_closed_form_stress() {
    let expected = 1.0 / PI + 4.0 / PI;
    let stress = neck_stress_mpa(1.0, 1.0, PI);
    assert!(
        (stress - expected).abs() < 1e-9,
        "1 N, 1 mm off a 1 mm radius neck: expected 5/π = {expected} MPa, got {stress}"
    );
}

#[test]
fn one_support_fewer_than_counted_leaves_the_neck_over_its_margin() {
    let heavy = lever_pulling(1.0);
    let tip_mm2 = 0.28_f64;
    let count = holds_for_lever(&heavy, tip_mm2 as f32);
    assert!(
        count > 1,
        "a heavy pull wants more than one support, got {count}"
    );
    let area = 2.0 + (count - 1) as f64 * tip_mm2;
    let lever = 6.0 * 2.0 / area + 2.0;
    let stress = neck_stress_mpa(1.0, lever, area);
    assert!(
        stress >= NECK_LIMIT_MPA * NECK_MARGIN,
        "{} supports already hold it at {stress} MPa",
        count - 1
    );
}
