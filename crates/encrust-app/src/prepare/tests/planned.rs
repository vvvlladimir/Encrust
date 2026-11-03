//! Runs by a plan a chat client's recipe makes: each step as given, or left out.

use super::*;

#[test]
fn a_planned_run_leaves_out_the_steps_its_plan_skips() {
    let (mut bench, id) = Bench::with(large_cube());
    let fresh = bench.object(id).clone();
    let plan = Plan {
        hollow: Choice::Skip,
        ..Plan::autopilot()
    };
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    start_planned(doc, tools, machine, plan, (&[fresh], &[])).expect("nothing else runs");
    bench.run_to_end();
    let object = bench.object(id);
    assert!(!object.hollow.is_hollow(), "Hollow was left out");
    let prepared = bench.tools.prepare.prepared(object).expect("said of it");
    assert_eq!(prepared.hollowed, None);
}

#[test]
fn a_planned_turn_turns_the_model_as_the_command_line_would() {
    let (mut bench, id) = Bench::with(large_cube());
    let fresh = bench.object(id).clone();
    let turn = Quat::from_rotation_z(0.5);
    let plan = Plan {
        scale: None,
        keep_solid: Vec::new(),
        orient: Choice::Given(Orient::Turn(turn)),
        repair: Choice::Skip,
        hollow: Choice::Skip,
        drains: Some(Drains {
            at_mm: Vec::new(),
            auto: false,
            vent: false,
            size: None,
            walled: false,
            channels: Vec::new(),
            channel_mm: None,
            support_at: Vec::new(),
        }),
        supports: None,
        cavity: false,
        shape: Shaping::default(),
    };
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    start_planned(doc, tools, machine, plan, (&[fresh], &[])).expect("nothing else runs");
    bench.run_to_end();
    let rotation = bench.object(id).transform.rotation;
    assert!(
        rotation.angle_between(turn) < 1e-4,
        "turned by 0.5 rad about Z as given, found {rotation:?}"
    );
    assert!(bench.object(id).supports.is_empty(), "no supports asked");
}

#[test]
fn a_planned_point_drills_the_surface_nearest_it() {
    let (mut bench, id) = Bench::with(large_cube());
    let fresh = bench.object(id).clone();
    let world = fresh.transform.to_matrix();
    let side = world.transform_point3(Vec3::new(0.0, 20.0, 20.0));
    let plan = Plan {
        scale: None,
        keep_solid: Vec::new(),
        orient: Choice::Skip,
        repair: Choice::Skip,
        hollow: Choice::Skip,
        drains: Some(Drains {
            at_mm: vec![side],
            auto: false,
            vent: false,
            size: Some(core_engine::project::DrainState::round(4.0, 3.0, 1.0)),
            walled: false,
            channels: Vec::new(),
            channel_mm: None,
            support_at: Vec::new(),
        }),
        supports: None,
        cavity: false,
        shape: Shaping::default(),
    };
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    start_planned(doc, tools, machine, plan, (&[fresh], &[])).expect("nothing else runs");
    bench.run_to_end();
    let holes = bench.object(id).hollow.drains();
    assert_eq!(holes.len(), 1, "one hole at the one point");
    assert!(
        (holes[0].diameter_mm - 4.0).abs() < 1e-4,
        "drilled 4 mm across as given, found {}",
        holes[0].diameter_mm
    );
    assert!(
        holes[0].at.x.abs() < 1e-3,
        "on the face x = 0 nearest the point, found {:?}",
        holes[0].at
    );
}

#[test]
fn a_named_support_profile_becomes_the_models_own() {
    let (mut bench, id) = Bench::with(large_cube());
    let fresh = bench.object(id).clone();
    let plan = Plan {
        scale: None,
        keep_solid: Vec::new(),
        orient: Choice::Skip,
        repair: Choice::Skip,
        hollow: Choice::Skip,
        drains: None,
        supports: Some(Stand {
            profile: Some("light".to_owned()),
            ..Stand::default()
        }),
        cavity: false,
        shape: Shaping::default(),
    };
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    start_planned(doc, tools, machine, plan, (&[fresh], &[])).expect("nothing else runs");
    bench.run_to_end();
    let light = &bench
        .machine
        .slicing
        .catalogue
        .support("light")
        .expect("the catalogue ships Light")
        .profile;
    let active = bench.tools.supports.active;
    assert_eq!(bench.object(id).own.supports.get(&active), Some(light));
}

/// A plan that does nothing but what a test sets.
fn nothing() -> Plan {
    Plan {
        scale: None,
        keep_solid: Vec::new(),
        orient: Choice::Skip,
        repair: Choice::Skip,
        hollow: Choice::Skip,
        drains: None,
        supports: None,
        cavity: false,
        shape: Shaping::default(),
    }
}

#[test]
fn a_planned_scale_and_supports_placed_only_by_hand_land_as_given() {
    let (mut bench, id) = Bench::with(large_cube());
    let fresh = bench.object(id).clone();
    let plan = Plan {
        scale: Some(Vec3::splat(0.5)),
        supports: Some(Stand {
            at_mm: vec![Vec3::new(10.0, 10.0, 0.0)],
            only: true,
            ..Stand::default()
        }),
        ..nothing()
    };
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    start_planned(doc, tools, machine, plan, (&[fresh], &[])).expect("nothing else runs");
    bench.run_to_end();
    let object = bench.object(id);
    assert_eq!(object.transform.scale, Vec3::splat(0.5), "halved as given");
    assert_eq!(
        object.supports.point_count(),
        1,
        "the one support placed by hand alone"
    );
}

#[test]
fn a_wall_kept_solid_round_a_point_leaves_a_blocker_there() {
    let (mut bench, id) = Bench::with(large_cube());
    let fresh = bench.object(id).clone();
    let world = fresh.transform.to_matrix();
    let state = core_engine::project::HollowState {
        thickness_mm: 2.0,
        mode: core_volume::HollowMode::Internal,
        precision: 0.5,
        infill_on: false,
        infill: core_volume::InfillSettings::default(),
        blocker_mm: 5.0,
        open_floor: false,
    };
    let plan = Plan {
        hollow: Choice::Given(state),
        keep_solid: vec![world.transform_point3(Vec3::new(2.0, 20.0, 20.0))],
        ..nothing()
    };
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    start_planned(doc, tools, machine, plan, (&[fresh], &[])).expect("nothing else runs");
    bench.run_to_end();
    let object = bench.object(id);
    assert!(object.hollow.is_hollow(), "hollowed as given");
    let blockers = object.hollow.blockers();
    assert_eq!(blockers.len(), 1, "one ball kept solid");
    assert!(
        (blockers[0].radius_mm - 5.0).abs() < 1e-4,
        "at the hollow's 5 mm radius, found {}",
        blockers[0].radius_mm
    );
}
