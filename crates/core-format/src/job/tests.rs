use super::*;
use crate::ExposureRange;
use crate::fixtures::sample_job;
use printer_profiles::Compensation;

#[test]
fn a_job_is_stamped_with_the_time_its_caller_gave() {
    // 2024-02-29T13:45:01Z, whatever the clock of the machine running the test says.
    let job = PrintJob {
        created_unix_s: 1_709_214_301,
        ..sample_job(1)
    };
    assert_eq!(job.created_utc(), "2024-02-29 13:45:01");
    assert_eq!(job.created_minutes(), 1_709_214_301 / 60);
}

#[test]
fn height_is_layer_count_times_layer_height() {
    let job = PrintJob {
        material: MaterialProfile {
            layer_height_mm: 0.05,
            ..MaterialProfile::default()
        },
        ..sample_job(20)
    };
    assert!((job.height_mm() - 1.0).abs() < 1e-6);
    assert!((job.layer_z_mm(0) - 0.05).abs() < 1e-6);
    assert!((job.layer_z_mm(19) - 1.0).abs() < 1e-6);
}

#[test]
fn a_stack_lifted_off_the_plate_still_states_whole_microns() {
    // 0.05 mm bands starting 3.32 mm up: subtracting two f32 of that size leaves
    // 0.05000019, which a reader rejects as more than three decimals.
    let bounds: Vec<f32> = (0..=200).map(|layer| 3.32 + layer as f32 * 0.05).collect();
    let job = PrintJob {
        plan: LayerPlan::from_bounds(bounds, 13.32),
        ..sample_job(200)
    };

    // The three decimals the formats carry: a whole micron leaves nothing below them.
    for (stated, expected) in [
        (job.nominal_height_mm(), 0.05),
        (job.layer_height_mm(7), 0.05),
        (job.layer_z_mm(0), 3.37),
        (job.height_mm(), 13.32),
    ] {
        assert!(
            (stated * 1000.0).fract() == 0.0 && (stated - expected).abs() < 1e-6,
            "expected {expected} mm to the micron, got {stated}"
        );
    }
}

#[test]
fn weight_is_the_volume_times_the_density() {
    let job = PrintJob {
        volume_mm3: 2000.0,
        material: MaterialProfile {
            density_g_cm3: 1.1,
            ..MaterialProfile::default()
        },
        ..sample_job(10)
    };
    // 2000 mm3 is 2 cm3, so 2.2 g at 1.1 g/cm3.
    assert!((job.weight_g() - 2.2).abs() < 1e-5);
}

#[test]
fn a_band_of_height_replaces_the_resins_exposure() {
    let job = PrintJob {
        material: MaterialProfile {
            layer_height_mm: 1.0,
            bottom_layers: 2,
            exposure_s: 2.0,
            ..MaterialProfile::default()
        },
        // Layer tops run 1, 2, 3 ... mm, and the band covers 3 mm to 5 mm.
        plan: LayerPlan::of_count(1.0, 6),
        exposure: ExposurePlan::new(vec![ExposureRange::new(3.0, 5.0, 7.0)]),
        ..sample_job(6)
    };

    assert!((job.exposure_of_layer_s(2) - 7.0).abs() < 1e-6);
    assert!((job.exposure_of_layer_s(3) - 7.0).abs() < 1e-6);
    assert!(
        (job.exposure_of_layer_s(4) - 2.0).abs() < 1e-6,
        "the fifth layer tops out at 5 mm, which the band stops below"
    );
    assert!(job.varies_by_layer());
}

#[test]
fn a_band_does_not_reach_into_the_bottom_block() {
    let job = PrintJob {
        material: MaterialProfile {
            layer_height_mm: 1.0,
            bottom_layers: 2,
            bottom_exposure_s: 30.0,
            transition_layers: 1,
            exposure_s: 2.0,
            ..MaterialProfile::default()
        },
        plan: LayerPlan::of_count(1.0, 5),
        exposure: ExposurePlan::new(vec![ExposureRange::new(0.0, 100.0, 7.0)]),
        ..sample_job(5)
    };

    assert!((job.exposure_of_layer_s(0) - 30.0).abs() < 1e-6);
    assert!(
        (job.exposure_of_layer_s(2) - 16.0).abs() < 1e-5,
        "the one transition layer sits half way between 30 s and 2 s"
    );
    assert!((job.exposure_of_layer_s(3) - 7.0).abs() < 1e-6);
}

