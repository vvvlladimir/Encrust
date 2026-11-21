use core_edit::{Edit, ObjectId, OrientAim};
use core_engine::PREPARE_REPAIR;
use core_geometry::{Aabb, Scalar, Vec3};
use core_volume::{HoleKind, HollowMode, PLUG_GAP_MM};

use super::{NotAnEdit, edits_of};
use crate::decide::Resolved;
use crate::recipe::StepKind;
use crate::resolved::ResolvedSettings;

const OBJECT: ObjectId = ObjectId(7);
const AREA: ([Scalar; 2], [Scalar; 2]) = ([5.0, 3.0], [200.0, 120.0]);

fn settings(pairs: &[(&str, Resolved)]) -> ResolvedSettings {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

fn word(text: &str) -> Resolved {
    Resolved::Word(text.to_owned())
}

fn edits(kind: StepKind, set: &ResolvedSettings) -> Result<Vec<Edit>, NotAnEdit> {
    let placed = Aabb::new(Vec3::ZERO, Vec3::splat(10.0));
    edits_of(kind, set, OBJECT, placed, AREA, 0.0)
}

#[test]
fn repair_with_no_method_is_the_prepare_default() {
    let got = edits(StepKind::Repair, &settings(&[])).expect("repair is an edit");
    assert_eq!(
        got,
        vec![Edit::Repair {
            object: OBJECT,
            method: PREPARE_REPAIR
        }]
    );
}

#[test]
fn orient_settings_land_in_the_edit() {
    let set = settings(&[
        ("goal", word("least-peel")),
        ("max_tilt_deg", Resolved::Number(30.0)),
        ("clean_surface", Resolved::Bool(true)),
    ]);
    let got = edits(StepKind::Orient, &set).expect("orient is an edit");
    assert_eq!(
        got,
        vec![Edit::Orient {
            object: OBJECT,
            goal: OrientAim::LeastPeel,
            max_tilt_deg: Some(30.0),
            clean_surface: true,
            pick: 0,
            plate_mm: Some(AREA.1),
            low_mm: AREA.0,
            printer: None,
            resin: None,
        }]
    );
}

#[test]
fn hollow_settings_land_in_the_edit() {
    let set = settings(&[
        ("wall_mm", Resolved::Number(2.5)),
        ("mode", word("open-floor")),
        ("infill", Resolved::Bool(true)),
        ("keep_solid_under_mm", Resolved::Number(0.4)),
    ]);
    let got = edits(StepKind::Hollow, &set).expect("hollow is an edit");
    let [Edit::Hollow { settings, .. }] = got.as_slice() else {
        panic!("one hollow edit, got {got:?}");
    };
    assert_eq!(settings.thickness_mm, 2.5);
    assert_eq!(settings.mode, HollowMode::Internal);
    assert!(settings.open_floor && settings.infill_on);
    assert_eq!(settings.solid_under_mm, Some(0.4));
}

#[test]
fn a_plug_gap_has_the_resin_drift_taken_off() {
    let set = settings(&[("plugs", Resolved::Bool(true))]);
    let placed = Aabb::new(Vec3::ZERO, Vec3::splat(10.0));
    let got = edits_of(StepKind::Drain, &set, OBJECT, placed, AREA, 0.05).expect("an edit");

    let Some(Edit::Plugs { gap_mm, .. }) = got.last() else {
        panic!("a drain then plugs, got {got:?}");
    };
    assert!(
        (*gap_mm - (PLUG_GAP_MM - 0.05)).abs() < 1e-6,
        "0.15 less a drift of 0.05 is 0.10, got {gap_mm}"
    );
}

#[test]
fn auto_drain_asks_the_search_and_a_plug() {
    let set = settings(&[
        ("diameter_mm", Resolved::Number(4.0)),
        ("depth_mm", Resolved::Number(5.0)),
        ("vent", Resolved::Bool(false)),
        ("plugs", Resolved::Bool(true)),
    ]);
    let got = edits(StepKind::Drain, &set).expect("drain is an edit");
    let [
        Edit::Drain {
            size,
            auto,
            vent,
            at_mm,
            ..
        },
        plugs,
    ] = got.as_slice()
    else {
        panic!("a drain then plugs, got {got:?}");
    };
    assert!(*auto && !*vent && at_mm.is_empty());
    assert_eq!((size.diameter_mm, size.depth_mm), (4.0, 5.0));
    assert_eq!(
        *plugs,
        Edit::Plugs {
            object: OBJECT,
            gap_mm: PLUG_GAP_MM,
            plate_mm: Some(AREA.1),
            low_mm: AREA.0,
        }
    );
}

#[test]
fn drain_points_are_fractions_of_the_placed_box() {
    let set = settings(&[
        ("at", word("points")),
        ("points", Resolved::Points(vec![[0.5, 0.5, 1.0]])),
    ]);
    let got = edits(StepKind::Drain, &set).expect("drain is an edit");
    let [
        Edit::Drain {
            at_mm, auto, kind, ..
        },
    ] = got.as_slice()
    else {
        panic!("one drain edit with no plug, got {got:?}");
    };
    assert!(!*auto);
    assert_eq!(*kind, HoleKind::AtDepth, "a point is drilled as a click is");
    let at = at_mm[0];
    assert!(
        (at - Vec3::new(5.0, 5.0, 10.0)).length() < 1e-6,
        "expected (5, 5, 10), got {at:?}"
    );
}

#[test]
fn supports_settings_land_in_the_edit() {
    let set = settings(&[("profile", word("heavy")), ("cavity", Resolved::Bool(true))]);
    let got = edits(StepKind::Supports, &set).expect("supports is an edit");
    let [
        Edit::Supports {
            profile,
            cavity,
            only_at,
            ..
        },
    ] = got.as_slice()
    else {
        panic!("one supports edit, got {got:?}");
    };
    assert_eq!(profile.as_deref(), Some("heavy"));
    assert!(*cavity && !*only_at);
}

#[test]
fn arrange_takes_the_usable_area_and_the_spacing() {
    let set = settings(&[("spacing_mm", Resolved::Number(6.0))]);
    let got = edits(StepKind::Arrange, &set).expect("arrange is an edit");
    assert_eq!(
        got,
        vec![Edit::Arrange {
            low_mm: [5.0, 3.0],
            plate_mm: [200.0, 120.0],
            clearance_mm: 6.0
        }]
    );
}

#[test]
fn steps_with_no_edit_say_so() {
    let in_the_way = settings(&[("only_in_the_way", Resolved::Bool(true))]);
    assert_eq!(
        edits(StepKind::Arrange, &in_the_way),
        Err(NotAnEdit(StepKind::Arrange))
    );
    for kind in [
        StepKind::Check,
        StepKind::Save,
        StepKind::Gate,
        StepKind::Pause,
    ] {
        assert_eq!(edits(kind, &settings(&[])), Err(NotAnEdit(kind)));
    }
}
