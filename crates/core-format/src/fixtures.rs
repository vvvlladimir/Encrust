//! A sample job the crate's own tests build on.

use std::path::Path;

use core_raster::{Grey, PixelPitch, RasterSettings, Shading};
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::{ExposurePlan, LayerPlan, PrintJob};

pub(crate) fn sample_printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(
        r#"
name = "Test"
manufacturer = "Test"

[display]
width_px = 8
height_px = 4
width_mm = 0.8
height_mm = 0.4

[build_volume]
x = 0.8
y = 0.4
z = 10.0
"#,
        Path::new("inline.toml"),
    )
    .expect("valid profile")
}

pub(crate) fn sample_job(layer_count: u32) -> PrintJob {
    PrintJob {
        printer: sample_printer(),
        material: MaterialProfile::default(),
        raster: RasterSettings {
            width_px: 8,
            height_px: 4,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        },
        plan: LayerPlan::of_count(
            MaterialProfile::default().layer_height_mm,
            layer_count as usize,
        ),
        volume_mm3: 1000.0,
        exposure: ExposurePlan::default(),
        thumbnail: None,
    }
}