#[test]
fn print_time_counts_a_band_rather_than_the_resins_exposure() {
    let material = MaterialProfile {
        layer_height_mm: 1.0,
        bottom_layers: 0,
        exposure_s: 2.0,
        light_off_delay_s: 0.0,
        lift_distance_mm: 0.0,
        retract_distance_mm: 0.0,
        ..MaterialProfile::default()
    };
    let plain = PrintJob {
        material: material.clone(),
        plan: LayerPlan::of_count(1.0, 4),
        ..sample_job(4)
    };
    let banded = PrintJob {
        exposure: ExposurePlan::new(vec![ExposureRange::new(0.0, 100.0, 12.0)]),
        material,
        plan: LayerPlan::of_count(1.0, 4),
        ..sample_job(4)
    };

    assert_eq!(plain.print_time_s(), 8);
    assert_eq!(banded.print_time_s(), 48);
}

#[test]
fn a_layer_is_exposed_against_the_height_the_resin_was_measured_at() {
    let job = PrintJob {
        material: MaterialProfile {
            // Measured at 0.05 mm, which is the only thickness 4 s is known right for.
            layer_height_mm: 0.05,
            bottom_layers: 0,
            exposure_s: 4.0,
            ..MaterialProfile::default()
        },
        plan: LayerPlan::from_bounds(vec![0.0, 0.1, 0.15, 0.175], 0.175),
        ..sample_job(3)
    };

    // Jacobs at the assumed 0.1 mm depth: 4 s times exp(±0.05/0.1) and exp(-0.025/0.1).
    assert!(
        (job.exposure_of_layer_s(0) - 6.5949).abs() < 1e-3,
        "0.10 mm is half a penetration depth thicker than 0.05"
    );
    assert!(
        (job.exposure_of_layer_s(1) - 4.0).abs() < 1e-6,
        "0.05 mm is what was measured"
    );
    assert!(
        (job.exposure_of_layer_s(2) - 3.1152).abs() < 1e-3,
        "0.025 mm takes far more than half of what 0.05 mm took"
    );
    assert!(
        (job.header_exposure_s() - 6.5949).abs() < 1e-3,
        "the header states the thickest layer, so it states that layer's exposure"
    );
    assert!(!job.is_uniform());
    assert!(job.varies_by_layer());
}

#[test]
fn a_uniform_stack_cut_off_the_resins_height_is_compensated_throughout() {
    let job = PrintJob {
        material: MaterialProfile {
            layer_height_mm: 0.05,
            bottom_layers: 0,
            exposure_s: 4.0,
            ..MaterialProfile::default()
        },
        plan: LayerPlan::of_count(0.1, 4),
        ..sample_job(4)
    };

    for index in 0..4 {
        assert!((job.exposure_of_layer_s(index) - 6.5949).abs() < 1e-3);
    }
    assert!(
        (job.header_exposure_s() - 6.5949).abs() < 1e-3,
        "every layer agrees with the header, so nothing has to be read per layer"
    );
    assert!(!job.varies_by_layer());
}

#[test]
fn a_machine_that_reads_the_header_alone_is_never_asked_for_per_layer_tables() {
    let mut job = sample_job(10);
    job.material.transition_layers = 4;
    assert!(job.varies_by_layer(), "a ramp needs the per-layer tables");

    job.printer.firmware.per_layer_settings = false;
    assert!(
        !job.varies_by_layer(),
        "the machine ramps the bottom block off the header instead"
    );
}

#[test]
fn a_thin_bottom_layer_still_gets_the_exposure_that_sticks_it_to_the_plate() {
    let job = PrintJob {
        material: MaterialProfile {
            layer_height_mm: 0.05,
            bottom_layers: 2,
            bottom_exposure_s: 30.0,
            exposure_s: 4.0,
            ..MaterialProfile::default()
        },
        plan: LayerPlan::of_count(0.025, 4),
        ..sample_job(4)
    };

    assert!((job.exposure_of_layer_s(0) - 30.0).abs() < 1e-6);
    assert!((job.exposure_of_layer_s(2) - 3.1152).abs() < 1e-3);
}

