use proptest::prelude::*;

use super::*;
use core_geometry::Vec3;
use core_joint::JointKind;
use core_volume::RepairMethod;

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
fn a_repair_and_a_mend_written_by_hand_read() {
    let repair: Edit = serde_json::from_str(r#"{"edit":"repair","object":2,"method":"wrap"}"#)
        .expect("a repair is valid JSON for an edit");
    let mend: Edit = serde_json::from_str(
        r#"{"edit":"mend","object":2,"flip_faces":[3],"bridge":[[[0,1],[2,3]]]}"#,
    )
    .expect("a mend names only what it changes");
    assert_eq!(
        repair,
        Edit::Repair {
            object: ObjectId(2),
            method: RepairMethod::Wrap,
        }
    );
    assert_eq!(
        mend,
        Edit::Mend {
            object: ObjectId(2),
            flip_faces: vec![3],
            drop_faces: Vec::new(),
            drop_shells: Vec::new(),
            bridge: vec![[[0, 1], [2, 3]]],
        }
    );
}

#[test]
fn a_repair_by_a_mend_method_reads_in_kebab_case_and_in_the_old_case() {
    for name in ["close-holes", "CloseHoles"] {
        let text = format!(r#"{{"edit":"repair","object":2,"method":{{"mend":"{name}"}}}}"#);
        let edit: Edit = serde_json::from_str(&text).expect("a mend method reads");
        assert_eq!(
            edit,
            Edit::Repair {
                object: ObjectId(2),
                method: RepairMethod::Mend(core_geometry::MendMethod::CloseHoles),
            }
        );
    }
}

#[test]
fn a_relief_names_only_what_it_changes_from_the_cli_defaults() {
    let text = r#"{"edit":"relief","object":2,"depth_mm":0.5,"image":"knurl.png"}"#;
    let edit: Edit = serde_json::from_str(text).expect("a relief needs a depth and a picture");
    assert_eq!(
        edit,
        Edit::Relief {
            object: ObjectId(2),
            depth_mm: 0.5,
            image: "knurl.png".into(),
            tile_mm: 12.0,
            angle_deg: 0.0,
            invert: false,
            symmetric: false,
            smoothing_px: 0.0,
            hold_bottom_deg: 0.0,
            hold_top_deg: 0.0,
            precision: 0.5,
        }
    );
}

#[test]
fn a_hollow_written_by_hand_reads_the_settings_it_carries() {
    let text = r#"{"edit":"hollow","object":2,"settings":{"thickness_mm":2.0,"mode":"internal",
        "precision":0.5,"infill_on":false,"infill":{"kind":"hive","cell_mm":6.0,"density":0.3,
        "wall_mm":null}}}"#;
    let parsed = serde_json::from_str::<Edit>(text);
    let Ok(Edit::Hollow { object, settings }) = parsed else {
        panic!("a hollow with its numbers is an edit: {parsed:?}");
    };
    assert_eq!(object, ObjectId(2));
    assert_eq!(settings.thickness_mm, 2.0);
    assert!(!settings.open_floor, "an open floor is left out as off");
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

#[test]
fn a_drain_written_by_hand_defaults_its_size_and_kind() {
    let text = r#"{"edit":"drain","object":2,"at_mm":[[5.0,5.0,10.5]]}"#;
    let parsed = serde_json::from_str::<Edit>(text);
    let Ok(Edit::Drain {
        object,
        at_mm,
        size,
        kind,
        auto,
        vent,
    }) = parsed
    else {
        panic!("a drain with its points is an edit: {parsed:?}");
    };
    assert_eq!(object, ObjectId(2));
    assert_eq!(at_mm.len(), 1);
    assert_eq!(size, core_volume::HoleSize::default());
    assert_eq!(kind, core_volume::HoleKind::AtDepth);
    assert!(!auto, "the search runs only when asked for");
    assert!(vent, "a vent is laid unless it is left out on purpose");
}

#[test]
fn a_searching_drain_written_by_hand_vents_by_default() {
    let text = r#"{"edit":"drain","object":2,"auto":true}"#;
    let parsed: Edit = serde_json::from_str(text).expect("a searching drain needs no points");
    assert!(matches!(
        parsed,
        Edit::Drain {
            auto: true,
            vent: true,
            ..
        }
    ));
}

#[test]
fn a_channel_and_plugs_written_by_hand_take_the_usual_sizes() {
    let channel: Edit = serde_json::from_str(r#"{"edit":"channel","object":2,"points_mm":[]}"#)
        .expect("a channel needs no diameter");
    let plugs: Edit =
        serde_json::from_str(r#"{"edit":"plugs","object":2}"#).expect("plugs need no gap");
    assert!(
        matches!(channel, Edit::Channel { diameter_mm, .. } if diameter_mm == core_volume::CHANNEL_DIAMETER_MM)
    );
    assert!(matches!(plugs, Edit::Plugs { gap_mm, .. } if gap_mm == core_volume::PLUG_GAP_MM));
}

#[test]
fn a_searching_drain_needs_no_points_and_a_channel_and_plugs_read_back() {
    let drain: Edit = serde_json::from_str(r#"{"edit":"drain","object":2,"auto":true}"#)
        .expect("a drain that searches needs no points");
    assert!(matches!(drain, Edit::Drain { auto: true, ref at_mm, .. } if at_mm.is_empty()));
    for edit in [
        Edit::Channel {
            object: ObjectId(2),
            points_mm: vec![Vec3::ZERO, Vec3::new(5.0, 0.0, 1.0)],
            diameter_mm: 2.0,
        },
        Edit::Plugs {
            object: ObjectId(2),
            gap_mm: 0.1,
            plate_mm: Some([100.0, 80.0]),
            low_mm: [2.0, 3.0],
        },
    ] {
        let text = serde_json::to_string(&edit).expect("an edit serialises");
        let back: Edit = serde_json::from_str(&text).expect("what was written reads");
        assert_eq!(back, edit);
    }
}

#[test]
fn an_arrange_reads_back_with_its_plate_and_clearance() {
    let text = r#"{"edit":"arrange","plate_mm":[100.0,80.0],"clearance_mm":2.0}"#;
    let parsed: Edit = serde_json::from_str(text).expect("an arrange names its plate");
    assert_eq!(
        parsed,
        Edit::Arrange {
            low_mm: [0.0, 0.0],
            plate_mm: [100.0, 80.0],
            clearance_mm: 2.0,
        }
    );
}

#[test]
fn supports_written_by_hand_default_to_the_active_group_and_no_points() {
    let text = r#"{"edit":"supports","object":3}"#;
    let parsed = serde_json::from_str::<Edit>(text);
    let Ok(Edit::Supports {
        object,
        profile,
        at_mm,
        only_at,
        cavity,
        shown_at,
        clean_surface,
    }) = parsed
    else {
        panic!("supports with only an object are an edit: {parsed:?}");
    };
    assert_eq!(object, ObjectId(3));
    assert_eq!(profile, None);
    assert!(at_mm.is_empty());
    assert!(!only_at);
    assert!(!cavity, "the cavity is held only when asked");
    assert!(shown_at.is_empty());
    assert!(!clean_surface, "the painted side is read only when asked");
}

#[test]
fn an_orient_written_by_hand_defaults_to_auto_and_the_first_pose() {
    let text = r#"{"edit":"orient","object":2}"#;
    let parsed: Edit = serde_json::from_str(text).expect("an orient names its object");
    assert_eq!(
        parsed,
        Edit::Orient {
            object: ObjectId(2),
            goal: OrientAim::Auto,
            max_tilt_deg: None,
            clean_surface: false,
            pick: 0,
            plate_mm: None,
            low_mm: [0.0, 0.0],
            printer: None,
            resin: None,
        }
    );
}

#[test]
fn every_aim_is_written_as_the_word_that_names_it() {
    for (word, aim) in OrientAim::WORDS {
        let written = serde_json::to_string(&aim).expect("an aim serialises");
        assert_eq!(written, format!("\"{word}\""));
        assert_eq!(OrientAim::named(word), aim);
    }
    assert_eq!(OrientAim::named("whatever"), OrientAim::Auto);
}
