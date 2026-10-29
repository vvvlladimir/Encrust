//! The job the writer tests build files from: the resin of a known GK3 Ultra file, on a
//! panel small enough to check by hand.

use std::io::Cursor;
use std::path::Path;

use core_format::{ExposurePlan, LayerPlan, LayerSink, PrintJob, SlicedFileWriter};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};

use crate::{CtbEncryptedSink, CtbEncryptedWriter};
use printer_profiles::{MaterialProfile, PrinterProfile};

pub(crate) const WIDTH_PX: u32 = 16;
pub(crate) const HEIGHT_PX: u32 = 8;

/// A resin peeling in two stages with a bottom light-off of its own, every number distinct.
const TEST_RESIN: &str = r#"
name = "normal"
layer_height_mm = 0.05
exposure_s = 2.5
bottom_exposure_s = 30.0
bottom_layers = 2
transition_layers = 0
light_off_delay_s = 0.5
bottom_light_off_delay_s = 5.0
lift_distance_mm = 2.0
lift_speed_mm_min = 360.0
retract_distance_mm = 2.0
retract_speed_mm_min = 900.0
bottom_lift_distance_mm = 5.0
bottom_lift_speed_mm_min = 120.0
bottom_retract_speed_mm_min = 240.0
light_pwm = 255
bottom_light_pwm = 255
density_g_cm3 = 1.1

[waits]
mode = "light-off"

[second_stage]
lift_distance_mm = 4.0
lift_speed_mm_min = 900.0
retract_distance_mm = 4.0
retract_speed_mm_min = 360.0
bottom_lift_distance_mm = 3.0
bottom_lift_speed_mm_min = 180.0
bottom_retract_distance_mm = 3.0
bottom_retract_speed_mm_min = 180.0

[details]
kind = "normal"
price = 30.0
currency = "$"
price_per = "kilogram"
"#;

const PRINTER: &str = r#"
name = "Test panel"
manufacturer = "Test"
mirror_x = true

[display]
width_px = 16
height_px = 8
width_mm = 1.6
height_mm = 0.8

[build_volume]
x = 1.6
y = 0.8
z = 300.0

[firmware]
per_layer_settings = true
"#;

pub(crate) fn test_resin_job(layer_count: u32) -> PrintJob {
    let material = MaterialProfile::from_toml_str(TEST_RESIN, Path::new("test-resin.toml"))
        .expect("the inline resin is valid");
    PrintJob {
        printer: PrinterProfile::from_toml_str(PRINTER, Path::new("inline.toml"))
            .expect("the inline profile is valid"),
        plan: LayerPlan::of_count(material.layer_height_mm, layer_count as usize),
        material,
        raster: RasterSettings {
            width_px: WIDTH_PX,
            height_px: HEIGHT_PX,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            mirror_x: true,
            mirror_y: false,
            shading: Shading::Binary,
            grey: Grey::default(),
            blur_px: 0,
        },
        volume_mm3: 245.0,
        exposure: ExposurePlan::default(),
        thumbnail: None,
        created_unix_s: 0,
    }
}

/// A layer lighting sixteen pixels of the panel's second and third rows.
pub(crate) fn lit_layer() -> LayerRuns {
    let mut mask = LayerMask::new(WIDTH_PX, HEIGHT_PX);
    mask.pixels_mut()[20..36].fill(255);
    LayerRuns::from_mask(&mask)
}

/// `job` written as an encrypted `.ctb`, each of its layers `lit_layer`.
pub(crate) fn written(job: &PrintJob) -> Vec<u8> {
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = CtbEncryptedWriter
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for _ in 0..job.layer_count() {
            sink.push(CtbEncryptedSink::encode(&lit_layer()))
                .expect("a layer is written");
        }
        sink.finish(job.volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}
