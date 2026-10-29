use super::*;
use crate::fixtures::sample_job;
use core_format::{ExposurePlan, ExposureRange};
use printer_profiles::{Mirror, WaitMode, Waits};

fn value_of<'a>(gcode: &'a str, key: &str) -> Option<&'a str> {
    gcode
        .lines()
        .find_map(|line| line.strip_prefix(&format!(";{key}:")))
}

#[test]
fn the_header_states_the_stack_the_blocks_go_on_to_run() {
    let job = sample_job(12);
    let text = preamble(&job);

    assert_eq!(value_of(&text, "totalLayer"), Some("12"));
    assert_eq!(value_of(&text, "resolutionX"), Some("8"));
    assert_eq!(value_of(&text, "machineType"), Some("Test"));
    assert_eq!(
        value_of(&text, "bottomLayExposureTime"),
        value_of(&text, "bottomLayerExposureTime"),
        "both keys carry the bottom exposure, for either generation of a reader"
    );
    assert_eq!(value_of(&text, "mirror"), Some("0"));
    assert_eq!(value_of(&text, "projectType"), Some("Normal"));
    assert!(text.contains("\nG90;Absolute positioning\n"));
}

#[test]
fn a_mirrored_panel_says_so_in_both_places_a_reader_looks() {
    let mut job = sample_job(1);
    job.printer.mirror = Mirror::LcdMirror;
    let text = preamble(&job);

    assert_eq!(value_of(&text, "mirror"), Some("1"));
    assert_eq!(value_of(&text, "projectType"), Some("LCD_mirror"));
}

#[test]
fn a_layer_shows_its_image_then_peels_and_cures_at_its_own_z() {
    let mut job = sample_job(3);
    job.material.bottom_layers = 0;
    job.material.exposure_s = 2.5;
    job.material.lift_distance_mm = 5.0;
    job.material.retract_distance_mm = 5.0;
    job.material.lift_speed_mm_min = 60.0;
    job.material.retract_speed_mm_min = 150.0;
    job.material.light_off_delay_s = 0.5;
    let z = job.layer_z_mm(1);

    let block = layer_block(&job, 1);
    let lines: Vec<&str> = block.lines().collect();
    assert_eq!(
        lines,
        vec![
            ";LAYER_START:1",
            ";PositionZ:0.1mm",
            "M6054 \"2.png\";Show image",
            &format!("G0 Z{} F60;Z Lift (1)", mm(z + 5.0)),
            "G0 Z0.1 F150;Retract (1)",
            "G4 P500;Wait before cure",
            "M106 S255;Turn LED ON",
            "G4 P2500;Cure time/delay",
            "M106 S0;Turn LED OFF",
            ";<Slice> Blank",
            ";LAYER_END",
            "",
        ]
    );
}

#[test]
fn a_retract_shorter_than_the_lift_still_ends_at_the_layers_own_z() {
    let mut job = sample_job(2);
    job.material.bottom_layers = 0;
    job.material.lift_distance_mm = 6.0;
    job.material.retract_distance_mm = 4.0;
    let block = layer_block(&job, 0);
    let z = mm(job.layer_z_mm(0));

    assert!(
        block.contains(&format!("G0 Z{} F", mm(job.layer_z_mm(0) + 2.0))),
        "the retract covers its own 4 mm of the 6 mm lift: {block}"
    );
    assert!(
        block.contains(&format!(
            "G0 Z{z} F{};Retract (2)",
            mm(job.material.retract_speed_mm_min)
        )),
        "a second move brings the plate down to {z} mm before the light goes on"
    );
}

#[test]
fn a_retract_matching_the_lift_is_one_move_and_not_two() {
    let mut job = sample_job(2);
    job.material.bottom_layers = 1;
    job.material.bottom_lift_distance_mm = 8.0;
    job.material.lift_distance_mm = 6.0;
    job.material.retract_distance_mm = 6.0;

    // The bottom layer retracts by its own lift distance, so neither layer needs two
    // moves — and z + d - d is a float step above z, which must not make one.
    for index in 0..2 {
        let block = layer_block(&job, index);
        assert!(
            !block.contains("Retract (2)"),
            "layer {index} comes down in one move: {block}"
        );
    }
}

