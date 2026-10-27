//! The archive's JSON entries and its two previews. Which keys, and the constants among
//! them, are in `docs/formats/pwsz.md`.

use core_format::{FormatError, PrintJob, Rgb, Thumbnail, encode_colour};
use serde_json::{Value, json};

use super::PwszFlavour;
use crate::rle::GREY_STEPS;

/// The two previews, in pixels, and the colour each is padded with.
const PREVIEWS: [(u32, u32, Rgb); 2] = [(224, 168, [2, 72, 100]), (336, 252, [20, 27, 41])];

/// Which resin the settings name: the code the vendor gives its standard resin.
const STANDARD_RESIN_CODE: &str = "10";

/// The name the vendor gives the one resin a file prints with, whatever it is called.
const USER_RESIN: &str = "user_resin";

/// The previews, smallest first, cut from the job's thumbnail.
pub(crate) fn previews(job: &PrintJob) -> Result<Vec<Vec<u8>>, FormatError> {
    PREVIEWS
        .iter()
        .map(|&(width, height, background)| {
            let image = match job.thumbnail.as_ref() {
                Some(thumbnail) => thumbnail.fitted_to(width, height, background),
                None => Thumbnail::filled(width, height, background),
            };
            encode_colour(&image)
        })
        .collect()
}