#[test]
fn a_resin_that_states_a_penetration_depth_is_compensated_by_its_working_curve() {
    let job = PrintJob {
        material: MaterialProfile {
            layer_height_mm: 0.05,
            bottom_layers: 0,
            exposure_s: 4.0,
            penetration_depth_mm: Some(0.05),
            ..MaterialProfile::default()
        },
        plan: LayerPlan::from_bounds(vec![0.0, 0.1, 0.15], 0.15),
        ..sample_job(2)
    };

    // Jacobs: a layer one Dp thicker than the measured one takes e times its exposure.
    assert!((job.exposure_of_layer_s(0) - 4.0 * 1.0f32.exp()).abs() < 1e-5);
    assert!((job.exposure_of_layer_s(1) - 4.0).abs() < 1e-6);
}

#[test]
fn print_time_counts_the_bottom_layers_at_their_own_exposure() {
    let material = MaterialProfile {
        bottom_layers: 2,
        bottom_exposure_s: 30.0,
        exposure_s: 2.0,
        light_off_delay_s: 0.0,
        lift_distance_mm: 6.0,
        lift_speed_mm_min: 60.0,
        retract_distance_mm: 6.0,
        retract_speed_mm_min: 60.0,
        bottom_lift_distance_mm: 6.0,
        bottom_lift_speed_mm_min: 60.0,
        bottom_retract_speed_mm_min: 60.0,
        transition_layers: 0,
        ..MaterialProfile::default()
    };
    let job = PrintJob {
        material,
        ..sample_job(4)
    };

    // 2 * 30 s + 2 * 2 s exposure, plus 6 mm up and down at 1 mm/s on all four layers.
    assert_eq!(job.print_time_s(), (60.0 + 4.0 + 4.0 * 12.0) as u32);
}

#[test]
fn time_through_a_layer_counts_only_the_layers_up_to_it() {
    let job = PrintJob {
        material: MaterialProfile {
            bottom_layers: 2,
            bottom_exposure_s: 30.0,
            exposure_s: 2.0,
            light_off_delay_s: 0.0,
            lift_speed_mm_min: 0.0,
            retract_speed_mm_min: 0.0,
            bottom_lift_speed_mm_min: 0.0,
            bottom_retract_speed_mm_min: 0.0,
            transition_layers: 0,
            ..MaterialProfile::default()
        },
        ..sample_job(4)
    };

    assert_eq!(job.time_through_s(0), 0, "nothing printed takes no time");
    assert_eq!(job.time_through_s(1), 30, "one bottom layer at 30 s");
    assert_eq!(
        job.time_through_s(3),
        62,
        "two bottom layers and one at 2 s"
    );
    assert_eq!(
        job.time_through_s(99),
        job.print_time_s(),
        "past the stack, the print"
    );
}

#[test]
fn a_placeholder_speed_adds_no_travel_to_the_print_time() {
    let job = |distance_mm, speed_mm_min| PrintJob {
        material: MaterialProfile {
            bottom_layers: 0,
            exposure_s: 2.0,
            light_off_delay_s: 0.0,
            lift_distance_mm: distance_mm,
            lift_speed_mm_min: speed_mm_min,
            retract_distance_mm: distance_mm,
            retract_speed_mm_min: speed_mm_min,
            ..MaterialProfile::default()
        },
        ..sample_job(4)
    };

    // 0.05 mm at 0.05 mm/min would read as two minutes a layer; a tilting vat takes seconds.
    assert_eq!(
        job(0.05, 0.05).print_time_s(),
        8,
        "four layers of 2 s exposure"
    );
    assert_eq!(
        job(10.0, 1200.0).print_time_s(),
        12,
        "10 mm up and down at 1200 mm/min is 1 s a layer on top of the 2 s exposure"
    );
}

/// A print that came in under its estimate gives a negative correction, which has to
/// shorten the estimate rather than being clamped away; see BUG-47 and ADR 0144.
#[test]
fn a_negative_unaccounted_time_shortens_the_estimate_and_never_goes_under_zero() {
    let job = |layer_time_s| PrintJob {
        material: MaterialProfile {
            exposure_s: 2.0,
            bottom_layers: 0,
            light_off_delay_s: 0.0,
            lift_distance_mm: 0.1,
            retract_distance_mm: 0.1,
            compensation: Compensation {
                layer_time_s,
                ..Compensation::default()
            },
            ..MaterialProfile::default()
        },
        ..sample_job(10)
    };

    let plain = job(0.0).print_time_s();
    assert!(
        job(-1.0).print_time_s() < plain,
        "ten layers a second shorter"
    );
    assert_eq!(job(-1000.0).print_time_s(), 0, "and never a negative clock");
}
