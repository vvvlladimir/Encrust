use super::*;
use crate::scene::ImportSummary;
use core_geometry::{Orientation, Transform, Vec3, diagnose};

fn scene_with_a_model(height_mm: Scalar) -> Scene {
    let mesh = Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, height_mm),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    );
    let mut scene = Scene::default();
    scene.insert(crate::scene::Imported::new(
        "tetra".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ));
    scene
}

#[test]
fn the_summary_states_both_volumes_and_prices_them_only_when_the_resin_has_a_price() {
    let tally = Tally {
        vertices: 22_579,
        triangles: 43_834,
        model_mm3: 6_660.0,
        supports_mm3: 1_520.0,
    };
    let mut resin = MaterialProfile::default();
    let keys = |readings: Vec<(String, String)>| {
        readings.into_iter().map(|(key, _)| key).collect::<Vec<_>>()
    };
    assert_eq!(
        keys(tally_readings(&tally, &resin)),
        ["Vertices", "Triangles", "Model/supports", "Weight"]
    );

    resin.details.price = 30.0;
    let priced = tally_readings(&tally, &resin);
    assert_eq!(priced[2].1, "6.66/1.52 ml");
    assert_eq!(priced[4].0, "Price");
}

fn slicing_on_a_printer() -> Slicing {
    let mut slicing = Slicing::default();
    let printer = slicing
        .catalogue
        .printer("elegoo-saturn-4-ultra")
        .expect("ships")
        .profile
        .clone();
    slicing.set_printer(printer, None);
    slicing
}

#[test]
fn the_plate_is_timed_before_it_is_cut_as_its_layers_would_print() {
    let mut slicing = slicing_on_a_printer();
    slicing.set_layer_height(0.05);
    let scene = scene_with_a_model(1.0);
    let expected = job_over(&slicing, LayerPlan::of_count(0.05, 20))
        .expect("a printer is chosen")
        .print_time_s();

    assert_eq!(estimated_time_s(&scene, &slicing), Some((expected, None)));
    assert!(expected > 0, "twenty layers take some time");
}

#[test]
fn an_adaptive_plate_is_timed_at_both_ends_of_its_range() {
    let mut slicing = slicing_on_a_printer();
    slicing.set_layer_height(0.1);
    slicing.adaptive = Some(core_slicer::AdaptiveSettings {
        min_height_mm: 0.02,
        ..core_slicer::AdaptiveSettings::default()
    });
    let (longest, shortest) =
        estimated_time_s(&scene_with_a_model(1.0), &slicing).expect("a printer is chosen");
    let shortest = shortest.expect("an adaptive stack has a range");
    assert!(
        longest > shortest,
        "fifty thin layers outlast ten thick ones: {longest} s against {shortest} s"
    );
}

#[test]
fn nothing_is_timed_without_a_printer_or_a_model() {
    assert_eq!(
        estimated_time_s(&scene_with_a_model(1.0), &Slicing::default()),
        None
    );
    assert_eq!(
        estimated_time_s(&Scene::default(), &slicing_on_a_printer()),
        None
    );
}

#[test]
fn the_estimate_counts_the_layers_the_model_needs() {
    let mut slicing = Slicing::default();
    slicing.set_layer_height(0.05);
    // One millimetre at 0.05 mm a layer.
    let (height_mm, layers) = stack(&scene_with_a_model(1.0), &slicing);
    assert!((height_mm - 1.0).abs() < 1e-6);
    assert_eq!(layers, (20, None));
}

#[test]
fn an_adaptive_estimate_states_both_ends_of_what_the_settings_allow() {
    let mut slicing = Slicing::default();
    slicing.set_layer_height(0.1);
    slicing.adaptive = Some(core_slicer::AdaptiveSettings {
        min_height_mm: 0.02,
        ..core_slicer::AdaptiveSettings::default()
    });
    // One millimetre is ten layers at the ceiling and fifty at the floor.
    let (_, layers) = stack(&scene_with_a_model(1.0), &slicing);
    assert_eq!(layers, (50, Some(10)));
}

#[test]
fn an_empty_plate_has_nothing_to_estimate() {
    assert_eq!(
        stack(&Scene::default(), &Slicing::default()),
        (0.0, (0, None))
    );
}

fn machine_with_a_ramp() -> Machine {
    let mut machine = Machine::default();
    let material = &mut machine.slicing.material;
    material.bottom_layers = 5;
    material.transition_layers = 5;
    material.bottom_exposure_s = 30.0;
    material.exposure_s = 5.2;
    machine
}

#[test]
fn a_transition_layer_reads_its_own_step_of_the_ramp() {
    let machine = machine_with_a_ramp();
    // Six equal steps from 30 s to 5.2 s, so the first transition layer is one down.
    let first = layer_exposure_s(&machine, 5);
    assert!(
        (first - (30.0 - 24.8 / 6.0)).abs() < 1e-3,
        "expected 25.867 s on the first transition layer, got {first}"
    );
    assert_eq!(
        layer_exposure_s(&machine, 4),
        30.0,
        "still the bottom block"
    );
    assert_eq!(layer_exposure_s(&machine, 10), 5.2, "past the ramp");
}

#[test]
fn the_layer_card_names_the_block_the_layer_is_in() {
    let machine = machine_with_a_ramp();
    assert_eq!(block(&machine, 4), "Bottom");
    assert_eq!(block(&machine, 5), "Transition");
    assert_eq!(block(&machine, 10), "Body");
}