/// Which program wrote the archive.
pub(crate) fn software_info() -> Result<Vec<u8>, FormatError> {
    text(&json!({
        "mark": "Encrust",
        "opengl": "3.3-CoreProfile",
        "os": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Every layer's exposure, height and lift, which the firmware prints by.
pub(crate) fn layers_controller(job: &PrintJob) -> Result<Vec<u8>, FormatError> {
    let material = &job.material;
    let paras: Vec<Value> = (0..job.layer_count())
        .map(|index| {
            let (lift_mm, lift_mm_min) = if material.is_bottom_layer(index) {
                (
                    material.bottom_lift_distance_mm,
                    material.bottom_lift_speed_mm_min,
                )
            } else {
                (material.lift_distance_mm, material.lift_speed_mm_min)
            };
            let thickness_mm = job.layer_height_mm(index);
            json!({
                "exposure_time": job.exposure_of_layer_s(index),
                "layer_index": index,
                "layer_minheight": job.layer_z_mm(index) - thickness_mm,
                "layer_thickness": thickness_mm,
                "zup_height": lift_mm,
                "zup_speed": lift_mm_min / 60.0,
            })
        })
        .collect();
    text(&json!({ "count": paras.len(), "paras": paras }))
}

/// What the print costs and takes, as the machine shows it before it starts.
pub(crate) fn print_info(job: &PrintJob) -> Result<Vec<u8>, FormatError> {
    text(&json!({
        "cost": job.cost().unwrap_or(0.0),
        // The vendor's slicer states dollars here whatever the resin is priced in.
        "currency": "$",
        "print_time": job.print_time_s(),
        "volume": job.volume_mm3 / 1000.0,
    }))
}

/// The machine and the resin it prints with, in the shape the vendor's own settings take.
pub(crate) fn machine(job: &PrintJob, flavour: PwszFlavour) -> Result<Vec<u8>, FormatError> {
    let printer = &job.printer;
    let resin = user_resin(job, flavour);
    text(&json!({
        "machine_extern": {
            "active_resins": [resin["property"]["name"].clone()],
            "alias": printer.machine_name(),
            "cloud_property": 0,
            "device_cn_code": "",
            "factory_resins": [],
            "firmware_calc_exp_time_paras": firmware_exposure_constants(),
            "firmware_calc_print_time": 1,
            "firmware_calc_print_time_paras": firmware_motion_constants(),
            "picture": format!("{}.png", printer.machine_name().replace(' ', "_")),
            "user_resins": [resin],
            "version": "2",
        },
        "machine_type": machine_type(job, flavour),
        "version": "3",
    }))
}

fn machine_type(job: &PrintJob, flavour: PwszFlavour) -> Value {
    let (printer, raster) = (&job.printer, &job.raster);
    let (pitch_x_mm, pitch_y_mm) = printer.display.pixel_pitch_mm();
    let [(preview_w, preview_h, _), (preview2_w, preview2_h, _)] = PREVIEWS;
    let mut machine = json!({
        "child_screen": [{ "x": 0, "y": 0, "width": raster.width_px, "height": raster.height_px }],
        "cloudprev_back_color": [0.007_812_5, 0.281_25, 0.390_625],
        "cloudprev_imag_size": [800, 600],
        "key_image_format": "pwszImg",
        "key_suffix": template_suffix(flavour),
        "max_file_version": 518,
        "max_samples": 16,
        "name": printer.machine_name(),
        "prev2_back_color": [0.078_43, 0.105_88, 0.160_78],
        "prev2_image_size": [preview2_w, preview2_h],
        "prev_back_color": [0.007_812_5, 0.281_25, 0.390_625],
        "prev_image_size": [preview_w, preview_h],
        "prev_model_color": [0.804_687_5, 0.804_687_5, 0.804_687_5],
        "prev_supports_color": [0.074_218_75, 0.925_781_25, 0.929_687_5],
        "print_xsize": printer.display.width_mm,
        "print_ysize": printer.display.height_mm,
        "print_zsize": printer.build_volume.z,
        "property": 119,
        "raster_antialiasing": 4,
        "raster_segments_capacity": 100_000,
        "res_x": raster.width_px,
        "res_y": raster.height_px,
        "version": "3",
        "xy_pixel": pitch_x_mm * 1000.0,
        "xy_pixel_y": pitch_y_mm * 1000.0,
    });
    // The P1 Max's settings alone carry these: its plate turned half round, and the two
    // heights its plate is homed at.
    if flavour == PwszFlavour::Pp1m {
        machine["rotate_z"] = json!(180.0);
        machine["print_platform_zheights"] = json!([0.0, 35.0]);
    }
    machine
}

/// The suffix the settings name: the vendor's slicer keeps its template's, which is the M7
/// Pro's for every machine but the P1 Max.
fn template_suffix(flavour: PwszFlavour) -> &'static str {
    match flavour {
        PwszFlavour::Pp1m => "pp1m",
        _ => "pwsz",
    }
}

/// The job's resin, named the way the vendor's settings name the one a file prints with.
fn user_resin(job: &PrintJob, flavour: PwszFlavour) -> Value {
    let (material, stage) = (&job.material, &job.material.second_stage);
    let used = stage.is_used();
    let [normal_0, normal_1] = stages(
        [material.lift_distance_mm, material.lift_speed_mm_min],
        [stage.lift_distance_mm, stage.lift_speed_mm_min],
        [stage.retract_speed_mm_min, material.retract_speed_mm_min],
        used,
    );
    let [bott_0, bott_1] = stages(
        [
            material.bottom_lift_distance_mm,
            material.bottom_lift_speed_mm_min,
        ],
        [
            stage.bottom_lift_distance_mm,
            stage.bottom_lift_speed_mm_min,
        ],
        [
            stage.bottom_retract_speed_mm_min,
            material.bottom_retract_speed_mm_min,
        ],
        used,
    );
    let zdown_speed = normal_0["down_speed"].clone();
    json!({
        "depth_penetration_curve": penetration_constants(),
        "property": {
            "code": STANDARD_RESIN_CODE,
            "currency": material.details.currency,
            "density": material.density_g_cm3,
            "name": USER_RESIN,
            "price": material.details.price,
            "subfunc_code": 0,
            "target_temperature": 25,
            "type": "nor_resin_type",
            "version": "3",
            "volume": 1000,
        },
        "slice_extpara": {
            "exposure_compensate": 0,
            "intelli_mode": 1,
            "max_acceleration": 2,
            "multi_state_paras": {
                "bott_0": bott_0,
                "bott_1": bott_1,
                "normal_0": normal_0,
                "normal_1": normal_1,
            },
            "multi_state_used": 1,
            "separate_support_exposure_delayed": 0,
            "transition_layercount": material.transition_layers,
            "transition_type": 0,
            "version": "3",
        },
        "slicepara": slice_parameters(job, flavour, &zdown_speed),
        "version": "2",
    })
}

/// One lift as the two stages the vendor's slicer always states, in mm and mm/s: the first
/// lift with the second retract, then the second lift with the first, as `EXTRA` lays them.
/// Without a second stage it climbs nothing at the first's speeds, which a firmware
/// estimating time divides by.
fn stages(
    [first_mm, first_up_mm_min]: [f32; 2],
    [second_mm, second_up_mm_min]: [f32; 2],
    [second_down_mm_min, first_down_mm_min]: [f32; 2],
    used: bool,
) -> [Value; 2] {
    let per_s = |mm_min: f32| mm_min / 60.0;
    let stage = |height: f32, up: f32, down: f32| json!({ "height": height, "up_speed": per_s(up), "down_speed": per_s(down) });
    if used {
        [
            stage(first_mm, first_up_mm_min, second_down_mm_min),
            stage(second_mm, second_up_mm_min, first_down_mm_min),
        ]
    } else {
        [
            stage(first_mm, first_up_mm_min, first_down_mm_min),
            stage(0.0, first_up_mm_min, first_down_mm_min),
        ]
    }
}

/// The resin's single-stage settings, which the firmware prints by where a layer does
/// not state its own.
fn slice_parameters(job: &PrintJob, flavour: PwszFlavour, zdown_speed: &Value) -> Value {
    let material = &job.material;
    let mut parameters = json!({
        "anti_count": job.raster.anti_alias_level(GREY_STEPS),
        "blur_level": job.raster.stated_blur_px(),
        "bott_layers": material.bottom_layers,
        "bott_off_time": material.bottom_light_off_s(),
        "bott_time": material.bottom_exposure_s,
        "exposure_time": material.exposure_s,
        // The vendor's slicer states the edge blur here as well.
        "gray_level": job.raster.stated_blur_px(),
        "off_time": material.light_off_s(),
        "use_indivi_layerpara": u32::from(job.varies_by_layer()),
        "use_random_erode": 0,
        // The speed of the first stage's way down, as the vendor states it.
        "zdown_speed": zdown_speed.clone(),
        "zthick": job.nominal_height_mm(),
        "zup_height": material.lift_distance_mm,
        "zup_speed": material.lift_speed_mm_min / 60.0,
    });
    // The P1 Max's template carries a second bottom exposure and the rests around a lift.
    if flavour == PwszFlavour::Pp1m {
        let [before_s, after_s, _] = material.waits.rests_s();
        for (key, value) in [
            ("bott_time_dual", material.bottom_exposure_s),
            ("wait_before_lift", before_s),
            ("wait_after_lift", after_s),
            ("bott_wait_before_lift", before_s),
            ("bott_wait_after_lift", after_s),
        ] {
            parameters[key] = json!(value);
        }
    }
    parameters
}

/// How the resin cures with depth, as the vendor's settings state it for every resin.
fn penetration_constants() -> Value {
    json!({
        "current_tempcurve_selector": 0,
        "light_intensity": 9000,
        "safety_coefficient": 1.6,
        "temperature_coefficients": [
            { "temperature": 25, "x_coefficient": 197.78, "y_compensation": 1803.2 },
            { "temperature": 10, "x_coefficient": 184.27, "y_compensation": 1675.8 },
            { "temperature": 55, "x_coefficient": 166.76, "y_compensation": 1474.1 },
            { "temperature": 35, "x_coefficient": 161.19, "y_compensation": 1417.3 },
            { "temperature": 45, "x_coefficient": 167.31, "y_compensation": 1480.9 },
        ],
        "zthick_max": 0.2,
        "zthick_min": 0.01,
    })
}

/// What the firmware estimates exposure with; the vendor states the same for every machine.
fn firmware_exposure_constants() -> Value {
    json!({
        "energy_coeff": 0,
        "machine_exposure_ton": 0.4,
        "precision_coeff_value": [0.024, 0.01, -0.2],
        "precision_per_volume": 5,
        "precision_range_branch": [0, 5, 25],
    })
}

/// What the firmware estimates print time with; the vendor states the same for every
/// machine.
fn firmware_motion_constants() -> Value {
    json!({
        "FUNCTION": 1,
        "HEIGHT_COMPENSATE": [0, 0, 0, 0],
        "LAYER_COMPENSATE": [0, 0, 0, 0],
        "MACHINE_AXIS_STEPS_PER_UNIT": [100, 100, 3200, 94],
        "MACHINE_BLOCK_BUFFER_SIZE": 32,
        "MACHINE_DEFAULT_ACCELERATION": 1000,
        "MACHINE_DEFAULT_MINSEGMENTTIME": 20000,
        "MACHINE_DEFAULT_XYJERK": 20,
        "MACHINE_DEFAULT_ZJERK": 0.2,
        "MACHINE_GENERATE_FRAME_TIME": 450,
        "MACHINE_MAX_ACCELERATION": [1000, 1000, 160, 1000],
        "MACHINE_MAX_FEEDRATE": [200, 200, 20, 45],
        "MACHINE_MAX_STEP_FREQUENCY": 256_000,
        "MACHINE_MINIMUM_PLANNER_SPEED": 0.05,
        "MACHINE_MODE_ACCELERATION": [0, 0, 0, 0],
        "MACHINE_NOR_LAYER_DOWN_HEIGHT_DIV": 0.25,
        "MACHINE_NOR_LAYER_DOWN_SPEED_DIV": 0.5,
        "MACHINE_NOR_LAYER_UP_HEIGHT_DIV": 0.25,
        "MACHINE_NOR_LAYER_UP_SPEED_DIV": 0.5,
        "MACHINE_STEP_MUL": 1,
        "MACHINE_TIME_COMPENSATE": 0,
        "MACHINE_TIM_PRES": 30,
        "MACHINE_TIM_RCC_CLK": 60,
        "TIMES_COMPENSATE": [0, 0, 0, 0],
        "version": "2",
    })
}

fn text(value: &Value) -> Result<Vec<u8>, FormatError> {
    serde_json::to_vec_pretty(value).map_err(|source| FormatError::Encoding {
        what: "the archive's settings",
        reason: source.to_string(),
    })
}