#[test]
fn the_bottom_block_peels_and_cures_on_its_own_numbers() {
    let mut job = sample_job(4);
    job.material.bottom_layers = 2;
    job.material.bottom_exposure_s = 30.0;
    job.material.bottom_lift_distance_mm = 7.0;
    job.material.bottom_lift_speed_mm_min = 40.0;
    job.material.bottom_light_pwm = 200;
    job.material.transition_layers = 0;

    let first = layer_block(&job, 0);
    assert!(first.contains("F40;Z Lift (1)"));
    assert!(first.contains("M106 S200;Turn LED ON"));
    assert!(first.contains("G4 P30000;Cure time/delay"));

    let third = layer_block(&job, 2);
    assert!(!third.contains("F40;Z Lift (1)"));
    assert!(third.contains("M106 S255;Turn LED ON"));
}

#[test]
fn the_bottom_block_waits_its_own_light_off_delay() {
    let mut job = sample_job(4);
    job.material.bottom_layers = 2;
    job.material.light_off_delay_s = 0.5;
    job.material.bottom_light_off_delay_s = Some(5.0);
    let text = preamble(&job);

    assert_eq!(value_of(&text, "bottomLightOffTime"), Some("5"));
    assert_eq!(value_of(&text, "lightOffTime"), Some("0.5"));
    assert!(layer_block(&job, 1).contains("G4 P5000;Wait before cure"));
    assert!(layer_block(&job, 2).contains("G4 P500;Wait before cure"));
}

#[test]
fn the_header_states_transition_drop_speed_and_shrinkage() {
    let mut job = sample_job(4);
    job.material.transition_layers = 3;
    job.material.bottom_retract_speed_mm_min = 240.0;
    job.material.compensation.shrinkage_on = true;
    job.material.compensation.shrink_x_pct = 101.5;
    let text = preamble(&job);

    assert_eq!(value_of(&text, "transitionLayerCount"), Some("3"));
    assert_eq!(value_of(&text, "bottomLayerDropSpeed"), Some("240"));
    assert_eq!(value_of(&text, "shrinkageX"), Some("101.5"));
    assert_eq!(value_of(&text, "shrinkageZ"), Some("100"));
}

#[test]
fn the_opening_block_closes_with_the_marker_of_the_start_program() {
    let text = preamble(&sample_job(1));

    assert!(text.ends_with("G28 Z0;Home Z\n;START_GCODE_END\n"));
    assert!(!text.contains("END_GCODE_BEGIN"));
}

#[test]
fn a_band_of_height_reaches_the_layer_it_covers() {
    let mut job = sample_job(6);
    job.material.bottom_layers = 0;
    job.material.layer_height_mm = 1.0;
    job.plan = core_format::LayerPlan::of_count(1.0, 6);
    job.exposure = ExposurePlan::new(vec![ExposureRange::new(3.0, 5.0, 7.0)]);

    assert!(layer_block(&job, 3).contains("G4 P7000;Cure time/delay"));
    assert!(!layer_block(&job, 5).contains("G4 P7000;Cure time/delay"));
}

#[test]
fn a_resting_resin_gets_its_three_rests_around_the_peel() {
    let mut job = sample_job(2);
    job.material.lift_distance_mm = 5.0;
    job.material.waits = Waits {
        mode: WaitMode::Rest,
        before_lift_s: 1.0,
        after_lift_s: 0.5,
        after_retract_s: 2.0,
    };
    let block = layer_block(&job, 0);

    assert!(block.contains("G4 P1000;Rest before lift"));
    assert!(block.contains("G4 P500;Wait after lift"));
    assert!(block.contains("G4 P2000;Wait after retract"));
    assert!(
        !block.contains("Wait before cure"),
        "a resin that rests holds no light-off delay"
    );
}

#[test]
fn the_program_ends_with_the_plate_out_of_the_vat() {
    let job = sample_job(4);
    let text = epilogue(&job);

    assert!(text.contains("M106 S0;Turn LED OFF"));
    assert!(
        text.starts_with(";END_GCODE_BEGIN\n"),
        "the closing block opens with the marker of the end program"
    );
    assert!(
        text.contains(&format!(
            "G1 Z{} F25;Move Z",
            mm(job.printer.build_volume.z)
        )),
        "the plate climbs slowly to the top of the travel so the part can be taken off"
    );
    assert!(text.ends_with("M18;Disable motors\n;END_GCODE_END\n;<Completed>\n"));
}
