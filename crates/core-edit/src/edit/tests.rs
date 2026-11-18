use proptest::prelude::*;

use super::*;
use core_joint::JointKind;

#[test]
fn a_cut_written_by_hand_reads_with_the_defaults_it_leaves_out() {
    let text = r#"{"edit": "cut", "object": 3, "axis": "z", "at_mm": 50.0}"#;
    let edit: Edit = serde_json::from_str(text).expect("a bare cut is valid JSON for an edit");
    assert_eq!(
        edit,
        Edit::Cut {
            object: ObjectId(3),
            axis: Axis::Z,
            at_mm: 50.0,
            keep: Keep::Both,
            joints: Joints::default(),
            lay_flat: false,
        }
    );
}

#[test]
fn an_edit_of_an_unknown_kind_is_refused() {
    let text = r#"{"edit": "melt", "object": 3}"#;
    assert!(serde_json::from_str::<Edit>(text).is_err());
}

fn kind() -> impl Strategy<Value = Option<JointKind>> {
    prop::option::of(prop::sample::select(vec![
        JointKind::Pins,
        JointKind::Dowels,
        JointKind::Magnets,
        JointKind::Key,
    ]))
}

proptest! {
    #[test]
    fn an_edit_comes_back_from_json_as_it_went(
        object in 1u64..1_000,
        axis in prop::sample::select(Axis::ALL.to_vec()),
        at_mm in -500.0f32..500.0,
        keep in prop::sample::select(Keep::ALL.to_vec()),
        kind in kind(),
        lay_flat: bool,
    ) {
        let edit = Edit::Cut {
            object: ObjectId(object),
            axis,
            at_mm,
            keep,
            joints: Joints { kind, ..Joints::default() },
            lay_flat,
        };
        let text = serde_json::to_string(&edit).expect("an edit serialises");
        let back: Edit = serde_json::from_str(&text).expect("what was written reads");
        prop_assert_eq!(back, edit);
    }
}
