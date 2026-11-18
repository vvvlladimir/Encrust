use clap::ValueEnum as _;
use serde_json::json;

use super::*;

fn steps(read: &Read) -> Vec<Step> {
    read.causes.iter().map(|cause| cause.step).collect()
}

#[test]
fn every_symptom_has_a_meaning_and_a_cause_no_check_shows() {
    let diagnosis = diagnose(&json!({}), Symptom::value_variants()).expect("the map reads");
    for read in &diagnosis.symptoms {
        assert!(!read.means.is_empty(), "{} means something", read.symptom);
        assert!(
            !read.causes.is_empty() && read.causes.iter().all(|cause| cause.seen.is_none()),
            "{} keeps a cause for a clean check: {:?}",
            read.symptom,
            read.causes
        );
    }
}

#[test]
fn every_sign_the_map_names_is_read_from_a_report() {
    let map: Map = toml::from_str(MAP).expect("the map reads");
    for sign in map.cause.iter().filter_map(|cause| cause.sign.as_deref()) {
        assert!(shown(sign, &json!({})).is_ok(), "the sign {sign} is read");
    }
    assert!(shown("no-such-sign", &json!({})).is_err());
}

#[test]
fn a_crack_in_a_print_holding_resin_is_put_on_its_drains_first() {
    let report = json!({
        "models": [{"hollow": {"wall_mm": 2.0}}],
        "slicing": {"trapped_pockets": 1, "trapped_mm3": 3185.7},
    });
    let diagnosis = diagnose(&report, &[Symptom::ResinCrack]).expect("the map reads");
    let read = &diagnosis.symptoms[0];
    assert_eq!(steps(read), [Step::Drains, Step::Drains, Step::After]);
    let seen = read.causes[0]
        .seen
        .as_deref()
        .expect("the trapped resin is seen");
    assert!(seen.contains("3186 mm³"), "{seen}");
}

#[test]
fn a_print_off_the_plate_names_the_layer_and_place_of_its_suction_cup() {
    let report = json!({
        "models": [{"supports": {"contacts": 39}}],
        "print": {"cured": {"risks": [
            {"kind": "peel", "layer": 12, "at_mm": [1.0, 2.0, 0.6]},
            {"kind": "suction-cup", "layer": 59, "at_mm": [40.0, 30.5, 2.95]},
        ]}},
    });
    let diagnosis = diagnose(&report, &[Symptom::OffPlate]).expect("the map reads");
    let read = &diagnosis.symptoms[0];
    assert_eq!(steps(read), [Step::Drains, Step::Resin]);
    let seen = read.causes[0].seen.as_deref().expect("the cup is seen");
    assert!(
        seen.contains("layer 59") && seen.contains("40.0, 30.5, 3.0"),
        "{seen}"
    );
}

#[test]
fn a_cause_whose_sign_the_check_does_not_show_is_left_out() {
    let report = json!({"models": [{}]});
    let diagnosis = diagnose(&report, &[Symptom::FaceMark]).expect("the map reads");
    assert_eq!(
        steps(&diagnosis.symptoms[0]),
        [Step::Pose],
        "no supports, so no support marks"
    );
}
