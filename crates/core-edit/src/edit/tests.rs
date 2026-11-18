use proptest::prelude::*;

use super::*;
use core_geometry::Vec3;
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
fn a_rotation_written_by_hand_reads() {
    let text = r#"{"edit":"rotate","object":1,"degrees":[90,0,0]}"#;
    let edit: Edit = serde_json::from_str(text).expect("a rotation is valid JSON for an edit");
    assert_eq!(
        edit,
        Edit::Rotate {
            object: ObjectId(1),
            degrees: Vec3::new(90.0, 0.0, 0.0),
        }
    );
}

#[test]
fn a_duplicate_and_a_mirror_written_by_hand_read() {
    let copy: Edit =
        serde_json::from_str(r#"{"edit":"duplicate","object":2,"offset_mm":[20,0,0]}"#)
            .expect("a duplicate is valid JSON for an edit");
    let flip: Edit = serde_json::from_str(r#"{"edit":"mirror","object":2,"axis":"x"}"#)
        .expect("a mirror is valid JSON for an edit");
    assert_eq!(
        copy,
        Edit::Duplicate {
            object: ObjectId(2),
            offset_mm: Vec3::new(20.0, 0.0, 0.0),
        }
    );
    assert_eq!(
        flip,
        Edit::Mirror {
            object: ObjectId(2),
            axis: Axis::X,
        }
    );
}

#[test]
fn a_split_and_a_part_off_written_by_hand_read() {
    let split: Edit = serde_json::from_str(r#"{"edit":"split","object":2}"#)
        .expect("a bare split is valid JSON for an edit");
    let off: Edit = serde_json::from_str(r#"{"edit":"part-off","object":2,"at_mm":[1,2,3]}"#)
        .expect("a part-off is valid JSON for an edit");
    assert_eq!(
        split,
        Edit::Split {
            object: ObjectId(2),
            smallest_mm3: None,
        }
    );
    assert_eq!(
        off,
        Edit::PartOff {
            object: ObjectId(2),
            at_mm: Vec3::new(1.0, 2.0, 3.0),
        }
    );
}

#[test]
fn a_combine_written_by_hand_reads_with_no_direction() {
    let text = r#"{"edit":"combine","objects":[2,5],"op":"difference"}"#;
    let edit: Edit = serde_json::from_str(text).expect("a bare combine is valid JSON");
    assert_eq!(
        edit,
        Edit::Combine {
            objects: vec![ObjectId(2), ObjectId(5)],
            op: Op::Difference,
            toward: None,
        }
    );
}

#[test]
fn a_label_written_by_hand_reads_raised_and_flat_at_the_default_size() {
    let text = r#"{"edit":"label","object":2,"text":"I","at_mm":[5,5,10]}"#;
    let edit: Edit = serde_json::from_str(text).expect("a bare label is valid JSON");
    assert_eq!(
        edit,
        Edit::Label {
            object: ObjectId(2),
            text: "I".to_owned(),
            at_mm: Vec3::new(5.0, 5.0, 10.0),
            height_mm: None,
            depth_mm: None,
            raised: true,
            follow_surface: false,
            angle_deg: 0.0,
            font: None,
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

    #[test]
    fn a_placement_comes_back_from_json_as_it_went(
        object in 1u64..1_000,
        x in -500.0f32..500.0,
        y in -500.0f32..500.0,
        z in -500.0f32..500.0,
        which in 0usize..3,
    ) {
        let (object, v) = (ObjectId(object), Vec3::new(x, y, z));
        let edit = match which {
            0 => Edit::Move { object, by_mm: v },
            1 => Edit::Rotate { object, degrees: v },
            _ => Edit::Scale { object, factors: v },
        };
        let text = serde_json::to_string(&edit).expect("an edit serialises");
        let back: Edit = serde_json::from_str(&text).expect("what was written reads");
        prop_assert_eq!(back, edit);
    }
}
